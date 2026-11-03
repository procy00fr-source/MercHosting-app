//! Floodgate para Spigot/Paper/Purpur desde la API oficial de descargas de GeyserMC.
//!
//! En Modrinth, Floodgate solo está publicado para Fabric y NeoForge: la versión de plugin se
//! publica en `download.geysermc.org`, con su SHA-256. Se consulta la compilación más reciente y
//! se descarga por su dirección fija (versión y número), comprobando esa huella.

use crate::{safe_jar_name, ContentError};
use merc_install::{Checksum, Http};
use serde_json::Value;
use std::path::Path;
use std::sync::atomic::AtomicBool;

pub const GEYSERMC_API: &str = "https://download.geysermc.org/v2";

/// Una compilación de Floodgate para Spigot/Paper en la API de GeyserMC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloodgateBuild {
    pub version: String,
    pub build: u64,
    pub file_name: String,
    pub sha256: String,
}

impl FloodgateBuild {
    /// Dirección fija de esta compilación (no «latest»: así la huella corresponde al archivo).
    pub fn url(&self, base: &str) -> String {
        format!(
            "{}/projects/floodgate/versions/{}/builds/{}/downloads/spigot",
            base.trim_end_matches('/'),
            self.version,
            self.build
        )
    }
}

fn bad() -> ContentError {
    ContentError::BadResponse("GeyserMC respondió algo inesperado sobre Floodgate.".into())
}

/// Lee la respuesta de `/projects/floodgate/versions/latest/builds/latest`
/// (`{"version":"2.2.5","build":120,"downloads":{"spigot":{"name":…,"sha256":…}}}`).
pub fn parse_floodgate_build(v: &Value) -> Result<FloodgateBuild, ContentError> {
    let version = v.get("version").and_then(Value::as_str).ok_or_else(bad)?;
    if version.is_empty()
        || version.len() > 40
        || !version.chars().all(|c| c.is_ascii_alphanumeric() || ".-+_".contains(c))
    {
        return Err(bad());
    }
    let build = v.get("build").and_then(Value::as_u64).ok_or_else(bad)?;
    let spigot = v.get("downloads").and_then(|d| d.get("spigot")).ok_or_else(bad)?;
    let name = spigot.get("name").and_then(Value::as_str).ok_or_else(bad)?;
    let file_name = safe_jar_name(name).map_err(|_| bad())?;
    let sha256 = spigot
        .get("sha256")
        .and_then(Value::as_str)
        .ok_or_else(bad)?
        .to_ascii_lowercase();
    if sha256.len() != 64 || !sha256.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(bad());
    }
    Ok(FloodgateBuild {
        version: version.to_string(),
        build,
        file_name,
        sha256,
    })
}

/// La compilación más reciente de Floodgate para Spigot/Paper (sin descargarla).
pub fn floodgate_latest(http: &Http, base: &str) -> Result<FloodgateBuild, ContentError> {
    let url = format!(
        "{}/projects/floodgate/versions/latest/builds/latest",
        base.trim_end_matches('/')
    );
    let v = http
        .get_json(&url)
        .map_err(|e| ContentError::Network(format!("download.geysermc.org: {e}")))?;
    parse_floodgate_build(&v)
}

/// Descarga Floodgate a `<root>/<folder>/` comprobando su SHA-256. Primero va a `staging`
/// (fuera del servidor) y solo con la huella correcta se mueve. Si ya existe un archivo con ese
/// nombre, no se toca y se avisa.
pub fn install_floodgate(
    http: &Http,
    base: &str,
    root: &Path,
    folder: &str,
    staging: &Path,
    fg: &FloodgateBuild,
    cancel: &AtomicBool,
) -> Result<String, ContentError> {
    let io = |p: &Path, e: std::io::Error| ContentError::Io(format!("{}: {e}", p.display()));
    let dir = root.join(folder);
    let dest = dir.join(&fg.file_name);
    if dest.exists() {
        return Err(ContentError::Conflict(format!(
            "Ya hay un archivo «{}» en {folder}.",
            fg.file_name
        )));
    }
    std::fs::create_dir_all(staging).map_err(|e| io(staging, e))?;
    let tmp = staging.join(&fg.file_name);
    let r = http.download(
        &fg.url(base),
        &tmp,
        &Checksum::Sha256(fg.sha256.clone()),
        None,
        &mut |_, _| {},
        cancel,
    );
    if let Err(e) = r {
        let _ = std::fs::remove_dir_all(staging);
        return Err(ContentError::from(e));
    }
    std::fs::create_dir_all(&dir).map_err(|e| io(&dir, e))?;
    if std::fs::rename(&tmp, &dest).is_err() {
        std::fs::copy(&tmp, &dest).map_err(|e| io(&dest, e))?;
    }
    let _ = std::fs::remove_dir_all(staging);
    Ok(fg.file_name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_official_shape_and_rejects_odd_values() {
        let v = serde_json::json!({
            "project_id": "floodgate", "version": "2.2.5", "build": 120,
            "downloads": {"spigot": {"name": "floodgate-spigot.jar", "sha256": "A".repeat(64)},
                          "velocity": {"name": "floodgate-velocity.jar", "sha256": "b".repeat(64)}}
        });
        let fg = parse_floodgate_build(&v).unwrap();
        assert_eq!(fg.file_name, "floodgate-spigot.jar");
        assert_eq!(fg.sha256, "a".repeat(64));
        assert_eq!(
            fg.url(GEYSERMC_API),
            "https://download.geysermc.org/v2/projects/floodgate/versions/2.2.5/builds/120/downloads/spigot"
        );
        for (path, value) in [
            ("name", serde_json::json!("../x.jar")),
            ("sha256", serde_json::json!("corta")),
        ] {
            let mut bad = v.clone();
            bad["downloads"]["spigot"][path] = value;
            assert!(parse_floodgate_build(&bad).is_err(), "{path}");
        }
        let mut bad = v;
        bad["version"] = serde_json::json!("2.2/../x");
        assert!(parse_floodgate_build(&bad).is_err());
    }
}
