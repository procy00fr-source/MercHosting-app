//! LuckPerms: detectar el plugin o mod, leer sus datos y armar cambios seguros.
//!
//! Lectura: MercHosting nunca abre la base de datos de LuckPerms (por defecto es H2, un
//! formato binario que además está en uso). Le pide a LuckPerms `luckperms export <nombre>`,
//! que escribe un JSON comprimido con gzip en su carpeta de datos, y lo lee. Ese formato lo
//! define el `Exporter` de LuckPerms (`metadata`, `groups`, `tracks`, `users`) y es el mismo
//! que acepta `luckperms import`, así que también sirve como copia de seguridad.
//!
//! Cambios: solo los de [`LpChange`], cada uno traducido a un único comando de LuckPerms con
//! nombres y nodos validados. Nunca texto libre.

use crate::AdminError;
use merc_core::detect::ServerKind;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Comando base. `luckperms` existe en todas las plataformas (`lp` es un alias).
pub const COMMAND: &str = "luckperms";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LpInstall {
    /// Nombre del .jar encontrado.
    pub jar: String,
    /// Carpeta de datos de LuckPerms, relativa a la del servidor (donde escribe `export`).
    pub data_dir: String,
}

/// Busca el .jar de LuckPerms en `plugins/` (Paper, Purpur, Spigot e híbridos) o en `mods/`.
///
/// Carpeta de datos según el código de cada plataforma de LuckPerms:
/// Bukkit `plugins/LuckPerms`, Fabric `mods/luckperms`, Forge y NeoForge `config/luckperms`.
pub fn detect(root: &Path, kind: ServerKind) -> Option<LpInstall> {
    let find = |dir: &str| -> Option<String> {
        let mut names: Vec<String> = std::fs::read_dir(root.join(dir))
            .ok()?
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| {
                let l = n.to_ascii_lowercase();
                l.ends_with(".jar") && l.contains("luckperms")
            })
            .collect();
        names.sort();
        names.into_iter().next()
    };
    if let Some(jar) = find("plugins") {
        return Some(LpInstall {
            jar,
            data_dir: "plugins/LuckPerms".into(),
        });
    }
    let jar = find("mods")?;
    let data_dir = match kind {
        ServerKind::Fabric | ServerKind::Quilt => "mods/luckperms",
        _ => "config/luckperms",
    };
    Some(LpInstall {
        jar,
        data_dir: data_dir.into(),
    })
}

pub fn data_path(root: &Path, install: &LpInstall) -> PathBuf {
    install.data_dir.split('/').fold(root.to_path_buf(), |p, s| p.join(s))
}

// ---------- Datos leídos ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LpNode {
    /// `permission`, `inheritance`, `prefix`, `suffix`, `meta`, `weight`, `display_name`…
    pub kind: String,
    pub key: String,
    pub value: bool,
    /// Fecha de caducidad (segundos Unix) si es temporal.
    pub expiry: Option<i64>,
    /// Contextos (`server=lobby`, `world=nether`). Vacío = global.
    pub context: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LpGroup {
    pub name: String,
    pub weight: Option<i32>,
    pub prefix: Option<String>,
    /// Grupos de los que hereda (globales y permanentes).
    pub parents: Vec<String>,
    /// El resto de nodos (permisos, prefijos, herencias con contexto o temporales…).
    pub nodes: Vec<LpNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LpUser {
    pub uuid: String,
    pub username: Option<String>,
    pub primary_group: String,
    pub parents: Vec<String>,
    pub nodes: Vec<LpNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LpTrack {
    pub name: String,
    pub groups: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LpSnapshot {
    pub generated_at: Option<String>,
    pub groups: Vec<LpGroup>,
    pub users: Vec<LpUser>,
    pub tracks: Vec<LpTrack>,
}

impl LpSnapshot {
    pub fn group(&self, name: &str) -> Option<&LpGroup> {
        self.groups.iter().find(|g| g.name.eq_ignore_ascii_case(name))
    }
    pub fn user(&self, uuid: &str) -> Option<&LpUser> {
        self.users.iter().find(|u| u.uuid.eq_ignore_ascii_case(uuid))
    }
}

#[derive(Deserialize)]
struct RawNode {
    #[serde(rename = "type", default)]
    kind: String,
    key: String,
    #[serde(default = "yes")]
    value: bool,
    expiry: Option<i64>,
    #[serde(default)]
    context: BTreeMap<String, serde_json::Value>,
}
fn yes() -> bool {
    true
}

#[derive(Deserialize)]
struct RawHolder {
    #[serde(default)]
    nodes: Vec<RawNode>,
    username: Option<String>,
    #[serde(rename = "primaryGroup")]
    primary_group: Option<String>,
}

#[derive(Deserialize)]
struct RawExport {
    #[serde(default)]
    metadata: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    groups: BTreeMap<String, RawHolder>,
    #[serde(default)]
    tracks: BTreeMap<String, RawTrack>,
    #[serde(default)]
    users: BTreeMap<String, RawHolder>,
}
#[derive(Deserialize)]
struct RawTrack {
    #[serde(default)]
    groups: Vec<String>,
}

fn convert(n: RawNode) -> LpNode {
    let context = n
        .context
        .into_iter()
        .map(|(k, v)| {
            let vals = match v {
                serde_json::Value::Array(a) => a.into_iter().filter_map(|x| x.as_str().map(String::from)).collect(),
                serde_json::Value::String(s) => vec![s],
                other => vec![other.to_string()],
            };
            (k, vals)
        })
        .collect();
    LpNode {
        kind: if n.kind.is_empty() { "permission".into() } else { n.kind },
        key: n.key,
        value: n.value,
        expiry: n.expiry,
        context,
    }
}

/// Separa los nodos: herencias globales y permanentes por un lado, el resto por otro.
fn split(nodes: Vec<RawNode>) -> (Vec<String>, Vec<LpNode>) {
    let mut parents = vec![];
    let mut rest = vec![];
    for n in nodes.into_iter().map(convert) {
        let parent = n.key.strip_prefix("group.").filter(|_| {
            n.value && n.expiry.is_none() && n.context.is_empty() && (n.kind == "inheritance" || n.kind == "permission")
        });
        match parent {
            Some(g) => parents.push(g.to_string()),
            None => rest.push(n),
        }
    }
    parents.sort();
    (parents, rest)
}

/// Lee un archivo de `luckperms export` (JSON con gzip).
pub fn parse_export(gz: &[u8]) -> Result<LpSnapshot, AdminError> {
    let mut text = String::new();
    flate2::read::GzDecoder::new(gz)
        .read_to_string(&mut text)
        .map_err(|e| AdminError::Invalid(format!("el archivo de LuckPerms está incompleto: {e}")))?;
    let raw: RawExport = serde_json::from_str(&text)
        .map_err(|e| AdminError::Invalid(format!("el archivo de LuckPerms no tiene el formato esperado: {e}")))?;
    let mut groups: Vec<LpGroup> = raw
        .groups
        .into_iter()
        .map(|(name, h)| {
            let (parents, nodes) = split(h.nodes);
            let weight = nodes
                .iter()
                .filter(|n| n.context.is_empty())
                .find_map(|n| n.key.strip_prefix("weight.").and_then(|w| w.parse().ok()));
            let prefix = nodes
                .iter()
                .filter(|n| n.context.is_empty() && n.value)
                .filter_map(|n| {
                    let rest = n.key.strip_prefix("prefix.")?;
                    let (prio, text) = rest.split_once('.')?;
                    Some((prio.parse::<i32>().ok()?, text.to_string()))
                })
                .max_by_key(|(p, _)| *p)
                .map(|(_, t)| t);
            LpGroup {
                name,
                weight,
                prefix,
                parents,
                nodes: nodes.into_iter().filter(|n| n.kind != "weight").collect(),
            }
        })
        .collect();
    groups.sort_by(|a, b| {
        b.weight
            .unwrap_or(0)
            .cmp(&a.weight.unwrap_or(0))
            .then(a.name.cmp(&b.name))
    });
    let mut users: Vec<LpUser> = raw
        .users
        .into_iter()
        .map(|(uuid, h)| {
            let (parents, nodes) = split(h.nodes);
            LpUser {
                uuid,
                username: h.username,
                primary_group: h.primary_group.unwrap_or_else(|| "default".into()),
                parents,
                nodes,
            }
        })
        .collect();
    users.sort_by(|a, b| {
        let k = |u: &LpUser| u.username.clone().unwrap_or_default().to_lowercase();
        k(a).cmp(&k(b)).then(a.uuid.cmp(&b.uuid))
    });
    Ok(LpSnapshot {
        generated_at: raw
            .metadata
            .get("generatedAt")
            .and_then(|v| v.as_str())
            .map(String::from),
        groups,
        users,
        tracks: raw
            .tracks
            .into_iter()
            .map(|(name, t)| LpTrack { name, groups: t.groups })
            .collect(),
    })
}

// ---------- Cambios ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LpTargetKind {
    Group,
    User,
}

/// A quién se le cambia algo. Para un usuario, `name` es su UUID (lo que trae el export).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LpTarget {
    pub kind: LpTargetKind,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LpChange {
    CreateGroup {
        group: String,
    },
    DeleteGroup {
        group: String,
    },
    SetPermission {
        target: LpTarget,
        node: String,
        value: bool,
    },
    /// Quita un permiso. `context` debe ser exactamente el del nodo (vacío = global).
    UnsetPermission {
        target: LpTarget,
        node: String,
        #[serde(default)]
        context: BTreeMap<String, Vec<String>>,
    },
    AddParent {
        target: LpTarget,
        group: String,
    },
    RemoveParent {
        target: LpTarget,
        group: String,
    },
}

/// Nombres de grupo para crear: como exige LuckPerms (sin espacios, hasta 36), y además
/// solo minúsculas, números, `_` y `-` para que no haya sorpresas en la consola.
pub fn valid_group_name(g: &str) -> bool {
    !g.is_empty()
        && g.len() <= 36
        && g.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Nombre de un grupo que ya existe (lo leímos del export): sin espacios ni comillas.
fn existing_group_ok(g: &str) -> bool {
    !g.is_empty() && g.len() <= 36 && plain_token(g)
}

fn plain_token(s: &str) -> bool {
    !s.chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '"' || c == '\'' || c == '\\')
}

pub fn valid_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && [8, 4, 4, 4, 12]
            .iter()
            .zip(&parts)
            .all(|(n, p)| p.len() == *n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

pub fn valid_node(n: &str) -> bool {
    !n.is_empty() && n.chars().count() <= 200 && plain_token(n)
}

fn target_cmd(t: &LpTarget) -> Result<String, AdminError> {
    match t.kind {
        LpTargetKind::Group if existing_group_ok(&t.name) => Ok(format!("group {}", t.name)),
        LpTargetKind::User if valid_uuid(&t.name) => Ok(format!("user {}", t.name.to_ascii_lowercase())),
        LpTargetKind::Group => Err(AdminError::Invalid(format!("«{}» no es un grupo válido", t.name))),
        LpTargetKind::User => Err(AdminError::Invalid("usuario no válido".into())),
    }
}

/// El comando de consola de un cambio (sin la barra).
pub fn change_command(c: &LpChange) -> Result<String, AdminError> {
    let bad_group = |g: &str| AdminError::Invalid(format!("«{g}» no es un nombre de grupo válido"));
    Ok(match c {
        LpChange::CreateGroup { group } => {
            if !valid_group_name(group) {
                return Err(AdminError::Invalid(
                    "el nombre del grupo solo puede tener minúsculas, números, _ y - (hasta 36)".into(),
                ));
            }
            format!("{COMMAND} creategroup {group}")
        }
        LpChange::DeleteGroup { group } => {
            if !existing_group_ok(group) {
                return Err(bad_group(group));
            }
            if group.eq_ignore_ascii_case("default") {
                return Err(AdminError::Invalid("el grupo «default» no se puede borrar".into()));
            }
            format!("{COMMAND} deletegroup {group}")
        }
        LpChange::SetPermission { target, node, value } => {
            check_node(node)?;
            format!("{COMMAND} {} permission set {node} {value}", target_cmd(target)?)
        }
        LpChange::UnsetPermission { target, node, context } => {
            if !valid_node(node) {
                return Err(AdminError::Invalid("permiso no válido".into()));
            }
            let mut cmd = format!("{COMMAND} {} permission unset {node}", target_cmd(target)?);
            for (k, vals) in context {
                for v in vals {
                    if k.is_empty() || v.is_empty() || !plain_token(k) || !plain_token(v) || k.contains('=') {
                        return Err(AdminError::Invalid(
                            "ese contexto no se puede escribir en la consola".into(),
                        ));
                    }
                    cmd.push_str(&format!(" {k}={v}"));
                }
            }
            cmd
        }
        LpChange::AddParent { target, group } | LpChange::RemoveParent { target, group } => {
            if !existing_group_ok(group) {
                return Err(bad_group(group));
            }
            if target.kind == LpTargetKind::Group && target.name.eq_ignore_ascii_case(group) {
                return Err(AdminError::Invalid("un grupo no puede heredar de sí mismo".into()));
            }
            let verb = if matches!(c, LpChange::AddParent { .. }) {
                "add"
            } else {
                "remove"
            };
            format!("{COMMAND} {} parent {verb} {group}", target_cmd(target)?)
        }
    })
}

fn check_node(node: &str) -> Result<(), AdminError> {
    if !valid_node(node) {
        return Err(AdminError::Invalid(
            "el permiso no puede tener espacios ni comillas y tiene hasta 200 caracteres".into(),
        ));
    }
    if node.to_ascii_lowercase().starts_with("group.") {
        return Err(AdminError::Invalid(
            "para heredar un grupo usa «Añadir grupo» en vez de escribir group.…".into(),
        ));
    }
    Ok(())
}

/// Comprueba antes de mandar el comando que el cambio tiene sentido con los datos actuales.
pub fn precheck(s: &LpSnapshot, c: &LpChange) -> Result<(), AdminError> {
    let target_exists = |t: &LpTarget| match t.kind {
        LpTargetKind::Group => s.group(&t.name).is_some(),
        LpTargetKind::User => s.user(&t.name).is_some(),
    };
    let missing = |t: &LpTarget| {
        AdminError::NotFound(match t.kind {
            LpTargetKind::Group => format!("el grupo «{}»", t.name),
            LpTargetKind::User => "ese usuario en LuckPerms".into(),
        })
    };
    match c {
        LpChange::CreateGroup { group } if s.group(group).is_some() => {
            Err(AdminError::Conflict(format!("ya existe el grupo «{group}»")))
        }
        LpChange::DeleteGroup { group } if s.group(group).is_none() => {
            Err(AdminError::NotFound(format!("el grupo «{group}»")))
        }
        LpChange::SetPermission { target, .. } | LpChange::UnsetPermission { target, .. } if !target_exists(target) => {
            Err(missing(target))
        }
        LpChange::AddParent { target, group } | LpChange::RemoveParent { target, group } => {
            if !target_exists(target) {
                Err(missing(target))
            } else if s.group(group).is_none() {
                Err(AdminError::NotFound(format!("el grupo «{group}»")))
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

fn holder<'a>(s: &'a LpSnapshot, t: &LpTarget) -> Option<(&'a [String], &'a [LpNode])> {
    match t.kind {
        LpTargetKind::Group => s.group(&t.name).map(|g| (g.parents.as_slice(), g.nodes.as_slice())),
        LpTargetKind::User => s.user(&t.name).map(|u| (u.parents.as_slice(), u.nodes.as_slice())),
    }
}

/// ¿Los datos nuevos (de un export hecho después) muestran el cambio aplicado?
pub fn change_applied(s: &LpSnapshot, c: &LpChange) -> bool {
    match c {
        LpChange::CreateGroup { group } => s.group(group).is_some(),
        LpChange::DeleteGroup { group } => s.group(group).is_none(),
        LpChange::SetPermission { target, node, value } => holder(s, target).is_some_and(|(_, nodes)| {
            nodes.iter().any(|n| {
                n.key.eq_ignore_ascii_case(node) && n.value == *value && n.context.is_empty() && n.expiry.is_none()
            })
        }),
        LpChange::UnsetPermission { target, node, context } => holder(s, target).is_some_and(|(_, nodes)| {
            !nodes
                .iter()
                .any(|n| n.key.eq_ignore_ascii_case(node) && &n.context == context && n.expiry.is_none())
        }),
        LpChange::AddParent { target, group } => {
            holder(s, target).is_some_and(|(p, _)| p.iter().any(|g| g.eq_ignore_ascii_case(group)))
        }
        LpChange::RemoveParent { target, group } => {
            holder(s, target).is_some_and(|(p, _)| !p.iter().any(|g| g.eq_ignore_ascii_case(group)))
        }
    }
}

/// Frase para la vista previa y la Actividad.
pub fn describe(s: &LpSnapshot, c: &LpChange) -> String {
    let who = |t: &LpTarget| match t.kind {
        LpTargetKind::Group => format!("el grupo «{}»", t.name),
        LpTargetKind::User => match s.user(&t.name).and_then(|u| u.username.clone()) {
            Some(n) => n,
            None => format!("el usuario {}", t.name),
        },
    };
    match c {
        LpChange::CreateGroup { group } => format!("Crear el grupo «{group}» (sin permisos)."),
        LpChange::DeleteGroup { group } => {
            let users = s
                .users
                .iter()
                .filter(|u| u.parents.iter().any(|p| p.eq_ignore_ascii_case(group)))
                .count();
            let mut t = format!("Borrar el grupo «{group}» con todos sus permisos.");
            if users > 0 {
                t.push_str(&format!(" {users} usuario(s) dejarán de estar en él."));
            }
            t
        }
        LpChange::SetPermission { target, node, value } => {
            if *value {
                format!("Dar el permiso {node} a {}.", who(target))
            } else {
                format!("Negar el permiso {node} a {} (queda en false).", who(target))
            }
        }
        LpChange::UnsetPermission { target, node, .. } => {
            format!(
                "Quitar el permiso {node} de {} (vuelve a decidirlo la herencia).",
                who(target)
            )
        }
        LpChange::AddParent { target, group } => match target.kind {
            LpTargetKind::Group => format!("Hacer que {} herede los permisos de «{group}».", who(target)),
            LpTargetKind::User => format!("Poner a {} en el grupo «{group}».", who(target)),
        },
        LpChange::RemoveParent { target, group } => match target.kind {
            LpTargetKind::Group => format!("Hacer que {} deje de heredar de «{group}».", who(target)),
            LpTargetKind::User => format!("Sacar a {} del grupo «{group}».", who(target)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn gz(text: &str) -> Vec<u8> {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(text.as_bytes()).unwrap();
        e.finish().unwrap()
    }

    const SAMPLE: &str = r#"{
      "metadata": {"generatedBy": "Console", "generatedAt": "2026-10-30 12:00:00 UTC"},
      "groups": {
        "admin": {"nodes": [
          {"type": "permission", "key": "essentials.fly", "value": true},
          {"type": "weight", "key": "weight.10", "value": true},
          {"type": "prefix", "key": "prefix.10.&c[Admin] ", "value": true},
          {"type": "inheritance", "key": "group.default", "value": true},
          {"type": "permission", "key": "worldedit.*", "value": true, "context": {"world": ["creativo"]}}
        ]},
        "default": {"nodes": []}
      },
      "tracks": {"staff": {"groups": ["default", "admin"]}},
      "users": {
        "069a79f4-44e9-4726-a5be-fca90e38aaf5": {"username": "Notch", "primaryGroup": "admin", "nodes": [
          {"type": "inheritance", "key": "group.admin", "value": true},
          {"type": "inheritance", "key": "group.vip", "value": true, "expiry": 1900000000},
          {"type": "permission", "key": "essentials.home", "value": false}
        ]}
      }
    }"#;

    #[test]
    fn reads_an_export() {
        let s = parse_export(&gz(SAMPLE)).unwrap();
        assert_eq!(s.generated_at.as_deref(), Some("2026-10-30 12:00:00 UTC"));
        assert_eq!(s.groups[0].name, "admin", "el de más peso va primero");
        let admin = &s.groups[0];
        assert_eq!(admin.weight, Some(10));
        assert_eq!(admin.prefix.as_deref(), Some("&c[Admin] "));
        assert_eq!(admin.parents, vec!["default"]);
        assert!(admin.nodes.iter().all(|n| n.kind != "weight"));
        let we = admin.nodes.iter().find(|n| n.key == "worldedit.*").unwrap();
        assert_eq!(we.context["world"], vec!["creativo"]);
        let u = &s.users[0];
        assert_eq!(u.username.as_deref(), Some("Notch"));
        assert_eq!(u.primary_group, "admin");
        assert_eq!(
            u.parents,
            vec!["admin"],
            "la herencia temporal no cuenta como grupo fijo"
        );
        assert!(u.nodes.iter().any(|n| n.key == "group.vip" && n.expiry.is_some()));
        assert_eq!(s.tracks[0].groups, vec!["default", "admin"]);
        // Un archivo a medio escribir se rechaza (el supervisor reintenta).
        let full = gz(SAMPLE);
        assert!(parse_export(&full[..full.len() / 2]).is_err());
    }

    #[test]
    fn commands_are_validated() {
        let g = |n: &str| LpTarget {
            kind: LpTargetKind::Group,
            name: n.into(),
        };
        let u = LpTarget {
            kind: LpTargetKind::User,
            name: "069A79F4-44e9-4726-a5be-fca90e38aaf5".into(),
        };
        let ok = |c: LpChange| change_command(&c).unwrap();
        assert_eq!(
            ok(LpChange::CreateGroup { group: "vip".into() }),
            "luckperms creategroup vip"
        );
        assert_eq!(
            ok(LpChange::SetPermission {
                target: g("vip"),
                node: "essentials.fly".into(),
                value: false
            }),
            "luckperms group vip permission set essentials.fly false"
        );
        assert_eq!(
            ok(LpChange::AddParent {
                target: u.clone(),
                group: "vip".into()
            }),
            "luckperms user 069a79f4-44e9-4726-a5be-fca90e38aaf5 parent add vip"
        );
        let mut ctx = BTreeMap::new();
        ctx.insert("world".to_string(), vec!["creativo".to_string()]);
        assert_eq!(
            ok(LpChange::UnsetPermission {
                target: g("admin"),
                node: "worldedit.*".into(),
                context: ctx
            }),
            "luckperms group admin permission unset worldedit.* world=creativo"
        );
        let bad = |c: LpChange| change_command(&c).is_err();
        assert!(bad(LpChange::CreateGroup {
            group: "Con Espacio".into()
        }));
        assert!(bad(LpChange::CreateGroup {
            group: "x;op Steve".into()
        }));
        assert!(bad(LpChange::DeleteGroup {
            group: "default".into()
        }));
        assert!(bad(LpChange::SetPermission {
            target: g("vip"),
            node: "a b".into(),
            value: true
        }));
        assert!(bad(LpChange::SetPermission {
            target: g("vip"),
            node: "fly\nop Steve".into(),
            value: true
        }));
        assert!(bad(LpChange::SetPermission {
            target: g("vip"),
            node: "group.admin".into(),
            value: true
        }));
        assert!(bad(LpChange::AddParent {
            target: g("vip"),
            group: "vip".into()
        }));
        let not_uuid = LpTarget {
            kind: LpTargetKind::User,
            name: "Notch".into(),
        };
        assert!(bad(LpChange::AddParent {
            target: not_uuid,
            group: "vip".into()
        }));
    }

    #[test]
    fn prechecks_and_verifies() {
        let s = parse_export(&gz(SAMPLE)).unwrap();
        let admin = LpTarget {
            kind: LpTargetKind::Group,
            name: "admin".into(),
        };
        let notch = LpTarget {
            kind: LpTargetKind::User,
            name: "069a79f4-44e9-4726-a5be-fca90e38aaf5".into(),
        };
        assert!(precheck(&s, &LpChange::CreateGroup { group: "admin".into() }).is_err());
        assert!(precheck(&s, &LpChange::DeleteGroup { group: "vip".into() }).is_err());
        assert!(precheck(
            &s,
            &LpChange::AddParent {
                target: notch.clone(),
                group: "vip".into()
            }
        )
        .is_err());
        assert!(precheck(
            &s,
            &LpChange::AddParent {
                target: notch.clone(),
                group: "default".into()
            }
        )
        .is_ok());

        assert!(change_applied(
            &s,
            &LpChange::SetPermission {
                target: admin.clone(),
                node: "essentials.fly".into(),
                value: true
            }
        ));
        assert!(!change_applied(
            &s,
            &LpChange::SetPermission {
                target: admin.clone(),
                node: "essentials.fly".into(),
                value: false
            }
        ));
        assert!(!change_applied(
            &s,
            &LpChange::UnsetPermission {
                target: notch.clone(),
                node: "essentials.home".into(),
                context: BTreeMap::new()
            }
        ));
        assert!(change_applied(
            &s,
            &LpChange::AddParent {
                target: notch.clone(),
                group: "admin".into()
            }
        ));
        assert!(change_applied(
            &s,
            &LpChange::RemoveParent {
                target: notch.clone(),
                group: "vip".into()
            }
        ));

        let d = describe(&s, &LpChange::DeleteGroup { group: "admin".into() });
        assert!(d.contains("1 usuario"), "{d}");
        assert_eq!(
            describe(
                &s,
                &LpChange::AddParent {
                    target: notch,
                    group: "default".into()
                }
            ),
            "Poner a Notch en el grupo «default»."
        );
    }

    #[test]
    fn detects_plugin_or_mod() {
        let t = tempfile::tempdir().unwrap();
        assert!(detect(t.path(), ServerKind::Paper).is_none());
        std::fs::create_dir_all(t.path().join("mods")).unwrap();
        std::fs::write(t.path().join("mods/LuckPerms-Fabric-5.5.17.jar"), b"").unwrap();
        assert_eq!(detect(t.path(), ServerKind::Fabric).unwrap().data_dir, "mods/luckperms");
        assert_eq!(
            detect(t.path(), ServerKind::NeoForge).unwrap().data_dir,
            "config/luckperms"
        );
        std::fs::create_dir_all(t.path().join("plugins")).unwrap();
        std::fs::write(t.path().join("plugins/LuckPerms-Bukkit-5.5.17.jar"), b"").unwrap();
        let d = detect(t.path(), ServerKind::Paper).unwrap();
        assert_eq!(d.data_dir, "plugins/LuckPerms");
        assert_eq!(data_path(t.path(), &d), t.path().join("plugins").join("LuckPerms"));
    }
}
