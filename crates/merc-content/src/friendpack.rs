//! Pack para amigos: un `.mrpack` de cliente con los mods que los jugadores necesitan.
//!
//! - Solo lleva `modrinth.index.json`: cada mod es un enlace oficial de Modrinth
//!   (`cdn.modrinth.com`) con sus huellas SHA-1 y SHA-512 y su tamaño. No se copia ningún
//!   archivo dentro del pack, así que no se redistribuye nada.
//! - Los mods que no están en Modrinth (CurseForge, archivos sueltos) no pueden ir en un
//!   `.mrpack` (el formato solo admite descargas de Modrinth, GitHub y GitLab). Se listan
//!   aparte con su página, para descargarlos a mano.
//! - Los mods marcados por su autor como «solo servidor» no se incluyen.
//!
//! El formato sigue la documentación oficial de Modrinth (`formatVersion` 1).

use crate::local::InstalledItem;
use crate::modrinth::{ContentVersion, Side};
use crate::{ContentError, ContentSource};
use merc_core::detect::ServerKind;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackMod {
    pub title: String,
    pub file_name: String,
    /// `true` si su autor lo marca como opcional en el juego.
    pub optional: bool,
    pub page_url: Option<String>,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualMod {
    pub title: String,
    pub file_name: String,
    pub page_url: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FriendPackPlan {
    pub mc_version: Option<String>,
    /// Clave del formato (`fabric-loader`, `quilt-loader`, `forge`, `neoforge`) y versión.
    pub loader: Option<(String, String)>,
    /// Van en el pack, con enlace oficial y huellas.
    pub included: Vec<PackMod>,
    /// Hacen falta (o pueden hacer falta) pero hay que bajarlos a mano.
    pub manual: Vec<ManualMod>,
    /// Solo del servidor: los jugadores no los necesitan.
    pub server_only: Vec<String>,
    /// Si no se puede hacer el pack, por qué.
    pub blocked: Option<String>,
}

/// Un archivo del índice (lo que no se muestra en la interfaz).
#[derive(Debug, Clone)]
pub struct PackEntry {
    pub path: String,
    pub sha1: String,
    pub sha512: String,
    pub url: String,
    pub size: u64,
    pub client: Side,
    pub server: Side,
}

/// Clave y versión del loader para `dependencies`, tal como las usa Modrinth.
pub fn loader_dependency(
    kind: ServerKind,
    mc: &str,
    loader_version: Option<&str>,
) -> Result<Option<(String, String)>, String> {
    let key = match kind {
        ServerKind::Fabric => "fabric-loader",
        ServerKind::Quilt => "quilt-loader",
        ServerKind::Forge => "forge",
        ServerKind::NeoForge => "neoforge",
        ServerKind::Vanilla | ServerKind::Paper | ServerKind::Purpur | ServerKind::Spigot => return Ok(None),
        ServerKind::Mohist | ServerKind::Arclight => {
            return Err("Este servidor es un híbrido (plugins y mods): no hay un loader de cliente que lo represente en un .mrpack.".into())
        }
        ServerKind::Unknown => return Err("No se sabe qué tipo de servidor es.".into()),
    };
    let v = loader_version
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "No se sabe la versión exacta del loader de este servidor.".to_string())?;
    // Forge se instala en libraries/…/forge/<mc>-<versión>; el formato quiere solo la versión.
    let v = v.strip_prefix(&format!("{mc}-")).unwrap_or(v);
    if v.chars().any(|c| c.is_whitespace()) {
        return Err(format!("La versión del loader («{v}») no tiene el formato esperado."));
    }
    Ok(Some((key.to_string(), v.to_string())))
}

fn side_str(s: Side) -> &'static str {
    match s {
        Side::Required => "required",
        Side::Optional | Side::Unknown => "optional",
        Side::Unsupported => "unsupported",
    }
}

fn name_of(i: &InstalledItem) -> String {
    i.title.clone().unwrap_or_else(|| i.file_name.clone())
}

/// Decide qué va en el pack. `versions` son las versiones de Modrinth por SHA-1 del archivo.
pub fn plan(
    kind: ServerKind,
    mc_version: Option<&str>,
    loader_version: Option<&str>,
    mods: &[InstalledItem],
    versions: &HashMap<String, ContentVersion>,
) -> (FriendPackPlan, Vec<PackEntry>) {
    let mut plan = FriendPackPlan {
        mc_version: mc_version.map(String::from),
        loader: None,
        included: vec![],
        manual: vec![],
        server_only: vec![],
        blocked: None,
    };
    match mc_version {
        None => plan.blocked = Some("No se sabe la versión de Minecraft de este servidor.".into()),
        Some(mc) => match loader_dependency(kind, mc, loader_version) {
            Ok(l) => plan.loader = l,
            Err(e) => plan.blocked = Some(e),
        },
    }
    let mut entries = vec![];
    for m in mods.iter().filter(|m| m.enabled) {
        let client = m.client_side.unwrap_or(Side::Unknown);
        if client == Side::Unsupported {
            plan.server_only.push(name_of(m));
            continue;
        }
        let file = versions.get(&m.sha1).and_then(|v| {
            v.files
                .iter()
                .find(|f| f.sha1.as_deref().is_some_and(|h| h.eq_ignore_ascii_case(&m.sha1)))
        });
        let usable = file.filter(|f| f.url.starts_with("https://cdn.modrinth.com/") && f.sha512.is_some());
        match usable {
            Some(f) => {
                plan.included.push(PackMod {
                    title: name_of(m),
                    file_name: f.filename.clone(),
                    optional: client != Side::Required,
                    page_url: m.page_url.clone(),
                    size: f.size,
                });
                entries.push(PackEntry {
                    path: format!("mods/{}", f.filename),
                    sha1: m.sha1.to_lowercase(),
                    sha512: f.sha512.clone().unwrap_or_default(),
                    url: f.url.clone(),
                    size: f.size,
                    client,
                    server: m.server_side.unwrap_or(Side::Unknown),
                });
            }
            None => plan.manual.push(ManualMod {
                title: name_of(m),
                file_name: m.file_name.clone(),
                page_url: m.page_url.clone(),
                reason: match m.source {
                    Some(ContentSource::Curseforge) => {
                        "Es de CurseForge: el formato .mrpack no admite sus descargas. Bájalo de su página.".into()
                    }
                    Some(_) if file.is_some() => "Modrinth no da un enlace verificable para este archivo.".into(),
                    _ => {
                        "No está en Modrinth (o no hubo conexión): no se sabe de dónde bajarlo de forma oficial.".into()
                    }
                },
            }),
        }
    }
    plan.included.sort_by_key(|m| m.title.to_lowercase());
    plan.manual.sort_by_key(|m| m.title.to_lowercase());
    plan.server_only.sort_by_key(|s| s.to_lowercase());
    if plan.blocked.is_none() && plan.included.is_empty() {
        plan.blocked = Some(
            "Ningún mod de este servidor se puede incluir en un pack (ninguno es de Modrinth para el juego).".into(),
        );
    }
    (plan, entries)
}

/// El `modrinth.index.json` del pack.
pub fn index_json(
    name: &str,
    version_id: &str,
    plan: &FriendPackPlan,
    entries: &[PackEntry],
) -> Result<serde_json::Value, ContentError> {
    if let Some(b) = &plan.blocked {
        return Err(ContentError::Invalid(b.clone()));
    }
    let mc = plan.mc_version.clone().unwrap_or_default();
    let mut deps = serde_json::Map::new();
    deps.insert("minecraft".into(), json!(mc));
    if let Some((k, v)) = &plan.loader {
        deps.insert(k.clone(), json!(v));
    }
    let files: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            json!({
                "path": e.path,
                "hashes": { "sha1": e.sha1, "sha512": e.sha512 },
                "env": { "client": side_str(e.client), "server": side_str(e.server) },
                "downloads": [e.url],
                "fileSize": e.size,
            })
        })
        .collect();
    Ok(json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": version_id,
        "name": name,
        "summary": "Mods para jugar en este servidor. Cada mod se descarga de su página oficial de Modrinth. Hecho con MercHosting.",
        "files": files,
        "dependencies": deps,
    }))
}

/// Escribe el `.mrpack` (un ZIP con solo el índice).
pub fn write(dest: &Path, index: &serde_json::Value) -> Result<(), ContentError> {
    let io = |e: std::io::Error| ContentError::Io(format!("{}: {e}", dest.display()));
    let tmp = dest.with_extension("mrpack.part");
    {
        let f = std::fs::File::create(&tmp).map_err(io)?;
        let mut z = zip::ZipWriter::new(f);
        z.start_file("modrinth.index.json", zip::write::SimpleFileOptions::default())
            .map_err(|e| ContentError::Io(e.to_string()))?;
        z.write_all(serde_json::to_string_pretty(index).unwrap_or_default().as_bytes())
            .map_err(io)?;
        z.finish().map_err(|e| ContentError::Io(e.to_string()))?;
    }
    std::fs::rename(&tmp, dest).map_err(io)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modrinth::VersionFile;

    fn item(name: &str, sha1: &str, client: Option<Side>, source: Option<ContentSource>) -> InstalledItem {
        InstalledItem {
            file_name: format!("{name}.jar"),
            enabled: true,
            size: 10,
            sha1: sha1.into(),
            project_id: None,
            version_id: None,
            title: Some(name.into()),
            version_number: None,
            icon_url: None,
            server_side: Some(Side::Required),
            client_side: client,
            page_url: Some(format!("https://modrinth.com/mod/{name}")),
            update: None,
            source,
            other_ids: vec![],
        }
    }

    fn version(sha1: &str, url: &str) -> ContentVersion {
        ContentVersion {
            id: "v1".into(),
            project_id: "p".into(),
            name: "x".into(),
            version_number: "1".into(),
            version_type: "release".into(),
            game_versions: vec![],
            loaders: vec![],
            date_published: String::new(),
            downloads: 0,
            changelog: None,
            dependencies: vec![],
            files: vec![VersionFile {
                url: url.into(),
                filename: "sodium-0.6.jar".into(),
                primary: true,
                size: 1234,
                sha1: Some(sha1.into()),
                sha512: Some("ab".repeat(64)),
                sha256: None,
            }],
            server_side: Side::Unknown,
        }
    }

    #[test]
    fn loader_versions() {
        let f = |k, v: Option<&str>| loader_dependency(k, "1.21.1", v);
        assert_eq!(
            f(ServerKind::NeoForge, Some("21.1.259")).unwrap(),
            Some(("neoforge".into(), "21.1.259".into()))
        );
        assert_eq!(
            f(ServerKind::Forge, Some("1.21.1-52.0.40")).unwrap(),
            Some(("forge".into(), "52.0.40".into()))
        );
        assert_eq!(
            f(ServerKind::Fabric, Some("0.16.5")).unwrap(),
            Some(("fabric-loader".into(), "0.16.5".into()))
        );
        assert_eq!(f(ServerKind::Paper, None).unwrap(), None);
        assert!(f(ServerKind::Fabric, None).is_err());
        assert!(f(ServerKind::Arclight, Some("neoforge 1.0.1")).is_err());
    }

    #[test]
    fn builds_a_pack_with_official_links_only() {
        let mods = vec![
            item(
                "Sodium",
                &"a".repeat(40),
                Some(Side::Required),
                Some(ContentSource::Modrinth),
            ),
            item(
                "Lithium",
                &"b".repeat(40),
                Some(Side::Unsupported),
                Some(ContentSource::Modrinth),
            ),
            item(
                "JEI",
                &"c".repeat(40),
                Some(Side::Required),
                Some(ContentSource::Curseforge),
            ),
            item("Suelto", &"d".repeat(40), None, None),
            item(
                "Raro",
                &"e".repeat(40),
                Some(Side::Optional),
                Some(ContentSource::Modrinth),
            ),
        ];
        let mut versions = HashMap::new();
        versions.insert(
            "a".repeat(40),
            version(&"A".repeat(40), "https://cdn.modrinth.com/data/x/sodium.jar"),
        );
        // Un enlace fuera de cdn.modrinth.com no se acepta.
        versions.insert("e".repeat(40), version(&"e".repeat(40), "https://example.com/raro.jar"));
        let (plan, entries) = plan(ServerKind::Fabric, Some("1.21.1"), Some("0.16.5"), &mods, &versions);
        assert!(plan.blocked.is_none());
        assert_eq!(plan.included.len(), 1);
        assert_eq!(plan.included[0].title, "Sodium");
        assert!(!plan.included[0].optional);
        assert_eq!(plan.server_only, vec!["Lithium"]);
        let manual: Vec<_> = plan.manual.iter().map(|m| m.title.as_str()).collect();
        assert_eq!(manual, vec!["JEI", "Raro", "Suelto"]);
        assert!(plan.manual[0].reason.contains("CurseForge"));

        let idx = index_json("Survival (cliente)", "2026-10-30", &plan, &entries).unwrap();
        assert_eq!(idx["dependencies"]["fabric-loader"], "0.16.5");
        assert_eq!(idx["dependencies"]["minecraft"], "1.21.1");
        assert_eq!(idx["files"][0]["path"], "mods/sodium-0.6.jar");
        assert_eq!(idx["files"][0]["hashes"]["sha1"], "a".repeat(40));
        assert_eq!(idx["files"][0]["env"]["client"], "required");

        // El pack escrito lo lee el mismo lector de .mrpack de MercHosting.
        let d = tempfile::tempdir().unwrap();
        let dest = d.path().join("Survival.mrpack");
        write(&dest, &idx).unwrap();
        let back = crate::mrpack::read_index(&dest).unwrap();
        assert_eq!(back.mc_version, "1.21.1");
        assert_eq!(back.loader.as_deref(), Some("fabric"));
        assert_eq!(back.files.len(), 1);
        assert_eq!(back.files[0].urls, vec!["https://cdn.modrinth.com/data/x/sodium.jar"]);
        assert!(!d.path().join("Survival.mrpack.part").exists());
    }

    #[test]
    fn blocked_without_loader_or_mods() {
        let (p, _) = plan(ServerKind::Fabric, Some("1.21.1"), None, &[], &HashMap::new());
        assert!(p.blocked.unwrap().contains("versión exacta del loader"));
        let (p, e) = plan(ServerKind::Fabric, Some("1.21.1"), Some("0.16.5"), &[], &HashMap::new());
        assert!(p.blocked.is_some());
        assert!(index_json("x", "1", &p, &e).is_err());
    }
}
