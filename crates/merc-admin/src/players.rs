//! Jugadores: listas que guarda Minecraft (lista blanca, operadores, baneos) y los comandos
//! oficiales para cambiarlas. Los cambios se hacen siempre con el servidor encendido, a
//! través de su consola, para que Minecraft resuelva el UUID y escriba sus propios archivos.

use crate::AdminError;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ListedPlayer {
    pub name: String,
    pub uuid: Option<String>,
    /// Nivel de operador (1–4), solo en `ops.json`.
    pub level: Option<u8>,
    /// Motivo del baneo, solo en `banned-players.json`.
    pub reason: Option<String>,
    /// Caducidad del baneo tal como la escribe Minecraft ("forever" o fecha).
    pub expires: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PlayerLists {
    pub whitelist: Vec<ListedPlayer>,
    pub ops: Vec<ListedPlayer>,
    pub banned: Vec<ListedPlayer>,
    pub banned_ips: Vec<String>,
    /// Avisos de lectura (archivo dañado…); un archivo que falta simplemente da lista vacía.
    pub warnings: Vec<String>,
}

fn read_array(root: &Path, file: &str, warnings: &mut Vec<String>) -> Vec<serde_json::Value> {
    match std::fs::read(root.join(file)) {
        Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(serde_json::Value::Array(a)) => a,
            _ => {
                warnings.push(format!(
                    "No se pudo leer {file}: el archivo no tiene el formato esperado"
                ));
                vec![]
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
        Err(e) => {
            warnings.push(format!("No se pudo leer {file}: {e}"));
            vec![]
        }
    }
}

fn s(v: &serde_json::Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).map(str::to_string)
}

pub fn read_lists(root: &Path) -> PlayerLists {
    let mut warnings = vec![];
    let player = |v: &serde_json::Value| ListedPlayer {
        name: s(v, "name").unwrap_or_default(),
        uuid: s(v, "uuid"),
        level: v.get("level").and_then(|x| x.as_u64()).map(|n| n.min(4) as u8),
        reason: s(v, "reason"),
        expires: s(v, "expires"),
    };
    let mut lists = PlayerLists {
        whitelist: read_array(root, "whitelist.json", &mut warnings)
            .iter()
            .map(player)
            .collect(),
        ops: read_array(root, "ops.json", &mut warnings).iter().map(player).collect(),
        banned: read_array(root, "banned-players.json", &mut warnings)
            .iter()
            .map(player)
            .collect(),
        banned_ips: read_array(root, "banned-ips.json", &mut warnings)
            .iter()
            .filter_map(|v| s(v, "ip"))
            .collect(),
        warnings: vec![],
    };
    for l in [&mut lists.whitelist, &mut lists.ops, &mut lists.banned] {
        l.retain(|p| !p.name.is_empty());
        l.sort_by_key(|p| p.name.to_lowercase());
    }
    lists.warnings = warnings;
    lists
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerAction {
    WhitelistAdd,
    WhitelistRemove,
    Op,
    Deop,
    Kick,
    Ban,
    Pardon,
}

impl PlayerAction {
    /// Acciones que conviene confirmar en la interfaz (afectan a otra persona o dan poder).
    pub fn needs_confirmation(self) -> bool {
        matches!(self, PlayerAction::Op | PlayerAction::Kick | PlayerAction::Ban)
    }
}

/// Nombre de jugador de Java Edition: 3–16 caracteres, letras, números y `_`.
/// (Algunas cuentas antiguas tienen 1–2 caracteres; se aceptan desde 1.)
pub fn valid_player_name(name: &str) -> bool {
    (1..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Construye el comando oficial de Minecraft. Nunca se ejecuta nada del sistema operativo.
pub fn command_for(action: PlayerAction, name: &str, reason: Option<&str>) -> Result<String, AdminError> {
    if !valid_player_name(name) {
        return Err(AdminError::Invalid(format!(
            "«{name}» no es un nombre de jugador válido (letras, números y _; máximo 16)"
        )));
    }
    let reason = reason.map(str::trim).filter(|r| !r.is_empty());
    if let Some(r) = reason {
        if r.chars().count() > 120 || r.chars().any(|c| c.is_control()) {
            return Err(AdminError::Invalid(
                "el motivo debe ser una sola línea de hasta 120 caracteres".into(),
            ));
        }
    }
    let with_reason = |base: String| match reason {
        Some(r) => format!("{base} {r}"),
        None => base,
    };
    Ok(match action {
        PlayerAction::WhitelistAdd => format!("whitelist add {name}"),
        PlayerAction::WhitelistRemove => format!("whitelist remove {name}"),
        PlayerAction::Op => format!("op {name}"),
        PlayerAction::Deop => format!("deop {name}"),
        PlayerAction::Kick => with_reason(format!("kick {name}")),
        PlayerAction::Ban => with_reason(format!("ban {name}")),
        PlayerAction::Pardon => format!("pardon {name}"),
    })
}

// ---------- Más acciones sobre un jugador conectado ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameMode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

/// Acciones con un jugador conectado (comandos oficiales de Minecraft).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlayerCommand {
    /// Mensaje privado (`tell`).
    Message {
        text: String,
    },
    GameMode {
        mode: GameMode,
    },
    /// Llevarlo junto a otro jugador.
    TeleportTo {
        target: String,
    },
    /// Llevarlo a unas coordenadas (de bloque).
    TeleportCoords {
        x: i32,
        y: i32,
        z: i32,
    },
}

/// Límite del mundo de Minecraft (horizontal) y altura razonable.
const WORLD_LIMIT: i32 = 30_000_000;
const HEIGHT_LIMIT: i32 = 4096;

pub fn player_command(name: &str, cmd: &PlayerCommand) -> Result<String, AdminError> {
    if !valid_player_name(name) {
        return Err(AdminError::Invalid(format!(
            "«{name}» no es un nombre de jugador válido"
        )));
    }
    Ok(match cmd {
        PlayerCommand::Message { text } => {
            let t = text.trim();
            if t.is_empty() || t.chars().count() > 200 || t.chars().any(|c| c.is_control()) {
                return Err(AdminError::Invalid(
                    "el mensaje debe ser una línea de 1 a 200 caracteres".into(),
                ));
            }
            format!("tell {name} {t}")
        }
        PlayerCommand::GameMode { mode } => {
            let m = match mode {
                GameMode::Survival => "survival",
                GameMode::Creative => "creative",
                GameMode::Adventure => "adventure",
                GameMode::Spectator => "spectator",
            };
            format!("gamemode {m} {name}")
        }
        PlayerCommand::TeleportTo { target } => {
            if !valid_player_name(target.trim()) {
                return Err(AdminError::Invalid(format!(
                    "«{target}» no es un nombre de jugador válido"
                )));
            }
            format!("tp {name} {}", target.trim())
        }
        PlayerCommand::TeleportCoords { x, y, z } => {
            if x.abs() > WORLD_LIMIT || z.abs() > WORLD_LIMIT || y.abs() > HEIGHT_LIMIT {
                return Err(AdminError::Invalid("coordenadas fuera del mundo".into()));
            }
            format!("tp {name} {x} {y} {z}")
        }
    })
}

// ---------- Historial de conexiones ----------

/// Lo que MercHosting vio de un jugador (desde que lo vigila; no lee registros antiguos).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PlayerRecord {
    pub name: String,
    pub first_seen_unix: i64,
    pub last_seen_unix: i64,
    pub sessions: u32,
    /// Segundos jugados en sesiones ya cerradas.
    pub total_secs: u64,
    /// Si está conectado ahora: desde cuándo.
    pub online_since_unix: Option<i64>,
    /// Nota privada del dueño (no la ve el jugador).
    #[serde(default)]
    pub note: Option<String>,
    /// Bedrock: XUID (identificador de Xbox) que dio el servidor al entrar. Java: siempre `None`.
    #[serde(default)]
    pub xuid: Option<String>,
}

/// Como mucho tantos jugadores en el historial (los más antiguos sin nota se descartan).
pub const MAX_HISTORY: usize = 1000;

/// Actualiza el historial con los conectados ahora. Devuelve si cambió algo.
pub fn track_online(history: &mut Vec<PlayerRecord>, online: &[String], now: i64) -> bool {
    let mut changed = false;
    for name in online {
        match history.iter_mut().find(|r| r.name.eq_ignore_ascii_case(name)) {
            Some(r) if r.online_since_unix.is_none() => {
                r.online_since_unix = Some(now);
                r.sessions += 1;
                r.last_seen_unix = now;
                changed = true;
            }
            Some(_) => {}
            None => {
                history.push(PlayerRecord {
                    name: name.clone(),
                    first_seen_unix: now,
                    last_seen_unix: now,
                    sessions: 1,
                    total_secs: 0,
                    online_since_unix: Some(now),
                    note: None,
                    xuid: None,
                });
                changed = true;
            }
        }
    }
    for r in history.iter_mut() {
        if let Some(since) = r.online_since_unix {
            if !online.iter().any(|n| n.eq_ignore_ascii_case(&r.name)) {
                r.total_secs += (now - since).max(0) as u64;
                r.last_seen_unix = now;
                r.online_since_unix = None;
                changed = true;
            }
        }
    }
    if history.len() > MAX_HISTORY {
        history.sort_by_key(|r| (r.note.is_none(), std::cmp::Reverse(r.last_seen_unix)));
        history.truncate(MAX_HISTORY);
        changed = true;
    }
    changed
}

/// Guarda el XUID de quienes ya lo dieron (Bedrock). Devuelve si cambió algo.
pub fn remember_xuids(history: &mut [PlayerRecord], xuids: &[(String, String)]) -> bool {
    let mut changed = false;
    for (name, xuid) in xuids {
        if let Some(r) = history.iter_mut().find(|r| r.name.eq_ignore_ascii_case(name)) {
            if r.xuid.as_deref() != Some(xuid.as_str()) {
                r.xuid = Some(xuid.clone());
                changed = true;
            }
        }
    }
    changed
}

pub fn validate_note(note: &str) -> Result<Option<String>, AdminError> {
    let n = note.trim();
    if n.chars().count() > 500 {
        return Err(AdminError::Invalid(
            "la nota puede tener como máximo 500 caracteres".into(),
        ));
    }
    Ok((!n.is_empty()).then(|| n.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_commands_are_validated() {
        assert_eq!(
            player_command("Alex", &PlayerCommand::Message { text: " hola ".into() }).unwrap(),
            "tell Alex hola"
        );
        assert!(player_command(
            "Alex",
            &PlayerCommand::Message {
                text: "a\nop Bob".into()
            }
        )
        .is_err());
        assert_eq!(
            player_command(
                "Alex",
                &PlayerCommand::GameMode {
                    mode: GameMode::Creative
                }
            )
            .unwrap(),
            "gamemode creative Alex"
        );
        assert_eq!(
            player_command("Alex", &PlayerCommand::TeleportTo { target: "Steve".into() }).unwrap(),
            "tp Alex Steve"
        );
        assert!(player_command("Alex", &PlayerCommand::TeleportTo { target: "@a".into() }).is_err());
        assert_eq!(
            player_command("Alex", &PlayerCommand::TeleportCoords { x: 10, y: 64, z: -3 }).unwrap(),
            "tp Alex 10 64 -3"
        );
        assert!(player_command(
            "Alex",
            &PlayerCommand::TeleportCoords {
                x: 40_000_000,
                y: 0,
                z: 0
            }
        )
        .is_err());
        assert!(player_command("Alex", &PlayerCommand::TeleportCoords { x: 0, y: 9000, z: 0 }).is_err());
        assert!(player_command(
            "@e",
            &PlayerCommand::GameMode {
                mode: GameMode::Survival
            }
        )
        .is_err());
    }

    #[test]
    fn history_counts_sessions_and_time() {
        let mut h = vec![];
        assert!(track_online(&mut h, &["Alex".into()], 100));
        assert!(!track_online(&mut h, &["Alex".into()], 105));
        assert!(track_online(&mut h, &[], 160));
        assert_eq!(
            (
                h[0].sessions,
                h[0].total_secs,
                h[0].last_seen_unix,
                h[0].online_since_unix
            ),
            (1, 60, 160, None)
        );
        track_online(&mut h, &["alex".into(), "Steve".into()], 200);
        track_online(&mut h, &[], 230);
        assert_eq!((h[0].sessions, h[0].total_secs), (2, 90));
        assert_eq!((h[1].name.as_str(), h[1].sessions, h[1].total_secs), ("Steve", 1, 30));
        assert_eq!(validate_note("  ").unwrap(), None);
        assert!(validate_note(&"x".repeat(501)).is_err());
    }

    #[test]
    fn reads_minecraft_json_files() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("ops.json"),
            r#"[{"uuid":"1-2","name":"Steve","level":4,"bypassesPlayerLimit":false}]"#,
        )
        .unwrap();
        std::fs::write(
            d.path().join("whitelist.json"),
            r#"[{"uuid":"b","name":"luna"},{"uuid":"a","name":"Alex"}]"#,
        )
        .unwrap();
        std::fs::write(
            d.path().join("banned-players.json"),
            r#"[{"uuid":"c","name":"Griefer","created":"x","source":"Server","expires":"forever","reason":"Destruir casas"}]"#,
        )
        .unwrap();
        std::fs::write(d.path().join("banned-ips.json"), "esto no es json").unwrap();
        let l = read_lists(d.path());
        assert_eq!(l.ops[0].level, Some(4));
        assert_eq!(
            l.whitelist.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["Alex", "luna"]
        );
        assert_eq!(l.banned[0].reason.as_deref(), Some("Destruir casas"));
        assert_eq!(l.banned_ips, Vec::<String>::new());
        assert_eq!(l.warnings.len(), 1);
    }

    #[test]
    fn commands_are_validated() {
        assert_eq!(command_for(PlayerAction::Op, "Steve", None).unwrap(), "op Steve");
        assert_eq!(
            command_for(PlayerAction::Ban, "Griefer", Some(" destruir casas ")).unwrap(),
            "ban Griefer destruir casas"
        );
        assert_eq!(
            command_for(PlayerAction::WhitelistAdd, "a_b", Some("ignorado")).unwrap(),
            "whitelist add a_b"
        );
        assert!(command_for(PlayerAction::Op, "Steve; stop", None).is_err());
        assert!(command_for(PlayerAction::Kick, "Steve", Some("a\nstop")).is_err());
        assert!(command_for(PlayerAction::Op, "nombre_demasiado_largo", None).is_err());
    }

    #[test]
    fn xuids_are_remembered_for_known_players() {
        let mut h = vec![];
        track_online(&mut h, &["Steve Craft".to_string()], 100);
        assert!(remember_xuids(&mut h, &[("steve craft".into(), "25354".into())]));
        assert_eq!(h[0].xuid.as_deref(), Some("25354"));
        assert!(!remember_xuids(&mut h, &[("Steve Craft".into(), "25354".into())]));
        // Alguien que no está en el historial no se inventa.
        assert!(!remember_xuids(&mut h, &[("Otro".into(), "1".into())]));
        assert_eq!(h.len(), 1);
        // Un historial guardado antes (sin xuid) se sigue leyendo.
        let old: Vec<PlayerRecord> =
            serde_json::from_str(r#"[{"name":"A","first_seen_unix":1,"last_seen_unix":2,"sessions":1,"total_secs":0,"online_since_unix":null}]"#).unwrap();
        assert_eq!(old[0].xuid, None);
    }
}
