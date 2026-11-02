//! Creación de servidores nuevos desde las fuentes oficiales:
//! Mojang (Vanilla), PaperMC Fill v3, Purpur, Fabric meta, Quilt, los maven de NeoForge y Forge
//! las publicaciones de Arclight en GitHub y el Bedrock Dedicated Server de minecraft.net.
//!
//! Cada descarga se verifica con la huella que publica la fuente (SHA-1 de Mojang,
//! SHA-256 de PaperMC, archivos .sha256/.sha1 del maven de NeoForge). Fabric no publica
//! huella del lanzador: se descarga por HTTPS y se avisa al usuario.

pub mod catalog;
pub mod http;
pub mod install;
pub mod temurin;

pub use catalog::{
    list_versions, loader_versions, resolve, resolve_pinned, Artifact, CatalogEntry, Flavor, LoaderOption, Resolved,
};
pub use http::Http;
pub use install::{
    check_target, install, validate_properties_for, validate_user_properties, InstallEvent, InstallOutcome,
    InstallPlan, Step,
};

use serde::{Deserialize, Serialize};

/// Direcciones base de las fuentes. Las pruebas las apuntan a un servidor local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sources {
    pub mojang_manifest: String,
    pub paper_api: String,
    pub fabric_meta: String,
    pub neoforge_maven: String,
    pub purpur_api: String,
    pub forge_promotions: String,
    pub forge_maven: String,
    pub quilt_meta: String,
    pub quilt_maven: String,
    pub arclight_releases: String,
    pub spigot_versions: String,
    pub buildtools_url: String,
    /// API de enlaces de descarga de minecraft.net (la que usa su página de Bedrock).
    pub bedrock_links: String,
}

impl Default for Sources {
    fn default() -> Self {
        Self {
            mojang_manifest: "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json".into(),
            paper_api: "https://fill.papermc.io".into(),
            fabric_meta: "https://meta.fabricmc.net".into(),
            neoforge_maven: "https://maven.neoforged.net/releases".into(),
            purpur_api: "https://api.purpurmc.org/v2".into(),
            forge_promotions: "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json".into(),
            forge_maven: "https://maven.minecraftforge.net".into(),
            quilt_meta: "https://meta.quiltmc.org/v3".into(),
            quilt_maven: "https://maven.quiltmc.org/repository/release".into(),
            arclight_releases: "https://api.github.com/repos/IzzelAliz/Arclight/releases?per_page=100".into(),
            spigot_versions: "https://hub.spigotmc.org/versions".into(),
            buildtools_url:
                "https://hub.spigotmc.org/jenkins/job/BuildTools/lastSuccessfulBuild/artifact/target/BuildTools.jar"
                    .into(),
            bedrock_links: "https://net-secondary.web.minecraft-services.net/api/v1.0/download/links".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "algorithm", content = "hex", rename_all = "snake_case")]
pub enum Checksum {
    /// MD5: solo detecta archivos dañados o cortados (es lo único que publica Purpur).
    Md5(String),
    Sha1(String),
    Sha256(String),
    None,
}

impl Checksum {
    pub fn expected(&self) -> Option<&str> {
        match self {
            Checksum::Md5(h) | Checksum::Sha1(h) | Checksum::Sha256(h) => Some(h),
            Checksum::None => None,
        }
    }
    pub fn algorithm(&self) -> &'static str {
        match self {
            Checksum::Md5(_) => "MD5",
            Checksum::Sha1(_) => "SHA-1",
            Checksum::Sha256(_) => "SHA-256",
            Checksum::None => "sin huella",
        }
    }
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum InstallError {
    #[error("sin conexión con la fuente oficial: {0}")]
    Network(String),
    #[error("no encontrado: {0}")]
    NotFound(String),
    #[error("{0}")]
    NotAvailable(String),
    #[error("{0}")]
    BadResponse(String),
    #[error("dirección no permitida: {0}")]
    UrlNotAllowed(String),
    #[error("el archivo es demasiado grande: {0}")]
    TooLarge(String),
    #[error("descarga dañada: {0}")]
    Integrity(String),
    #[error("{0}")]
    InvalidInput(String),
    #[error("la carpeta no está vacía: {0}")]
    TargetNotEmpty(String),
    #[error("{0}")]
    TargetNotUsable(String),
    #[error("{0}")]
    InstallerFailed(String),
    #[error("error de archivos: {0}")]
    Io(String),
    #[error("cancelado por el usuario")]
    Cancelled,
}
