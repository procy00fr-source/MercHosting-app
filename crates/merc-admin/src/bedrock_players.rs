//! Jugadores de un servidor Bedrock (Bedrock Dedicated Server): la lista blanca
//! (`allowlist.json`) y los permisos (`permissions.json`).
//!
//! Formatos (los del servidor oficial):
//! - `allowlist.json`: `[{"ignoresPlayerLimit": false, "name": "Gamertag", "xuid": "123"}]`. El
//!   servidor rellena `xuid` la primera vez que entra ese jugador.
//! - `permissions.json`: `[{"permission": "operator"|"member"|"visitor", "xuid": "123"}]`. Va por
//!   XUID (el identificador de Xbox), no por nombre. Quien no está usa
//!   `default-player-permission-level`.
//!
//! Se conservan los campos y entradas que MercHosting no entiende, y se escribe de forma atómica
//! (archivo temporal + renombrar). Con el servidor encendido, después de escribir hay que pedirle
//! que relea: `allowlist reload` / `permission reload`.

use crate::AdminError;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::Path;

pub const ALLOWLIST: &str = "allowlist.json";
pub const PERMISSIONS: &str = "permissions.json";
/// Si existe, el servidor lo usa EN LUGAR de `allowlist.json` (nombre antiguo).
pub const LEGACY_WHITELIST: &str = "whitelist.json";

/// Como mucho tantas entradas por archivo (para no cargar archivos absurdos).
const MAX_ENTRIES: usize = 5000;
const MAX_FILE: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllowEntry {
    pub name: String,
    pub xuid: Option<String>,
    pub ignores_player_limit: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Operator,
    Member,
    Visitor,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Operator => "operator",
            Level::Member => "member",
            Level::Visitor => "visitor",
        }
    }

    pub fn parse(s: &str) -> Option<Level> {
        match s.trim() {
            "operator" => Some(Level::Operator),
            "member" => Some(Level::Member),
            "visitor" => Some(Level::Visitor),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermEntry {
    pub xuid: String,
    pub level: Level,
}

/// Gamertag aceptable para guardarlo y para usarlo entre comillas en la consola: 1 a 32
/// caracteres, letras, números, espacios, `_`, `-`, `#` y `.`; sin comillas ni saltos.
pub fn valid_gamertag(name: &str) -> bool {
    let n = name.trim();
    !n.is_empty()
        && n.chars().count() <= 32
        && n.chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '_' | '-' | '#' | '.'))
}

/// XUID: solo dígitos (es un número de 64 bits).
pub fn valid_xuid(x: &str) -> bool {
    !x.is_empty() && x.len() <= 20 && x.chars().all(|c| c.is_ascii_digit())
}

fn read_array(path: &Path) -> Result<Vec<Value>, AdminError> {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(AdminError::Io(e.to_string())),
    };
    if meta.len() > MAX_FILE {
        return Err(AdminError::Invalid(format!(
            "{} es demasiado grande",
            path.file_name().unwrap_or_default().to_string_lossy()
        )));
    }
    let text = std::fs::read_to_string(path).map_err(|e| AdminError::Io(e.to_string()))?;
    let text = text.trim_start_matches('\u{feff}');
    if text.trim().is_empty() {
        return Ok(vec![]);
    }
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Array(a)) if a.len() <= MAX_ENTRIES => Ok(a),
        Ok(Value::Array(_)) => Err(AdminError::Invalid("demasiadas entradas".into())),
        _ => Err(AdminError::Invalid(format!(
            "{} no es una lista JSON válida; corrígelo en Archivos antes de cambiarlo aquí",
            path.file_name().unwrap_or_default().to_string_lossy()
        ))),
    }
}

fn write_array(path: &Path, items: &[Value]) -> Result<(), AdminError> {
    let text = serde_json::to_string_pretty(items).map_err(|e| AdminError::Io(e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, format!("{text}\n")).map_err(|e| AdminError::Io(e.to_string()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        AdminError::Io(e.to_string())
    })
}

fn allow_of(v: &Value) -> Option<AllowEntry> {
    let o = v.as_object()?;
    let name = o.get("name")?.as_str()?.to_string();
    Some(AllowEntry {
        name,
        xuid: o
            .get("xuid")
            .and_then(|x| x.as_str())
            .filter(|x| !x.is_empty())
            .map(String::from),
        ignores_player_limit: o.get("ignoresPlayerLimit").and_then(|b| b.as_bool()).unwrap_or(false),
    })
}

/// Entradas de `allowlist.json` (las que no tienen nombre se ignoran al mostrar, pero se conservan).
pub fn read_allowlist(root: &Path) -> Result<Vec<AllowEntry>, AdminError> {
    Ok(read_array(&root.join(ALLOWLIST))?.iter().filter_map(allow_of).collect())
}

/// Entradas de `permissions.json` con un nivel conocido.
pub fn read_permissions(root: &Path) -> Result<Vec<PermEntry>, AdminError> {
    Ok(read_array(&root.join(PERMISSIONS))?
        .iter()
        .filter_map(|v| {
            let o = v.as_object()?;
            Some(PermEntry {
                xuid: o.get("xuid")?.as_str()?.to_string(),
                level: Level::parse(o.get("permission")?.as_str()?)?,
            })
        })
        .collect())
}

/// Cambio en la lista blanca.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AllowChange {
    Add { name: String, ignores_player_limit: bool },
    Remove { name: String },
    SetIgnoresLimit { name: String, value: bool },
}

fn same_name(v: &Value, name: &str) -> bool {
    v.get("name")
        .and_then(|n| n.as_str())
        .is_some_and(|n| n.trim().eq_ignore_ascii_case(name.trim()))
}

/// Aplica el cambio sobre `allowlist.json`, conservando campos y entradas desconocidos.
pub fn apply_allow(root: &Path, change: &AllowChange) -> Result<(), AdminError> {
    let path = root.join(ALLOWLIST);
    let mut items = read_array(&path)?;
    match change {
        AllowChange::Add {
            name,
            ignores_player_limit,
        } => {
            if !valid_gamertag(name) {
                return Err(AdminError::Invalid("gamertag no válido".into()));
            }
            if let Some(v) = items.iter_mut().find(|v| same_name(v, name)) {
                if let Some(o) = v.as_object_mut() {
                    o.insert("ignoresPlayerLimit".into(), Value::Bool(*ignores_player_limit));
                }
            } else {
                if items.len() >= MAX_ENTRIES {
                    return Err(AdminError::Invalid("la lista está llena".into()));
                }
                let mut o = Map::new();
                o.insert("ignoresPlayerLimit".into(), Value::Bool(*ignores_player_limit));
                o.insert("name".into(), Value::String(name.trim().to_string()));
                items.push(Value::Object(o));
            }
        }
        AllowChange::Remove { name } => {
            let before = items.len();
            items.retain(|v| !same_name(v, name));
            if items.len() == before {
                return Err(AdminError::NotFound(name.trim().to_string()));
            }
        }
        AllowChange::SetIgnoresLimit { name, value } => {
            let v = items
                .iter_mut()
                .find(|v| same_name(v, name))
                .ok_or_else(|| AdminError::NotFound(name.trim().to_string()))?;
            if let Some(o) = v.as_object_mut() {
                o.insert("ignoresPlayerLimit".into(), Value::Bool(*value));
            }
        }
    }
    write_array(&path, &items)
}

/// Pone el nivel de un XUID (`None` = quitarlo: vuelve al nivel por defecto del servidor).
pub fn set_permission(root: &Path, xuid: &str, level: Option<Level>) -> Result<(), AdminError> {
    if !valid_xuid(xuid) {
        return Err(AdminError::Invalid("XUID no válido".into()));
    }
    let path = root.join(PERMISSIONS);
    let mut items = read_array(&path)?;
    let is_it = |v: &Value| v.get("xuid").and_then(|x| x.as_str()) == Some(xuid);
    match level {
        Some(l) => {
            if let Some(v) = items.iter_mut().find(|v| is_it(v)) {
                if let Some(o) = v.as_object_mut() {
                    o.insert("permission".into(), Value::String(l.as_str().into()));
                }
            } else {
                if items.len() >= MAX_ENTRIES {
                    return Err(AdminError::Invalid("la lista está llena".into()));
                }
                let mut o = Map::new();
                o.insert("permission".into(), Value::String(l.as_str().into()));
                o.insert("xuid".into(), Value::String(xuid.into()));
                items.push(Value::Object(o));
            }
        }
        None => items.retain(|v| !is_it(v)),
    }
    write_array(&path, &items)
}

/// Comando para que el servidor encendido relea el archivo que se acaba de escribir.
pub const ALLOWLIST_RELOAD: &str = "allowlist reload";
pub const PERMISSION_RELOAD: &str = "permission reload";

/// `allowlist on` / `allowlist off`: solo cambia el servidor encendido (el archivo de ajustes se
/// cambia aparte, para que dure tras reiniciar).
pub fn allowlist_switch(on: bool) -> &'static str {
    if on {
        "allowlist on"
    } else {
        "allowlist off"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_xuids() {
        assert!(valid_gamertag("Steve Craft"));
        assert!(valid_gamertag("Gamer#1234"));
        assert!(!valid_gamertag("a\"; op b"));
        assert!(!valid_gamertag(""));
        assert!(!valid_gamertag(&"x".repeat(33)));
        assert!(valid_xuid("2535428000000000"));
        assert!(!valid_xuid("12a"));
        assert!(!valid_xuid(""));
    }

    #[test]
    fn allowlist_round_trip_keeps_unknown_fields() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join(ALLOWLIST),
            "\u{feff}[\n {\"ignoresPlayerLimit\": false, \"name\": \"Steve Craft\", \"xuid\": \"123\", \"futuro\": 7},\n {\"otra\": true}\n]",
        )
        .unwrap();
        let l = read_allowlist(d.path()).unwrap();
        assert_eq!(
            l,
            vec![AllowEntry {
                name: "Steve Craft".into(),
                xuid: Some("123".into()),
                ignores_player_limit: false
            }]
        );
        apply_allow(
            d.path(),
            &AllowChange::Add {
                name: " Alex ".into(),
                ignores_player_limit: true,
            },
        )
        .unwrap();
        apply_allow(
            d.path(),
            &AllowChange::SetIgnoresLimit {
                name: "steve craft".into(),
                value: true,
            },
        )
        .unwrap();
        let raw: Value = serde_json::from_str(&std::fs::read_to_string(d.path().join(ALLOWLIST)).unwrap()).unwrap();
        let a = raw.as_array().unwrap();
        assert_eq!(a.len(), 3);
        assert_eq!(a[0]["futuro"], 7);
        assert_eq!(a[0]["ignoresPlayerLimit"], true);
        assert_eq!(a[1]["otra"], true);
        assert_eq!(a[2]["name"], "Alex");
        assert!(!d.path().join("allowlist.json.tmp").exists());

        apply_allow(d.path(), &AllowChange::Remove { name: "ALEX".into() }).unwrap();
        assert_eq!(read_allowlist(d.path()).unwrap().len(), 1);
        assert!(matches!(
            apply_allow(d.path(), &AllowChange::Remove { name: "Nadie".into() }),
            Err(AdminError::NotFound(_))
        ));
        assert!(apply_allow(
            d.path(),
            &AllowChange::Add {
                name: "x\nop y".into(),
                ignores_player_limit: false
            }
        )
        .is_err());
    }

    #[test]
    fn missing_or_broken_files() {
        let d = tempfile::tempdir().unwrap();
        assert!(read_allowlist(d.path()).unwrap().is_empty());
        assert!(read_permissions(d.path()).unwrap().is_empty());
        std::fs::write(d.path().join(PERMISSIONS), "{no es json").unwrap();
        assert!(read_permissions(d.path()).is_err());
        // No se pisa un archivo roto: el usuario lo corrige primero.
        assert!(set_permission(d.path(), "1", Some(Level::Operator)).is_err());
        assert_eq!(
            std::fs::read_to_string(d.path().join(PERMISSIONS)).unwrap(),
            "{no es json"
        );
    }

    #[test]
    fn permissions_by_xuid() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join(PERMISSIONS),
            r#"[{"permission":"member","xuid":"1","nota":"x"},{"permission":"raro","xuid":"9"}]"#,
        )
        .unwrap();
        assert_eq!(
            read_permissions(d.path()).unwrap(),
            vec![PermEntry {
                xuid: "1".into(),
                level: Level::Member
            }]
        );
        set_permission(d.path(), "1", Some(Level::Operator)).unwrap();
        set_permission(d.path(), "2", Some(Level::Visitor)).unwrap();
        let raw: Value = serde_json::from_str(&std::fs::read_to_string(d.path().join(PERMISSIONS)).unwrap()).unwrap();
        assert_eq!(raw[0]["permission"], "operator");
        assert_eq!(raw[0]["nota"], "x");
        assert_eq!(raw[1]["permission"], "raro");
        assert_eq!(raw[2]["xuid"], "2");
        set_permission(d.path(), "1", None).unwrap();
        let p = read_permissions(d.path()).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].level, Level::Visitor);
        assert!(set_permission(d.path(), "abc", Some(Level::Operator)).is_err());
    }
}
