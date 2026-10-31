//! Alertas que vigilan cosas fuera del ciclo de cada servidor:
//! - túnel de Playit caído (y cuando vuelve) con un servidor encendido;
//! - actualización disponible del software del servidor (una vez por versión nueva).
//!
//! Las copias fallidas se avisan donde se hacen (`backups.rs`).

use super::Daemon;
use merc_admin::health::{AlertKind, AlertLevel};
use merc_core::RuntimeState;
use merc_net::playit::{tunnel_check, TunnelCheck};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct TunnelWatch {
    /// Se vio el túnel funcionando con un servidor encendido (si no, no hay nada que «se caiga»).
    ever_served: bool,
    down_since: Option<Instant>,
    alerted: bool,
}

static TUNNEL: Mutex<Option<TunnelWatch>> = Mutex::new(None);

fn debug_ms(var: &str) -> Option<Duration> {
    if cfg!(debug_assertions) {
        std::env::var(var)
            .ok()
            .and_then(|v| v.parse().ok())
            .map(Duration::from_millis)
    } else {
        None
    }
}

/// Cuánto tiene que durar el problema antes de avisar (Playit se reconecta solo a menudo).
fn tunnel_grace() -> Duration {
    debug_ms("MERC_TUNNEL_GRACE_MS").unwrap_or(Duration::from_secs(90))
}

fn update_every() -> Duration {
    debug_ms("MERC_UPDATE_CHECK_MS").unwrap_or(Duration::from_secs(6 * 3600))
}

impl Daemon {
    /// Se llama en cada vuelta del ciclo de salud.
    pub(crate) fn tunnel_watch(&self) {
        let instances = self
            .store
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .list_instances()
            .unwrap_or_default();
        let online: Vec<&merc_core::Instance> = instances
            .iter()
            .filter(|i| self.sup.status(i.id).state == RuntimeState::Online)
            .collect();
        let ports: Vec<u16> = online.iter().filter_map(|i| Self::server_port(i)).collect();
        let mut g = TUNNEL.lock().unwrap_or_else(|p| p.into_inner());
        let w = g.get_or_insert_with(TunnelWatch::default);
        if ports.is_empty() || !self.playit_on() {
            // Sin servidores encendidos (o sin Playit) no hay nada que vigilar.
            *w = TunnelWatch {
                ever_served: w.ever_served,
                ..Default::default()
            };
            return;
        }
        let agent = self.playit_agent_status();
        let names = online
            .iter()
            .map(|i| format!("«{}»", i.name))
            .collect::<Vec<_>>()
            .join(", ");
        match tunnel_check(agent.as_ref(), &ports) {
            TunnelCheck::Serving => {
                if w.alerted {
                    self.push_alert(
                        AlertKind::TunnelDown,
                        AlertLevel::Info,
                        None,
                        "El túnel de Playit volvió a funcionar".into(),
                        format!("Tus amigos ya pueden entrar otra vez a {names}."),
                    );
                }
                *w = TunnelWatch {
                    ever_served: true,
                    ..Default::default()
                };
            }
            TunnelCheck::Problem(why) if w.ever_served => {
                let since = *w.down_since.get_or_insert_with(Instant::now);
                if !w.alerted && since.elapsed() >= tunnel_grace() {
                    w.alerted = true;
                    self.push_alert(
                        AlertKind::TunnelDown,
                        AlertLevel::Warning,
                        None,
                        "El túnel de Playit no funciona".into(),
                        format!(
                            "{why} Mientras tanto, tus amigos no pueden entrar desde fuera a {names}. \
                             Mira la pestaña Conexión."
                        ),
                    );
                }
            }
            _ => {
                w.down_since = None;
            }
        }
    }

    /// Hilo: busca de vez en cuando versiones nuevas del software de cada servidor.
    pub(crate) fn update_watch_loop(self: Arc<Self>) {
        // Primera revisión un rato después de arrancar (no compite con el inicio).
        std::thread::sleep(debug_ms("MERC_UPDATE_CHECK_MS").unwrap_or(Duration::from_secs(120)));
        loop {
            self.update_watch_once();
            std::thread::sleep(update_every());
        }
    }

    fn update_watch_once(&self) {
        if !self.alert_settings().updates {
            return;
        }
        let instances = self
            .store
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .list_instances()
            .unwrap_or_default();
        for inst in instances.iter().filter(|i| !self.is_archived(i.id)) {
            let Ok(info) = self.server_update_info_for_alert(inst) else {
                continue; // Sin conexión: se vuelve a mirar en la próxima vuelta.
            };
            let (true, true, Some(latest)) = (info.supported, info.newer, info.latest.clone()) else {
                continue;
            };
            let marker = self.store_dir(inst.id).join("aviso-actualizacion.txt");
            if std::fs::read_to_string(&marker).is_ok_and(|v| v.trim() == latest) {
                continue; // Ya se avisó de esta versión.
            }
            self.push_alert(
                AlertKind::UpdateAvailable,
                AlertLevel::Info,
                Some((inst.id, inst.name.clone())),
                format!("Hay una actualización para «{}»", inst.name),
                format!(
                    "{} → {latest} (misma versión de Minecraft{}). Se instala desde la pestaña Actualizaciones, con copia previa.",
                    info.current.as_deref().unwrap_or("versión desconocida"),
                    if info.experimental { ", experimental" } else { "" }
                ),
            );
            let _ = std::fs::create_dir_all(self.store_dir(inst.id));
            let _ = std::fs::write(&marker, &latest);
        }
    }
}
