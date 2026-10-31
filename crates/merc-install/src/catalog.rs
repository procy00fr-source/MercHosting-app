//! Catálogo de versiones desde las fuentes oficiales de cada proyecto.
//!
//! Las funciones `parse_*` son puras (reciben el JSON/XML ya descargado) para poder
//! probarlas con muestras; `list_versions` y `resolve` hacen las peticiones.

use crate::http::Http;
use crate::{Checksum, InstallError, Sources};
use merc_core::detect::{looks_like_mc, neoforge_to_mc, version_cmp, ServerKind};
use merc_core::java::recommended_java_major;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Flavor {
    Vanilla,
    Paper,
    Purpur,
    /// Compilado en el PC del usuario con BuildTools, la herramienta oficial de SpigotMC.
    Spigot,
    Fabric,
    Quilt,
    Forge,
    NeoForge,
    /// Híbrido de mods (NeoForge, Forge o Fabric) + plugins de Bukkit (Arclight, GPL-3.0).
    Arclight,
}

impl Flavor {
    pub fn kind(self) -> ServerKind {
        match self {
            Flavor::Vanilla => ServerKind::Vanilla,
            Flavor::Paper => ServerKind::Paper,
            Flavor::Purpur => ServerKind::Purpur,
            Flavor::Spigot => ServerKind::Spigot,
            Flavor::Fabric => ServerKind::Fabric,
            Flavor::Quilt => ServerKind::Quilt,
            Flavor::Forge => ServerKind::Forge,
            Flavor::NeoForge => ServerKind::NeoForge,
            Flavor::Arclight => ServerKind::Arclight,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub mc_version: String,
    /// Versión de Java recomendada para esta versión de Minecraft.
    pub java_major: u32,
    /// Última versión estable publicada por la fuente.
    pub latest: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub url: String,
    /// Otras direcciones del mismo archivo, por si la principal falla. La huella decide.
    #[serde(default)]
    pub mirrors: Vec<String>,
    pub file_name: String,
    pub checksum: Checksum,
    pub size: Option<u64>,
}

/// Todo lo necesario para instalar una versión concreta, ya resuelto contra la fuente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolved {
    pub flavor: Flavor,
    pub mc_version: String,
    /// Versión del loader (Fabric, NeoForge) o número de compilación (Paper).
    pub loader_version: Option<String>,
    pub java_major: u32,
    /// La fuente no marca esta compilación como estable.
    pub experimental: bool,
    pub artifact: Artifact,
    /// Si el archivo es un instalador oficial: argumentos tras `java -jar <archivo>`
    /// (se ejecuta dentro de la carpeta del servidor y después se borra el instalador).
    #[serde(default)]
    pub installer_args: Option<Vec<String>>,
    /// El instalador trabaja en una carpeta de caché fuera del servidor (BuildTools).
    #[serde(default)]
    pub installer_in_cache: bool,
    /// Necesita un JDK, no solo Java (compila).
    #[serde(default)]
    pub needs_jdk: bool,
    /// Avisos para mostrar al usuario antes de instalar.
    pub notes: Vec<String>,
}

/// Solo caracteres que pueden ir en una ruta de URL sin escapar.
fn safe_segment(s: &str) -> Result<&str, InstallError> {
    let ok = !s.is_empty()
        && s.len() <= 64
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-+".contains(&b))
        && !s.starts_with('.');
    if ok {
        Ok(s)
    } else {
        Err(InstallError::InvalidInput(format!("versión no válida: {s:?}")))
    }
}

fn entries(list: Vec<String>) -> Vec<CatalogEntry> {
    list.into_iter()
        .enumerate()
        .map(|(i, v)| CatalogEntry {
            java_major: recommended_java_major(Some(&v)),
            latest: i == 0,
            mc_version: v,
        })
        .collect()
}

fn bad(what: &str) -> InstallError {
    InstallError::BadResponse(format!("respuesta inesperada de {what}"))
}

// ---------- Vanilla (Mojang) ----------

pub fn parse_mojang_manifest(v: &Value) -> Result<Vec<CatalogEntry>, InstallError> {
    let versions = v["versions"].as_array().ok_or_else(|| bad("Mojang"))?;
    let latest = v["latest"]["release"].as_str();
    let mut list: Vec<String> = versions
        .iter()
        .filter(|x| x["type"] == "release")
        .filter_map(|x| x["id"].as_str().map(str::to_string))
        .collect();
    // Por si el manifiesto no viniera ordenado, la última publicada va primero.
    if let Some(l) = latest {
        if let Some(pos) = list.iter().position(|x| x == l) {
            let item = list.remove(pos);
            list.insert(0, item);
        }
    }
    Ok(entries(list))
}

fn mojang_version_url(manifest: &Value, mc: &str) -> Result<String, InstallError> {
    manifest["versions"]
        .as_array()
        .and_then(|a| a.iter().find(|x| x["id"] == mc))
        .and_then(|x| x["url"].as_str())
        .map(str::to_string)
        .ok_or_else(|| InstallError::NotAvailable(format!("Mojang no publica la versión {mc}")))
}

pub fn parse_mojang_version(v: &Value, mc: &str) -> Result<Resolved, InstallError> {
    let server = &v["downloads"]["server"];
    let url = server["url"]
        .as_str()
        .ok_or_else(|| InstallError::NotAvailable(format!("Mojang no ofrece servidor para {mc}")))?;
    let sha1 = server["sha1"].as_str().ok_or_else(|| bad("Mojang (sin sha1)"))?;
    let java_major = v["javaVersion"]["majorVersion"]
        .as_u64()
        .map(|m| m as u32)
        .unwrap_or_else(|| recommended_java_major(Some(mc)));
    Ok(Resolved {
        flavor: Flavor::Vanilla,
        mc_version: mc.to_string(),
        loader_version: None,
        java_major,
        experimental: false,
        artifact: Artifact {
            mirrors: vec![],
            url: url.to_string(),
            file_name: format!("minecraft_server.{mc}.jar"),
            checksum: Checksum::Sha1(sha1.to_string()),
            size: server["size"].as_u64(),
        },
        installer_args: None,
        installer_in_cache: false,
        needs_jdk: false,
        notes: vec![],
    })
}

// ---------- Paper (Fill v3) ----------

pub fn parse_paper_project(v: &Value) -> Result<Vec<CatalogEntry>, InstallError> {
    let groups = v["versions"].as_object().ok_or_else(|| bad("PaperMC"))?;
    let mut list: Vec<String> = groups
        .values()
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(Value::as_str)
        .filter(|s| looks_like_mc(s))
        .map(str::to_string)
        .collect();
    list.sort_by(|a, b| version_cmp(b, a));
    list.dedup();
    Ok(entries(list))
}

pub fn parse_paper_builds(v: &Value, mc: &str) -> Result<Resolved, InstallError> {
    let builds = v.as_array().ok_or_else(|| bad("PaperMC"))?;
    if builds.is_empty() {
        return Err(InstallError::NotAvailable(format!(
            "PaperMC aún no tiene compilaciones para {mc}"
        )));
    }
    let stable = builds.iter().find(|b| b["channel"] == "STABLE");
    let (build, experimental) = match stable {
        Some(b) => (b, false),
        None => (&builds[0], true),
    };
    let dl = &build["downloads"]["server:default"];
    let url = dl["url"].as_str().ok_or_else(|| bad("PaperMC (sin URL)"))?;
    let sha256 = dl["checksums"]["sha256"]
        .as_str()
        .ok_or_else(|| bad("PaperMC (sin sha256)"))?;
    let id = build["id"].as_u64().map(|n| n.to_string());
    let name = dl["name"].as_str().unwrap_or("");
    let file_name = if valid_jar_name(name) && name.starts_with("paper-") {
        name.to_string()
    } else {
        format!("paper-{mc}-{}.jar", id.clone().unwrap_or_else(|| "0".into()))
    };
    let mut notes =
        vec!["En el primer arranque, Paper descarga el servidor oficial de Mojang (necesita Internet).".to_string()];
    if experimental {
        notes.push(format!(
            "PaperMC todavía no marca ninguna compilación de {mc} como estable. Puede tener errores: haz copias de seguridad."
        ));
    }
    Ok(Resolved {
        flavor: Flavor::Paper,
        mc_version: mc.to_string(),
        loader_version: id,
        java_major: recommended_java_major(Some(mc)),
        experimental,
        artifact: Artifact {
            mirrors: vec![],
            url: url.to_string(),
            file_name,
            checksum: Checksum::Sha256(sha256.to_string()),
            size: dl["size"].as_u64(),
        },
        installer_args: None,
        installer_in_cache: false,
        needs_jdk: false,
        notes,
    })
}

// ---------- Fabric (meta) ----------

pub fn parse_fabric_games(v: &Value) -> Result<Vec<CatalogEntry>, InstallError> {
    let list = v.as_array().ok_or_else(|| bad("Fabric"))?;
    Ok(entries(
        list.iter()
            .filter(|x| x["stable"] == true)
            .filter_map(|x| x["version"].as_str())
            .filter(|s| looks_like_mc(s))
            .map(str::to_string)
            .collect(),
    ))
}

fn first_stable(v: &Value, what: &str) -> Result<String, InstallError> {
    v.as_array()
        .and_then(|a| a.iter().find(|x| x["stable"] == true))
        .and_then(|x| x["version"].as_str())
        .map(str::to_string)
        .ok_or_else(|| bad(what))
}

pub fn fabric_resolved(base: &str, mc: &str, loader: &str, installer: &str) -> Resolved {
    Resolved {
        flavor: Flavor::Fabric,
        mc_version: mc.to_string(),
        loader_version: Some(loader.to_string()),
        java_major: recommended_java_major(Some(mc)),
        experimental: false,
        artifact: Artifact {
            mirrors: vec![],
            url: format!("{base}/v2/versions/loader/{mc}/{loader}/{installer}/server/jar"),
            file_name: format!("fabric-server-mc.{mc}-loader.{loader}-launcher.{installer}.jar"),
            checksum: Checksum::None,
            size: None,
        },
        installer_args: None,
        installer_in_cache: false,
        needs_jdk: false,
        notes: vec![
            "Fabric no publica una huella del lanzador; se descarga por HTTPS desde meta.fabricmc.net.".into(),
            "En el primer arranque, Fabric descarga el servidor oficial de Mojang (necesita Internet).".into(),
        ],
    }
}

// ---------- NeoForge (maven) ----------

pub fn parse_maven_versions(xml: &str) -> Vec<String> {
    let re = regex::Regex::new(r"<version>\s*([^<\s]+)\s*</version>").unwrap();
    re.captures_iter(xml).map(|c| c[1].to_string()).collect()
}

/// Mejor versión de NeoForge para cada versión de Minecraft: la más alta, prefiriendo
/// las que no llevan sufijo (`-beta`).
pub fn neoforge_best(versions: &[String]) -> Vec<(String, String)> {
    let mut best: std::collections::BTreeMap<String, String> = Default::default();
    // Una versión sin sufijo (estable) gana a una beta; a igualdad, la más alta.
    let better = |new: &str, cur: &str| {
        let (ns, cs) = (!new.contains('-'), !cur.contains('-'));
        if ns != cs {
            ns
        } else {
            version_cmp(new, cur).is_gt()
        }
    };
    for v in versions {
        let Some(mc) = neoforge_to_mc(v) else { continue };
        if !looks_like_mc(&mc) {
            continue;
        }
        match best.get(&mc) {
            Some(cur) => {
                if better(v, cur) {
                    best.insert(mc, v.clone());
                }
            }
            None => {
                best.insert(mc, v.clone());
            }
        }
    }
    let mut out: Vec<(String, String)> = best.into_iter().collect();
    out.sort_by(|a, b| version_cmp(&b.0, &a.0));
    out
}

pub fn parse_hash_file(text: &str) -> Option<String> {
    let token = text.split_whitespace().next()?;
    (token.len() >= 40 && token.bytes().all(|b| b.is_ascii_hexdigit())).then(|| token.to_ascii_lowercase())
}

fn neoforge_installer_url(base: &str, v: &str) -> String {
    format!("{base}/net/neoforged/neoforge/{v}/neoforge-{v}-installer.jar")
}

/// Huella de un archivo de un repositorio maven: `.sha256`, y si no hay, `.sha1`.
fn maven_checksum(http: &Http, url: &str, notes: &mut Vec<String>) -> Checksum {
    if let Some(h) = http
        .get_text(&format!("{url}.sha256"))
        .ok()
        .and_then(|t| parse_hash_file(&t))
    {
        if h.len() == 64 {
            return Checksum::Sha256(h);
        }
    }
    if let Some(h) = http
        .get_text(&format!("{url}.sha1"))
        .ok()
        .and_then(|t| parse_hash_file(&t))
    {
        if h.len() == 40 {
            return Checksum::Sha1(h);
        }
    }
    notes.push("El repositorio no publicó la huella del instalador; se descarga por HTTPS.".into());
    Checksum::None
}

pub fn maven_release(xml: &str) -> Option<String> {
    let re = regex::Regex::new(r"<release>\s*([^<\s]+)\s*</release>").unwrap();
    re.captures(xml).map(|c| c[1].to_string())
}

// ---------- Purpur ----------

pub fn parse_purpur_versions(v: &Value) -> Result<Vec<CatalogEntry>, InstallError> {
    let list = v["versions"].as_array().ok_or_else(|| bad("Purpur"))?;
    let mut out: Vec<String> = list
        .iter()
        .filter_map(Value::as_str)
        .filter(|s| looks_like_mc(s))
        .map(str::to_string)
        .collect();
    out.sort_by(|a, b| version_cmp(b, a));
    out.dedup();
    Ok(entries(out))
}

pub fn purpur_resolved(base: &str, mc: &str, build: &str, detail: &Value) -> Resolved {
    let md5 = detail["md5"]
        .as_str()
        .filter(|h| h.len() == 32 && h.bytes().all(|b| b.is_ascii_hexdigit()));
    let mut notes =
        vec!["En el primer arranque, Purpur descarga el servidor oficial de Mojang (necesita Internet).".into()];
    if md5.is_none() {
        notes.push("La API de Purpur no devolvió huella para esta compilación; se descarga por HTTPS.".into());
    }
    Resolved {
        flavor: Flavor::Purpur,
        mc_version: mc.to_string(),
        loader_version: Some(build.to_string()),
        java_major: recommended_java_major(Some(mc)),
        experimental: detail["result"].as_str().is_some_and(|r| r != "SUCCESS"),
        artifact: Artifact {
            mirrors: vec![],
            url: format!("{base}/purpur/{mc}/{build}/download"),
            file_name: format!("purpur-{mc}-{build}.jar"),
            checksum: md5
                .map(|h| Checksum::Md5(h.to_ascii_lowercase()))
                .unwrap_or(Checksum::None),
            size: None,
        },
        installer_args: None,
        installer_in_cache: false,
        needs_jdk: false,
        notes,
    }
}

// ---------- Forge ----------

/// Forge moderno (instalador con `--installServer` y `run.bat`) existe desde Minecraft 1.17.
fn forge_supported(mc: &str) -> bool {
    match mc.split('.').collect::<Vec<_>>().as_slice() {
        ["1", minor, ..] => minor.parse::<u32>().is_ok_and(|m| m >= 17),
        [year, ..] => year.parse::<u32>().is_ok_and(|y| y >= 26),
        _ => false,
    }
}

/// Versiones de Minecraft con Forge (de `promotions_slim.json`), la más nueva primero.
pub fn forge_versions(promos: &Value) -> Vec<String> {
    let mut out: Vec<String> = promos["promos"]
        .as_object()
        .map(|m| {
            m.keys()
                .filter_map(|k| k.rsplit_once('-').map(|(mc, _)| mc.to_string()))
                .filter(|mc| looks_like_mc(mc) && forge_supported(mc))
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| version_cmp(b, a));
    out.dedup();
    out
}

/// Compilación de Forge para una versión: la recomendada; si no hay, la más reciente.
pub fn forge_build_for(promos: &Value, mc: &str) -> Option<(String, bool)> {
    let p = &promos["promos"];
    if let Some(b) = p[format!("{mc}-recommended")].as_str() {
        return Some((b.to_string(), true));
    }
    p[format!("{mc}-latest")].as_str().map(|b| (b.to_string(), false))
}

// ---------- Quilt ----------

/// Primer loader de Quilt sin sufijo beta (la meta los lista del más nuevo al más viejo).
pub fn quilt_stable_loader(v: &Value) -> Option<String> {
    v.as_array()?
        .iter()
        .filter_map(|x| x["version"].as_str())
        .find(|ver| !ver.contains('-'))
        .map(str::to_string)
}

pub fn quilt_args(mc: &str, loader: &str) -> Vec<String> {
    vec![
        "install".into(),
        "server".into(),
        mc.into(),
        loader.into(),
        "--download-server".into(),
        "--install-dir=.".into(),
    ]
}

// ---------- Spigot (BuildTools) ----------

/// Versiones que BuildTools sabe compilar: el listado `hub.spigotmc.org/versions/` tiene un
/// `<versión>.json` por cada una (además de alias como `latest.json`).
pub fn parse_spigot_versions(html: &str) -> Vec<String> {
    let re = regex::Regex::new(r#"href="([0-9][0-9.]*)\.json""#).unwrap();
    let mut out: Vec<String> = re
        .captures_iter(html)
        .map(|c| c[1].to_string())
        .filter(|v| looks_like_mc(v))
        .collect();
    out.sort_by(|a, b| version_cmp(b, a));
    out.dedup();
    out
}

pub fn spigot_resolved(buildtools_url: &str, mc: &str) -> Resolved {
    Resolved {
        flavor: Flavor::Spigot,
        mc_version: mc.to_string(),
        loader_version: None,
        java_major: recommended_java_major(Some(mc)),
        experimental: false,
        artifact: Artifact {
            mirrors: vec![],
            url: buildtools_url.to_string(),
            file_name: "BuildTools.jar".into(),
            checksum: Checksum::None,
            size: None,
        },
        installer_args: Some(vec![
            "--rev".into(),
            mc.into(),
            "--compile".into(),
            "spigot".into(),
            "--output-dir".into(),
            "{server_dir}".into(),
        ]),
        installer_in_cache: true,
        needs_jdk: true,
        notes: vec![
            "SpigotMC no publica servidores ya hechos: se compilan en tu PC con su herramienta oficial BuildTools. Tarda entre 5 y 20 minutos y necesita Internet.".into(),
            "Necesita un JDK (Java con compilador). En Windows, BuildTools descarga Git portátil si no lo tienes.".into(),
            "BuildTools se descarga por HTTPS desde hub.spigotmc.org (no publica huella).".into(),
            "Paper y Purpur aceptan los mismos plugins que Spigot y suelen ir más fluidos.".into(),
        ],
    }
}

// ---------- Arclight (GitHub Releases) ----------

/// Cargadores de Arclight, en orden de preferencia.
const ARCLIGHT_LOADERS: [&str; 3] = ["neoforge", "forge", "fabric"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArclightAsset {
    pub mc: String,
    pub loader: String,
    pub name: String,
    pub url: String,
    pub sha256: Option<String>,
    pub size: Option<u64>,
    pub prerelease: bool,
    pub tag: String,
}

/// Archivos de servidor de las publicaciones de Arclight: `arclight-<cargador>-<mc>-<versión>.jar`.
pub fn arclight_assets(releases: &Value) -> Vec<ArclightAsset> {
    let re = regex::Regex::new(r"^arclight-(neoforge|forge|fabric)-([0-9][0-9.]*)-[A-Za-z0-9._+-]+\.jar$").unwrap();
    let mut out = vec![];
    for rel in releases.as_array().into_iter().flatten() {
        if rel["draft"] == true {
            continue;
        }
        let prerelease = rel["prerelease"] == true;
        let tag = rel["tag_name"].as_str().unwrap_or("").to_string();
        for a in rel["assets"].as_array().into_iter().flatten() {
            let (Some(name), Some(url)) = (a["name"].as_str(), a["browser_download_url"].as_str()) else {
                continue;
            };
            let Some(c) = re.captures(name) else { continue };
            if !looks_like_mc(&c[2]) || !crate::http::url_allowed(url) {
                continue;
            }
            let sha256 = a["digest"]
                .as_str()
                .and_then(|d| d.strip_prefix("sha256:"))
                .filter(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()))
                .map(|h| h.to_ascii_lowercase());
            out.push(ArclightAsset {
                mc: c[2].to_string(),
                loader: c[1].to_string(),
                name: name.to_string(),
                url: url.to_string(),
                sha256,
                size: a["size"].as_u64(),
                prerelease,
                tag: tag.clone(),
            });
        }
    }
    out
}

pub fn arclight_versions(releases: &Value) -> Vec<String> {
    let mut v: Vec<String> = arclight_assets(releases).into_iter().map(|a| a.mc).collect();
    v.sort_by(|a, b| version_cmp(b, a));
    v.dedup();
    v
}

/// El mejor archivo para una versión: publicación estable antes que experimental (las
/// publicaciones vienen de la más nueva a la más vieja) y NeoForge > Forge > Fabric.
pub fn arclight_pick(releases: &Value, mc: &str) -> Option<ArclightAsset> {
    let assets: Vec<ArclightAsset> = arclight_assets(releases).into_iter().filter(|a| a.mc == mc).collect();
    for want_stable in [true, false] {
        let pool: Vec<&ArclightAsset> = assets.iter().filter(|a| a.prerelease != want_stable).collect();
        let Some(newest_tag) = pool.first().map(|a| a.tag.clone()) else {
            continue;
        };
        for loader in ARCLIGHT_LOADERS {
            if let Some(a) = pool.iter().find(|a| a.tag == newest_tag && a.loader == loader) {
                return Some((*a).clone());
            }
        }
    }
    None
}

pub fn arclight_resolved(a: &ArclightAsset) -> Resolved {
    let loader = match a.loader.as_str() {
        "neoforge" => "NeoForge",
        "forge" => "Forge",
        _ => "Fabric",
    };
    let mut notes = vec![
        format!("Arclight para {loader}: usa mods de {loader} (carpeta mods) y plugins de Bukkit/Spigot/Paper (carpeta plugins)."),
        "Es un servidor híbrido: algunas combinaciones de mods y plugins pueden fallar. Haz copias de seguridad.".into(),
        format!("En el primer arranque, Arclight descarga {loader} y el servidor de Mojang (necesita Internet)."),
    ];
    if a.prerelease {
        notes.push(format!(
            "El autor de Arclight marca la publicación {} como experimental: puede tener errores.",
            a.tag
        ));
    }
    if a.sha256.is_none() {
        notes.push("Esta publicación no tiene huella en GitHub; se descarga por HTTPS.".into());
    }
    Resolved {
        flavor: Flavor::Arclight,
        mc_version: a.mc.clone(),
        loader_version: Some(format!("{loader} · {}", a.tag)),
        java_major: recommended_java_major(Some(&a.mc)),
        experimental: a.prerelease,
        artifact: Artifact {
            url: a.url.clone(),
            mirrors: vec![],
            file_name: a.name.clone(),
            checksum: a.sha256.clone().map(Checksum::Sha256).unwrap_or(Checksum::None),
            size: a.size,
        },
        installer_args: None,
        installer_in_cache: false,
        needs_jdk: false,
        notes,
    }
}

// ---------- API pública ----------

pub fn valid_jar_name(name: &str) -> bool {
    name.len() <= 128
        && name.to_ascii_lowercase().ends_with(".jar")
        && !name.starts_with('.')
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-+".contains(&b))
}

pub fn list_versions(http: &Http, src: &Sources, flavor: Flavor) -> Result<Vec<CatalogEntry>, InstallError> {
    match flavor {
        Flavor::Vanilla => parse_mojang_manifest(&http.get_json(&src.mojang_manifest)?),
        Flavor::Paper => parse_paper_project(&http.get_json(&format!("{}/v3/projects/paper", src.paper_api))?),
        Flavor::Fabric => parse_fabric_games(&http.get_json(&format!("{}/v2/versions/game", src.fabric_meta))?),
        Flavor::Purpur => parse_purpur_versions(&http.get_json(&format!("{}/purpur", src.purpur_api))?),
        Flavor::Forge => {
            let promos = http.get_json(&src.forge_promotions)?;
            Ok(entries(forge_versions(&promos)))
        }
        Flavor::Quilt => parse_fabric_games(&http.get_json(&format!("{}/versions/game", src.quilt_meta))?),
        Flavor::Spigot => Ok(entries(parse_spigot_versions(
            &http.get_text(&format!("{}/", src.spigot_versions))?,
        ))),
        Flavor::Arclight => Ok(entries(arclight_versions(&http.get_json(&src.arclight_releases)?))),
        Flavor::NeoForge => {
            let xml = http.get_text(&format!(
                "{}/net/neoforged/neoforge/maven-metadata.xml",
                src.neoforge_maven
            ))?;
            let best = neoforge_best(&parse_maven_versions(&xml));
            if best.is_empty() {
                return Err(bad("NeoForge"));
            }
            Ok(entries(best.into_iter().map(|(mc, _)| mc).collect()))
        }
    }
}

/// Una versión del loader que se puede elegir al crear un servidor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoaderOption {
    pub version: String,
    /// Estable según la fuente (las beta se marcan para avisar).
    pub stable: bool,
    /// La recomendada por el proyecto (Forge) o la que se elegiría por defecto.
    pub recommended: bool,
}

/// Cuántas versiones se ofrecen como mucho (las más nuevas).
const MAX_LOADER_OPTIONS: usize = 40;

/// Loaders de Fabric (`/v2/versions/loader`: `[{version, stable}]`, del más nuevo al más viejo).
pub fn parse_fabric_loaders(v: &Value) -> Vec<LoaderOption> {
    let mut out: Vec<LoaderOption> = v
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| {
            Some(LoaderOption {
                version: x["version"].as_str()?.to_string(),
                stable: x["stable"] == true,
                recommended: false,
            })
        })
        .take(MAX_LOADER_OPTIONS)
        .collect();
    if let Some(first) = out.iter_mut().find(|o| o.stable) {
        first.recommended = true;
    }
    out
}

/// Loaders de Quilt (`/versions/loader`): estable si no lleva sufijo beta.
pub fn parse_quilt_loaders(v: &Value) -> Vec<LoaderOption> {
    let mut out: Vec<LoaderOption> = v
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| x["version"].as_str())
        .map(|ver| LoaderOption {
            version: ver.to_string(),
            stable: !ver.contains('-'),
            recommended: false,
        })
        .take(MAX_LOADER_OPTIONS)
        .collect();
    if let Some(first) = out.iter_mut().find(|o| o.stable) {
        first.recommended = true;
    }
    out
}

/// Versiones de NeoForge publicadas para una versión de Minecraft, de la más nueva a la más vieja.
pub fn neoforge_for_mc(all: &[String], mc: &str) -> Vec<LoaderOption> {
    let mut v: Vec<&String> = all
        .iter()
        .filter(|x| neoforge_to_mc(x).as_deref() == Some(mc))
        .collect();
    v.sort_by(|a, b| version_cmp(b, a));
    let best = neoforge_best(all).into_iter().find(|(m, _)| m == mc).map(|(_, b)| b);
    v.into_iter()
        .take(MAX_LOADER_OPTIONS)
        .map(|x| LoaderOption {
            version: x.clone(),
            stable: !x.contains('-'),
            recommended: best.as_deref() == Some(x.as_str()),
        })
        .collect()
}

/// Forge solo publica en su API las promociones «recomendada» y «última» de cada versión.
pub fn forge_promoted(promos: &Value, mc: &str) -> Vec<LoaderOption> {
    let p = &promos["promos"];
    let mut out = vec![];
    if let Some(b) = p[format!("{mc}-recommended")].as_str() {
        out.push(LoaderOption {
            version: b.to_string(),
            stable: true,
            recommended: true,
        });
    }
    if let Some(b) = p[format!("{mc}-latest")].as_str() {
        if !out.iter().any(|o| o.version == b) {
            out.push(LoaderOption {
                version: b.to_string(),
                stable: true,
                recommended: out.is_empty(),
            });
        }
    }
    out
}

/// Todas las versiones de Forge publicadas para una versión de Minecraft (del maven oficial,
/// donde se llaman `<mc>-<build>`), de la más nueva a la más vieja. La recomendada por Forge
/// se marca y siempre se incluye.
pub fn forge_for_mc(all: &[String], promos: &Value, mc: &str) -> Vec<LoaderOption> {
    let prefix = format!("{mc}-");
    let mut builds: Vec<String> = all
        .iter()
        .filter_map(|v| v.strip_prefix(&prefix))
        // Las de versiones antiguas con otro sufijo (`-1.7.10`) no son de este Minecraft.
        .filter(|b| !b.is_empty() && !b.contains('-'))
        .map(str::to_string)
        .collect();
    builds.sort_by(|a, b| version_cmp(b, a));
    builds.dedup();
    let promoted = forge_promoted(promos, mc);
    let best = promoted.iter().find(|o| o.recommended).map(|o| o.version.clone());
    let mut out: Vec<LoaderOption> = builds
        .into_iter()
        .take(MAX_LOADER_OPTIONS)
        .map(|b| LoaderOption {
            recommended: best.as_deref() == Some(b.as_str()),
            version: b,
            stable: true,
        })
        .collect();
    if let Some(b) = best {
        if !out.iter().any(|o| o.version == b) {
            out.push(LoaderOption {
                version: b,
                stable: true,
                recommended: true,
            });
        }
    }
    out
}

/// Versiones del loader que se pueden elegir. Vacío para tipos sin loader elegible (Vanilla,
/// Paper, Purpur, Spigot, Arclight): esos usan su última compilación estable.
pub fn loader_versions(
    http: &Http,
    src: &Sources,
    flavor: Flavor,
    mc: &str,
) -> Result<Vec<LoaderOption>, InstallError> {
    let mc = safe_segment(mc.trim())?;
    Ok(match flavor {
        Flavor::Fabric => parse_fabric_loaders(&http.get_json(&format!("{}/v2/versions/loader", src.fabric_meta))?),
        Flavor::Quilt => parse_quilt_loaders(&http.get_json(&format!("{}/versions/loader", src.quilt_meta))?),
        Flavor::NeoForge => {
            let xml = http.get_text(&format!(
                "{}/net/neoforged/neoforge/maven-metadata.xml",
                src.neoforge_maven
            ))?;
            neoforge_for_mc(&parse_maven_versions(&xml), mc)
        }
        Flavor::Forge => {
            let promos = http.get_json(&src.forge_promotions)?;
            // Todas las versiones salen del maven oficial; si no responde, al menos las
            // promocionadas (recomendada y última).
            match http.get_text(&format!(
                "{}/net/minecraftforge/forge/maven-metadata.xml",
                src.forge_maven
            )) {
                Ok(xml) => {
                    let all = forge_for_mc(&parse_maven_versions(&xml), &promos, mc);
                    if all.is_empty() {
                        forge_promoted(&promos, mc)
                    } else {
                        all
                    }
                }
                Err(_) => forge_promoted(&promos, mc),
            }
        }
        _ => vec![],
    })
}

pub fn resolve(http: &Http, src: &Sources, flavor: Flavor, mc: &str) -> Result<Resolved, InstallError> {
    resolve_pinned(http, src, flavor, mc, None)
}

/// Como [`resolve`], pero con una versión exacta del loader (la que pide un modpack). Solo
/// aplica a Fabric, Quilt, Forge y NeoForge; para el resto se ignora.
pub fn resolve_pinned(
    http: &Http,
    src: &Sources,
    flavor: Flavor,
    mc: &str,
    loader: Option<&str>,
) -> Result<Resolved, InstallError> {
    let mc = safe_segment(mc.trim())?;
    let loader = match loader.map(str::trim).filter(|l| !l.is_empty()) {
        Some(l) => Some(safe_segment(l)?.to_string()),
        None => None,
    };
    let resolved = match flavor {
        Flavor::Vanilla => {
            let manifest = http.get_json(&src.mojang_manifest)?;
            let url = mojang_version_url(&manifest, mc)?;
            parse_mojang_version(&http.get_json(&url)?, mc)?
        }
        Flavor::Paper => parse_paper_builds(
            &http.get_json(&format!("{}/v3/projects/paper/versions/{mc}/builds", src.paper_api))?,
            mc,
        )?,
        Flavor::Fabric => {
            let base = &src.fabric_meta;
            let loader = match &loader {
                Some(l) => l.clone(),
                None => first_stable(
                    &http.get_json(&format!("{base}/v2/versions/loader"))?,
                    "Fabric (loader)",
                )?,
            };
            let installer = first_stable(
                &http.get_json(&format!("{base}/v2/versions/installer"))?,
                "Fabric (instalador)",
            )?;
            fabric_resolved(base, mc, safe_segment(&loader)?, safe_segment(&installer)?)
        }
        Flavor::NeoForge => {
            let base = &src.neoforge_maven;
            let xml = http.get_text(&format!("{base}/net/neoforged/neoforge/maven-metadata.xml"))?;
            let all = parse_maven_versions(&xml);
            let v = match &loader {
                Some(l) if all.iter().any(|x| x == l) => l.clone(),
                Some(l) => {
                    return Err(InstallError::NotAvailable(format!(
                        "NeoForge {l} no está publicado en el repositorio oficial"
                    )))
                }
                None => {
                    neoforge_best(&all)
                        .into_iter()
                        .find(|(m, _)| m == mc)
                        .ok_or_else(|| {
                            InstallError::NotAvailable(format!("NeoForge no tiene versión para Minecraft {mc}"))
                        })?
                        .1
                }
            };
            let v = safe_segment(&v)?.to_string();
            let url = neoforge_installer_url(base, &v);
            let mut notes = vec![
                "El instalador oficial de NeoForge descargará librerías y el servidor de Mojang (necesita Internet)."
                    .to_string(),
            ];
            let checksum = maven_checksum(http, &url, &mut notes);
            let experimental = v.contains('-');
            if experimental {
                notes.push(format!("NeoForge {v} está marcado como beta. Haz copias de seguridad."));
            }
            Resolved {
                flavor,
                mc_version: mc.to_string(),
                loader_version: Some(v.clone()),
                java_major: recommended_java_major(Some(mc)),
                experimental,
                artifact: Artifact {
                    mirrors: vec![],
                    url,
                    file_name: format!("neoforge-{v}-installer.jar"),
                    checksum,
                    size: None,
                },
                installer_args: Some(vec!["--installServer".into()]),
                installer_in_cache: false,
                needs_jdk: false,
                notes,
            }
        }
        Flavor::Purpur => {
            let base = &src.purpur_api;
            let info = http.get_json(&format!("{base}/purpur/{mc}"))?;
            let build = info["builds"]["latest"]
                .as_str()
                .map(str::to_string)
                .or_else(|| info["builds"]["latest"].as_u64().map(|n| n.to_string()))
                .ok_or_else(|| InstallError::NotAvailable(format!("Purpur aún no tiene compilaciones para {mc}")))?;
            let build = safe_segment(&build)?.to_string();
            let detail = http.get_json(&format!("{base}/purpur/{mc}/{build}"))?;
            purpur_resolved(base, mc, &build, &detail)
        }
        Flavor::Forge => {
            let (build, recommended) = match &loader {
                Some(l) => (l.clone(), true),
                None => {
                    let promos = http.get_json(&src.forge_promotions)?;
                    forge_build_for(&promos, mc).ok_or_else(|| {
                        InstallError::NotAvailable(format!("Forge no tiene versión para Minecraft {mc}"))
                    })?
                }
            };
            let build = safe_segment(&build)?.to_string();
            let full = format!("{mc}-{build}");
            let url = format!(
                "{}/net/minecraftforge/forge/{full}/forge-{full}-installer.jar",
                src.forge_maven
            );
            let mut notes = vec![
                "El instalador oficial de Forge descargará librerías y el servidor de Mojang (necesita Internet)."
                    .to_string(),
            ];
            if !recommended {
                notes.push(format!(
                    "Forge todavía no marca una versión recomendada para {mc}; se usa la más reciente ({build})."
                ));
            }
            let checksum = maven_checksum(http, &url, &mut notes);
            Resolved {
                flavor,
                mc_version: mc.to_string(),
                loader_version: Some(build),
                java_major: recommended_java_major(Some(mc)),
                experimental: false,
                artifact: Artifact {
                    mirrors: vec![],
                    url,
                    file_name: format!("forge-{full}-installer.jar"),
                    checksum,
                    size: None,
                },
                installer_args: Some(vec!["--installServer".into()]),
                installer_in_cache: false,
                needs_jdk: false,
                notes,
            }
        }
        Flavor::Quilt => {
            let loader = match &loader {
                Some(l) => l.clone(),
                None => {
                    let loaders = http.get_json(&format!("{}/versions/loader", src.quilt_meta))?;
                    quilt_stable_loader(&loaders).ok_or_else(|| bad("Quilt (loader)"))?
                }
            };
            let loader = safe_segment(&loader)?.to_string();
            let meta = http.get_text(&format!(
                "{}/org/quiltmc/quilt-installer/maven-metadata.xml",
                src.quilt_maven
            ))?;
            let installer = maven_release(&meta).ok_or_else(|| bad("Quilt (instalador)"))?;
            let installer = safe_segment(&installer)?.to_string();
            let url = format!(
                "{}/org/quiltmc/quilt-installer/{installer}/quilt-installer-{installer}.jar",
                src.quilt_maven
            );
            let mut notes = vec![
                "El instalador oficial de Quilt descargará librerías y el servidor de Mojang (necesita Internet)."
                    .to_string(),
            ];
            let checksum = maven_checksum(http, &url, &mut notes);
            Resolved {
                flavor,
                mc_version: mc.to_string(),
                loader_version: Some(loader.clone()),
                java_major: recommended_java_major(Some(mc)),
                experimental: false,
                artifact: Artifact {
                    mirrors: vec![],
                    url,
                    file_name: format!("quilt-installer-{installer}.jar"),
                    checksum,
                    size: None,
                },
                installer_args: Some(quilt_args(mc, &loader)),
                installer_in_cache: false,
                needs_jdk: false,
                notes,
            }
        }
        Flavor::Spigot => {
            let html = http.get_text(&format!("{}/", src.spigot_versions))?;
            if !parse_spigot_versions(&html).iter().any(|v| v == mc) {
                return Err(InstallError::NotAvailable(format!(
                    "Spigot no tiene versión para Minecraft {mc}"
                )));
            }
            spigot_resolved(&src.buildtools_url, mc)
        }
        Flavor::Arclight => {
            let releases = http.get_json(&src.arclight_releases)?;
            let asset = arclight_pick(&releases, mc)
                .ok_or_else(|| InstallError::NotAvailable(format!("Arclight no tiene versión para Minecraft {mc}")))?;
            arclight_resolved(&asset)
        }
    };
    if !valid_jar_name(&resolved.artifact.file_name) {
        return Err(InstallError::BadResponse("nombre de archivo no válido".into()));
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_options_mark_stable_and_recommended() {
        let fab = serde_json::json!([{"version":"0.17.0-beta.1","stable":false},{"version":"0.16.10","stable":true},{"version":"0.16.9","stable":true}]);
        let f = parse_fabric_loaders(&fab);
        assert_eq!(
            f.iter()
                .map(|o| (o.version.as_str(), o.stable, o.recommended))
                .collect::<Vec<_>>(),
            vec![
                ("0.17.0-beta.1", false, false),
                ("0.16.10", true, true),
                ("0.16.9", true, false)
            ]
        );
        let q = parse_quilt_loaders(&serde_json::json!([{"version":"0.29.0-beta.3"},{"version":"0.28.1"}]));
        assert!(q[1].recommended && !q[0].stable);
        let all: Vec<String> = ["21.1.10", "21.1.77", "21.1.200-beta", "21.0.5"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let n = neoforge_for_mc(&all, "1.21.1");
        assert_eq!(
            n.iter().map(|o| o.version.as_str()).collect::<Vec<_>>(),
            vec!["21.1.200-beta", "21.1.77", "21.1.10"]
        );
        assert!(n[1].recommended && !n[0].stable);
        let promos = serde_json::json!({"promos": {"1.20.1-recommended": "47.3.0", "1.20.1-latest": "47.4.10"}});
        let g = forge_promoted(&promos, "1.20.1");
        assert_eq!(
            (g[0].version.as_str(), g[0].recommended, g[1].version.as_str()),
            ("47.3.0", true, "47.4.10")
        );
        assert!(forge_promoted(&promos, "1.12.2").is_empty());
    }
    use serde_json::json;

    #[test]
    fn mojang_releases_only_latest_first() {
        let m = json!({
            "latest": {"release": "26.1.2", "snapshot": "26w44a"},
            "versions": [
                {"id": "26w44a", "type": "snapshot", "url": "u0"},
                {"id": "26.1.2", "type": "release", "url": "u1"},
                {"id": "1.21.11", "type": "release", "url": "u2"},
                {"id": "b1.7.3", "type": "old_beta", "url": "u3"}
            ]
        });
        let e = parse_mojang_manifest(&m).unwrap();
        assert_eq!(
            e.iter().map(|x| x.mc_version.as_str()).collect::<Vec<_>>(),
            ["26.1.2", "1.21.11"]
        );
        assert!(e[0].latest && !e[1].latest);
        assert_eq!((e[0].java_major, e[1].java_major), (25, 21));
        assert_eq!(mojang_version_url(&m, "1.21.11").unwrap(), "u2");
        assert!(matches!(
            mojang_version_url(&m, "9.9"),
            Err(InstallError::NotAvailable(_))
        ));
    }

    #[test]
    fn mojang_version_uses_published_java_and_sha1() {
        let v = json!({
            "javaVersion": {"component": "java-runtime-epsilon", "majorVersion": 25},
            "downloads": {"server": {"sha1": "abc", "size": 10, "url": "https://piston-data.mojang.com/x/server.jar"}}
        });
        let r = parse_mojang_version(&v, "26.1").unwrap();
        assert_eq!(r.java_major, 25);
        assert_eq!(r.artifact.checksum, Checksum::Sha1("abc".into()));
        assert_eq!(r.artifact.file_name, "minecraft_server.26.1.jar");
        let none = json!({"downloads": {}});
        assert!(matches!(
            parse_mojang_version(&none, "1.0"),
            Err(InstallError::NotAvailable(_))
        ));
    }

    #[test]
    fn paper_flatten_and_prefer_stable() {
        let p = json!({"project": {"id": "paper"}, "versions": {
            "1.21": ["1.21.11", "1.21.10", "1.21.11-pre3"],
            "26.1": ["26.1.2", "26.1"]
        }});
        let e = parse_paper_project(&p).unwrap();
        assert_eq!(
            e.iter().map(|x| x.mc_version.as_str()).collect::<Vec<_>>(),
            ["26.1.2", "26.1", "1.21.11", "1.21.10"]
        );
        let builds = json!([
            {"id": 9, "channel": "ALPHA", "downloads": {"server:default": {"name": "paper-26.1.2-9.jar", "url": "https://x/9", "checksums": {"sha256": "aa"}, "size": 5}}},
            {"id": 7, "channel": "STABLE", "downloads": {"server:default": {"name": "paper-26.1.2-7.jar", "url": "https://x/7", "checksums": {"sha256": "bb"}, "size": 6}}}
        ]);
        let r = parse_paper_builds(&builds, "26.1.2").unwrap();
        assert_eq!((r.loader_version.as_deref(), r.experimental), (Some("7"), false));
        assert_eq!(r.artifact.file_name, "paper-26.1.2-7.jar");
        let only_alpha = json!([builds[0].clone()]);
        let r = parse_paper_builds(&only_alpha, "26.1.2").unwrap();
        assert!(r.experimental && !r.notes.is_empty());
        // Un nombre extraño de la API no se usa como ruta.
        let weird = json!([{"id": 3, "channel": "STABLE", "downloads": {"server:default": {"name": "../../evil.jar", "url": "https://x", "checksums": {"sha256": "cc"}}}}]);
        assert_eq!(
            parse_paper_builds(&weird, "1.21.11").unwrap().artifact.file_name,
            "paper-1.21.11-3.jar"
        );
    }

    #[test]
    fn fabric_stable_games() {
        let g = json!([
            {"version": "26w44a", "stable": false},
            {"version": "26.1.2", "stable": true},
            {"version": "1.21.11", "stable": true}
        ]);
        let e = parse_fabric_games(&g).unwrap();
        assert_eq!(e.len(), 2);
        let r = fabric_resolved("https://meta.fabricmc.net", "26.1.2", "0.18.2", "1.1.0");
        assert_eq!(
            r.artifact.url,
            "https://meta.fabricmc.net/v2/versions/loader/26.1.2/0.18.2/1.1.0/server/jar"
        );
        assert!(valid_jar_name(&r.artifact.file_name));
    }

    #[test]
    fn neoforge_grouping() {
        let xml = "<metadata><versioning><versions>\
            <version>21.1.200</version><version>21.1.259</version>\
            <version>21.10.64</version><version>26.1.0.5-beta</version>\
            <version>26.1.2.70-beta</version><version>26.1.2.76</version>\
            </versions></versioning></metadata>";
        let v = parse_maven_versions(xml);
        assert_eq!(v.len(), 6);
        let best = neoforge_best(&v);
        assert_eq!(
            best,
            vec![
                ("26.1.2".to_string(), "26.1.2.76".to_string()),
                ("26.1".to_string(), "26.1.0.5-beta".to_string()),
                ("1.21.10".to_string(), "21.10.64".to_string()),
                ("1.21.1".to_string(), "21.1.259".to_string()),
            ]
        );
    }

    #[test]
    fn hash_files_and_segments() {
        assert_eq!(
            parse_hash_file("DA39A3EE5E6B4B0D3255BFEF95601890AFD80709  file.jar\n").as_deref(),
            Some("da39a3ee5e6b4b0d3255bfef95601890afd80709")
        );
        assert_eq!(parse_hash_file("<html>"), None);
        assert!(safe_segment("26.1.2").is_ok());
        assert!(safe_segment("../x").is_err() && safe_segment("a/b").is_err() && safe_segment("a?b").is_err());
    }

    #[test]
    fn forge_lists_every_build_from_maven() {
        let xml = "<metadata><versioning><versions>\
            <version>1.7.10-10.13.4.1614-1.7.10</version>\
            <version>1.20.1-46.0.14</version>\
            <version>1.20.1-47.1.0</version>\
            <version>1.20.1-47.4.28</version>\
            <version>1.20.1-47.4.30</version>\
            <version>1.20.2-48.0.1</version>\
            </versions></versioning></metadata>";
        let promos = serde_json::json!({"promos": {"1.20.1-recommended": "47.4.28", "1.20.1-latest": "47.4.30"}});
        let v = forge_for_mc(&parse_maven_versions(xml), &promos, "1.20.1");
        let names: Vec<&str> = v.iter().map(|o| o.version.as_str()).collect();
        assert_eq!(names, ["47.4.30", "47.4.28", "47.1.0", "46.0.14"]);
        assert!(v[1].recommended && !v[0].recommended);
        assert!(forge_for_mc(&parse_maven_versions(xml), &promos, "1.7.10").is_empty());
        // Sin recomendada, la «última» de las promociones queda marcada.
        let promos2 = serde_json::json!({"promos": {"1.20.2-latest": "48.0.1"}});
        let v2 = forge_for_mc(&parse_maven_versions(xml), &promos2, "1.20.2");
        assert!(v2[0].recommended);
    }
}
