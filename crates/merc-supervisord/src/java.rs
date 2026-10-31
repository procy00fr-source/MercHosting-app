//! Instalador de Java: Eclipse Temurin (JRE) en la carpeta de datos de MercHosting.
//! Nada se instala en Windows: cada versión queda en `java/<versión>-jre/` y se usa por su ruta.

use super::{api, Daemon};
use merc_install::temurin;
use merc_install::Step;
use merc_ipc::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

impl Daemon {
    pub(crate) fn java_dir(&self) -> PathBuf {
        self.data_dir.join("java")
    }

    /// Java instalados por MercHosting (se suman a los que se buscan en la PC).
    pub(crate) fn managed_java(&self) -> Vec<PathBuf> {
        let exe = merc_core::java::java_exe_name();
        std::fs::read_dir(self.java_dir())
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path().join("bin").join(exe))
            .filter(|p| p.is_file())
            .collect()
    }

    fn temurin_api() -> String {
        if cfg!(debug_assertions) {
            if let Ok(v) = std::env::var("MERC_TEMURIN_API") {
                if merc_install::http::url_allowed(&v) {
                    return v;
                }
            }
        }
        temurin::API_BASE.to_string()
    }

    pub(crate) fn handle_java(self: &Arc<Self>, client: &str, body: RequestBody) -> Result<ResponseBody, ApiError> {
        let RequestBody::InstallJava { major } = body else {
            return Err(api(ErrorCode::BadRequest, "Petición no válida"));
        };
        if !(8..=40).contains(&major) {
            return Err(api(ErrorCode::BadRequest, "Versión de Java no válida."));
        }
        let job = self
            .jobs
            .create(format!("Java {major}"), self.java_dir().join(major.to_string()))
            .map_err(|e| api(ErrorCode::Conflict, e))?;
        let d = self.clone();
        let j = job.clone();
        let client = client.to_string();
        std::thread::spawn(move || {
            let r = d.install_java(&j, major);
            match r {
                Ok(name) => {
                    d.audit(&client, None, "java.install", &name);
                    j.finish_with(JobState::Succeeded, None, None, format!("{name} listo"));
                }
                Err(e) if j.cancel.load(std::sync::atomic::Ordering::SeqCst) => {
                    j.finish_with(
                        JobState::Cancelled,
                        None,
                        None,
                        format!("Descarga de Java cancelada ({e})"),
                    );
                }
                Err(e) => j.finish_with(
                    JobState::Failed,
                    Some(e),
                    None,
                    format!("No se pudo instalar Java {major}"),
                ),
            }
        });
        Ok(ResponseBody::Job(job.snapshot(0)))
    }

    fn install_java(&self, job: &crate::jobs::Job, major: u32) -> Result<String, String> {
        job.set_step(
            Step::Preparing,
            &format!("Consultando Eclipse Temurin {major} (Adoptium)"),
        );
        let v = self
            .http
            .get_json(&temurin::assets_url(&Self::temurin_api(), major))
            .map_err(|e| e.to_string())?;
        let rel = temurin::parse_latest(&v).map_err(|e| e.to_string())?;
        let root = self.java_dir();
        let target = root.join(rel.folder_name());
        let exe = merc_core::java::java_exe_name();
        if target.join("bin").join(exe).is_file() {
            job.log(format!("{} ya estaba instalado", rel.release_name));
            return Ok(format!("Java {}", rel.semver));
        }
        std::fs::create_dir_all(root.join("descargas")).map_err(|e| e.to_string())?;
        let zip = root.join("descargas").join(rel.file_name.replace(['/', '\\'], "_"));
        job.set_step(Step::Downloading, &format!("Descargando {}", rel.file_name));
        job.log(format!("Origen: {} (SHA-256 publicado por Adoptium)", rel.link));
        self.http
            .download(
                &rel.link,
                &zip,
                &rel.checksum(),
                rel.size,
                &mut |done, total| {
                    job.event(merc_install::InstallEvent::Download {
                        file: rel.file_name.clone(),
                        done,
                        total,
                    })
                },
                &job.cancel,
            )
            .map_err(|e| e.to_string())?;
        job.log("Huella SHA-256 correcta");
        job.set_step(Step::Installing, "Descomprimiendo");
        let staging = root.join(format!(".instalando-{}", uuid::Uuid::new_v4().simple()));
        let result = (|| {
            let java = temurin::extract(&zip, &staging).map_err(|e| e.to_string())?;
            job.set_step(Step::Finishing, "Comprobando que funciona");
            let probe = merc_core::java::probe_java(&java, Duration::from_secs(20))
                .map_err(|e| format!("el Java descargado no arranca: {e}"))?;
            if probe.major != major {
                return Err(format!("se esperaba Java {major} y es Java {}", probe.major));
            }
            // La carpeta raíz del zip pasa a ser la carpeta definitiva.
            let inner = java
                .parent()
                .and_then(|b| b.parent())
                .map(|p| p.to_path_buf())
                .ok_or("estructura inesperada")?;
            std::fs::rename(&inner, &target).map_err(|e| e.to_string())?;
            job.log(format!("Instalado en {}", target.display()));
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(&staging);
        let _ = std::fs::remove_file(&zip);
        result.map(|()| format!("Java {}", rel.semver))
    }
}
