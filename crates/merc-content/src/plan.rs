//! Plan de instalación: qué archivos se descargan, por qué, qué se reemplaza y qué lo impide.
//!
//! Se resuelve antes de tocar nada y se vuelve a resolver en el supervisor al instalar (no se
//! confía en un plan que venga de la interfaz).

use crate::local::InstalledItem;
use crate::modrinth::{ContentVersion, DependencyType, Side, Source};
use crate::{safe_file_name, ContentError, ContentKind, Platform};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const MAX_ITEMS: usize = 60;
const MAX_DEPTH: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanItem {
    pub project_id: String,
    pub title: String,
    pub icon_url: Option<String>,
    pub version_id: String,
    pub version_number: String,
    pub file_name: String,
    pub url: String,
    /// SHA-1 de la descarga (vacío si el catálogo publica SHA-256, como Hangar).
    pub sha1: String,
    #[serde(default)]
    pub sha256: Option<String>,
    pub size: u64,
    /// «Lo elegiste tú» o «Lo necesita X».
    pub reason: String,
    /// Archivo instalado que se reemplaza (va a la papelera de MercHosting).
    pub replaces: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionalDep {
    pub project_id: String,
    pub title: String,
    pub needed_by: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ContentPlan {
    pub items: Vec<PlanItem>,
    /// Dependencias que ya están instaladas (no se tocan).
    pub already_installed: Vec<String>,
    pub optional: Vec<OptionalDep>,
    pub warnings: Vec<String>,
    /// Si hay alguno, no se puede instalar.
    pub blockers: Vec<String>,
    pub download_bytes: u64,
}

/// La versión preferida de una lista (Modrinth la devuelve de la más nueva a la más vieja):
/// la estable más nueva; si no hay, la más nueva de cualquier tipo.
pub fn pick_version(list: &[ContentVersion]) -> Option<&ContentVersion> {
    list.iter()
        .find(|v| v.version_type == "release")
        .or_else(|| list.first())
}

struct Ctx<'a> {
    source: &'a dyn Source,
    platform: &'a Platform,
    mc: Option<&'a str>,
    installed: &'a [InstalledItem],
    seen: HashSet<String>,
    plan: ContentPlan,
}

impl Ctx<'_> {
    fn installed_project(&self, project_id: &str) -> Option<&InstalledItem> {
        self.installed
            .iter()
            .find(|i| i.project_id.as_deref() == Some(project_id) || i.other_ids.iter().any(|o| o == project_id))
    }

    fn add(
        &mut self,
        version: &ContentVersion,
        reason: String,
        depth: usize,
        chosen: bool,
    ) -> Result<(), ContentError> {
        if self.plan.items.len() >= MAX_ITEMS || depth > MAX_DEPTH {
            self.plan
                .blockers
                .push("Hay demasiadas dependencias encadenadas; instálalas por partes.".into());
            return Ok(());
        }
        self.seen.insert(version.project_id.clone());
        let project = self.source.project(&version.project_id)?;
        let title = if project.title.is_empty() {
            version.name.clone()
        } else {
            project.title.clone()
        };
        if !version.fits(&self.platform.loaders, self.mc) {
            self.plan.blockers.push(format!(
                "La versión {} de «{title}» no es para este servidor ({}{}).",
                version.version_number,
                self.platform.loaders.join("/"),
                self.mc.map(|m| format!(" {m}")).unwrap_or_default()
            ));
        }
        if self.platform.kind == ContentKind::Mod {
            let side = match project.server_side {
                Side::Unknown => version.server_side,
                s => s,
            };
            match side {
                Side::Unsupported => self.plan.blockers.push(format!(
                    "«{title}» es solo para el juego de cada jugador (cliente): Modrinth indica que no funciona en un servidor."
                )),
                Side::Unknown => self.plan.warnings.push(format!(
                    "Modrinth no indica si «{title}» funciona en un servidor; si da errores al encender, quítalo."
                )),
                _ => {}
            }
            if project.client_side == Side::Required && chosen {
                self.plan.warnings.push(format!(
                    "«{title}» también hace falta en el juego de cada jugador: tus amigos deben instalarlo."
                ));
            }
        }
        if !project.distribution_allowed || version.primary_file().is_some_and(|f| f.url.is_empty()) {
            self.plan.blockers.push(format!(
                "El autor de «{title}» no permite descargarlo desde otras aplicaciones o lo publica en otra web. Descárgalo tú desde su página y súbelo en Archivos › {}.",
                self.platform.folder
            ));
            return Ok(());
        }
        let Some(file) = version.primary_file() else {
            self.plan.blockers.push(format!(
                "La versión {} de «{title}» no tiene archivos.",
                version.version_number
            ));
            return Ok(());
        };
        let ext = self.platform.kind.ext();
        let file_name = match safe_file_name(&file.filename, ext) {
            Ok(n) => n,
            Err(_) => {
                self.plan
                    .blockers
                    .push(format!("«{title}» no se distribuye como un {ext} ({}).", file.filename));
                return Ok(());
            }
        };
        let hex = |h: &Option<String>, len: usize| {
            h.clone()
                .filter(|h| h.len() == len && h.bytes().all(|b| b.is_ascii_hexdigit()))
                .map(|h| h.to_lowercase())
        };
        let sha1 = hex(&file.sha1, 40);
        let sha256 = hex(&file.sha256, 64);
        if sha1.is_none() && sha256.is_none() {
            self.plan.blockers.push(format!(
                "El catálogo no publica la huella (SHA-1 o SHA-256) de «{title}»; no se puede verificar la descarga."
            ));
            return Ok(());
        }
        let mut replaces = None;
        if let Some(inst) = self.installed_project(&version.project_id) {
            if inst.version_id.as_deref() == Some(version.id.as_str()) {
                self.plan
                    .blockers
                    .push(format!("Ya tienes instalada esta versión de «{title}»."));
                return Ok(());
            }
            replaces = Some(inst.file_name.clone());
        }
        self.plan.download_bytes += file.size;
        self.plan.items.push(PlanItem {
            project_id: version.project_id.clone(),
            title: title.clone(),
            icon_url: project.icon_url.clone(),
            version_id: version.id.clone(),
            version_number: version.version_number.clone(),
            file_name,
            url: file.url.clone(),
            sha1: sha1.unwrap_or_default(),
            sha256,
            size: file.size,
            reason,
            replaces,
        });

        for dep in &version.dependencies {
            match dep.dependency_type {
                DependencyType::Embedded => {}
                DependencyType::Incompatible => {
                    let pid = match (&dep.project_id, &dep.version_id) {
                        (Some(p), _) => Some(p.clone()),
                        (None, Some(v)) => self.source.version(v).ok().map(|v| v.project_id),
                        _ => None,
                    };
                    if let Some(inst) = pid.as_deref().and_then(|p| self.installed_project(p)) {
                        let other = inst.title.clone().unwrap_or_else(|| inst.file_name.clone());
                        self.plan.blockers.push(format!(
                            "«{title}» es incompatible con «{other}», que ya está instalado. Quítalo antes si quieres seguir."
                        ));
                    }
                }
                DependencyType::Optional => {
                    if let Some(pid) = &dep.project_id {
                        if self.installed_project(pid).is_none() && !self.seen.contains(pid) {
                            let name = self
                                .source
                                .project(pid)
                                .map(|p| p.title)
                                .unwrap_or_else(|_| pid.clone());
                            self.plan.optional.push(OptionalDep {
                                project_id: pid.clone(),
                                title: name,
                                needed_by: title.clone(),
                            });
                        }
                    }
                }
                DependencyType::Required => {
                    let (pid, fixed) = match (&dep.project_id, &dep.version_id) {
                        (_, Some(vid)) => match self.source.version(vid) {
                            Ok(v) => (v.project_id.clone(), Some(v)),
                            Err(_) => (dep.project_id.clone().unwrap_or_default(), None),
                        },
                        (Some(p), None) => (p.clone(), None),
                        (None, None) => {
                            if let Some(f) = &dep.file_name {
                                self.plan.warnings.push(format!(
                                    "«{title}» necesita «{f}», que no está en este catálogo: tendrás que conseguirlo aparte."
                                ));
                            }
                            continue;
                        }
                    };
                    if pid.is_empty() || self.seen.contains(&pid) {
                        continue;
                    }
                    if let Some(inst) = self.installed_project(&pid) {
                        let name = inst.title.clone().unwrap_or_else(|| inst.file_name.clone());
                        if inst.enabled {
                            self.plan.already_installed.push(name);
                        } else {
                            self.plan.warnings.push(format!(
                                "«{name}» (lo necesita «{title}») está instalado pero desactivado: actívalo."
                            ));
                        }
                        self.seen.insert(pid);
                        continue;
                    }
                    let chosen_dep = match fixed.filter(|v| v.fits(&self.platform.loaders, self.mc)) {
                        Some(v) => Some(v),
                        None => {
                            let list = self.source.versions(&pid, &self.platform.loaders, self.mc)?;
                            pick_version(&list).cloned()
                        }
                    };
                    match chosen_dep {
                        Some(v) => self.add(&v, format!("Lo necesita «{title}»"), depth + 1, false)?,
                        None => {
                            let name = self.source.project(&pid).map(|p| p.title).unwrap_or(pid.clone());
                            self.seen.insert(pid);
                            self.plan.blockers.push(format!(
                                "«{title}» necesita «{name}», pero no hay una versión de «{name}» para este servidor."
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Plan para instalar (o actualizar a) la versión `version_id`.
pub fn build_plan(
    source: &dyn Source,
    platform: &Platform,
    mc: Option<&str>,
    version_id: &str,
    installed: &[InstalledItem],
) -> Result<ContentPlan, ContentError> {
    let root = source.version(version_id)?;
    let mut ctx = Ctx {
        source,
        platform,
        mc,
        installed,
        seen: HashSet::new(),
        plan: ContentPlan::default(),
    };
    if mc.is_none() {
        ctx.plan.warnings.push(
            "No se sabe la versión de Minecraft de este servidor: comprueba tú que el archivo sea para ella.".into(),
        );
    }
    if platform.experimental {
        ctx.plan.warnings.push(
            "Este servidor es híbrido (mods + plugins): la compatibilidad no está garantizada aunque la versión coincida."
                .into(),
        );
    }
    ctx.add(&root, "Lo elegiste tú".into(), 0, true)?;
    // Dos dependencias con el mismo nombre de archivo se pisarían.
    let mut names = HashSet::new();
    for it in &ctx.plan.items {
        if !names.insert(it.file_name.to_lowercase()) {
            ctx.plan
                .blockers
                .push(format!("Dos archivos del plan se llaman igual: «{}».", it.file_name));
        }
    }
    ctx.plan.optional.dedup_by(|a, b| a.project_id == b.project_id);
    ctx.plan.already_installed.sort();
    ctx.plan.already_installed.dedup();
    Ok(ctx.plan)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::modrinth::{ContentHit, ContentProject, Dependency, SearchPage, SearchSort, VersionFile};
    use std::collections::HashMap;

    #[derive(Default)]
    pub struct Mem {
        pub projects: HashMap<String, ContentProject>,
        pub versions: Vec<ContentVersion>,
    }

    impl Mem {
        pub fn add_project(&mut self, id: &str, server: Side) {
            self.projects.insert(
                id.into(),
                ContentProject {
                    id: id.into(),
                    slug: id.to_lowercase(),
                    title: format!("Mod {id}"),
                    description: String::new(),
                    body: String::new(),
                    icon_url: None,
                    client_side: Side::Optional,
                    server_side: server,
                    license: "MIT".into(),
                    downloads: 0,
                    categories: vec![],
                    source_url: None,
                    issues_url: None,
                    wiki_url: None,
                    page_url: String::new(),
                    distribution_allowed: true,
                },
            );
        }
        pub fn add_version(&mut self, id: &str, project: &str, mc: &str, deps: Vec<(DependencyType, &str)>) {
            self.versions.push(ContentVersion {
                id: id.into(),
                project_id: project.into(),
                name: id.into(),
                version_number: format!("{id}-1.0"),
                version_type: "release".into(),
                game_versions: vec![mc.into()],
                loaders: vec!["fabric".into()],
                date_published: String::new(),
                downloads: 0,
                changelog: None,
                dependencies: deps
                    .into_iter()
                    .map(|(t, p)| Dependency {
                        version_id: None,
                        project_id: Some(p.into()),
                        file_name: None,
                        dependency_type: t,
                    })
                    .collect(),
                files: vec![VersionFile {
                    url: format!("https://cdn.modrinth.com/{id}.jar"),
                    filename: format!("{id}.jar"),
                    primary: true,
                    size: 100,
                    sha1: Some("a".repeat(40)),
                    sha512: None,
                    sha256: None,
                }],
                server_side: Side::Unknown,
            });
        }
    }

    impl Source for Mem {
        fn search(
            &self,
            _: &Platform,
            _: Option<&str>,
            _: &str,
            _: SearchSort,
            _: u64,
        ) -> Result<SearchPage, ContentError> {
            Ok(SearchPage {
                hits: Vec::<ContentHit>::new(),
                total: 0,
                offset: 0,
            })
        }
        fn project(&self, id: &str) -> Result<ContentProject, ContentError> {
            self.projects.get(id).cloned().ok_or(ContentError::NotFound(id.into()))
        }
        fn projects(&self, ids: &[String]) -> Result<Vec<ContentProject>, ContentError> {
            Ok(ids.iter().filter_map(|i| self.projects.get(i).cloned()).collect())
        }
        fn versions(
            &self,
            project: &str,
            loaders: &[String],
            mc: Option<&str>,
        ) -> Result<Vec<ContentVersion>, ContentError> {
            Ok(self
                .versions
                .iter()
                .filter(|v| v.project_id == project && v.fits(loaders, mc))
                .cloned()
                .collect())
        }
        fn version(&self, id: &str) -> Result<ContentVersion, ContentError> {
            self.versions
                .iter()
                .find(|v| v.id == id)
                .cloned()
                .ok_or(ContentError::NotFound(id.into()))
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

    pub fn fabric() -> Platform {
        Platform {
            kind: ContentKind::Mod,
            folder: "mods".into(),
            loaders: vec!["fabric".into()],
            experimental: false,
            sources: vec![crate::ContentSource::Modrinth],
        }
    }

    pub fn installed(file: &str, project: &str, version: &str, enabled: bool) -> InstalledItem {
        InstalledItem {
            file_name: file.into(),
            enabled,
            size: 1,
            sha1: "b".repeat(40),
            project_id: Some(project.into()),
            version_id: Some(version.into()),
            title: Some(format!("Mod {project}")),
            version_number: None,
            icon_url: None,
            server_side: None,
            client_side: None,
            page_url: None,
            update: None,
            source: Some(crate::ContentSource::Modrinth),
            other_ids: vec![],
        }
    }

    fn catalog() -> Mem {
        let mut m = Mem::default();
        for (p, side) in [
            ("A", Side::Required),
            ("API", Side::Required),
            ("LIB", Side::Optional),
            ("OPT", Side::Optional),
            ("CLI", Side::Unsupported),
            ("NOPE", Side::Required),
        ] {
            m.add_project(p, side);
        }
        m.add_version(
            "a1",
            "A",
            "1.21.1",
            vec![
                (DependencyType::Required, "API"),
                (DependencyType::Optional, "OPT"),
                (DependencyType::Incompatible, "NOPE"),
            ],
        );
        m.add_version(
            "api1",
            "API",
            "1.21.1",
            vec![(DependencyType::Required, "LIB"), (DependencyType::Required, "A")],
        );
        m.add_version("lib1", "LIB", "1.21.1", vec![]);
        m.add_version("lib0", "LIB", "1.20.1", vec![]);
        m.add_version("cli1", "CLI", "1.21.1", vec![]);
        m.add_version("b1", "B", "1.21.1", vec![(DependencyType::Required, "MISSING")]);
        m.add_project("B", Side::Required);
        m.add_project("MISSING", Side::Required);
        m
    }

    #[test]
    fn resolves_required_dependencies_recursively_without_cycles() {
        let m = catalog();
        let plan = build_plan(&m, &fabric(), Some("1.21.1"), "a1", &[]).unwrap();
        assert!(plan.blockers.is_empty(), "{:?}", plan.blockers);
        let files: Vec<&str> = plan.items.iter().map(|i| i.file_name.as_str()).collect();
        assert_eq!(files, ["a1.jar", "api1.jar", "lib1.jar"]);
        assert_eq!(plan.items[1].reason, "Lo necesita «Mod A»");
        assert_eq!(plan.optional[0].project_id, "OPT");
        assert_eq!(plan.download_bytes, 300);
    }

    #[test]
    fn skips_installed_and_blocks_incompatible_and_client_only() {
        let m = catalog();
        let have = [
            installed("api.jar", "API", "api0", true),
            installed("nope.jar", "NOPE", "n1", true),
        ];
        let plan = build_plan(&m, &fabric(), Some("1.21.1"), "a1", &have).unwrap();
        assert_eq!(plan.items.len(), 1);
        assert_eq!(plan.already_installed, ["Mod API"]);
        assert!(plan.blockers.iter().any(|b| b.contains("incompatible con «Mod NOPE»")));

        let plan = build_plan(&m, &fabric(), Some("1.21.1"), "cli1", &[]).unwrap();
        assert!(plan
            .blockers
            .iter()
            .any(|b| b.contains("solo para el juego de cada jugador")));

        let plan = build_plan(&m, &fabric(), Some("1.21.1"), "b1", &[]).unwrap();
        assert!(plan
            .blockers
            .iter()
            .any(|b| b.contains("no hay una versión de «Mod MISSING»")));

        // Otra versión de Minecraft: no encaja.
        let plan = build_plan(&m, &fabric(), Some("1.20.1"), "a1", &[]).unwrap();
        assert!(plan.blockers.iter().any(|b| b.contains("no es para este servidor")));
    }

    #[test]
    fn update_replaces_old_file_and_same_version_is_blocked() {
        let m = catalog();
        let have = [
            installed("lib-old.jar", "LIB", "lib0", true),
            installed("api.jar", "API", "api1", true),
            installed("a.jar", "A", "a0", true),
        ];
        let plan = build_plan(&m, &fabric(), Some("1.21.1"), "lib1", &have).unwrap();
        assert_eq!(plan.items[0].replaces.as_deref(), Some("lib-old.jar"));
        let have = [installed("lib.jar", "LIB", "lib1", true)];
        let plan = build_plan(&m, &fabric(), Some("1.21.1"), "lib1", &have).unwrap();
        assert!(plan.blockers.iter().any(|b| b.contains("Ya tienes instalada")));
        // Dependencia instalada pero desactivada: se avisa.
        let have = [installed("api.jar", "API", "api1", false)];
        let plan = build_plan(&m, &fabric(), Some("1.21.1"), "a1", &have).unwrap();
        assert!(plan.warnings.iter().any(|w| w.contains("desactivado")));
    }
}
