//! Diagnóstico de un servidor (mods, plugins, Java y registros) y cambio de Java y memoria.

use super::{api, Daemon};
use merc_admin::diagnose::{self, Diagnosis, Inputs, LogSource};
use merc_admin::modmeta;
use merc_core::{Instance, InstanceId};
use merc_ipc::*;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const LOG_TAIL: u64 = 1024 * 1024;
/// Informes de error más antiguos que esto no se tienen en cuenta.
const CRASH_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);

fn tail(path: &Path) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    f.seek(SeekFrom::Start(len.saturating_sub(LOG_TAIL))).ok()?;
    let mut buf = vec![];
    f.read_to_end(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

fn when(t: SystemTime) -> String {
    let ms = t.duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|d| d.with_timezone(&chrono::Local).format("%d/%m %H:%M").to_string())
        .unwrap_or_default()
}

type JavaCache = Mutex<HashMap<(PathBuf, Option<SystemTime>), Option<u32>>>;

/// Versión real de un Java (se guarda por ruta y fecha del archivo para no repetirlo).
fn java_major(java: &Path) -> Option<u32> {
    static CACHE: OnceLock<JavaCache> = OnceLock::new();
    let key = (
        java.to_path_buf(),
        std::fs::metadata(java).and_then(|m| m.modified()).ok(),
    );
    let cache = CACHE.get_or_init(Default::default);
    if let Some(v) = cache.lock().unwrap_or_else(|p| p.into_inner()).get(&key) {
        return *v;
    }
    let v = merc_core::java::probe_java(java, Duration::from_secs(10))
        .ok()
        .map(|p| p.major);
    cache.lock().unwrap_or_else(|p| p.into_inner()).insert(key, v);
    v
}

impl Daemon {
    fn log_sources(&self, inst: &Instance) -> Vec<LogSource> {
        let mut out = vec![];
        let lines = self.sup.console_since(inst.id, 0);
        if !lines.is_empty() {
            out.push(LogSource {
                name: "consola (última sesión)".into(),
                text: lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n"),
            });
        }
        let latest = inst.root.join("logs").join("latest.log");
        if let (Some(text), Ok(m)) = (tail(&latest), std::fs::metadata(&latest)) {
            out.push(LogSource {
                name: format!("logs/latest.log ({})", m.modified().map(when).unwrap_or_default()),
                text,
            });
        }
        // El informe de error más reciente (si es de los últimos días).
        let newest = std::fs::read_dir(inst.root.join("crash-reports"))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("txt")))
            .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
            .filter(|(t, _)| t.elapsed().is_ok_and(|a| a < CRASH_MAX_AGE))
            .max_by_key(|(t, _)| *t);
        if let Some((t, p)) = newest {
            if let Some(text) = tail(&p) {
                out.push(LogSource {
                    name: format!(
                        "crash-reports/{} ({})",
                        p.file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        when(t)
                    ),
                    text,
                });
            }
        }
        out
    }

    /// Un puerto libre para proponer (no lo usa otro servidor registrado y se puede abrir ahora).
    fn suggest_port(&self, inst: &Instance) -> Option<u16> {
        let current = Self::server_port(inst);
        let used: Vec<u16> = self.others_ports(inst).iter().filter_map(|o| o.minecraft).collect();
        (25565u16..25600)
            .find(|p| Some(*p) != current && !used.contains(p) && std::net::TcpListener::bind(("0.0.0.0", *p)).is_ok())
    }

    pub(crate) fn diagnosis(&self, inst: &Instance) -> Diagnosis {
        let mods = modmeta::scan(&inst.root, "mods");
        let plugins = modmeta::scan(&inst.root, "plugins");
        let logs = self.log_sources(inst);
        let pc_mb =
            (*self.pc_load.lock().unwrap_or_else(|p| p.into_inner())).map(|(_, _, total)| total / (1024 * 1024));
        let inputs = Inputs {
            kind: inst.kind,
            mc_version: inst.mc_version.as_deref(),
            java_major: java_major(&inst.launch.java),
            max_memory_mb: inst.launch.max_memory_mb,
            pc_memory_mb: pc_mb,
            free_port: self.suggest_port(inst),
            mods: &mods,
            plugins: &plugins,
            logs: &logs,
        };
        diagnose::diagnose(&inputs, super::playit::now_ms())
    }

    pub(crate) fn handle_diagnose(self: &Arc<Self>, client: &str, body: RequestBody) -> Result<ResponseBody, ApiError> {
        use RequestBody as R;
        Ok(match body {
            R::Diagnose { id } => ResponseBody::Diagnosis(Box::new(self.diagnosis(&self.instance(id)?))),
            R::SetLaunch {
                id,
                java,
                min_memory_mb,
                max_memory_mb,
            } => self.set_launch(client, id, java, min_memory_mb, max_memory_mb)?,
            _ => return Err(api(ErrorCode::BadRequest, "Petición no válida")),
        })
    }

    fn set_launch(
        &self,
        client: &str,
        id: InstanceId,
        java: Option<PathBuf>,
        min: Option<u32>,
        max: Option<u32>,
    ) -> Result<ResponseBody, ApiError> {
        let inst = self.instance(id)?;
        self.require_stopped(id, "cambiar el Java o la memoria")?;
        if inst.launch.is_native() {
            return Err(api(
                ErrorCode::BadRequest,
                "Bedrock Dedicated Server no usa Java ni una memoria fija: no hay nada que cambiar aquí.",
            ));
        }
        let mut launch = inst.launch.clone();
        let mut detail = vec![];
        if let Some(j) = java {
            if !j.is_absolute() || !j.is_file() {
                return Err(api(
                    ErrorCode::BadRequest,
                    "Indica la ruta completa del ejecutable de Java.",
                ));
            }
            let probe = merc_core::java::probe_java(&j, Duration::from_secs(10))
                .map_err(|e| api(ErrorCode::BadRequest, format!("Ese Java no responde: {e}")))?;
            if let Some(req) = inst.mc_version.as_deref().and_then(merc_core::java::required_java) {
                if probe.major < req.min_major {
                    return Err(api(
                        ErrorCode::BadRequest,
                        format!(
                            "Ese es Java {}; Minecraft {} necesita Java {} o más nuevo.",
                            probe.major,
                            inst.mc_version.as_deref().unwrap_or("?"),
                            req.min_major
                        ),
                    ));
                }
            }
            detail.push(format!("java {} ({})", probe.major, j.display()));
            launch.java = j;
        }
        if let Some(m) = max {
            launch.max_memory_mb = m;
            detail.push(format!("memoria máxima {m} MB"));
        }
        if let Some(m) = min {
            launch.min_memory_mb = m;
        }
        if launch.min_memory_mb > launch.max_memory_mb {
            launch.min_memory_mb = launch.max_memory_mb;
        }
        launch
            .validate()
            .map_err(|e| api(ErrorCode::BadRequest, e.to_string()))?;
        let pc_mb =
            (*self.pc_load.lock().unwrap_or_else(|p| p.into_inner())).map(|(_, _, total)| total / (1024 * 1024));
        if let Some(total) = pc_mb {
            if launch.max_memory_mb as u64 > total * 9 / 10 {
                return Err(api(
                    ErrorCode::BadRequest,
                    format!(
                        "Tu PC tiene unos {} MB; deja memoria para el sistema (máximo {} MB).",
                        total,
                        total * 9 / 10
                    ),
                ));
            }
        }
        self.store
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .update_launch(id, &launch)
            .map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        self.audit(client, Some(id), "launch.set", &detail.join(", "));
        Ok(ResponseBody::Instance(self.instance(id)?))
    }
}
