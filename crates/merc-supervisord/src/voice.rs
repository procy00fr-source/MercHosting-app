//! Simple Voice Chat y estado por capas de la conexión de un servidor.
//!
//! Todo lo que se muestra sale de datos reales: el jar instalado, su archivo de configuración,
//! lo que dijo la consola en esta sesión, los túneles que informa el agente de Playit y las
//! pruebas que la persona lanzó. La voz (UDP) no se puede probar desde aquí, y así se dice.

use super::{admin_err, api, Daemon};
use merc_admin::voicechat::{self as vc, LogSignal};
use merc_core::{Instance, RuntimeState};
use merc_ipc::*;
use merc_supervisor::StatusSnapshot;
use std::collections::BTreeSet;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;

/// Puertos de los otros servidores registrados.
pub(crate) struct OtherServer {
    pub name: String,
    pub minecraft: Option<u16>,
    pub voice: Option<u16>,
}

fn problem(code: &str, level: &str, message: impl Into<String>) -> VoiceProblem {
    VoiceProblem {
        code: code.into(),
        level: level.into(),
        message: message.into(),
    }
}

fn has_port(address: &str) -> bool {
    address
        .rsplit_once(':')
        .is_some_and(|(_, p)| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn modified_ms(path: &std::path::Path) -> Option<u64> {
    let m = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(m.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as u64)
}

/// Últimos ~1 MB de `logs/latest.log` si se escribió en esta sesión del servidor.
fn latest_log_tail(inst: &Instance, since_ms: u64) -> Option<String> {
    let path = inst.root.join("logs").join("latest.log");
    if modified_ms(&path)? < since_ms {
        return None;
    }
    let mut f = std::fs::File::open(&path).ok()?;
    let len = f.metadata().ok()?.len();
    let from = len.saturating_sub(1024 * 1024);
    f.seek(SeekFrom::Start(from)).ok()?;
    let mut buf = vec![];
    f.read_to_end(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

pub(crate) fn is_running(s: RuntimeState) -> bool {
    matches!(
        s,
        RuntimeState::Validating | RuntimeState::Starting | RuntimeState::Loading | RuntimeState::Online
    )
}

impl Daemon {
    /// Puerto real de la voz de un servidor (si tiene el mod y su configuración).
    pub(crate) fn voice_port_of(inst: &Instance, minecraft: Option<u16>) -> Option<u16> {
        vc::find_jar(&inst.root)?;
        vc::read_config(&inst.root)?.0.effective_port(minecraft)
    }

    fn voice_log(&self, inst: &Instance, status: &StatusSnapshot) -> Option<LogSignal> {
        let since = status.started_at_unix_ms.unwrap_or(0);
        let lines = self.sup.console_since(inst.id, 0);
        let from_console = vc::scan_log(lines.iter().filter(|l| l.at_unix_ms >= since).map(|l| l.text.as_str()));
        from_console.or_else(|| {
            let text = latest_log_tail(inst, since)?;
            vc::scan_log(text.lines())
        })
    }

    pub(crate) fn voice_info(
        &self,
        inst: &Instance,
        minecraft: Option<u16>,
        playit_on: bool,
        agent: Option<&AgentStatus>,
        status: &StatusSnapshot,
        others: &[OtherServer],
    ) -> Option<VoiceInfo> {
        let jar = vc::find_jar(&inst.root);
        let cfg = vc::read_config(&inst.root).map(|(c, _)| c);
        let port = cfg.as_ref().and_then(|c| c.effective_port(minecraft));
        let shared = port.is_some() && port == minecraft;
        // Si comparte puerto con Minecraft no se puede saber qué túnel es de la voz.
        let tunnels: Vec<Tunnel> = match (agent, port) {
            (Some(a), Some(p)) if !shared => a
                .tunnels
                .iter()
                .filter(|t| t.local_port() == Some(p))
                .cloned()
                .collect(),
            _ => vec![],
        };
        let base = |installed: bool, enabled: bool| VoiceInfo {
            installed,
            enabled,
            jar: jar.as_ref().map(|j| j.rel.clone()),
            config: cfg.as_ref().map(|c| c.rel.clone()),
            port_setting: cfg.as_ref().and_then(|c| c.port),
            port,
            voice_host: cfg.as_ref().map(|c| c.voice_host.clone()).unwrap_or_default(),
            bind_address: cfg.as_ref().map(|c| c.bind_address.clone()).unwrap_or_default(),
            tunnels: tunnels.clone(),
            suggested_port: None,
            listening: None,
            restart_needed: false,
            problems: vec![],
        };
        let Some(jar) = jar.clone() else {
            // Ya no está el mod pero queda un túnel hacia su puerto.
            if tunnels.is_empty() {
                return None;
            }
            let mut v = base(false, false);
            v.problems.push(problem(
                "leftover_tunnel",
                "info",
                format!(
                    "Ya no tienes Simple Voice Chat en este servidor, pero sigue habiendo un túnel de Playit hacia su puerto {}. Si no lo vas a usar, puedes desactivarlo o borrarlo en playit.gg (MercHosting no borra túneles).",
                    port.unwrap_or_default()
                ),
            ));
            return Some(v);
        };
        let mut v = base(true, jar.enabled);
        let p = &mut v.problems;

        // Puertos ya ocupados por otros servidores (y el de Minecraft de este).
        let mut used: BTreeSet<u16> = BTreeSet::new();
        used.extend(minecraft);
        for o in others {
            used.extend(o.minecraft);
            used.extend(o.voice);
        }
        let free_port = || (vc::DEFAULT_PORT..=65535).find(|q| !used.contains(q));

        if !jar.enabled {
            p.push(problem(
                "disabled",
                "warning",
                "Simple Voice Chat está desactivado. Actívalo en la pestaña Contenido para usar la voz.",
            ));
        }
        let Some(cfg) = cfg else {
            p.push(problem(
                "no_config",
                "info",
                "Todavía no existe su configuración. Enciende el servidor una vez con el mod para que la cree; después aquí verás su puerto real.",
            ));
            p.push(client_mod());
            return Some(v);
        };

        let running = is_running(status.state);
        if running {
            let started = status.started_at_unix_ms.unwrap_or(0);
            let changed = modified_ms(&inst.root.join(&cfg.rel)).is_some_and(|m| m > started);
            match self.voice_log(inst, status) {
                Some(LogSignal::Started { port: l }) => {
                    v.listening = Some(l);
                    if Some(l) != port {
                        v.restart_needed = true;
                    }
                }
                Some(LogSignal::Failed(line)) => p.push(problem(
                    "log_error",
                    "error",
                    format!("La consola dice que la voz no pudo abrir su puerto: «{line}». Suele pasar cuando otro programa (otro servidor, Geyser…) ya lo usa."),
                )),
                None if status.state == RuntimeState::Online => p.push(problem(
                    "not_listening",
                    "warning",
                    "El servidor está encendido pero la consola no muestra que la voz arrancara. Busca errores de «voicechat» en la consola.",
                )),
                None => {}
            }
            if changed {
                v.restart_needed = true;
            }
            if v.restart_needed {
                p.push(problem(
                    "restart_needed",
                    "warning",
                    "La configuración de la voz cambió después de encender el servidor. Reinícialo para aplicarla.",
                ));
            }
        }

        match port {
            None => p.push(problem(
                "no_port",
                "error",
                "El archivo de la voz no tiene un «port» válido.",
            )),
            Some(_) if shared => {
                v.suggested_port = free_port();
                p.push(problem(
                    "shared_port",
                    "error",
                    "La voz usa el mismo puerto que Minecraft. Simple Voice Chat recomienda no hacerlo: ese puerto UDP también lo usa la consulta del servidor y puede cerrarlo. Dale un puerto propio.",
                ));
            }
            Some(q) => {
                if let Some(o) = others.iter().find(|o| o.voice == Some(q) || o.minecraft == Some(q)) {
                    v.suggested_port = free_port();
                    p.push(problem(
                        "port_conflict",
                        "error",
                        format!(
                            "El puerto {q} también lo usa «{}». Si los dos servidores se encienden a la vez, la voz de uno no arrancará.",
                            o.name
                        ),
                    ));
                }
            }
        }

        if vc::bind_address_is_restrictive(&cfg.bind_address) {
            p.push(problem(
                "bind_address",
                "warning",
                format!(
                    "«bind_address» es «{}». Si no te lo pidió alguien que sabe por qué, déjalo vacío: con otro valor la voz puede no aceptar a nadie de fuera.",
                    cfg.bind_address
                ),
            ));
        }

        if !playit_on {
            p.push(problem(
                "playit_off",
                "info",
                "Quien juega en tu misma red ya puede usar la voz. Para amigos de fuera, activa Playit y crea un túnel de voz.",
            ));
        } else if agent.is_some() && port.is_some() && !shared {
            let usable: Vec<&Tunnel> = tunnels.iter().filter(|t| !t.is_disabled).collect();
            if tunnels.is_empty() {
                p.push(problem(
                    "no_tunnel",
                    "warning",
                    format!(
                        "Falta el túnel de voz. En playit.gg crea un túnel de tipo «Simple Voice Chat» con IP local 127.0.0.1 y puerto local {}.",
                        port.unwrap_or_default()
                    ),
                ));
            } else if usable.is_empty() {
                let why = tunnels[0].disabled_reason.clone().unwrap_or_default();
                p.push(problem(
                    "tunnel_disabled",
                    "error",
                    format!("Playit tiene desactivado el túnel de voz. {why}")
                        .trim()
                        .to_string(),
                ));
            } else if usable
                .iter()
                .any(|t| vc::host_matches(&cfg.voice_host, &t.display_address))
            {
                // Bien configurado.
            } else if !usable.iter().any(|t| has_port(&t.display_address)) {
                p.push(problem(
                    "tunnel_no_port",
                    "warning",
                    "El túnel de voz no informa su puerto público. Copia la «IP:puerto» que muestra arriba la página del túnel en playit.gg y ponla en «voice_host».",
                ));
            } else if cfg.voice_host.is_empty() {
                p.push(problem(
                    "host_missing",
                    "error",
                    "Los amigos de fuera no podrán hablar: «voice_host» está vacío, así que su juego buscará la voz en la dirección de Minecraft, que no la lleva.",
                ));
            } else {
                p.push(problem(
                    "host_mismatch",
                    "error",
                    format!(
                        "«voice_host» dice «{}», pero tu túnel de voz es «{}».",
                        cfg.voice_host, usable[0].display_address
                    ),
                ));
            }
        }
        p.push(client_mod());
        Some(v)
    }

    /// Estado por capas: proceso, agente, túnel, prueba externa y voz.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn layers(
        &self,
        status: &StatusSnapshot,
        playit_on: bool,
        agent: Option<&AgentStatus>,
        agent_error: Option<&str>,
        port: Option<u16>,
        tunnels: &[Tunnel],
        tests: &[AddressTest],
        voice: Option<&VoiceInfo>,
    ) -> Vec<ConnLayer> {
        let now = super::playit::now_ms();
        let layer = |key: &str, state: LayerState, detail: String| ConnLayer {
            key: key.into(),
            state,
            detail,
            checked_at_unix_ms: now,
        };
        let mut out = vec![];
        use LayerState::*;
        out.push(match status.state {
            RuntimeState::Online => layer("server", Operativo, "Encendido y aceptando jugadores.".into()),
            RuntimeState::Validating | RuntimeState::Starting | RuntimeState::Loading => {
                layer("server", Degradado, "Arrancando: todavía no acepta jugadores.".into())
            }
            RuntimeState::Stopping => layer("server", Degradado, "Apagándose.".into()),
            RuntimeState::Crashed => layer("server", Caido, "Se cerró solo. Mira la consola.".into()),
            _ => layer("server", Caido, "Apagado.".into()),
        });
        if playit_on {
            out.push(match agent {
                None => layer(
                    "agent",
                    Caido,
                    agent_error.unwrap_or("El agente de Playit no responde.").to_string(),
                ),
                Some(a) => match a.phase {
                    AgentPhase::Running if a.notices.iter().any(|n| n.priority.eq_ignore_ascii_case("high")) => {
                        layer("agent", Degradado, "Funciona, pero Playit tiene avisos para ti.".into())
                    }
                    AgentPhase::Running => layer("agent", Operativo, "En marcha y vinculado.".into()),
                    AgentPhase::Starting => layer("agent", Degradado, "Arrancando.".into()),
                    AgentPhase::WaitingForSecret => {
                        layer("agent", Degradado, "En marcha, falta vincular tu cuenta.".into())
                    }
                    AgentPhase::Stopping => layer("agent", Degradado, "Deteniéndose.".into()),
                    AgentPhase::InvalidSecret => layer(
                        "agent",
                        Caido,
                        "La vinculación ya no es válida. Vuelve a vincular.".into(),
                    ),
                    AgentPhase::OverLimit => layer(
                        "agent",
                        Caido,
                        "Tu cuenta superó el límite de agentes de su plan.".into(),
                    ),
                    AgentPhase::Error => layer(
                        "agent",
                        Caido,
                        a.message
                            .clone()
                            .unwrap_or_else(|| "El agente informó un error.".into()),
                    ),
                },
            });
            let running = agent.is_some_and(|a| a.phase == AgentPhase::Running);
            out.push(if !running {
                layer(
                    "tunnel",
                    Desconocido,
                    "Sin agente en marcha no se pueden leer los túneles.".into(),
                )
            } else if let Some(t) = tunnels.iter().find(|t| !t.is_disabled) {
                layer(
                    "tunnel",
                    Operativo,
                    format!("{} → {}", t.display_address, t.destination),
                )
            } else if let Some(t) = tunnels.first() {
                layer(
                    "tunnel",
                    Caido,
                    format!(
                        "Playit lo tiene desactivado. {}",
                        t.disabled_reason.clone().unwrap_or_default()
                    )
                    .trim()
                    .to_string(),
                )
            } else {
                layer(
                    "tunnel",
                    Caido,
                    match port {
                        Some(p) => format!("No hay un túnel hacia el puerto {p} de este servidor."),
                        None => "No se sabe el puerto del servidor.".into(),
                    },
                )
            });
            let last = tests
                .iter()
                .filter(|t| {
                    tunnels
                        .iter()
                        .any(|x| x.display_address.eq_ignore_ascii_case(&t.address))
                })
                .max_by_key(|t| t.at_unix_ms);
            out.push(match last {
                None => layer(
                    "external",
                    Desconocido,
                    "Aún no se probó la dirección pública. Usa «Probar conexión».".into(),
                ),
                Some(t) => ConnLayer {
                    key: "external".into(),
                    state: if t.result.is_some() { Operativo } else { Caido },
                    detail: match (&t.result, &t.error) {
                        (Some(r), _) => format!("{} respondió en {} ms.", t.address, r.latency_ms),
                        (None, e) => format!("{}: {}", t.address, e.clone().unwrap_or_default()),
                    },
                    checked_at_unix_ms: t.at_unix_ms,
                },
            });
        }
        if let Some(v) = voice.filter(|v| v.installed) {
            let errors = v.problems.iter().any(|p| p.level == "error");
            let warnings = v.problems.iter().any(|p| p.level == "warning");
            out.push(if !v.enabled {
                layer("voice", Caido, "Simple Voice Chat está desactivado.".into())
            } else if !is_running(status.state) {
                layer("voice", Desconocido, "El servidor está apagado.".into())
            } else if v.problems.iter().any(|p| p.code == "log_error") {
                layer("voice", Caido, "No pudo abrir su puerto UDP.".into())
            } else if let Some(l) = v.listening {
                let base = format!("Escuchando en el puerto UDP {l}.");
                if errors || warnings {
                    layer("voice", Degradado, format!("{base} Hay cosas que revisar abajo."))
                } else {
                    layer(
                        "voice",
                        Operativo,
                        format!(
                            "{base} La voz no se puede probar desde fuera; en el juego usa /voicechat test <jugador>."
                        ),
                    )
                }
            } else {
                layer(
                    "voice",
                    Desconocido,
                    "La consola aún no muestra que la voz arrancara.".into(),
                )
            });
        }
        out
    }

    pub(crate) fn others_ports(&self, inst: &Instance) -> Vec<OtherServer> {
        self.store
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .list_instances()
            .unwrap_or_default()
            .into_iter()
            .filter(|i| i.id != inst.id)
            .map(|i| {
                let mc = Self::server_port(&i);
                OtherServer {
                    voice: Self::voice_port_of(&i, mc),
                    name: i.name.clone(),
                    minecraft: mc,
                }
            })
            .collect()
    }

    /// `SetVoiceConfig`: solo direcciones de túneles de voz detectados y puertos libres.
    pub(crate) fn set_voice_config(
        self: &Arc<Self>,
        client: &str,
        inst: &Instance,
        voice_host: Option<String>,
        port: Option<u16>,
    ) -> Result<ConnectionInfo, ApiError> {
        if voice_host.is_none() && port.is_none() {
            return Err(api(ErrorCode::BadRequest, "No hay cambios que guardar."));
        }
        if vc::find_jar(&inst.root).is_none() {
            return Err(api(
                ErrorCode::InvalidState,
                "Este servidor no tiene Simple Voice Chat.",
            ));
        }
        let Some((cfg, text)) = vc::read_config(&inst.root) else {
            return Err(api(
                ErrorCode::InvalidState,
                "Todavía no existe la configuración de la voz. Enciende el servidor una vez con el mod.",
            ));
        };
        let minecraft = Self::server_port(inst);
        if let Some(p) = port {
            if p < 1024 {
                return Err(api(ErrorCode::BadRequest, "Usa un puerto entre 1024 y 65535."));
            }
            if Some(p) == minecraft {
                return Err(api(
                    ErrorCode::BadRequest,
                    "Ese es el puerto de Minecraft; la voz necesita uno propio.",
                ));
            }
            if let Some(o) = self
                .others_ports(inst)
                .into_iter()
                .find(|o| o.minecraft == Some(p) || o.voice == Some(p))
            {
                return Err(api(
                    ErrorCode::Conflict,
                    format!("El puerto {p} ya lo usa «{}».", o.name),
                ));
            }
        }
        if let Some(h) = voice_host.as_deref().map(str::trim).filter(|h| !h.is_empty()) {
            // Se valida contra los túneles del puerto que tendrá la voz.
            let voice_port = port.or_else(|| cfg.effective_port(minecraft));
            let agent = self.playit_agent_status();
            let ok = agent.is_some_and(|a| {
                a.tunnels.iter().any(|t| {
                    !t.is_disabled
                        && t.local_port() == voice_port
                        && voice_port != minecraft
                        && has_port(&t.display_address)
                        && t.display_address.eq_ignore_ascii_case(h)
                })
            });
            if !ok {
                return Err(api(
                    ErrorCode::BadRequest,
                    "Esa dirección no es la de un túnel de voz activo de este servidor.",
                ));
            }
        }
        let new_text = vc::apply(&text, voice_host.as_deref(), port);
        let ex = self.explorer(inst)?;
        // write_text guarda antes una copia del archivo en el historial de MercHosting.
        ex.write_text(&cfg.rel, &new_text, None).map_err(admin_err)?;
        let mut detail = vec![];
        if let Some(h) = &voice_host {
            detail.push(format!("voice_host={}", h.trim()));
        }
        if let Some(p) = port {
            detail.push(format!("port={p}"));
        }
        self.audit(client, Some(inst.id), "voicechat.set", &detail.join(", "));
        Ok(self.connection(inst))
    }
}

fn client_mod() -> VoiceProblem {
    problem(
        "client_mod",
        "info",
        "Cada jugador necesita Simple Voice Chat instalado en su Minecraft para hablar y oír.",
    )
}
