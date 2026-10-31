//! Hangar, el catálogo oficial de plugins de PaperMC (API pública v1:
//! <https://hangar.papermc.io/api-docs>).
//!
//! - Leer no necesita cuenta ni clave. Hangar pide un `User-Agent` que identifique a la
//!   aplicación (lo pone [`merc_install::Http`]) y limita a unas 20 peticiones cada 5 segundos,
//!   así que lo instalado se consulta con un tope por vez y se guarda en caché unos minutos.
//! - Cada descarga se verifica con el SHA-256 que publica Hangar.
//! - Algunos autores publican el archivo en otra web (`externalUrl`): eso no se descarga desde
//!   aquí; se indica que lo baje la persona desde la página del autor.
//! - Solo se usa la plataforma `PAPER` (Purpur carga los plugins de Paper).

use crate::local::{InstalledItem, UpdateInfo};
use crate::modrinth::{
    encode, ContentHit, ContentProject, ContentVersion, Dependency, DependencyType, SearchPage, SearchSort, Side,
    Source, VersionFile,
};
use crate::{ContentError, ContentKind, ContentSource, Platform};
use merc_install::Http;
use serde_json::Value;
use sha2::Digest as _;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant};

pub const HANGAR_API: &str = "https://hangar.papermc.io/api/v1";
pub const HANGAR_SITE: &str = "https://hangar.papermc.io";
/// Plataforma de Hangar que sirve para Paper y Purpur.
pub const PLATFORM: &str = "PAPER";
/// Archivos sin reconocer que se consultan a Hangar en cada listado (el resto, la próxima vez).
pub const MAX_LOOKUPS: usize = 8;
const CACHE_TTL: Duration = Duration::from_secs(10 * 60);

pub struct Hangar<'a> {
    http: &'a Http,
    base: String,
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn opt(v: &Value, k: &str) -> Option<String> {
    v.get(k)
        .and_then(Value::as_str)
        .filter(|x| !x.is_empty())
        .map(str::to_string)
}

/// Número o texto como texto (los identificadores de Hangar son números).
fn id_of(v: &Value, k: &str) -> String {
    match v.get(k) {
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::String(t)) => t.clone(),
        _ => String::new(),
    }
}

fn bad(what: &str) -> ContentError {
    ContentError::BadResponse(format!("Hangar respondió algo inesperado ({what})"))
}

/// Identificador numérico o nombre de proyecto de Hangar.
fn check_id(id: &str) -> Result<&str, ContentError> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        || id.starts_with('.')
    {
        return Err(ContentError::Invalid(format!("identificador no válido: «{id}»")));
    }
    Ok(id)
}

fn version_type(channel: &str) -> String {
    let c = channel.to_lowercase();
    if c == "release" || c == "stable" {
        "release".into()
    } else if c.contains("alpha") || c.contains("snapshot") || c.contains("dev") {
        "alpha".into()
    } else {
        "beta".into()
    }
}

/// Todos los enlaces del proyecto (Hangar los agrupa en secciones).
fn links(v: &Value) -> Vec<(String, String)> {
    let mut out = vec![];
    let sections = v
        .get("settings")
        .and_then(|s| s.get("links"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for sec in sections {
        for l in sec.get("links").and_then(Value::as_array).cloned().unwrap_or_default() {
            let url = s(&l, "url");
            if url.starts_with("https://") {
                out.push((s(&l, "name").to_lowercase(), url));
            }
        }
    }
    out
}

pub fn parse_project(v: &Value) -> Result<ContentProject, ContentError> {
    let id = id_of(v, "id");
    let name = s(v, "name");
    if id.is_empty() || name.is_empty() {
        return Err(bad("proyecto"));
    }
    let ns = v.get("namespace").cloned().unwrap_or(Value::Null);
    let owner = s(&ns, "owner");
    let slug = opt(&ns, "slug").unwrap_or_else(|| name.clone());
    let l = links(v);
    let find = |words: &[&str]| {
        l.iter()
            .find(|(n, _)| words.iter().any(|w| n.contains(w)))
            .map(|(_, u)| u.clone())
    };
    Ok(ContentProject {
        page_url: if owner.is_empty() {
            format!("{HANGAR_SITE}/{slug}")
        } else {
            format!("{HANGAR_SITE}/{owner}/{slug}")
        },
        id,
        slug,
        title: name,
        description: s(v, "description"),
        body: s(v, "mainPageContent"),
        icon_url: opt(v, "avatarUrl").filter(|u| u.starts_with("https://")),
        client_side: Side::Unsupported,
        server_side: Side::Required,
        license: v
            .get("settings")
            .and_then(|s| s.get("license"))
            .map(|l| s(l, "name"))
            .unwrap_or_default(),
        downloads: v
            .get("stats")
            .and_then(|s| s.get("downloads"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        categories: opt(v, "category").into_iter().collect(),
        source_url: find(&["source", "código", "github"]),
        issues_url: find(&["issue", "bug"]),
        wiki_url: find(&["wiki", "doc"]),
        distribution_allowed: true,
    })
}

fn parse_hit(v: &Value) -> Result<ContentHit, ContentError> {
    let p = parse_project(v)?;
    Ok(ContentHit {
        project_id: p.id,
        slug: p.slug,
        title: p.title,
        author: v.get("namespace").map(|n| s(n, "owner")).unwrap_or_default(),
        description: p.description,
        icon_url: p.icon_url,
        downloads: p.downloads,
        categories: p.categories,
        client_side: Side::Unsupported,
        server_side: Side::Required,
        date_modified: s(v, "lastUpdated"),
        license: p.license,
    })
}

pub fn parse_version(v: &Value) -> Result<ContentVersion, ContentError> {
    let id = id_of(v, "id");
    let name = s(v, "name");
    if id.is_empty() || name.is_empty() {
        return Err(bad("versión"));
    }
    let download = v.get("downloads").and_then(|d| d.get(PLATFORM));
    let mut files = vec![];
    let mut loaders = vec![];
    if let Some(d) = download {
        loaders.push("paper".to_string());
        let info = d.get("fileInfo").cloned().unwrap_or(Value::Null);
        let url = opt(d, "downloadUrl")
            .filter(|u| merc_install::http::url_allowed(u))
            .unwrap_or_default();
        let filename = opt(&info, "name").unwrap_or_else(|| format!("{}-{name}.jar", s(v, "projectId")));
        files.push(VersionFile {
            // Vacía si el archivo está en otra web: no se descarga desde aquí.
            url,
            filename,
            primary: true,
            size: info.get("sizeBytes").and_then(Value::as_u64).unwrap_or(0),
            sha1: None,
            sha512: None,
            sha256: opt(&info, "sha256Hash").map(|h| h.to_lowercase()),
        });
    }
    let dependencies = v
        .get("pluginDependencies")
        .and_then(|d| d.get(PLATFORM))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|d| {
                    let dep_name = opt(d, "name")?;
                    let project = id_of(d, "projectId");
                    let external = opt(d, "externalUrl").is_some();
                    let required = d.get("required").and_then(Value::as_bool).unwrap_or(false);
                    let (project_id, file_name) = if !project.is_empty() {
                        (Some(project), None)
                    } else if external {
                        (None, Some(dep_name))
                    } else {
                        (Some(dep_name), None)
                    };
                    Some(Dependency {
                        version_id: None,
                        project_id,
                        file_name,
                        dependency_type: if required {
                            DependencyType::Required
                        } else {
                            DependencyType::Optional
                        },
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let game_versions = v
        .get("platformDependencies")
        .and_then(|d| d.get(PLATFORM))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default();
    Ok(ContentVersion {
        project_id: id_of(v, "projectId"),
        id,
        version_number: name.clone(),
        name,
        version_type: version_type(&v.get("channel").map(|c| s(c, "name")).unwrap_or_default()),
        game_versions,
        loaders,
        date_published: s(v, "createdAt"),
        downloads: v
            .get("stats")
            .and_then(|s| s.get("totalDownloads"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        changelog: opt(v, "description"),
        dependencies,
        files,
        server_side: Side::Required,
    })
}

impl<'a> Hangar<'a> {
    pub fn new(http: &'a Http, base: impl Into<String>) -> Self {
        Self {
            http,
            base: base.into(),
        }
    }

    fn get(&self, path: &str) -> Result<Value, ContentError> {
        Ok(self.http.get_json(&format!("{}{path}", self.base))?)
    }

    fn result(v: &Value) -> Result<&Vec<Value>, ContentError> {
        v.get("result").and_then(Value::as_array).ok_or_else(|| bad("lista"))
    }

    /// Proyecto al que pertenece un archivo, por su SHA-256 (`None` si Hangar no lo conoce).
    pub fn project_by_sha256(&self, hash: &str) -> Result<Option<ContentProject>, ContentError> {
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ContentError::Invalid("huella SHA-256 no válida".into()));
        }
        match self.get(&format!("/versions/hash/{hash}")) {
            Ok(v) => parse_project(&v).map(Some),
            Err(ContentError::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Las últimas versiones de un proyecto para Paper (sin filtrar por Minecraft).
    pub fn recent_versions(&self, project: &str) -> Result<Vec<ContentVersion>, ContentError> {
        self.versions_query(project, None)
    }

    fn versions_query(&self, project: &str, mc: Option<&str>) -> Result<Vec<ContentVersion>, ContentError> {
        let mut path = format!(
            "/projects/{}/versions?limit=25&offset=0&platform={PLATFORM}",
            check_id(project)?
        );
        if let Some(m) = mc {
            path.push_str(&format!("&platformVersion={}", encode(m)));
        }
        let v = self.get(&path)?;
        let mut list: Vec<ContentVersion> = Self::result(&v)?
            .iter()
            .filter_map(|x| parse_version(x).ok())
            .filter(|x| !x.files.is_empty())
            .collect();
        list.sort_by(|a, b| b.date_published.cmp(&a.date_published));
        Ok(list)
    }
}

impl Source for Hangar<'_> {
    fn search(
        &self,
        platform: &Platform,
        mc: Option<&str>,
        query: &str,
        sort: SearchSort,
        offset: u64,
    ) -> Result<SearchPage, ContentError> {
        if platform.kind != ContentKind::Plugin {
            return Ok(SearchPage {
                hits: vec![],
                total: 0,
                offset,
            });
        }
        let q: String = query.trim().chars().take(100).collect();
        let mut path = format!("/projects?limit=20&offset={offset}&platform={PLATFORM}");
        if !q.is_empty() {
            path.push_str(&format!("&q={}", encode(&q)));
        }
        if let Some(m) = mc {
            path.push_str(&format!("&version={}", encode(m)));
        }
        let sort = match sort {
            SearchSort::Relevance if !q.is_empty() => None,
            SearchSort::Relevance | SearchSort::Downloads => Some("-downloads"),
            SearchSort::Updated => Some("-updated"),
            SearchSort::Newest => Some("-newest"),
        };
        if let Some(s) = sort {
            path.push_str(&format!("&sort={s}"));
        }
        let v = self.get(&path)?;
        let hits = Self::result(&v)?.iter().filter_map(|h| parse_hit(h).ok()).collect();
        let pag = v.get("pagination").cloned().unwrap_or(Value::Null);
        Ok(SearchPage {
            hits,
            total: pag.get("count").and_then(Value::as_u64).unwrap_or(0),
            offset: pag.get("offset").and_then(Value::as_u64).unwrap_or(offset),
        })
    }

    fn project(&self, id: &str) -> Result<ContentProject, ContentError> {
        parse_project(&self.get(&format!("/projects/{}", check_id(id)?))?)
    }

    fn projects(&self, ids: &[String]) -> Result<Vec<ContentProject>, ContentError> {
        // Hangar no tiene consulta por lotes; se limita para no pasar el límite de peticiones.
        ids.iter().take(MAX_LOOKUPS).map(|id| self.project(id)).collect()
    }

    fn versions(
        &self,
        project: &str,
        loaders: &[String],
        mc: Option<&str>,
    ) -> Result<Vec<ContentVersion>, ContentError> {
        Ok(self
            .versions_query(project, mc)?
            .into_iter()
            .filter(|v| v.fits(loaders, mc))
            .collect())
    }

    fn version(&self, id: &str) -> Result<ContentVersion, ContentError> {
        if id.is_empty() || id.len() > 20 || !id.bytes().all(|b| b.is_ascii_digit()) {
            return Err(ContentError::Invalid(format!("versión no válida: «{id}»")));
        }
        parse_version(&self.get(&format!("/versions/{id}"))?)
    }

    fn versions_by_sha1(&self, _: &[String]) -> Result<HashMap<String, ContentVersion>, ContentError> {
        // Hangar reconoce por SHA-256: ver [`enrich_hangar`].
        Ok(HashMap::new())
    }

    fn latest_by_sha1(
        &self,
        _: &[String],
        _: &[String],
        _: Option<&str>,
    ) -> Result<HashMap<String, ContentVersion>, ContentError> {
        Ok(HashMap::new())
    }
}

pub fn sha256_file(p: &Path) -> Result<String, ContentError> {
    let mut f = std::fs::File::open(p).map_err(|e| ContentError::Io(format!("{}: {e}", p.display())))?;
    let mut h = sha2::Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| ContentError::Io(format!("{}: {e}", p.display())))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Respuestas de Hangar recientes, para no repetir consultas al listar varias veces seguidas.
#[derive(Default)]
pub struct HangarCache {
    by_hash: HashMap<String, (Instant, Option<ContentProject>)>,
    versions: HashMap<String, (Instant, Vec<ContentVersion>)>,
}

impl HangarCache {
    fn fresh<T: Clone>(map: &HashMap<String, (Instant, T)>, k: &str) -> Option<T> {
        map.get(k)
            .filter(|(at, _)| at.elapsed() < CACHE_TTL)
            .map(|(_, v)| v.clone())
    }

    /// Olvida lo guardado (por ejemplo, después de instalar).
    pub fn clear(&mut self) {
        self.by_hash.clear();
        self.versions.clear();
    }
}

/// Completa con Hangar lo que Modrinth (y CurseForge) no reconocieron: nombre, icono, versión y
/// actualización. Consulta como mucho [`MAX_LOOKUPS`] archivos nuevos por vez.
pub fn enrich_hangar(
    h: &Hangar,
    cache: &mut HangarCache,
    root: &Path,
    platform: &Platform,
    mc: Option<&str>,
    items: &mut [InstalledItem],
) -> Result<(), ContentError> {
    let mut lookups = 0;
    for it in items.iter_mut().filter(|i| i.source.is_none()) {
        let hash = sha256_file(&root.join(&platform.folder).join(&it.file_name))?;
        let project = match HangarCache::fresh(&cache.by_hash, &hash) {
            Some(p) => p,
            None => {
                if lookups >= MAX_LOOKUPS {
                    continue;
                }
                lookups += 1;
                let p = h.project_by_sha256(&hash)?;
                cache.by_hash.insert(hash.clone(), (Instant::now(), p.clone()));
                p
            }
        };
        let Some(project) = project else { continue };
        let versions = match HangarCache::fresh(&cache.versions, &project.id) {
            Some(v) => v,
            None => {
                if lookups >= MAX_LOOKUPS {
                    continue;
                }
                lookups += 1;
                let v = h.recent_versions(&project.id)?;
                cache.versions.insert(project.id.clone(), (Instant::now(), v.clone()));
                v
            }
        };
        it.source = Some(ContentSource::Hangar);
        it.project_id = Some(project.id.clone());
        it.title = Some(project.title.clone());
        it.icon_url = project.icon_url.clone();
        it.server_side = Some(Side::Required);
        let current = versions
            .iter()
            .find(|v| v.files.iter().any(|f| f.sha256.as_deref() == Some(hash.as_str())));
        if let Some(cur) = current {
            it.version_id = Some(cur.id.clone());
            it.version_number = Some(cur.version_number.clone());
            if it.enabled {
                let newest = versions
                    .iter()
                    .filter(|v| v.version_type == "release" && v.fits(&platform.loaders, mc))
                    .max_by(|a, b| a.date_published.cmp(&b.date_published));
                if let Some(n) = newest.filter(|n| n.id != cur.id && n.date_published > cur.date_published) {
                    it.update = Some(UpdateInfo {
                        version_id: n.id.clone(),
                        version_number: n.version_number.clone(),
                    });
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    pub fn version_json(id: u64, name: &str, mc: &[&str], sha: &str, external: bool) -> Value {
        json!({
            "id": id,
            "projectId": 7,
            "name": name,
            "createdAt": format!("2026-0{}-01T00:00:00Z", id % 9 + 1),
            "description": "Cambios",
            "stats": {"totalDownloads": 10, "platformDownloads": {"PAPER": 10}},
            "channel": {"name": "Release", "flags": []},
            "downloads": {"PAPER": {
                "fileInfo": {"name": format!("Ejemplo-{name}.jar"), "sizeBytes": 4, "sha256Hash": sha},
                "externalUrl": if external { json!("https://ejemplo.dev/descarga") } else { Value::Null },
                "downloadUrl": if external { Value::Null } else { json!(format!("https://hangarcdn.papermc.io/plugins/Autor/Ejemplo/versions/{name}/PAPER/Ejemplo-{name}.jar")) }
            }},
            "pluginDependencies": {"PAPER": [
                {"name": "Vault", "projectId": null, "required": true, "externalUrl": "https://github.com/MilkBowl/Vault", "platform": "PAPER"},
                {"name": "LibreriaX", "projectId": 9, "required": true, "externalUrl": null, "platform": "PAPER"},
                {"name": "Extra", "projectId": 10, "required": false, "externalUrl": null, "platform": "PAPER"}
            ]},
            "platformDependencies": {"PAPER": mc}
        })
    }

    #[test]
    fn parses_documented_project_and_version() {
        let p = parse_project(&json!({
            "id": 7, "name": "Ejemplo", "namespace": {"owner": "Autor", "slug": "Ejemplo"},
            "stats": {"downloads": 1234, "stars": 3}, "category": "admin_tools",
            "lastUpdated": "2026-10-01T00:00:00Z", "avatarUrl": "https://hangarcdn.papermc.io/avatars/project/7.webp",
            "description": "Un plugin", "mainPageContent": "# Hola",
            "settings": {"license": {"name": "MIT"}, "links": [{"id": 0, "type": "top", "links": [
                {"id": 0, "name": "Issues", "url": "https://github.com/a/b/issues"},
                {"id": 1, "name": "Source", "url": "https://github.com/a/b"}
            ]}]}
        }))
        .unwrap();
        assert_eq!((p.id.as_str(), p.title.as_str()), ("7", "Ejemplo"));
        assert_eq!(p.page_url, "https://hangar.papermc.io/Autor/Ejemplo");
        assert_eq!(p.source_url.as_deref(), Some("https://github.com/a/b"));
        assert_eq!(p.issues_url.as_deref(), Some("https://github.com/a/b/issues"));
        assert_eq!(
            (p.license.as_str(), p.downloads, p.body.as_str()),
            ("MIT", 1234, "# Hola")
        );

        let sha = "ab".repeat(32);
        let v = parse_version(&version_json(3, "1.2.0", &["1.21.1", "1.21.3"], &sha, false)).unwrap();
        assert_eq!((v.id.as_str(), v.project_id.as_str()), ("3", "7"));
        assert_eq!(v.version_type, "release");
        assert!(v.fits(&["purpur".into(), "paper".into()], Some("1.21.1")));
        assert!(!v.fits(&["paper".into()], Some("1.20.4")));
        let f = v.primary_file().unwrap();
        assert_eq!(f.sha256.as_deref(), Some(sha.as_str()));
        assert!(f.url.starts_with("https://hangarcdn.papermc.io/"));
        assert_eq!(f.filename, "Ejemplo-1.2.0.jar");
        // Vault está fuera de Hangar: solo se avisa. LibreriaX es obligatoria; Extra, opcional.
        assert_eq!(v.dependencies[0].file_name.as_deref(), Some("Vault"));
        assert_eq!(v.dependencies[1].project_id.as_deref(), Some("9"));
        assert_eq!(v.dependencies[1].dependency_type, DependencyType::Required);
        assert_eq!(v.dependencies[2].dependency_type, DependencyType::Optional);

        let ext = parse_version(&version_json(4, "1.3.0", &["1.21.1"], &sha, true)).unwrap();
        assert!(ext.primary_file().unwrap().url.is_empty());
    }

    #[test]
    fn ids_and_channels() {
        assert!(check_id("ViaVersion").is_ok() && check_id("123").is_ok());
        assert!(check_id("../x").is_err() && check_id("a/b").is_err() && check_id("").is_err());
        assert_eq!(version_type("Release"), "release");
        assert_eq!(version_type("Snapshot"), "alpha");
        assert_eq!(version_type("Beta"), "beta");
    }
}
