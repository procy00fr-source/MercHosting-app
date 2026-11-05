//! Mods y plugins de los servidores del celular, desde el catálogo de Modrinth (API pública, sin
//! cuenta ni clave) y, para mods, desde CurseForge si la app se compiló con su clave
//! (`MERC_CURSEFORGE_API_KEY`, nunca en el repositorio). Sin clave sale como «Próximamente».
//!
//! - Plataforma según el tipo: Paper/Purpur → plugins, Fabric → mods, Vanilla → nada.
//! - Instalar usa el mismo plan verificado de escritorio (dependencias, hash de cada descarga);
//!   lo que se reemplaza va a la papelera de la app, nunca se borra.
//! - Con el servidor encendido no se toca nada.
//! - «Java + Bedrock»: Geyser y Floodgate (GeyserMC, MIT) desde Modrinth en un Paper o Purpur.

use crate::local::{Phone, PhoneError};
use merc_content::curseforge::CurseForge;
use merc_content::local::{self as cl, InstallProgress, InstalledItem};
use merc_content::modrinth::{SearchPage, SearchSort, Source};
use merc_content::plan::{self, ContentPlan};
use merc_content::{ContentError, ContentKind, ContentSource, Platform};
use merc_install::{Flavor, Http};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Proyectos de GeyserMC en Modrinth.
pub const GEYSER: &str = "geyser";
pub const FLOODGATE: &str = "floodgate";

impl From<ContentError> for PhoneError {
    fn from(e: ContentError) -> Self {
        match e {
            ContentError::Invalid(m) | ContentError::Conflict(m) => PhoneError::Invalid(m),
            ContentError::NotFound(m) => PhoneError::Unsupported(format!("No se encontró en Modrinth: {m}")),
            other => PhoneError::Io(other.to_string()),
        }
    }
}

/// Mods o plugins que admite un servidor de este tipo (solo el catálogo de Modrinth).
pub fn platform_for(flavor: Flavor, root: &Path) -> Option<Platform> {
    merc_content::platforms(flavor.kind(), root)
        .into_iter()
        .find(|p| matches!(p.kind, ContentKind::Mod | ContentKind::Plugin))
        .map(|mut p| {
            p.sources = vec![ContentSource::Modrinth];
            p
        })
}

/// Puerto UDP de Geyser si está instalado en ese servidor.
pub fn bedrock_port(root: &Path) -> Option<u16> {
    merc_admin::geyser::find(root).0?;
    Some(
        merc_admin::geyser::read_config(root)
            .and_then(|(_, p)| p)
            .unwrap_or(merc_admin::geyser::DEFAULT_PORT),
    )
}

fn label(flavor: Flavor) -> &'static str {
    match flavor {
        Flavor::Paper => "Paper",
        Flavor::Purpur => "Purpur",
        Flavor::Fabric => "Fabric",
        _ => "este tipo",
    }
}

/// Versión de Geyser de Modrinth para ese tipo y esa versión de Minecraft (sin descargar nada).
pub fn crossplay_versions(source: &dyn Source, flavor: Flavor, mc: &str) -> Result<Vec<String>, PhoneError> {
    let platform = platform_for(flavor, Path::new(""))
        .filter(|p| p.kind == ContentKind::Plugin)
        .ok_or_else(|| PhoneError::Invalid("Geyser solo se instala en Paper o Purpur.".into()))?;
    let list = source.versions(GEYSER, &platform.loaders, Some(mc))?;
    let v = plan::pick_version(&list).ok_or_else(|| {
        PhoneError::Unsupported(format!(
            "Geyser todavía no tiene versión para {} {mc} en Modrinth. Elige otra versión de Minecraft (la más nueva suele funcionar).",
            label(flavor)
        ))
    })?;
    Ok(vec![v.id.clone()])
}

pub use merc_content::geysermc::{FloodgateBuild, GEYSERMC_API};

/// La compilación más reciente de Floodgate para Spigot/Paper (sin descargarla).
pub fn floodgate_latest(http: &Http, base: &str) -> Result<FloodgateBuild, PhoneError> {
    merc_content::geysermc::floodgate_latest(http, base)
        .map_err(|e| PhoneError::Io(format!("No se pudo consultar Floodgate en download.geysermc.org: {e}")))
}

/// Descarga Floodgate a `plugins/`, comprobando su SHA-256, en un servidor recién creado.
pub fn install_floodgate(
    phone: &Phone,
    http: &Http,
    base: &str,
    root: &Path,
    fg: &FloodgateBuild,
    cancel: &AtomicBool,
) -> Result<(), PhoneError> {
    let staging = phone.data().join("descargas").join("floodgate");
    merc_content::geysermc::install_floodgate(http, base, root, "plugins", &staging, fg, cancel)?;
    Ok(())
}

/// Instala Geyser (de Modrinth) en un servidor recién creado (carpeta `root`).
#[allow(clippy::too_many_arguments)]
pub fn install_crossplay(
    phone: &Phone,
    http: &Http,
    source: &dyn Source,
    root: &Path,
    flavor: Flavor,
    mc: &str,
    versions: &[String],
    cancel: &AtomicBool,
) -> Result<(), PhoneError> {
    let platform = platform_for(flavor, root)
        .ok_or_else(|| PhoneError::Invalid("Geyser solo se instala en Paper o Purpur.".into()))?;
    let trash = trash_dir(phone, "nuevo");
    for vid in versions {
        let installed = cl::list_local(root, &platform.folder, platform.kind.ext())?;
        let plan = plan::build_plan(source, &platform, Some(mc), vid, &installed)?;
        let progress = InstallProgress::default();
        if cancel.load(Ordering::Relaxed) {
            return Err(PhoneError::Invalid("Cancelado.".into()));
        }
        apply(
            http,
            root,
            &platform,
            &plan,
            &phone.data().join("descargas").join("geyser"),
            &trash,
            &progress,
        )?;
    }
    Ok(())
}

fn trash_dir(phone: &Phone, id: &str) -> PathBuf {
    phone
        .data()
        .join("papelera")
        .join(format!("contenido-{id}-{}", crate::local::now_ms()))
}

fn apply(
    http: &Http,
    root: &Path,
    platform: &Platform,
    plan: &ContentPlan,
    staging: &Path,
    trash: &Path,
    progress: &InstallProgress,
) -> Result<Vec<String>, PhoneError> {
    let mut to_trash = |rel: &str| -> Result<(), String> { move_to_trash(root, rel, trash) };
    Ok(cl::apply_plan(
        http,
        root,
        &platform.folder,
        platform.kind.ext(),
        plan,
        staging,
        &mut to_trash,
        progress,
    )?)
}

/// Mueve `root/rel` a la papelera conservando la ruta relativa.
fn move_to_trash(root: &Path, rel: &str, trash: &Path) -> Result<(), String> {
    if rel.split('/').any(|p| p.is_empty() || p == "." || p == "..") {
        return Err(format!("ruta no válida: {rel}"));
    }
    let from = root.join(rel);
    let to = trash.join(rel);
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if std::fs::rename(&from, &to).is_err() {
        std::fs::copy(&from, &to).map_err(|e| e.to_string())?;
        std::fs::remove_file(&from).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Lo que se ve del contenido de un servidor del celular.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ContentInfo {
    /// `None` si este tipo no admite mods ni plugins (Vanilla).
    pub platform: Option<Platform>,
    pub mc_version: String,
    pub running: bool,
}

impl Phone {
    fn content_target(&self, id: &str) -> Result<(String, Platform, PathBuf), PhoneError> {
        let s = self.get(id)?;
        let root = self.server_dir(id);
        let p = platform_for(s.flavor, &root).ok_or_else(|| {
            PhoneError::Unsupported(
                "Vanilla no admite mods ni plugins. Crea un servidor Paper o Purpur (plugins) o Fabric (mods).".into(),
            )
        })?;
        Ok((s.mc_version, p, root))
    }

    fn content_stopped(&self, id: &str) -> Result<(), PhoneError> {
        if self.pid(id).is_some() {
            return Err(PhoneError::Invalid(
                "Detén el servidor antes de cambiar sus mods o plugins.".into(),
            ));
        }
        Ok(())
    }

    /// `curseforge`: la app trae la clave de CurseForge (entonces los mods también se buscan allí).
    pub fn content_info(&self, id: &str, curseforge: bool) -> Result<ContentInfo, PhoneError> {
        let s = self.get(id)?;
        let mut platform = platform_for(s.flavor, &self.server_dir(id));
        if let Some(p) = platform.as_mut() {
            if curseforge && p.kind == ContentKind::Mod {
                p.sources.push(ContentSource::Curseforge);
            }
        }
        Ok(ContentInfo {
            platform,
            mc_version: s.mc_version,
            running: self.pid(id).is_some(),
        })
    }

    /// Busca en Modrinth lo que sirve para este servidor (sin texto: lo más descargado).
    pub fn content_search(
        &self,
        source: &dyn Source,
        id: &str,
        query: &str,
        offset: u64,
    ) -> Result<SearchPage, PhoneError> {
        let (mc, p, _) = self.content_target(id)?;
        let sort = if query.trim().is_empty() {
            SearchSort::Downloads
        } else {
            SearchSort::Relevance
        };
        Ok(source.search(&p, Some(&mc), query, sort, offset)?)
    }

    /// Lo instalado; con conexión se completa con nombre y versión de Modrinth (y de CurseForge,
    /// por huella, si la app trae su clave y son mods).
    pub fn content_installed(
        &self,
        source: Option<&dyn Source>,
        curseforge: Option<&CurseForge>,
        id: &str,
    ) -> Result<Vec<InstalledItem>, PhoneError> {
        let (mc, p, root) = self.content_target(id)?;
        let mut items = cl::list_local(&root, &p.folder, p.kind.ext())?;
        if !items.is_empty() {
            if let Some(src) = source {
                let _ = cl::enrich(src, &p, Some(&mc), &mut items);
            }
            if let Some(cf) = curseforge.filter(|_| p.kind == ContentKind::Mod) {
                let _ = merc_content::curseforge::enrich_curseforge(cf, &root, &p, Some(&mc), &mut items);
            }
        }
        Ok(items)
    }

    /// Plan para instalar la versión más reciente de un proyecto que sirva para este servidor.
    pub fn content_plan(
        &self,
        source: &dyn Source,
        id: &str,
        project: &str,
    ) -> Result<(String, ContentPlan), PhoneError> {
        let (mc, p, root) = self.content_target(id)?;
        let list = source.versions(project, &p.loaders, Some(&mc))?;
        let v = plan::pick_version(&list).ok_or_else(|| {
            PhoneError::Unsupported(format!(
                "No hay una versión de esto para Minecraft {mc} en este tipo de servidor."
            ))
        })?;
        let installed = cl::list_local(&root, &p.folder, p.kind.ext())?;
        let plan = plan::build_plan(source, &p, Some(&mc), &v.id, &installed)?;
        Ok((v.id.clone(), plan))
    }

    /// Instala una versión (el plan se vuelve a resolver aquí: no se confía en el de la interfaz).
    pub fn content_install(
        &self,
        http: &Http,
        source: &dyn Source,
        id: &str,
        version_id: &str,
        progress: &InstallProgress,
    ) -> Result<Vec<String>, PhoneError> {
        self.content_stopped(id)?;
        let (mc, p, root) = self.content_target(id)?;
        let installed = cl::list_local(&root, &p.folder, p.kind.ext())?;
        let plan = plan::build_plan(source, &p, Some(&mc), version_id, &installed)?;
        let staging = self.data().join("descargas").join(format!("contenido-{id}"));
        apply(http, &root, &p, &plan, &staging, &trash_dir(self, id), progress)
    }

    /// Activar o desactivar (renombrar a `.disabled`; no se borra nada).
    pub fn content_set_enabled(&self, id: &str, file: &str, enabled: bool) -> Result<String, PhoneError> {
        self.content_stopped(id)?;
        let (_, p, root) = self.content_target(id)?;
        Ok(cl::set_enabled(&root, &p.folder, p.kind.ext(), file, enabled)?)
    }

    /// Quitar = mover a la papelera de la app (se puede recuperar).
    pub fn content_remove(&self, id: &str, file: &str) -> Result<(), PhoneError> {
        self.content_stopped(id)?;
        let (_, p, root) = self.content_target(id)?;
        let base = file.strip_suffix(cl::DISABLED_SUFFIX).unwrap_or(file);
        merc_content::safe_file_name(base, p.kind.ext())?;
        if !root.join(&p.folder).join(file).is_file() {
            return Err(PhoneError::Invalid(format!("No existe «{file}».")));
        }
        move_to_trash(&root, &format!("{}/{file}", p.folder), &trash_dir(self, id)).map_err(PhoneError::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local::LocalServer;
    use merc_content::modrinth::{ContentProject, ContentVersion, Side, VersionFile};
    use std::collections::HashMap;

    struct Fake {
        versions: HashMap<String, Vec<ContentVersion>>,
    }

    fn ver(id: &str, project: &str) -> ContentVersion {
        ContentVersion {
            id: id.into(),
            project_id: project.into(),
            name: id.into(),
            version_number: "1".into(),
            version_type: "release".into(),
            game_versions: vec!["1.21.1".into()],
            loaders: vec!["paper".into()],
            date_published: String::new(),
            downloads: 0,
            changelog: None,
            dependencies: vec![],
            files: vec![VersionFile {
                url: "https://cdn.modrinth.com/x.jar".into(),
                filename: format!("{project}.jar"),
                primary: true,
                size: 1,
                sha1: Some("0".repeat(40)),
                sha512: None,
                sha256: None,
            }],
            server_side: Default::default(),
        }
    }

    impl Source for Fake {
        fn search(
            &self,
            _: &Platform,
            _: Option<&str>,
            _: &str,
            _: SearchSort,
            offset: u64,
        ) -> Result<SearchPage, ContentError> {
            Ok(SearchPage {
                hits: vec![],
                total: 0,
                offset,
            })
        }
        fn project(&self, id: &str) -> Result<ContentProject, ContentError> {
            Ok(ContentProject {
                id: id.into(),
                slug: id.into(),
                title: id.into(),
                description: String::new(),
                body: String::new(),
                icon_url: None,
                client_side: Side::Optional,
                server_side: Side::Required,
                license: "MIT".into(),
                downloads: 0,
                categories: vec![],
                source_url: None,
                issues_url: None,
                wiki_url: None,
                page_url: format!("https://modrinth.com/plugin/{id}"),
                distribution_allowed: true,
            })
        }
        fn projects(&self, ids: &[String]) -> Result<Vec<ContentProject>, ContentError> {
            ids.iter().map(|i| self.project(i)).collect()
        }
        fn versions(
            &self,
            project: &str,
            loaders: &[String],
            mc: Option<&str>,
        ) -> Result<Vec<ContentVersion>, ContentError> {
            assert!(loaders.contains(&"paper".to_string()));
            Ok(self
                .versions
                .get(project)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|v| mc.is_none_or(|m| v.game_versions.iter().any(|g| g == m)))
                .collect())
        }
        fn version(&self, id: &str) -> Result<ContentVersion, ContentError> {
            self.versions
                .values()
                .flatten()
                .find(|v| v.id == id)
                .cloned()
                .ok_or_else(|| ContentError::NotFound(id.into()))
        }
        fn versions_by_sha1(&self, _: &[String]) -> Result<HashMap<String, ContentVersion>, ContentError> {
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

    fn fake() -> Fake {
        let mut versions = HashMap::new();
        versions.insert(GEYSER.to_string(), vec![ver("g1", "geyser-id")]);
        versions.insert(FLOODGATE.to_string(), vec![ver("f1", "floodgate-id")]);
        Fake { versions }
    }

    #[test]
    fn platforms_by_phone_flavor() {
        let d = tempfile::tempdir().unwrap();
        assert!(platform_for(Flavor::Vanilla, d.path()).is_none());
        let p = platform_for(Flavor::Paper, d.path()).unwrap();
        assert_eq!((p.kind, p.folder.as_str()), (ContentKind::Plugin, "plugins"));
        assert_eq!(
            p.sources,
            vec![ContentSource::Modrinth],
            "sin Hangar ni CurseForge en el celular"
        );
        let f = platform_for(Flavor::Fabric, d.path()).unwrap();
        assert_eq!((f.kind, f.folder.as_str()), (ContentKind::Mod, "mods"));
    }

    #[test]
    fn crossplay_needs_both_projects_for_that_version() {
        let src = fake();
        assert_eq!(crossplay_versions(&src, Flavor::Paper, "1.21.1").unwrap(), ["g1"]);
        let e = crossplay_versions(&src, Flavor::Paper, "1.18.2").unwrap_err();
        assert!(e.to_string().contains("Geyser todavía no tiene versión"), "{e}");
        assert!(crossplay_versions(&src, Flavor::Fabric, "1.21.1").is_err());
    }

    #[test]
    fn bedrock_port_only_with_geyser() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(bedrock_port(d.path()), None);
        std::fs::create_dir_all(d.path().join("plugins")).unwrap();
        let f = std::fs::File::create(d.path().join("plugins/Geyser-Spigot.jar")).unwrap();
        let mut z = zip::ZipWriter::new(f);
        z.start_file("plugin.yml", zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut z, b"name: Geyser-Spigot\nversion: 2\n").unwrap();
        z.finish().unwrap();
        assert_eq!(bedrock_port(d.path()), Some(19132));
    }

    #[test]
    fn content_ops_respect_type_and_keep_files() {
        let d = tempfile::tempdir().unwrap();
        let phone = Phone::new(d.path().into(), "aarch64", 8_192, None);
        let base = LocalServer {
            id: "p".into(),
            name: "S".into(),
            flavor: Flavor::Paper,
            mc_version: "1.21.1".into(),
            java_major: 21,
            heap_mb: 2048,
            jar: "paper.jar".into(),
            created_unix_ms: 0,
            playit: false,
            bedrock: false,
        };
        phone.register(base.clone()).unwrap();
        phone
            .register(LocalServer {
                id: "v".into(),
                flavor: Flavor::Vanilla,
                ..base.clone()
            })
            .unwrap();
        let src = fake();
        assert!(phone.content_info("v", true).unwrap().platform.is_none());
        // Con la clave de CurseForge: los plugins siguen solo en Modrinth; los mods, también en CurseForge.
        let sources = |id: &str, cf: bool| phone.content_info(id, cf).unwrap().platform.unwrap().sources;
        assert_eq!(sources("p", true), vec![ContentSource::Modrinth]);
        phone
            .register(LocalServer {
                id: "f".into(),
                flavor: Flavor::Fabric,
                ..base
            })
            .unwrap();
        assert_eq!(sources("f", false), vec![ContentSource::Modrinth]);
        assert_eq!(
            sources("f", true),
            vec![ContentSource::Modrinth, ContentSource::Curseforge]
        );
        assert!(matches!(
            phone.content_search(&src, "v", "", 0),
            Err(PhoneError::Unsupported(_))
        ));
        assert_eq!(phone.content_search(&src, "p", "", 0).unwrap().total, 0);

        let plugins = phone.server_dir("p").join("plugins");
        std::fs::create_dir_all(&plugins).unwrap();
        std::fs::write(plugins.join("a.jar"), b"hola").unwrap();
        assert_eq!(phone.content_installed(Some(&src), None, "p").unwrap().len(), 1);
        assert_eq!(
            phone.content_set_enabled("p", "a.jar", false).unwrap(),
            "a.jar.disabled"
        );
        phone.content_remove("p", "a.jar.disabled").unwrap();
        assert!(phone.content_installed(None, None, "p").unwrap().is_empty());
        let trashed: Vec<_> = walk(&d.path().join("papelera"));
        assert!(
            trashed.iter().any(|p| p.ends_with("plugins/a.jar.disabled")),
            "{trashed:?}"
        );
        assert!(phone.content_remove("p", "../servers.json").is_err());

        let (vid, plan) = phone.content_plan(&src, "p", GEYSER).unwrap();
        assert_eq!(vid, "g1");
        assert_eq!(plan.items.len(), 1);
    }

    fn walk(dir: &Path) -> Vec<PathBuf> {
        let mut out = vec![];
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    out.extend(walk(&p));
                } else {
                    out.push(p);
                }
            }
        }
        out
    }
}
