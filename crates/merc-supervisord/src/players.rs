//! Jugadores: más acciones con un jugador conectado (mensaje, modo de juego, teletransporte), el
//! historial de conexiones que ve MercHosting y notas privadas del dueño.

use super::{admin_err, api, now_unix, sup_err, Daemon};
use merc_admin::players::{self as pl, PlayerRecord};
use merc_core::{Instance, InstanceId, RuntimeState};
use merc_ipc::*;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

static LOCK: Mutex<()> = Mutex::new(());

impl Daemon {
    fn history_file(&self, id: InstanceId) -> PathBuf {
        self.store_dir(id).join("jugadores.json")
    }

    pub(crate) fn load_history(&self, id: InstanceId) -> Vec<PlayerRecord> {
        std::fs::read_to_string(self.history_file(id))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn save_history(&self, id: InstanceId, h: &[PlayerRecord]) {
        let f = self.history_file(id);
        if let Some(p) = f.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        let tmp = f.with_extension("json.tmp");
        if std::fs::write(&tmp, serde_json::to_string(h).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, &f);
        }
    }

    /// Llamado en cada vuelta de salud: compara los conectados con el historial.
    pub(crate) fn track_players(&self, inst: &Instance, state: RuntimeState) {
        let online: Vec<String> = if state == RuntimeState::Online {
            self.sup.metrics(inst.id).1
        } else {
            vec![]
        };
        let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let mut h = self.load_history(inst.id);
        if h.is_empty() && online.is_empty() {
            return;
        }
        let mut changed = pl::track_online(&mut h, &online, now_unix());
        if inst.kind == merc_core::detect::ServerKind::Bedrock {
            changed |= pl::remember_xuids(&mut h, &self.sup.player_xuids(inst.id));
        }
        if changed {
            self.save_history(inst.id, &h);
        }
    }

    fn history_sorted(&self, id: InstanceId) -> Vec<PlayerRecord> {
        let mut h = self.load_history(id);
        h.sort_by_key(|r| (r.online_since_unix.is_none(), std::cmp::Reverse(r.last_seen_unix)));
        h
    }

    pub(crate) fn handle_players(self: &Arc<Self>, client: &str, body: RequestBody) -> Result<ResponseBody, ApiError> {
        match body {
            RequestBody::PlayerCommand { id, name, command } => {
                self.instance(id)?;
                let cmd = pl::player_command(name.trim(), &command).map_err(admin_err)?;
                if self.sup.status(id).state != RuntimeState::Online {
                    return Err(api(
                        ErrorCode::InvalidState,
                        "Enciende el servidor: estas acciones las hace Minecraft.",
                    ));
                }
                if !self
                    .sup
                    .metrics(id)
                    .1
                    .iter()
                    .any(|n| n.eq_ignore_ascii_case(name.trim()))
                {
                    return Err(api(
                        ErrorCode::InvalidState,
                        format!("{} no está conectado ahora.", name.trim()),
                    ));
                }
                self.sup.send_command(id, &cmd).map_err(sup_err)?;
                // El texto de un mensaje privado no se guarda en la Actividad.
                let detail = match command {
                    pl::PlayerCommand::Message { .. } => format!("tell {} (mensaje)", name.trim()),
                    _ => cmd,
                };
                self.audit(client, Some(id), "player.command", &detail);
                Ok(ResponseBody::Ok)
            }
            RequestBody::PlayerHistory { id } => {
                self.instance(id)?;
                Ok(ResponseBody::PlayerHistory(self.history_sorted(id)))
            }
            RequestBody::SetPlayerNote { id, name, note } => {
                self.instance(id)?;
                if !pl::valid_player_name(name.trim()) {
                    return Err(api(ErrorCode::BadRequest, "Nombre de jugador no válido"));
                }
                let note = pl::validate_note(&note).map_err(admin_err)?;
                {
                    let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
                    let mut h = self.load_history(id);
                    match h.iter_mut().find(|r| r.name.eq_ignore_ascii_case(name.trim())) {
                        Some(r) => r.note = note,
                        None => h.push(PlayerRecord {
                            name: name.trim().to_string(),
                            note,
                            ..Default::default()
                        }),
                    }
                    self.save_history(id, &h);
                }
                self.audit(client, Some(id), "player.note", name.trim());
                Ok(ResponseBody::PlayerHistory(self.history_sorted(id)))
            }
            RequestBody::ClearPlayerHistory { id } => {
                self.instance(id)?;
                {
                    let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
                    let _ = std::fs::remove_file(self.history_file(id));
                }
                self.audit(client, Some(id), "player.history.clear", "");
                Ok(ResponseBody::PlayerHistory(vec![]))
            }
            _ => Err(api(ErrorCode::BadRequest, "Petición no válida")),
        }
    }
}
