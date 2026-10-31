//! Confinamiento de rutas a la raíz de una instancia.
//!
//! Toda ruta que venga de la interfaz, de un compañero o de un archivo comprimido
//! debe pasar por [`SandboxRoot::resolve`] antes de tocar el disco.
//!
//! Se rechaza:
//! - rutas absolutas, prefijos de unidad (`C:`), rutas UNC y raíz (`\`);
//! - componentes `..`;
//! - nombres reservados de Windows (`CON`, `NUL`, `COM1`…), flujos alternos (`a.txt:secreto`)
//!   y caracteres inválidos en Windows, aunque se ejecute en otro sistema;
//! - cualquier componente existente que sea symlink o junction (en Windows, `is_symlink()`
//!   de la biblioteca estándar también reporta puntos de montaje/junctions);
//! - cualquier resultado cuya ruta canónica quede fuera de la raíz canónica.
//!
//! Limitación conocida (TOCTOU): entre la validación y el uso, otro proceso local con
//! permisos sobre la carpeta podría sustituir un directorio por un enlace. Las operaciones
//! de escritura deberán revalidar justo antes de abrir y abrir sin seguir enlaces cuando
//! la plataforma lo permita. Esto se documenta, no se oculta.

use std::path::{Component, Path, PathBuf};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PathError {
    #[error("la carpeta raíz no existe o no es accesible: {0}")]
    RootUnavailable(String),
    #[error("la ruta está vacía")]
    Empty,
    #[error("no se permiten rutas absolutas ni de otra unidad")]
    Absolute,
    #[error("no se permite subir de carpeta con '..'")]
    ParentTraversal,
    #[error("nombre no permitido en Windows: {0}")]
    InvalidName(String),
    #[error("la ruta pasa por un enlace simbólico o junction: {0}")]
    LinkNotAllowed(String),
    #[error("la ruta sale de la carpeta del servidor")]
    Escapes,
    #[error("error de E/S: {0}")]
    Io(String),
}

const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2",
    "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

fn validate_name(name: &str) -> Result<(), PathError> {
    if name.is_empty() {
        return Err(PathError::Empty);
    }
    if name
        .chars()
        .any(|c| matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') || (c as u32) < 32)
    {
        return Err(PathError::InvalidName(name.to_string()));
    }
    if name.ends_with(' ') || name.ends_with('.') {
        return Err(PathError::InvalidName(name.to_string()));
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if RESERVED.contains(&stem.as_str()) {
        return Err(PathError::InvalidName(name.to_string()));
    }
    Ok(())
}

/// Quita el prefijo `\\?\` que `canonicalize` añade en Windows (`\\?\C:\...` → `C:\...`),
/// solo para rutas de unidad. Las rutas UNC verbatim se dejan tal cual.
pub fn strip_verbatim(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        let b = rest.as_bytes();
        if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
            return PathBuf::from(rest);
        }
    }
    p.to_path_buf()
}

/// Raíz canónica de una instancia.
#[derive(Debug, Clone)]
pub struct SandboxRoot {
    root: PathBuf,
}

impl SandboxRoot {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, PathError> {
        let root = root.as_ref();
        let canon =
            std::fs::canonicalize(root).map_err(|e| PathError::RootUnavailable(format!("{}: {e}", root.display())))?;
        if !canon.is_dir() {
            return Err(PathError::RootUnavailable(root.display().to_string()));
        }
        Ok(Self { root: canon })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Valida la forma de una ruta relativa sin tocar el disco y la normaliza.
    pub fn normalize_relative(rel: &str) -> Result<PathBuf, PathError> {
        let rel = rel.trim();
        if rel.is_empty() {
            return Err(PathError::Empty);
        }
        // Tratar ambos separadores igual en todas las plataformas.
        let unified = rel.replace('\\', "/");
        if unified.starts_with('/') || unified.starts_with("//") {
            return Err(PathError::Absolute);
        }
        let mut out = PathBuf::new();
        for part in unified.split('/') {
            match part {
                "" | "." => continue,
                ".." => return Err(PathError::ParentTraversal),
                p => {
                    // "C:" o "C:algo" como primer componente => unidad
                    if p.len() >= 2 && p.as_bytes()[1] == b':' {
                        return Err(PathError::Absolute);
                    }
                    validate_name(p)?;
                    out.push(p);
                }
            }
        }
        if out.as_os_str().is_empty() {
            return Err(PathError::Empty);
        }
        // Defensa adicional: el resultado solo debe tener componentes normales.
        if out.components().any(|c| !matches!(c, Component::Normal(_))) {
            return Err(PathError::Absolute);
        }
        Ok(out)
    }

    /// Resuelve `rel` dentro de la raíz. El destino puede no existir todavía (para crear archivos),
    /// pero todos los componentes existentes se verifican.
    pub fn resolve(&self, rel: &str) -> Result<PathBuf, PathError> {
        let rel = Self::normalize_relative(rel)?;
        let mut current = self.root.clone();
        for comp in rel.components() {
            current.push(comp);
            match std::fs::symlink_metadata(&current) {
                Ok(meta) => {
                    if meta.file_type().is_symlink() {
                        return Err(PathError::LinkNotAllowed(current.display().to_string()));
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    // A partir de aquí no existe nada: no puede haber enlaces.
                    break;
                }
                Err(e) => return Err(PathError::Io(e.to_string())),
            }
        }
        let full = self.root.join(&rel);
        // Comprobar la parte existente más profunda con canonicalize.
        let mut probe = full.clone();
        loop {
            if probe.exists() {
                let canon = std::fs::canonicalize(&probe).map_err(|e| PathError::Io(e.to_string()))?;
                if !canon.starts_with(&self.root) {
                    return Err(PathError::Escapes);
                }
                break;
            }
            if !probe.pop() {
                return Err(PathError::Escapes);
            }
        }
        Ok(full)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox() -> (tempfile::TempDir, SandboxRoot) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("world/region")).unwrap();
        std::fs::write(dir.path().join("server.properties"), "a=b").unwrap();
        let sb = SandboxRoot::new(dir.path()).unwrap();
        (dir, sb)
    }

    #[test]
    fn strips_verbatim_prefix() {
        assert_eq!(
            strip_verbatim(Path::new(r"\\?\C:\Servers\A")),
            PathBuf::from(r"C:\Servers\A")
        );
        assert_eq!(
            strip_verbatim(Path::new(r"\\?\UNC\srv\x")),
            PathBuf::from(r"\\?\UNC\srv\x")
        );
        assert_eq!(strip_verbatim(Path::new("/srv/a")), PathBuf::from("/srv/a"));
    }

    #[test]
    fn accepts_normal_paths() {
        let (_d, sb) = sandbox();
        assert!(sb.resolve("server.properties").is_ok());
        assert!(sb.resolve("world/region").is_ok());
        assert!(sb.resolve("world\\region\\r.0.0.mca").is_ok());
        assert!(sb.resolve("config/nuevo.toml").is_ok());
        assert!(sb.resolve("mundo con tildes ñ/á.txt").is_ok());
    }

    #[test]
    fn rejects_traversal_and_absolute() {
        let (_d, sb) = sandbox();
        assert_eq!(sb.resolve("../x"), Err(PathError::ParentTraversal));
        assert_eq!(sb.resolve("world/../../x"), Err(PathError::ParentTraversal));
        assert_eq!(sb.resolve("..\\..\\Windows"), Err(PathError::ParentTraversal));
        assert_eq!(sb.resolve("/etc/passwd"), Err(PathError::Absolute));
        assert_eq!(sb.resolve("\\Windows"), Err(PathError::Absolute));
        assert_eq!(sb.resolve("\\\\server\\share"), Err(PathError::Absolute));
        assert_eq!(sb.resolve("C:\\Windows"), Err(PathError::Absolute));
        assert_eq!(sb.resolve("C:x"), Err(PathError::Absolute));
        assert_eq!(sb.resolve(""), Err(PathError::Empty));
        assert_eq!(sb.resolve("./."), Err(PathError::Empty));
    }

    #[test]
    fn rejects_windows_invalid_names() {
        let (_d, sb) = sandbox();
        assert!(matches!(sb.resolve("CON"), Err(PathError::InvalidName(_))));
        assert!(matches!(sb.resolve("logs/nul.txt"), Err(PathError::InvalidName(_))));
        assert!(matches!(sb.resolve("a.txt:stream"), Err(PathError::InvalidName(_))));
        assert!(matches!(sb.resolve("fin."), Err(PathError::InvalidName(_))));
        assert!(matches!(sb.resolve("a*b"), Err(PathError::InvalidName(_))));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape() {
        let (d, sb) = sandbox();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secreto.txt"), "x").unwrap();
        std::os::unix::fs::symlink(outside.path(), d.path().join("enlace")).unwrap();
        assert!(matches!(
            sb.resolve("enlace/secreto.txt"),
            Err(PathError::LinkNotAllowed(_))
        ));
        assert!(matches!(sb.resolve("enlace"), Err(PathError::LinkNotAllowed(_))));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_internal_symlink_too() {
        // Incluso enlaces que apuntan dentro se rechazan: simplifica el modelo de seguridad.
        let (d, sb) = sandbox();
        std::os::unix::fs::symlink(d.path().join("world"), d.path().join("alias")).unwrap();
        assert!(matches!(sb.resolve("alias/region"), Err(PathError::LinkNotAllowed(_))));
    }

    /// En Windows, una junction (`mklink /J`) no necesita permisos de administrador y es la forma
    /// más común de "enlazar" carpetas. Debe rechazarse igual que un symlink.
    #[cfg(windows)]
    #[test]
    fn rejects_junction_escape() {
        let (d, sb) = sandbox();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secreto.txt"), "x").unwrap();
        let link = d.path().join("enlace");
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(outside.path())
            .output()
            .expect("ejecutar mklink");
        assert!(
            status.status.success(),
            "mklink /J falló: {}",
            String::from_utf8_lossy(&status.stdout)
        );
        assert!(
            std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink(),
            "std debe ver la junction como enlace"
        );
        assert!(matches!(
            sb.resolve("enlace/secreto.txt"),
            Err(PathError::LinkNotAllowed(_))
        ));
        assert!(matches!(sb.resolve("enlace"), Err(PathError::LinkNotAllowed(_))));
        std::fs::remove_dir(&link).unwrap();
    }
}
