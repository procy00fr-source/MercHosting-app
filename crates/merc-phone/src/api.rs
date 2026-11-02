//! Lo que la interfaz del celular pide para los servidores de ESTE celular, en un solo punto
//! (la app de Tauri solo pasa las peticiones y arranca el servicio de Android).

use crate::local::{CreateRequest, Phone, PhoneError};
use crate::playit::Playit;
use merc_install::{Flavor, Http, Sources};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Lo que hace la parte de Android (servicio en primer plano).
pub trait Host: Send + Sync {
    fn start_service(&self, id: &str, name: &str, spec_json: &str, fifo: &str) -> Result<(), String>;
    fn force_stop(&self) -> Result<(), String>;
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    Info,
    Versions { flavor: Flavor },
    Create { req: CreateRequest },
    Progress,
    Cancel,
    Servers,
    Status { id: String },
    Console { id: String },
    Start { id: String },
    Stop { id: String },
    ForceStop { id: String },
    Command { id: String, text: String },
    AcceptEula { id: String },
    SetPlayit { id: String, on: bool },
    SetHeap { id: String, heap_mb: u32 },
    Backup { id: String },
    Backups { id: String },
    Remove { id: String },
    Playit { id: Option<String> },
    PlayitLink,
    PlayitUnlink,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Progress {
    pub running: bool,
    pub step: String,
    pub done: u64,
    pub total: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
}

impl From<PhoneError> for ApiError {
    fn from(e: PhoneError) -> Self {
        let code = match &e {
            PhoneError::Invalid(_) => "invalid_request",
            PhoneError::Unsupported(_) => "unsupported",
            PhoneError::Io(_) => "io",
            PhoneError::Install(_) => "install",
        };
        ApiError {
            code,
            message: e.to_string(),
        }
    }
}

fn err(code: &'static str, message: impl Into<String>) -> ApiError {
    ApiError {
        code,
        message: message.into(),
    }
}

pub struct Api {
    pub phone: Phone,
    playit: Playit,
    http: Arc<Http>,
    src: Sources,
    progress: Mutex<Progress>,
    cancel: AtomicBool,
    playit_api: String,
}

fn to_value<T: Serialize>(v: T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

impl Api {
    pub fn new(phone: Phone, http: Http, src: Sources) -> Self {
        Self {
            phone,
            playit: Playit::default(),
            http: Arc::new(http),
            src,
            progress: Mutex::new(Progress::default()),
            cancel: AtomicBool::new(false),
            playit_api: merc_net::playit::API_BASE.to_string(),
        }
    }

    fn set_progress(&self, f: impl FnOnce(&mut Progress)) {
        f(&mut self.progress.lock().unwrap_or_else(|p| p.into_inner()));
    }

    pub fn handle(&self, host: &dyn Host, op: Op) -> Result<Value, ApiError> {
        let p = &self.phone;
        Ok(match op {
            Op::Info => to_value(p.info()),
            Op::Versions { flavor } => {
                if !crate::local::flavor_allowed(flavor) {
                    return Err(err("unsupported", "Ese tipo no se puede crear en el celular."));
                }
                let list = merc_install::list_versions(&self.http, &self.src, flavor).map_err(PhoneError::from)?;
                to_value(
                    list.into_iter()
                        .filter(|e| crate::assets::jre_major_for(&e.mc_version).is_some())
                        .collect::<Vec<_>>(),
                )
            }
            Op::Create { req } => {
                {
                    let mut g = self.progress.lock().unwrap_or_else(|p| p.into_inner());
                    if g.running {
                        return Err(err("busy", "Ya se está creando un servidor."));
                    }
                    *g = Progress {
                        running: true,
                        step: "Comprobando".into(),
                        ..Default::default()
                    };
                }
                self.cancel.store(false, Ordering::SeqCst);
                let r = p.create(
                    &self.http,
                    &self.src,
                    &req,
                    &mut |step, done, total| {
                        self.set_progress(|g| {
                            g.step = step.to_string();
                            g.done = done;
                            g.total = total;
                        })
                    },
                    &self.cancel,
                );
                self.set_progress(|g| {
                    g.running = false;
                    g.error = r.as_ref().err().map(|e| e.to_string());
                });
                to_value(r?)
            }
            Op::Progress => to_value(self.progress.lock().unwrap_or_else(|p| p.into_inner()).clone()),
            Op::Cancel => {
                self.cancel.store(true, Ordering::SeqCst);
                Value::Null
            }
            Op::Servers => to_value(p.list()),
            Op::Status { id } => to_value(p.status(&id)?),
            Op::Console { id } => to_value(p.console(&id, 64 * 1024)),
            Op::Start { id } => {
                let s = p.get(&id)?;
                let spec = p.launch_spec(&id)?;
                let json = serde_json::to_string(&spec).map_err(|e| err("internal", e.to_string()))?;
                // Consola nueva: el estado no debe leer el «Done» de la vez anterior.
                let _ = std::fs::write(&spec.log, "");
                host.start_service(&id, &s.name, &json, &spec.fifo.display().to_string())
                    .map_err(|e| err("android", e))?;
                Value::Null
            }
            Op::Stop { id } => {
                p.stop(&id)?;
                Value::Null
            }
            Op::ForceStop { id } => {
                p.get(&id)?;
                host.force_stop().map_err(|e| err("android", e))?;
                Value::Null
            }
            Op::Command { id, text } => {
                p.command(&id, &text)?;
                Value::Null
            }
            Op::AcceptEula { id } => {
                p.accept_eula(&id)?;
                Value::Null
            }
            Op::SetPlayit { id, on } => {
                if on && p.playit_exe().is_none() {
                    return Err(err(
                        "unsupported",
                        "Esta versión de la app no trae el agente de Playit.",
                    ));
                }
                to_value(p.set_playit(&id, on)?)
            }
            Op::SetHeap { id, heap_mb } => to_value(p.set_heap(&id, heap_mb)?),
            Op::Backup { id } => to_value(p.backup(&id)?),
            Op::Backups { id } => to_value(p.backups(&id)),
            Op::Remove { id } => json!({ "moved_to": p.remove(&id)?.display().to_string() }),
            Op::Playit { id } => {
                let port = match id {
                    Some(id) => p.status(&id)?.port,
                    None => None,
                };
                to_value(self.playit.info(&p.playit_paths(), p.playit_exe().is_some(), port))
            }
            Op::PlayitLink => {
                if p.playit_exe().is_none() {
                    return Err(err(
                        "unsupported",
                        "Esta versión de la app no trae el agente de Playit.",
                    ));
                }
                to_value(
                    self.playit
                        .start_claim(self.http.clone(), p.playit_paths(), self.playit_api.clone())
                        .map_err(|e| err("playit", e))?,
                )
            }
            Op::PlayitUnlink => {
                self.playit.unlink(&p.playit_paths()).map_err(|e| err("playit", e))?;
                Value::Null
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local::LocalServer;

    #[derive(Default)]
    struct FakeHost {
        started: Mutex<Vec<(String, String)>>,
    }

    impl Host for FakeHost {
        fn start_service(&self, id: &str, _name: &str, spec_json: &str, _fifo: &str) -> Result<(), String> {
            self.started.lock().unwrap().push((id.into(), spec_json.into()));
            Ok(())
        }
        fn force_stop(&self) -> Result<(), String> {
            Ok(())
        }
    }

    fn op(v: Value) -> Op {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn ops_parse_like_the_ui_sends_them() {
        assert!(matches!(op(json!({"op": "info"})), Op::Info));
        assert!(matches!(
            op(json!({"op": "versions", "flavor": "paper"})),
            Op::Versions { flavor: Flavor::Paper }
        ));
        assert!(matches!(
            op(json!({"op": "set_heap", "id": "a", "heap_mb": 2048})),
            Op::SetHeap { .. }
        ));
        let c = op(json!({"op": "create", "req": {
            "name": "S", "flavor": "fabric", "mc_version": "1.21.1", "heap_mb": 2048,
            "accept_eula": true, "warnings_accepted": ["bateria"]
        }}));
        assert!(matches!(c, Op::Create { .. }));
    }

    #[test]
    fn start_hands_the_spec_to_android() {
        let d = tempfile::tempdir().unwrap();
        let api = Api::new(
            Phone::new(d.path().into(), "aarch64", 8_192, None),
            Http::new(),
            Sources::default(),
        );
        let host = FakeHost::default();
        assert_eq!(api.handle(&host, Op::Servers).unwrap(), json!([]));
        assert_eq!(
            api.handle(
                &host,
                Op::Versions {
                    flavor: Flavor::Bedrock
                }
            )
            .unwrap_err()
            .code,
            "unsupported"
        );
        api.phone
            .register(LocalServer {
                id: "x".into(),
                name: "S".into(),
                flavor: Flavor::Paper,
                mc_version: "1.21.1".into(),
                java_major: 21,
                heap_mb: 2_048,
                jar: "paper.jar".into(),
                created_unix_ms: 0,
                playit: false,
            })
            .unwrap();
        std::fs::create_dir_all(api.phone.server_dir("x")).unwrap();
        let e = api.handle(&host, Op::Start { id: "x".into() }).unwrap_err();
        assert!(e.message.contains("EULA"));
        api.handle(&host, Op::AcceptEula { id: "x".into() }).unwrap();
        let jre = api.phone.jre_dir(21);
        std::fs::create_dir_all(jre.join("lib/server")).unwrap();
        std::fs::write(jre.join("lib/libjli.so"), b"").unwrap();
        std::fs::write(jre.join("lib/server/libjvm.so"), b"").unwrap();
        api.handle(&host, Op::Start { id: "x".into() }).unwrap();
        let started = host.started.lock().unwrap();
        assert_eq!(started.len(), 1);
        let spec: crate::jli::LaunchSpec = serde_json::from_str(&started[0].1).unwrap();
        assert!(spec.args.contains(&"-Xmx2048M".to_string()));
        assert_eq!(
            api.handle(
                &host,
                Op::SetPlayit {
                    id: "x".into(),
                    on: true
                }
            )
            .unwrap_err()
            .code,
            "unsupported"
        );
        let pl = api.handle(&host, Op::Playit { id: Some("x".into()) }).unwrap();
        assert_eq!(pl["available"], json!(false));
        assert_eq!(api.handle(&host, Op::Progress).unwrap()["running"], json!(false));
    }

    #[test]
    fn create_refuses_without_warnings_and_reports_progress() {
        let d = tempfile::tempdir().unwrap();
        let api = Api::new(
            Phone::new(d.path().into(), "aarch64", 8_192, None),
            Http::new(),
            Sources::default(),
        );
        let host = FakeHost::default();
        let req = CreateRequest {
            name: "S".into(),
            flavor: Flavor::Paper,
            mc_version: "1.21.1".into(),
            heap_mb: 2_048,
            accept_eula: true,
            warnings_accepted: vec![],
        };
        let e = api.handle(&host, Op::Create { req }).unwrap_err();
        assert_eq!(e.code, "invalid_request");
        let pr = api.handle(&host, Op::Progress).unwrap();
        assert_eq!(pr["running"], json!(false));
        assert!(pr["error"].as_str().unwrap().contains("avisos"));
    }
}
