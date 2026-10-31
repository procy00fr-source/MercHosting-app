//! Reglas del juego (`/gamerule`). Se leen y cambian con el comando oficial, con el servidor
//! encendido. Desde Java 1.21.11 (snapshot 25w44a) las reglas se renombraron a `snake_case` y
//! algunas cambiaron de nombre del todo: solo se incluyen reglas cuya equivalencia entre el
//! nombre antiguo y el nuevo está documentada, con el mismo significado en ambos.

use crate::AdminError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuleKind {
    Bool,
    Int { min: i64, max: i64 },
}

#[derive(Debug, Clone, Copy)]
pub struct RuleDef {
    /// Identificador estable que usa la interfaz (el nombre moderno).
    pub key: &'static str,
    /// Nombre antes de 1.21.11.
    pub legacy: &'static str,
    pub kind: RuleKind,
}

pub const RULES: [RuleDef; 21] = [
    RuleDef {
        key: "keep_inventory",
        legacy: "keepInventory",
        kind: RuleKind::Bool,
    },
    RuleDef {
        key: "mob_griefing",
        legacy: "mobGriefing",
        kind: RuleKind::Bool,
    },
    RuleDef {
        key: "advance_time",
        legacy: "doDaylightCycle",
        kind: RuleKind::Bool,
    },
    RuleDef {
        key: "natural_health_regeneration",
        legacy: "naturalRegeneration",
        kind: RuleKind::Bool,
    },
    RuleDef {
        key: "immediate_respawn",
        legacy: "doImmediateRespawn",
        kind: RuleKind::Bool,
    },
    RuleDef {
        key: "fall_damage",
        legacy: "fallDamage",
        kind: RuleKind::Bool,
    },
    RuleDef {
        key: "fire_damage",
        legacy: "fireDamage",
        kind: RuleKind::Bool,
    },
    RuleDef {
        key: "drowning_damage",
        legacy: "drowningDamage",
        kind: RuleKind::Bool,
    },
    RuleDef {
        key: "spawn_phantoms",
        legacy: "doInsomnia",
        kind: RuleKind::Bool,
    },
    RuleDef {
        key: "show_advancement_messages",
        legacy: "announceAdvancements",
        kind: RuleKind::Bool,
    },
    RuleDef {
        key: "advance_weather",
        legacy: "doWeatherCycle",
        kind: RuleKind::Bool,
    },
    RuleDef {
        key: "players_sleeping_percentage",
        legacy: "playersSleepingPercentage",
        kind: RuleKind::Int { min: 0, max: 100 },
    },
    RuleDef {
        key: "pvp",
        legacy: "pvp",
        kind: RuleKind::Bool,
    },
    RuleDef {
        key: "freeze_damage",
        legacy: "freezeDamage",
        kind: RuleKind::Bool,
    },
    RuleDef {
        key: "spawn_mobs",
        legacy: "doMobSpawning",
        kind: RuleKind::Bool,
    },
    RuleDef {
        key: "spawn_patrols",
        legacy: "doPatrolSpawning",
        kind: RuleKind::Bool,
    },
    RuleDef {
        key: "spawn_wandering_traders",
        legacy: "doTraderSpawning",
        kind: RuleKind::Bool,
    },
    RuleDef {
        key: "mob_drops",
        legacy: "doMobLoot",
        kind: RuleKind::Bool,
    },
    RuleDef {
        key: "block_drops",
        legacy: "doTileDrops",
        kind: RuleKind::Bool,
    },
    RuleDef {
        key: "show_death_messages",
        legacy: "showDeathMessages",
        kind: RuleKind::Bool,
    },
    RuleDef {
        key: "random_tick_speed",
        legacy: "randomTickSpeed",
        kind: RuleKind::Int { min: 0, max: 4096 },
    },
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleValue {
    pub key: String,
    /// Nombre que entiende esta versión del servidor.
    pub name: String,
    pub kind: RuleKind,
    /// Valor que respondió el servidor; `None` si no respondió (versión o mod distinto).
    pub value: Option<String>,
}

/// ¿Esta versión usa los nombres nuevos (1.21.11 o posterior, o numeración por año)?
pub fn uses_new_names(mc: Option<&str>) -> bool {
    let Some(mc) = mc else { return true };
    let parts: Vec<u32> = mc.split('.').map_while(|p| p.parse().ok()).collect();
    match parts.as_slice() {
        [1, minor, rest @ ..] => *minor > 21 || (*minor == 21 && rest.first().copied().unwrap_or(0) >= 11),
        [year, ..] => *year >= 26,
        _ => true,
    }
}

pub fn rule_name(def: &RuleDef, mc: Option<&str>) -> &'static str {
    if uses_new_names(mc) {
        def.key
    } else {
        def.legacy
    }
}

pub fn find(key: &str) -> Result<&'static RuleDef, AdminError> {
    RULES
        .iter()
        .find(|r| r.key == key)
        .ok_or_else(|| AdminError::Invalid(format!("regla desconocida: {key}")))
}

/// Comando para consultar una regla.
pub fn query_command(def: &RuleDef, mc: Option<&str>) -> String {
    format!("gamerule {}", rule_name(def, mc))
}

/// Comando validado para cambiar una regla.
pub fn set_command(key: &str, value: &str, mc: Option<&str>) -> Result<String, AdminError> {
    let def = find(key)?;
    let ok = match def.kind {
        RuleKind::Bool => value == "true" || value == "false",
        RuleKind::Int { min, max } => value.parse::<i64>().is_ok_and(|n| (min..=max).contains(&n)),
    };
    if !ok {
        return Err(AdminError::Invalid(format!("valor no válido para {key}")));
    }
    Ok(format!("gamerule {} {value}", rule_name(def, mc)))
}

/// Lee la respuesta del servidor: `Gamerule keepInventory is currently set to: false`
/// (en versiones nuevas el nombre puede llevar `minecraft:`).
pub fn parse_response(line: &str) -> Option<(String, String)> {
    let re = regex::Regex::new(r"Gamerule (?:minecraft:)?([A-Za-z_]+) is (?:currently|now) set to: (\S+)").unwrap();
    let c = re.captures(line)?;
    Some((c[1].to_string(), c[2].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_by_version() {
        assert!(!uses_new_names(Some("1.21.1")));
        assert!(!uses_new_names(Some("1.20.1")));
        assert!(uses_new_names(Some("1.21.11")));
        assert!(uses_new_names(Some("26.1.2")));
        let keep = find("keep_inventory").unwrap();
        assert_eq!(query_command(keep, Some("1.21.1")), "gamerule keepInventory");
        assert_eq!(query_command(keep, Some("26.3")), "gamerule keep_inventory");
        assert_eq!(
            set_command("advance_time", "false", Some("1.20.1")).unwrap(),
            "gamerule doDaylightCycle false"
        );
        assert!(set_command("keep_inventory", "si", None).is_err());
        assert!(set_command("random_tick_speed", "5000", None).is_err());
        assert!(set_command("keep_inventory; stop", "true", None).is_err());
    }

    #[test]
    fn parses_server_output() {
        assert_eq!(
            parse_response("[12:00:00] [Server thread/INFO]: Gamerule keepInventory is currently set to: false"),
            Some(("keepInventory".into(), "false".into()))
        );
        assert_eq!(
            parse_response("[Server thread/INFO]: Gamerule minecraft:random_tick_speed is now set to: 3"),
            Some(("random_tick_speed".into(), "3".into()))
        );
        assert_eq!(parse_response("Alex joined the game"), None);
    }
}
