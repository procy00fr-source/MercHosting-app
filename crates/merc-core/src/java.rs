//! Requisitos de Java por versión de Minecraft y verificación real del ejecutable.
//!
//! Tabla basada en los requisitos publicados por Mojang para el servidor:
//! 1.20.5+ (incluida 1.21.x) -> Java 21; 1.18 – 1.20.4 -> Java 17; 1.17.x -> Java 16;
//! 1.16.5 o anterior -> Java 8. Algunos loaders antiguos fallan con Java más nuevo
//! (p. ej. Forge 1.16.5 necesita Java 8), por eso para <= 1.16 se recomienda exactamente 8.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JavaRequirement {
    pub min_major: u32,
    /// Si es `Some`, versiones mayores pueden no funcionar con mods/loaders de esa época.
    pub max_tested_major: Option<u32>,
}

fn parse_mc(v: &str) -> Option<(u32, u32)> {
    let mut it = v.trim().split('.');
    if it.next()? != "1" {
        return None;
    }
    let minor = it.next()?.parse().ok()?;
    let patch = it.next().map(|p| p.parse().unwrap_or(0)).unwrap_or(0);
    Some((minor, patch))
}

pub fn required_java(mc_version: &str) -> Option<JavaRequirement> {
    let (minor, patch) = parse_mc(mc_version)?;
    Some(match (minor, patch) {
        (m, _) if m >= 21 => JavaRequirement {
            min_major: 21,
            max_tested_major: None,
        },
        (20, p) if p >= 5 => JavaRequirement {
            min_major: 21,
            max_tested_major: None,
        },
        (18..=20, _) => JavaRequirement {
            min_major: 17,
            max_tested_major: None,
        },
        (17, _) => JavaRequirement {
            min_major: 16,
            max_tested_major: None,
        },
        _ => JavaRequirement {
            min_major: 8,
            max_tested_major: Some(8),
        },
    })
}

/// Extrae la versión mayor de la salida de `java -version` (va a stderr).
/// `openjdk version "21.0.4" ...` -> 21; `java version "1.8.0_421"` -> 8.
pub fn parse_java_version_output(text: &str) -> Option<u32> {
    let start = text.find("version \"")? + "version \"".len();
    let rest = &text[start..];
    let ver = &rest[..rest.find('"')?];
    let mut parts = ver.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaProbe {
    pub major: u32,
    pub raw: String,
}

#[derive(Debug, thiserror::Error)]
pub enum JavaProbeError {
    #[error("no se encontró Java en {0}")]
    NotFound(String),
    #[error("Java no respondió a tiempo")]
    Timeout,
    #[error("no se pudo interpretar la versión de Java: {0}")]
    Unparseable(String),
    #[error("error al ejecutar Java: {0}")]
    Io(String),
}

/// Ejecuta `<java> -version` sin ventana y con límite de tiempo.
pub fn probe_java(java: &Path, timeout: Duration) -> Result<JavaProbe, JavaProbeError> {
    let mut cmd = Command::new(java);
    cmd.arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::process_flags::no_console_window(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => JavaProbeError::NotFound(java.display().to_string()),
        _ => JavaProbeError::Io(e.to_string()),
    })?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(JavaProbeError::Timeout);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(e) => return Err(JavaProbeError::Io(e.to_string())),
        }
    }
    let out = child
        .wait_with_output()
        .map_err(|e| JavaProbeError::Io(e.to_string()))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let major = parse_java_version_output(&text)
        .ok_or_else(|| JavaProbeError::Unparseable(text.lines().next().unwrap_or("").to_string()))?;
    Ok(JavaProbe {
        major,
        raw: text.lines().find(|l| l.contains("version")).unwrap_or("").to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requirement_table() {
        assert_eq!(required_java("1.21.1").unwrap().min_major, 21);
        assert_eq!(required_java("1.21").unwrap().min_major, 21);
        assert_eq!(required_java("1.20.6").unwrap().min_major, 21);
        assert_eq!(required_java("1.20.4").unwrap().min_major, 17);
        assert_eq!(required_java("1.20.1").unwrap().min_major, 17);
        assert_eq!(required_java("1.18.2").unwrap().min_major, 17);
        assert_eq!(required_java("1.17.1").unwrap().min_major, 16);
        assert_eq!(
            required_java("1.16.5").unwrap(),
            JavaRequirement {
                min_major: 8,
                max_tested_major: Some(8)
            }
        );
        assert_eq!(required_java("25w14a"), None);
    }

    #[test]
    fn parse_versions() {
        assert_eq!(
            parse_java_version_output("openjdk version \"21.0.4\" 2024-07-16"),
            Some(21)
        );
        assert_eq!(parse_java_version_output("java version \"1.8.0_421\""), Some(8));
        assert_eq!(parse_java_version_output("openjdk version \"17\" 2021-09-14"), Some(17));
        assert_eq!(parse_java_version_output("basura"), None);
    }

    #[test]
    fn missing_java() {
        let r = probe_java(Path::new("/no/existe/java"), Duration::from_secs(2));
        assert!(matches!(r, Err(JavaProbeError::NotFound(_))));
    }
}
