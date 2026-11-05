//! Catálogo de CurseForge (API REST v1: <https://docs.curseforge.com/rest-api/>).
//!
//! CurseForge exige una clave de API (`x-api-key`) que Overwolf concede a cada aplicación tras
//! revisar una solicitud. La clave nunca va en el repositorio: se inyecta al compilar o la pone
//! quien desarrolla en su PC. Sin clave, CurseForge simplemente no aparece en la app.
//!
//! Reglas de CurseForge que se respetan aquí:
//! - Si el autor no permite la distribución por terceros (`allowModDistribution: false`, o el
//!   archivo llega sin `downloadUrl`), no se descarga: se explica cómo hacerlo a mano.
//! - Lo instalado se reconoce por su huella (`fingerprint`, MurmurHash2), como hace CurseForge.

use crate::local::{InstalledItem, UpdateInfo};
use crate::modrinth::{
    ContentHit, ContentProject, ContentVersion, Dependency, DependencyType, SearchPage, SearchSort, Side, Source,
    VersionFile,
};
use crate::{ContentError, ContentKind, ContentSource, Platform};
use merc_install::Http;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

pub const CURSEFORGE_API: &str = "https://api.curseforge.com";
/// Identificador de Minecraft en CurseForge.
pub const MINECRAFT: u64 = 432;
/// Clase «Mods» de Minecraft.
pub const CLASS_MODS: u64 = 6;
/// Clase «Modpacks» de Minecraft.
pub const CLASS_MODPACKS: u64 = 4471;

/// Juego «Minecraft Bedrock» en CurseForge. Sus identificadores (juego y clase «Addons») se
/// buscan en la API en vez de escribirlos aquí: así, si la clave no incluye Bedrock, se sabe.
pub const BEDROCK_GAME_SLUG: &str = "minecraft-bedrock";
pub const BEDROCK_ADDONS_SLUG: &str = "addons";
/// Extensiones de archivo de un addon que MercHosting sabe instalar.
pub const ADDON_EXTENSIONS: [&str; 3] = [".mcaddon", ".mcpack", ".zip"];

/// Dónde están los addons de Bedrock en CurseForge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BedrockCatalog {
    pub game_id: u64,
    pub class_id: u64,
}

/// ¿Se puede instalar este archivo de CurseForge como addon? (extensión conocida y el autor deja
/// descargarlo desde otras apps).
pub fn is_installable_addon(v: &ContentVersion) -> bool {
    v.files.first().is_some_and(|f| {
        let name = f.filename.to_ascii_lowercase();
        !f.url.is_empty() && ADDON_EXTENSIONS.iter().any(|e| name.ends_with(e))
    })
}

/// Elige qué archivo instalar: el más nuevo estable que se pueda instalar; si no hay estables,
/// el más nuevo que se pueda. `files` llega con los más nuevos primero.
pub fn pick_addon_file(files: &[ContentVersion]) -> Option<&ContentVersion> {
    files
        .iter()
        .find(|f| f.version_type == "release" && is_installable_addon(f))
        .or_else(|| files.iter().find(|f| is_installable_addon(f)))
}

/// `modLoaderType` de CurseForge para un loader de Modrinth.
pub fn loader_type(loader: &str) -> Option<u32> {
    match loader {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

/// Huella de CurseForge: MurmurHash2 (semilla 1) del archivo sin espacios, tabuladores ni saltos
/// de línea (bytes 9, 10, 13 y 32).
pub fn fingerprint(data: &[u8]) -> u32 {
    let bytes: Vec<u8> = data
        .iter()
        .copied()
        .filter(|b| !matches!(b, 9 | 10 | 13 | 32))
        .collect();
    murmur2(&bytes, 1)
}

pub fn fingerprint_file(p: &Path) -> Result<u32, ContentError> {
    let mut f = std::fs::File::open(p).map_err(|e| ContentError::Io(format!("{}: {e}", p.display())))?;
    let mut data = vec![];
    f.read_to_end(&mut data)
        .map_err(|e| ContentError::Io(format!("{}: {e}", p.display())))?;
    Ok(fingerprint(&data))
}

/// MurmurHash2 de 32 bits (Austin Appleby), en little-endian.
pub fn murmur2(data: &[u8], seed: u32) -> u32 {
    const M: u32 = 0x5bd1_e995;
    let len = data.len();
    let mut h: u32 = seed ^ (len as u32);
    let mut chunks = data.chunks_exact(4);
    for c in &mut chunks {
        let mut k = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        k = k.wrapping_mul(M);
        k ^= k >> 24;
        k = k.wrapping_mul(M);
        h = h.wrapping_mul(M);
        h ^= k;
    }
    let tail = chunks.remainder();
    if tail.len() >= 3 {
        h ^= (tail[2] as u32) << 16;
    }
    if tail.len() >= 2 {
        h ^= (tail[1] as u32) << 8;
    }
    if !tail.is_empty() {
        h ^= tail[0] as u32;
        h = h.wrapping_mul(M);
    }
    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^= h >> 15;
    h
}

pub struct CurseForge<'a> {
    http: &'a Http,
    base: String,
    key: String,
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn num(v: &Value, k: &str) -> u64 {
    v.get(k).and_then(Value::as_u64).unwrap_or(0)
}

fn bad(what: &str) -> ContentError {
    ContentError::BadResponse(format!("CurseForge respondió algo inesperado ({what})"))
}

/// Etiquetas de `gameVersions` de un archivo → loaders de Modrinth.
fn loaders_of(tags: &[String]) -> Vec<String> {
    tags.iter()
        .filter_map(|t| match t.to_lowercase().as_str() {
            "forge" => Some("forge"),
            "neoforge" => Some("neoforge"),
            "fabric" => Some("fabric"),
            "quilt" => Some("quilt"),
            _ => None,
        })
        .map(str::to_string)
        .collect()
}

pub fn parse_mod(v: &Value) -> Result<ContentProject, ContentError> {
    let id = num(v, "id");
    if id == 0 {
        return Err(bad("mod"));
    }
    let slug = s(v, "slug");
    let page_url = v
        .get("links")
        .map(|l| s(l, "websiteUrl"))
        .filter(|u| u.starts_with("https://www.curseforge.com/"))
        .unwrap_or_else(|| format!("https://www.curseforge.com/minecraft/mc-mods/{slug}"));
    Ok(ContentProject {
        id: id.to_string(),
        slug,
        title: s(v, "name"),
        description: s(v, "summary"),
        body: String::new(),
        icon_url: v.get("logo").map(|l| s(l, "thumbnailUrl")).filter(|u| !u.is_empty()),
        client_side: Side::Unknown,
        server_side: Side::Unknown,
        license: String::new(),
        downloads: v.get("downloadCount").and_then(Value::as_f64).unwrap_or(0.0) as u64,
        categories: v
            .get("categories")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|c| s(c, "name")).filter(|n| !n.is_empty()).collect())
            .unwrap_or_default(),
        source_url: v.get("links").map(|l| s(l, "sourceUrl")).filter(|u| !u.is_empty()),
        issues_url: v.get("links").map(|l| s(l, "issuesUrl")).filter(|u| !u.is_empty()),
        wiki_url: v.get("links").map(|l| s(l, "wikiUrl")).filter(|u| !u.is_empty()),
        page_url,
        // `null` = no lo indicó; igualmente, si un archivo llega sin `downloadUrl` no se descarga.
        distribution_allowed: v.get("allowModDistribution").and_then(Value::as_bool) != Some(false),
    })
}

fn parse_hit(v: &Value) -> Result<ContentHit, ContentError> {
    let p = parse_mod(v)?;
    Ok(ContentHit {
        project_id: p.id,
        slug: p.slug,
        title: p.title,
        author: v
            .get("authors")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .map(|a| s(a, "name"))
            .unwrap_or_default(),
        description: p.description,
        icon_url: p.icon_url,
        downloads: p.downloads,
        categories: p.categories,
        client_side: Side::Unknown,
        server_side: Side::Unknown,
        date_modified: s(v, "dateModified"),
        license: String::new(),
    })
}

pub fn parse_file(v: &Value) -> Result<ContentVersion, ContentError> {
    let id = num(v, "id");
    if id == 0 {
        return Err(bad("archivo"));
    }
    let tags: Vec<String> = v
        .get("gameVersions")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default();
    let has = |t: &str| tags.iter().any(|x| x.eq_ignore_ascii_case(t));
    let server_side = match (has("Server"), has("Client")) {
        (true, _) => Side::Required,
        (false, true) => Side::Unsupported,
        _ => Side::Unknown,
    };
    let game_versions = tags
        .iter()
        .filter(|t| t.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .cloned()
        .collect();
    let sha1 = v.get("hashes").and_then(Value::as_array).and_then(|a| {
        a.iter()
            .find(|h| h.get("algo").and_then(Value::as_u64) == Some(1))
            .map(|h| s(h, "value").to_lowercase())
    });
    let dependencies = v
        .get("dependencies")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|d| {
                    let t = match d.get("relationType").and_then(Value::as_u64)? {
                        3 => DependencyType::Required,
                        2 => DependencyType::Optional,
                        5 => DependencyType::Incompatible,
                        1 | 6 => DependencyType::Embedded,
                        _ => return None,
                    };
                    Some(Dependency {
                        version_id: None,
                        project_id: Some(num(d, "modId").to_string()).filter(|x| x != "0"),
                        file_name: None,
                        dependency_type: t,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let version_type = match num(v, "releaseType") {
        2 => "beta",
        3 => "alpha",
        _ => "release",
    };
    let display = s(v, "displayName");
    Ok(ContentVersion {
        id: id.to_string(),
        project_id: num(v, "modId").to_string(),
        name: display.clone(),
        version_number: if display.is_empty() { s(v, "fileName") } else { display },
        version_type: version_type.into(),
        game_versions,
        loaders: loaders_of(&tags),
        date_published: s(v, "fileDate"),
        downloads: num(v, "downloadCount"),
        changelog: None,
        dependencies,
        files: vec![VersionFile {
            // Vacía si el autor no permite descargas desde otras apps.
            url: s(v, "downloadUrl"),
            filename: s(v, "fileName"),
            primary: true,
            size: num(v, "fileLength"),
            sha1,
            sha512: None,
            sha256: None,
        }],
        server_side,
    })
}

impl<'a> CurseForge<'a> {
    pub fn new(http: &'a Http, base: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            http,
            base: base.into().trim_end_matches('/').to_string(),
            key: key.into(),
        }
    }

    fn map_err(e: merc_install::InstallError) -> ContentError {
        let msg = e.to_string();
        if msg.contains("HTTP 403") || msg.contains("HTTP 401") {
            ContentError::Network("CurseForge rechazó la clave de API de esta instalación".into())
        } else {
            e.into()
        }
    }

    fn get(&self, path: &str) -> Result<Value, ContentError> {
        self.http
            .get_json_with(
                &format!("{}{path}", self.base),
                &[("x-api-key", &self.key), ("Accept", "application/json")],
            )
            .map_err(Self::map_err)
    }

    fn post(&self, path: &str, body: Value) -> Result<Value, ContentError> {
        self.http
            .post_json_with(
                &format!("{}{path}", self.base),
                &body,
                &[("x-api-key", &self.key), ("Accept", "application/json")],
            )
            .map_err(Self::map_err)
    }

    fn data(v: &Value) -> Result<&Value, ContentError> {
        v.get("data").ok_or_else(|| bad("sin «data»"))
    }

    /// Busca modpacks de Minecraft.
    pub fn search_modpacks(&self, query: &str, sort: SearchSort, offset: u64) -> Result<SearchPage, ContentError> {
        let sort_field = match sort {
            SearchSort::Relevance => 2,
            SearchSort::Downloads => 6,
            SearchSort::Updated => 3,
            SearchSort::Newest => 11,
        };
        let q: String = query.trim().chars().take(100).collect();
        let path = format!(
            "/v1/mods/search?gameId={MINECRAFT}&classId={CLASS_MODPACKS}&searchFilter={}&sortField={sort_field}&sortOrder=desc&index={offset}&pageSize=20",
            crate::modrinth::encode(&q)
        );
        let v = self.get(&path)?;
        let hits = Self::data(&v)?
            .as_array()
            .ok_or_else(|| bad("búsqueda"))?
            .iter()
            .filter_map(|m| parse_hit(m).ok())
            .collect();
        let total = v
            .get("pagination")
            .and_then(|p| p.get("totalCount"))
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .min(10_000);
        Ok(SearchPage { hits, total, offset })
    }

    /// Juego y clase de los addons de Bedrock. Error claro si la clave no incluye Bedrock.
    pub fn bedrock_catalog(&self) -> Result<BedrockCatalog, ContentError> {
        let v = self.get("/v1/games?index=0&pageSize=50")?;
        let game_id = Self::data(&v)?
            .as_array()
            .ok_or_else(|| bad("juegos"))?
            .iter()
            .find(|g| s(g, "slug") == BEDROCK_GAME_SLUG)
            .map(|g| num(g, "id"))
            .filter(|n| *n > 0)
            .ok_or_else(|| {
                ContentError::NotFound("la clave de CurseForge de MercHosting no incluye Minecraft Bedrock".into())
            })?;
        let v = self.get(&format!("/v1/categories?gameId={game_id}&classesOnly=true"))?;
        let class_id = Self::data(&v)?
            .as_array()
            .ok_or_else(|| bad("clases"))?
            .iter()
            .find(|c| s(c, "slug") == BEDROCK_ADDONS_SLUG || s(c, "name").eq_ignore_ascii_case("addons"))
            .map(|c| num(c, "id"))
            .filter(|n| *n > 0)
            .ok_or_else(|| ContentError::NotFound("CurseForge no tiene la sección de addons de Bedrock".into()))?;
        Ok(BedrockCatalog { game_id, class_id })
    }

    /// Busca addons de Minecraft Bedrock.
    pub fn search_bedrock_addons(
        &self,
        cat: BedrockCatalog,
        query: &str,
        sort: SearchSort,
        offset: u64,
    ) -> Result<SearchPage, ContentError> {
        let sort_field = match sort {
            SearchSort::Relevance => 2,
            SearchSort::Downloads => 6,
            SearchSort::Updated => 3,
            SearchSort::Newest => 11,
        };
        let q: String = query.trim().chars().take(100).collect();
        let path = format!(
            "/v1/mods/search?gameId={}&classId={}&searchFilter={}&sortField={sort_field}&sortOrder=desc&index={offset}&pageSize=20",
            cat.game_id,
            cat.class_id,
            crate::modrinth::encode(&q)
        );
        let v = self.get(&path)?;
        let hits = Self::data(&v)?
            .as_array()
            .ok_or_else(|| bad("búsqueda"))?
            .iter()
            .filter(|m| num(m, "gameId") == 0 || num(m, "gameId") == cat.game_id)
            .filter_map(|m| parse_hit(m).ok())
            .collect();
        let total = v
            .get("pagination")
            .and_then(|p| p.get("totalCount"))
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .min(10_000);
        Ok(SearchPage { hits, total, offset })
    }

    /// El proyecto es un addon de Bedrock (y no un mod de Java con el mismo número).
    pub fn is_bedrock_addon(&self, cat: BedrockCatalog, project: &str) -> Result<bool, ContentError> {
        let v = self.get(&format!("/v1/mods/{}", check_num(project)?))?;
        let m = Self::data(&v)?;
        Ok(num(m, "gameId") == cat.game_id && num(m, "classId") == cat.class_id)
    }

    /// Archivos publicados de un proyecto (los más nuevos primero), sin filtrar.
    pub fn all_files(&self, project: &str) -> Result<Vec<ContentVersion>, ContentError> {
        let v = self.get(&format!("/v1/mods/{}/files?pageSize=50", check_num(project)?))?;
        let mut list: Vec<ContentVersion> = Self::data(&v)?
            .as_array()
            .ok_or_else(|| bad("archivos"))?
            .iter()
            .filter_map(|f| parse_file(f).ok())
            .collect();
        list.sort_by(|a, b| b.date_published.cmp(&a.date_published));
        Ok(list)
    }

    /// Varios archivos por identificador (en lotes de 100).
    pub fn files_by_ids(&self, ids: &[u64]) -> Result<Vec<ContentVersion>, ContentError> {
        let mut out = vec![];
        for chunk in ids.chunks(100) {
            let v = self.post("/v1/mods/files", json!({ "fileIds": chunk }))?;
            out.extend(
                Self::data(&v)?
                    .as_array()
                    .ok_or_else(|| bad("archivos"))?
                    .iter()
                    .filter_map(|f| parse_file(f).ok()),
            );
        }
        Ok(out)
    }

    /// Clase (mods, resource packs, shaders…) y página de cada proyecto, en lotes de 100.
    pub fn classes(&self, ids: &[u64]) -> Result<HashMap<u64, (u64, ContentProject)>, ContentError> {
        let mut out = HashMap::new();
        for chunk in ids.chunks(100) {
            let v = self.post("/v1/mods", json!({ "modIds": chunk }))?;
            for m in Self::data(&v)?.as_array().ok_or_else(|| bad("mods"))? {
                if let Ok(p) = parse_mod(m) {
                    out.insert(num(m, "id"), (num(m, "classId"), p));
                }
            }
        }
        Ok(out)
    }

    /// Reconoce archivos por su huella. Devuelve, por huella, el archivo y los últimos publicados.
    pub fn match_fingerprints(
        &self,
        fingerprints: &[u32],
    ) -> Result<HashMap<u32, (ContentVersion, Vec<ContentVersion>)>, ContentError> {
        if fingerprints.is_empty() {
            return Ok(HashMap::new());
        }
        let v = self.post(
            &format!("/v1/fingerprints/{MINECRAFT}"),
            json!({ "fingerprints": fingerprints }),
        )?;
        let mut out = HashMap::new();
        for m in Self::data(&v)?
            .get("exactMatches")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(file) = m.get("file") else { continue };
            let Some(fp) = file.get("fileFingerprint").and_then(Value::as_u64) else {
                continue;
            };
            let Ok(cur) = parse_file(file) else { continue };
            let latest = m
                .get("latestFiles")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|f| parse_file(f).ok()).collect())
                .unwrap_or_default();
            out.insert(fp as u32, (cur, latest));
        }
        Ok(out)
    }
}

fn check_num(id: &str) -> Result<u64, ContentError> {
    id.parse::<u64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| ContentError::Invalid(format!("identificador de CurseForge no válido: «{id}»")))
}

impl Source for CurseForge<'_> {
    fn search(
        &self,
        platform: &Platform,
        mc: Option<&str>,
        query: &str,
        sort: SearchSort,
        offset: u64,
    ) -> Result<SearchPage, ContentError> {
        if platform.kind != ContentKind::Mod {
            return Err(ContentError::Invalid("en CurseForge solo se buscan mods".into()));
        }
        // sortField: 1 destacados, 2 popularidad, 3 última actualización, 6 descargas totales, 11 fecha de publicación.
        let sort_field = match sort {
            SearchSort::Relevance => 2,
            SearchSort::Downloads => 6,
            SearchSort::Updated => 3,
            SearchSort::Newest => 11,
        };
        let q: String = query.trim().chars().take(100).collect();
        let mut path = format!(
            "/v1/mods/search?gameId={MINECRAFT}&classId={CLASS_MODS}&searchFilter={}&sortField={sort_field}&sortOrder=desc&index={offset}&pageSize=20",
            crate::modrinth::encode(&q)
        );
        if let Some(v) = mc {
            path.push_str(&format!("&gameVersion={}", crate::modrinth::encode(v)));
        }
        let types: Vec<String> = platform
            .loaders
            .iter()
            .filter_map(|l| loader_type(l))
            .map(|n| n.to_string())
            .collect();
        match types.len() {
            0 => {}
            1 => path.push_str(&format!("&modLoaderType={}", types[0])),
            _ => path.push_str(&format!(
                "&modLoaderTypes={}",
                crate::modrinth::encode(&format!("[{}]", types.join(",")))
            )),
        }
        let v = self.get(&path)?;
        let hits = Self::data(&v)?
            .as_array()
            .ok_or_else(|| bad("búsqueda"))?
            .iter()
            .filter_map(|m| parse_hit(m).ok())
            .collect();
        let total = v
            .get("pagination")
            .and_then(|p| p.get("totalCount"))
            .and_then(Value::as_u64)
            .unwrap_or(0)
            // La API no deja pasar del resultado 10 000.
            .min(10_000);
        Ok(SearchPage { hits, total, offset })
    }

    fn project(&self, id: &str) -> Result<ContentProject, ContentError> {
        let v = self.get(&format!("/v1/mods/{}", check_num(id)?))?;
        parse_mod(Self::data(&v)?)
    }

    fn projects(&self, ids: &[String]) -> Result<Vec<ContentProject>, ContentError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let nums: Vec<u64> = ids.iter().map(|i| check_num(i)).collect::<Result<_, _>>()?;
        let v = self.post("/v1/mods", json!({ "modIds": nums }))?;
        Ok(Self::data(&v)?
            .as_array()
            .ok_or_else(|| bad("mods"))?
            .iter()
            .filter_map(|m| parse_mod(m).ok())
            .collect())
    }

    fn versions(
        &self,
        project: &str,
        loaders: &[String],
        mc: Option<&str>,
    ) -> Result<Vec<ContentVersion>, ContentError> {
        let mut path = format!("/v1/mods/{}/files?pageSize=50", check_num(project)?);
        if let Some(v) = mc {
            path.push_str(&format!("&gameVersion={}", crate::modrinth::encode(v)));
        }
        if let [one] = loaders {
            if let Some(t) = loader_type(one) {
                path.push_str(&format!("&modLoaderType={t}"));
            }
        }
        let v = self.get(&path)?;
        let mut list: Vec<ContentVersion> = Self::data(&v)?
            .as_array()
            .ok_or_else(|| bad("archivos"))?
            .iter()
            .filter_map(|f| parse_file(f).ok())
            // Los archivos sin loader declarado no se dan por compatibles.
            .filter(|f| f.fits(loaders, mc))
            .collect();
        list.sort_by(|a, b| b.date_published.cmp(&a.date_published));
        Ok(list)
    }

    fn version(&self, id: &str) -> Result<ContentVersion, ContentError> {
        let n = check_num(id)?;
        let v = self.post("/v1/mods/files", json!({ "fileIds": [n] }))?;
        Self::data(&v)?
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|f| parse_file(f).ok())
            .find(|f| f.id == n.to_string())
            .ok_or_else(|| ContentError::NotFound(format!("archivo {id} de CurseForge")))
    }

    fn versions_by_sha1(&self, _: &[String]) -> Result<HashMap<String, ContentVersion>, ContentError> {
        // CurseForge reconoce por huella, no por SHA-1: ver [`enrich_curseforge`].
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

/// Completa con CurseForge lo instalado. Lo que Modrinth no reconoció pasa a ser de CurseForge;
/// lo que ya era de Modrinth solo guarda el identificador de CurseForge (para no instalarlo dos veces).
pub fn enrich_curseforge(
    cf: &CurseForge,
    root: &Path,
    platform: &Platform,
    mc: Option<&str>,
    items: &mut [InstalledItem],
) -> Result<(), ContentError> {
    let mut fps = vec![];
    for it in items.iter() {
        fps.push(fingerprint_file(&root.join(&platform.folder).join(&it.file_name))?);
    }
    let found = cf.match_fingerprints(&fps)?;
    let mut ids: Vec<String> = found.values().map(|(f, _)| f.project_id.clone()).collect();
    ids.sort();
    ids.dedup();
    let projects = cf.projects(&ids).unwrap_or_default();
    for (it, fp) in items.iter_mut().zip(fps) {
        let Some((file, latest)) = found.get(&fp) else { continue };
        if it.source == Some(ContentSource::Modrinth) {
            it.other_ids.push(file.project_id.clone());
            continue;
        }
        it.source = Some(ContentSource::Curseforge);
        it.project_id = Some(file.project_id.clone());
        it.version_id = Some(file.id.clone());
        it.version_number = Some(file.version_number.clone());
        if let Some(p) = projects.iter().find(|p| p.id == file.project_id) {
            it.title = Some(p.title.clone());
            it.icon_url = p.icon_url.clone();
            it.page_url = Some(p.page_url.clone()).filter(|u| !u.is_empty());
        }
        it.server_side = Some(file.server_side);
        if it.enabled {
            let newest = latest
                .iter()
                .filter(|l| l.fits(&platform.loaders, mc) && l.version_type == "release")
                .max_by(|a, b| a.date_published.cmp(&b.date_published));
            if let Some(n) = newest {
                if n.id != file.id && n.date_published > file.date_published {
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

    #[test]
    fn murmur2_matches_reference_values() {
        // Valores de la implementación de referencia de MurmurHash2 (32 bits).
        assert_eq!(murmur2(b"", 0), 0);
        assert_eq!(murmur2(b"", 1), 0x5bd1_5e36);
        // La huella ignora espacios y saltos de línea.
        assert_eq!(fingerprint(b"a b\r\nc\td"), fingerprint(b"abcd"));
        assert_ne!(fingerprint(b"abcd"), fingerprint(b"abce"));
    }

    #[test]
    fn parses_documented_file_and_mod() {
        let f = parse_file(&json!({
            "id": 5000001, "modId": 238222, "displayName": "jei-1.21.1-neoforge-19.21.0.247.jar",
            "fileName": "jei-1.21.1-neoforge-19.21.0.247.jar", "releaseType": 1, "fileDate": "2026-09-01T10:00:00Z",
            "fileLength": 1234, "downloadCount": 9, "downloadUrl": "https://edge.forgecdn.net/files/5000/1/jei.jar",
            "gameVersions": ["1.21.1", "NeoForge", "Server", "Client"],
            "hashes": [{ "value": "ABCDEF0123456789ABCDEF0123456789ABCDEF01", "algo": 1 }, { "value": "x", "algo": 2 }],
            "dependencies": [{ "modId": 306612, "relationType": 3 }, { "modId": 1, "relationType": 4 }],
            "fileFingerprint": 123456
        }))
        .unwrap();
        assert_eq!(f.project_id, "238222");
        assert_eq!(f.loaders, ["neoforge"]);
        assert_eq!(f.game_versions, ["1.21.1"]);
        assert_eq!(f.server_side, Side::Required);
        assert_eq!(
            f.files[0].sha1.as_deref(),
            Some("abcdef0123456789abcdef0123456789abcdef01")
        );
        assert_eq!(f.dependencies.len(), 1);
        assert!(f.fits(&["neoforge".into()], Some("1.21.1")));

        let client_only =
            parse_file(&json!({ "id": 2, "modId": 3, "gameVersions": ["1.21.1", "Fabric", "Client"] })).unwrap();
        assert_eq!(client_only.server_side, Side::Unsupported);

        let m = parse_mod(&json!({ "id": 238222, "name": "Just Enough Items (JEI)", "slug": "jei",
            "summary": "Ver recetas", "downloadCount": 4.5e8, "allowModDistribution": false,
            "links": { "websiteUrl": "https://www.curseforge.com/minecraft/mc-mods/jei" },
            "logo": { "thumbnailUrl": "https://media.forgecdn.net/avatars/thumbnails/1/1/256/256/x.png" } }))
        .unwrap();
        assert!(!m.distribution_allowed);
        assert_eq!(m.downloads, 450_000_000);
        assert_eq!(m.page_url, "https://www.curseforge.com/minecraft/mc-mods/jei");
        assert!(check_num("../1").is_err());
    }
}
