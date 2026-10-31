//! Centro de seguridad de un servidor: comprobaciones reales (ajustes, exposición a Internet,
//! compañeros, copias y claves) con una explicación y, si se puede, un arreglo a un clic.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Peligro,
    Aviso,
    Info,
    Bien,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SecurityAction {
    /// Cambiar un valor de `server.properties` (con copia previa, como en Ajustes).
    SetProperty {
        key: String,
        value: String,
    },
    OpenTab {
        tab: String,
    },
    OpenCompanions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityCheck {
    pub code: String,
    pub level: Level,
    pub title: String,
    pub detail: String,
    pub action: Option<SecurityAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityReport {
    pub checked_at_unix_ms: i64,
    /// Direcciones públicas (túneles) que llevan a este servidor.
    pub public_addresses: Vec<String>,
    pub checks: Vec<SecurityCheck>,
}

pub struct CompanionAccess {
    pub name: String,
    /// Permisos de alto riesgo concedidos (sus nombres para personas).
    pub high_risk: Vec<String>,
}

pub struct Inputs<'a> {
    pub props: &'a BTreeMap<String, String>,
    pub public_addresses: &'a [String],
    pub companions: &'a [CompanionAccess],
    /// Intentos denegados de compañeros en los últimos 7 días.
    pub denied_recently: u32,
    pub last_backup_ms: Option<i64>,
    pub backup_scheduled: bool,
    /// Claves de MercHosting o Playit encontradas dentro de la carpeta del servidor.
    pub secrets_in_server: &'a [String],
    pub now_ms: i64,
}

fn check(
    code: &str,
    level: Level,
    title: impl Into<String>,
    detail: impl Into<String>,
    action: Option<SecurityAction>,
) -> SecurityCheck {
    SecurityCheck {
        code: code.into(),
        level,
        title: title.into(),
        detail: detail.into(),
        action,
    }
}

fn set(key: &str, value: &str) -> Option<SecurityAction> {
    Some(SecurityAction::SetProperty {
        key: key.into(),
        value: value.into(),
    })
}

pub fn evaluate(i: &Inputs) -> SecurityReport {
    let p = |k: &str| i.props.get(k).map(|v| v.trim().to_ascii_lowercase());
    let is_true = |k: &str| p(k).as_deref() == Some("true");
    let exposed = !i.public_addresses.is_empty();
    let mut out = vec![];

    // Cuentas oficiales.
    if p("online-mode").as_deref() == Some("false") {
        out.push(check(
            "online_mode",
            Level::Peligro,
            "Cualquiera puede entrar con el nombre que quiera",
            "online-mode está desactivado: el servidor no comprueba las cuentas de Minecraft, así que alguien podría entrar haciéndose pasar por ti o por un OP. Actívalo salvo que sepas exactamente por qué lo quitaste.",
            set("online-mode", "true"),
        ));
    } else {
        out.push(check(
            "online_mode",
            Level::Bien,
            "Solo entran cuentas oficiales de Minecraft",
            "online-mode está activado.",
            None,
        ));
    }

    // Exposición a Internet y lista blanca.
    let whitelist = is_true("white-list");
    if exposed {
        out.push(check(
            "exposed",
            Level::Info,
            "Tu servidor se puede alcanzar desde Internet",
            format!(
                "Por {}. Quien tenga la dirección puede intentar entrar.",
                i.public_addresses.join(", ")
            ),
            None,
        ));
        if !whitelist {
            out.push(check(
                "whitelist",
                Level::Aviso,
                "Cualquiera con la dirección puede entrar",
                "La lista blanca está desactivada. Si solo juegas con amigos, añádelos en Jugadores › Lista blanca y luego actívala.",
                Some(SecurityAction::OpenTab { tab: "players".into() }),
            ));
        }
    } else {
        out.push(check(
            "exposed",
            Level::Bien,
            "No hay ninguna dirección pública hacia este servidor",
            "MercHosting no detecta túneles de Playit hacia su puerto. Si abriste puertos en tu router a mano, eso no se puede ver desde aquí.",
            None,
        ));
    }
    if whitelist {
        if !is_true("enforce-whitelist") {
            out.push(check(
                "enforce_whitelist",
                Level::Info,
                "La lista blanca no expulsa a quien ya está dentro",
                "Si quitas a alguien de la lista mientras juega, seguirá conectado hasta que salga. Con enforce-whitelist se le expulsa al momento.",
                set("enforce-whitelist", "true"),
            ));
        } else {
            out.push(check(
                "whitelist",
                Level::Bien,
                "Lista blanca activada",
                "Solo entran los jugadores de la lista.",
                None,
            ));
        }
    }

    // Accesos de control remoto propios de Minecraft.
    if is_true("enable-rcon") {
        let pass = i.props.get("rcon.password").map(|s| s.trim().len()).unwrap_or(0);
        out.push(check(
            "rcon",
            if pass < 12 { Level::Peligro } else { Level::Aviso },
            if pass < 12 { "RCON activado con una contraseña débil o vacía" } else { "RCON activado" },
            "RCON permite controlar el servidor (comandos de OP) desde otros programas con su contraseña. MercHosting no lo necesita: si no lo usas, desactívalo.",
            set("enable-rcon", "false"),
        ));
    }
    if is_true("enable-query") {
        out.push(check(
            "query",
            Level::Info,
            "Query activado",
            "Permite que otros programas consulten la lista de jugadores y los plugins del servidor. Si no lo usas, puedes desactivarlo.",
            set("enable-query", "false"),
        ));
    }

    // Compañeros.
    if i.companions.is_empty() {
        out.push(check(
            "companions",
            Level::Bien,
            "Nadie más administra este servidor",
            "No hay compañeros con acceso.",
            None,
        ));
    } else {
        for c in i.companions {
            if c.high_risk.is_empty() {
                out.push(check(
                    "companion",
                    Level::Info,
                    format!("{} puede administrar este servidor", c.name),
                    "Sin permisos de alto riesgo.",
                    Some(SecurityAction::OpenCompanions),
                ));
            } else {
                out.push(check(
                    "companion",
                    Level::Aviso,
                    format!("{} tiene permisos de alto riesgo", c.name),
                    format!("Puede: {}. Revísalo si ya no lo necesita.", c.high_risk.join(", ")),
                    Some(SecurityAction::OpenCompanions),
                ));
            }
        }
    }
    if i.denied_recently > 0 {
        out.push(check(
            "denied",
            Level::Aviso,
            format!(
                "{} intento(s) de compañeros sin permiso en los últimos 7 días",
                i.denied_recently
            ),
            "Tu PC los rechazó. En Actividad ves quién y qué intentó.",
            Some(SecurityAction::OpenTab { tab: "activity".into() }),
        ));
    }

    // Copias.
    match i.last_backup_ms {
        None => out.push(check(
            "backups",
            Level::Aviso,
            "No hay copias de seguridad",
            "Si algo sale mal (un mod, un griefer, un apagón) no habrá forma de volver atrás.",
            Some(SecurityAction::OpenTab { tab: "backups".into() }),
        )),
        Some(t) => {
            let days = (i.now_ms - t).max(0) / 86_400_000;
            out.push(check(
                "backups",
                if days > 7 { Level::Info } else { Level::Bien },
                if days == 0 {
                    "Hay una copia de hoy".to_string()
                } else {
                    format!("La última copia es de hace {days} día(s)")
                },
                "Las copias se comprueban con su huella al hacerse.",
                (days > 7).then(|| SecurityAction::OpenTab { tab: "backups".into() }),
            ))
        }
    }
    if !i.backup_scheduled {
        out.push(check(
            "backup_schedule",
            Level::Info,
            "Sin copias automáticas",
            "Programa una copia diaria en Tareas programadas para no depender de acordarte.",
            Some(SecurityAction::OpenTab { tab: "tasks".into() }),
        ));
    }

    // Claves.
    if i.secrets_in_server.is_empty() {
        out.push(check(
            "secrets",
            Level::Bien,
            "Ninguna clave de MercHosting o de Playit está en la carpeta del servidor",
            "Se guardan aparte, en la carpeta de datos de MercHosting, donde ni los compañeros ni los mods del servidor las tienen a mano por los archivos del servidor.",
            None,
        ));
    } else {
        out.push(check(
            "secrets",
            Level::Peligro,
            "Hay claves dentro de la carpeta del servidor",
            format!(
                "Se encontró: {}. Cualquiera con acceso a los archivos del servidor podría leerlas. Muévelas fuera de la carpeta.",
                i.secrets_in_server.join(", ")
            ),
            Some(SecurityAction::OpenTab { tab: "files".into() }),
        ));
    }

    out.sort_by_key(|c| c.level);
    SecurityReport {
        checked_at_unix_ms: i.now_ms,
        public_addresses: i.public_addresses.to_vec(),
        checks: out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn risky_public_server() {
        let p = props(&[
            ("online-mode", "false"),
            ("white-list", "false"),
            ("enable-rcon", "true"),
            ("rcon.password", "123"),
        ]);
        let addrs = vec!["dark-sea.joinmc.link".to_string()];
        let comps = [CompanionAccess {
            name: "Shandé".into(),
            high_risk: vec!["enviar comandos".into()],
        }];
        let r = evaluate(&Inputs {
            props: &p,
            public_addresses: &addrs,
            companions: &comps,
            denied_recently: 2,
            last_backup_ms: None,
            backup_scheduled: false,
            secrets_in_server: &["playit.toml".to_string()],
            now_ms: 10 * 86_400_000,
        });
        let codes: Vec<(&str, Level)> = r.checks.iter().map(|c| (c.code.as_str(), c.level)).collect();
        for want in [
            ("online_mode", Level::Peligro),
            ("rcon", Level::Peligro),
            ("secrets", Level::Peligro),
            ("whitelist", Level::Aviso),
            ("companion", Level::Aviso),
            ("denied", Level::Aviso),
            ("backups", Level::Aviso),
            ("backup_schedule", Level::Info),
        ] {
            assert!(codes.contains(&want), "falta {want:?}: {codes:?}");
        }
        assert_eq!(r.checks[0].level, Level::Peligro);
        let om = r.checks.iter().find(|c| c.code == "online_mode").unwrap();
        assert_eq!(om.action, set("online-mode", "true"));
    }

    #[test]
    fn calm_private_server() {
        let p = props(&[
            ("online-mode", "true"),
            ("white-list", "true"),
            ("enforce-whitelist", "true"),
        ]);
        let r = evaluate(&Inputs {
            props: &p,
            public_addresses: &[],
            companions: &[],
            denied_recently: 0,
            last_backup_ms: Some(10 * 86_400_000 - 3_600_000),
            backup_scheduled: true,
            secrets_in_server: &[],
            now_ms: 10 * 86_400_000,
        });
        assert!(r.checks.iter().all(|c| c.level == Level::Bien), "{:?}", r.checks);
        assert!(r.checks.iter().any(|c| c.title == "Hay una copia de hoy"));
    }
}
