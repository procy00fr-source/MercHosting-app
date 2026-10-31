//! Administración local de un servidor ya registrado.
//!
//! - [`props`]: lectura y edición de `server.properties` conservando comentarios y orden.
//! - [`players`]: listas de lista blanca, operadores y baneos, y comandos validados.
//! - [`files`]: explorador confinado a la carpeta del servidor, con copia previa y papelera.
//!
//! Regla común: nada se borra de golpe ni se sobrescribe sin copia de seguridad.

pub mod backups;
pub mod diagnose;
pub mod files;
pub mod gamerules;
pub mod health;
pub mod icon;
pub mod luckperms;
pub mod modmeta;
pub mod modsides;
pub mod players;
pub mod props;
pub mod schedule;
pub mod security;
pub mod voicechat;
pub mod worlds;

/// Tamaño de una carpeta (sin seguir enlaces).
pub(crate) fn worlds_size(p: &std::path::Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(p) else {
        return 0;
    };
    if meta.file_type().is_symlink() {
        return 0;
    }
    if meta.is_file() {
        return meta.len();
    }
    std::fs::read_dir(p)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| worlds_size(&e.path()))
        .sum()
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum AdminError {
    #[error("{0}")]
    Invalid(String),
    #[error("no existe: {0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Path(String),
    #[error("error de archivos: {0}")]
    Io(String),
    /// El usuario canceló la operación; lo que se había hecho a medias se deshizo.
    #[error("operación cancelada; se deshizo lo que se había hecho")]
    Cancelled,
}

impl From<merc_core::paths::PathError> for AdminError {
    fn from(e: merc_core::paths::PathError) -> Self {
        AdminError::Path(e.to_string())
    }
}

pub(crate) fn io(p: &std::path::Path, e: std::io::Error) -> AdminError {
    if e.kind() == std::io::ErrorKind::NotFound {
        AdminError::NotFound(p.display().to_string())
    } else {
        AdminError::Io(format!("{}: {e}", p.display()))
    }
}

/// Marca de tiempo para nombres de copias: `20261030-031500-123`.
pub fn stamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs() as i64;
    let (y, mo, d) = civil_from_days(secs.div_euclid(86_400));
    let rem = secs.rem_euclid(86_400);
    format!(
        "{y:04}{mo:02}{d:02}-{:02}{:02}{:02}-{:03}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60,
        now.subsec_millis()
    )
}

/// Fecha civil (UTC) a partir de días desde 1970-01-01 (algoritmo de H. Hinnant).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    #[test]
    fn civil_dates() {
        assert_eq!(super::civil_from_days(0), (1970, 1, 1));
        assert_eq!(super::civil_from_days(20_756), (2026, 10, 30));
        assert_eq!(super::stamp().len(), 19);
    }
}
