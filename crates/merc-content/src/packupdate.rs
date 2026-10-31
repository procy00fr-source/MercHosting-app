//! Actualizar el modpack de un servidor sin pisar lo que cambió la persona.
//!
//! Al crear (o vincular) un servidor desde un modpack se guarda, fuera de la carpeta del
//! servidor, qué archivos puso el modpack y su huella SHA-1 ([`PackManifest`]). Al actualizar,
//! cada archivo se compara en tres puntos: lo que puso la versión instalada, lo que hay ahora
//! en el disco y lo que trae la versión nueva ([`plan`]):
//!
//! - sin tocar por la persona → se reemplaza o se quita según la versión nueva;
//! - cambiado por la persona (un config editado) → se conserva el suyo;
//! - quitado o desactivado por la persona → no se vuelve a poner (tampoco la versión nueva
//!   del mismo proyecto, si se sabe cuál es);
//! - un archivo propio en una ruta que ahora usa el modpack → se conserva el suyo.

use crate::local::sha1_file;
use crate::mrpack::{safe_rel, PackIndex};
use crate::{ContentError, ContentSource};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

/// Nombre del registro (en la carpeta de datos de MercHosting de la instancia).
pub const MANIFEST_FILE: &str = "modpack.json";
const MAX_OVERRIDES_BYTES: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedFile {
    pub sha1: String,
    /// Proyecto de Modrinth (sacado del enlace de descarga), si se sabe.
    #[serde(default)]
    pub project: Option<String>,
    /// Viene de `overrides/` (configuración, scripts…), no de una descarga.
    #[serde(default)]
    pub from_override: bool,
}

/// Qué modpack (y qué versión) hay instalado y qué archivos puso.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackManifest {
    pub source: ContentSource,
    pub project_id: String,
    pub title: String,
    pub version_id: String,
    pub version_number: String,
    pub mc_version: String,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub installed_at_unix: i64,
    pub files: BTreeMap<String, OwnedFile>,
}

/// Datos del modpack sin la lista de archivos (para la interfaz).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkedPack {
    pub source: ContentSource,
    pub project_id: String,
    pub title: String,
    pub version_id: String,
    pub version_number: String,
    pub mc_version: String,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub installed_at_unix: i64,
    pub files: u32,
}

impl PackManifest {
    pub fn summary(&self) -> LinkedPack {
        LinkedPack {
            source: self.source,
            project_id: self.project_id.clone(),
            title: self.title.clone(),
            version_id: self.version_id.clone(),
            version_number: self.version_number.clone(),
            mc_version: self.mc_version.clone(),
            loader: self.loader.clone(),
            loader_version: self.loader_version.clone(),
            installed_at_unix: self.installed_at_unix,
            files: self.files.len() as u32,
        }
    }
}

pub fn load(store_dir: &Path) -> Option<PackManifest> {
    let s = std::fs::read_to_string(store_dir.join(MANIFEST_FILE)).ok()?;
    serde_json::from_str(&s).ok()
}

pub fn save(store_dir: &Path, m: &PackManifest) -> Result<(), ContentError> {
    std::fs::create_dir_all(store_dir).map_err(|e| ContentError::Io(e.to_string()))?;
    let tmp = store_dir.join(format!("{MANIFEST_FILE}.tmp"));
    let body = serde_json::to_string_pretty(m).map_err(|e| ContentError::Io(e.to_string()))?;
    std::fs::write(&tmp, body).map_err(|e| ContentError::Io(e.to_string()))?;
    std::fs::rename(&tmp, store_dir.join(MANIFEST_FILE)).map_err(|e| ContentError::Io(e.to_string()))
}

/// `https://cdn.modrinth.com/data/<proyecto>/versions/...` → `<proyecto>`.
pub fn project_from_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://cdn.modrinth.com/data/")?;
    let id = rest.split('/').next()?;
    (!id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric())).then(|| id.to_string())
}

/// Lo mismo que protege `mrpack::apply_with`: nunca se toca.
fn protected(rel: &str) -> bool {
    let l = rel.to_lowercase();
    l == "eula.txt" || l == "server.properties"
}

/// Archivos que deja una versión del modpack: las descargas del índice y los `overrides`
/// (los de prefijos posteriores tienen prioridad, como al instalar).
pub fn desired(index: &PackIndex, pack: &Path, prefixes: &[&str]) -> Result<BTreeMap<String, OwnedFile>, ContentError> {
    use sha1::Digest;
    let mut out = BTreeMap::new();
    for f in &index.files {
        out.insert(
            f.path.clone(),
            OwnedFile {
                sha1: f.sha1.to_lowercase(),
                project: f.urls.iter().find_map(|u| project_from_url(u)),
                from_override: false,
            },
        );
    }
    let file = std::fs::File::open(pack).map_err(|e| ContentError::Io(format!("{}: {e}", pack.display())))?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|_| ContentError::Invalid("el modpack no es un ZIP válido".into()))?;
    let mut total = 0u64;
    for prefix in prefixes {
        for i in 0..zip.len() {
            let mut e = zip.by_index(i).map_err(|e| ContentError::Invalid(e.to_string()))?;
            if e.is_dir() {
                continue;
            }
            let Some(name) = e.enclosed_name() else { continue };
            let name = name.to_string_lossy().replace('\\', "/");
            let Some(rel) = name.strip_prefix(prefix).and_then(safe_rel) else {
                continue;
            };
            if protected(&rel) {
                continue;
            }
            let mut h = sha1::Sha1::new();
            let mut buf = [0u8; 64 * 1024];
            loop {
                let n = e.read(&mut buf).map_err(|e| ContentError::Io(e.to_string()))?;
                if n == 0 {
                    break;
                }
                total += n as u64;
                if total > MAX_OVERRIDES_BYTES {
                    return Err(ContentError::Invalid(
                        "los archivos extra del modpack son demasiado grandes".into(),
                    ));
                }
                h.update(&buf[..n]);
            }
            out.insert(
                rel,
                OwnedFile {
                    sha1: h.finalize().iter().map(|b| format!("{b:02x}")).collect(),
                    project: None,
                    from_override: true,
                },
            );
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// Archivo nuevo del modpack.
    Add,
    /// La versión nueva lo cambia y la persona no lo había tocado.
    Replace,
    /// La versión nueva ya no lo trae y la persona no lo había tocado (va a la papelera).
    Remove,
    /// La persona lo cambió: se conserva el suyo (la versión nueva lo traía distinto o ya no lo trae).
    KeepModified,
    /// Hay un archivo de la persona en una ruta que ahora usa el modpack: se conserva.
    KeepUserFile,
    /// La persona lo quitó o lo desactivó: no se vuelve a poner.
    SkipRemoved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedChange {
    pub path: String,
    pub kind: ChangeKind,
    pub from_override: bool,
}

/// ¿Está el archivo desactivado (`.disabled`) o apartado por el separador de mods?
fn set_aside(root: &Path, rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    root.join(format!("{rel}.disabled")).exists() || root.join("mods-cuarentena").join(name).exists()
}

/// Compara la versión instalada, el disco y la versión nueva. No toca nada.
pub fn plan(root: &Path, old: &BTreeMap<String, OwnedFile>, new: &BTreeMap<String, OwnedFile>) -> Vec<PlannedChange> {
    let disk = |rel: &str| -> Option<String> {
        let p = root.join(rel);
        p.is_file().then(|| sha1_file(&p).ok()).flatten()
    };
    let mut out = vec![];
    // Proyectos que la persona quitó o desactivó: tampoco se pone su versión nueva.
    let mut removed_projects = BTreeSet::new();
    let mut disk_cache = BTreeMap::new();
    for (path, f) in old {
        let d = disk(path);
        if d.is_none() {
            if let Some(p) = &f.project {
                removed_projects.insert(p.clone());
            }
        }
        disk_cache.insert(path.clone(), d);
    }
    let paths: BTreeSet<&String> = old.keys().chain(new.keys()).collect();
    for path in paths {
        let o = old.get(path);
        let n = new.get(path);
        let d = match disk_cache.get(path) {
            Some(d) => d.clone(),
            None => disk(path),
        };
        let from_override = n.or(o).is_some_and(|f| f.from_override);
        let kind = match (o, n, d.as_deref()) {
            (_, Some(n), Some(d)) if d == n.sha1 => None,
            (Some(o), Some(_), Some(d)) if d == o.sha1 => Some(ChangeKind::Replace),
            (Some(_), Some(_), Some(_)) => Some(ChangeKind::KeepModified),
            (None, Some(_), Some(_)) => Some(ChangeKind::KeepUserFile),
            (Some(_), Some(_), None) => Some(ChangeKind::SkipRemoved),
            (None, Some(n), None) => {
                if n.project.as_ref().is_some_and(|p| removed_projects.contains(p)) || set_aside(root, path) {
                    Some(ChangeKind::SkipRemoved)
                } else {
                    Some(ChangeKind::Add)
                }
            }
            (Some(o), None, Some(d)) if d == o.sha1 => Some(ChangeKind::Remove),
            (Some(_), None, Some(_)) => Some(ChangeKind::KeepModified),
            (Some(_), None, None) | (None, None, _) => None,
        };
        if let Some(kind) = kind {
            out.push(PlannedChange {
                path: path.clone(),
                kind,
                from_override,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(sha: &str, project: Option<&str>) -> OwnedFile {
        OwnedFile {
            sha1: sha.into(),
            project: project.map(str::to_string),
            from_override: false,
        }
    }
    fn sha(s: &str) -> String {
        use sha1::Digest;
        let mut h = sha1::Sha1::new();
        h.update(s.as_bytes());
        h.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn three_way_plan_respects_user_changes() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        std::fs::create_dir_all(r.join("mods")).unwrap();
        std::fs::create_dir_all(r.join("config")).unwrap();
        std::fs::create_dir_all(r.join("mods-cuarentena")).unwrap();
        // En disco
        std::fs::write(r.join("mods/igual.jar"), "igual").unwrap();
        std::fs::write(r.join("mods/a-1.jar"), "a1").unwrap();
        std::fs::write(r.join("mods/viejo.jar"), "viejo").unwrap();
        std::fs::write(r.join("config/a.toml"), "mio").unwrap(); // editado
        std::fs::write(r.join("config/b.toml"), "b1").unwrap();
        std::fs::write(r.join("mods/propio.jar"), "propio").unwrap(); // de la persona
        std::fs::write(r.join("mods-cuarentena/minimapa.jar"), "mm").unwrap();
        // mods/zoom-1.jar fue borrado por la persona (proyecto ZOOM)
        let old: BTreeMap<_, _> = [
            ("mods/igual.jar", f(&sha("igual"), None)),
            ("mods/a-1.jar", f(&sha("a1"), Some("AAA"))),
            ("mods/viejo.jar", f(&sha("viejo"), None)),
            ("mods/zoom-1.jar", f(&sha("z1"), Some("ZOOM"))),
            ("config/a.toml", f(&sha("a-orig"), None)),
            ("config/b.toml", f(&sha("b1"), None)),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        let new: BTreeMap<_, _> = [
            ("mods/igual.jar", f(&sha("igual"), None)),
            ("mods/a-2.jar", f(&sha("a2"), Some("AAA"))),
            ("mods/zoom-2.jar", f(&sha("z2"), Some("ZOOM"))),
            ("mods/nuevo.jar", f(&sha("n"), Some("NEW"))),
            ("mods/propio.jar", f(&sha("otro"), None)),
            ("mods/minimapa.jar", f(&sha("mm2"), None)),
            ("config/a.toml", f(&sha("a-nuevo"), None)),
            ("config/b.toml", f(&sha("b2"), None)),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        let p = plan(r, &old, &new);
        let kind = |path: &str| p.iter().find(|c| c.path == path).map(|c| c.kind);
        assert_eq!(kind("mods/igual.jar"), None);
        assert_eq!(kind("mods/a-1.jar"), Some(ChangeKind::Remove));
        assert_eq!(kind("mods/a-2.jar"), Some(ChangeKind::Add));
        assert_eq!(kind("mods/viejo.jar"), Some(ChangeKind::Remove));
        assert_eq!(kind("mods/zoom-1.jar"), None);
        assert_eq!(kind("mods/zoom-2.jar"), Some(ChangeKind::SkipRemoved));
        assert_eq!(kind("mods/nuevo.jar"), Some(ChangeKind::Add));
        assert_eq!(kind("mods/propio.jar"), Some(ChangeKind::KeepUserFile));
        assert_eq!(kind("mods/minimapa.jar"), Some(ChangeKind::SkipRemoved));
        assert_eq!(kind("config/a.toml"), Some(ChangeKind::KeepModified));
        assert_eq!(kind("config/b.toml"), Some(ChangeKind::Replace));
    }

    #[test]
    fn project_ids_come_from_modrinth_cdn() {
        assert_eq!(
            project_from_url("https://cdn.modrinth.com/data/AANobbMI/versions/x/sodium.jar").as_deref(),
            Some("AANobbMI")
        );
        assert_eq!(project_from_url("https://edge.forgecdn.net/files/1/2/a.jar"), None);
    }
}
