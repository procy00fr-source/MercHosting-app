//! Catálogo de Modrinth (API pública v2: <https://docs.modrinth.com/api/>).
//!
//! No hace falta cuenta ni clave para leer. Modrinth pide un `User-Agent` que identifique a la
//! aplicación (lo pone [`merc_install::Http`]) y limita a 300 peticiones por minuto por IP, así
//! que las consultas de lo instalado se hacen por lotes (huellas SHA-1 en una sola petición).

use crate::{ContentError, ContentKind, Platform};
use merc_install::Http;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

pub const MODRINTH_API: &str = "https://api.modrinth.com/v2";

/// Lado en el que hace falta un proyecto según Modrinth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Required,
    Optional,
    Unsupported,
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub author: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub categories: Vec<String>,
    pub client_side: Side,
    pub server_side: Side,
    pub date_modified: String,
    pub license: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchPage {
    pub hits: Vec<ContentHit>,
    pub total: u64,
    pub offset: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SearchSort {
    #[default]
    Relevance,
    Downloads,
    Updated,
    Newest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentProject {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    /// Descripción larga en Markdown (se muestra como texto, nunca como HTML).
    pub body: String,
    pub icon_url: Option<String>,
    pub client_side: Side,
    pub server_side: Side,
    pub license: String,
    pub downloads: u64,
    pub categories: Vec<String>,
    pub source_url: Option<String>,
    pub issues_url: Option<String>,
    pub wiki_url: Option<String>,
    /// Página oficial del proyecto (Modrinth o CurseForge).
    pub page_url: String,
    /// El autor permite descargarlo desde otras aplicaciones (en CurseForge puede no permitirlo).
    #[serde(default = "yes")]
    pub distribution_allowed: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyType {
    Required,
    Optional,
    Incompatible,
    Embedded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    pub version_id: Option<String>,
    pub project_id: Option<String>,
    /// Dependencia fuera de Modrinth (solo se conoce el nombre del archivo).
    pub file_name: Option<String>,
    pub dependency_type: DependencyType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u64,
    pub sha1: Option<String>,
    pub sha512: Option<String>,
    /// Hangar publica SHA-256 en lugar de SHA-1.
    #[serde(default)]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentVersion {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    /// `release`, `beta` o `alpha`.
    pub version_type: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub date_published: String,
    pub downloads: u64,
    pub changelog: Option<String>,
    pub dependencies: Vec<Dependency>,
    pub files: Vec<VersionFile>,
    /// Lado indicado en el propio archivo (CurseForge marca «Server»/«Client»); si no, `unknown`.
    #[serde(default)]
    pub server_side: Side,
}

impl ContentVersion {
    /// El archivo principal (el que se instala).
    pub fn primary_file(&self) -> Option<&VersionFile> {
        self.files.iter().find(|f| f.primary).or_else(|| self.files.first())
    }

    pub fn fits(&self, loaders: &[String], mc: Option<&str>) -> bool {
        self.loaders.iter().any(|l| loaders.contains(l)) && mc.is_none_or(|v| self.game_versions.iter().any(|g| g == v))
    }
}

/// Fuente del catálogo. La real es [`Modrinth`]; las pruebas usan una en memoria.
pub trait Source {
    fn search(
        &self,
        platform: &Platform,
        mc: Option<&str>,
        query: &str,
        sort: SearchSort,
        offset: u64,
    ) -> Result<SearchPage, ContentError>;
    fn project(&self, id: &str) -> Result<ContentProject, ContentError>;
    fn projects(&self, ids: &[String]) -> Result<Vec<ContentProject>, ContentError>;
    /// Versiones de un proyecto para estos loaders y esta versión de Minecraft (más nuevas primero).
    fn versions(
        &self,
        project: &str,
        loaders: &[String],
        mc: Option<&str>,
    ) -> Result<Vec<ContentVersion>, ContentError>;
    fn version(&self, id: &str) -> Result<ContentVersion, ContentError>;
    /// Versión a la que pertenece cada archivo, por su SHA-1.
    fn versions_by_sha1(&self, hashes: &[String]) -> Result<HashMap<String, ContentVersion>, ContentError>;
    /// Versión más nueva compatible del mismo proyecto que cada archivo, por su SHA-1.
    fn latest_by_sha1(
        &self,
        hashes: &[String],
        loaders: &[String],
        mc: Option<&str>,
    ) -> Result<HashMap<String, ContentVersion>, ContentError>;
}

pub struct Modrinth<'a> {
    http: &'a Http,
    base: String,
}

/// Codifica un valor para la parte de consulta de una URL.
pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Filtros de búsqueda de Modrinth. Las condiciones de un mismo grupo se unen con «o».
pub fn search_facets(platform: &Platform, mc: Option<&str>) -> Value {
    let mut facets = vec![];
    if platform.kind == ContentKind::Mod {
        facets.push(json!(["project_type:mod"]));
        // Para un servidor solo sirven los mods que funcionan en el servidor.
        facets.push(json!(["server_side:required", "server_side:optional"]));
    }
    facets.push(Value::Array(
        platform
            .loaders
            .iter()
            .map(|l| Value::String(format!("categories:{l}")))
            .collect(),
    ));
    if let Some(v) = mc {
        facets.push(json!([format!("versions:{v}")]));
    }
    Value::Array(facets)
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

fn strings(v: &Value, k: &str) -> Vec<String> {
    v.get(k)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default()
}

fn side(v: &Value, k: &str) -> Side {
    v.get(k)
        .cloned()
        .and_then(|x| serde_json::from_value(x).ok())
        .unwrap_or_default()
}

fn bad(what: &str) -> ContentError {
    ContentError::BadResponse(format!("Modrinth respondió algo inesperado ({what})"))
}

/// Filtros de búsqueda de paquetes de recursos.
pub fn resourcepack_facets(mc: Option<&str>) -> Value {
    let mut f = vec![json!(["project_type:resourcepack"])];
    if let Some(v) = mc.filter(|v| !v.is_empty()) {
        f.push(json!([format!("versions:{v}")]));
    }
    Value::Array(f)
}

pub fn parse_search(v: &Value) -> Result<SearchPage, ContentError> {
    let hits = v.get("hits").and_then(Value::as_array).ok_or_else(|| bad("búsqueda"))?;
    Ok(SearchPage {
        hits: hits
            .iter()
            .map(|h| ContentHit {
                project_id: s(h, "project_id"),
                slug: s(h, "slug"),
                title: s(h, "title"),
                author: s(h, "author"),
                description: s(h, "description"),
                icon_url: opt(h, "icon_url"),
                downloads: h.get("downloads").and_then(Value::as_u64).unwrap_or(0),
                categories: strings(h, "display_categories"),
                client_side: side(h, "client_side"),
                server_side: side(h, "server_side"),
                date_modified: s(h, "date_modified"),
                license: s(h, "license"),
            })
            .filter(|h| !h.project_id.is_empty())
            .collect(),
        total: v.get("total_hits").and_then(Value::as_u64).unwrap_or(0),
        offset: v.get("offset").and_then(Value::as_u64).unwrap_or(0),
    })
}

pub fn parse_project(v: &Value) -> Result<ContentProject, ContentError> {
    let id = s(v, "id");
    if id.is_empty() {
        return Err(bad("proyecto"));
    }
    let slug = s(v, "slug");
    let license = v
        .get("license")
        .map(|l| {
            let name = s(l, "name");
            if name.is_empty() {
                s(l, "id")
            } else {
                name
            }
        })
        .unwrap_or_default();
    Ok(ContentProject {
        page_url: format!(
            "https://modrinth.com/project/{}",
            if slug.is_empty() { &id } else { &slug }
        ),
        id,
        slug,
        title: s(v, "title"),
        description: s(v, "description"),
        body: s(v, "body"),
        icon_url: opt(v, "icon_url"),
        client_side: side(v, "client_side"),
        server_side: side(v, "server_side"),
        license,
        downloads: v.get("downloads").and_then(Value::as_u64).unwrap_or(0),
        categories: strings(v, "categories"),
        source_url: opt(v, "source_url"),
        issues_url: opt(v, "issues_url"),
        wiki_url: opt(v, "wiki_url"),
        distribution_allowed: true,
    })
}

pub fn parse_version(v: &Value) -> Result<ContentVersion, ContentError> {
    let id = s(v, "id");
    if id.is_empty() {
        return Err(bad("versión"));
    }
    let dependencies = v
        .get("dependencies")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|d| {
                    Some(Dependency {
                        version_id: opt(d, "version_id"),
                        project_id: opt(d, "project_id"),
                        file_name: opt(d, "file_name"),
                        dependency_type: serde_json::from_value(d.get("dependency_type")?.clone()).ok()?,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let files = v
        .get("files")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|f| VersionFile {
                    url: s(f, "url"),
                    filename: s(f, "filename"),
                    primary: f.get("primary").and_then(Value::as_bool).unwrap_or(false),
                    size: f.get("size").and_then(Value::as_u64).unwrap_or(0),
                    sha1: f.get("hashes").and_then(|h| opt(h, "sha1")),
                    sha512: f.get("hashes").and_then(|h| opt(h, "sha512")),
                    sha256: None,
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(ContentVersion {
        id,
        project_id: s(v, "project_id"),
        name: s(v, "name"),
        version_number: s(v, "version_number"),
        version_type: s(v, "version_type"),
        game_versions: strings(v, "game_versions"),
        loaders: strings(v, "loaders"),
        date_published: s(v, "date_published"),
        downloads: v.get("downloads").and_then(Value::as_u64).unwrap_or(0),
        changelog: opt(v, "changelog"),
        dependencies,
        files,
        server_side: Side::Unknown,
    })
}

fn parse_hash_map(v: &Value) -> Result<HashMap<String, ContentVersion>, ContentError> {
    let obj = v.as_object().ok_or_else(|| bad("huellas"))?;
    Ok(obj
        .iter()
        .filter_map(|(k, ver)| parse_version(ver).ok().map(|p| (k.to_lowercase(), p)))
        .collect())
}

impl<'a> Modrinth<'a> {
    pub fn new(http: &'a Http, base: impl Into<String>) -> Self {
        Self {
            http,
            base: base.into().trim_end_matches('/').to_string(),
        }
    }

    /// Busca modpacks (solo los que tienen versión para servidor no se puede filtrar aquí: el
    /// plan lo comprueba al leer el `.mrpack`).
    pub fn search_modpacks(&self, query: &str, sort: SearchSort, offset: u64) -> Result<SearchPage, ContentError> {
        let index = match sort {
            SearchSort::Relevance => "relevance",
            SearchSort::Downloads => "downloads",
            SearchSort::Updated => "updated",
            SearchSort::Newest => "newest",
        };
        let q: String = query.trim().chars().take(100).collect();
        let facets = json!([["project_type:modpack"]]).to_string();
        let path = format!(
            "/search?query={}&facets={}&index={index}&offset={offset}&limit=20",
            encode(&q),
            encode(&facets)
        );
        parse_search(&self.get(&path)?)
    }

    /// Busca paquetes de recursos (texturas/sonidos) para una versión de Minecraft.
    pub fn search_resourcepacks(
        &self,
        query: &str,
        mc: Option<&str>,
        sort: SearchSort,
        offset: u64,
    ) -> Result<SearchPage, ContentError> {
        let index = match sort {
            SearchSort::Relevance => "relevance",
            SearchSort::Downloads => "downloads",
            SearchSort::Updated => "updated",
            SearchSort::Newest => "newest",
        };
        let q: String = query.trim().chars().take(100).collect();
        let path = format!(
            "/search?query={}&facets={}&index={index}&offset={offset}&limit=20",
            encode(&q),
            encode(&resourcepack_facets(mc).to_string())
        );
        parse_search(&self.get(&path)?)
    }

    /// Todas las versiones de un proyecto (para modpacks: cada una fija su Minecraft y loader).
    pub fn all_versions(&self, project: &str) -> Result<Vec<ContentVersion>, ContentError> {
        let v = self.get(&format!("/project/{}/version", check_id(project)?))?;
        Ok(v.as_array()
            .ok_or_else(|| bad("versiones"))?
            .iter()
            .filter_map(|x| parse_version(x).ok())
            .collect())
    }

    fn get(&self, path: &str) -> Result<Value, ContentError> {
        Ok(self.http.get_json(&format!("{}{path}", self.base))?)
    }
}

/// Identificadores de Modrinth: letras, números, `-` y `_` (también slugs).
fn check_id(id: &str) -> Result<&str, ContentError> {
    if id.is_empty() || id.len() > 64 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(ContentError::Invalid(format!("identificador no válido: «{id}»")));
    }
    Ok(id)
}

impl Source for Modrinth<'_> {
    fn search(
        &self,
        platform: &Platform,
        mc: Option<&str>,
        query: &str,
        sort: SearchSort,
        offset: u64,
    ) -> Result<SearchPage, ContentError> {
        let index = match sort {
            SearchSort::Relevance => "relevance",
            SearchSort::Downloads => "downloads",
            SearchSort::Updated => "updated",
            SearchSort::Newest => "newest",
        };
        let q: String = query.trim().chars().take(100).collect();
        let path = format!(
            "/search?query={}&facets={}&index={index}&offset={offset}&limit=20",
            encode(&q),
            encode(&search_facets(platform, mc).to_string())
        );
        parse_search(&self.get(&path)?)
    }

    fn project(&self, id: &str) -> Result<ContentProject, ContentError> {
        parse_project(&self.get(&format!("/project/{}", check_id(id)?))?)
    }

    fn projects(&self, ids: &[String]) -> Result<Vec<ContentProject>, ContentError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        for id in ids {
            check_id(id)?;
        }
        let v = self.get(&format!("/projects?ids={}", encode(&json!(ids).to_string())))?;
        Ok(v.as_array()
            .ok_or_else(|| bad("proyectos"))?
            .iter()
            .filter_map(|p| parse_project(p).ok())
            .collect())
    }

    fn versions(
        &self,
        project: &str,
        loaders: &[String],
        mc: Option<&str>,
    ) -> Result<Vec<ContentVersion>, ContentError> {
        let mut path = format!(
            "/project/{}/version?loaders={}",
            check_id(project)?,
            encode(&json!(loaders).to_string())
        );
        if let Some(v) = mc {
            path.push_str(&format!("&game_versions={}", encode(&json!([v]).to_string())));
        }
        let v = self.get(&path)?;
        Ok(v.as_array()
            .ok_or_else(|| bad("versiones"))?
            .iter()
            .filter_map(|x| parse_version(x).ok())
            .collect())
    }

    fn version(&self, id: &str) -> Result<ContentVersion, ContentError> {
        parse_version(&self.get(&format!("/version/{}", check_id(id)?))?)
    }

    fn versions_by_sha1(&self, hashes: &[String]) -> Result<HashMap<String, ContentVersion>, ContentError> {
        if hashes.is_empty() {
            return Ok(HashMap::new());
        }
        let v = self.http.post_json(
            &format!("{}/version_files", self.base),
            &json!({ "hashes": hashes, "algorithm": "sha1" }),
        )?;
        parse_hash_map(&v)
    }

    fn latest_by_sha1(
        &self,
        hashes: &[String],
        loaders: &[String],
        mc: Option<&str>,
    ) -> Result<HashMap<String, ContentVersion>, ContentError> {
        if hashes.is_empty() {
            return Ok(HashMap::new());
        }
        let mut body = json!({ "hashes": hashes, "algorithm": "sha1", "loaders": loaders });
        if let Some(v) = mc {
            body["game_versions"] = json!([v]);
        }
        let v = self
            .http
            .post_json(&format!("{}/version_files/update", self.base), &body)?;
        parse_hash_map(&v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facets_for_resourcepacks() {
        assert_eq!(
            resourcepack_facets(Some("1.21.1")).to_string(),
            r#"[["project_type:resourcepack"],["versions:1.21.1"]]"#
        );
        assert_eq!(
            resourcepack_facets(None).to_string(),
            r#"[["project_type:resourcepack"]]"#
        );
    }

    #[test]
    fn facets_for_mods_and_plugins() {
        let mods = Platform {
            kind: ContentKind::Mod,
            folder: "mods".into(),
            loaders: vec!["quilt".into(), "fabric".into()],
            experimental: false,
            sources: vec![],
        };
        assert_eq!(
            search_facets(&mods, Some("1.21.1")).to_string(),
            r#"[["project_type:mod"],["server_side:required","server_side:optional"],["categories:quilt","categories:fabric"],["versions:1.21.1"]]"#
        );
        let plugins = Platform {
            kind: ContentKind::Plugin,
            folder: "plugins".into(),
            loaders: vec!["paper".into()],
            experimental: false,
            sources: vec![],
        };
        assert_eq!(search_facets(&plugins, None).to_string(), r#"[["categories:paper"]]"#);
        assert_eq!(encode("a b/ñ"), "a%20b%2F%C3%B1");
    }

    #[test]
    fn parses_documented_shapes() {
        let v = json!({
            "id": "AANobbMI", "project_id": "AANobbMI", "name": "Sodium 0.6", "version_number": "mc1.21.1-0.6.0",
            "version_type": "release", "game_versions": ["1.21.1"], "loaders": ["fabric"],
            "date_published": "2024-08-01T00:00:00Z", "downloads": 10, "changelog": null,
            "dependencies": [
                {"version_id": null, "project_id": "P7dR8mSH", "file_name": null, "dependency_type": "required"},
                {"version_id": null, "project_id": null, "file_name": "x.jar", "dependency_type": "raro-nuevo"}
            ],
            "files": [
                {"hashes": {"sha1": "ABC", "sha512": "DEF"}, "url": "https://cdn.modrinth.com/a.jar",
                 "filename": "a.jar", "primary": true, "size": 5, "file_type": null}
            ]
        });
        let p = parse_version(&v).unwrap();
        assert_eq!(p.dependencies.len(), 1, "los tipos desconocidos se ignoran");
        assert_eq!(p.primary_file().unwrap().sha1.as_deref(), Some("ABC"));
        assert!(p.fits(&["quilt".into(), "fabric".into()], Some("1.21.1")));
        assert!(!p.fits(&["forge".into()], Some("1.21.1")));
        assert!(!p.fits(&["fabric".into()], Some("1.20.1")));

        let proj = parse_project(&json!({
            "id": "AANobbMI", "slug": "sodium", "title": "Sodium", "client_side": "required",
            "server_side": "unsupported", "license": {"id": "LicenseRef-Polyform", "name": ""}
        }))
        .unwrap();
        assert_eq!(proj.server_side, Side::Unsupported);
        assert_eq!(proj.license, "LicenseRef-Polyform");
        assert_eq!(proj.page_url, "https://modrinth.com/project/sodium");
        assert_eq!(
            side(&json!({"server_side": "algo-nuevo"}), "server_side"),
            Side::Unknown
        );
        assert!(check_id("../x").is_err());
    }
}
