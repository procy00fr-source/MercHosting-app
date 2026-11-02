//! Playit en el celular: el agente oficial va dentro del APK (`libplayit.so`) y lo arranca el
//! servicio en primer plano junto al servidor. Desde aquí solo se habla con él por su canal local
//! (archivo de socket en la carpeta privada de la app) y se hace la vinculación oficial («claim»)
//! con la cuenta de playit.gg de la persona. El secreto lo guarda el agente en su archivo; aquí
//! solo pasa por memoria. Los túneles se crean en playit.gg (igual que en escritorio).

use merc_net::playit::{self as pl, AgentIpc, AgentPhase, AgentStatus, ClaimClient, ClaimState};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Rutas del agente dentro de la carpeta de la app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentPaths {
    pub dir: PathBuf,
    pub secret: PathBuf,
    pub socket: PathBuf,
    pub log: PathBuf,
}

impl AgentPaths {
    pub fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            secret: dir.join("playit.toml"),
            socket: dir.join("playit.sock"),
            log: dir.join("playitd.log"),
        }
    }

    /// Argumentos con los que el servicio arranca el agente (los mismos que en escritorio).
    pub fn args(&self) -> Vec<String> {
        vec![
            "--secret-path".into(),
            self.secret.display().to_string(),
            "--socket-path".into(),
            self.socket.display().to_string(),
            "--log-path".into(),
            self.log.display().to_string(),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClaimInfo {
    pub url: String,
    pub state: ClaimState,
    pub done: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlayitInfo {
    /// El agente va dentro de esta app (si no, no se puede usar Playit en el celular).
    pub available: bool,
    /// El agente responde por su canal local.
    pub running: bool,
    /// El agente en marcha ya tiene la clave de una cuenta.
    pub linked: bool,
    pub agent: Option<AgentStatus>,
    pub claim: Option<ClaimInfo>,
    /// Dirección pública del túnel que lleva al puerto del servidor de este celular.
    pub address: Option<String>,
    pub error: Option<String>,
}

const CLAIM_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Default)]
pub struct Playit {
    claim: Arc<Mutex<Option<ClaimInfo>>>,
}

impl Playit {
    fn ipc(paths: &AgentPaths) -> Result<AgentIpc, String> {
        AgentIpc::connect(&paths.socket.display().to_string(), Duration::from_secs(2)).map_err(|e| e.to_string())
    }

    pub fn info(&self, paths: &AgentPaths, available: bool, server_port: Option<u16>) -> PlayitInfo {
        let (agent, error) = match Self::ipc(paths).and_then(|mut c| c.state().map_err(|e| e.to_string())) {
            Ok(s) => (Some(s), None),
            Err(e) => (None, Some(e)),
        };
        PlayitInfo {
            available,
            running: agent.is_some(),
            linked: agent
                .as_ref()
                .is_some_and(|a| !matches!(a.phase, AgentPhase::WaitingForSecret | AgentPhase::InvalidSecret)),
            address: agent.as_ref().and_then(|a| tunnel_for(a, server_port?)),
            agent,
            claim: self.claim.lock().unwrap_or_else(|p| p.into_inner()).clone(),
            // Si el agente no está en marcha no es un error: se arranca con el servidor.
            error: if available {
                None
            } else {
                error.map(|_| "Esta versión de la app no trae el agente de Playit.".into())
            },
        }
    }

    /// Empieza la vinculación: devuelve el enlace que la persona abre con SU cuenta.
    pub fn start_claim(
        &self,
        http: Arc<merc_install::Http>,
        paths: AgentPaths,
        api_base: String,
    ) -> Result<ClaimInfo, String> {
        {
            let c = self.claim.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(c) = c.as_ref() {
                if !c.done && c.error.is_none() {
                    return Ok(c.clone());
                }
            }
        }
        let bytes = *uuid::Uuid::new_v4().as_bytes();
        let code = pl::claim_code([bytes[0], bytes[1], bytes[2], bytes[3], bytes[4]]);
        let url = pl::claim_url(&code).map_err(|e| e.to_string())?;
        let info = ClaimInfo {
            url,
            state: ClaimState::WaitingForVisit,
            done: false,
            error: None,
        };
        *self.claim.lock().unwrap_or_else(|p| p.into_inner()) = Some(info.clone());
        let slot = self.claim.clone();
        std::thread::spawn(move || {
            let set = |f: &dyn Fn(&mut ClaimInfo)| {
                if let Some(c) = slot.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
                    f(c);
                }
            };
            let r = run_claim(&http, &api_base, &code, &paths, &|s| set(&|c| c.state = s));
            match r {
                Ok(()) => set(&|c| {
                    c.done = true;
                    c.state = ClaimState::Approved;
                    c.error = None;
                }),
                Err(e) => set(&|c| c.error = Some(e.clone())),
            }
        });
        Ok(info)
    }

    /// Desvincula: el agente borra su secreto.
    pub fn unlink(&self, paths: &AgentPaths) -> Result<(), String> {
        Self::ipc(paths)?.reset_secret().map_err(|e| e.to_string())?;
        *self.claim.lock().unwrap_or_else(|p| p.into_inner()) = None;
        Ok(())
    }
}

/// Túnel activo hacia ese puerto local.
pub fn tunnel_for(agent: &AgentStatus, port: u16) -> Option<String> {
    agent
        .tunnels
        .iter()
        .find(|t| !t.is_disabled && t.local_port() == Some(port))
        .map(|t| t.display_address.clone())
}

fn run_claim(
    http: &merc_install::Http,
    api_base: &str,
    code: &str,
    paths: &AgentPaths,
    on_state: &dyn Fn(ClaimState),
) -> Result<(), String> {
    let client = ClaimClient::new(http, api_base);
    let started = Instant::now();
    loop {
        if started.elapsed() > CLAIM_TIMEOUT {
            return Err("Se acabó el tiempo para aprobar la vinculación. Empieza de nuevo.".into());
        }
        match client.setup(code) {
            Ok(ClaimState::Approved) => break,
            Ok(ClaimState::Rejected) => return Err("La vinculación se rechazó en playit.gg.".into()),
            Ok(ClaimState::Expired) => return Err("El enlace de vinculación caducó. Empieza de nuevo.".into()),
            Ok(s) => on_state(s),
            Err(_) => {}
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    on_state(ClaimState::Approved);
    let deadline = Instant::now() + Duration::from_secs(120);
    let secret = loop {
        if let Ok(Some(s)) = client.exchange(code) {
            break s;
        }
        if Instant::now() > deadline {
            return Err("Playit no entregó la clave del agente a tiempo.".into());
        }
        std::thread::sleep(Duration::from_secs(2));
    };
    // El agente tiene que estar en marcha (enciende el servidor con Playit activado).
    let deadline = Instant::now() + Duration::from_secs(10 * 60);
    loop {
        match Playit::ipc(paths).and_then(|mut c| c.set_secret(&secret).map_err(|e| e.to_string())) {
            Ok(()) => return Ok(()),
            Err(e) if Instant::now() > deadline => {
                return Err(format!("No se pudo entregar la clave al agente de Playit: {e}"))
            }
            Err(_) => std::thread::sleep(Duration::from_secs(2)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_paths_and_args() {
        let d = tempfile::tempdir().unwrap();
        let p = AgentPaths::new(d.path());
        let a = p.args();
        assert_eq!(a[0], "--secret-path");
        assert!(a[1].ends_with("playit.toml"));
        assert_eq!(a[2], "--socket-path");
        assert!(a[3].ends_with("playit.sock"));
        // Ruta corta: un socket Unix admite como mucho 108 bytes.
        assert!("/data/user/0/app.merchosting.movil/files/playit/playit.sock".len() < 100);
    }

    #[test]
    fn tunnel_matches_port() {
        let a = AgentStatus {
            phase: AgentPhase::Running,
            message: None,
            version: None,
            account_status: None,
            tunnels: vec![
                pl::Tunnel {
                    display_address: "otro.joinmc.link".into(),
                    destination: "127.0.0.1:25566".into(),
                    ..Default::default()
                },
                pl::Tunnel {
                    display_address: "mio.joinmc.link".into(),
                    destination: "127.0.0.1:25565".into(),
                    ..Default::default()
                },
            ],
            pending: vec![],
            notices: vec![],
        };
        assert_eq!(tunnel_for(&a, 25565).as_deref(), Some("mio.joinmc.link"));
        assert_eq!(tunnel_for(&a, 25570), None);
    }

    #[test]
    fn info_without_agent() {
        let d = tempfile::tempdir().unwrap();
        let p = Playit::default();
        let i = p.info(&AgentPaths::new(d.path()), false, Some(25565));
        assert!(!i.available && !i.running && i.error.is_some());
        let i = p.info(&AgentPaths::new(d.path()), true, Some(25565));
        assert!(i.available && !i.running && !i.linked && i.error.is_none() && i.address.is_none());
    }
}
