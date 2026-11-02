//! Contenido de un servidor: mods, plugins y datapacks.
//!
//! - [`modrinth`]: catálogo de Modrinth por su API pública (v2), sin cuenta ni clave.
//! - [`hangar`]: catálogo de plugins de PaperMC (Hangar), sin cuenta ni clave.
//! - [`resourcepack`]: paquete de recursos del servidor (`resource-pack` y su SHA-1).
//! - [`plan`]: plan de instalación con dependencias obligatorias, opcionales e incompatibles.
//! - [`local`]: lo instalado en `mods/` o `plugins/`, desactivar (reversible) e instalar un plan.
//!
//! Reglas: nunca se sobrescribe un archivo sin mandarlo antes a la papelera de MercHosting; cada
//! descarga se verifica con el hash que publica Modrinth; un mod que Modrinth marca como solo
//! para el cliente no se instala en un servidor.

pub mod cfpack;
pub mod curseforge;
pub mod friendpack;
pub mod hangar;
pub mod local;
pub mod modrinth;
pub mod mrpack;
pub mod packupdate;
pub mod plan;
pub mod resourcepack;

use merc_core::detect::ServerKind;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    Mod,
    Plugin,
    /// Datapacks del mundo activo (`<mundo>/datapacks`, archivos `.zip`).
    Datapack,
}

impl ContentKind {
    /// Carpeta de mods y plugins. La de datapacks depende del mundo: ver [`platforms`].
    pub fn folder(self) -> &'static str {
        match self {
            ContentKind::Mod => "mods",
            ContentKind::Plugin => "plugins",
            ContentKind::Datapack => "world/datapacks",
        }
    }

    /// Extensión de los archivos que carga el juego.
    pub fn ext(self) -> &'static str {
        match self {
            ContentKind::Datapack => ".zip",
            _ => ".jar",
        }
    }
}

/// De dónde viene el contenido.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContentSource {
    #[default]
    Modrinth,
    Curseforge,
    /// Hangar, el catálogo oficial de plugins de PaperMC (solo Paper y Purpur).
    Hangar,
}

fn default_sources() -> Vec<ContentSource> {
    vec![ContentSource::Modrinth]
}

/// Lo que admite un servidor: tipo de contenido, carpeta y «loaders» de Modrinth compatibles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Platform {
    pub kind: ContentKind,
    pub folder: String,
    /// Valores de Modrinth (`fabric`, `neoforge`, `paper`…), del más específico al más general.
    pub loaders: Vec<String>,
    /// Híbridos (Arclight, Mohist): la compatibilidad no está garantizada.
    pub experimental: bool,
    /// Catálogos disponibles en esta instalación (CurseForge necesita una clave de API).
    #[serde(default = "default_sources")]
    pub sources: Vec<ContentSource>,
}

fn platform(kind: ContentKind, loaders: &[&str], experimental: bool) -> Platform {
    Platform {
        kind,
        folder: kind.folder().to_string(),
        loaders: loaders.iter().map(|s| s.to_string()).collect(),
        experimental,
        sources: default_sources(),
    }
}

/// Carpeta del mundo activo (`level-name` de `server.properties`), si es un nombre seguro.
pub fn level_folder(root: &Path) -> String {
    let text = std::fs::read_to_string(root.join("server.properties")).unwrap_or_default();
    let name = merc_core::properties::parse_properties(&text)
        .get("level-name")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "world".to_string());
    let safe = !name.starts_with('.')
        && name.len() <= 200
        && !name.contains(['\\', ':', '*', '?', '"', '<', '>', '|'])
        && !name.chars().any(char::is_control)
        && name.split('/').all(|p| !p.is_empty() && p != "." && p != "..");
    if safe {
        name
    } else {
        "world".to_string()
    }
}

/// Loader de mods de un híbrido según el nombre de su JAR (`arclight-neoforge-…`).
fn hybrid_mod_loaders(root: &Path) -> Vec<&'static str> {
    let names: Vec<String> = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .map(|n| n.to_lowercase())
        .filter(|n| n.ends_with(".jar"))
        .collect();
    let has = |w: &str| names.iter().any(|n| n.contains(w));
    if has("neoforge") {
        vec!["neoforge"]
    } else if has("fabric") {
        vec!["fabric"]
    } else if has("forge") {
        vec!["forge"]
    } else {
        vec![]
    }
}

/// Plataformas de contenido de un servidor. Vanilla no admite mods ni plugins, pero todos
/// (Vanilla incluido) admiten datapacks en el mundo activo.
pub fn platforms(kind: ServerKind, root: &Path) -> Vec<Platform> {
    let mut out = code_platforms(kind, root);
    // Bedrock no tiene datapacks de Java: usa addons (behavior/resource packs), con su propio panel.
    if kind != ServerKind::Unknown && kind != ServerKind::Bedrock {
        let mut d = platform(ContentKind::Datapack, &["datapack"], false);
        d.folder = format!("{}/datapacks", level_folder(root));
        out.push(d);
    }
    out
}

fn code_platforms(kind: ServerKind, root: &Path) -> Vec<Platform> {
    use ContentKind::*;
    let with_hangar = |mut p: Platform| {
        p.sources.push(ContentSource::Hangar);
        p
    };
    match kind {
        ServerKind::Paper => vec![with_hangar(platform(Plugin, &["paper", "spigot", "bukkit"], false))],
        ServerKind::Purpur => vec![with_hangar(platform(
            Plugin,
            &["purpur", "paper", "spigot", "bukkit"],
            false,
        ))],
        ServerKind::Spigot => vec![platform(Plugin, &["spigot", "bukkit"], false)],
        ServerKind::Fabric => vec![platform(Mod, &["fabric"], false)],
        // Quilt carga también la mayoría de los mods de Fabric.
        ServerKind::Quilt => vec![platform(Mod, &["quilt", "fabric"], false)],
        ServerKind::Forge => vec![platform(Mod, &["forge"], false)],
        ServerKind::NeoForge => vec![platform(Mod, &["neoforge"], false)],
        ServerKind::Arclight | ServerKind::Mohist => {
            let mut out = vec![];
            let mods = if kind == ServerKind::Mohist {
                vec!["forge"]
            } else {
                hybrid_mod_loaders(root)
            };
            if !mods.is_empty() {
                out.push(platform(Mod, &mods, true));
            }
            out.push(platform(Plugin, &["spigot", "bukkit"], true));
            out
        }
        ServerKind::Vanilla | ServerKind::Bedrock | ServerKind::Unknown => vec![],
    }
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ContentError {
    #[error("sin conexión con el catálogo: {0}")]
    Network(String),
    #[error("no encontrado: {0}")]
    NotFound(String),
    #[error("{0}")]
    BadResponse(String),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Conflict(String),
    #[error("error de archivos: {0}")]
    Io(String),
    #[error("descarga dañada: {0}")]
    Integrity(String),
    #[error("operación cancelada; no se cambió nada en el servidor")]
    Cancelled,
}

impl From<merc_install::InstallError> for ContentError {
    fn from(e: merc_install::InstallError) -> Self {
        use merc_install::InstallError as I;
        match e {
            I::Network(m) => ContentError::Network(m),
            I::NotFound(m) => ContentError::NotFound(m),
            I::Integrity(m) => ContentError::Integrity(m),
            I::Cancelled => ContentError::Cancelled,
            I::Io(m) => ContentError::Io(m),
            other => ContentError::BadResponse(other.to_string()),
        }
    }
}

/// Nombre de archivo de un JAR que viene de fuera: un solo nombre, sin rutas ni caracteres raros.
pub fn safe_jar_name(name: &str) -> Result<String, ContentError> {
    safe_file_name(name, ".jar")
}

/// Como [`safe_jar_name`], con la extensión indicada (`.jar` o `.zip`).
pub fn safe_file_name(name: &str, ext: &str) -> Result<String, ContentError> {
    let n = name.trim();
    let bad = n.is_empty()
        || n.len() > 200
        || n.starts_with('.')
        || n.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
        || n.chars().any(char::is_control)
        || n.len() <= ext.len()
        || !n.to_lowercase().ends_with(ext);
    if bad {
        return Err(ContentError::Invalid(format!("nombre de archivo no válido: «{name}»")));
    }
    Ok(n.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platforms_by_server_kind() {
        let d = tempfile::tempdir().unwrap();
        // Vanilla: solo datapacks, en el mundo activo.
        let v = platforms(ServerKind::Vanilla, d.path());
        assert_eq!(v.len(), 1);
        assert_eq!(
            (v[0].kind, v[0].folder.as_str()),
            (ContentKind::Datapack, "world/datapacks")
        );
        std::fs::write(d.path().join("server.properties"), "level-name=Isla\n").unwrap();
        assert_eq!(platforms(ServerKind::Vanilla, d.path())[0].folder, "Isla/datapacks");
        std::fs::write(d.path().join("server.properties"), "level-name=../fuera\n").unwrap();
        assert_eq!(level_folder(d.path()), "world");
        assert!(platforms(ServerKind::Unknown, d.path()).is_empty());
        let p = platforms(ServerKind::Purpur, d.path());
        assert!(p[0].sources.contains(&ContentSource::Hangar));
        assert!(!platforms(ServerKind::Spigot, d.path())[0]
            .sources
            .contains(&ContentSource::Hangar));
        assert_eq!(p[0].kind, ContentKind::Plugin);
        assert_eq!(p[0].folder, "plugins");
        assert_eq!(p[0].loaders[0], "purpur");
        assert_eq!(platforms(ServerKind::Quilt, d.path())[0].loaders, ["quilt", "fabric"]);
        // Arclight sin JAR reconocible: solo plugins.
        let a = platforms(ServerKind::Arclight, d.path());
        assert_eq!(a.len(), 2);
        assert!(a[0].experimental);
        std::fs::write(d.path().join("arclight-neoforge-1.21.1-1.0.jar"), "x").unwrap();
        let a = platforms(ServerKind::Arclight, d.path());
        assert_eq!(a[0].kind, ContentKind::Mod);
        assert_eq!(a[0].loaders, ["neoforge"]);
    }

    #[test]
    fn jar_names_are_checked() {
        assert!(safe_jar_name("sodium-0.6.jar").is_ok());
        for bad in ["../x.jar", "a/b.jar", "x.zip", "C:x.jar", ".jar", "con\u{0}.jar"] {
            assert!(safe_jar_name(bad).is_err(), "{bad}");
        }
        assert!(safe_file_name("pack.zip", ".zip").is_ok());
        assert!(safe_file_name("pack.jar", ".zip").is_err());
        assert!(safe_file_name(".zip", ".zip").is_err());
    }
}
