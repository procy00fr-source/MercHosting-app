//! Modpacks de Modrinth (`.mrpack`), según la documentación oficial del formato de Modrinth.
//!
//! Un `.mrpack` es un ZIP con `modrinth.index.json` (lista de archivos con huellas, URLs y si
//! hacen falta en cliente/servidor, más la versión de Minecraft y del loader) y carpetas
//! `overrides/` (para todos) y `server-overrides/` (solo servidor, tienen prioridad).
//!
//! Para un servidor:
//! - se descargan solo los archivos que no están marcados como «no compatibles» con servidor;
//! - cada descarga se verifica con su SHA-1 y solo desde los dominios que permite el formato;
//! - nunca se copia `eula.txt` (la EULA la acepta la persona) ni se pisa `server.properties`
//!   (se conservan los ajustes elegidos en el asistente).

use crate::{ContentError, ContentSource};
use merc_install::{Checksum, Http};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Dominios desde los que el formato permite descargar.
const ALLOWED_HOSTS: [&str; 4] = [
    "cdn.modrinth.com",
    "github.com",
    "raw.githubusercontent.com",
    "gitlab.com",
];
const MAX_FILES: usize = 2000;
const MAX_INDEX_BYTES: u64 = 16 * 1024 * 1024;
/// Límite de lo que se extrae de `overrides` (protege de «bombas» de compresión).
const MAX_OVERRIDES_BYTES: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackFile {
    pub path: String,
    pub sha1: String,
    pub size: u64,
    pub urls: Vec<String>,
    /// `env.server` es `optional` (se instala igual: casi siempre son mejoras).
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackIndex {
    pub name: String,
    pub version_id: String,
    pub summary: String,
    pub mc_version: String,
    /// `fabric`, `quilt`, `forge` o `neoforge`; `None` si es vanilla.
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    /// Archivos para el servidor.
    pub files: Vec<PackFile>,
    /// Archivos solo para el cliente (no se descargan).
    pub client_only: usize,
}

/// Resumen para la interfaz antes de crear el servidor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModpackSummary {
    pub source: ContentSource,
    /// Tipo de servidor que se creará.
    pub flavor: merc_install::Flavor,
    pub name: String,
    pub pack_version: String,
    pub mc_version: String,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub java_major: u32,
    pub server_files: u32,
    pub client_only_files: u32,
    pub download_bytes: u64,
    pub warnings: Vec<String>,
}

fn bad(msg: impl Into<String>) -> ContentError {
    ContentError::Invalid(format!("el modpack no es válido: {}", msg.into()))
}

/// Ruta relativa segura dentro del servidor: sin `..`, sin raíz ni unidad, con `/`.
pub fn safe_rel(path: &str) -> Option<String> {
    if path.is_empty() || path.contains('\\') || path.contains(':') || path.starts_with('/') {
        return None;
    }
    let p = Path::new(path);
    let mut parts = vec![];
    for c in p.components() {
        match c {
            Component::Normal(s) => parts.push(s.to_str()?.to_string()),
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

pub fn url_allowed(url: &str) -> bool {
    // Solo en compilaciones de desarrollo y si se pide expresamente (pruebas con un servidor local).
    if cfg!(debug_assertions)
        && std::env::var_os("MERC_TEST_LOOPBACK").is_some()
        && url.starts_with("http://127.0.0.1:")
    {
        return true;
    }
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    ALLOWED_HOSTS.contains(&host)
}

pub fn parse_index(v: &Value) -> Result<PackIndex, ContentError> {
    if v.get("formatVersion").and_then(Value::as_u64) != Some(1) {
        return Err(bad("versión de formato desconocida"));
    }
    if v.get("game").and_then(Value::as_str) != Some("minecraft") {
        return Err(bad("no es de Minecraft"));
    }
    let deps = v
        .get("dependencies")
        .and_then(Value::as_object)
        .ok_or_else(|| bad("sin dependencias"))?;
    let mc_version = deps
        .get("minecraft")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("no indica la versión de Minecraft"))?
        .to_string();
    let mut loader = None;
    for (key, name) in [
        ("neoforge", "neoforge"),
        ("forge", "forge"),
        ("fabric-loader", "fabric"),
        ("quilt-loader", "quilt"),
    ] {
        if let Some(ver) = deps.get(key).and_then(Value::as_str) {
            if loader.is_some() {
                return Err(bad("pide dos loaders a la vez"));
            }
            loader = Some((name.to_string(), ver.to_string()));
        }
    }
    let list = v
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("sin lista de archivos"))?;
    if list.len() > MAX_FILES {
        return Err(bad("demasiados archivos"));
    }
    let mut files = vec![];
    let mut client_only = 0;
    for f in list {
        let raw = f.get("path").and_then(Value::as_str).unwrap_or_default();
        let path = safe_rel(raw).ok_or_else(|| bad(format!("ruta no permitida «{raw}»")))?;
        let server = f
            .get("env")
            .and_then(|e| e.get("server"))
            .and_then(Value::as_str)
            .unwrap_or("required");
        if server == "unsupported" {
            client_only += 1;
            continue;
        }
        let sha1 = f
            .get("hashes")
            .and_then(|h| h.get("sha1"))
            .and_then(Value::as_str)
            .filter(|h| h.len() == 40)
            .ok_or_else(|| bad(format!("«{path}» no tiene huella SHA-1")))?
            .to_lowercase();
        let urls: Vec<String> = f
            .get("downloads")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .filter(|u| url_allowed(u))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        if urls.is_empty() {
            return Err(bad(format!("«{path}» no se puede descargar de un sitio permitido")));
        }
        files.push(PackFile {
            path,
            sha1,
            size: f.get("fileSize").and_then(Value::as_u64).unwrap_or(0),
            urls,
            optional: server == "optional",
        });
    }
    Ok(PackIndex {
        name: v.get("name").and_then(Value::as_str).unwrap_or("Modpack").to_string(),
        version_id: v
            .get("versionId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        summary: v.get("summary").and_then(Value::as_str).unwrap_or_default().to_string(),
        mc_version,
        loader_version: loader.as_ref().map(|(_, v)| v.clone()),
        loader: loader.map(|(l, _)| l),
        files,
        client_only,
    })
}

fn open_zip(mrpack: &Path) -> Result<zip::ZipArchive<std::fs::File>, ContentError> {
    let f = std::fs::File::open(mrpack).map_err(|e| ContentError::Io(format!("{}: {e}", mrpack.display())))?;
    zip::ZipArchive::new(f).map_err(|_| bad("no es un archivo .mrpack (ZIP) válido"))
}

pub fn read_index(mrpack: &Path) -> Result<PackIndex, ContentError> {
    let mut zip = open_zip(mrpack)?;
    let mut entry = zip
        .by_name("modrinth.index.json")
        .map_err(|_| bad("falta modrinth.index.json"))?;
    let mut text = String::new();
    entry
        .by_ref()
        .take(MAX_INDEX_BYTES)
        .read_to_string(&mut text)
        .map_err(|_| bad("modrinth.index.json ilegible"))?;
    parse_index(&serde_json::from_str(&text).map_err(|_| bad("modrinth.index.json no es JSON"))?)
}

pub fn flavor_of(loader: Option<&str>) -> merc_install::Flavor {
    use merc_install::Flavor;
    match loader {
        Some("fabric") => Flavor::Fabric,
        Some("quilt") => Flavor::Quilt,
        Some("forge") => Flavor::Forge,
        Some("neoforge") => Flavor::NeoForge,
        _ => Flavor::Vanilla,
    }
}

pub fn summarize(index: &PackIndex) -> ModpackSummary {
    let mut warnings = vec![];
    if index.loader.is_none() {
        warnings.push("Este modpack no usa loader de mods: se creará un servidor Vanilla.".into());
    }
    if index.client_only > 0 {
        warnings.push(format!(
            "{} archivo{} solo para el juego de cada jugador no se instala{} en el servidor.",
            index.client_only,
            if index.client_only == 1 { "" } else { "s" },
            if index.client_only == 1 { "" } else { "n" }
        ));
    }
    warnings.push("Tus amigos necesitan el mismo modpack instalado en su juego para entrar.".into());
    ModpackSummary {
        source: ContentSource::Modrinth,
        flavor: flavor_of(index.loader.as_deref()),
        name: index.name.clone(),
        pack_version: index.version_id.clone(),
        mc_version: index.mc_version.clone(),
        loader: index.loader.clone(),
        loader_version: index.loader_version.clone(),
        java_major: merc_core::java::recommended_java_major(Some(&index.mc_version)),
        server_files: index.files.len() as u32,
        client_only_files: index.client_only as u32,
        download_bytes: index.files.iter().map(|f| f.size).sum(),
        warnings,
    }
}

/// Descarga a `staging` los archivos del servidor, verificados. No toca el servidor.
pub fn stage_files(
    http: &Http,
    index: &PackIndex,
    staging: &Path,
    on_file: &mut dyn FnMut(&str, u64, u64),
    cancel: &AtomicBool,
) -> Result<(), ContentError> {
    let done = AtomicU64::new(0);
    let total: u64 = index.files.iter().map(|f| f.size).sum();
    for f in &index.files {
        if cancel.load(Ordering::Relaxed) {
            return Err(ContentError::Cancelled);
        }
        let dest = staging.join(&f.path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ContentError::Io(format!("{}: {e}", parent.display())))?;
        }
        let before = done.load(Ordering::Relaxed);
        let mut last_err = None;
        for url in &f.urls {
            let mut cb = |d: u64, _: Option<u64>| on_file(&f.path, before + d, total);
            match http.download(
                url,
                &dest,
                &Checksum::Sha1(f.sha1.clone()),
                Some(f.size).filter(|s| *s > 0),
                &mut cb,
                cancel,
            ) {
                Ok(_) => {
                    last_err = None;
                    break;
                }
                Err(merc_install::InstallError::Cancelled) => return Err(ContentError::Cancelled),
                Err(e) => last_err = Some(e),
            }
        }
        if let Some(e) = last_err {
            return Err(e.into());
        }
        done.fetch_add(f.size, Ordering::Relaxed);
    }
    Ok(())
}

/// Archivos de `overrides` que nunca se copian al servidor.
fn protected(rel: &str) -> bool {
    let lower = rel.to_lowercase();
    lower == "eula.txt" || lower == "server.properties"
}

/// Copia lo descargado y extrae `overrides/` y luego `server-overrides/` en el servidor.
/// Devuelve avisos (p. ej. si el modpack traía su propio `server.properties`).
pub fn apply(mrpack: &Path, staging: &Path, target: &Path) -> Result<Vec<String>, ContentError> {
    apply_with(mrpack, staging, target, &["overrides/", "server-overrides/"])
}

/// Como [`apply`], con las carpetas de extras que use el formato (en orden de prioridad creciente).
pub fn apply_with(pack: &Path, staging: &Path, target: &Path, prefixes: &[&str]) -> Result<Vec<String>, ContentError> {
    let mrpack = pack;
    let mut notes = vec![];
    // 1. Archivos descargados.
    let mut stack = vec![staging.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir)
            .map_err(|e| ContentError::Io(e.to_string()))?
            .flatten()
        {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let rel = p.strip_prefix(staging).map_err(|_| ContentError::Io("ruta".into()))?;
            let dst = target.join(rel);
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent).map_err(|e| ContentError::Io(e.to_string()))?;
            }
            if std::fs::rename(&p, &dst).is_err() {
                std::fs::copy(&p, &dst).map_err(|e| ContentError::Io(format!("{}: {e}", dst.display())))?;
            }
        }
    }
    // 2. Overrides (primero los comunes, después los del servidor, que tienen prioridad).
    let mut zip = open_zip(mrpack)?;
    let mut total: u64 = 0;
    for prefix in prefixes {
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|e| bad(e.to_string()))?;
            let Some(name) = f.enclosed_name() else { continue };
            let name = name.to_string_lossy().replace('\\', "/");
            let Some(rel) = name.strip_prefix(prefix) else { continue };
            let Some(rel) = safe_rel(rel.trim_end_matches('/')) else {
                continue;
            };
            if protected(&rel) {
                let msg = format!(
                    "El modpack trae su propio «{rel}»: no se copió (se conservan tus ajustes y la EULA la aceptas tú)."
                );
                if !notes.contains(&msg) {
                    notes.push(msg);
                }
                continue;
            }
            let dst: PathBuf = target.join(&rel);
            if f.is_dir() {
                std::fs::create_dir_all(&dst).map_err(|e| ContentError::Io(e.to_string()))?;
                continue;
            }
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent).map_err(|e| ContentError::Io(e.to_string()))?;
            }
            let mut out =
                std::fs::File::create(&dst).map_err(|e| ContentError::Io(format!("{}: {e}", dst.display())))?;
            let n = std::io::copy(&mut f.by_ref().take(MAX_OVERRIDES_BYTES - total + 1), &mut out)
                .map_err(|e| ContentError::Io(e.to_string()))?;
            total += n;
            if total > MAX_OVERRIDES_BYTES {
                return Err(bad("los archivos extra son demasiado grandes"));
            }
        }
    }
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn index() -> Value {
        json!({
            "formatVersion": 1, "game": "minecraft", "versionId": "1.2.0", "name": "Aventura",
            "dependencies": { "minecraft": "1.21.1", "neoforge": "21.1.77" },
            "files": [
                { "path": "mods/a.jar", "hashes": { "sha1": "a".repeat(40), "sha512": "x" },
                  "env": { "client": "required", "server": "required" },
                  "downloads": ["https://cdn.modrinth.com/data/a.jar"], "fileSize": 10 },
                { "path": "mods/shaders.jar", "hashes": { "sha1": "b".repeat(40) },
                  "env": { "client": "required", "server": "unsupported" },
                  "downloads": ["https://cdn.modrinth.com/data/b.jar"], "fileSize": 5 },
                { "path": "config/x.toml", "hashes": { "sha1": "c".repeat(40) },
                  "downloads": ["https://evil.example/x", "https://github.com/a/b/x.toml"], "fileSize": 3 }
            ]
        })
    }

    #[test]
    fn parses_server_files_and_loader() {
        let i = parse_index(&index()).unwrap();
        assert_eq!(
            (i.mc_version.as_str(), i.loader.as_deref(), i.loader_version.as_deref()),
            ("1.21.1", Some("neoforge"), Some("21.1.77"))
        );
        assert_eq!(i.files.len(), 2);
        assert_eq!(i.client_only, 1);
        assert_eq!(
            i.files[1].urls,
            ["https://github.com/a/b/x.toml"],
            "solo dominios permitidos"
        );
        let s = summarize(&i);
        assert_eq!(s.java_major, 21);
        assert_eq!(s.download_bytes, 13);
        assert!(s.warnings.iter().any(|w| w.contains("1 archivo solo para el juego")));
    }

    #[test]
    fn rejects_unsafe_packs() {
        let mut v = index();
        v["files"][0]["path"] = json!("../../Windows/x.jar");
        assert!(parse_index(&v).is_err());
        let mut v = index();
        v["files"][0]["downloads"] = json!(["http://cdn.modrinth.com/a.jar"]);
        assert!(parse_index(&v).is_err(), "sin HTTPS no se acepta");
        let mut v = index();
        v["dependencies"]["fabric-loader"] = json!("0.16.0");
        assert!(parse_index(&v).is_err(), "dos loaders");
        assert_eq!(safe_rel("mods/x.jar").as_deref(), Some("mods/x.jar"));
        for bad in ["/etc/x", "C:/x", "a\\b", "", "a/../b"] {
            assert!(safe_rel(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn apply_copies_overrides_but_not_eula_or_properties() {
        use std::io::Write;
        let d = tempfile::tempdir().unwrap();
        let pack = d.path().join("p.mrpack");
        {
            let mut z = zip::ZipWriter::new(std::fs::File::create(&pack).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            for (name, body) in [
                ("modrinth.index.json", "{}"),
                ("overrides/config/a.toml", "comun"),
                ("server-overrides/config/a.toml", "servidor"),
                ("overrides/eula.txt", "eula=true"),
                ("server-overrides/server.properties", "pvp=false"),
                ("overrides/../../fuera.txt", "x"),
            ] {
                z.start_file(name, o).unwrap();
                z.write_all(body.as_bytes()).unwrap();
            }
            z.finish().unwrap();
        }
        let staging = d.path().join("staging");
        std::fs::create_dir_all(staging.join("mods")).unwrap();
        std::fs::write(staging.join("mods/a.jar"), "jar").unwrap();
        let target = d.path().join("servidor");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("server.properties"), "motd=mio").unwrap();
        let notes = apply(&pack, &staging, &target).unwrap();
        assert_eq!(std::fs::read_to_string(target.join("mods/a.jar")).unwrap(), "jar");
        assert_eq!(
            std::fs::read_to_string(target.join("config/a.toml")).unwrap(),
            "servidor"
        );
        assert!(!target.join("eula.txt").exists());
        assert_eq!(
            std::fs::read_to_string(target.join("server.properties")).unwrap(),
            "motd=mio"
        );
        assert!(!d.path().join("fuera.txt").exists());
        assert_eq!(notes.len(), 2);
    }
}
