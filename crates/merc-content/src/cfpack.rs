//! Modpacks de CurseForge (ZIP con `manifest.json` y carpeta de extras).
//!
//! El manifiesto lista los archivos por proyecto y archivo de CurseForge. Para un servidor:
//! - solo se instalan mods (los resource packs y shaders son del juego de cada jugador);
//! - se saltan los archivos que CurseForge marca solo para cliente;
//! - si el autor no permite descargas desde otras apps, el archivo no se descarga: se avisa con
//!   su nombre y su página para que la persona lo baje y lo suba a `mods/`;
//! - cada archivo se verifica con el SHA-1 que publica CurseForge.

use crate::curseforge::{CurseForge, CLASS_MODS};
use crate::modrinth::Side;
use crate::mrpack::{safe_rel, PackFile, PackIndex};
use crate::{safe_jar_name, ContentError};
use serde_json::Value;
use std::collections::HashSet;
use std::io::Read;
use std::path::Path;

const MAX_FILES: usize = 2000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfManifest {
    pub name: String,
    pub version: String,
    pub mc_version: String,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    /// (proyecto, archivo) obligatorios.
    pub files: Vec<(u64, u64)>,
    /// Carpeta de extras dentro del ZIP (normalmente `overrides`).
    pub overrides: String,
}

fn bad(msg: impl Into<String>) -> ContentError {
    ContentError::Invalid(format!("el modpack no es válido: {}", msg.into()))
}

/// `forge-47.2.0`, `neoforge-21.1.77`, `fabric-0.16.5`, `quilt-0.26.0` → (loader, versión).
pub fn parse_loader_id(id: &str) -> Option<(String, String)> {
    for (prefix, name) in [
        ("neoforge-", "neoforge"),
        ("forge-", "forge"),
        ("fabric-", "fabric"),
        ("quilt-", "quilt"),
    ] {
        if let Some(v) = id.strip_prefix(prefix) {
            // NeoForge para 1.20.1 usaba «neoforge-1.20.1-47.1.x»: la versión es la última parte.
            let v = if name == "neoforge" && v.starts_with("1.20.1-") {
                v.trim_start_matches("1.20.1-")
            } else {
                v
            };
            if !v.is_empty() {
                return Some((name.to_string(), v.to_string()));
            }
        }
    }
    None
}

pub fn parse_manifest(v: &Value) -> Result<CfManifest, ContentError> {
    if v.get("manifestType").and_then(Value::as_str) != Some("minecraftModpack") {
        return Err(bad("no es un modpack de Minecraft de CurseForge"));
    }
    let mc = v.get("minecraft").ok_or_else(|| bad("no indica Minecraft"))?;
    let mc_version = mc
        .get("version")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("no indica la versión de Minecraft"))?
        .to_string();
    let loaders: Vec<&Value> = mc
        .get("modLoaders")
        .and_then(Value::as_array)
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    let primary = loaders
        .iter()
        .find(|l| l.get("primary").and_then(Value::as_bool) == Some(true))
        .or(loaders.first())
        .and_then(|l| l.get("id").and_then(Value::as_str));
    let loader = match primary {
        Some(id) => Some(parse_loader_id(id).ok_or_else(|| bad(format!("loader desconocido «{id}»")))?),
        None => None,
    };
    let list = v
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("sin lista de archivos"))?;
    if list.len() > MAX_FILES {
        return Err(bad("demasiados archivos"));
    }
    let files = list
        .iter()
        .filter(|f| f.get("required").and_then(Value::as_bool).unwrap_or(true))
        .filter_map(|f| Some((f.get("projectID")?.as_u64()?, f.get("fileID")?.as_u64()?)))
        .collect();
    let overrides = v.get("overrides").and_then(Value::as_str).unwrap_or("overrides");
    let overrides = safe_rel(overrides).ok_or_else(|| bad("carpeta de extras no válida"))?;
    Ok(CfManifest {
        name: v.get("name").and_then(Value::as_str).unwrap_or("Modpack").to_string(),
        version: v.get("version").and_then(Value::as_str).unwrap_or_default().to_string(),
        mc_version,
        loader_version: loader.as_ref().map(|(_, v)| v.clone()),
        loader: loader.map(|(l, _)| l),
        files,
        overrides,
    })
}

pub fn read_manifest(zip_path: &Path) -> Result<CfManifest, ContentError> {
    let f = std::fs::File::open(zip_path).map_err(|e| ContentError::Io(format!("{}: {e}", zip_path.display())))?;
    let mut zip = zip::ZipArchive::new(f).map_err(|_| bad("no es un ZIP válido"))?;
    let mut entry = zip.by_name("manifest.json").map_err(|_| bad("falta manifest.json"))?;
    let mut text = String::new();
    entry
        .by_ref()
        .take(16 * 1024 * 1024)
        .read_to_string(&mut text)
        .map_err(|_| bad("manifest.json ilegible"))?;
    parse_manifest(&serde_json::from_str(&text).map_err(|_| bad("manifest.json no es JSON"))?)
}

/// Resuelve los archivos del manifiesto con la API de CurseForge. Devuelve el índice para el
/// servidor (mismo formato que un `.mrpack`) y los avisos (archivos a descargar a mano, etc.).
pub fn resolve(cf: &CurseForge, m: &CfManifest) -> Result<(PackIndex, Vec<String>), ContentError> {
    let file_ids: Vec<u64> = m.files.iter().map(|(_, f)| *f).collect();
    let project_ids: Vec<u64> = {
        let mut v: Vec<u64> = m.files.iter().map(|(p, _)| *p).collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    let files = cf.files_by_ids(&file_ids)?;
    let classes = cf.classes(&project_ids)?;
    let mut out = vec![];
    let mut warnings = vec![];
    let mut manual = vec![];
    let mut client_only = 0;
    let mut not_mods = 0;
    let mut names = HashSet::new();
    for (pid, fid) in &m.files {
        let Some(file) = files.iter().find(|f| f.id == fid.to_string()) else {
            warnings.push(format!(
                "CurseForge no devolvió el archivo {fid} del proyecto {pid}; se omite."
            ));
            continue;
        };
        let (class, project) = match classes.get(pid) {
            Some((c, p)) => (*c, Some(p)),
            None => (CLASS_MODS, None),
        };
        if class != CLASS_MODS {
            not_mods += 1;
            continue;
        }
        if file.server_side == Side::Unsupported {
            client_only += 1;
            continue;
        }
        let Some(f) = file.files.first() else { continue };
        let title = project.map(|p| p.title.clone()).unwrap_or_else(|| f.filename.clone());
        let page = project.map(|p| p.page_url.clone()).unwrap_or_default();
        let allowed = project.is_none_or(|p| p.distribution_allowed);
        let Ok(name) = safe_jar_name(&f.filename) else {
            warnings.push(format!("«{title}» tiene un nombre de archivo no válido; se omite."));
            continue;
        };
        if !names.insert(name.to_lowercase()) {
            continue;
        }
        match (&f.sha1, f.url.is_empty() || !allowed) {
            (Some(sha1), false) if sha1.len() == 40 => out.push(PackFile {
                path: format!("mods/{name}"),
                sha1: sha1.clone(),
                size: f.size,
                urls: vec![f.url.clone()],
                optional: false,
            }),
            _ => manual.push(format!(
                "«{title}» ({name}){}",
                if page.is_empty() {
                    String::new()
                } else {
                    format!(": {page}")
                }
            )),
        }
    }
    if !manual.is_empty() {
        warnings.insert(
            0,
            format!(
                "{} mod{} no se pueden descargar desde otras apps (lo decidió su autor). El servidor puede no arrancar sin ellos: descárgalos desde su página y súbelos en Archivos › mods. {}",
                manual.len(),
                if manual.len() == 1 { "" } else { "s" },
                manual.join("; ")
            ),
        );
    }
    if not_mods > 0 {
        warnings.push(format!(
            "{not_mods} archivo{} (resource packs, shaders…) {} para el juego de cada jugador y no se instala{} en el servidor.",
            if not_mods == 1 { "" } else { "s" },
            if not_mods == 1 { "es" } else { "son" },
            if not_mods == 1 { "" } else { "n" }
        ));
    }
    Ok((
        PackIndex {
            name: m.name.clone(),
            version_id: m.version.clone(),
            summary: String::new(),
            mc_version: m.mc_version.clone(),
            loader: m.loader.clone(),
            loader_version: m.loader_version.clone(),
            files: out,
            client_only,
        },
        warnings,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_manifest_and_loader_ids() {
        let m = parse_manifest(&json!({
            "manifestType": "minecraftModpack", "manifestVersion": 1, "name": "Pack", "version": "1.2",
            "minecraft": { "version": "1.20.1", "modLoaders": [{ "id": "forge-47.2.0", "primary": true }] },
            "files": [
                { "projectID": 1, "fileID": 10, "required": true },
                { "projectID": 2, "fileID": 20, "required": false }
            ],
            "overrides": "overrides"
        }))
        .unwrap();
        assert_eq!(
            (m.loader.as_deref(), m.loader_version.as_deref()),
            (Some("forge"), Some("47.2.0"))
        );
        assert_eq!(m.files, [(1, 10)]);
        assert_eq!(
            parse_loader_id("neoforge-21.1.77"),
            Some(("neoforge".into(), "21.1.77".into()))
        );
        assert_eq!(
            parse_loader_id("neoforge-1.20.1-47.1.106"),
            Some(("neoforge".into(), "47.1.106".into()))
        );
        assert_eq!(
            parse_loader_id("fabric-0.16.5"),
            Some(("fabric".into(), "0.16.5".into()))
        );
        assert!(parse_loader_id("rift-1").is_none());
        let mut bad = json!({ "manifestType": "minecraftModpack", "minecraft": { "version": "1.20.1" }, "files": [], "overrides": "../x" });
        assert!(parse_manifest(&bad).is_err());
        bad["overrides"] = json!("overrides");
        bad["manifestType"] = json!("otra");
        assert!(parse_manifest(&bad).is_err());
    }
}
