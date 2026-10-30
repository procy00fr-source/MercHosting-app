//! Lectura de `server.properties` y `eula.txt`.
//!
//! Solo lectura en este incremento. La escritura (con backup y escritura atómica)
//! llegará con el editor guiado; MercHosting nunca acepta la EULA en nombre del usuario.

use std::collections::BTreeMap;
use std::path::Path;

pub const DEFAULT_PORT: u16 = 25565;

/// Archivo .properties de Java simplificado: `clave=valor`, comentarios con `#` o `!`.
/// Conserva el orden en un BTreeMap solo para consulta; no se usa para reescribir.
pub fn parse_properties(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for raw in text.lines() {
        let line = raw.trim_start();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }
        let (k, v) = match line.find(['=', ':']) {
            Some(i) => (&line[..i], &line[i + 1..]),
            None => (line, ""),
        };
        map.insert(k.trim().to_string(), v.trim().to_string());
    }
    map
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerProperties {
    pub port: u16,
    /// `true` si el puerto vino del archivo; `false` si se asumió el valor por defecto.
    pub port_explicit: bool,
    pub level_name: String,
    pub server_ip: Option<String>,
    pub raw: BTreeMap<String, String>,
}

impl ServerProperties {
    pub fn read(root: &Path) -> std::io::Result<Option<Self>> {
        let path = root.join("server.properties");
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(Self::from_text(&String::from_utf8_lossy(&bytes)))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn from_text(text: &str) -> Self {
        let raw = parse_properties(text);
        let parsed_port = raw
            .get("server-port")
            .and_then(|p| p.parse::<u16>().ok())
            .filter(|p| *p != 0);
        Self {
            port: parsed_port.unwrap_or(DEFAULT_PORT),
            port_explicit: parsed_port.is_some(),
            level_name: raw
                .get("level-name")
                .filter(|s| !s.is_empty())
                .cloned()
                .unwrap_or_else(|| "world".to_string()),
            server_ip: raw.get("server-ip").filter(|s| !s.is_empty()).cloned(),
            raw,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EulaStatus {
    Accepted,
    NotAccepted,
    Missing,
}

pub fn read_eula(root: &Path) -> std::io::Result<EulaStatus> {
    match std::fs::read_to_string(root.join("eula.txt")) {
        Ok(text) => {
            let props = parse_properties(&text);
            Ok(match props.get("eula").map(|v| v.eq_ignore_ascii_case("true")) {
                Some(true) => EulaStatus::Accepted,
                _ => EulaStatus::NotAccepted,
            })
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(EulaStatus::Missing),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_port_and_level() {
        let p = ServerProperties::from_text("#c\nserver-port=25570\nlevel-name=MiMundo\nmotd=Hola: mundo\n");
        assert_eq!(p.port, 25570);
        assert!(p.port_explicit);
        assert_eq!(p.level_name, "MiMundo");
        assert_eq!(p.raw.get("motd").unwrap(), "Hola: mundo");
    }

    #[test]
    fn defaults() {
        let p = ServerProperties::from_text("server-port=abc\n");
        assert_eq!(p.port, DEFAULT_PORT);
        assert!(!p.port_explicit);
        assert_eq!(p.level_name, "world");
    }

    #[test]
    fn eula_states() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(read_eula(d.path()).unwrap(), EulaStatus::Missing);
        std::fs::write(d.path().join("eula.txt"), "#x\neula=false\n").unwrap();
        assert_eq!(read_eula(d.path()).unwrap(), EulaStatus::NotAccepted);
        std::fs::write(d.path().join("eula.txt"), "eula=TRUE\n").unwrap();
        assert_eq!(read_eula(d.path()).unwrap(), EulaStatus::Accepted);
    }
}
