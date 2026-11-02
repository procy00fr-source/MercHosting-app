//! Lado «compañero» sin supervisor: lo usa la app del celular, que no tiene servidores propios y
//! solo administra los de una PC (o servidor casero) a la que se unió con un código.
//!
//! Es la misma lógica que el supervisor de escritorio usa para hablar con la PC de un amigo
//! (`merc-supervisord/src/companions.rs`, sección «Compañero»): unirse con un código, guardar las
//! PCs conocidas, refrescar sus servidores y mandar peticiones que esa PC decide si permite.
//! Nada aquí abre puertos ni atiende conexiones entrantes: el celular solo llama.
//!
//! En la carpeta de datos de la app quedan:
//! - `llave-dispositivo`: la llave privada de este celular (32 bytes, permisos 0600 en Unix).
//! - `amigos.json`: las PCs conocidas (llave pública, nombre, servidores y permisos).

use crate::code::Invite;
use crate::wire::{WireRequest, WireResponse};
use crate::{fingerprint, hex, parse_hex32, Hints, Mode, Node};
use merc_ipc::remote::{RemoteHost, RemoteServer};
use merc_ipc::{ApiError, ErrorCode, RequestBody, ResponseBody, ResponseResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CALL_TIMEOUT: Duration = Duration::from_secs(25);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredHost {
    key: String,
    name: String,
    #[serde(default)]
    hints: Vec<SocketAddr>,
    #[serde(default)]
    relay: Option<String>,
    servers: Vec<RemoteServer>,
    #[serde(default)]
    host_admin: bool,
}

fn api(code: ErrorCode, msg: impl Into<String>) -> ApiError {
    ApiError::new(code, msg)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Nombre con el que esta app se presenta (1–40 caracteres, sin saltos de línea).
pub fn clean_name(name: &str) -> Result<String, ApiError> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 40 || n.chars().any(|c| c.is_control()) {
        return Err(api(
            ErrorCode::BadRequest,
            "Escribe un nombre para este dispositivo (hasta 40 caracteres).",
        ));
    }
    Ok(n.to_string())
}

fn unexpected(w: WireResponse) -> ApiError {
    match w {
        WireResponse::Denied(m) => api(ErrorCode::Forbidden, m),
        WireResponse::Result(r) => match *r {
            ResponseResult::Err(e) => e,
            ResponseResult::Ok(_) => api(ErrorCode::Internal, "Respuesta inesperada"),
        },
        _ => api(ErrorCode::Internal, "Respuesta inesperada"),
    }
}

fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, path)
}

/// Por PC: cuándo respondió bien por última vez (ms) y su último error.
type HostState = HashMap<String, (Option<i64>, Option<String>)>;

/// Compañero sin supervisor (app del celular).
pub struct Guest {
    dir: PathBuf,
    mode: Mode,
    node: Mutex<Option<Arc<Node>>>,
    /// Última respuesta buena y último error de cada PC (en memoria).
    state: Mutex<HostState>,
    /// Escrituras de `amigos.json` de una en una.
    lock: Mutex<()>,
}

impl Guest {
    pub fn new(dir: impl Into<PathBuf>, mode: Mode) -> Self {
        Self {
            dir: dir.into(),
            mode,
            node: Mutex::new(None),
            state: Mutex::default(),
            lock: Mutex::new(()),
        }
    }

    /// Llave privada de este dispositivo (se crea una vez y nunca sale de su carpeta).
    fn device_key(&self) -> Result<[u8; 32], ApiError> {
        let path = self.dir.join("llave-dispositivo");
        if let Ok(b) = std::fs::read(&path) {
            if let Ok(k) = <[u8; 32]>::try_from(b.as_slice()) {
                return Ok(k);
            }
        }
        let k = crate::new_device_key();
        write_private(&path, &k).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        Ok(k)
    }

    /// Huella de este dispositivo (la que ve la PC en su lista de compañeros).
    pub fn fingerprint(&self) -> Result<String, ApiError> {
        Ok(fingerprint(&crate::public_of(&self.device_key()?)))
    }

    fn node(&self) -> Result<Arc<Node>, ApiError> {
        let mut n = self.node.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(node) = n.as_ref() {
            return Ok(node.clone());
        }
        let node =
            Arc::new(Node::start(self.device_key()?, self.mode).map_err(|e| api(ErrorCode::Network, e.to_string()))?);
        *n = Some(node.clone());
        Ok(node)
    }

    fn friends(&self) -> Vec<StoredHost> {
        std::fs::read_to_string(self.dir.join("amigos.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn save_friends(&self, f: &[StoredHost]) -> Result<(), ApiError> {
        let text = serde_json::to_string_pretty(f).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        write_private(&self.dir.join("amigos.json"), text.as_bytes())
            .map_err(|e| api(ErrorCode::Internal, e.to_string()))
    }

    fn friend(&self, host: &str) -> Result<StoredHost, ApiError> {
        self.friends()
            .into_iter()
            .find(|h| h.key == host)
            .ok_or_else(|| api(ErrorCode::NotFound, "No tienes esa PC guardada."))
    }

    fn update_friend(
        &self,
        key: &str,
        name: Option<String>,
        servers: Vec<RemoteServer>,
        hints: Option<(Vec<SocketAddr>, Option<String>)>,
        host_admin: bool,
    ) -> Result<(), ApiError> {
        let _g = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        let mut list = self.friends();
        match list.iter_mut().find(|h| h.key == key) {
            Some(h) => {
                if let Some(n) = name {
                    h.name = n;
                }
                if let Some((a, r)) = hints {
                    h.hints = a;
                    h.relay = r;
                }
                h.servers = servers;
                h.host_admin = host_admin;
            }
            None => {
                let (a, r) = hints.unwrap_or_default();
                list.push(StoredHost {
                    key: key.to_string(),
                    name: name.unwrap_or_default(),
                    hints: a,
                    relay: r,
                    servers,
                    host_admin,
                })
            }
        }
        self.save_friends(&list)
    }

    fn call_host(&self, host: &StoredHost, req: &WireRequest) -> Result<WireResponse, ApiError> {
        let key = parse_hex32(&host.key).ok_or_else(|| api(ErrorCode::BadRequest, "PC no válida"))?;
        let node = self.node()?;
        let hints = Hints {
            addrs: host.hints.clone(),
            relay: host.relay.clone(),
        };
        let r = node.call(key, &hints, req, CALL_TIMEOUT);
        let mut st = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let e = st.entry(host.key.clone()).or_default();
        match r {
            Ok(w) => {
                e.0 = Some(now_ms());
                e.1 = None;
                Ok(w)
            }
            Err(err) => {
                let msg = format!(
                    "{err}. Esa PC tiene que estar encendida, con MercHosting abierto o en segundo plano y conectada a Internet."
                );
                e.1 = Some(msg.clone());
                Err(api(ErrorCode::Network, msg))
            }
        }
    }

    /// PCs guardadas, con el estado de la última conexión.
    pub fn hosts(&self) -> Vec<RemoteHost> {
        let st = self.state.lock().unwrap_or_else(|p| p.into_inner());
        self.friends()
            .into_iter()
            .map(|h| {
                let (ok, err) = st.get(&h.key).cloned().unwrap_or_default();
                RemoteHost {
                    fingerprint: parse_hex32(&h.key).map(|k| fingerprint(&k)).unwrap_or_default(),
                    host: h.key,
                    name: h.name,
                    servers: h.servers,
                    last_ok_unix_ms: ok,
                    last_error: err,
                    host_admin: h.host_admin,
                }
            })
            .collect()
    }

    /// Usar un código de invitación.
    pub fn join(&self, code: &str, name: &str) -> Result<Vec<RemoteHost>, ApiError> {
        let name = clean_name(name)?;
        let inv = Invite::decode(code).map_err(|e| api(ErrorCode::BadRequest, e.to_string()))?;
        if inv.host == crate::public_of(&self.device_key()?) {
            return Err(api(ErrorCode::BadRequest, "Ese código es de este mismo dispositivo."));
        }
        let host = StoredHost {
            key: hex(&inv.host),
            name: String::new(),
            hints: inv.addrs.clone(),
            relay: inv.relay.clone(),
            servers: vec![],
            host_admin: false,
        };
        let resp = self.call_host(
            &host,
            &WireRequest::Pair {
                secret: hex(&inv.secret),
                name,
            },
        )?;
        match resp {
            WireResponse::Servers {
                host_name,
                servers,
                host_admin,
            } => self.update_friend(
                &host.key,
                Some(host_name),
                servers,
                Some((inv.addrs, inv.relay)),
                host_admin,
            )?,
            w => return Err(unexpected(w)),
        }
        Ok(self.hosts())
    }

    /// Pide a esa PC sus servidores y permisos actuales.
    pub fn refresh(&self, host: &str) -> Result<Vec<RemoteHost>, ApiError> {
        let h = self.friend(host)?;
        match self.call_host(&h, &WireRequest::Servers)? {
            WireResponse::Servers {
                host_name,
                servers,
                host_admin,
            } => self.update_friend(&h.key, Some(host_name), servers, None, host_admin)?,
            WireResponse::Denied(m) => {
                self.update_friend(&h.key, None, vec![], None, false)?;
                self.state
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .entry(h.key.clone())
                    .or_default()
                    .1 = Some(m);
            }
            _ => {}
        }
        Ok(self.hosts())
    }

    /// Olvida una PC (en esa PC sigue apareciendo hasta que la quiten allí).
    pub fn forget(&self, host: &str) -> Result<Vec<RemoteHost>, ApiError> {
        {
            let _g = self.lock.lock().unwrap_or_else(|p| p.into_inner());
            let mut list = self.friends();
            list.retain(|h| h.key != host);
            self.save_friends(&list)?;
        }
        if let (Some(n), Some(k)) = (
            self.node.lock().unwrap_or_else(|p| p.into_inner()).clone(),
            parse_hex32(host),
        ) {
            n.drop_host(&k);
        }
        Ok(self.hosts())
    }

    /// Una petición normal a esa PC (esa PC decide si la permite).
    pub fn call(&self, host: &str, request: RequestBody) -> Result<ResponseBody, ApiError> {
        let h = self.friend(host)?;
        match self.call_host(
            &h,
            &WireRequest::Call {
                request: Box::new(request),
            },
        )? {
            WireResponse::Result(r) => match *r {
                ResponseResult::Ok(b) => Ok(b),
                ResponseResult::Err(e) => Err(e),
            },
            w => Err(unexpected(w)),
        }
    }

    /// Lo que la app del celular sabe responder (el resto no existe sin supervisor).
    pub fn handle(&self, body: RequestBody) -> Result<ResponseBody, ApiError> {
        use RequestBody as R;
        Ok(match body {
            R::JoinWithCode { code, name } => ResponseBody::RemoteHosts(self.join(&code, &name)?),
            R::RemoteHosts => ResponseBody::RemoteHosts(self.hosts()),
            R::RefreshHost { host } => ResponseBody::RemoteHosts(self.refresh(&host)?),
            R::ForgetHost { host } => ResponseBody::RemoteHosts(self.forget(&host)?),
            R::RemoteCall { host, request } => self.call(&host, *request)?,
            R::RemoteDownload { .. } | R::RemoteUpload { .. } => return Err(api(
                ErrorCode::BadRequest,
                "En el celular todavía no se pueden descargar ni subir archivos. Hazlo desde MercHosting en una PC.",
            )),
            _ => {
                return Err(api(
                    ErrorCode::BadRequest,
                    "En el celular MercHosting solo administra servidores de tus PCs.",
                ))
            }
        })
    }

    pub fn shutdown(&self) {
        if let Some(n) = self.node.lock().unwrap_or_else(|p| p.into_inner()).take() {
            if let Ok(n) = Arc::try_unwrap(n) {
                n.shutdown();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{new_device_key, public_of};
    use merc_core::InstanceId;
    use merc_ipc::remote::Perm;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// PC anfitriona mínima: acepta un código, lista un servidor y responde un `Ping`.
    fn host_pc(secret: [u8; 16], guest_seen: Arc<Mutex<Option<[u8; 32]>>>) -> (Node, Invite, InstanceId) {
        let host = Node::start(new_device_key(), Mode::Local).unwrap();
        let id = InstanceId::new();
        let paired = Arc::new(AtomicBool::new(false));
        host.serve(
            Arc::new(|| true),
            Arc::new(move |peer, req| match req {
                WireRequest::Pair { secret: s, name } if s == hex(&secret) && name == "Celular de Amir" => {
                    paired.store(true, Ordering::SeqCst);
                    *guest_seen.lock().unwrap() = Some(peer);
                    WireResponse::Servers {
                        host_name: "PC casera".into(),
                        servers: vec![RemoteServer {
                            id,
                            name: "Survival".into(),
                            perms: vec![Perm::Status, Perm::Start],
                        }],
                        host_admin: true,
                    }
                }
                WireRequest::Pair { .. } => WireResponse::Denied("Código no válido".into()),
                _ if !paired.load(Ordering::SeqCst) => WireResponse::Denied("No eres compañero".into()),
                WireRequest::Servers => WireResponse::Servers {
                    host_name: "PC casera".into(),
                    servers: vec![],
                    host_admin: true,
                },
                WireRequest::Call { request } => match *request {
                    RequestBody::Ping => WireResponse::Result(Box::new(ResponseResult::Ok(ResponseBody::Pong {
                        server_version: "1.0.0".into(),
                        supervisor_pid: 7,
                        protocol_id: String::new(),
                    }))),
                    _ => WireResponse::Denied("No tienes permiso para esto.".into()),
                },
                _ => WireResponse::Denied("no".into()),
            }),
        );
        let inv = Invite {
            host: host.public(),
            secret,
            addrs: host.hint_addrs(),
            relay: None,
        };
        (host, inv, id)
    }

    #[test]
    fn pairs_lists_calls_and_forgets_without_a_supervisor() {
        let dir = tempfile::tempdir().unwrap();
        let seen = Arc::new(Mutex::new(None));
        let (host, inv, id) = host_pc([7; 16], seen.clone());
        let g = Guest::new(dir.path(), Mode::Local);
        assert!(g.hosts().is_empty());
        assert!(g.join(&inv.encode(), "  ").is_err(), "sin nombre no se une");

        let hosts = g.join(&inv.encode(), "Celular de Amir").unwrap();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].name, "PC casera");
        assert!(hosts[0].host_admin);
        assert_eq!(hosts[0].servers[0].id, id);
        assert!(hosts[0].last_ok_unix_ms.is_some());
        // La PC ve la llave real de este dispositivo, la misma que da su huella.
        let key = std::fs::read(dir.path().join("llave-dispositivo")).unwrap();
        let key: [u8; 32] = key.try_into().unwrap();
        assert_eq!(seen.lock().unwrap().unwrap(), public_of(&key));
        assert_eq!(g.fingerprint().unwrap(), fingerprint(&public_of(&key)));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join("llave-dispositivo"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o077, 0, "la llave solo la lee la app");
        }

        let h = hosts[0].host.clone();
        let r = g
            .handle(RequestBody::RemoteCall {
                host: h.clone(),
                request: Box::new(RequestBody::Ping),
            })
            .unwrap();
        assert!(matches!(r, ResponseBody::Pong { supervisor_pid: 7, .. }));
        let e = g.call(&h, RequestBody::Shutdown).unwrap_err();
        assert_eq!(e.code, ErrorCode::Forbidden);
        // Lo que no existe sin supervisor se rechaza con un mensaje claro.
        assert!(g.handle(RequestBody::ListInstances).is_err());

        // Se guarda entre aperturas de la app.
        let g2 = Guest::new(dir.path(), Mode::Local);
        assert_eq!(g2.hosts().len(), 1);
        let after = g2.refresh(&h).unwrap();
        assert!(after[0].servers.is_empty() && after[0].host_admin);
        assert!(g2.forget(&h).unwrap().is_empty());
        assert!(g2.call(&h, RequestBody::Ping).is_err());
        g.shutdown();
        g2.shutdown();
        host.shutdown();
    }

    #[test]
    fn wrong_code_is_rejected_and_nothing_is_saved() {
        let dir = tempfile::tempdir().unwrap();
        let (host, mut inv, _) = host_pc([1; 16], Arc::new(Mutex::new(None)));
        inv.secret = [2; 16];
        let g = Guest::new(dir.path(), Mode::Local);
        let e = g.join(&inv.encode(), "Celular de Amir").unwrap_err();
        assert_eq!(e.code, ErrorCode::Forbidden);
        assert!(g.hosts().is_empty());
        assert!(g.join("MH1-basura", "Celular").is_err());
        g.shutdown();
        host.shutdown();
    }
}
