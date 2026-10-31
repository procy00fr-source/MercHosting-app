//! Compañeros entre PCs con MercHosting, sin cuentas ni servidor central.
//!
//! Anfitrión: crea códigos de invitación de un solo uso (se guarda solo su SHA-256), reconoce a
//! cada compañero por la llave de su PC y decide **cada** petición con sus permisos actuales
//! (denegar por defecto). Revocar corta la conexión al momento.
//!
//! Compañero: usa el código, guarda la PC del amigo y le reenvía peticiones; nunca decide qué
//! se permite.

use super::{api, Daemon};
use base64::Engine as _;
use merc_core::InstanceId;
use merc_ipc::remote::{
    self as rm, CompanionInfo, CompanionsInfo, InviteInfo, Need, NewInvite, Perm, RemoteHost, RemoteItem, RemoteServer,
};
use merc_ipc::*;
use merc_remote::code::Invite;
use merc_remote::wire::{WireRequest, WireResponse};
use merc_remote::{Mode, Node};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

const CALL_TIMEOUT: Duration = Duration::from_secs(25);
/// Intentos fallidos de código permitidos por PC en 10 minutos, y en total.
const MAX_FAILS_PER_PEER: usize = 5;
const MAX_FAILS_TOTAL: usize = 30;
/// Tamaño de cada trozo de un archivo que viaja entre PCs.
const CHUNK: usize = 1024 * 1024;
/// Lo preparado o a medio subir se borra si nadie lo termina en este tiempo.
const TRANSFER_TTL: Duration = Duration::from_secs(2 * 3600);

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

struct Outgoing {
    peer: [u8; 32],
    path: PathBuf,
    size: u64,
    created: Instant,
}

struct Incoming {
    peer: [u8; 32],
    id: InstanceId,
    dest_dir: String,
    path: PathBuf,
    size: u64,
    received: u64,
    created: Instant,
}

#[derive(Default)]
pub(crate) struct RemoteRuntime {
    node: Option<Arc<Node>>,
    error: Option<String>,
    fails: HashMap<[u8; 32], Vec<Instant>>,
    /// Tareas en curso que inició cada compañero (solo esas puede seguir o cancelar).
    jobs: HashMap<[u8; 32], HashSet<String>>,
    /// Estado de las últimas peticiones a cada amigo (en memoria).
    host_state: HashMap<String, (Option<i64>, Option<String>)>,
    /// Archivos preparados para que un compañero se los lleve.
    outgoing: HashMap<String, Outgoing>,
    /// Archivos que un compañero está subiendo.
    incoming: HashMap<String, Incoming>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredInvite {
    id: String,
    secret_sha256: String,
    server: InstanceId,
    perms: Vec<Perm>,
    created_at_unix_ms: i64,
    expires_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredCompanion {
    key: String,
    name: String,
    added_at_unix_ms: i64,
    #[serde(default)]
    last_seen_unix_ms: Option<i64>,
    grants: BTreeMap<InstanceId, Vec<Perm>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct HostFile {
    enabled: bool,
    #[serde(default)]
    invites: Vec<StoredInvite>,
    #[serde(default)]
    companions: Vec<StoredCompanion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredHost {
    key: String,
    name: String,
    #[serde(default)]
    hints: Vec<SocketAddr>,
    #[serde(default)]
    relay: Option<String>,
    servers: Vec<RemoteServer>,
}

fn now_ms() -> i64 {
    super::playit::now_ms()
}

fn sha_hex(b: &[u8]) -> String {
    merc_remote::hex(&Sha256::digest(b))
}

/// Comparación en tiempo constante.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn forbidden(msg: impl Into<String>) -> ApiError {
    api(ErrorCode::Forbidden, msg)
}

/// Peticiones que solo leen (no se anotan en el historial: la UI las repite cada segundo).
fn is_read(body: &RequestBody) -> bool {
    use RequestBody as R;
    matches!(
        body,
        R::Ping
            | R::ListInstances
            | R::Status { .. }
            | R::Metrics { .. }
            | R::AdminState { .. }
            | R::Health { .. }
            | R::Tasks { .. }
            | R::Console { .. }
            | R::Activity { .. }
            | R::Players { .. }
            | R::ListFiles { .. }
            | R::SavedItems { .. }
            | R::ReadFile { .. }
            | R::Properties { .. }
            | R::GameRules { .. }
            | R::Worlds { .. }
            | R::Backups { .. }
            | R::Connection { .. }
            | R::ContentInstalled { .. }
            | R::ContentPlatforms { .. }
            | R::ContentSearch { .. }
            | R::ContentProject { .. }
            | R::ContentPlan { .. }
            | R::ResourcePackSearch { .. }
            | R::ResourcePackProject { .. }
            | R::Job { .. }
    )
}

/// Quita rutas completas de esta PC de las respuestas (nombre de usuario de Windows, otras
/// carpetas…). Solo se tocan textos que son *enteros* una ruta absoluta.
fn redact(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::String(s) => {
            let b = s.as_bytes();
            let windows = b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/');
            if windows
                || s.starts_with("\\\\")
                || (s.starts_with('/') && s.matches('/').count() >= 2 && !s.contains('\n'))
            {
                *s = String::new();
            }
        }
        serde_json::Value::Array(a) => a.iter_mut().for_each(redact),
        serde_json::Value::Object(o) => o.values_mut().for_each(redact),
        _ => {}
    }
}

fn pc_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "PC con MercHosting".into())
}

fn clean_name(name: &str) -> Result<String, ApiError> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 32 || n.chars().any(|c| c.is_control()) {
        return Err(api(ErrorCode::BadRequest, "Pon un nombre de 1 a 32 caracteres."));
    }
    Ok(n.to_string())
}

fn sorted(mut p: Vec<Perm>) -> Vec<Perm> {
    p.sort();
    p.dedup();
    p
}

impl Daemon {
    fn remote_dir(&self) -> PathBuf {
        self.data_dir.join("companeros")
    }

    fn remote_mode() -> Mode {
        if cfg!(debug_assertions) && std::env::var("MERC_REMOTE_LOCAL").is_ok_and(|v| v == "1") {
            Mode::Local
        } else {
            Mode::Internet
        }
    }

    /// Llave privada de esta PC (se crea una vez; nunca sale de la carpeta de datos).
    fn device_key(&self) -> Result<[u8; 32], ApiError> {
        let path = self.remote_dir().join("llave-dispositivo");
        if let Ok(b) = std::fs::read(&path) {
            if let Ok(k) = <[u8; 32]>::try_from(b.as_slice()) {
                return Ok(k);
            }
        }
        std::fs::create_dir_all(self.remote_dir()).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        let k = merc_remote::new_device_key();
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, k).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        std::fs::rename(&tmp, &path).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        Ok(k)
    }

    fn host_file(&self) -> HostFile {
        std::fs::read_to_string(self.remote_dir().join("anfitrion.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn save_host_file(&self, f: &HostFile) -> Result<(), ApiError> {
        write_json(&self.remote_dir().join("anfitrion.json"), f)
    }

    fn friends(&self) -> Vec<StoredHost> {
        std::fs::read_to_string(self.remote_dir().join("amigos.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn save_friends(&self, f: &[StoredHost]) -> Result<(), ApiError> {
        write_json(&self.remote_dir().join("amigos.json"), &f)
    }

    /// Pone en marcha el canal (una sola vez) y atiende a compañeros si está permitido.
    fn node(self: &Arc<Self>) -> Result<Arc<Node>, ApiError> {
        let mut rt = self.remote.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(n) = &rt.node {
            return Ok(n.clone());
        }
        let key = self.device_key()?;
        let node = match Node::start(key, Self::remote_mode()) {
            Ok(n) => Arc::new(n),
            Err(e) => {
                rt.error = Some(e.to_string());
                return Err(api(ErrorCode::Network, e.to_string()));
            }
        };
        let weak: Weak<Daemon> = Arc::downgrade(self);
        let gate_weak = weak.clone();
        node.serve(
            Arc::new(move || {
                gate_weak
                    .upgrade()
                    .is_some_and(|d| d.companions_enabled.load(Ordering::SeqCst))
            }),
            Arc::new(move |peer, req| match weak.upgrade() {
                Some(d) => d.remote_handle(peer, req),
                None => WireResponse::Denied("MercHosting se está cerrando.".into()),
            }),
        );
        self.log.line(&format!(
            "compañeros: canal en marcha (huella {})",
            merc_remote::fingerprint(&node.public())
        ));
        rt.node = Some(node.clone());
        rt.error = None;
        Ok(node)
    }

    /// Al arrancar el supervisor: si compartir estaba activado, se abre el canal.
    pub(crate) fn companions_boot(self: &Arc<Self>) {
        // Restos de transferencias de una sesión anterior.
        let _ = std::fs::remove_dir_all(self.remote_dir().join("salida"));
        let _ = std::fs::remove_dir_all(self.remote_dir().join("entrada"));
        let enabled = self.host_file().enabled;
        self.companions_enabled.store(enabled, Ordering::SeqCst);
        if enabled || !self.friends().is_empty() {
            let _ = self.node();
        }
    }

    // ───────────────────────── Anfitrión ─────────────────────────

    fn companion_by_key<'a>(f: &'a HostFile, peer: &[u8; 32]) -> Option<&'a StoredCompanion> {
        let k = merc_remote::hex(peer);
        f.companions.iter().find(|c| c.key == k)
    }

    fn servers_of(&self, c: &StoredCompanion) -> Vec<RemoteServer> {
        c.grants
            .iter()
            .filter(|(_, p)| !p.is_empty())
            .filter_map(|(id, perms)| {
                self.instance(*id).ok().map(|inst| RemoteServer {
                    id: *id,
                    name: inst.name,
                    perms: perms.clone(),
                })
            })
            .collect()
    }

    /// Una petición que llegó de la PC de un compañero.
    fn remote_handle(self: &Arc<Self>, peer: [u8; 32], req: WireRequest) -> WireResponse {
        match req {
            WireRequest::Pair { secret, name } => self.remote_pair(peer, &secret, &name),
            WireRequest::Servers => {
                let _g = self.companions_lock.lock().unwrap_or_else(|p| p.into_inner());
                let mut f = self.host_file();
                let key = merc_remote::hex(&peer);
                let Some(c) = f.companions.iter_mut().find(|c| c.key == key) else {
                    return WireResponse::Denied("Esta PC ya no tiene acceso a los servidores de tu amigo.".into());
                };
                c.last_seen_unix_ms = Some(now_ms());
                let c = c.clone();
                let _ = self.save_host_file(&f);
                WireResponse::Servers {
                    host_name: pc_name(),
                    servers: self.servers_of(&c),
                }
            }
            WireRequest::Call { request } => {
                WireResponse::Result(Box::new(match self.remote_call_in(peer, *request) {
                    Ok(b) => ResponseResult::Ok(b),
                    Err(e) => ResponseResult::Err(e),
                }))
            }
            WireRequest::Prepare { item } => match self.remote_prepare(peer, item) {
                Ok(w) => w,
                Err(e) => WireResponse::Result(Box::new(ResponseResult::Err(e))),
            },
            WireRequest::Chunk { token, offset } => match self.remote_chunk(peer, &token, offset) {
                Ok(w) => w,
                Err(e) => WireResponse::Result(Box::new(ResponseResult::Err(e))),
            },
            WireRequest::Upload {
                upload_id,
                id,
                dest_dir,
                name,
                size,
                offset,
                data,
                sha256,
            } => match self.remote_upload_in(peer, upload_id, id, dest_dir, &name, size, offset, &data, sha256) {
                Ok(w) => w,
                Err(e) => WireResponse::Result(Box::new(ResponseResult::Err(e))),
            },
            WireRequest::Invalid(e) => WireResponse::Denied(format!("Mensaje no válido: {e}")),
        }
    }

    /// Comprueba que el compañero sigue teniendo ese permiso en ese servidor.
    fn grant_ok(&self, peer: &[u8; 32], id: InstanceId, perm: Perm, action: &str) -> Result<String, ApiError> {
        if !self.companions_enabled.load(Ordering::SeqCst) {
            return Err(forbidden("Tu amigo desactivó el acceso de compañeros."));
        }
        let f = self.host_file();
        let Some(c) = Self::companion_by_key(&f, peer) else {
            return Err(forbidden("Esta PC ya no tiene acceso a los servidores de tu amigo."));
        };
        let who = format!("companero:{}", c.name);
        if !c.grants.get(&id).is_some_and(|p| p.contains(&perm)) {
            self.audit(&who, Some(id), "companero.denegado", action);
            return Err(forbidden(format!(
                "No tienes el permiso «{}» en este servidor.",
                perm_label(perm)
            )));
        }
        Ok(who)
    }

    fn expire_transfers(&self) {
        let mut rt = self.remote.lock().unwrap_or_else(|p| p.into_inner());
        let old: Vec<String> = rt
            .outgoing
            .iter()
            .filter(|(_, o)| o.created.elapsed() > TRANSFER_TTL)
            .map(|(k, _)| k.clone())
            .collect();
        for k in old {
            if let Some(o) = rt.outgoing.remove(&k) {
                let _ = std::fs::remove_file(o.path);
            }
        }
        let old: Vec<String> = rt
            .incoming
            .iter()
            .filter(|(_, i)| i.created.elapsed() > TRANSFER_TTL)
            .map(|(k, _)| k.clone())
            .collect();
        for k in old {
            if let Some(i) = rt.incoming.remove(&k) {
                let _ = std::fs::remove_dir_all(i.path.parent().unwrap_or(&i.path));
            }
        }
    }

    /// Deja listo en la carpeta de datos (fuera del servidor) lo que el compañero se lleva.
    fn remote_prepare(self: &Arc<Self>, peer: [u8; 32], item: RemoteItem) -> Result<WireResponse, ApiError> {
        let id = item.server();
        let who = self.grant_ok(&peer, id, item.perm(), "descargar")?;
        self.expire_transfers();
        {
            let rt = self.remote.lock().unwrap_or_else(|p| p.into_inner());
            if rt.outgoing.values().filter(|o| o.peer == peer).count() >= 3 {
                return Err(api(
                    ErrorCode::Conflict,
                    "Ya tienes 3 descargas a medias. Espera a que terminen.",
                ));
            }
        }
        let dir = self.remote_dir().join("salida");
        std::fs::create_dir_all(&dir).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        let token = uuid::Uuid::new_v4().simple().to_string();
        let dest = dir.join(&token);
        let (name, body) = match item {
            RemoteItem::File { id, path } => (
                path.rsplit(['/', '\\']).next().unwrap_or("archivo").to_string(),
                RequestBody::DownloadFile {
                    id,
                    path,
                    dest: dest.clone(),
                },
            ),
            RemoteItem::World { id, name } => (
                format!("{name}.zip"),
                RequestBody::ExportWorld {
                    id,
                    name,
                    dest: dest.clone(),
                },
            ),
            RemoteItem::Backup { id, backup } => (
                format!("{backup}.zip"),
                RequestBody::DownloadBackup {
                    id,
                    backup_id: backup,
                    dest: dest.clone(),
                },
            ),
            RemoteItem::Console { id } => (
                "consola.log".to_string(),
                RequestBody::ExportConsole { id, dest: dest.clone() },
            ),
        };
        // Los mismos controles que en el uso local (rutas dentro del servidor, mundo detenido…).
        if let Err(e) = self.handle(&who, body) {
            let _ = std::fs::remove_file(&dest);
            return Err(e);
        }
        let mut f = std::fs::File::open(&dest).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        let mut h = Sha256::new();
        let size = std::io::copy(&mut f, &mut h).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        let sha256 = merc_remote::hex(&h.finalize());
        self.remote.lock().unwrap_or_else(|p| p.into_inner()).outgoing.insert(
            token.clone(),
            Outgoing {
                peer,
                path: dest,
                size,
                created: Instant::now(),
            },
        );
        Ok(WireResponse::Prepared {
            token,
            name,
            size,
            sha256,
        })
    }

    fn remote_chunk(&self, peer: [u8; 32], token: &str, offset: u64) -> Result<WireResponse, ApiError> {
        use std::io::{Read, Seek, SeekFrom};
        let (path, size) = {
            let rt = self.remote.lock().unwrap_or_else(|p| p.into_inner());
            match rt.outgoing.get(token) {
                Some(o) if o.peer == peer => (o.path.clone(), o.size),
                _ => {
                    return Err(api(
                        ErrorCode::NotFound,
                        "Esa descarga ya no existe. Vuelve a empezarla.",
                    ))
                }
            }
        };
        if !self.companions_enabled.load(Ordering::SeqCst) {
            return Err(forbidden("Tu amigo desactivó el acceso de compañeros."));
        }
        if offset > size {
            return Err(api(ErrorCode::BadRequest, "Posición no válida"));
        }
        let mut f = std::fs::File::open(&path).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        f.seek(SeekFrom::Start(offset))
            .map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        let mut buf = vec![0u8; CHUNK.min((size - offset) as usize)];
        f.read_exact(&mut buf)
            .map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        if offset + buf.len() as u64 >= size {
            // Último trozo: ya no hace falta guardarlo.
            if let Some(o) = self
                .remote
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .outgoing
                .remove(token)
            {
                let _ = std::fs::remove_file(o.path);
            }
        }
        Ok(WireResponse::Chunk {
            data: b64().encode(&buf),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn remote_upload_in(
        self: &Arc<Self>,
        peer: [u8; 32],
        upload_id: Option<String>,
        id: InstanceId,
        dest_dir: String,
        name: &str,
        size: u64,
        offset: u64,
        data: &str,
        sha256: Option<String>,
    ) -> Result<WireResponse, ApiError> {
        use std::io::Write;
        let who = self.grant_ok(&peer, id, Perm::FilesEdit, "subir")?;
        let bytes = b64()
            .decode(data)
            .map_err(|_| api(ErrorCode::BadRequest, "Trozo dañado"))?;
        if bytes.len() > CHUNK {
            return Err(api(ErrorCode::BadRequest, "Trozo demasiado grande"));
        }
        let uid = match upload_id {
            None => {
                self.require_stopped(id, "subir archivos")?;
                if size > merc_admin::files::MAX_UPLOAD_BYTES {
                    return Err(api(
                        ErrorCode::BadRequest,
                        "El archivo es demasiado grande (máximo 2 GB).",
                    ));
                }
                let n = name.trim();
                if n.is_empty()
                    || n.len() > 200
                    || n == "."
                    || n == ".."
                    || n.contains(['/', '\\', ':'])
                    || n.chars().any(|c| c.is_control())
                {
                    return Err(api(ErrorCode::BadRequest, "Nombre de archivo no válido"));
                }
                if offset != 0 {
                    return Err(api(ErrorCode::BadRequest, "La subida debe empezar por el principio"));
                }
                self.expire_transfers();
                let uid = uuid::Uuid::new_v4().simple().to_string();
                let dir = self.remote_dir().join("entrada").join(&uid);
                std::fs::create_dir_all(&dir).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
                let mut rt = self.remote.lock().unwrap_or_else(|p| p.into_inner());
                if rt.incoming.values().filter(|i| i.peer == peer).count() >= 2 {
                    let _ = std::fs::remove_dir_all(&dir);
                    return Err(api(
                        ErrorCode::Conflict,
                        "Ya tienes 2 subidas a medias. Espera a que terminen.",
                    ));
                }
                rt.incoming.insert(
                    uid.clone(),
                    Incoming {
                        peer,
                        id,
                        dest_dir,
                        path: dir.join(n),
                        size,
                        received: 0,
                        created: Instant::now(),
                    },
                );
                uid
            }
            Some(u) => u,
        };
        let (path, total, received, dest_dir) = {
            let mut rt = self.remote.lock().unwrap_or_else(|p| p.into_inner());
            let Some(i) = rt.incoming.get_mut(&uid).filter(|i| i.peer == peer && i.id == id) else {
                return Err(api(ErrorCode::NotFound, "Esa subida ya no existe. Vuelve a empezarla."));
            };
            if offset != i.received || i.received + bytes.len() as u64 > i.size {
                return Err(api(ErrorCode::BadRequest, "Trozo fuera de orden"));
            }
            i.received += bytes.len() as u64;
            (i.path.clone(), i.size, i.received, i.dest_dir.clone())
        };
        let cleanup = |d: &Daemon| {
            d.remote.lock().unwrap_or_else(|p| p.into_inner()).incoming.remove(&uid);
            let _ = std::fs::remove_dir_all(path.parent().unwrap_or(&path));
        };
        let write = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut f| f.write_all(&bytes));
        if let Err(e) = write {
            cleanup(self);
            return Err(api(ErrorCode::Internal, e.to_string()));
        }
        let Some(expected) = sha256 else {
            return Ok(WireResponse::UploadProgress {
                upload_id: uid,
                received,
            });
        };
        // Último trozo: se comprueba completo y con su huella antes de tocar el servidor.
        let check = (|| {
            if received != total {
                return Err(api(ErrorCode::BadRequest, "La subida llegó incompleta."));
            }
            let mut f = std::fs::File::open(&path).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
            let mut h = Sha256::new();
            std::io::copy(&mut f, &mut h).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
            if !same(&merc_remote::hex(&h.finalize()), expected.trim()) {
                return Err(api(
                    ErrorCode::BadRequest,
                    "El archivo llegó dañado (no coincide su huella). Vuelve a subirlo.",
                ));
            }
            self.handle(
                &who,
                RequestBody::UploadFile {
                    id,
                    source: path.clone(),
                    dest_dir,
                },
            )
            .map(|(b, _)| b)
        })();
        cleanup(self);
        check.map(|b| WireResponse::Result(Box::new(ResponseResult::Ok(b))))
    }

    fn remote_pair(self: &Arc<Self>, peer: [u8; 32], secret: &str, name: &str) -> WireResponse {
        let deny =
            WireResponse::Denied("El código no es válido, ya se usó o caducó. Pide a tu amigo uno nuevo.".into());
        {
            let mut rt = self.remote.lock().unwrap_or_else(|p| p.into_inner());
            let window = Duration::from_secs(600);
            for v in rt.fails.values_mut() {
                v.retain(|t| t.elapsed() < window);
            }
            let total: usize = rt.fails.values().map(Vec::len).sum();
            if rt.fails.get(&peer).map_or(0, Vec::len) >= MAX_FAILS_PER_PEER || total >= MAX_FAILS_TOTAL {
                return WireResponse::Denied(
                    "Demasiados intentos con códigos incorrectos. Espera unos minutos.".into(),
                );
            }
        }
        let name = match clean_name(name) {
            Ok(n) => n,
            Err(e) => return WireResponse::Denied(e.message),
        };
        let hash = sha_hex(secret.trim().as_bytes());
        let _g = self.companions_lock.lock().unwrap_or_else(|p| p.into_inner());
        let mut f = self.host_file();
        let now = now_ms();
        f.invites.retain(|i| i.expires_at_unix_ms > now);
        let Some(pos) = f.invites.iter().position(|i| same(&i.secret_sha256, &hash)) else {
            drop(_g);
            self.remote
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .fails
                .entry(peer)
                .or_default()
                .push(Instant::now());
            self.audit(
                "companeros",
                None,
                "companero.codigo_rechazado",
                &merc_remote::fingerprint(&peer),
            );
            return deny;
        };
        // Un solo uso.
        let inv = f.invites.remove(pos);
        let Ok(inst) = self.instance(inv.server) else {
            let _ = self.save_host_file(&f);
            return WireResponse::Denied("Ese servidor ya no existe en la PC de tu amigo.".into());
        };
        let key = merc_remote::hex(&peer);
        let c = match f.companions.iter_mut().find(|c| c.key == key) {
            Some(c) => {
                c.name = name.clone();
                c
            }
            None => {
                f.companions.push(StoredCompanion {
                    key: key.clone(),
                    name: name.clone(),
                    added_at_unix_ms: now,
                    last_seen_unix_ms: None,
                    grants: BTreeMap::new(),
                });
                f.companions.last_mut().expect("recién añadido")
            }
        };
        c.grants.insert(inv.server, sorted(inv.perms.clone()));
        c.last_seen_unix_ms = Some(now);
        let c = c.clone();
        if let Err(e) = self.save_host_file(&f) {
            return WireResponse::Denied(e.message);
        }
        drop(_g);
        let fp = merc_remote::fingerprint(&peer);
        self.audit(
            &format!("companero:{name}"),
            Some(inv.server),
            "companero.unido",
            &format!("PC {fp}; permisos: {:?}", inv.perms),
        );
        self.push_alert(
            merc_admin::health::AlertKind::Companion,
            merc_admin::health::AlertLevel::Info,
            Some((inv.server, inst.name.clone())),
            format!("{name} se unió como compañero"),
            format!(
                "Ya puede administrar «{}» con los permisos que elegiste (PC {fp}).",
                inst.name
            ),
        );
        WireResponse::Servers {
            host_name: pc_name(),
            servers: self.servers_of(&c),
        }
    }

    /// Autoriza y ejecuta una petición de un compañero. Siempre se leen sus permisos actuales.
    fn remote_call_in(self: &Arc<Self>, peer: [u8; 32], body: RequestBody) -> Result<ResponseBody, ApiError> {
        if !self.companions_enabled.load(Ordering::SeqCst) {
            return Err(forbidden("Tu amigo desactivó el acceso de compañeros."));
        }
        let f = self.host_file();
        let Some(c) = Self::companion_by_key(&f, &peer).cloned() else {
            return Err(forbidden("Esta PC ya no tiene acceso a los servidores de tu amigo."));
        };
        let who = format!("companero:{}", c.name);
        let need = rm::need(&body);
        let allowed = match &need {
            Need::Any | Need::ListInstances => !c.grants.values().all(Vec::is_empty),
            Need::On(id, perm) => c.grants.get(id).is_some_and(|p| p.contains(perm)),
            Need::OwnJob(job) => self
                .remote
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .jobs
                .get(&peer)
                .is_some_and(|s| s.contains(job)),
            Need::Deny => false,
        };
        let instance = match &need {
            Need::On(id, _) => Some(*id),
            _ => None,
        };
        let action = serde_json::to_value(&body)
            .ok()
            .and_then(|v| v.get("op").and_then(|o| o.as_str()).map(str::to_string))
            .unwrap_or_default();
        if !allowed {
            self.audit(&who, instance, "companero.denegado", &action);
            return Err(forbidden(match need {
                Need::Deny => "Esto solo lo puede hacer el dueño del servidor en su PC.".to_string(),
                Need::On(_, p) => format!("No tienes el permiso «{}» en este servidor.", perm_label(p)),
                _ => "No tienes permiso para esto.".to_string(),
            }));
        }
        let read = is_read(&body);
        let result = self.handle(&who, body).map(|(b, _)| b);
        if !read {
            self.audit(
                &who,
                instance,
                &format!("companero.{action}"),
                match &result {
                    Ok(_) => "ok".to_string(),
                    Err(e) => format!("error: {}", e.message),
                }
                .as_str(),
            );
        }
        let mut body = result?;
        if let ResponseBody::Job(j) = &body {
            self.remote
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .jobs
                .entry(peer)
                .or_default()
                .insert(j.job_id.clone());
        }
        if let ResponseBody::Instances(list) = &mut body {
            list.retain(|s| c.grants.get(&s.instance.id).is_some_and(|p| !p.is_empty()));
        }
        // Sin rutas de esta PC en lo que sale.
        let mut v = serde_json::to_value(&body).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        redact(&mut v);
        serde_json::from_value(v).map_err(|e| api(ErrorCode::Internal, e.to_string()))
    }

    /// Compañeros con acceso a un servidor y sus permisos.
    pub(crate) fn companions_on(&self, id: InstanceId) -> Vec<(String, Vec<Perm>)> {
        self.host_file()
            .companions
            .into_iter()
            .filter_map(|c| {
                c.grants
                    .get(&id)
                    .filter(|p| !p.is_empty())
                    .map(|p| (c.name.clone(), p.clone()))
            })
            .collect()
    }

    fn companions_info(&self) -> CompanionsInfo {
        let f = self.host_file();
        let (node, error) = {
            let rt = self.remote.lock().unwrap_or_else(|p| p.into_inner());
            (rt.node.clone(), rt.error.clone())
        };
        let connected: HashSet<String> = node
            .as_ref()
            .map(|n| n.connected().iter().map(|k| merc_remote::hex(k)).collect())
            .unwrap_or_default();
        let now = now_ms();
        CompanionsInfo {
            enabled: f.enabled,
            online: node.is_some() && f.enabled,
            fingerprint: self
                .device_key()
                .ok()
                .map(|k| merc_remote::fingerprint(&merc_remote::public_of(&k))),
            error,
            invites: f
                .invites
                .iter()
                .filter(|i| i.expires_at_unix_ms > now)
                .map(|i| InviteInfo {
                    invite_id: i.id.clone(),
                    server: i.server,
                    server_name: self.instance(i.server).map(|x| x.name).unwrap_or_default(),
                    perms: i.perms.clone(),
                    created_at_unix_ms: i.created_at_unix_ms,
                    expires_at_unix_ms: i.expires_at_unix_ms,
                })
                .collect(),
            companions: f
                .companions
                .iter()
                .map(|c| CompanionInfo {
                    companion: c.key.clone(),
                    name: c.name.clone(),
                    fingerprint: merc_remote::parse_hex32(&c.key)
                        .map(|k| merc_remote::fingerprint(&k))
                        .unwrap_or_default(),
                    added_at_unix_ms: c.added_at_unix_ms,
                    last_seen_unix_ms: c.last_seen_unix_ms,
                    connected: connected.contains(&c.key),
                    servers: self.servers_of(c),
                })
                .collect(),
        }
    }

    // ───────────────────────── Compañero ─────────────────────────

    fn friend_call(self: &Arc<Self>, host: &StoredHost, req: &WireRequest) -> Result<WireResponse, ApiError> {
        let key = merc_remote::parse_hex32(&host.key).ok_or_else(|| api(ErrorCode::BadRequest, "PC no válida"))?;
        let node = self.node()?;
        let hints = merc_remote::Hints {
            addrs: host.hints.clone(),
            relay: host.relay.clone(),
        };
        let r = node.call(key, &hints, req, CALL_TIMEOUT);
        let mut rt = self.remote.lock().unwrap_or_else(|p| p.into_inner());
        let st = rt.host_state.entry(host.key.clone()).or_default();
        match r {
            Ok(w) => {
                st.0 = Some(now_ms());
                st.1 = None;
                Ok(w)
            }
            Err(e) => {
                let msg = format!(
                    "{e}. Su PC tiene que estar encendida, con MercHosting abierto o en segundo plano y conectada a Internet."
                );
                st.1 = Some(msg.clone());
                Err(api(ErrorCode::Network, msg))
            }
        }
    }

    fn friend(&self, host: &str) -> Result<StoredHost, ApiError> {
        self.friends()
            .into_iter()
            .find(|h| h.key == host)
            .ok_or_else(|| api(ErrorCode::NotFound, "No tienes esa PC guardada."))
    }

    fn remote_hosts(&self) -> Vec<RemoteHost> {
        let rt = self.remote.lock().unwrap_or_else(|p| p.into_inner());
        self.friends()
            .into_iter()
            .map(|h| {
                let (ok, err) = rt.host_state.get(&h.key).cloned().unwrap_or_default();
                RemoteHost {
                    fingerprint: merc_remote::parse_hex32(&h.key)
                        .map(|k| merc_remote::fingerprint(&k))
                        .unwrap_or_default(),
                    host: h.key,
                    name: h.name,
                    servers: h.servers,
                    last_ok_unix_ms: ok,
                    last_error: err,
                }
            })
            .collect()
    }

    fn update_friend(
        &self,
        key: &str,
        name: Option<String>,
        servers: Vec<RemoteServer>,
        hints: Option<(Vec<SocketAddr>, Option<String>)>,
    ) -> Result<(), ApiError> {
        let _g = self.companions_lock.lock().unwrap_or_else(|p| p.into_inner());
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
            }
            None => {
                let (a, r) = hints.unwrap_or_default();
                list.push(StoredHost {
                    key: key.to_string(),
                    name: name.unwrap_or_default(),
                    hints: a,
                    relay: r,
                    servers,
                })
            }
        }
        self.save_friends(&list)
    }

    pub(crate) fn handle_companions(
        self: &Arc<Self>,
        client: &str,
        body: RequestBody,
    ) -> Result<ResponseBody, ApiError> {
        use RequestBody as R;
        Ok(match body {
            R::Companions => ResponseBody::Companions(Box::new(self.companions_info())),
            R::SetCompanionsEnabled { enabled } => {
                {
                    let _g = self.companions_lock.lock().unwrap_or_else(|p| p.into_inner());
                    let mut f = self.host_file();
                    f.enabled = enabled;
                    self.save_host_file(&f)?;
                }
                self.companions_enabled.store(enabled, Ordering::SeqCst);
                if enabled {
                    self.node()?;
                } else if let Some(n) = self.remote.lock().unwrap_or_else(|p| p.into_inner()).node.clone() {
                    for peer in n.connected() {
                        n.disconnect(&peer);
                    }
                }
                self.audit(
                    client,
                    None,
                    if enabled {
                        "companeros.activar"
                    } else {
                        "companeros.pausar"
                    },
                    "",
                );
                ResponseBody::Companions(Box::new(self.companions_info()))
            }
            R::CreateInvite { id, perms, minutes } => {
                let inst = self.instance(id)?;
                // «Ver el estado» va siempre: sin él la app del compañero no puede mostrar nada.
                let perms = sorted([perms, vec![Perm::Status]].concat());
                if !(5..=1440).contains(&minutes) {
                    return Err(api(
                        ErrorCode::BadRequest,
                        "La invitación debe durar entre 5 minutos y 24 horas.",
                    ));
                }
                if !self.companions_enabled.load(Ordering::SeqCst) {
                    return Err(api(
                        ErrorCode::InvalidState,
                        "Activa «Permitir compañeros» antes de invitar.",
                    ));
                }
                let node = self.node()?;
                let secret_bytes: [u8; 16] = *uuid::Uuid::new_v4().as_bytes();
                let secret_hex = merc_remote::hex(&secret_bytes);
                let now = now_ms();
                let stored = StoredInvite {
                    id: uuid::Uuid::new_v4().simple().to_string()[..12].to_string(),
                    secret_sha256: sha_hex(secret_hex.as_bytes()),
                    server: id,
                    perms: perms.clone(),
                    created_at_unix_ms: now,
                    expires_at_unix_ms: now + minutes as i64 * 60_000,
                };
                {
                    let _g = self.companions_lock.lock().unwrap_or_else(|p| p.into_inner());
                    let mut f = self.host_file();
                    f.invites.retain(|i| i.expires_at_unix_ms > now);
                    if f.invites.len() >= 20 {
                        return Err(api(
                            ErrorCode::Conflict,
                            "Tienes demasiadas invitaciones abiertas. Cancela alguna.",
                        ));
                    }
                    f.invites.push(stored.clone());
                    self.save_host_file(&f)?;
                }
                self.audit(
                    client,
                    Some(id),
                    "companeros.invitar",
                    &format!("{perms:?}; {minutes} min"),
                );
                let code = Invite {
                    host: node.public(),
                    secret: secret_bytes,
                    addrs: node.hint_addrs(),
                    relay: node.relay_url(),
                }
                .encode();
                ResponseBody::Invite(Box::new(NewInvite {
                    code,
                    invite: InviteInfo {
                        invite_id: stored.id,
                        server: id,
                        server_name: inst.name,
                        perms,
                        created_at_unix_ms: now,
                        expires_at_unix_ms: stored.expires_at_unix_ms,
                    },
                }))
            }
            R::CancelInvite { invite_id } => {
                {
                    let _g = self.companions_lock.lock().unwrap_or_else(|p| p.into_inner());
                    let mut f = self.host_file();
                    f.invites.retain(|i| i.id != invite_id);
                    self.save_host_file(&f)?;
                }
                self.audit(client, None, "companeros.cancelar_invitacion", &invite_id);
                ResponseBody::Companions(Box::new(self.companions_info()))
            }
            R::SetCompanionPerms { companion, id, perms } => {
                let perms = if perms.is_empty() {
                    perms
                } else {
                    sorted([perms, vec![Perm::Status]].concat())
                };
                let name = {
                    let _g = self.companions_lock.lock().unwrap_or_else(|p| p.into_inner());
                    let mut f = self.host_file();
                    let Some(c) = f.companions.iter_mut().find(|c| c.key == companion) else {
                        return Err(api(ErrorCode::NotFound, "Ese compañero ya no está."));
                    };
                    if perms.is_empty() {
                        c.grants.remove(&id);
                    } else {
                        self.instance(id)?;
                        c.grants.insert(id, perms.clone());
                    }
                    let name = c.name.clone();
                    self.save_host_file(&f)?;
                    name
                };
                // Lo que estuviera haciendo con permisos viejos se corta (vuelve a conectar solo).
                if let (Some(n), Some(k)) = (
                    self.remote.lock().unwrap_or_else(|p| p.into_inner()).node.clone(),
                    merc_remote::parse_hex32(&companion),
                ) {
                    n.disconnect(&k);
                }
                self.audit(client, Some(id), "companeros.permisos", &format!("{name}: {perms:?}"));
                ResponseBody::Companions(Box::new(self.companions_info()))
            }
            R::RemoveCompanion { companion } => {
                let name = {
                    let _g = self.companions_lock.lock().unwrap_or_else(|p| p.into_inner());
                    let mut f = self.host_file();
                    let name = f
                        .companions
                        .iter()
                        .find(|c| c.key == companion)
                        .map(|c| c.name.clone())
                        .ok_or_else(|| api(ErrorCode::NotFound, "Ese compañero ya no está."))?;
                    f.companions.retain(|c| c.key != companion);
                    self.save_host_file(&f)?;
                    name
                };
                if let Some(k) = merc_remote::parse_hex32(&companion) {
                    let mut rt = self.remote.lock().unwrap_or_else(|p| p.into_inner());
                    rt.jobs.remove(&k);
                    if let Some(n) = rt.node.clone() {
                        n.disconnect(&k);
                    }
                }
                self.audit(client, None, "companeros.quitar", &name);
                ResponseBody::Companions(Box::new(self.companions_info()))
            }
            R::JoinWithCode { code, name } => {
                let name = clean_name(&name)?;
                let inv = Invite::decode(&code).map_err(|e| api(ErrorCode::BadRequest, e.to_string()))?;
                let node = self.node()?;
                if inv.host == node.public() {
                    return Err(api(ErrorCode::BadRequest, "Ese código es de esta misma PC."));
                }
                let host = StoredHost {
                    key: merc_remote::hex(&inv.host),
                    name: String::new(),
                    hints: inv.addrs.clone(),
                    relay: inv.relay.clone(),
                    servers: vec![],
                };
                let resp = self.friend_call(
                    &host,
                    &WireRequest::Pair {
                        secret: merc_remote::hex(&inv.secret),
                        name: name.clone(),
                    },
                )?;
                match resp {
                    WireResponse::Servers { host_name, servers } => {
                        self.update_friend(
                            &host.key,
                            Some(host_name.clone()),
                            servers,
                            Some((inv.addrs, inv.relay)),
                        )?;
                        self.audit(client, None, "companeros.unirse", &host_name);
                    }
                    w => return Err(unexpected(w)),
                }
                ResponseBody::RemoteHosts(self.remote_hosts())
            }
            R::RemoteHosts => ResponseBody::RemoteHosts(self.remote_hosts()),
            R::RefreshHost { host } => {
                let h = self
                    .friends()
                    .into_iter()
                    .find(|h| h.key == host)
                    .ok_or_else(|| api(ErrorCode::NotFound, "No tienes esa PC guardada."))?;
                match self.friend_call(&h, &WireRequest::Servers)? {
                    WireResponse::Servers { host_name, servers } => {
                        self.update_friend(&h.key, Some(host_name), servers, None)?
                    }
                    WireResponse::Denied(m) => {
                        // Ya no tiene acceso: se muestra sin servidores.
                        self.update_friend(&h.key, None, vec![], None)?;
                        self.remote
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .host_state
                            .entry(h.key.clone())
                            .or_default()
                            .1 = Some(m);
                    }
                    _ => {}
                }
                ResponseBody::RemoteHosts(self.remote_hosts())
            }
            R::ForgetHost { host } => {
                {
                    let _g = self.companions_lock.lock().unwrap_or_else(|p| p.into_inner());
                    let mut list = self.friends();
                    list.retain(|h| h.key != host);
                    self.save_friends(&list)?;
                }
                if let (Some(n), Some(k)) = (
                    self.remote.lock().unwrap_or_else(|p| p.into_inner()).node.clone(),
                    merc_remote::parse_hex32(&host),
                ) {
                    n.drop_host(&k);
                }
                ResponseBody::RemoteHosts(self.remote_hosts())
            }
            R::RemoteDownload { host, item, dest } => {
                if !dest.is_absolute() {
                    return Err(api(ErrorCode::BadRequest, "Destino no válido"));
                }
                let h = self.friend(&host)?;
                let (token, size, sha) = match self.friend_call(&h, &WireRequest::Prepare { item })? {
                    WireResponse::Prepared {
                        token, size, sha256, ..
                    } => (token, size, sha256),
                    w => return Err(unexpected(w)),
                };
                let part = dest.with_file_name(format!(
                    "{}.merchosting-parte",
                    dest.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                ));
                let result = (|| {
                    use std::io::Write;
                    let mut f = std::fs::File::create(&part).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
                    let mut hasher = Sha256::new();
                    let mut offset = 0u64;
                    while offset < size {
                        let data = match self.friend_call(
                            &h,
                            &WireRequest::Chunk {
                                token: token.clone(),
                                offset,
                            },
                        )? {
                            WireResponse::Chunk { data } => b64()
                                .decode(data)
                                .map_err(|_| api(ErrorCode::Network, "Trozo dañado"))?,
                            w => return Err(unexpected(w)),
                        };
                        if data.is_empty() {
                            return Err(api(ErrorCode::Network, "La descarga se cortó."));
                        }
                        hasher.update(&data);
                        f.write_all(&data)
                            .map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
                        offset += data.len() as u64;
                    }
                    f.sync_all().map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
                    drop(f);
                    if !same(&merc_remote::hex(&hasher.finalize()), &sha) {
                        return Err(api(
                            ErrorCode::Network,
                            "El archivo llegó dañado (no coincide su huella). Vuelve a descargarlo.",
                        ));
                    }
                    std::fs::rename(&part, &dest).map_err(|e| api(ErrorCode::Internal, e.to_string()))
                })();
                if let Err(e) = result {
                    let _ = std::fs::remove_file(&part);
                    return Err(e);
                }
                ResponseBody::FileDone {
                    path: dest.display().to_string(),
                }
            }
            R::RemoteUpload {
                host,
                id,
                source,
                dest_dir,
            } => {
                use std::io::Read;
                if !source.is_absolute() || !source.is_file() {
                    return Err(api(ErrorCode::BadRequest, "Archivo de origen no válido"));
                }
                let name = source
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .ok_or_else(|| api(ErrorCode::BadRequest, "Archivo de origen no válido"))?;
                let size = std::fs::metadata(&source)
                    .map_err(|e| api(ErrorCode::BadRequest, e.to_string()))?
                    .len();
                if size > merc_admin::files::MAX_UPLOAD_BYTES {
                    return Err(api(
                        ErrorCode::BadRequest,
                        "El archivo es demasiado grande (máximo 2 GB).",
                    ));
                }
                // Huella primero, para mandarla con el último trozo.
                let mut f = std::fs::File::open(&source).map_err(|e| api(ErrorCode::BadRequest, e.to_string()))?;
                let mut hasher = Sha256::new();
                std::io::copy(&mut f, &mut hasher).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
                let sha = merc_remote::hex(&hasher.finalize());
                let h = self.friend(&host)?;
                let mut f = std::fs::File::open(&source).map_err(|e| api(ErrorCode::BadRequest, e.to_string()))?;
                let mut upload_id = None;
                let mut offset = 0u64;
                loop {
                    let mut buf = vec![0u8; CHUNK];
                    let mut n = 0;
                    while n < CHUNK {
                        let r = f
                            .read(&mut buf[n..])
                            .map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
                        if r == 0 {
                            break;
                        }
                        n += r;
                    }
                    buf.truncate(n);
                    let last = offset + n as u64 >= size;
                    let resp = self.friend_call(
                        &h,
                        &WireRequest::Upload {
                            upload_id: upload_id.clone(),
                            id,
                            dest_dir: dest_dir.clone(),
                            name: name.clone(),
                            size,
                            offset,
                            data: b64().encode(&buf),
                            sha256: last.then(|| sha.clone()),
                        },
                    )?;
                    offset += n as u64;
                    match resp {
                        WireResponse::UploadProgress { upload_id: u, .. } if !last => upload_id = Some(u),
                        WireResponse::Result(r) if last => match *r {
                            ResponseResult::Ok(b) => break b,
                            ResponseResult::Err(e) => return Err(e),
                        },
                        w => return Err(unexpected(w)),
                    }
                }
            }
            R::RemoteCall { host, request } => {
                let h = self
                    .friends()
                    .into_iter()
                    .find(|h| h.key == host)
                    .ok_or_else(|| api(ErrorCode::NotFound, "No tienes esa PC guardada."))?;
                match self.friend_call(&h, &WireRequest::Call { request })? {
                    WireResponse::Result(r) => match *r {
                        ResponseResult::Ok(b) => b,
                        ResponseResult::Err(e) => return Err(e),
                    },
                    w => return Err(unexpected(w)),
                }
            }
            _ => return Err(api(ErrorCode::BadRequest, "Petición no válida")),
        })
    }
}

pub(crate) fn perm_label(p: Perm) -> &'static str {
    match p {
        Perm::Status => "ver estado",
        Perm::Start => "iniciar",
        Perm::Stop => "detener",
        Perm::Restart => "reiniciar",
        Perm::ConsoleRead => "leer la consola",
        Perm::ConsoleCommand => "enviar comandos",
        Perm::Logs => "ver el historial",
        Perm::Players => "jugadores",
        Perm::Whitelist => "lista blanca",
        Perm::Op => "dar OP",
        Perm::FilesList => "ver la lista de archivos",
        Perm::FilesRead => "leer y descargar archivos",
        Perm::FilesEdit => "editar y subir archivos",
        Perm::FilesDelete => "borrar archivos",
        Perm::Content => "mods y plugins",
        Perm::Backups => "copias de seguridad",
        Perm::BackupsRestore => "restaurar copias",
        Perm::BackupsDelete => "borrar copias",
        Perm::World => "mundos",
        Perm::Settings => "ajustes",
        Perm::Network => "conexión",
    }
}

/// Respuesta de la PC del amigo que no encaja (o un rechazo).
fn unexpected(w: WireResponse) -> ApiError {
    match w {
        WireResponse::Denied(m) => forbidden(m),
        WireResponse::Result(r) => match *r {
            ResponseResult::Err(e) => e,
            ResponseResult::Ok(_) => api(ErrorCode::Internal, "Respuesta inesperada"),
        },
        _ => api(ErrorCode::Internal, "Respuesta inesperada"),
    }
}

fn write_json<T: Serialize + ?Sized>(path: &std::path::Path, v: &T) -> Result<(), ApiError> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
    }
    let tmp = path.with_extension("tmp");
    let text = serde_json::to_string_pretty(v).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
    std::fs::write(&tmp, text).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
    std::fs::rename(&tmp, path).map_err(|e| api(ErrorCode::Internal, e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_only_whole_paths() {
        let mut v = serde_json::json!({
            "root": r"C:\Users\amir\Servidores\Survival",
            "unc": r"\\pc\share",
            "unix": "/home/amir/x",
            "motd": "Hola C:\\ no es ruta entera",
            "rel": "world/level.dat",
            "list": [r"D:/x", "ok"]
        });
        redact(&mut v);
        assert_eq!(v["root"], "");
        assert_eq!(v["unc"], "");
        assert_eq!(v["unix"], "");
        assert_eq!(v["motd"], "Hola C:\\ no es ruta entera");
        assert_eq!(v["rel"], "world/level.dat");
        assert_eq!(v["list"][0], "");
        assert!(same("abc", "abc") && !same("abc", "abd") && !same("a", "ab"));
    }
}
