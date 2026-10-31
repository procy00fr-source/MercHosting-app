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

/// Versión de Minecraft simplificada para decidir el Java.
enum McVersion {
    /// Numeración clásica `1.MINOR.PATCH` (hasta 1.21.11).
    Classic(u32, u32),
    /// Numeración por año `AA.DROP[.HOTFIX]` (desde 26.1).
    Yearly,
}

fn parse_mc(v: &str) -> Option<McVersion> {
    let v = v.trim();
    // Sufijos como "-pre1", "-rc2" o "-snapshot-1" no cambian el Java requerido.
    let core = v.split(['-', ' ']).next()?;
    let mut it = core.split('.');
    let first: u32 = it.next()?.parse().ok()?;
    if first == 1 {
        let minor = it.next()?.parse().ok()?;
        let patch = it.next().map(|p| p.parse().unwrap_or(0)).unwrap_or(0);
        return Some(McVersion::Classic(minor, patch));
    }
    if first >= 26 {
        let _drop: u32 = it.next()?.parse().ok()?;
        return Some(McVersion::Yearly);
    }
    None
}

pub fn required_java(mc_version: &str) -> Option<JavaRequirement> {
    let (minor, patch) = match parse_mc(mc_version)? {
        McVersion::Yearly => {
            return Some(JavaRequirement {
                min_major: 25,
                max_tested_major: None,
            })
        }
        McVersion::Classic(m, p) => (m, p),
    };
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

/// Nombre del ejecutable de Java en esta plataforma.
pub fn java_exe_name() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

/// Busca posibles ejecutables de Java sin ejecutarlos: `JAVA_HOME`, el `PATH` y las carpetas
/// habituales de los instaladores de Windows (Adoptium/Temurin, Microsoft, Oracle, Zulu, Amazon).
/// Devuelve rutas absolutas existentes y sin duplicados. La versión se comprueba aparte con
/// [`probe_java`].
pub fn find_java_candidates() -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
    let exe = java_exe_name();
    let mut out: Vec<PathBuf> = vec![];
    let mut push = |p: PathBuf| {
        if p.is_file() {
            let canon = std::fs::canonicalize(&p).unwrap_or(p);
            // En Windows, canonicalize añade el prefijo \\?\; se quita para mostrarlo.
            let shown = PathBuf::from(canon.to_string_lossy().trim_start_matches(r"\\?\").to_string());
            let key = shown.to_string_lossy().to_lowercase();
            if !out.iter().any(|o| o.to_string_lossy().to_lowercase() == key) {
                out.push(shown);
            }
        }
    };
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        push(PathBuf::from(home).join("bin").join(exe));
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            push(dir.join(exe));
        }
    }
    if cfg!(windows) {
        let mut roots = vec![];
        for var in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
            if let Some(v) = std::env::var_os(var) {
                roots.push(PathBuf::from(v));
            }
        }
        for root in roots {
            for vendor in [
                "Eclipse Adoptium",
                "Microsoft",
                "Java",
                "Zulu",
                "Amazon Corretto",
                "BellSoft",
                "Semeru",
            ] {
                let dir = root.join(vendor);
                for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                    push(entry.path().join("bin").join(exe));
                }
            }
        }
    }
    out
}

/// Versión de Java recomendada (no solo la mínima) para una versión de Minecraft.
/// Usar el Java más nuevo instalado no siempre funciona: loaders y mods de una época se prueban
/// con su Java LTS. Para versiones desconocidas se recomienda 21.
pub fn recommended_java_major(mc_version: Option<&str>) -> u32 {
    match mc_version.and_then(required_java) {
        Some(r) if r.min_major >= 25 => 25,
        Some(r) if r.min_major >= 21 => 21,
        Some(r) if r.min_major >= 16 => 17,
        Some(_) => 8,
        None => 21,
    }
}

/// Elige el mejor Java entre candidatos ya comprobados `(ruta, versión mayor)`: primero la
/// versión recomendada; si no hay, la más baja que cumpla el mínimo (y el máximo probado).
/// A igualdad, se prefieren instalaciones reales antes que los accesos `javapath` de Oracle,
/// que pueden cambiar de destino con cada actualización.
pub fn pick_java<'a>(
    cands: &'a [(std::path::PathBuf, u32)],
    mc_version: Option<&str>,
) -> Option<&'a std::path::PathBuf> {
    let req = mc_version.and_then(required_java).unwrap_or(JavaRequirement {
        min_major: 17,
        max_tested_major: None,
    });
    let rec = recommended_java_major(mc_version);
    let is_shim = |p: &std::path::Path| p.to_string_lossy().to_lowercase().contains("javapath");
    cands
        .iter()
        .filter(|(_, m)| *m >= req.min_major && req.max_tested_major.is_none_or(|max| *m <= max))
        .min_by_key(|(p, m)| (*m != rec, *m, is_shim(p)))
        .map(|(p, _)| p)
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
        assert_eq!(required_java("1.21.11").unwrap().min_major, 21);
        assert_eq!(required_java("26.1").unwrap().min_major, 25);
        assert_eq!(required_java("26.1.2").unwrap().min_major, 25);
        assert_eq!(required_java("26.3-pre1").unwrap().min_major, 25);
        assert_eq!(recommended_java_major(Some("26.1")), 25);
        assert_eq!(recommended_java_major(Some("1.21.11")), 21);
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
    fn finds_java_on_path_when_installed() {
        // En el entorno de pruebas hay un JDK en PATH; se exige que la búsqueda lo encuentre.
        let found = find_java_candidates();
        assert!(!found.is_empty(), "no se encontró Java aunque las pruebas lo necesitan");
        assert!(found.iter().all(|p| p.is_absolute() && p.is_file()));
    }

    #[test]
    fn picks_recommended_not_newest() {
        use std::path::PathBuf;
        let c = vec![
            (
                PathBuf::from(r"C:\Program Files\Common Files\Oracle\Java\javapath\java.exe"),
                25,
            ),
            (
                PathBuf::from(r"C:\Program Files (x86)\Common Files\Oracle\Java\java8path\java.exe"),
                8,
            ),
            (PathBuf::from(r"C:\Program Files\Java\jdk-17\bin\java.exe"), 17),
            (
                PathBuf::from(r"C:\Program Files\Common Files\Oracle\Java\javapath_x\java.exe"),
                21,
            ),
            (
                PathBuf::from(r"C:\Program Files\Eclipse Adoptium\jdk-21\bin\java.exe"),
                21,
            ),
            (PathBuf::from(r"C:\Program Files\Java\jdk-25\bin\java.exe"), 25),
        ];
        assert_eq!(
            pick_java(&c, Some("1.21.1")).unwrap(),
            &c[4].0,
            "Java 21 real, no el acceso javapath ni Java 25"
        );
        assert_eq!(pick_java(&c, Some("1.20.1")).unwrap(), &c[2].0, "1.20.1 -> Java 17");
        assert_eq!(
            pick_java(&c, Some("1.16.5")).unwrap(),
            &c[1].0,
            "1.16.5 -> Java 8 exactamente"
        );
        assert_eq!(pick_java(&c, None).unwrap(), &c[4].0, "versión desconocida -> 21");
        let only25 = vec![(PathBuf::from("j25"), 25)];
        assert_eq!(
            pick_java(&only25, Some("1.21.1")).unwrap(),
            &only25[0].0,
            "si solo hay uno compatible, ese"
        );
        assert_eq!(pick_java(&only25, Some("1.16.5")), None, "Java 25 no sirve para 1.16.5");
        let only8 = vec![(PathBuf::from("j8"), 8)];
        assert_eq!(pick_java(&only8, Some("1.21.1")), None);
    }

    #[test]
    fn missing_java() {
        let r = probe_java(Path::new("/no/existe/java"), Duration::from_secs(2));
        assert!(matches!(r, Err(JavaProbeError::NotFound(_))));
    }
}
