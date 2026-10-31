//! Centro de actualizaciones: software del servidor (misma versión de Minecraft) con copia
//! completa previa, descarga verificada y vuelta atrás restaurando esa copia.

use super::{admin_err, api, install_err, Daemon};
use merc_admin::backups::{BackupReason, BackupScope};
use merc_core::detect::ServerKind;
use merc_core::launch::LaunchTarget;
use merc_core::Instance;
use merc_install::{Flavor, Step};
use merc_ipc::*;
use std::cmp::Ordering;
use std::sync::Arc;

/// Servidores de un solo archivo que se pueden actualizar cambiando el jar.
fn flavor_of(kind: ServerKind) -> Option<Flavor> {
    match kind {
        ServerKind::Paper => Some(Flavor::Paper),
        ServerKind::Purpur => Some(Flavor::Purpur),
        ServerKind::Fabric => Some(Flavor::Fabric),
        _ => None,
    }
}

fn why_not(kind: ServerKind) -> String {
    match kind {
        ServerKind::Forge | ServerKind::NeoForge | ServerKind::Quilt => {
            "Este tipo de servidor se actualiza con su instalador oficial, que reescribe librerías y scripts. MercHosting todavía no lo hace en una carpeta existente.".into()
        }
        ServerKind::Spigot => "Spigot se compila en tu PC con BuildTools; actualizarlo aquí todavía no está disponible.".into(),
        ServerKind::Vanilla => "Vanilla no tiene compilaciones: solo cambia con la versión de Minecraft, y cambiar de versión todavía no se hace desde aquí.".into(),
        _ => "MercHosting todavía no actualiza este tipo de servidor.".into(),
    }
}

/// Compara versiones con números separados por puntos («0.16.10» > «0.16.9», «130» > «98»).
pub(crate) fn cmp_versions(a: &str, b: &str) -> Option<Ordering> {
    let parse = |s: &str| -> Option<Vec<u64>> { s.split(['.', '-', '+']).map(|p| p.parse().ok()).collect() };
    Some(parse(a)?.cmp(&parse(b)?))
}

impl Daemon {
    fn server_update_info(&self, inst: &Instance) -> Result<ServerUpdateInfo, ApiError> {
        let base = ServerUpdateInfo {
            supported: false,
            reason: None,
            mc_version: inst.mc_version.clone(),
            current: inst.loader_version.clone(),
            latest: None,
            newer: false,
            experimental: false,
            notes: vec![],
        };
        let (Some(flavor), Some(mc)) = (flavor_of(inst.kind), inst.mc_version.as_deref()) else {
            return Ok(ServerUpdateInfo {
                reason: Some(if inst.mc_version.is_none() {
                    "No se sabe la versión de Minecraft de este servidor.".into()
                } else {
                    why_not(inst.kind)
                }),
                ..base
            });
        };
        if !matches!(inst.launch.target, LaunchTarget::Jar { .. }) {
            return Ok(ServerUpdateInfo {
                reason: Some(why_not(inst.kind)),
                ..base
            });
        }
        let r = merc_install::resolve(&self.http, &self.sources, flavor, mc).map_err(install_err)?;
        let newer = match (&r.loader_version, &inst.loader_version) {
            (Some(l), Some(c)) => cmp_versions(l, c).map_or(l != c, |o| o == Ordering::Greater),
            (Some(_), None) => true,
            _ => false,
        };
        Ok(ServerUpdateInfo {
            supported: true,
            latest: r.loader_version.clone(),
            newer,
            experimental: r.experimental,
            notes: r.notes.clone(),
            ..base
        })
    }

    pub(crate) fn server_update_info_for_alert(&self, inst: &Instance) -> Result<ServerUpdateInfo, ApiError> {
        self.server_update_info(inst)
    }

    pub(crate) fn handle_updates(self: &Arc<Self>, client: &str, body: RequestBody) -> Result<ResponseBody, ApiError> {
        match body {
            RequestBody::ServerUpdateCheck { id } => Ok(ResponseBody::ServerUpdate(Box::new(
                self.server_update_info(&self.instance(id)?)?,
            ))),
            RequestBody::ApplyServerUpdate { id, loader_version } => {
                let inst = self.instance(id)?;
                self.require_stopped(id, "actualizar el servidor")?;
                let Some(flavor) = flavor_of(inst.kind) else {
                    return Err(api(ErrorCode::BadRequest, why_not(inst.kind)));
                };
                let job = self
                    .jobs
                    .create(format!("Actualizar {}", inst.name), inst.root.clone())
                    .map_err(|e| api(ErrorCode::Conflict, e))?;
                let d = self.clone();
                let j = job.clone();
                let client = client.to_string();
                std::thread::spawn(move || {
                    let r = d.apply_server_update(&client, &j, &inst, flavor, &loader_version);
                    match r {
                        Ok(msg) => j.finish_with(JobState::Succeeded, None, Some(inst.id), msg),
                        Err(e) => j.finish_with(
                            JobState::Failed,
                            Some(e),
                            Some(inst.id),
                            "No se pudo actualizar; el servidor quedó como estaba".into(),
                        ),
                    }
                });
                Ok(ResponseBody::Job(job.snapshot(0)))
            }
            _ => Err(api(ErrorCode::BadRequest, "Petición no válida")),
        }
    }

    fn apply_server_update(
        &self,
        client: &str,
        job: &crate::jobs::Job,
        inst: &Instance,
        flavor: Flavor,
        want: &str,
    ) -> Result<String, String> {
        let mc = inst.mc_version.clone().ok_or("No se sabe la versión de Minecraft")?;
        let LaunchTarget::Jar { jar: old_jar } = inst.launch.target.clone() else {
            return Err(why_not(inst.kind));
        };
        job.set_step(Step::Preparing, "Comprobando la versión en la fuente oficial");
        let r = merc_install::resolve_pinned(&self.http, &self.sources, flavor, &mc, Some(want))
            .map_err(|e| e.to_string())?;
        if r.loader_version.as_deref() != Some(want) {
            return Err("La versión más nueva cambió mientras tanto. Vuelve a buscar actualizaciones.".into());
        }
        if r.installer_args.is_some() || !merc_install::catalog::valid_jar_name(&r.artifact.file_name) {
            return Err(why_not(inst.kind));
        }
        // 1. Copia completa y verificada antes de tocar nada.
        job.set_step(Step::Preparing, "Haciendo una copia completa antes de actualizar");
        let backup = self
            .make_backup(
                client,
                inst,
                BackupScope::Full,
                BackupReason::BeforeUpdate,
                Some(format!("Antes de actualizar a {want}")),
            )
            .map_err(|e| {
                format!(
                    "No se pudo hacer la copia previa, así que no se actualizó nada: {}",
                    e.message
                )
            })?;
        job.log(format!(
            "Copia previa: {} (verificada). Para volver atrás, restáurala en Copias de seguridad.",
            backup.id
        ));
        // 2. Descarga verificada del nuevo archivo.
        let new_name = r.artifact.file_name.clone();
        let dest = inst.root.join(&new_name);
        if old_jar.to_string_lossy().eq_ignore_ascii_case(&new_name) {
            return Err("La versión nueva tiene el mismo nombre de archivo que la instalada.".into());
        }
        job.set_step(Step::Downloading, &format!("Descargando {new_name}"));
        self.http
            .download(
                &r.artifact.url,
                &dest,
                &r.artifact.checksum,
                r.artifact.size,
                &mut |done, total| {
                    job.event(merc_install::InstallEvent::Download {
                        file: new_name.clone(),
                        done,
                        total,
                    })
                },
                &job.cancel,
            )
            .map_err(|e| e.to_string())?;
        job.log("Huella del archivo correcta");
        // 3. Cambiar el arranque y la versión registrada.
        job.set_step(Step::Configuring, "Usando la versión nueva");
        let mut launch = inst.launch.clone();
        launch.target = LaunchTarget::Jar {
            jar: new_name.clone().into(),
        };
        {
            let mut store = self.store.lock().unwrap_or_else(|p| p.into_inner());
            store.update_launch(inst.id, &launch).map_err(|e| e.to_string())?;
            store
                .update_loader_version(inst.id, Some(want))
                .map_err(|e| e.to_string())?;
        }
        // 4. El archivo viejo va a la papelera de MercHosting (se puede recuperar).
        if inst.root.join(&old_jar).is_file() {
            match self
                .explorer(inst)
                .map_err(|e| e.message)?
                .trash(&old_jar.to_string_lossy())
                .map_err(admin_err)
            {
                Ok(_) => job.log(format!("{} se movió a la papelera de MercHosting", old_jar.display())),
                Err(e) => job.log(format!(
                    "No se pudo mover {} a la papelera: {}",
                    old_jar.display(),
                    e.message
                )),
            }
        }
        self.audit(
            client,
            Some(inst.id),
            "server.update",
            &format!(
                "{} -> {want} (copia previa {})",
                inst.loader_version.as_deref().unwrap_or("?"),
                backup.id
            ),
        );
        Ok(format!("Actualizado a {want}. Copia previa: {}", backup.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_builds_and_loader_versions() {
        assert_eq!(cmp_versions("130", "98"), Some(Ordering::Greater));
        assert_eq!(cmp_versions("0.16.10", "0.16.9"), Some(Ordering::Greater));
        assert_eq!(cmp_versions("0.16.9", "0.16.9"), Some(Ordering::Equal));
        assert_eq!(cmp_versions("1.0-beta", "1.0"), None);
    }
}
