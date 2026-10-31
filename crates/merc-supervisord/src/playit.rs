//! Playit.gg en el supervisor: el agente oficial corre en segundo plano (sin ventana) mientras
//! esté activado, aunque la ventana de MercHosting esté cerrada; la vinculación usa el flujo
//! oficial de Playit; los túneles se leen del propio agente.

use super::{api, Daemon};
use merc_core::InstanceId;
use merc_ipc::*;
use merc_net::playit::{self as pl, AgentIpc, AgentStatus, ClaimClient, ClaimState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const IPC_TIMEOUT: Duration = Duration::from_secs(5);
/// Cuánto se espera a que la persona apruebe la vinculación.
const CLAIM_TIMEOUT: Duration = Duration::from_secs(20 * 60);

#[derive(Default)]
pub(crate) struct PlayitRuntime {
    child: Option<Child>,
    /// Próximo intento de arrancar el agente (tras un fallo se espera más).
    retry_at: Option<Instant>,
    failures: u32,
    last_error: Option<String>,
    claim: Option<ClaimInfo>,
    claim_running: bool,
    /// Última prueba de cada dirección, por servidor.
    tests: HashMap<InstanceId, Vec<AddressTest>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Saved {
    enabled: bool,
}

pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl Daemon {
    fn playit_dir(&self) -> PathBuf {
        self.data_dir.join("playit")
    }

    fn playit_exe(&self) -> PathBuf {
        self.playit_dir().join(format!("playitd-{}.exe", pl::AGENT_VERSION))
    }

    /// Agente ya en marcha fuera de MercHosting (solo pruebas de desarrollo).
    fn playit_external() -> Option<String> {
        if cfg!(debug_assertions) {
            std::env::var("MERC_PLAYIT_SOCKET").ok().filter(|s| !s.is_empty())
        } else {
            None
        }
    }

    fn playit_api() -> String {
        if cfg!(debug_assertions) {
            if let Ok(v) = std::env::var("MERC_PLAYIT_API") {
                if merc_install::http::url_allowed(&v) {
                    return v;
                }
            }
        }
        pl::API_BASE.to_string()
    }

    /// Canal privado del agente de MercHosting (distinto del de una instalación de Playit).
    fn playit_socket(&self) -> String {
        if let Some(s) = Self::playit_external() {
            return s;
        }
        if cfg!(windows) {
            // Un nombre por carpeta de datos (cada usuario de Windows tiene la suya).
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            for b in self.data_dir.to_string_lossy().bytes() {
                h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
            }
            format!(r"\\.\pipe\merchosting-playit-{h:016x}")
        } else {
            self.playit_dir().join("playitd.sock").display().to_string()
        }
    }

    fn playit_supported() -> bool {
        cfg!(windows) || Self::playit_external().is_some()
    }

    fn playit_saved(&self) -> Saved {
        std::fs::read_to_string(self.playit_dir().join("activo.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn playit_save(&self, s: &Saved) -> Result<(), ApiError> {
        let dir = self.playit_dir();
        std::fs::create_dir_all(&dir).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        std::fs::write(dir.join("activo.json"), serde_json::to_string(s).unwrap_or_default())
            .map_err(|e| api(ErrorCode::Internal, e.to_string()))
    }

    fn agent_ipc(&self) -> Result<AgentIpc, merc_net::NetError> {
        AgentIpc::connect(&self.playit_socket(), IPC_TIMEOUT)
    }

    /// Arranca el agente si está activado y no responde. Lo llama el bucle cada pocos segundos.
    fn playit_ensure_running(&self) {
        if !self.playit_saved().enabled || Self::playit_external().is_some() || !cfg!(windows) {
            return;
        }
        let mut rt = self.playit.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(child) = rt.child.as_mut() {
            match child.try_wait() {
                Ok(None) => return,
                Ok(Some(status)) => {
                    rt.child = None;
                    rt.failures += 1;
                    let wait = Duration::from_secs((5u64 << rt.failures.min(4)).min(60));
                    rt.retry_at = Some(Instant::now() + wait);
                    rt.last_error = Some(format!(
                        "El agente de Playit se cerró ({status}). Se vuelve a intentar en {} s.",
                        wait.as_secs()
                    ));
                    self.log.line(&format!("playit: el agente se cerró ({status})"));
                    return;
                }
                Err(_) => return,
            }
        }
        if rt.retry_at.is_some_and(|t| Instant::now() < t) {
            return;
        }
        drop(rt);
        // ¿Ya hay uno nuestro respondiendo (por ejemplo, de antes de reiniciar el supervisor)?
        if self.agent_ipc().is_ok() {
            return;
        }
        let exe = self.playit_exe();
        if !exe.is_file() {
            return;
        }
        let dir = self.playit_dir();
        let mut cmd = Command::new(&exe);
        cmd.arg("--secret-path")
            .arg(dir.join("playit.toml"))
            .arg("--socket-path")
            .arg(self.playit_socket())
            .arg("--log-path")
            .arg(dir.join("playitd.log"))
            .current_dir(&dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut rt = self.playit.lock().unwrap_or_else(|p| p.into_inner());
        match cmd.spawn() {
            Ok(c) => {
                self.log.line(&format!(
                    "playit: agente {} en marcha (pid {})",
                    pl::AGENT_VERSION,
                    c.id()
                ));
                rt.child = Some(c);
                rt.last_error = None;
            }
            Err(e) => {
                rt.failures += 1;
                rt.retry_at = Some(Instant::now() + Duration::from_secs(30));
                rt.last_error = Some(format!("No se pudo arrancar el agente de Playit: {e}"));
            }
        }
    }

    pub(crate) fn playit_loop(self: Arc<Self>) {
        loop {
            self.playit_ensure_running();
            std::thread::sleep(Duration::from_secs(2));
        }
    }

    fn playit_stop(&self) {
        if let Ok(mut c) = self.agent_ipc() {
            let _ = c.stop();
        }
        let mut rt = self.playit.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(mut child) = rt.child.take() {
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                if let Ok(Some(_)) = child.try_wait() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        rt.retry_at = None;
        rt.failures = 0;
    }

    /// Estado del agente por su canal (o por qué no responde).
    fn playit_agent(&self) -> (Option<AgentStatus>, Option<String>) {
        match self.agent_ipc().and_then(|mut c| c.state()) {
            Ok(s) => (Some(s), None),
            Err(e) => {
                let rt = self.playit.lock().unwrap_or_else(|p| p.into_inner());
                (None, rt.last_error.clone().or_else(|| Some(e.to_string())))
            }
        }
    }

    fn playit_info(&self) -> PlayitInfo {
        let enabled = self.playit_saved().enabled;
        let (agent, agent_error) = if enabled || Self::playit_external().is_some() {
            self.playit_agent()
        } else {
            (None, None)
        };
        let rt = self.playit.lock().unwrap_or_else(|p| p.into_inner());
        PlayitInfo {
            supported: Self::playit_supported(),
            enabled,
            installed: Self::playit_external().is_some() || self.playit_exe().is_file(),
            agent_version: pl::AGENT_VERSION.into(),
            running: agent.is_some(),
            agent,
            agent_error: if enabled { agent_error } else { None },
            claim: rt.claim.clone(),
        }
    }

    /// Descarga el agente oficial (versión fijada) y comprueba su SHA-256.
    fn playit_install(&self) -> Result<(), ApiError> {
        let exe = self.playit_exe();
        if exe.is_file() || Self::playit_external().is_some() {
            return Ok(());
        }
        let dir = self.playit_dir();
        std::fs::create_dir_all(&dir).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        let cancel = std::sync::atomic::AtomicBool::new(false);
        self.http
            .download(
                pl::AGENT_URL,
                &exe,
                &merc_install::Checksum::Sha256(pl::AGENT_SHA256.into()),
                Some(pl::AGENT_SIZE),
                &mut |_, _| {},
                &cancel,
            )
            .map_err(|e| {
                api(
                    ErrorCode::Network,
                    format!("No se pudo descargar el agente oficial de Playit: {e}"),
                )
            })?;
        self.log
            .line(&format!("playit: agente {} descargado y verificado", pl::AGENT_VERSION));
        Ok(())
    }

    /// Vinculación: se espera la aprobación en playit.gg y se entrega el secreto al agente.
    fn playit_claim_thread(self: Arc<Self>, code: String) {
        let set = |d: &Daemon, f: &dyn Fn(&mut ClaimInfo)| {
            let mut rt = d.playit.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(c) = rt.claim.as_mut() {
                f(c);
            }
        };
        let api_base = Self::playit_api();
        let started = Instant::now();
        let result = (|| -> Result<(), String> {
            let client = ClaimClient::new(&self.http, api_base.clone());
            loop {
                if started.elapsed() > CLAIM_TIMEOUT {
                    return Err("Se acabó el tiempo para aprobar la vinculación. Empieza de nuevo.".into());
                }
                match client.setup(&code) {
                    Ok(ClaimState::Approved) => break,
                    Ok(ClaimState::Rejected) => return Err("La vinculación se rechazó en playit.gg.".into()),
                    Ok(ClaimState::Expired) => return Err("El enlace de vinculación caducó. Empieza de nuevo.".into()),
                    Ok(s) => set(&self, &|c| {
                        c.state = s;
                        c.error = None;
                    }),
                    // Sin conexión un momento: se reintenta.
                    Err(e) => set(&self, &|c| c.error = Some(e.to_string())),
                }
                std::thread::sleep(Duration::from_secs(2));
            }
            set(&self, &|c| {
                c.state = ClaimState::Approved;
                c.error = None;
            });
            let deadline = Instant::now() + Duration::from_secs(120);
            let secret = loop {
                match client.exchange(&code) {
                    Ok(Some(s)) => break s,
                    Ok(None) => {}
                    Err(e) if Instant::now() > deadline => return Err(e.to_string()),
                    Err(_) => {}
                }
                if Instant::now() > deadline {
                    return Err("Playit no entregó la clave del agente a tiempo.".into());
                }
                std::thread::sleep(Duration::from_secs(2));
            };
            // El agente guarda el secreto en su archivo; aquí solo pasa por memoria.
            let deadline = Instant::now() + Duration::from_secs(60);
            loop {
                match self.agent_ipc().and_then(|mut c| c.set_secret(&secret)) {
                    Ok(()) => return Ok(()),
                    Err(e) if Instant::now() > deadline => return Err(e.to_string()),
                    Err(_) => std::thread::sleep(Duration::from_secs(1)),
                }
            }
        })();
        match &result {
            Ok(()) => {
                set(&self, &|c| {
                    c.done = true;
                    c.error = None;
                });
                self.audit("playit", None, "playit.link", "vinculado con la cuenta de playit.gg");
            }
            Err(e) => set(&self, &|c| c.error = Some(e.clone())),
        }
        self.playit.lock().unwrap_or_else(|p| p.into_inner()).claim_running = false;
    }

    pub(crate) fn server_port(inst: &merc_core::Instance) -> Option<u16> {
        merc_core::properties::ServerProperties::read(&inst.root)
            .ok()
            .flatten()
            .map(|p| p.port)
    }

    pub(crate) fn playit_on(&self) -> bool {
        self.playit_saved().enabled || Self::playit_external().is_some()
    }

    /// Estado del agente si Playit está activado y responde.
    pub(crate) fn playit_agent_status(&self) -> Option<AgentStatus> {
        if self.playit_saved().enabled || Self::playit_external().is_some() {
            self.playit_agent().0
        } else {
            None
        }
    }

    pub(crate) fn connection(&self, inst: &merc_core::Instance) -> ConnectionInfo {
        let port = Self::server_port(inst);
        let lan = match (super::lan_ipv4(), port) {
            (Some(ip), Some(p)) => vec![format!("{ip}:{p}")],
            _ => vec![],
        };
        let playit_on = self.playit_saved().enabled || Self::playit_external().is_some();
        let (agent, agent_error) = if playit_on { self.playit_agent() } else { (None, None) };
        let tunnels: Vec<Tunnel> = agent
            .as_ref()
            .map(|a| {
                a.tunnels
                    .iter()
                    .filter(|t| port.is_some() && t.local_port() == port)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let tests = self
            .playit
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .tests
            .get(&inst.id)
            .cloned()
            .unwrap_or_default();
        let others = self.others_ports(inst);
        let ports_in_use = others
            .iter()
            .filter_map(|o| {
                o.minecraft.map(|port| PortUse {
                    port,
                    server: o.name.clone(),
                })
            })
            .collect();
        let status = self.sup.status(inst.id);
        let voice = self.voice_info(inst, port, playit_on, agent.as_ref(), &status, &others);
        let layers = self.layers(
            &status,
            playit_on,
            agent.as_ref(),
            agent_error.as_deref(),
            port,
            &tunnels,
            &tests,
            voice.as_ref(),
        );
        ConnectionInfo {
            port,
            lan,
            tunnels,
            tests,
            ports_in_use,
            voice,
            layers,
        }
    }

    pub(crate) fn handle_playit(self: &Arc<Self>, client: &str, body: RequestBody) -> Result<ResponseBody, ApiError> {
        use RequestBody as R;
        Ok(match body {
            R::PlayitStatus => ResponseBody::Playit(Box::new(self.playit_info())),
            R::PlayitEnable => {
                if !Self::playit_supported() {
                    return Err(api(
                        ErrorCode::BadRequest,
                        "El agente oficial de Playit que usa MercHosting es para Windows.",
                    ));
                }
                self.playit_install()?;
                self.playit_save(&Saved { enabled: true })?;
                {
                    let mut rt = self.playit.lock().unwrap_or_else(|p| p.into_inner());
                    rt.retry_at = None;
                    rt.failures = 0;
                }
                self.playit_ensure_running();
                self.audit(client, None, "playit.enable", pl::AGENT_VERSION);
                // Un momento para que el agente abra su canal.
                let deadline = Instant::now() + Duration::from_secs(5);
                while Instant::now() < deadline && self.agent_ipc().is_err() {
                    std::thread::sleep(Duration::from_millis(200));
                }
                ResponseBody::Playit(Box::new(self.playit_info()))
            }
            R::PlayitDisable => {
                self.playit_save(&Saved { enabled: false })?;
                self.playit_stop();
                self.audit(client, None, "playit.disable", "");
                ResponseBody::Playit(Box::new(self.playit_info()))
            }
            R::PlayitLink => {
                if !self.playit_saved().enabled && Self::playit_external().is_none() {
                    return Err(api(
                        ErrorCode::InvalidState,
                        "Activa Playit antes de vincular tu cuenta.",
                    ));
                }
                {
                    let mut rt = self.playit.lock().unwrap_or_else(|p| p.into_inner());
                    if rt.claim_running {
                        drop(rt);
                        return Ok(ResponseBody::Playit(Box::new(self.playit_info())));
                    }
                    let bytes = *uuid::Uuid::new_v4().as_bytes();
                    let code = pl::claim_code([bytes[0], bytes[1], bytes[2], bytes[3], bytes[4]]);
                    let url = pl::claim_url(&code).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
                    rt.claim = Some(ClaimInfo {
                        url,
                        state: ClaimState::WaitingForVisit,
                        done: false,
                        error: None,
                    });
                    rt.claim_running = true;
                    let d = self.clone();
                    std::thread::spawn(move || d.playit_claim_thread(code));
                }
                self.audit(client, None, "playit.link.start", "");
                ResponseBody::Playit(Box::new(self.playit_info()))
            }
            R::PlayitUnlink => {
                self.agent_ipc()
                    .and_then(|mut c| c.reset_secret())
                    .map_err(|e| api(ErrorCode::InvalidState, format!("No se pudo desvincular: {e}")))?;
                self.playit.lock().unwrap_or_else(|p| p.into_inner()).claim = None;
                self.audit(client, None, "playit.unlink", "");
                ResponseBody::Playit(Box::new(self.playit_info()))
            }
            R::PlayitLoginUrl => ResponseBody::PlayitLoginUrl(
                self.agent_ipc()
                    .and_then(|mut c| c.login_url())
                    .map_err(|e| api(ErrorCode::InvalidState, e.to_string()))?,
            ),
            R::Connection { id } => {
                let inst = self.instance(id)?;
                ResponseBody::Connection(Box::new(self.connection(&inst)))
            }
            R::TestAddress { id, address } => {
                let inst = self.instance(id)?;
                let address = address.trim().to_string();
                merc_net::slp::split_address(&address).map_err(|e| api(ErrorCode::BadRequest, e.to_string()))?;
                let r = merc_net::slp::ping_address(&address, Duration::from_secs(8));
                let test = AddressTest {
                    address: address.clone(),
                    at_unix_ms: now_ms(),
                    error: r.as_ref().err().map(|e| e.to_string()),
                    result: r.ok(),
                };
                {
                    let mut rt = self.playit.lock().unwrap_or_else(|p| p.into_inner());
                    let list = rt.tests.entry(id).or_default();
                    list.retain(|t| t.address != address);
                    list.insert(0, test.clone());
                    list.truncate(10);
                }
                self.audit(
                    client,
                    Some(id),
                    "connection.test",
                    &match &test.result {
                        Some(r) => format!("{address}: responde en {} ms", r.latency_ms),
                        None => format!("{address}: {}", test.error.clone().unwrap_or_default()),
                    },
                );
                ResponseBody::Connection(Box::new(self.connection(&inst)))
            }
            R::SetVoiceConfig { id, voice_host, port } => {
                let inst = self.instance(id)?;
                ResponseBody::Connection(Box::new(self.set_voice_config(client, &inst, voice_host, port)?))
            }
            _ => return Err(api(ErrorCode::BadRequest, "Petición no válida")),
        })
    }
}
