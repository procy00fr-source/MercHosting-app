//! Supervisor persistente de MercHosting.
//!
//! Es un proceso aparte de la ventana de la aplicación: si la interfaz se cierra o se cae, los
//! servidores siguen funcionando y al volver a abrirla se recupera el control (estado, consola
//! acumulada, comandos). Solo puede haber un supervisor por carpeta de datos.

mod addons;
mod alerts_watch;
mod automation;
mod awake;
mod backups;
mod bedrock_players;
mod companions;
mod diagnose;
mod friendpack;
mod headless;
mod health;
mod icon;
mod java;
mod jobs;
mod luckperms;
mod modpacks;
mod modsides;
mod players;
mod playit;
mod rpbrowse;
mod security;
mod servers;
pub mod service;
mod supporter;
mod tasks;
mod updates;
mod voice;

use merc_core::detect::{detect_server, ServerKind};
use merc_core::instance::validate_instance_name;
use merc_core::launch::LaunchProfile;
use merc_core::paths::strip_verbatim;
use merc_core::{Instance, InstanceId};
use merc_install::{CatalogEntry, Flavor, Http, InstallError, InstallPlan, Sources, Step};
use merc_ipc::*;
use merc_store::{Store, StoreError};
use merc_supervisor::{RecoveryOutcome, RuntimeDir, StartSpec, Supervisor, SupervisorError};
use std::collections::{BTreeMap, HashMap};
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_CONNECTIONS: usize = 32;
const MAX_STOP_TIMEOUT_SECS: u32 = 600;
const CATALOG_TTL: Duration = Duration::from_secs(10 * 60);

pub enum RunOutcome {
    /// Otro supervisor ya atiende esta carpeta de datos.
    AlreadyRunning,
}

struct DaemonLog {
    file: Mutex<std::fs::File>,
}

impl DaemonLog {
    fn open(path: &Path) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self { file: Mutex::new(file) })
    }
    fn line(&self, msg: &str) {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "[{secs}] {msg}");
        }
    }
}

struct Daemon {
    data_dir: PathBuf,
    store: Mutex<Store>,
    sup: Supervisor,
    token: String,
    log: DaemonLog,
    endpoint: Endpoint,
    http: Http,
    sources: Sources,
    jobs: jobs::Jobs,
    catalog_cache: Mutex<HashMap<Flavor, (Instant, Vec<CatalogEntry>)>>,
    /// Servidores con un reinicio avisado en cuenta atrás.
    pending_restarts: Mutex<HashMap<InstanceId, Arc<PendingRestart>>>,
    /// Operación de mundos en curso por servidor (una a la vez, con progreso y cancelación).
    world_ops: Mutex<HashMap<InstanceId, Arc<RunningWorldOp>>>,
    /// API de Modrinth (en pruebas de desarrollo se puede apuntar a un servidor local).
    modrinth_api: String,
    hangar_api: String,
    /// API de descargas de GeyserMC (Floodgate para plugins).
    geysermc_api: String,
    hangar_cache: Mutex<merc_content::hangar::HangarCache>,
    /// Tareas programadas ejecutándose ahora (servidor, tarea).
    running_tasks: Mutex<std::collections::HashSet<(InstanceId, String)>>,
    /// Serializa las lecturas y escrituras de los archivos de tareas.
    tasks_lock: Mutex<()>,
    /// Última medición de CPU y memoria de la PC (la hace el hilo de salud).
    pc_load: Mutex<health::PcLoad>,
    alerts_lock: Mutex<()>,
    playit: Mutex<playit::PlayitRuntime>,
    remote: Mutex<companions::RemoteRuntime>,
    /// Serializa los archivos de compañeros (anfitrión y amigos).
    companions_lock: Mutex<()>,
    /// Se consulta en cada conexión y petición de un compañero.
    companions_enabled: std::sync::atomic::AtomicBool,
    restarts: Mutex<automation::RestartBook>,
}

struct PendingRestart {
    deadline: Instant,
    cancel: std::sync::atomic::AtomicBool,
}

struct RunningWorldOp {
    kind: &'static str,
    world: String,
    progress: merc_admin::worlds::Progress,
}

/// Quita la operación de mundos del registro al terminar (bien, mal o cancelada).
struct WorldOpGuard<'a> {
    daemon: &'a Daemon,
    id: InstanceId,
}

impl Drop for WorldOpGuard<'_> {
    fn drop(&mut self) {
        self.daemon
            .world_ops
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.id);
    }
}

struct CreateParams {
    name: String,
    flavor: Flavor,
    mc_version: String,
    path: PathBuf,
    java: PathBuf,
    min_memory_mb: u32,
    max_memory_mb: u32,
    properties: BTreeMap<String, String>,
    accept_eula: bool,
    modpack_version: Option<String>,
    modpack_source: ContentSource,
    loader_version: Option<String>,
    game_rules: BTreeMap<String, String>,
}

/// Modpack listo para instalar: el archivo (para los extras), su índice y avisos.
struct LoadedPack {
    path: PathBuf,
    index: merc_content::mrpack::PackIndex,
    project_id: String,
    version_number: String,
    /// Carpetas de extras dentro del archivo, de menor a mayor prioridad.
    overrides: Vec<String>,
    warnings: Vec<String>,
}

fn admin_err(e: merc_admin::AdminError) -> ApiError {
    use merc_admin::AdminError as A;
    let code = match &e {
        A::Invalid(_) | A::Path(_) => ErrorCode::BadRequest,
        A::NotFound(_) => ErrorCode::NotFound,
        A::Conflict(_) => ErrorCode::Conflict,
        A::Io(_) => ErrorCode::Internal,
        A::Cancelled => ErrorCode::Conflict,
    };
    api(code, e.to_string())
}

fn install_err(e: InstallError) -> ApiError {
    use InstallError as I;
    let code = match &e {
        I::Network(_) => ErrorCode::Network,
        I::NotFound(_) | I::NotAvailable(_) => ErrorCode::NotFound,
        I::TargetNotEmpty(_) => ErrorCode::Conflict,
        I::InvalidInput(_) | I::TargetNotUsable(_) | I::UrlNotAllowed(_) => ErrorCode::BadRequest,
        _ => ErrorCode::Internal,
    };
    api(code, e.to_string())
}

/// Fuentes oficiales. En compilaciones de desarrollo se pueden redirigir (solo a HTTPS o
/// al propio equipo) para las pruebas automáticas.
fn sources() -> Sources {
    let mut s = Sources::default();
    if cfg!(debug_assertions) {
        for (var, slot) in [
            ("MERC_SOURCE_MOJANG", &mut s.mojang_manifest),
            ("MERC_SOURCE_PAPER", &mut s.paper_api),
            ("MERC_SOURCE_FABRIC", &mut s.fabric_meta),
            ("MERC_SOURCE_NEOFORGE", &mut s.neoforge_maven),
            ("MERC_SOURCE_PURPUR", &mut s.purpur_api),
            ("MERC_SOURCE_FORGE_PROMOS", &mut s.forge_promotions),
            ("MERC_SOURCE_FORGE", &mut s.forge_maven),
            ("MERC_SOURCE_QUILT", &mut s.quilt_meta),
            ("MERC_SOURCE_QUILT_MAVEN", &mut s.quilt_maven),
            ("MERC_SOURCE_ARCLIGHT", &mut s.arclight_releases),
            ("MERC_SOURCE_SPIGOT", &mut s.spigot_versions),
            ("MERC_SOURCE_BUILDTOOLS", &mut s.buildtools_url),
            ("MERC_SOURCE_BEDROCK", &mut s.bedrock_links),
        ] {
            if let Ok(v) = std::env::var(var) {
                if merc_install::http::url_allowed(&v) {
                    *slot = v;
                }
            }
        }
    }
    s
}

const NO_HANGAR_PACKS: &str = "Hangar no tiene modpacks; usa Modrinth o CurseForge.";

fn hangar_api() -> String {
    if cfg!(debug_assertions) {
        if let Ok(v) = std::env::var("MERC_SOURCE_HANGAR") {
            if merc_install::http::url_allowed(&v) {
                return v;
            }
        }
    }
    merc_content::hangar::HANGAR_API.to_string()
}

fn geysermc_api() -> String {
    if cfg!(debug_assertions) {
        if let Ok(v) = std::env::var("MERC_SOURCE_GEYSERMC") {
            if merc_install::http::url_allowed(&v) {
                return v;
            }
        }
    }
    merc_content::geysermc::GEYSERMC_API.to_string()
}

fn modrinth_api() -> String {
    if cfg!(debug_assertions) {
        if let Ok(v) = std::env::var("MERC_SOURCE_MODRINTH") {
            if merc_install::http::url_allowed(&v) {
                return v;
            }
        }
    }
    merc_content::modrinth::MODRINTH_API.to_string()
}

/// Clave de API de CurseForge de esta instalación, si la hay. Nunca está en el repositorio:
/// se inyecta al compilar (`MERC_CURSEFORGE_API_KEY`) o quien desarrolla la deja en
/// `curseforge-api-key.txt` dentro de la carpeta de datos.
fn curseforge_key(data_dir: &Path) -> Option<String> {
    if cfg!(debug_assertions) {
        if let Ok(k) = std::env::var("MERC_CURSEFORGE_API_KEY") {
            if !k.trim().is_empty() {
                return Some(k.trim().to_string());
            }
        }
    }
    if let Some(k) = option_env!("MERC_CURSEFORGE_API_KEY") {
        if !k.trim().is_empty() {
            return Some(k.trim().to_string());
        }
    }
    std::fs::read_to_string(data_dir.join("curseforge-api-key.txt"))
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty() && k.len() < 200 && !k.contains(char::is_whitespace))
}

fn curseforge_api() -> String {
    if cfg!(debug_assertions) {
        if let Ok(v) = std::env::var("MERC_SOURCE_CURSEFORGE") {
            if merc_install::http::url_allowed(&v) {
                return v;
            }
        }
    }
    merc_content::curseforge::CURSEFORGE_API.to_string()
}

fn content_err(e: merc_content::ContentError) -> ApiError {
    use merc_content::ContentError as C;
    let code = match &e {
        C::Network(_) => ErrorCode::Network,
        C::NotFound(_) => ErrorCode::NotFound,
        C::Invalid(_) => ErrorCode::BadRequest,
        C::Conflict(_) | C::Cancelled => ErrorCode::Conflict,
        C::BadResponse(_) | C::Io(_) | C::Integrity(_) => ErrorCode::Internal,
    };
    api(code, e.to_string())
}

/// Carpeta por defecto para servidores nuevos: `%USERPROFILE%\MercHosting\Servidores`.
pub(crate) fn default_servers_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("MERC_SERVERS_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    home.join("MercHosting").join("Servidores")
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn api(code: ErrorCode, msg: impl Into<String>) -> ApiError {
    ApiError::new(code, msg)
}

fn sup_err(e: SupervisorError) -> ApiError {
    use SupervisorError as S;
    let code = match &e {
        S::AlreadyRunning(_) | S::RootInUse | S::PortInUseByInstance(_) | S::PortBusy(_) => ErrorCode::Conflict,
        S::NotRunning | S::StopTimeout => ErrorCode::InvalidState,
        S::NoConsole => ErrorCode::NoConsole,
        S::UnknownInstance => ErrorCode::NotFound,
        S::Spawn(_) | S::Stdin(_) => ErrorCode::Internal,
        _ => ErrorCode::BadRequest,
    };
    api(code, e.to_string())
}

fn store_err(e: StoreError) -> ApiError {
    match e {
        StoreError::DuplicateRoot(name) => api(
            ErrorCode::Conflict,
            format!("Esa carpeta ya está registrada como «{name}»"),
        ),
        other => api(ErrorCode::Internal, other.to_string()),
    }
}

impl Daemon {
    fn instance(&self, id: InstanceId) -> Result<Instance, ApiError> {
        self.store
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get_instance(id)
            .map_err(store_err)?
            .ok_or_else(|| api(ErrorCode::NotFound, "No existe un servidor con ese identificador"))
    }

    fn audit(&self, client: &str, id: Option<InstanceId>, action: &str, detail: &str) {
        let r = self.store.lock().unwrap_or_else(|p| p.into_inner()).audit(
            &if client.starts_with("companero:") {
                client.to_string()
            } else {
                format!("local:{client}")
            },
            id,
            action,
            detail,
        );
        if let Err(e) = r {
            self.log.line(&format!("no se pudo auditar {action}: {e}"));
        }
    }

    fn spec(inst: &Instance) -> StartSpec {
        // Los modpacks de Forge/NeoForge pueden tardar varios minutos en cargar.
        let readiness = match inst.kind {
            ServerKind::NeoForge | ServerKind::Forge | ServerKind::Mohist | ServerKind::Arclight => {
                Duration::from_secs(900)
            }
            _ => Duration::from_secs(300),
        };
        StartSpec {
            id: inst.id,
            root: inst.root.clone(),
            profile: inst.launch.clone(),
            mc_version: inst.mc_version.clone(),
            readiness_timeout: readiness,
        }
    }

    fn explorer(&self, inst: &Instance) -> Result<merc_admin::files::Explorer, ApiError> {
        let store = self.data_dir.join("instancias").join(inst.id.to_string());
        merc_admin::files::Explorer::new(&inst.root, &store).map_err(admin_err)
    }

    /// Las modificaciones de archivos exigen el servidor detenido: Minecraft los tiene abiertos.
    fn require_stopped(&self, id: InstanceId, what: &str) -> Result<(), ApiError> {
        if self.sup.status(id).state.has_process() {
            return Err(api(
                ErrorCode::InvalidState,
                format!("Detén el servidor antes de {what}: podría estar usando esos archivos."),
            ));
        }
        Ok(())
    }

    fn properties_body(&self, inst: &Instance) -> Result<ResponseBody, ApiError> {
        let path = inst.root.join("server.properties");
        match std::fs::read(&path) {
            Ok(bytes) => {
                let doc = merc_admin::props::PropsDoc::parse(&String::from_utf8_lossy(&bytes));
                let modified_unix_ms = std::fs::metadata(&path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64);
                Ok(ResponseBody::Properties {
                    exists: true,
                    entries: doc.entries(),
                    modified_unix_ms,
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ResponseBody::Properties {
                exists: false,
                entries: vec![],
                modified_unix_ms: None,
            }),
            Err(e) => Err(api(ErrorCode::Internal, format!("{}: {e}", path.display()))),
        }
    }

    fn level_name(inst: &Instance) -> String {
        merc_core::properties::ServerProperties::read(&inst.root)
            .ok()
            .flatten()
            .map(|p| p.level_name)
            .unwrap_or_else(|| "world".into())
    }

    fn worlds_body(inst: &Instance) -> Result<ResponseBody, ApiError> {
        let level_name = Self::level_name(inst);
        Ok(ResponseBody::Worlds {
            worlds: merc_admin::worlds::list_worlds(&inst.root, &level_name).map_err(admin_err)?,
            level_name,
        })
    }

    fn game_rules(&self, inst: &Instance) -> Result<ResponseBody, ApiError> {
        use merc_admin::gamerules as g;
        let st = self.sup.status(inst.id);
        if st.state != merc_core::RuntimeState::Online || !st.has_console {
            return Err(api(
                ErrorCode::InvalidState,
                "Enciende el servidor: las reglas se leen y cambian con el comando oficial /gamerule.",
            ));
        }
        let mc = inst.mc_version.as_deref();
        let after = self.sup.console_since(inst.id, 0).last().map(|l| l.seq).unwrap_or(0);
        for def in &g::RULES {
            self.sup
                .send_command(inst.id, &g::query_command(def, mc))
                .map_err(sup_err)?;
        }
        let mut found: HashMap<String, String> = HashMap::new();
        let deadline = Instant::now() + Duration::from_secs(4);
        while found.len() < g::RULES.len() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(150));
            for l in self.sup.console_since(inst.id, after) {
                if let Some((name, value)) = g::parse_response(&l.text) {
                    found.insert(name, value);
                }
            }
        }
        let rules = g::RULES
            .iter()
            .map(|def| {
                let name = g::rule_name(def, mc).to_string();
                let value = found
                    .get(&name)
                    .or_else(|| found.get(def.key))
                    .or_else(|| found.get(def.legacy))
                    .cloned();
                merc_admin::gamerules::RuleValue {
                    key: def.key.to_string(),
                    name,
                    kind: def.kind,
                    value,
                }
            })
            .collect();
        Ok(ResponseBody::GameRules {
            rules,
            new_names: g::uses_new_names(mc),
        })
    }

    fn restart_with_notice(self: &Arc<Self>, client: &str, inst: Instance, seconds: u32) -> Result<(), ApiError> {
        self.countdown(client, inst, seconds, false)
    }

    /// Cuenta atrás avisando en el chat y luego reinicia (o apaga, si `stop_only`).
    fn countdown(
        self: &Arc<Self>,
        client: &str,
        inst: Instance,
        seconds: u32,
        stop_only: bool,
    ) -> Result<(), ApiError> {
        let id = inst.id;
        let st = self.sup.status(id);
        if st.state != merc_core::RuntimeState::Online || !st.has_console {
            return Err(api(
                ErrorCode::InvalidState,
                "El servidor tiene que estar en línea para avisar a los jugadores",
            ));
        }
        let seconds = seconds.clamp(10, 600);
        let pending = Arc::new(PendingRestart {
            deadline: Instant::now() + Duration::from_secs(seconds as u64),
            cancel: Default::default(),
        });
        {
            let mut map = self.pending_restarts.lock().unwrap_or_else(|p| p.into_inner());
            if map.contains_key(&id) {
                return Err(api(
                    ErrorCode::Conflict,
                    "Ya hay un reinicio o apagado avisado en marcha para este servidor",
                ));
            }
            map.insert(id, pending.clone());
        }
        let verb = if stop_only { "apagará" } else { "reiniciará" };
        self.audit(
            client,
            Some(id),
            if stop_only {
                "server.stop.notice"
            } else {
                "server.restart.notice"
            },
            &format!("{seconds} s"),
        );
        let d = self.clone();
        std::thread::spawn(move || {
            let say = |text: String| {
                let _ = d.sup.send_command(id, &format!("say {text}"));
            };
            let mut marks: Vec<u32> = [seconds, 300, 120, 60, 30, 10, 5, 3, 2, 1]
                .into_iter()
                .filter(|m| *m <= seconds)
                .collect();
            marks.dedup();
            let started = Instant::now();
            let mut aborted = false;
            let cancelled = || pending.cancel.load(Ordering::SeqCst);
            // Espera en pasos cortos para atender una cancelación enseguida.
            let sleep_until = |at: Duration| {
                while started.elapsed() < at && !cancelled() {
                    std::thread::sleep(Duration::from_millis(200).min(at.saturating_sub(started.elapsed())));
                }
            };
            for m in marks {
                sleep_until(Duration::from_secs((seconds - m) as u64));
                if cancelled() || d.sup.status(id).state != merc_core::RuntimeState::Online {
                    aborted = true;
                    break;
                }
                say(if m >= 60 && m % 60 == 0 {
                    format!(
                        "[MercHosting] El servidor se {verb} en {} min. Guarda lo que estés haciendo.",
                        m / 60
                    )
                } else {
                    format!("[MercHosting] El servidor se {verb} en {m} s.")
                });
            }
            if !aborted {
                sleep_until(Duration::from_secs(seconds as u64 + 1));
                aborted = cancelled();
            }
            if cancelled() {
                say(if stop_only {
                    "[MercHosting] Apagado cancelado. ¡Sigan jugando!".into()
                } else {
                    "[MercHosting] Reinicio cancelado. ¡Sigan jugando!".into()
                });
            }
            if !aborted {
                if stop_only {
                    say("[MercHosting] Apagando ahora. ¡Hasta la próxima!".into());
                    std::thread::sleep(Duration::from_millis(500));
                    if let Err(e) = d.sup.stop(id, Duration::from_secs(180)) {
                        d.log.line(&format!("apagado avisado de {id}: {e}"));
                    }
                } else {
                    say("[MercHosting] Reiniciando ahora. ¡Vuelve en un momento!".into());
                    std::thread::sleep(Duration::from_millis(500));
                    if let Err(e) = d.sup.restart(Self::spec(&inst), Duration::from_secs(180)) {
                        d.log.line(&format!("reinicio avisado de {id}: {e}"));
                    }
                }
            }
            d.pending_restarts.lock().unwrap_or_else(|p| p.into_inner()).remove(&id);
        });
        Ok(())
    }

    fn store_dir(&self, id: InstanceId) -> PathBuf {
        self.data_dir.join("instancias").join(id.to_string())
    }

    fn maintenance_file(&self, id: InstanceId) -> PathBuf {
        self.store_dir(id).join("mantenimiento.json")
    }

    fn maintenance(&self, id: InstanceId) -> Option<Maintenance> {
        let text = std::fs::read_to_string(self.maintenance_file(id)).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn admin_state(&self, inst: &Instance) -> AdminState {
        let raw = merc_core::properties::ServerProperties::read(&inst.root)
            .ok()
            .flatten()
            .map(|p| p.raw)
            .unwrap_or_default();
        let flag = |k: &str| raw.get(k).is_some_and(|v| v.trim().eq_ignore_ascii_case("true"));
        let restart_in_seconds = self
            .pending_restarts
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&inst.id)
            .filter(|p| !p.cancel.load(Ordering::SeqCst))
            .map(|p| p.deadline.saturating_duration_since(Instant::now()).as_secs() as u32);
        let world_op = self
            .world_ops
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&inst.id)
            .map(|op| WorldOp {
                kind: op.kind.to_string(),
                world: op.world.clone(),
                done_bytes: op.progress.done.load(Ordering::Relaxed),
                total_bytes: op.progress.total.load(Ordering::Relaxed),
            });
        AdminState {
            maintenance: self.maintenance(inst.id),
            whitelist_on: flag("white-list"),
            enforce_whitelist: flag("enforce-whitelist"),
            restart_in_seconds,
            world_op,
        }
    }

    /// Activa o termina el modo mantenimiento. Con el servidor encendido usa el comando oficial
    /// `whitelist`; apagado, cambia `white-list` en server.properties (con copia previa).
    fn set_maintenance(&self, client: &str, inst: &Instance, on: bool) -> Result<(), ApiError> {
        let id = inst.id;
        let st = self.sup.status(id);
        let online = st.state == merc_core::RuntimeState::Online && st.has_console;
        let current = self.admin_state(inst);
        let file = self.maintenance_file(id);
        if on {
            if current.maintenance.is_some() {
                return Ok(());
            }
            if !online {
                return Err(api(
                    ErrorCode::InvalidState,
                    "Enciende el servidor para activar el mantenimiento: lo aplica Minecraft con su lista blanca.",
                ));
            }
            let m = Maintenance {
                since_unix: now_unix(),
                previous_whitelist: current.whitelist_on,
            };
            std::fs::create_dir_all(self.store_dir(id))
                .and_then(|_| std::fs::write(&file, serde_json::to_vec(&m).unwrap_or_default()))
                .map_err(|e| api(ErrorCode::Internal, format!("No se pudo guardar el estado: {e}")))?;
            if !current.whitelist_on {
                self.sup.send_command(id, "whitelist on").map_err(sup_err)?;
            }
            let _ = self.sup.send_command(
                id,
                "say [MercHosting] Servidor en mantenimiento: por ahora no pueden entrar jugadores nuevos.",
            );
            self.audit(client, Some(id), "maintenance.on", "");
        } else {
            let Some(m) = current.maintenance else {
                return Ok(());
            };
            if !m.previous_whitelist {
                if online {
                    self.sup.send_command(id, "whitelist off").map_err(sup_err)?;
                    let _ = self
                        .sup
                        .send_command(id, "say [MercHosting] Mantenimiento terminado: ya pueden entrar todos.");
                } else if st.state.has_process() {
                    return Err(api(
                        ErrorCode::InvalidState,
                        "Espera a que el servidor termine de encender o de apagarse.",
                    ));
                } else if current.whitelist_on {
                    let ex = self.explorer(inst)?;
                    let text = ex.read_text("server.properties").map_err(admin_err)?;
                    let mut changes = BTreeMap::new();
                    changes.insert("white-list".to_string(), "false".to_string());
                    let new = merc_admin::props::apply_changes(&text.content, &changes).map_err(admin_err)?;
                    ex.write_text("server.properties", &new, text.modified_unix_ms)
                        .map_err(admin_err)?;
                }
            }
            let _ = std::fs::remove_file(&file);
            self.audit(client, Some(id), "maintenance.off", "");
        }
        Ok(())
    }

    /// Ejecuta una operación de mundos registrándola para el progreso y la cancelación.
    fn world_op<T>(
        &self,
        inst: &Instance,
        kind: &'static str,
        world: &str,
        f: impl FnOnce(&merc_admin::worlds::Progress) -> Result<T, merc_admin::AdminError>,
    ) -> Result<T, ApiError> {
        let op = Arc::new(RunningWorldOp {
            kind,
            world: world.to_string(),
            progress: Default::default(),
        });
        {
            let mut map = self.world_ops.lock().unwrap_or_else(|p| p.into_inner());
            if map.contains_key(&inst.id) {
                return Err(api(
                    ErrorCode::Conflict,
                    "Ya hay una operación de mundos en marcha en este servidor; espera a que termine.",
                ));
            }
            map.insert(inst.id, op.clone());
        }
        let _guard = WorldOpGuard {
            daemon: self,
            id: inst.id,
        };
        f(&op.progress).map_err(admin_err)
    }

    fn curseforge(&self) -> Option<merc_content::curseforge::CurseForge<'_>> {
        curseforge_key(&self.data_dir)
            .map(|k| merc_content::curseforge::CurseForge::new(&self.http, curseforge_api(), k))
    }

    /// Plataformas con los catálogos disponibles (CurseForge solo para mods y con clave).
    fn platforms_of(&self, inst: &Instance) -> Vec<Platform> {
        let cf = curseforge_key(&self.data_dir).is_some();
        let mut list = merc_content::platforms(inst.kind, &inst.root);
        for p in &mut list {
            if cf && p.kind == ContentKind::Mod {
                p.sources.push(ContentSource::Curseforge);
            }
        }
        list
    }

    fn platform(&self, inst: &Instance, kind: ContentKind) -> Result<Platform, ApiError> {
        self.platforms_of(inst)
            .into_iter()
            .find(|p| p.kind == kind)
            .ok_or_else(|| {
                api(
                    ErrorCode::BadRequest,
                    match kind {
                        ContentKind::Mod => "Este servidor no admite mods.",
                        ContentKind::Plugin => "Este servidor no admite plugins.",
                        ContentKind::Datapack => "Este servidor no admite datapacks.",
                    },
                )
            })
    }

    /// Lo instalado, completado con Modrinth si hay conexión.
    fn content_list(&self, inst: &Instance, platform: Platform) -> Result<ContentList, ApiError> {
        let mut items =
            merc_content::local::list_local(&inst.root, &platform.folder, platform.kind.ext()).map_err(content_err)?;
        let mut online = true;
        let mut offline_reason = None;
        if !items.is_empty() {
            let source = merc_content::modrinth::Modrinth::new(&self.http, &self.modrinth_api);
            let mc = inst.mc_version.as_deref();
            if let Err(e) = merc_content::local::enrich(&source, &platform, mc, &mut items) {
                online = false;
                offline_reason = Some(format!("Modrinth: {e}"));
            }
            if let Some(cf) = self
                .curseforge()
                .filter(|_| platform.sources.contains(&ContentSource::Curseforge))
            {
                if let Err(e) = merc_content::curseforge::enrich_curseforge(&cf, &inst.root, &platform, mc, &mut items)
                {
                    online = false;
                    offline_reason = Some(
                        [offline_reason.unwrap_or_default(), format!("CurseForge: {e}")]
                            .join(" ")
                            .trim()
                            .to_string(),
                    );
                }
            }
            if platform.sources.contains(&ContentSource::Hangar) {
                let h = merc_content::hangar::Hangar::new(&self.http, &self.hangar_api);
                let mut cache = self.hangar_cache.lock().unwrap_or_else(|p| p.into_inner());
                if let Err(e) =
                    merc_content::hangar::enrich_hangar(&h, &mut cache, &inst.root, &platform, mc, &mut items)
                {
                    online = false;
                    offline_reason = Some(
                        [offline_reason.unwrap_or_default(), format!("Hangar: {e}")]
                            .join(" ")
                            .trim()
                            .to_string(),
                    );
                }
            }
        }
        Ok(ContentList {
            platform,
            items,
            online,
            offline_reason,
        })
    }

    fn handle_content(&self, client: &str, body: RequestBody) -> Result<ResponseBody, ApiError> {
        use merc_content::modrinth::{Modrinth, Source};
        use RequestBody as R;
        let modrinth = Modrinth::new(&self.http, &self.modrinth_api);
        let curseforge = self.curseforge();
        let hangar = merc_content::hangar::Hangar::new(&self.http, &self.hangar_api);
        let pick = |src: ContentSource, p: &Platform| -> Result<&dyn Source, ApiError> {
            if !p.sources.contains(&src) {
                return Err(api(
                    ErrorCode::BadRequest,
                    match src {
                        ContentSource::Curseforge => "CurseForge no está disponible en esta instalación (falta la clave de API) o no tiene este tipo de contenido.",
                        ContentSource::Hangar => "Hangar solo tiene plugins para Paper y Purpur.",
                        ContentSource::Modrinth => "Modrinth no tiene este tipo de contenido para este servidor.",
                    },
                ));
            }
            Ok(match src {
                ContentSource::Modrinth => &modrinth,
                ContentSource::Hangar => &hangar,
                ContentSource::Curseforge => match &curseforge {
                    Some(c) => c,
                    None => return Err(api(ErrorCode::BadRequest, "CurseForge no está disponible")),
                },
            })
        };
        Ok(match body {
            R::ContentPlatforms { id } => {
                let inst = self.instance(id)?;
                ResponseBody::ContentPlatforms(self.platforms_of(&inst))
            }
            R::ContentSearch {
                id,
                kind,
                source,
                query,
                sort,
                offset,
            } => {
                let inst = self.instance(id)?;
                let p = self.platform(&inst, kind)?;
                let source = pick(source, &p)?;
                ResponseBody::ContentSearch(
                    source
                        .search(&p, inst.mc_version.as_deref(), &query, sort, offset.min(10_000))
                        .map_err(content_err)?,
                )
            }
            R::ContentProject {
                id,
                kind,
                source,
                project,
            } => {
                let inst = self.instance(id)?;
                let p = self.platform(&inst, kind)?;
                let source = pick(source, &p)?;
                let proj = source.project(&project).map_err(content_err)?;
                let mut versions = source
                    .versions(&proj.id, &p.loaders, inst.mc_version.as_deref())
                    .map_err(content_err)?;
                versions.truncate(40);
                for v in &mut versions {
                    // El registro de cambios puede ser largo: se recorta para la ficha.
                    if let Some(c) = &mut v.changelog {
                        if c.chars().count() > 4000 {
                            *c = c.chars().take(4000).collect::<String>() + "…";
                        }
                    }
                }
                let mut proj = proj;
                if proj.body.chars().count() > 20_000 {
                    proj.body = proj.body.chars().take(20_000).collect::<String>() + "…";
                }
                ResponseBody::ContentDetails {
                    project: Box::new(proj),
                    versions,
                }
            }
            R::ContentPlan {
                id,
                kind,
                source,
                version_id,
            } => {
                let inst = self.instance(id)?;
                let p = self.platform(&inst, kind)?;
                let source = pick(source, &p)?;
                let installed = self.content_list(&inst, p.clone())?;
                ResponseBody::ContentPlan(
                    merc_content::plan::build_plan(
                        source,
                        &p,
                        inst.mc_version.as_deref(),
                        &version_id,
                        &installed.items,
                    )
                    .map_err(content_err)?,
                )
            }
            R::ContentInstall {
                id,
                kind,
                source,
                version_id,
            } => {
                let inst = self.instance(id)?;
                let p = self.platform(&inst, kind)?;
                let source = pick(source, &p)?;
                self.require_stopped(id, "instalar mods o plugins (se cargan al encender)")?;
                let installed = self.content_list(&inst, p.clone())?;
                if !installed.online {
                    return Err(api(
                        ErrorCode::Network,
                        "No se pudo consultar el catálogo para revisar lo instalado. Comprueba tu conexión.",
                    ));
                }
                let plan = merc_content::plan::build_plan(
                    source,
                    &p,
                    inst.mc_version.as_deref(),
                    &version_id,
                    &installed.items,
                )
                .map_err(content_err)?;
                if !plan.blockers.is_empty() {
                    return Err(api(ErrorCode::Conflict, plan.blockers.join(" ")));
                }
                let title = plan.items.first().map(|i| i.title.clone()).unwrap_or_default();
                let staging = self.store_dir(id).join("descargas").join(merc_admin::stamp());
                let ex = self.explorer(&inst)?;
                let placed = self.world_op(&inst, "install", &title, |progress| {
                    let ip = merc_content::local::InstallProgress::default();
                    // Se reflejan los bytes en el progreso común (el mismo que usan los mundos).
                    let done = std::sync::atomic::AtomicBool::new(false);
                    let r = std::thread::scope(|sc| {
                        sc.spawn(|| {
                            while !done.load(Ordering::Relaxed) {
                                progress.done.store(ip.done.load(Ordering::Relaxed), Ordering::Relaxed);
                                progress
                                    .total
                                    .store(ip.total.load(Ordering::Relaxed), Ordering::Relaxed);
                                if progress.cancel.load(Ordering::Relaxed) {
                                    ip.cancel.store(true, Ordering::Relaxed);
                                }
                                std::thread::sleep(Duration::from_millis(150));
                            }
                        });
                        let r = merc_content::local::apply_plan(
                            &self.http,
                            &inst.root,
                            &p.folder,
                            p.kind.ext(),
                            &plan,
                            &staging,
                            &mut |rel| ex.trash(rel).map(|_| ()).map_err(|e| e.to_string()),
                            &ip,
                        );
                        done.store(true, Ordering::Relaxed);
                        r
                    });
                    r.map_err(|e| match e {
                        merc_content::ContentError::Cancelled => merc_admin::AdminError::Cancelled,
                        merc_content::ContentError::Conflict(m) => merc_admin::AdminError::Conflict(m),
                        merc_content::ContentError::Invalid(m) => merc_admin::AdminError::Invalid(m),
                        other => merc_admin::AdminError::Io(other.to_string()),
                    })
                })?;
                for it in &plan.items {
                    self.audit(
                        client,
                        Some(id),
                        "content.install",
                        &format!(
                            "{} {} ({}){}",
                            it.title,
                            it.version_number,
                            it.file_name,
                            it.replaces
                                .as_ref()
                                .map(|r| format!(", reemplaza {r}"))
                                .unwrap_or_default()
                        ),
                    );
                }
                self.log
                    .line(&format!("contenido instalado en {id}: {}", placed.join(", ")));
                ResponseBody::ContentList(self.content_list(&inst, p)?)
            }
            R::InstallFloodgate { id } => {
                let inst = self.instance(id)?;
                let p = self.platform(&inst, ContentKind::Plugin).map_err(|_| {
                    api(
                        ErrorCode::BadRequest,
                        "Floodgate como plugin es para Paper, Purpur o Spigot. En Fabric o NeoForge se instala como mod desde Modrinth.",
                    )
                })?;
                self.require_stopped(id, "instalar Floodgate (se carga al encender)")?;
                if merc_admin::geyser::find(&inst.root).1.is_some() {
                    return Err(api(
                        ErrorCode::Conflict,
                        "Floodgate ya está instalado en este servidor.",
                    ));
                }
                let fg =
                    merc_content::geysermc::floodgate_latest(&self.http, &self.geysermc_api).map_err(content_err)?;
                let staging = self.store_dir(id).join("descargas").join(merc_admin::stamp());
                let file = merc_content::geysermc::install_floodgate(
                    &self.http,
                    &self.geysermc_api,
                    &inst.root,
                    &p.folder,
                    &staging,
                    &fg,
                    &std::sync::atomic::AtomicBool::new(false),
                )
                .map_err(content_err)?;
                self.audit(
                    client,
                    Some(id),
                    "content.install",
                    &format!(
                        "Floodgate {} (compilación {}) desde GeyserMC, {file}",
                        fg.version, fg.build
                    ),
                );
                self.log.line(&format!("Floodgate instalado en {id}: {file}"));
                ResponseBody::ContentList(self.content_list(&inst, p)?)
            }
            R::ContentInstalled { id, kind } => {
                let inst = self.instance(id)?;
                let p = self.platform(&inst, kind)?;
                ResponseBody::ContentList(self.content_list(&inst, p)?)
            }
            R::ContentToggle {
                id,
                kind,
                file_name,
                enabled,
            } => {
                let inst = self.instance(id)?;
                let p = self.platform(&inst, kind)?;
                self.require_stopped(id, "activar o desactivar mods o plugins")?;
                let name = merc_content::local::set_enabled(&inst.root, &p.folder, p.kind.ext(), &file_name, enabled)
                    .map_err(content_err)?;
                self.audit(
                    client,
                    Some(id),
                    if enabled { "content.enable" } else { "content.disable" },
                    &name,
                );
                ResponseBody::ContentList(self.content_list(&inst, p)?)
            }
            R::ContentRemove { id, kind, file_name } => {
                let inst = self.instance(id)?;
                let p = self.platform(&inst, kind)?;
                self.require_stopped(id, "quitar mods o plugins")?;
                let base = file_name
                    .strip_suffix(merc_content::local::DISABLED_SUFFIX)
                    .unwrap_or(&file_name);
                merc_content::safe_file_name(base, p.kind.ext()).map_err(content_err)?;
                let rel = format!("{}/{file_name}", p.folder);
                if !inst.root.join(&rel).is_file() {
                    return Err(api(ErrorCode::NotFound, format!("No existe «{file_name}»")));
                }
                self.explorer(&inst)?.trash(&rel).map_err(admin_err)?;
                self.audit(client, Some(id), "content.remove", &file_name);
                ResponseBody::ContentList(self.content_list(&inst, p)?)
            }
            _ => return Err(api(ErrorCode::BadRequest, "Petición no válida")),
        })
    }

    fn world_op_running(&self, id: InstanceId) -> bool {
        self.world_ops
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains_key(&id)
    }

    /// Manda la carpeta a la Papelera de reciclaje (recuperable desde Windows).
    fn recycle(&self, path: &Path) -> Result<(), String> {
        #[cfg(debug_assertions)]
        if let Some(dir) = std::env::var_os("MERC_RECYCLE_DIR") {
            let name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
            let dest = PathBuf::from(dir).join(merc_admin::stamp()).join(name);
            std::fs::create_dir_all(dest.parent().unwrap_or(&dest)).map_err(|e| e.to_string())?;
            return std::fs::rename(path, &dest).map_err(|e| e.to_string());
        }
        trash::delete(path).map_err(|e| e.to_string())
    }

    /// Antes de mandar la carpeta de un servidor a la papelera: que sea de verdad la carpeta de
    /// un servidor y no algo que contenga otras cosas importantes.
    fn check_deletable(&self, inst: &Instance) -> Result<PathBuf, ApiError> {
        let refuse = |why: &str| {
            api(
                ErrorCode::Conflict,
                format!("No se borró la carpeta: {why}. Bórrala tú desde el Explorador si de verdad quieres."),
            )
        };
        let meta = std::fs::symlink_metadata(&inst.root).map_err(|_| refuse("ya no existe o no se puede leer"))?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(refuse("no es una carpeta normal (es un enlace)"));
        }
        let root = std::fs::canonicalize(&inst.root).map_err(|_| refuse("no se pudo resolver su ruta"))?;
        if root.parent().is_none() || root.components().count() < 3 {
            return Err(refuse("es la raíz de un disco o está demasiado arriba"));
        }
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
        let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        let mut protected: Vec<PathBuf> = vec![canon(&self.data_dir), canon(&default_servers_dir())];
        if let Some(h) = home {
            for sub in [
                "",
                "Desktop",
                "Documents",
                "Downloads",
                "Escritorio",
                "Documentos",
                "Descargas",
                "OneDrive",
            ] {
                protected.push(canon(&h.join(sub)));
            }
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                protected.push(canon(dir));
            }
        }
        if protected.iter().any(|p| p.starts_with(&root)) {
            return Err(refuse("contiene carpetas importantes de tu PC o de MercHosting"));
        }
        let others = self
            .store
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .list_instances()
            .map_err(store_err)?;
        if others
            .iter()
            .filter(|o| o.id != inst.id)
            .any(|o| canon(&o.root).starts_with(&root) || root.starts_with(canon(&o.root)))
        {
            return Err(refuse("comparte carpeta con otro servidor de la lista"));
        }
        let looks_like_server = ["server.properties", "eula.txt"].iter().any(|f| root.join(f).is_file())
            || std::fs::read_dir(&root)
                .into_iter()
                .flatten()
                .flatten()
                .any(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("jar")));
        if !looks_like_server {
            return Err(refuse("no parece la carpeta de un servidor de Minecraft"));
        }
        Ok(root)
    }

    fn handle_admin(self: &Arc<Self>, client: &str, body: RequestBody) -> Result<ResponseBody, ApiError> {
        use RequestBody as R;
        Ok(match body {
            R::Properties { id } => self.properties_body(&self.instance(id)?)?,
            R::SetProperties {
                id,
                changes,
                expected_modified,
            } => {
                let inst = self.instance(id)?;
                let ex = self.explorer(&inst)?;
                let current = match ex.read_text("server.properties") {
                    Ok(f) => f.content,
                    Err(merc_admin::AdminError::NotFound(_)) => String::new(),
                    Err(e) => return Err(admin_err(e)),
                };
                let raw = inst.kind == ServerKind::Bedrock;
                let text = merc_admin::props::apply_changes_for(&current, &changes, raw).map_err(admin_err)?;
                ex.write_text("server.properties", &text, expected_modified)
                    .map_err(admin_err)?;
                let detail = changes
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.audit(client, Some(id), "server.properties.set", &detail);
                self.properties_body(&inst)?
            }
            R::SetResourcePack { id, url, required } => {
                let inst = self.instance(id)?;
                let ex = self.explorer(&inst)?;
                let url = url.trim().to_string();
                let mut changes = BTreeMap::new();
                let detail = if url.is_empty() {
                    changes.insert("resource-pack".to_string(), String::new());
                    changes.insert("resource-pack-sha1".to_string(), String::new());
                    changes.insert("require-resource-pack".to_string(), "false".to_string());
                    "quitado".to_string()
                } else {
                    let staging = self.store_dir(id).join("descargas").join(merc_admin::stamp());
                    let info = merc_content::resourcepack::inspect(&self.http, &url, &staging).map_err(content_err)?;
                    changes.insert("resource-pack".to_string(), info.url.clone());
                    changes.insert("resource-pack-sha1".to_string(), info.sha1.clone());
                    changes.insert("require-resource-pack".to_string(), required.to_string());
                    format!("{} (sha1 {}, {} bytes)", info.url, info.sha1, info.size)
                };
                let current = match ex.read_text("server.properties") {
                    Ok(f) => f,
                    Err(merc_admin::AdminError::NotFound(_)) => {
                        return Err(api(
                            ErrorCode::NotFound,
                            "Todavía no hay server.properties: enciende el servidor una vez.",
                        ))
                    }
                    Err(e) => return Err(admin_err(e)),
                };
                let text = merc_admin::props::apply_changes(&current.content, &changes).map_err(admin_err)?;
                ex.write_text("server.properties", &text, current.modified_unix_ms)
                    .map_err(admin_err)?;
                self.audit(client, Some(id), "server.resource_pack", &detail);
                self.properties_body(&inst)?
            }
            R::Players { id } => {
                let inst = self.instance(id)?;
                let st = self.sup.status(id);
                let (_, online) = self.sup.metrics(id);
                ResponseBody::Players {
                    lists: merc_admin::players::read_lists(&inst.root),
                    online,
                    can_act: st.state == merc_core::RuntimeState::Online && st.has_console,
                }
            }
            R::PlayerAction {
                id,
                action,
                name,
                reason,
            } => {
                self.instance(id)?;
                let cmd =
                    merc_admin::players::command_for(action, name.trim(), reason.as_deref()).map_err(admin_err)?;
                let st = self.sup.status(id);
                if st.state != merc_core::RuntimeState::Online {
                    return Err(api(
                        ErrorCode::InvalidState,
                        "Enciende el servidor: estos cambios los hace Minecraft con sus comandos oficiales.",
                    ));
                }
                self.sup.send_command(id, &cmd).map_err(sup_err)?;
                self.audit(client, Some(id), "player.action", &cmd);
                ResponseBody::Ok
            }
            R::ListFiles { id, path } => {
                let inst = self.instance(id)?;
                let entries = self.explorer(&inst)?.list(&path).map_err(admin_err)?;
                ResponseBody::Files { path, entries }
            }
            R::ReadFile { id, path } => {
                let inst = self.instance(id)?;
                ResponseBody::TextFile(self.explorer(&inst)?.read_text(&path).map_err(admin_err)?)
            }
            R::WriteFile {
                id,
                path,
                content,
                expected_modified,
            } => {
                let inst = self.instance(id)?;
                self.require_stopped(id, "guardar archivos")?;
                let f = self
                    .explorer(&inst)?
                    .write_text(&path, &content, expected_modified)
                    .map_err(admin_err)?;
                self.audit(client, Some(id), "file.write", &f.path);
                ResponseBody::TextFile(f)
            }
            R::MakeDir { id, path } => {
                let inst = self.instance(id)?;
                self.require_stopped(id, "crear carpetas")?;
                self.explorer(&inst)?.mkdir(&path).map_err(admin_err)?;
                self.audit(client, Some(id), "file.mkdir", &path);
                ResponseBody::FileDone { path }
            }
            R::RenameFile { id, path, new_name } => {
                let inst = self.instance(id)?;
                self.require_stopped(id, "renombrar archivos")?;
                let to = self.explorer(&inst)?.rename(&path, &new_name).map_err(admin_err)?;
                self.audit(client, Some(id), "file.rename", &format!("{path} -> {to}"));
                ResponseBody::FileDone { path: to }
            }
            R::MoveFile { id, path, dest_dir } => {
                let inst = self.instance(id)?;
                self.require_stopped(id, "mover archivos")?;
                let to = self.explorer(&inst)?.move_to(&path, &dest_dir).map_err(admin_err)?;
                self.audit(client, Some(id), "file.move", &format!("{path} -> {to}"));
                ResponseBody::FileDone { path: to }
            }
            R::CopyFile { id, path, dest_dir } => {
                let inst = self.instance(id)?;
                self.require_stopped(id, "copiar archivos")?;
                let to = self.explorer(&inst)?.copy_to(&path, &dest_dir).map_err(admin_err)?;
                self.audit(client, Some(id), "file.copy", &format!("{path} -> {to}"));
                ResponseBody::FileDone { path: to }
            }
            R::ExtractZip { id, path } => {
                let inst = self.instance(id)?;
                self.require_stopped(id, "extraer archivos")?;
                let r = self.explorer(&inst)?.extract_zip(&path).map_err(admin_err)?;
                self.audit(client, Some(id), "file.extract", &format!("{path} -> {}", r.folder));
                ResponseBody::ZipExtracted(r)
            }
            R::TrashFile { id, path } => {
                let inst = self.instance(id)?;
                self.require_stopped(id, "mover archivos a la papelera")?;
                let dest = self.explorer(&inst)?.trash(&path).map_err(admin_err)?;
                self.audit(client, Some(id), "file.trash", &format!("{path} -> {}", dest.display()));
                ResponseBody::FileDone {
                    path: dest.display().to_string(),
                }
            }
            R::UploadFile { id, source, dest_dir } => {
                let inst = self.instance(id)?;
                self.require_stopped(id, "subir archivos")?;
                if !source.is_absolute() {
                    return Err(api(ErrorCode::BadRequest, "Archivo de origen no válido"));
                }
                let rel = self.explorer(&inst)?.upload(&source, &dest_dir).map_err(admin_err)?;
                self.audit(client, Some(id), "file.upload", &rel);
                ResponseBody::FileDone { path: rel }
            }
            R::DownloadFile { id, path, dest } => {
                let inst = self.instance(id)?;
                self.explorer(&inst)?.download(&path, &dest).map_err(admin_err)?;
                self.audit(client, Some(id), "file.download", &path);
                ResponseBody::FileDone {
                    path: dest.display().to_string(),
                }
            }
            R::ExportConsole { id, dest } => {
                self.instance(id)?;
                if !dest.is_absolute() {
                    return Err(api(ErrorCode::BadRequest, "Destino no válido"));
                }
                let log = self
                    .sup
                    .status(id)
                    .log_file
                    .unwrap_or_else(|| self.data_dir.join("logs").join(id.to_string()).join("console.log"));
                std::fs::copy(&log, &dest).map_err(|e| {
                    api(
                        if e.kind() == std::io::ErrorKind::NotFound {
                            ErrorCode::NotFound
                        } else {
                            ErrorCode::Internal
                        },
                        if e.kind() == std::io::ErrorKind::NotFound {
                            "Todavía no hay registro de consola: enciende el servidor al menos una vez.".to_string()
                        } else {
                            e.to_string()
                        },
                    )
                })?;
                ResponseBody::FileDone {
                    path: dest.display().to_string(),
                }
            }
            R::Worlds { id } => Self::worlds_body(&self.instance(id)?)?,
            R::DuplicateWorld { id, name, new_name } => {
                let inst = self.instance(id)?;
                self.require_stopped(id, "duplicar un mundo")?;
                self.world_op(&inst, "duplicate", &name, |p| {
                    merc_admin::worlds::duplicate(&inst.root, &name, &new_name, p)
                })?;
                self.audit(client, Some(id), "world.duplicate", &format!("{name} -> {new_name}"));
                Self::worlds_body(&inst)?
            }
            R::ExportWorld { id, name, dest } => {
                let inst = self.instance(id)?;
                self.require_stopped(id, "exportar un mundo (así queda guardado del todo)")?;
                self.world_op(&inst, "export", &name, |p| {
                    merc_admin::worlds::export_zip(&inst.root, &name, &dest, p)
                })?;
                self.audit(
                    client,
                    Some(id),
                    "world.export",
                    &format!("{name} -> {}", dest.display()),
                );
                ResponseBody::FileDone {
                    path: dest.display().to_string(),
                }
            }
            R::ImportWorld { id, source, new_name } => {
                let inst = self.instance(id)?;
                self.require_stopped(id, "importar un mundo")?;
                if !source.is_absolute() {
                    return Err(api(ErrorCode::BadRequest, "Archivo de origen no válido"));
                }
                self.world_op(&inst, "import", &new_name, |p| {
                    merc_admin::worlds::import_zip(&inst.root, &source, &new_name, p)
                })?;
                self.audit(
                    client,
                    Some(id),
                    "world.import",
                    &format!("{} -> {new_name}", source.display()),
                );
                Self::worlds_body(&inst)?
            }
            R::SetActiveWorld { id, name } => {
                let inst = self.instance(id)?;
                self.require_stopped(id, "cambiar el mundo activo")?;
                let name = merc_admin::worlds::validate_world_name(&name).map_err(admin_err)?;
                if !inst.root.join(&name).join("level.dat").is_file() {
                    return Err(api(ErrorCode::NotFound, format!("No existe el mundo «{name}»")));
                }
                let ex = self.explorer(&inst)?;
                let current = match ex.read_text("server.properties") {
                    Ok(f) => f.content,
                    Err(merc_admin::AdminError::NotFound(_)) => String::new(),
                    Err(e) => return Err(admin_err(e)),
                };
                let changes = [("level-name".to_string(), name.clone())].into_iter().collect();
                let text = merc_admin::props::apply_changes(&current, &changes).map_err(admin_err)?;
                ex.write_text("server.properties", &text, None).map_err(admin_err)?;
                self.audit(client, Some(id), "world.activate", &name);
                Self::worlds_body(&inst)?
            }
            R::TrashWorld { id, name } => {
                let inst = self.instance(id)?;
                self.require_stopped(id, "mover un mundo a la papelera")?;
                let name = merc_admin::worlds::validate_world_name(&name).map_err(admin_err)?;
                if name == Self::level_name(&inst) {
                    return Err(api(
                        ErrorCode::Conflict,
                        "Es el mundo activo. Activa otro antes, o el servidor crearía uno nuevo al encender.",
                    ));
                }
                let ex = self.explorer(&inst)?;
                for suf in ["", "_nether", "_the_end"] {
                    let dir = format!("{name}{suf}");
                    if inst.root.join(&dir).is_dir() {
                        ex.trash(&dir).map_err(admin_err)?;
                    }
                }
                self.audit(client, Some(id), "world.trash", &name);
                Self::worlds_body(&inst)?
            }
            R::AdminState { id } => ResponseBody::AdminState(self.admin_state(&self.instance(id)?)),
            R::SetMaintenance { id, on } => {
                let inst = self.instance(id)?;
                self.set_maintenance(client, &inst, on)?;
                ResponseBody::AdminState(self.admin_state(&inst))
            }
            R::CancelRestart { id } => {
                let inst = self.instance(id)?;
                let found = self
                    .pending_restarts
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .get(&id)
                    .map(|p| p.cancel.store(true, Ordering::SeqCst))
                    .is_some();
                if !found {
                    return Err(api(ErrorCode::NotFound, "No hay un reinicio avisado pendiente"));
                }
                self.audit(client, Some(id), "server.restart.cancel", "");
                ResponseBody::AdminState(self.admin_state(&inst))
            }
            R::CancelWorldOp { id } => {
                let inst = self.instance(id)?;
                match self.world_ops.lock().unwrap_or_else(|p| p.into_inner()).get(&id) {
                    Some(op) => op.progress.cancel.store(true, Ordering::Relaxed),
                    None => return Err(api(ErrorCode::NotFound, "No hay ninguna operación en marcha")),
                }
                ResponseBody::AdminState(self.admin_state(&inst))
            }
            R::SavedItems { id } => ResponseBody::SavedItems(self.explorer(&self.instance(id)?)?.saved_items()),
            R::RestoreSaved { id, item_id } => {
                let inst = self.instance(id)?;
                self.require_stopped(id, "restaurar archivos")?;
                let path = self.explorer(&inst)?.restore(&item_id).map_err(admin_err)?;
                self.audit(client, Some(id), "file.restore", &format!("{item_id} -> {path}"));
                ResponseBody::FileDone { path }
            }
            R::PurgeSaved { id, item_id } => {
                let inst = self.instance(id)?;
                self.explorer(&inst)?.purge(&item_id).map_err(admin_err)?;
                self.audit(client, Some(id), "file.purge", &item_id);
                ResponseBody::Ok
            }
            R::GameRules { id } => self.game_rules(&self.instance(id)?)?,
            R::SetGameRule { id, key, value } => {
                let inst = self.instance(id)?;
                let cmd = merc_admin::gamerules::set_command(&key, value.trim(), inst.mc_version.as_deref())
                    .map_err(admin_err)?;
                if self.sup.status(id).state != merc_core::RuntimeState::Online {
                    return Err(api(
                        ErrorCode::InvalidState,
                        "Enciende el servidor para cambiar las reglas",
                    ));
                }
                self.sup.send_command(id, &cmd).map_err(sup_err)?;
                self.audit(client, Some(id), "world.gamerule", &cmd);
                ResponseBody::Ok
            }
            R::Activity { id, limit } => {
                self.instance(id)?;
                let limit = if limit == 0 { 200 } else { limit.min(1000) };
                let rows = self
                    .store
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .recent_audit(Some(id), limit)
                    .map_err(store_err)?;
                ResponseBody::Activity(
                    rows.into_iter()
                        .map(|r| ActivityEntry {
                            at_unix: r.at_unix,
                            actor: r.actor,
                            action: r.action,
                            detail: r.detail,
                        })
                        .collect(),
                )
            }
            R::RestartWithNotice { id, seconds } => {
                let inst = self.instance(id)?;
                self.restart_with_notice(client, inst, seconds)?;
                ResponseBody::Ok
            }
            _ => return Err(api(ErrorCode::Internal, "operación no administrativa")),
        })
    }

    fn used_ports(&self) -> Vec<u16> {
        let list = self
            .store
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .list_instances()
            .unwrap_or_default();
        list.iter()
            .filter_map(|i| merc_core::properties::ServerProperties::read(&i.root).ok().flatten())
            .map(|p| p.port)
            .collect()
    }

    /// Primer puerto desde 25565 que no use otro servidor registrado ni esté ocupado ahora.
    fn suggested_port(&self) -> u16 {
        let used = self.used_ports();
        (merc_core::properties::DEFAULT_PORT..=25665)
            .find(|p| !used.contains(p) && std::net::TcpListener::bind(("0.0.0.0", *p)).is_ok())
            .unwrap_or(merc_core::properties::DEFAULT_PORT)
    }

    fn catalog(&self, flavor: Flavor) -> Result<Vec<CatalogEntry>, ApiError> {
        let cached = self
            .catalog_cache
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&flavor)
            .cloned();
        if let Some((at, list)) = &cached {
            if at.elapsed() < CATALOG_TTL {
                return Ok(list.clone());
            }
        }
        match merc_install::list_versions(&self.http, &self.sources, flavor) {
            Ok(list) => {
                self.catalog_cache
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .insert(flavor, (Instant::now(), list.clone()));
                Ok(list)
            }
            // Sin conexión: mejor una lista algo antigua que ninguna.
            Err(e) => cached.map(|(_, l)| l).ok_or_else(|| install_err(e)),
        }
    }

    fn require_curseforge(&self) -> Result<merc_content::curseforge::CurseForge<'_>, ApiError> {
        self.curseforge().ok_or_else(|| {
            api(
                ErrorCode::BadRequest,
                "CurseForge estará disponible próximamente: esta instalación aún no tiene la clave de API oficial.",
            )
        })
    }

    /// ZIP de un modpack de CurseForge (por identificador de archivo), verificado y en caché.
    fn cf_modpack_file(
        &self,
        file_id: &str,
        cancel: &AtomicBool,
    ) -> Result<(PathBuf, merc_content::modrinth::ContentVersion), ApiError> {
        use merc_content::modrinth::Source;
        let cf = self.require_curseforge()?;
        let v = cf.version(file_id).map_err(content_err)?;
        let f = v
            .files
            .first()
            .ok_or_else(|| api(ErrorCode::NotFound, "Ese archivo de CurseForge no existe"))?;
        if !f.filename.to_lowercase().ends_with(".zip") {
            return Err(api(
                ErrorCode::BadRequest,
                "Ese archivo no es un modpack (.zip) de CurseForge",
            ));
        }
        if f.url.is_empty() {
            return Err(api(
                ErrorCode::Conflict,
                "El autor de este modpack no permite descargarlo desde otras aplicaciones.",
            ));
        }
        let sha1 = f
            .sha1
            .clone()
            .filter(|h| h.len() == 40)
            .ok_or_else(|| api(ErrorCode::Internal, "CurseForge no publica la huella de este modpack"))?;
        let dir = self.data_dir.join("cache").join("modpacks");
        std::fs::create_dir_all(&dir).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        let path = dir.join(format!("{sha1}.zip"));
        let ok = path.is_file()
            && merc_content::local::sha1_file(&path)
                .map(|h| h.eq_ignore_ascii_case(&sha1))
                .unwrap_or(false);
        if !ok {
            let _ = std::fs::remove_file(&path);
            self.http
                .download(
                    &f.url,
                    &path,
                    &merc_install::Checksum::Sha1(sha1),
                    Some(f.size).filter(|s| *s > 0),
                    &mut |_, _| {},
                    cancel,
                )
                .map_err(install_err)?;
        }
        Ok((path, v))
    }

    fn load_modpack(
        &self,
        source: ContentSource,
        version_id: &str,
        cancel: &AtomicBool,
    ) -> Result<LoadedPack, ApiError> {
        match source {
            ContentSource::Hangar => Err(api(ErrorCode::BadRequest, NO_HANGAR_PACKS)),
            ContentSource::Modrinth => {
                let (path, v) = self.modpack_file(version_id, cancel)?;
                let index = merc_content::mrpack::read_index(&path).map_err(content_err)?;
                Ok(LoadedPack {
                    path,
                    index,
                    project_id: v.project_id,
                    version_number: v.version_number,
                    overrides: vec!["overrides/".into(), "server-overrides/".into()],
                    warnings: vec![],
                })
            }
            ContentSource::Curseforge => {
                let (path, v) = self.cf_modpack_file(version_id, cancel)?;
                let manifest = merc_content::cfpack::read_manifest(&path).map_err(content_err)?;
                let cf = self.require_curseforge()?;
                let (index, warnings) = merc_content::cfpack::resolve(&cf, &manifest).map_err(content_err)?;
                Ok(LoadedPack {
                    path,
                    index,
                    project_id: v.project_id,
                    version_number: if v.version_number.is_empty() {
                        manifest.version.clone()
                    } else {
                        v.version_number
                    },
                    overrides: vec![format!("{}/", manifest.overrides)],
                    warnings,
                })
            }
        }
    }

    /// `.mrpack` de una versión de Modrinth, descargado y verificado una vez en la caché.
    fn modpack_file(
        &self,
        version_id: &str,
        cancel: &AtomicBool,
    ) -> Result<(PathBuf, merc_content::modrinth::ContentVersion), ApiError> {
        use merc_content::modrinth::Source;
        let m = merc_content::modrinth::Modrinth::new(&self.http, &self.modrinth_api);
        let v = m.version(version_id).map_err(content_err)?;
        let file = v
            .files
            .iter()
            .find(|f| f.primary && f.filename.to_lowercase().ends_with(".mrpack"))
            .or_else(|| v.files.iter().find(|f| f.filename.to_lowercase().ends_with(".mrpack")))
            .ok_or_else(|| {
                api(
                    ErrorCode::BadRequest,
                    "Esa versión no es un modpack de Modrinth (.mrpack)",
                )
            })?;
        let sha1 = file
            .sha1
            .clone()
            .filter(|h| h.len() == 40)
            .ok_or_else(|| api(ErrorCode::Internal, "Modrinth no publica la huella del modpack"))?;
        let dir = self.data_dir.join("cache").join("modpacks");
        std::fs::create_dir_all(&dir).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        let path = dir.join(format!("{}.mrpack", sha1.to_lowercase()));
        let ok = path.is_file()
            && merc_content::local::sha1_file(&path)
                .map(|h| h.eq_ignore_ascii_case(&sha1))
                .unwrap_or(false);
        if !ok {
            let _ = std::fs::remove_file(&path);
            self.http
                .download(
                    &file.url,
                    &path,
                    &merc_install::Checksum::Sha1(sha1.clone()),
                    Some(file.size).filter(|s| *s > 0),
                    &mut |_, _| {},
                    cancel,
                )
                .map_err(install_err)?;
        }
        Ok((path, v))
    }

    fn start_create(self: &Arc<Self>, client: &str, p: CreateParams) -> Result<ResponseBody, ApiError> {
        let name = validate_instance_name(&p.name).map_err(|e| api(ErrorCode::BadRequest, e))?;
        if !p.path.is_absolute() {
            return Err(api(ErrorCode::BadRequest, "La carpeta debe ser una ruta completa"));
        }
        merc_install::check_target(&p.path).map_err(install_err)?;
        let native = p.flavor.is_native();
        if native {
            // Bedrock: ejecutable nativo de Mojang. Sin Java, sin modpacks de Java ni reglas de Java.
            if p.modpack_version.is_some() {
                return Err(api(ErrorCode::BadRequest, "Bedrock no usa modpacks de Java"));
            }
            if !p.game_rules.is_empty() {
                return Err(api(
                    ErrorCode::BadRequest,
                    "Las reglas del juego de Bedrock se cambian desde la consola una vez creado",
                ));
            }
        } else if !p.java.is_absolute() || !p.java.is_file() {
            return Err(api(
                ErrorCode::BadRequest,
                "Indica la ruta completa del ejecutable de Java",
            ));
        }
        merc_install::validate_properties_for(p.flavor, &p.properties).map_err(install_err)?;
        if p.game_rules.len() > merc_admin::gamerules::RULES.len() {
            return Err(api(ErrorCode::BadRequest, "Demasiadas reglas del juego"));
        }
        for (k, v) in &p.game_rules {
            merc_admin::gamerules::set_command(k, v.trim(), Some(&p.mc_version)).map_err(admin_err)?;
        }
        if !native {
            LaunchProfile {
                java: p.java.clone(),
                min_memory_mb: p.min_memory_mb,
                max_memory_mb: p.max_memory_mb,
                extra_jvm_args: vec![],
                target: merc_core::launch::LaunchTarget::Jar {
                    jar: "server.jar".into(),
                },
            }
            .validate()
            .map_err(|e| api(ErrorCode::BadRequest, e.to_string()))?;
        }
        let job = self
            .jobs
            .create(name.clone(), p.path.clone())
            .map_err(|e| api(ErrorCode::Conflict, e))?;
        self.audit(
            client,
            None,
            "server.create",
            &format!(
                "{:?} {} en {} (EULA {})",
                p.flavor,
                p.mc_version,
                p.path.display(),
                if p.accept_eula {
                    "aceptada expresamente por el usuario"
                } else {
                    "no aceptada"
                }
            ),
        );
        let snap = job.snapshot(0);
        let d = self.clone();
        let p = CreateParams { name, ..p };
        std::thread::spawn(move || d.run_create(job, p));
        Ok(ResponseBody::Job(snap))
    }

    fn run_create(self: Arc<Self>, job: Arc<jobs::Job>, p: CreateParams) {
        use merc_ipc::JobState;
        let fail = |job: &jobs::Job, e: InstallError| {
            if e == InstallError::Cancelled {
                job.finish(JobState::Cancelled, None, None)
            } else {
                job.finish(JobState::Failed, Some(e.to_string()), None)
            }
        };
        // Modpack: se lee primero para fijar Minecraft y el loader exactos.
        let mut modpack: Option<(LoadedPack, merc_content::mrpack::PackIndex)> = None;
        if let Some(vid) = &p.modpack_version {
            job.set_step(Step::Preparing, "Leyendo el modpack");
            let pack = match self.load_modpack(p.modpack_source, vid, &job.cancel) {
                Ok(x) => x,
                Err(e) => return job.finish(JobState::Failed, Some(e.message), None),
            };
            for w in &pack.warnings {
                job.log(format!("⚠ {w}"));
            }
            let index = pack.index.clone();
            if merc_content::mrpack::flavor_of(index.loader.as_deref()) != p.flavor || index.mc_version != p.mc_version
            {
                return job.finish(
                    JobState::Failed,
                    Some("El modpack no coincide con el tipo o la versión elegidos. Vuelve a elegirlo.".into()),
                    None,
                );
            }
            job.log(format!(
                "Modpack «{}» {}: Minecraft {}{} · {} archivos para el servidor",
                index.name,
                index.version_id,
                index.mc_version,
                index
                    .loader
                    .as_ref()
                    .map(|l| format!(", {l} {}", index.loader_version.clone().unwrap_or_default()))
                    .unwrap_or_default(),
                index.files.len()
            ));
            modpack = Some((pack, index));
        }
        job.set_step(Step::Preparing, "Consultando la fuente oficial");
        // El modpack manda; si no hay, la versión que eligió la persona.
        let pinned = modpack
            .as_ref()
            .and_then(|(_, i)| i.loader_version.clone())
            .or_else(|| p.loader_version.clone());
        let resolved =
            match merc_install::resolve_pinned(&self.http, &self.sources, p.flavor, &p.mc_version, pinned.as_deref()) {
                Ok(r) => r,
                Err(e) => return fail(&job, e),
            };
        job.log(format!(
            "{:?} {}{} · Java {} recomendado",
            resolved.flavor,
            resolved.mc_version,
            resolved
                .loader_version
                .as_ref()
                .map(|l| format!(" ({l})"))
                .unwrap_or_default(),
            resolved.java_major
        ));
        for n in &resolved.notes {
            job.log(format!("ⓘ {n}"));
        }
        // El Java elegido debe cumplir lo que pide esta versión de Minecraft.
        let min = merc_core::java::required_java(&resolved.mc_version)
            .map(|r| r.min_major)
            .unwrap_or(resolved.java_major)
            .max(if p.flavor == Flavor::Vanilla {
                resolved.java_major
            } else {
                0
            });
        if p.flavor.is_native() {
            job.log("Bedrock Dedicated Server no usa Java");
        } else {
            let legacy_forge =
                p.flavor == Flavor::Forge && merc_install::catalog::forge_is_legacy(&resolved.mc_version);
            match merc_core::java::probe_java(&p.java, Duration::from_secs(15)) {
                Ok(probe) if legacy_forge && probe.major != 8 => {
                    return job.finish(
                        JobState::Failed,
                        Some(format!(
                            "Forge {} solo funciona con Java 8 y el elegido es Java {}. Elige o instala Java 8.",
                            resolved.mc_version, probe.major
                        )),
                        None,
                    )
                }
                Ok(probe) if probe.major >= min => {
                    job.log(format!("Java {} encontrado en {}", probe.major, p.java.display()))
                }
                Ok(probe) => {
                    return job.finish(
                        JobState::Failed,
                        Some(format!(
                            "Minecraft {} necesita Java {min} o superior y el elegido es Java {}",
                            resolved.mc_version, probe.major
                        )),
                        None,
                    )
                }
                Err(e) => return job.finish(JobState::Failed, Some(format!("No se pudo usar ese Java: {e}")), None),
            }
        }
        if job.cancel.load(Ordering::SeqCst) {
            return job.finish(JobState::Cancelled, None, None);
        }
        let plan = InstallPlan {
            resolved: resolved.clone(),
            target: p.path.clone(),
            java: p.java.clone(),
            properties: p.properties.clone(),
            accept_eula: p.accept_eula,
            // BuildTools reutiliza su carpeta de trabajo entre creaciones de la misma versión.
            cache_dir: Some(
                self.data_dir
                    .join("cache")
                    .join(format!("buildtools-{}", resolved.mc_version)),
            ),
        };
        // Los archivos del modpack se descargan y verifican antes de tocar la carpeta.
        let staging = self
            .data_dir
            .join("cache")
            .join("modpack-staging")
            .join(uuid::Uuid::new_v4().to_string());
        let cleanup = |st: &Path| {
            let _ = std::fs::remove_dir_all(st);
        };
        if let Some((_, index)) = &modpack {
            job.set_step(
                Step::Downloading,
                &format!("Descargando {} archivos del modpack", index.files.len()),
            );
            let r = merc_content::mrpack::stage_files(
                &self.http,
                index,
                &staging,
                &mut |file, done, total| {
                    job.event(merc_install::InstallEvent::Download {
                        file: file.to_string(),
                        done,
                        total: Some(total),
                    })
                },
                &job.cancel,
            );
            match r {
                Ok(()) => job.log(format!("✔ {} archivos verificados", index.files.len())),
                Err(merc_content::ContentError::Cancelled) => {
                    cleanup(&staging);
                    return job.finish(JobState::Cancelled, None, None);
                }
                Err(e) => {
                    cleanup(&staging);
                    return job.finish(JobState::Failed, Some(e.to_string()), None);
                }
            }
        }
        let outcome = match merc_install::install(&self.http, &plan, &mut |e| job.event(e), &job.cancel) {
            Ok(o) => o,
            Err(e) => {
                cleanup(&staging);
                return fail(&job, e);
            }
        };
        if let Some((pack, _)) = &modpack {
            job.set_step(Step::Configuring, "Copiando el modpack al servidor");
            let prefixes: Vec<&str> = pack.overrides.iter().map(String::as_str).collect();
            match merc_content::mrpack::apply_with(&pack.path, &staging, &p.path, &prefixes) {
                Ok(notes) => {
                    for n in notes {
                        job.log(format!("ⓘ {n}"));
                    }
                }
                Err(e) => {
                    cleanup(&staging);
                    // La carpeta estaba vacía antes de empezar: se deja como estaba.
                    if let Ok(rd) = std::fs::read_dir(&p.path) {
                        for entry in rd.flatten() {
                            let ep = entry.path();
                            let _ = if ep.is_dir() {
                                std::fs::remove_dir_all(&ep)
                            } else {
                                std::fs::remove_file(&ep)
                            };
                        }
                    }
                    return job.finish(
                        JobState::Failed,
                        Some(format!("No se pudo copiar el modpack: {e}")),
                        None,
                    );
                }
            }
            cleanup(&staging);
        }
        let root = std::fs::canonicalize(&p.path)
            .map(|r| strip_verbatim(&r))
            .unwrap_or(p.path.clone());
        let det = outcome.detected;
        let Some(target) = det.launch.clone() else {
            return job.finish(
                JobState::Failed,
                Some("La instalación no dejó un arranque válido".into()),
                None,
            );
        };
        let inst = Instance {
            id: InstanceId::new(),
            name: p.name.clone(),
            root,
            kind: det.kind,
            mc_version: Some(resolved.mc_version.clone()),
            loader_version: det.loader_version.or(resolved.loader_version),
            launch: LaunchProfile {
                java: if p.flavor.is_native() { PathBuf::new() } else { p.java },
                min_memory_mb: p.min_memory_mb,
                max_memory_mb: p.max_memory_mb,
                extra_jvm_args: vec![],
                target,
            },
            imported: false,
            created_at_unix: now_unix(),
        };
        let r = self
            .store
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert_instance(&inst);
        match r {
            Ok(()) => {
                self.audit(
                    "supervisor",
                    Some(inst.id),
                    "server.created",
                    &inst.root.display().to_string(),
                );
                if let (Some((pack, _)), Some(vid)) = (&modpack, &p.modpack_version) {
                    match self.record_modpack(inst.id, p.modpack_source, vid, pack) {
                        Ok(()) => job.log("Se guardó qué archivos puso el modpack, para poder actualizarlo"),
                        Err(e) => job.log(format!(
                            "⚠ No se pudo guardar el registro del modpack ({e}): para actualizarlo, vincúlalo en Mods"
                        )),
                    }
                }
                if !p.game_rules.is_empty() {
                    self.save_pending_rules(inst.id, &p.game_rules);
                    job.log("Las reglas del juego elegidas se aplicarán la primera vez que se encienda");
                }
                job.finish(JobState::Succeeded, None, Some(inst.id));
            }
            Err(e) => job.finish(
                JobState::Failed,
                Some(format!(
                    "El servidor se instaló en {} pero no se pudo registrar ({}). Puedes importarlo.",
                    inst.root.display(),
                    store_err(e)
                )),
                None,
            ),
        }
    }

    fn handle(self: &Arc<Self>, client: &str, body: RequestBody) -> Result<(ResponseBody, bool), ApiError> {
        use RequestBody as R;
        let out = match body {
            R::Ping => ResponseBody::Pong {
                server_version: VERSION.into(),
                supervisor_pid: std::process::id(),
                protocol_id: merc_ipc::PROTOCOL_ID.into(),
            },
            R::Detect { path } => {
                if !path.is_absolute() || !path.is_dir() {
                    return Err(api(
                        ErrorCode::BadRequest,
                        "Indica una carpeta existente con ruta completa",
                    ));
                }
                ResponseBody::Detected(detect_server(&path))
            }
            R::FindJava => {
                let mut found = merc_core::java::find_java_candidates();
                for p in self.managed_java() {
                    if !found.contains(&p) {
                        found.push(p);
                    }
                }
                let handles: Vec<_> = found
                    .into_iter()
                    .map(|p| {
                        std::thread::spawn(move || match merc_core::java::probe_java(&p, Duration::from_secs(10)) {
                            Ok(probe) => JavaCandidate {
                                jdk: merc_install::install::has_javac(&p),
                                path: p,
                                major: Some(probe.major),
                                description: probe.raw,
                                error: None,
                            },
                            Err(e) => JavaCandidate {
                                jdk: false,
                                path: p,
                                major: None,
                                description: String::new(),
                                error: Some(e.to_string()),
                            },
                        })
                    })
                    .collect();
                ResponseBody::JavaCandidates(handles.into_iter().filter_map(|h| h.join().ok()).collect())
            }
            R::ImportInstance {
                name,
                path,
                java,
                min_memory_mb,
                max_memory_mb,
            } => {
                let name = validate_instance_name(&name).map_err(|e| api(ErrorCode::BadRequest, e))?;
                if !path.is_absolute() || !path.is_dir() {
                    return Err(api(ErrorCode::BadRequest, "La carpeta del servidor no existe"));
                }
                let root = std::fs::canonicalize(&path).map_err(|e| api(ErrorCode::BadRequest, e.to_string()))?;
                let root = strip_verbatim(&root);
                let det = detect_server(&root);
                // Bedrock Dedicated Server es nativo: no necesita Java.
                let java = if det.kind == ServerKind::Bedrock {
                    PathBuf::new()
                } else if !java.is_absolute() || !java.is_file() {
                    return Err(api(
                        ErrorCode::BadRequest,
                        "Indica la ruta completa del ejecutable de Java",
                    ));
                } else {
                    java
                };
                let Some(target) = det.launch.clone() else {
                    return Err(api(
                        ErrorCode::BadRequest,
                        format!(
                            "No reconozco un servidor de Minecraft en esa carpeta. {}",
                            det.warnings.join(" ")
                        ),
                    ));
                };
                let launch = LaunchProfile {
                    java,
                    min_memory_mb,
                    max_memory_mb,
                    extra_jvm_args: vec![],
                    target,
                };
                launch
                    .validate()
                    .map_err(|e| api(ErrorCode::BadRequest, e.to_string()))?;
                let inst = Instance {
                    id: InstanceId::new(),
                    name,
                    root,
                    kind: det.kind,
                    mc_version: det.mc_version,
                    loader_version: det.loader_version,
                    launch,
                    imported: true,
                    created_at_unix: now_unix(),
                };
                self.store
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .insert_instance(&inst)
                    .map_err(store_err)?;
                ResponseBody::Instance(inst)
            }
            R::ListInstances => {
                let list = self
                    .store
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .list_instances()
                    .map_err(store_err)?;
                ResponseBody::Instances(
                    list.into_iter()
                        .map(|instance| {
                            let status = self.sup.status(instance.id);
                            let eula_accepted = matches!(
                                merc_core::properties::read_eula(&instance.root),
                                Ok(merc_core::properties::EulaStatus::Accepted)
                            );
                            let configured_port = merc_core::properties::ServerProperties::read(&instance.root)
                                .ok()
                                .flatten()
                                .filter(|p| p.port_explicit)
                                .map(|p| p.port);
                            let archived = self.is_archived(instance.id);
                            InstanceSummary {
                                instance,
                                status,
                                eula_accepted,
                                configured_port,
                                archived,
                            }
                        })
                        .collect(),
                )
            }
            R::Status { id } => {
                self.instance(id)?;
                ResponseBody::Status(self.sup.status(id))
            }
            R::Start { id } => {
                let inst = self.instance(id)?;
                self.require_not_archived(id)?;
                self.clear_gave_up(id);
                if self.world_op_running(id) {
                    return Err(api(
                        ErrorCode::Conflict,
                        "Espera a que termine la operación con el mundo antes de encender el servidor.",
                    ));
                }
                self.audit(client, Some(id), "server.start", "");
                self.sup.start(Self::spec(&inst)).map_err(sup_err)?;
                ResponseBody::Status(self.sup.status(id))
            }
            R::Stop { id, timeout_secs } => {
                self.instance(id)?;
                self.audit(client, Some(id), "server.stop", "");
                let t = timeout_secs.clamp(1, MAX_STOP_TIMEOUT_SECS);
                self.sup.stop(id, Duration::from_secs(t as u64)).map_err(sup_err)?;
                ResponseBody::Status(self.sup.status(id))
            }
            R::Restart { id, stop_timeout_secs } => {
                let inst = self.instance(id)?;
                self.audit(client, Some(id), "server.restart", "");
                let t = stop_timeout_secs.clamp(1, MAX_STOP_TIMEOUT_SECS);
                self.sup
                    .restart(Self::spec(&inst), Duration::from_secs(t as u64))
                    .map_err(sup_err)?;
                ResponseBody::Status(self.sup.status(id))
            }
            R::ForceKill { id, confirm } => {
                self.instance(id)?;
                if confirm != FORCE_KILL_CONFIRMATION {
                    return Err(api(
                        ErrorCode::BadRequest,
                        "El cierre forzado necesita confirmación explícita: se puede perder lo no guardado del mundo",
                    ));
                }
                self.audit(client, Some(id), "server.force_kill", "confirmado por el usuario");
                self.sup
                    .force_kill(id, merc_supervisor::ForceKill::IUnderstandUnsavedDataMayBeLost)
                    .map_err(sup_err)?;
                ResponseBody::Ok
            }
            R::SendCommand { id, command } => {
                self.instance(id)?;
                self.sup.send_command(id, &command).map_err(sup_err)?;
                self.audit(client, Some(id), "server.command", &command);
                ResponseBody::Ok
            }
            R::Console { id, after, limit } => {
                self.instance(id)?;
                let limit = limit.clamp(1, 2000) as usize;
                let lines: Vec<_> = self.sup.console_since(id, after).into_iter().take(limit).collect();
                let next_after = lines.last().map(|l| l.seq).unwrap_or(after);
                ResponseBody::Console { lines, next_after }
            }
            R::AcceptEula { id, accepted } => {
                let inst = self.instance(id)?;
                if !accepted {
                    return Err(api(
                        ErrorCode::BadRequest,
                        "La EULA solo se acepta si el usuario marca expresamente que la leyó y la acepta",
                    ));
                }
                if self.sup.status(id).state.has_process() {
                    return Err(api(
                        ErrorCode::InvalidState,
                        "Detén el servidor antes de cambiar la EULA",
                    ));
                }
                write_accepted_eula(&inst.root).map_err(|e| api(ErrorCode::Internal, e))?;
                self.audit(
                    client,
                    Some(id),
                    "server.eula.accept",
                    "aceptada expresamente por el usuario",
                );
                ResponseBody::Ok
            }
            R::Metrics { id } => {
                let inst = self.instance(id)?;
                let (samples, players) = self.sup.metrics(id);
                let max_players = merc_core::properties::ServerProperties::read(&inst.root)
                    .ok()
                    .flatten()
                    .and_then(|p| p.raw.get("max-players").and_then(|v| v.parse().ok()));
                ResponseBody::Metrics {
                    samples,
                    players,
                    max_players,
                }
            }
            R::SystemInfo => ResponseBody::System {
                total_memory_bytes: merc_supervisor::metrics::total_memory_bytes(),
                logical_cpus: std::thread::available_parallelism()
                    .map(|n| n.get() as u32)
                    .unwrap_or(1),
                lan_ipv4: lan_ipv4(),
            },
            R::Unregister { id, delete_files } => {
                let inst = self.instance(id)?;
                if self.sup.status(id).state.has_process() {
                    return Err(api(
                        ErrorCode::InvalidState,
                        "Detén el servidor antes de quitarlo de la lista",
                    ));
                }
                if self.world_op_running(id) {
                    return Err(api(
                        ErrorCode::Conflict,
                        "Hay una operación de mundos en marcha; espera a que termine o cancélala.",
                    ));
                }
                if delete_files {
                    let root = self.check_deletable(&inst)?;
                    self.recycle(&root).map_err(|e| {
                        api(
                            ErrorCode::Internal,
                            format!(
                                "El sistema no pudo mandar la carpeta a la Papelera ({e}). No se quitó nada de la lista."
                            ),
                        )
                    })?;
                    self.audit(client, Some(id), "server.delete_files", &root.display().to_string());
                    // La papelera y las copias internas de MercHosting de este servidor, también a la Papelera.
                    let store_dir = self.store_dir(id);
                    if store_dir.is_dir() {
                        if let Err(e) = self.recycle(&store_dir) {
                            self.log
                                .line(&format!("no se pudo mandar a la papelera {}: {e}", store_dir.display()));
                        }
                    }
                }
                self.store
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .unregister_instance(id)
                    .map_err(store_err)?;
                ResponseBody::Ok
            }
            R::Catalog { flavor } => ResponseBody::Catalog(self.catalog(flavor)?),
            R::Resolve {
                flavor,
                mc_version,
                loader_version,
            } => ResponseBody::Resolved(
                merc_install::resolve_pinned(
                    &self.http,
                    &self.sources,
                    flavor,
                    &mc_version,
                    loader_version.as_deref(),
                )
                .map_err(install_err)?,
            ),
            R::ModpackSearch {
                source,
                query,
                sort,
                offset,
            } => ResponseBody::ContentSearch(match source {
                ContentSource::Modrinth => merc_content::modrinth::Modrinth::new(&self.http, &self.modrinth_api)
                    .search_modpacks(&query, sort, offset.min(10_000))
                    .map_err(content_err)?,
                ContentSource::Curseforge => self
                    .require_curseforge()?
                    .search_modpacks(&query, sort, offset.min(10_000))
                    .map_err(content_err)?,
                ContentSource::Hangar => return Err(api(ErrorCode::BadRequest, NO_HANGAR_PACKS)),
            }),
            R::ModpackProject { source, project } => {
                use merc_content::modrinth::Source;
                let (mut proj, mut versions) = match source {
                    ContentSource::Hangar => return Err(api(ErrorCode::BadRequest, NO_HANGAR_PACKS)),
                    ContentSource::Modrinth => {
                        let m = merc_content::modrinth::Modrinth::new(&self.http, &self.modrinth_api);
                        let p = m.project(&project).map_err(content_err)?;
                        let v = m.all_versions(&p.id).map_err(content_err)?;
                        (p, v)
                    }
                    ContentSource::Curseforge => {
                        let cf = self.require_curseforge()?;
                        let p = cf.project(&project).map_err(content_err)?;
                        let v = cf.all_files(&p.id).map_err(content_err)?;
                        (p, v)
                    }
                };
                if proj.body.chars().count() > 20_000 {
                    proj.body = proj.body.chars().take(20_000).collect::<String>() + "…";
                }
                versions.truncate(60);
                for v in &mut versions {
                    if let Some(c) = &mut v.changelog {
                        if c.chars().count() > 4000 {
                            *c = c.chars().take(4000).collect::<String>() + "…";
                        }
                    }
                }
                ResponseBody::ContentDetails {
                    project: Box::new(proj),
                    versions,
                }
            }
            R::ModpackInspect { source, version_id } => {
                let pack = self.load_modpack(source, &version_id, &AtomicBool::new(false))?;
                let mut sum = merc_content::mrpack::summarize(&pack.index);
                sum.source = source;
                for (i, w) in pack.warnings.into_iter().enumerate() {
                    sum.warnings.insert(i, w);
                }
                ResponseBody::ModpackSummary(sum)
            }
            R::CreationDefaults => ResponseBody::CreationDefaults {
                servers_dir: default_servers_dir(),
                suggested_port: self.suggested_port(),
                curseforge: curseforge_key(&self.data_dir).is_some(),
            },
            R::CheckTarget { path } => {
                if !path.is_absolute() {
                    return Err(api(ErrorCode::BadRequest, "La carpeta debe ser una ruta completa"));
                }
                merc_install::check_target(&path).map_err(install_err)?;
                ResponseBody::Ok
            }
            R::CreateServer {
                name,
                flavor,
                mc_version,
                path,
                java,
                min_memory_mb,
                max_memory_mb,
                properties,
                accept_eula,
                modpack_version,
                modpack_source,
                loader_version,
                game_rules,
            } => self.start_create(
                client,
                CreateParams {
                    name,
                    flavor,
                    mc_version,
                    path,
                    java,
                    min_memory_mb,
                    max_memory_mb,
                    properties,
                    accept_eula,
                    modpack_version,
                    modpack_source,
                    loader_version,
                    game_rules,
                },
            )?,
            R::Job { job_id, after } => ResponseBody::Job(
                self.jobs
                    .get(&job_id)
                    .ok_or_else(|| api(ErrorCode::NotFound, "Ese trabajo no existe o ya caducó"))?
                    .snapshot(after),
            ),
            R::CancelJob { job_id } => {
                if !self.jobs.cancel(&job_id) {
                    return Err(api(ErrorCode::NotFound, "Ese trabajo no existe o ya caducó"));
                }
                self.audit(client, None, "server.create.cancel", &job_id);
                ResponseBody::Ok
            }
            b @ (R::Properties { .. }
            | R::SetProperties { .. }
            | R::SetResourcePack { .. }
            | R::Players { .. }
            | R::PlayerAction { .. }
            | R::ListFiles { .. }
            | R::ReadFile { .. }
            | R::WriteFile { .. }
            | R::MakeDir { .. }
            | R::RenameFile { .. }
            | R::MoveFile { .. }
            | R::CopyFile { .. }
            | R::ExtractZip { .. }
            | R::TrashFile { .. }
            | R::UploadFile { .. }
            | R::DownloadFile { .. }
            | R::ExportConsole { .. }
            | R::Worlds { .. }
            | R::DuplicateWorld { .. }
            | R::ExportWorld { .. }
            | R::ImportWorld { .. }
            | R::SetActiveWorld { .. }
            | R::TrashWorld { .. }
            | R::SavedItems { .. }
            | R::RestoreSaved { .. }
            | R::PurgeSaved { .. }
            | R::GameRules { .. }
            | R::SetGameRule { .. }
            | R::Activity { .. }
            | R::RestartWithNotice { .. }
            | R::AdminState { .. }
            | R::SetMaintenance { .. }
            | R::CancelRestart { .. }
            | R::CancelWorldOp { .. }) => self.handle_admin(client, b)?,
            b @ (R::Backups { .. }
            | R::CreateBackup { .. }
            | R::VerifyBackup { .. }
            | R::RestoreBackup { .. }
            | R::DeleteBackup { .. }
            | R::ProtectBackup { .. }
            | R::SetBackupSettings { .. }
            | R::DownloadBackup { .. }) => self.handle_backups(client, b)?,
            b @ (R::Tasks { .. } | R::SaveTask { .. } | R::DeleteTask { .. } | R::RunTaskNow { .. }) => {
                self.handle_tasks(client, b)?
            }
            b @ (R::Health { .. }
            | R::MeasureTps { .. }
            | R::Alerts
            | R::MarkAlertsRead { .. }
            | R::SetAlertSettings { .. }) => self.handle_health(client, b)?,
            b @ (R::PlayitStatus
            | R::PlayitEnable
            | R::PlayitDisable
            | R::PlayitLink
            | R::PlayitUnlink
            | R::PlayitLoginUrl
            | R::Connection { .. }
            | R::TestAddress { .. }
            | R::TestBedrockAddress { .. }
            | R::SetVoiceConfig { .. }) => self.handle_playit(client, b)?,
            b @ (R::Diagnose { .. } | R::SetLaunch { .. }) => self.handle_diagnose(client, b)?,
            b @ R::SecurityReport { .. } => self.handle_security(b)?,
            b @ R::InstallJava { .. } => self.handle_java(client, b)?,
            b @ (R::ServerUpdateCheck { .. } | R::ApplyServerUpdate { .. }) => self.handle_updates(client, b)?,
            b @ (R::LoaderVersions { .. } | R::CloneServer { .. } | R::SetArchived { .. }) => {
                self.handle_servers(client, b)?
            }
            b @ (R::Automation { .. } | R::SetAutomation { .. } | R::SetStartup { .. } | R::SetKeepAwake { .. }) => {
                self.handle_automation(client, b)?
            }
            b @ (R::PlayerCommand { .. }
            | R::PlayerHistory { .. }
            | R::SetPlayerNote { .. }
            | R::ClearPlayerHistory { .. }) => self.handle_players(client, b)?,
            b @ (R::FriendPack { .. } | R::ExportFriendPack { .. }) => self.handle_friend_pack(client, b)?,
            b @ (R::ResourcePackSearch { .. } | R::ResourcePackProject { .. }) => self.handle_rp_browse(b)?,
            b @ (R::ModpackStatus { .. }
            | R::ModpackUpdatePreview { .. }
            | R::ModpackUpdateApply { .. }
            | R::ModpackLink { .. }
            | R::ModpackUnlink { .. }) => self.handle_modpacks(client, b)?,
            b @ (R::BedrockPlayers { .. }
            | R::BedrockAllowlist { .. }
            | R::BedrockAllowlistEnabled { .. }
            | R::BedrockPermission { .. }) => self.handle_bedrock_players(client, b)?,
            b @ (R::Addons { .. } | R::AddonImport { .. } | R::AddonSetActive { .. } | R::AddonRemove { .. }) => {
                self.handle_addons(client, b)?
            }
            b @ (R::ModSides { .. } | R::QuarantineMods { .. } | R::RestoreMods { .. }) => {
                self.handle_mod_sides(client, b)?
            }
            b @ (R::LuckPerms { .. } | R::LuckPermsPreview { .. } | R::LuckPermsApply { .. }) => {
                self.handle_luckperms(client, b)?
            }
            b @ (R::Supporter | R::ActivateSupporter { .. } | R::RefreshSupporter | R::ForgetSupporter) => {
                self.handle_supporter(client, b)?
            }
            b @ (R::ServerIcon { .. }
            | R::PreviewServerIcon { .. }
            | R::SetServerIcon { .. }
            | R::RemoveServerIcon { .. }) => self.handle_icon(client, b)?,
            b @ (R::Companions
            | R::SetCompanionsEnabled { .. }
            | R::CreateInvite { .. }
            | R::CreateHostInvite { .. }
            | R::CancelInvite { .. }
            | R::SetCompanionPerms { .. }
            | R::RemoveCompanion { .. }
            | R::JoinWithCode { .. }
            | R::RemoteHosts
            | R::ForgetHost { .. }
            | R::RefreshHost { .. }
            | R::RemoteDownload { .. }
            | R::RemoteUpload { .. }
            | R::RemoteCall { .. }) => self.handle_companions(client, b)?,
            b @ (R::ContentPlatforms { .. }
            | R::ContentSearch { .. }
            | R::ContentProject { .. }
            | R::ContentPlan { .. }
            | R::ContentInstall { .. }
            | R::InstallFloodgate { .. }
            | R::ContentInstalled { .. }
            | R::ContentToggle { .. }
            | R::ContentRemove { .. }) => self.handle_content(client, b)?,
            R::Shutdown => {
                if self.jobs.any_running() {
                    return Err(api(
                        ErrorCode::Conflict,
                        "Se está creando un servidor. Espera a que termine o cancélalo antes de cerrar el supervisor.",
                    ));
                }
                let list = self
                    .store
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .list_instances()
                    .map_err(store_err)?;
                let running: Vec<String> = list
                    .into_iter()
                    .filter(|i| self.sup.status(i.id).state.has_process())
                    .map(|i| i.name)
                    .collect();
                if !running.is_empty() {
                    return Err(api(
                        ErrorCode::Conflict,
                        format!(
                            "Hay servidores en marcha: {}. Detenlos antes de cerrar el supervisor.",
                            running.join(", ")
                        ),
                    ));
                }
                return Ok((ResponseBody::Ok, true));
            }
        };
        Ok((out, false))
    }

    fn serve(self: &Arc<Self>, stream: interprocess::local_socket::Stream) {
        let mut r = BufReader::new(stream);
        let hello: Hello = match read_message(&mut r) {
            Ok(h) => h,
            Err(_) => return,
        };
        if hello.protocol != PROTOCOL_VERSION {
            let _ = write_message(
                r.get_mut(),
                &HelloReply::Rejected {
                    error: api(
                        ErrorCode::VersionMismatch,
                        format!(
                            "Versión de protocolo incompatible (cliente {}, supervisor {}). Actualiza MercHosting.",
                            hello.protocol, PROTOCOL_VERSION
                        ),
                    ),
                },
            );
            return;
        }
        if !token_eq(&hello.token, &self.token) {
            self.log.line(&format!(
                "conexión rechazada: token no válido (cliente «{}»)",
                hello.client
            ));
            let _ = write_message(
                r.get_mut(),
                &HelloReply::Rejected {
                    error: api(ErrorCode::AuthFailed, "Credencial de sesión no válida"),
                },
            );
            return;
        }
        let client = hello.client.chars().take(64).collect::<String>();
        if write_message(
            r.get_mut(),
            &HelloReply::Welcome(Welcome {
                protocol: PROTOCOL_VERSION,
                server_version: VERSION.into(),
                supervisor_pid: std::process::id(),
            }),
        )
        .is_err()
        {
            return;
        }
        loop {
            let req: Request = match read_message(&mut r) {
                Ok(req) => req,
                Err(FrameError::Invalid(e)) => {
                    let _ = write_message(
                        r.get_mut(),
                        &Response {
                            seq: 0,
                            result: ResponseResult::Err(api(ErrorCode::BadRequest, e)),
                        },
                    );
                    continue;
                }
                Err(_) => return,
            };
            let (result, shutdown) = match self.handle(&client, req.body) {
                Ok((b, s)) => (ResponseResult::Ok(b), s),
                Err(e) => (ResponseResult::Err(e), false),
            };
            if write_message(r.get_mut(), &Response { seq: req.seq, result }).is_err() {
                return;
            }
            if shutdown {
                self.log
                    .line(&format!("cierre del supervisor solicitado por «{client}»"));
                self.cleanup();
                std::process::exit(0);
            }
        }
    }

    fn cleanup(&self) {
        let _ = std::fs::remove_file(token_path(&self.data_dir));
        self.endpoint.remove_stale();
    }
}

/// IPv4 privada de esta PC en la red local. Se pregunta al sistema qué interfaz usaría para salir
/// (un socket UDP "conectado" no envía ningún paquete). Solo se devuelven direcciones privadas.
fn lan_ipv4() -> Option<String> {
    let sock = std::net::UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    sock.connect(("192.0.2.1", 9)).ok()?; // TEST-NET-1: nunca recibe tráfico real
    match sock.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(ip) if ip.is_private() => Some(ip.to_string()),
        _ => None,
    }
}

/// Escribe `eula.txt` de forma atómica dentro de la carpeta del servidor (ruta confinada).
fn write_accepted_eula(root: &Path) -> Result<(), String> {
    let sandbox = merc_core::paths::SandboxRoot::new(root).map_err(|e| e.to_string())?;
    let target = sandbox.resolve("eula.txt").map_err(|e| e.to_string())?;
    let tmp = sandbox.resolve("eula.txt.merchosting.tmp").map_err(|e| e.to_string())?;
    let text = format!(
        "# Aceptada por el usuario desde MercHosting (marca de tiempo Unix {}).\n\
         # EULA de Minecraft: https://aka.ms/MinecraftEULA\n\
         eula=true\n",
        now_unix()
    );
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &target).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

/// Arranca el supervisor y atiende conexiones hasta que se pida cerrarlo.
pub fn run(data_dir: PathBuf) -> Result<RunOutcome, String> {
    run_with(data_dir, false)
}

/// Como [`run`]. `at_login`: lo arrancó Windows al iniciar sesión (enciende los servidores marcados).
pub fn run_with(data_dir: PathBuf, at_login: bool) -> Result<RunOutcome, String> {
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("no se pudo crear {}: {e}", data_dir.display()))?;
    let log = DaemonLog::open(&data_dir.join("supervisor.log")).map_err(|e| e.to_string())?;
    let endpoint = Endpoint::for_data_dir(&data_dir);

    // ¿Ya hay uno funcionando? Se comprueba con una conexión autenticada real.
    if let Ok(token) = read_token(&data_dir) {
        if let Ok(mut c) = Client::connect_with_token(&endpoint, &token, "merc-supervisord-check") {
            if c.call(RequestBody::Ping).is_ok() {
                log.line("ya hay un supervisor en ejecución para esta carpeta de datos; no se inicia otro");
                return Ok(RunOutcome::AlreadyRunning);
            }
        }
    }

    endpoint.remove_stale();
    let listener = endpoint.listen().map_err(|e| {
        let m = format!("no se pudo abrir el canal {}: {e}", endpoint.describe());
        log.line(&m);
        m
    })?;
    let token = generate_token();
    write_token(&data_dir, &token).map_err(|e| format!("no se pudo guardar el token: {e}"))?;

    let store = Store::open(&data_dir.join("merchosting.db")).map_err(|e| e.to_string())?;
    let runtime = RuntimeDir::new(data_dir.join("runtime")).map_err(|e| e.to_string())?;
    let sup = Supervisor::with_runtime_dir(data_dir.join("logs"), runtime);

    let daemon = Arc::new(Daemon {
        data_dir: data_dir.clone(),
        store: Mutex::new(store),
        sup,
        token,
        log,
        endpoint,
        http: Http::new(),
        sources: sources(),
        jobs: jobs::Jobs::default(),
        catalog_cache: Mutex::new(HashMap::new()),
        pending_restarts: Mutex::new(Default::default()),
        world_ops: Mutex::new(Default::default()),
        modrinth_api: modrinth_api(),
        hangar_api: hangar_api(),
        geysermc_api: geysermc_api(),
        hangar_cache: Mutex::new(Default::default()),
        running_tasks: Mutex::new(Default::default()),
        tasks_lock: Mutex::new(()),
        pc_load: Mutex::new(None),
        alerts_lock: Mutex::new(()),
        playit: Mutex::new(Default::default()),
        remote: Mutex::new(Default::default()),
        companions_lock: Mutex::new(()),
        companions_enabled: std::sync::atomic::AtomicBool::new(false),
        restarts: Mutex::new(Default::default()),
    });
    daemon.log.line(&format!(
        "supervisor {VERSION} iniciado (pid {}) en {}",
        std::process::id(),
        daemon.endpoint.describe()
    ));
    for (id, outcome) in daemon.sup.recover() {
        let msg = match &outcome {
            RecoveryOutcome::Adopted { pid } => {
                format!("servidor {id}: proceso {pid} de una sesión anterior adoptado (sin consola)")
            }
            RecoveryOutcome::Gone { pid } => format!("servidor {id}: el proceso {pid} ya no existe; registro limpiado"),
        };
        daemon.log.line(&msg);
        daemon.audit("supervisor", Some(id), "supervisor.recover", &msg);
    }

    {
        let d = daemon.clone();
        std::thread::spawn(move || d.scheduler_loop());
        let d = daemon.clone();
        std::thread::spawn(move || d.health_loop());
        let d = daemon.clone();
        std::thread::spawn(move || d.pc_sampler_loop());
        let d = daemon.clone();
        std::thread::spawn(move || d.playit_loop());
        let d = daemon.clone();
        std::thread::spawn(move || d.update_watch_loop());
        let d = daemon.clone();
        std::thread::spawn(move || d.companions_boot());
        if at_login {
            daemon
                .log
                .line("arrancado al iniciar sesión: se encienden los servidores marcados");
            let d = daemon.clone();
            std::thread::spawn(move || d.start_at_login());
        }
    }

    let active = Arc::new(AtomicUsize::new(0));
    use interprocess::local_socket::traits::Listener as _;
    loop {
        let stream = match listener.accept() {
            Ok(s) => s,
            Err(e) => {
                daemon.log.line(&format!("error al aceptar conexión: {e}"));
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
        };
        if active.load(Ordering::SeqCst) >= MAX_CONNECTIONS {
            daemon.log.line("demasiadas conexiones simultáneas; se rechaza una");
            drop(stream);
            continue;
        }
        active.fetch_add(1, Ordering::SeqCst);
        let d = daemon.clone();
        let a = active.clone();
        std::thread::spawn(move || {
            d.serve(stream);
            a.fetch_sub(1, Ordering::SeqCst);
        });
    }
}
