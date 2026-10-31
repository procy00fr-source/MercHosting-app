//! Lo instalado en la carpeta `mods/` o `plugins/` del servidor.
//!
//! - Desactivar = renombrar `x.jar` a `x.jar.disabled` (Minecraft y Bukkit solo cargan `.jar`);
//!   activar lo deshace. No se borra nada.
//! - Instalar = descargar todo a una carpeta temporal fuera del servidor, verificar cada SHA-1 y
//!   solo entonces moverlo; lo que se reemplaza va antes a la papelera de MercHosting.

use crate::modrinth::{Side, Source};
use crate::plan::ContentPlan;
use crate::{safe_file_name, ContentError, ContentSource, Platform};
use merc_install::{Checksum, Http};
use serde::{Deserialize, Serialize};
use sha1::Digest as _;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub const DISABLED_SUFFIX: &str = ".disabled";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub version_id: String,
    pub version_number: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledItem {
    /// Nombre del archivo tal como está (con `.disabled` si está desactivado).
    pub file_name: String,
    pub enabled: bool,
    pub size: u64,
    pub sha1: String,
    /// Datos de Modrinth, si el archivo es de Modrinth y hubo conexión.
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub title: Option<String>,
    pub version_number: Option<String>,
    pub icon_url: Option<String>,
    pub server_side: Option<Side>,
    /// Si los jugadores lo necesitan en su juego (Modrinth lo informa).
    #[serde(default)]
    pub client_side: Option<Side>,
    /// Página oficial del proyecto (para que los jugadores lo descarguen de ahí).
    #[serde(default)]
    pub page_url: Option<String>,
    pub update: Option<UpdateInfo>,
    /// Catálogo del que se reconoció el archivo.
    #[serde(default)]
    pub source: Option<ContentSource>,
    /// Identificador del mismo proyecto en el otro catálogo, si también se reconoció ahí.
    #[serde(default)]
    pub other_ids: Vec<String>,
}

fn io(p: &Path, e: std::io::Error) -> ContentError {
    ContentError::Io(format!("{}: {e}", p.display()))
}

pub fn sha1_file(p: &Path) -> Result<String, ContentError> {
    let mut f = std::fs::File::open(p).map_err(|e| io(p, e))?;
    let mut h = sha1::Sha1::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(|e| io(p, e))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Archivos `.jar` (o la extensión indicada) y `.jar.disabled` de la carpeta (sin entrar en
/// subcarpetas ni seguir enlaces).
pub fn list_local(root: &Path, folder: &str, ext: &str) -> Result<Vec<InstalledItem>, ContentError> {
    let dir = root.join(folder);
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Ok(vec![]);
    };
    let mut out = vec![];
    for e in rd.flatten() {
        let Ok(meta) = std::fs::symlink_metadata(e.path()) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let Ok(name) = e.file_name().into_string() else {
            continue;
        };
        let lower = name.to_lowercase();
        let enabled = lower.ends_with(ext);
        if !enabled && !lower.ends_with(&format!("{ext}{DISABLED_SUFFIX}")) {
            continue;
        }
        out.push(InstalledItem {
            sha1: sha1_file(&e.path())?,
            file_name: name,
            enabled,
            size: meta.len(),
            project_id: None,
            version_id: None,
            title: None,
            version_number: None,
            icon_url: None,
            server_side: None,
            client_side: None,
            page_url: None,
            update: None,
            source: None,
            other_ids: vec![],
        });
    }
    out.sort_by_key(|i| i.file_name.to_lowercase());
    Ok(out)
}

/// Completa lo instalado con datos de Modrinth (nombre, icono, versión y actualización).
/// Si no hay conexión devuelve el error y la lista queda solo con los datos locales.
pub fn enrich(
    source: &dyn Source,
    platform: &Platform,
    mc: Option<&str>,
    items: &mut [InstalledItem],
) -> Result<(), ContentError> {
    let hashes: Vec<String> = items.iter().map(|i| i.sha1.clone()).collect();
    let found = source.versions_by_sha1(&hashes)?;
    let mut ids: Vec<String> = found.values().map(|v| v.project_id.clone()).collect();
    ids.sort();
    ids.dedup();
    let projects = source.projects(&ids)?;
    let enabled_known: Vec<String> = items
        .iter()
        .filter(|i| i.enabled && found.contains_key(&i.sha1))
        .map(|i| i.sha1.clone())
        .collect();
    // Las actualizaciones son un extra: si falla esa consulta, se sigue sin ellas.
    let latest = source
        .latest_by_sha1(&enabled_known, &platform.loaders, mc)
        .unwrap_or_default();
    for it in items.iter_mut() {
        let Some(v) = found.get(&it.sha1) else { continue };
        it.source = Some(ContentSource::Modrinth);
        it.project_id = Some(v.project_id.clone());
        it.version_id = Some(v.id.clone());
        it.version_number = Some(v.version_number.clone());
        if let Some(p) = projects.iter().find(|p| p.id == v.project_id) {
            it.title = Some(p.title.clone());
            it.icon_url = p.icon_url.clone();
            it.server_side = Some(p.server_side);
            it.client_side = Some(p.client_side);
            it.page_url = Some(p.page_url.clone());
        }
        if let Some(l) = latest.get(&it.sha1) {
            if l.id != v.id && l.project_id == v.project_id {
                it.update = Some(UpdateInfo {
                    version_id: l.id.clone(),
                    version_number: l.version_number.clone(),
                });
            }
        }
    }
    Ok(())
}

/// Activa o desactiva un archivo renombrándolo. Devuelve el nombre nuevo.
pub fn set_enabled(
    root: &Path,
    folder: &str,
    ext: &str,
    file_name: &str,
    enabled: bool,
) -> Result<String, ContentError> {
    let base = file_name.strip_suffix(DISABLED_SUFFIX).unwrap_or(file_name);
    let base = safe_file_name(base, ext)?;
    let dir = root.join(folder);
    let (from, to) = if enabled {
        (format!("{base}{DISABLED_SUFFIX}"), base.clone())
    } else {
        (base.clone(), format!("{base}{DISABLED_SUFFIX}"))
    };
    let from_p = dir.join(&from);
    let to_p = dir.join(&to);
    if !from_p.is_file() {
        return Err(ContentError::NotFound(from));
    }
    if to_p.exists() {
        return Err(ContentError::Conflict(format!(
            "ya existe «{to}»; quita uno de los dos antes"
        )));
    }
    std::fs::rename(&from_p, &to_p).map_err(|e| io(&from_p, e))?;
    Ok(to)
}

/// Progreso de una instalación (bytes descargados de un total) y cancelación.
#[derive(Debug, Default)]
pub struct InstallProgress {
    pub done: AtomicU64,
    pub total: AtomicU64,
    pub cancel: AtomicBool,
}

/// Instala un plan sin bloqueos. `staging` es una carpeta temporal fuera del servidor (se borra
/// al terminar). `to_trash` manda a la papelera de MercHosting una ruta relativa al servidor.
#[allow(clippy::too_many_arguments)]
pub fn apply_plan(
    http: &Http,
    root: &Path,
    folder: &str,
    ext: &str,
    plan: &ContentPlan,
    staging: &Path,
    to_trash: &mut dyn FnMut(&str) -> Result<(), String>,
    progress: &InstallProgress,
) -> Result<Vec<String>, ContentError> {
    if !plan.blockers.is_empty() {
        return Err(ContentError::Invalid(plan.blockers.join(" ")));
    }
    if plan.items.is_empty() {
        return Err(ContentError::Invalid("no hay nada que instalar".into()));
    }
    progress.total.store(plan.download_bytes, Ordering::Relaxed);
    std::fs::create_dir_all(staging).map_err(|e| io(staging, e))?;
    let result = (|| {
        // 1. Descargar y verificar todo antes de tocar el servidor.
        let mut ready: Vec<(PathBuf, String)> = vec![];
        for it in &plan.items {
            let name = safe_file_name(&it.file_name, ext)?;
            if it.sha1.len() != 40 && it.sha256.as_ref().is_none_or(|h| h.len() != 64) {
                return Err(ContentError::Integrity(format!(
                    "«{}» no tiene huella para verificarlo",
                    it.title
                )));
            }
            let dest = staging.join(&name);
            let before = progress.done.load(Ordering::Relaxed);
            let mut cb = |done: u64, _total: Option<u64>| {
                progress.done.store(before + done, Ordering::Relaxed);
            };
            http.download(
                &it.url,
                &dest,
                &match &it.sha256 {
                    Some(h) if it.sha1.is_empty() => Checksum::Sha256(h.clone()),
                    _ => Checksum::Sha1(it.sha1.clone()),
                },
                Some(it.size).filter(|s| *s > 0),
                &mut cb,
                &progress.cancel,
            )?;
            ready.push((dest, name));
        }
        if progress.cancel.load(Ordering::Relaxed) {
            return Err(ContentError::Cancelled);
        }
        // 2. Lo que se reemplaza (o lo que tenga el mismo nombre) va a la papelera.
        let dir = root.join(folder);
        std::fs::create_dir_all(&dir).map_err(|e| io(&dir, e))?;
        for it in &plan.items {
            for old in it.replaces.iter().chain(std::iter::once(&it.file_name)) {
                for candidate in [old.clone(), format!("{old}{DISABLED_SUFFIX}")] {
                    if dir.join(&candidate).is_file() {
                        to_trash(&format!("{folder}/{candidate}")).map_err(ContentError::Io)?;
                    }
                }
            }
        }
        // 3. Mover lo verificado a su sitio.
        let mut placed = vec![];
        for (src, name) in ready {
            let dst = dir.join(&name);
            if std::fs::rename(&src, &dst).is_err() {
                std::fs::copy(&src, &dst).map_err(|e| io(&dst, e))?;
            }
            placed.push(name);
        }
        Ok(placed)
    })();
    let _ = std::fs::remove_dir_all(staging);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_and_toggles_without_deleting() {
        let d = tempfile::tempdir().unwrap();
        let mods = d.path().join("mods");
        std::fs::create_dir_all(mods.join("sub")).unwrap();
        std::fs::write(mods.join("b.jar"), "hola").unwrap();
        std::fs::write(mods.join("a.jar.disabled"), "x").unwrap();
        std::fs::write(mods.join("notas.txt"), "x").unwrap();
        let l = list_local(d.path(), "mods", ".jar").unwrap();
        assert_eq!(
            l.iter().map(|i| i.file_name.as_str()).collect::<Vec<_>>(),
            ["a.jar.disabled", "b.jar"]
        );
        assert!(!l[0].enabled && l[1].enabled);
        // SHA-1 de "hola".
        assert_eq!(l[1].sha1, "99800b85d3383e3a2fb45eb7d0066a4879a9dad0");

        assert_eq!(
            set_enabled(d.path(), "mods", ".jar", "b.jar", false).unwrap(),
            "b.jar.disabled"
        );
        assert!(mods.join("b.jar.disabled").is_file() && !mods.join("b.jar").exists());
        assert_eq!(
            set_enabled(d.path(), "mods", ".jar", "b.jar.disabled", true).unwrap(),
            "b.jar"
        );
        std::fs::write(mods.join("b.jar.disabled"), "otra").unwrap();
        assert!(matches!(
            set_enabled(d.path(), "mods", ".jar", "b.jar", false),
            Err(ContentError::Conflict(_))
        ));
        assert!(set_enabled(d.path(), "mods", ".jar", "../b.jar", false).is_err());
        assert!(list_local(d.path(), "plugins", ".jar").unwrap().is_empty());
    }
}
