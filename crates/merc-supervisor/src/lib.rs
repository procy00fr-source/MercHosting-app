//! Supervisor de procesos Java de MercHosting.
//!
//! Este crate es la lógica del supervisor. En el siguiente incremento se ejecutará dentro de
//! un proceso propio (independiente de la ventana) con IPC por named pipe; aquí se prueba
//! como biblioteca.
//!
//! Garantías de este módulo:
//! - Nunca hay dos procesos para la misma instancia (bloqueo por instancia + estado VALIDATING).
//! - Dos instancias no pueden usar la misma carpeta ni el mismo puerto a la vez.
//! - ONLINE solo tras la línea "Done (…)! For help" del propio servidor.
//! - Detener = enviar `stop` por stdin y esperar. Nunca se mata a la fuerza sin
//!   [`ForceKill::IUnderstandUnsavedDataMayBeLost`].
//! - No se mata ningún proceso ajeno para liberar un puerto.
//! - Java se lanza directamente (sin CMD ni shell) y sin ventana en Windows.

mod log;

pub use log::RotatingLog;

use merc_core::java::{probe_java, required_java};
use merc_core::launch::{LaunchProfile, LaunchTarget};
use merc_core::paths::SandboxRoot;
use merc_core::properties::{read_eula, EulaStatus, ServerProperties};
use merc_core::readiness::{classify_line, LogSignal};
use merc_core::{InstanceId, RuntimeState};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const CONSOLE_BUFFER_LINES: usize = 5_000;
pub const LOG_MAX_BYTES: u64 = 10 * 1024 * 1024;
pub const LOG_KEEP: u32 = 5;
pub const MAX_COMMAND_LEN: usize = 32_000;

#[derive(Debug, Clone)]
pub struct StartSpec {
    pub id: InstanceId,
    pub root: PathBuf,
    pub profile: LaunchProfile,
    pub mc_version: Option<String>,
    /// Tiempo máximo para ver "Done". Pasado ese tiempo el estado pasa a UNKNOWN (no se mata).
    pub readiness_timeout: Duration,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SupervisorError {
    #[error("el servidor ya está en marcha o en transición ({0:?})")]
    AlreadyRunning(RuntimeState),
    #[error("el servidor no está en marcha")]
    NotRunning,
    #[error("la carpeta del servidor no es válida: {0}")]
    InvalidRoot(String),
    #[error("la EULA de Minecraft no está aceptada. Debes leerla y aceptarla tú: https://aka.ms/MinecraftEULA")]
    EulaNotAccepted,
    #[error("falta un archivo necesario para arrancar: {0}")]
    MissingFile(String),
    #[error("Java no válido: {0}")]
    Java(String),
    #[error("esta versión de Minecraft necesita Java {required} o superior; el seleccionado es Java {found}")]
    JavaTooOld { required: u32, found: u32 },
    #[error("el puerto {0} ya está en uso en esta PC. MercHosting no cierra programas ajenos; libera el puerto o cambia server-port")]
    PortBusy(u16),
    #[error("otra instancia en marcha ya usa esta carpeta")]
    RootInUse,
    #[error("otra instancia en marcha ya usa el puerto {0}")]
    PortInUseByInstance(u16),
    #[error("perfil de arranque no válido: {0}")]
    Launch(String),
    #[error("no se pudo iniciar Java: {0}")]
    Spawn(String),
    #[error("comando no válido: {0}")]
    InvalidCommand(&'static str),
    #[error("no se pudo escribir en la consola del servidor: {0}")]
    Stdin(String),
    #[error("el servidor no terminó en el tiempo de espera; sigue en STOPPING. Puedes esperar más o forzar el cierre aceptando el riesgo")]
    StopTimeout,
    #[error("instancia desconocida")]
    UnknownInstance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleStream {
    Stdout,
    Stderr,
    /// Mensajes de MercHosting (no del servidor), p. ej. "Enviado: stop".
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleLine {
    pub seq: u64,
    pub at_unix_ms: u64,
    pub stream: ConsoleStream,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitInfo {
    pub code: Option<i32>,
    pub forced: bool,
    pub requested: bool,
    pub at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusSnapshot {
    pub state: RuntimeState,
    pub pid: Option<u32>,
    pub port: Option<u16>,
    pub started_at_unix_ms: Option<u64>,
    pub online_at_unix_ms: Option<u64>,
    pub last_exit: Option<ExitInfo>,
    /// Explicación legible del último problema (EULA, puerto, readiness…).
    pub hint: Option<String>,
    pub log_file: Option<PathBuf>,
}

/// Confirmación explícita para cerrar el proceso sin guardado limpio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForceKill {
    IUnderstandUnsavedDataMayBeLost,
}

struct SlotState {
    state: RuntimeState,
    generation: u64,
    child: Option<Arc<Mutex<Child>>>,
    stdin: Option<ChildStdin>,
    pid: Option<u32>,
    root_key: Option<String>,
    port: Option<u16>,
    started_at: Option<u64>,
    online_at: Option<u64>,
    last_exit: Option<ExitInfo>,
    hint: Option<String>,
    stop_requested: bool,
    force_requested: bool,
    lines: VecDeque<ConsoleLine>,
    next_seq: u64,
    log: Option<Arc<Mutex<RotatingLog>>>,
    log_file: Option<PathBuf>,
}

struct Slot {
    st: Mutex<SlotState>,
    changed: Condvar,
}

impl Slot {
    fn new() -> Self {
        Self {
            st: Mutex::new(SlotState {
                state: RuntimeState::Stopped,
                generation: 0,
                child: None,
                stdin: None,
                pid: None,
                root_key: None,
                port: None,
                started_at: None,
                online_at: None,
                last_exit: None,
                hint: None,
                stop_requested: false,
                force_requested: false,
                lines: VecDeque::new(),
                next_seq: 1,
                log: None,
                log_file: None,
            }),
            changed: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, SlotState> {
        // Un pánico en un hilo lector no debe dejar la instancia inutilizable.
        self.st.lock().unwrap_or_else(|p| p.into_inner())
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn set_state(st: &mut SlotState, to: RuntimeState) -> bool {
    if st.state.can_transition_to(to) {
        st.state = to;
        if to == RuntimeState::Online {
            st.online_at = Some(now_ms());
        }
        true
    } else {
        false
    }
}

fn push_line(st: &mut SlotState, stream: ConsoleStream, text: String) {
    let line = ConsoleLine {
        seq: st.next_seq,
        at_unix_ms: now_ms(),
        stream,
        text,
    };
    st.next_seq += 1;
    if st.lines.len() >= CONSOLE_BUFFER_LINES {
        st.lines.pop_front();
    }
    if let Some(log) = &st.log {
        let prefix = match stream {
            ConsoleStream::Stdout => "",
            ConsoleStream::Stderr => "[stderr] ",
            ConsoleStream::System => "[MercHosting] ",
        };
        if let Ok(mut l) = log.lock() {
            let _ = l.write_line(&format!("{prefix}{}", line.text));
        }
    }
    st.lines.push_back(line);
}

fn root_key(p: &std::path::Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s
    }
}

#[derive(Clone)]
pub struct Supervisor {
    log_root: PathBuf,
    slots: Arc<Mutex<HashMap<InstanceId, Arc<Slot>>>>,
}

impl Supervisor {
    /// `log_root`: carpeta de datos de MercHosting donde se guardan los logs de consola
    /// (`<log_root>/<id>/console.log`). No se escribe dentro de la carpeta del servidor.
    pub fn new(log_root: impl Into<PathBuf>) -> Self {
        Self {
            log_root: log_root.into(),
            slots: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn slot(&self, id: InstanceId) -> Option<Arc<Slot>> {
        self.slots.lock().unwrap_or_else(|p| p.into_inner()).get(&id).cloned()
    }

    pub fn status(&self, id: InstanceId) -> StatusSnapshot {
        match self.slot(id) {
            None => StatusSnapshot {
                state: RuntimeState::Stopped,
                pid: None,
                port: None,
                started_at_unix_ms: None,
                online_at_unix_ms: None,
                last_exit: None,
                hint: None,
                log_file: None,
            },
            Some(slot) => {
                let st = slot.lock();
                StatusSnapshot {
                    state: st.state,
                    pid: st.pid,
                    port: st.port,
                    started_at_unix_ms: st.started_at,
                    online_at_unix_ms: st.online_at,
                    last_exit: st.last_exit.clone(),
                    hint: st.hint.clone(),
                    log_file: st.log_file.clone(),
                }
            }
        }
    }

    /// Líneas con `seq > after` (para que la UI se reconecte sin perder ni duplicar).
    pub fn console_since(&self, id: InstanceId, after: u64) -> Vec<ConsoleLine> {
        self.slot(id)
            .map(|s| s.lock().lines.iter().filter(|l| l.seq > after).cloned().collect())
            .unwrap_or_default()
    }

    /// Espera hasta que el estado cumpla `pred` o venza el tiempo. Devuelve el estado final.
    pub fn wait_for(&self, id: InstanceId, timeout: Duration, pred: impl Fn(RuntimeState) -> bool) -> RuntimeState {
        let Some(slot) = self.slot(id) else {
            return RuntimeState::Stopped;
        };
        let deadline = Instant::now() + timeout;
        let mut st = slot.lock();
        while !pred(st.state) {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            st = slot
                .changed
                .wait_timeout(st, deadline - now)
                .unwrap_or_else(|p| p.into_inner())
                .0;
        }
        st.state
    }

    pub fn start(&self, spec: StartSpec) -> Result<(), SupervisorError> {
        // 1) Reserva atómica: estado VALIDATING bajo el bloqueo global.
        let canonical_root =
            std::fs::canonicalize(&spec.root).map_err(|e| SupervisorError::InvalidRoot(e.to_string()))?;
        let key = root_key(&canonical_root);
        let slot = {
            let mut map = self.slots.lock().unwrap_or_else(|p| p.into_inner());
            for (other_id, other) in map.iter() {
                if *other_id == spec.id {
                    continue;
                }
                let o = other.lock();
                if (o.state.has_process() || o.state == RuntimeState::Validating) && o.root_key.as_deref() == Some(&key)
                {
                    return Err(SupervisorError::RootInUse);
                }
            }
            let slot = map.entry(spec.id).or_insert_with(|| Arc::new(Slot::new())).clone();
            let mut st = slot.lock();
            if !st.state.can_start() {
                return Err(SupervisorError::AlreadyRunning(st.state));
            }
            st.state = RuntimeState::Validating;
            st.root_key = Some(key.clone());
            st.hint = None;
            st.port = None;
            st.stop_requested = false;
            st.force_requested = false;
            st.generation += 1;
            drop(st);
            slot
        };
        slot.changed.notify_all();

        match self.validate_and_spawn(&slot, &spec, canonical_root) {
            Ok(()) => Ok(()),
            Err(e) => {
                let mut st = slot.lock();
                st.state = RuntimeState::Error;
                st.hint = Some(e.to_string());
                st.root_key = None;
                st.port = None;
                push_line(&mut st, ConsoleStream::System, format!("No se pudo iniciar: {e}"));
                drop(st);
                slot.changed.notify_all();
                Err(e)
            }
        }
    }

    fn validate_and_spawn(&self, slot: &Arc<Slot>, spec: &StartSpec, root: PathBuf) -> Result<(), SupervisorError> {
        let sandbox = SandboxRoot::new(&root).map_err(|e| SupervisorError::InvalidRoot(e.to_string()))?;

        match read_eula(&root).map_err(|e| SupervisorError::InvalidRoot(e.to_string()))? {
            EulaStatus::Accepted => {}
            _ => return Err(SupervisorError::EulaNotAccepted),
        }

        let args = spec
            .profile
            .build_args()
            .map_err(|e| SupervisorError::Launch(e.to_string()))?;
        let mut required_files: Vec<PathBuf> = vec![];
        match &spec.profile.target {
            LaunchTarget::Jar { jar } => required_files.push(jar.clone()),
            LaunchTarget::ArgsFile {
                args_file,
                user_jvm_args,
            } => {
                required_files.push(args_file.clone());
                if let Some(u) = user_jvm_args {
                    required_files.push(u.clone());
                }
            }
        }
        for rel in &required_files {
            let rel_s = rel.to_string_lossy();
            let p = sandbox
                .resolve(&rel_s)
                .map_err(|e| SupervisorError::MissingFile(format!("{rel_s}: {e}")))?;
            if !p.is_file() {
                return Err(SupervisorError::MissingFile(rel_s.into_owned()));
            }
        }

        let probe = probe_java(&spec.profile.java, Duration::from_secs(15))
            .map_err(|e| SupervisorError::Java(e.to_string()))?;
        if let Some(req) = spec.mc_version.as_deref().and_then(required_java) {
            if probe.major < req.min_major {
                return Err(SupervisorError::JavaTooOld {
                    required: req.min_major,
                    found: probe.major,
                });
            }
        }

        let props = ServerProperties::read(&root).map_err(|e| SupervisorError::InvalidRoot(e.to_string()))?;
        let port = props
            .as_ref()
            .map(|p| p.port)
            .unwrap_or(merc_core::properties::DEFAULT_PORT);

        // Puerto: primero contra nuestras instancias, luego contra el sistema.
        {
            let map = self.slots.lock().unwrap_or_else(|p| p.into_inner());
            for (other_id, other) in map.iter() {
                if *other_id == spec.id {
                    continue;
                }
                let o = other.lock();
                if (o.state.has_process() || o.state == RuntimeState::Validating) && o.port == Some(port) {
                    return Err(SupervisorError::PortInUseByInstance(port));
                }
            }
            slot.lock().port = Some(port);
        }
        // Comprobación orientativa: entre esta prueba y el arranque otro programa podría
        // tomar el puerto; en ese caso el propio servidor lo informará y se detectará.
        if TcpListener::bind(("0.0.0.0", port)).is_err() {
            return Err(SupervisorError::PortBusy(port));
        }

        let log_dir = self.log_root.join(spec.id.to_string());
        let log = RotatingLog::open(&log_dir, LOG_MAX_BYTES, LOG_KEEP)
            .map_err(|e| SupervisorError::Spawn(format!("log: {e}")))?;
        let log_file = log.current_path();

        let mut cmd = Command::new(&spec.profile.java);
        cmd.args(&args)
            .current_dir(&root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        merc_core::process_flags::no_console_window(&mut cmd);
        // Fuera de Windows, con locale POSIX Java no puede abrir rutas con tildes (sun.jnu.encoding
        // queda en ASCII). En Windows Java usa la API Unicode del sistema y esto no aplica.
        #[cfg(not(windows))]
        if std::env::var_os("LC_ALL").is_none() && std::env::var_os("LANG").is_none() {
            cmd.env("LC_ALL", "C.UTF-8");
        }

        let mut st = slot.lock();
        st.log = Some(Arc::new(Mutex::new(log)));
        st.log_file = Some(log_file);
        push_line(
            &mut st,
            ConsoleStream::System,
            format!("Iniciando: {} {}", spec.profile.java.display(), args.join(" ")),
        );

        let mut child = cmd.spawn().map_err(|e| SupervisorError::Spawn(e.to_string()))?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        st.stdin = child.stdin.take();
        st.pid = Some(child.id());
        st.started_at = Some(now_ms());
        st.online_at = None;
        st.last_exit = None;
        let child = Arc::new(Mutex::new(child));
        st.child = Some(child.clone());
        set_state(&mut st, RuntimeState::Starting);
        let generation = st.generation;
        drop(st);
        slot.changed.notify_all();

        let mut readers = vec![];
        if let Some(out) = stdout {
            readers.push(spawn_reader(slot.clone(), out, ConsoleStream::Stdout, generation));
        }
        if let Some(err) = stderr {
            readers.push(spawn_reader(slot.clone(), err, ConsoleStream::Stderr, generation));
        }
        spawn_waiter(slot.clone(), child, readers, generation);
        spawn_readiness_timer(slot.clone(), spec.readiness_timeout, generation);
        Ok(())
    }

    /// Envía un comando de Minecraft a la consola (stdin del proceso). No es una shell del SO.
    pub fn send_command(&self, id: InstanceId, command: &str) -> Result<(), SupervisorError> {
        let cmd = command.trim();
        let cmd = cmd.strip_prefix('/').unwrap_or(cmd);
        if cmd.is_empty() {
            return Err(SupervisorError::InvalidCommand("vacío"));
        }
        if cmd.len() > MAX_COMMAND_LEN {
            return Err(SupervisorError::InvalidCommand("demasiado largo"));
        }
        if cmd.chars().any(|c| c.is_control()) {
            // Un salto de línea permitiría encadenar varios comandos en uno.
            return Err(SupervisorError::InvalidCommand(
                "contiene saltos de línea o caracteres de control",
            ));
        }
        let slot = self.slot(id).ok_or(SupervisorError::NotRunning)?;
        let mut st = slot.lock();
        if !matches!(
            st.state,
            RuntimeState::Online | RuntimeState::Loading | RuntimeState::Unknown
        ) {
            return Err(SupervisorError::NotRunning);
        }
        write_stdin(&mut st, cmd)?;
        push_line(&mut st, ConsoleStream::System, format!("> {cmd}"));
        Ok(())
    }

    /// Apagado limpio: envía `stop` y espera hasta `timeout`. Si vence, el servidor sigue en
    /// STOPPING (no se mata) y se devuelve [`SupervisorError::StopTimeout`].
    pub fn stop(&self, id: InstanceId, timeout: Duration) -> Result<(), SupervisorError> {
        let slot = self.slot(id).ok_or(SupervisorError::NotRunning)?;
        {
            let mut st = slot.lock();
            match st.state {
                RuntimeState::Stopping => {} // ya pedido: solo esperar
                s if s.has_process() => {
                    st.stop_requested = true;
                    set_state(&mut st, RuntimeState::Stopping);
                    push_line(
                        &mut st,
                        ConsoleStream::System,
                        "Enviando 'stop' (guardado y cierre limpio)".into(),
                    );
                    if let Err(e) = write_stdin(&mut st, "stop") {
                        push_line(&mut st, ConsoleStream::System, format!("No se pudo enviar 'stop': {e}"));
                        drop(st);
                        slot.changed.notify_all();
                        return Err(e);
                    }
                }
                _ => return Err(SupervisorError::NotRunning),
            }
        }
        slot.changed.notify_all();
        let final_state = self.wait_for(id, timeout, |s| !s.has_process());
        if final_state.has_process() {
            Err(SupervisorError::StopTimeout)
        } else {
            Ok(())
        }
    }

    /// Reinicio: apagado limpio completo y luego arranque. Nunca en paralelo.
    pub fn restart(&self, spec: StartSpec, stop_timeout: Duration) -> Result<(), SupervisorError> {
        match self.stop(spec.id, stop_timeout) {
            Ok(()) | Err(SupervisorError::NotRunning) => {}
            Err(e) => return Err(e),
        }
        self.start(spec)
    }

    /// Cierre forzado. Solo con confirmación explícita; puede perder lo no guardado.
    pub fn force_kill(&self, id: InstanceId, _confirm: ForceKill) -> Result<(), SupervisorError> {
        let slot = self.slot(id).ok_or(SupervisorError::NotRunning)?;
        let child = {
            let mut st = slot.lock();
            if !st.state.has_process() {
                return Err(SupervisorError::NotRunning);
            }
            st.force_requested = true;
            push_line(
                &mut st,
                ConsoleStream::System,
                "Cierre forzado solicitado por el usuario".into(),
            );
            st.child.clone().ok_or(SupervisorError::NotRunning)?
        };
        let mut c = child.lock().unwrap_or_else(|p| p.into_inner());
        c.kill().map_err(|e| SupervisorError::Spawn(e.to_string()))
    }
}

fn write_stdin(st: &mut SlotState, line: &str) -> Result<(), SupervisorError> {
    let stdin = st.stdin.as_mut().ok_or(SupervisorError::NotRunning)?;
    stdin
        .write_all(format!("{line}\n").as_bytes())
        .and_then(|_| stdin.flush())
        .map_err(|e| SupervisorError::Stdin(e.to_string()))
}

fn spawn_reader<R: Read + Send + 'static>(
    slot: Arc<Slot>,
    pipe: R,
    stream: ConsoleStream,
    generation: u64,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(pipe);
        let mut buf = Vec::with_capacity(512);
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    while matches!(buf.last(), Some(b'\n' | b'\r')) {
                        buf.pop();
                    }
                    let text = String::from_utf8_lossy(&buf).into_owned();
                    let signal = classify_line(&text);
                    let mut st = slot.lock();
                    if st.generation != generation {
                        break;
                    }
                    push_line(&mut st, stream, text);
                    let changed = match signal {
                        Some(LogSignal::Loading) if st.state == RuntimeState::Starting => {
                            set_state(&mut st, RuntimeState::Loading)
                        }
                        Some(LogSignal::Ready)
                            if matches!(
                                st.state,
                                RuntimeState::Starting | RuntimeState::Loading | RuntimeState::Unknown
                            ) =>
                        {
                            set_state(&mut st, RuntimeState::Online)
                        }
                        Some(LogSignal::PortInUse) => {
                            st.hint =
                                Some("El servidor no pudo usar su puerto: otro programa lo está ocupando.".into());
                            true
                        }
                        Some(LogSignal::EulaNotAccepted) => {
                            st.hint = Some("El servidor indica que la EULA no está aceptada.".into());
                            true
                        }
                        _ => false,
                    };
                    drop(st);
                    if changed {
                        slot.changed.notify_all();
                    }
                }
            }
        }
    })
}

fn spawn_waiter(slot: Arc<Slot>, child: Arc<Mutex<Child>>, readers: Vec<std::thread::JoinHandle<()>>, generation: u64) {
    std::thread::spawn(move || {
        let status = loop {
            {
                let mut c = child.lock().unwrap_or_else(|p| p.into_inner());
                match c.try_wait() {
                    Ok(Some(s)) => break Some(s),
                    Ok(None) => {}
                    Err(_) => break None,
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        // Esperar a que se lean las últimas líneas (crash reports, "Stopping server"…).
        for r in readers {
            let _ = r.join();
        }
        let mut st = slot.lock();
        if st.generation != generation {
            return;
        }
        let code = status.and_then(|s| s.code());
        let requested = st.stop_requested;
        let forced = st.force_requested;
        st.last_exit = Some(ExitInfo {
            code,
            forced,
            requested,
            at_unix_ms: now_ms(),
        });
        st.stdin = None;
        st.child = None;
        st.pid = None;
        st.root_key = None;
        st.port = None;
        // Si el usuario pidió detener (o forzó), el cierre es esperado aunque el código no sea 0:
        // algunos loaders terminan con códigos distintos tras `stop`. El código queda registrado.
        if requested || forced {
            st.state = RuntimeState::Stopped;
        } else {
            st.state = RuntimeState::Crashed;
            if st.hint.is_none() {
                st.hint = Some(match code {
                    Some(c) => {
                        format!("El servidor terminó inesperadamente (código {c}). Revisa la consola y crash-reports.")
                    }
                    None => "El servidor terminó inesperadamente.".into(),
                });
            }
        }
        let msg = format!(
            "Proceso terminado (código {}){}",
            code.map(|c| c.to_string()).unwrap_or_else(|| "desconocido".into()),
            if forced { " — cierre forzado" } else { "" }
        );
        push_line(&mut st, ConsoleStream::System, msg);
        if let Some(log) = &st.log {
            if let Ok(mut l) = log.lock() {
                let _ = l.flush();
            }
        }
        drop(st);
        slot.changed.notify_all();
    });
}

fn spawn_readiness_timer(slot: Arc<Slot>, timeout: Duration, generation: u64) {
    std::thread::spawn(move || {
        let deadline = Instant::now() + timeout;
        let mut st = slot.lock();
        loop {
            if st.generation != generation || !matches!(st.state, RuntimeState::Starting | RuntimeState::Loading) {
                return;
            }
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            st = slot
                .changed
                .wait_timeout(st, deadline - now)
                .unwrap_or_else(|p| p.into_inner())
                .0;
        }
        if set_state(&mut st, RuntimeState::Unknown) {
            st.hint = Some(format!(
                "El proceso sigue vivo pero no confirmó que esté listo tras {} s. Puede seguir cargando (modpacks grandes) o estar bloqueado.",
                timeout.as_secs()
            ));
            push_line(
                &mut st,
                ConsoleStream::System,
                "Readiness no confirmada a tiempo: estado no confirmado".into(),
            );
            drop(st);
            slot.changed.notify_all();
        }
    });
}
