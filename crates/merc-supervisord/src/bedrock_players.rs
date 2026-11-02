//! Jugadores de servidores Bedrock: lista blanca y permisos (ver `merc_admin::bedrock_players`).
//!
//! Siempre se escribe el archivo (de forma atómica, conservando lo desconocido) y, si el servidor
//! está encendido, se le pide que lo relea con `allowlist reload` / `permission reload`. Así el
//! cambio vale ahora y después de reiniciar, esté o no conectado el jugador.

use super::{admin_err, api, sup_err, Daemon};
use merc_admin::bedrock_players::{self as bp, AllowChange};
use merc_core::detect::ServerKind;
use merc_core::{Instance, InstanceId, RuntimeState};
use merc_ipc::*;
use std::collections::BTreeMap;
use std::sync::Arc;

impl Daemon {
    fn bedrock_players_instance(&self, id: InstanceId) -> Result<Instance, ApiError> {
        let inst = self.instance(id)?;
        if inst.kind != ServerKind::Bedrock {
            return Err(api(
                ErrorCode::BadRequest,
                "Esta pantalla es para servidores Bedrock; en Java se usa la de siempre.",
            ));
        }
        Ok(inst)
    }

    fn bedrock_players_info(&self, inst: &Instance) -> Result<BedrockPlayersInfo, ApiError> {
        let props = std::fs::read_to_string(inst.root.join("server.properties")).unwrap_or_default();
        let doc = merc_admin::props::PropsDoc::parse(&props);
        let flag = |k: &str, default: bool| doc.get(k).map(|v| v.trim() == "true").unwrap_or(default);
        let allowlist = bp::read_allowlist(&inst.root).map_err(admin_err)?;
        let perms = bp::read_permissions(&inst.root).map_err(admin_err)?;
        let running = self.sup.status(inst.id).state == RuntimeState::Online;
        let online: Vec<String> = if running { self.sup.metrics(inst.id).1 } else { vec![] };

        // Gamertag → XUID: lo que vio MercHosting al entrar cada jugador y lo que el servidor
        // anotó en la lista blanca.
        let mut known: BTreeMap<String, BedrockKnownPlayer> = BTreeMap::new();
        for r in self.load_history(inst.id) {
            if let Some(x) = r.xuid.filter(|x| bp::valid_xuid(x)) {
                known.insert(
                    x.clone(),
                    BedrockKnownPlayer {
                        online: online.iter().any(|n| n.eq_ignore_ascii_case(&r.name)),
                        name: r.name,
                        xuid: x,
                        last_seen_unix: r.last_seen_unix,
                    },
                );
            }
        }
        for (name, x) in self.sup.player_xuids(inst.id) {
            known.entry(x.clone()).or_insert(BedrockKnownPlayer {
                online: online.iter().any(|n| n.eq_ignore_ascii_case(&name)),
                name,
                xuid: x,
                last_seen_unix: 0,
            });
        }
        for a in &allowlist {
            if let Some(x) = a.xuid.as_ref().filter(|x| bp::valid_xuid(x)) {
                known.entry(x.clone()).or_insert(BedrockKnownPlayer {
                    online: online.iter().any(|n| n.eq_ignore_ascii_case(&a.name)),
                    name: a.name.clone(),
                    xuid: x.clone(),
                    last_seen_unix: 0,
                });
            }
        }
        let permissions = perms
            .into_iter()
            .map(|p| BedrockPermRow {
                name: known.get(&p.xuid).map(|k| k.name.clone()),
                xuid: p.xuid,
                level: p.level,
            })
            .collect();
        let mut known: Vec<BedrockKnownPlayer> = known.into_values().collect();
        known.sort_by_key(|k| (!k.online, std::cmp::Reverse(k.last_seen_unix), k.name.to_lowercase()));
        Ok(BedrockPlayersInfo {
            allow_list_enabled: flag("allow-list", false),
            online_mode: flag("online-mode", true),
            default_permission: doc
                .get("default-player-permission-level")
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "member".into()),
            legacy_whitelist: inst.root.join(bp::LEGACY_WHITELIST).is_file(),
            running,
            allowlist,
            permissions,
            known,
            online,
        })
    }

    /// Con el servidor en línea, le manda el comando (el archivo ya quedó escrito).
    fn bedrock_tell(&self, id: InstanceId, cmd: &str) -> Result<(), ApiError> {
        if self.sup.status(id).state == RuntimeState::Online {
            self.sup.send_command(id, cmd).map_err(sup_err)?;
        }
        Ok(())
    }

    pub(crate) fn handle_bedrock_players(
        self: &Arc<Self>,
        client: &str,
        body: RequestBody,
    ) -> Result<ResponseBody, ApiError> {
        use RequestBody as R;
        let inst = match &body {
            R::BedrockPlayers { id }
            | R::BedrockAllowlist { id, .. }
            | R::BedrockAllowlistEnabled { id, .. }
            | R::BedrockPermission { id, .. } => self.bedrock_players_instance(*id)?,
            _ => return Err(api(ErrorCode::BadRequest, "Petición no válida")),
        };
        let id = inst.id;
        match body {
            R::BedrockPlayers { .. } => {}
            R::BedrockAllowlist { change, .. } => {
                bp::apply_allow(&inst.root, &change).map_err(admin_err)?;
                self.bedrock_tell(id, bp::ALLOWLIST_RELOAD)?;
                let detail = match &change {
                    AllowChange::Add { name, .. } => format!("añadido {}", name.trim()),
                    AllowChange::Remove { name } => format!("quitado {}", name.trim()),
                    AllowChange::SetIgnoresLimit { name, value } => {
                        format!("{}: ignora el límite = {value}", name.trim())
                    }
                };
                self.audit(client, Some(id), "bedrock.allowlist", &detail);
            }
            R::BedrockAllowlistEnabled { enabled, .. } => {
                let ex = self.explorer(&inst)?;
                let current = match ex.read_text("server.properties") {
                    Ok(f) => f.content,
                    Err(merc_admin::AdminError::NotFound(_)) => String::new(),
                    Err(e) => return Err(admin_err(e)),
                };
                let mut changes = BTreeMap::new();
                changes.insert("allow-list".to_string(), enabled.to_string());
                let text = merc_admin::props::apply_changes_for(&current, &changes, true).map_err(admin_err)?;
                ex.write_text("server.properties", &text, None).map_err(admin_err)?;
                self.bedrock_tell(id, bp::allowlist_switch(enabled))?;
                self.audit(
                    client,
                    Some(id),
                    "bedrock.allowlist.enabled",
                    if enabled { "activada" } else { "desactivada" },
                );
            }
            R::BedrockPermission { xuid, level, .. } => {
                let xuid = xuid.trim().to_string();
                bp::set_permission(&inst.root, &xuid, level).map_err(admin_err)?;
                self.bedrock_tell(id, bp::PERMISSION_RELOAD)?;
                self.audit(
                    client,
                    Some(id),
                    "bedrock.permission",
                    &format!("{xuid}: {}", level.map(|l| l.as_str()).unwrap_or("por defecto")),
                );
            }
            _ => unreachable!(),
        }
        Ok(ResponseBody::BedrockPlayers(Box::new(
            self.bedrock_players_info(&inst)?,
        )))
    }
}
