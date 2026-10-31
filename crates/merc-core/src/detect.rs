//! Detección del tipo de servidor en una carpeta existente, sin modificarla.
//!
//! Cada conclusión se acompaña de evidencia y de un nivel de confianza. Lo que no se puede
//! confirmar se marca como desconocido para que el usuario lo corrija; nunca se inventa.

use crate::launch::LaunchTarget;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerKind {
    Vanilla,
    Paper,
    Purpur,
    Spigot,
    Fabric,
    Forge,
    NeoForge,
    Quilt,
    /// Híbrido Forge + plugins de Bukkit/Spigot (MohistMC).
    Mohist,
    /// Híbrido de mods (NeoForge/Forge/Fabric) + plugins de Bukkit (Arclight).
    Arclight,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Leído de un archivo generado por el instalador oficial (p. ej. run.bat de NeoForge).
    Verified,
    /// Deducido de nombres de archivos o carpetas.
    Inferred,
    /// No se pudo determinar.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedServer {
    pub kind: ServerKind,
    pub mc_version: Option<String>,
    pub loader_version: Option<String>,
    pub launch: Option<LaunchTarget>,
    pub confidence: Confidence,
    /// Frases explicativas para mostrar al usuario ("Encontré run.bat que apunta a …").
    pub evidence: Vec<String>,
    /// Advertencias (varias versiones instaladas, archivos faltantes…).
    pub warnings: Vec<String>,
}

fn args_file_name() -> &'static str {
    if cfg!(windows) {
        "win_args.txt"
    } else {
        "unix_args.txt"
    }
}

/// Convierte la versión de NeoForge a versión de Minecraft.
/// NeoForge >= 20.2 usa `<mc_minor>.<mc_patch>.<build>`: 21.1.77 -> 1.21.1, 21.0.x -> 1.21.
/// NeoForge 47.x (solo 1.20.1) sigue el esquema de Forge y se trata aparte.
pub fn neoforge_to_mc(version: &str) -> Option<String> {
    let core = version.split('-').next()?;
    let parts: Vec<u32> = core.split('.').map(|p| p.parse().ok()).collect::<Option<_>>()?;
    let major = *parts.first()?;
    let minor = *parts.get(1)?;
    if major == 47 {
        return Some("1.20.1".into());
    }
    // Desde Minecraft 26.1, NeoForge usa 4 partes: AÑO.DROP.HOTFIX.BUILD
    // (26.1.0.x → 26.1, 26.1.2.76 → 26.1.2).
    if (26..=99).contains(&major) && parts.len() == 4 {
        let hotfix = parts[2];
        return Some(if hotfix == 0 {
            format!("{major}.{minor}")
        } else {
            format!("{major}.{minor}.{hotfix}")
        });
    }
    // Numeración antigua de 3 partes: 21.1.x → 1.21.1, 21.10.x → 1.21.10, 20.4.x → 1.20.4.
    if !(20..=25).contains(&major) || parts.len() != 3 {
        return None;
    }
    Some(if minor == 0 {
        format!("1.{major}")
    } else {
        format!("1.{major}.{minor}")
    })
}

fn list_dirs(p: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(p)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    v.sort_by(|a, b| version_cmp(a, b));
    v
}

fn list_files(p: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(p)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    v.sort();
    v
}

/// ¿Parece una versión de lanzamiento de Minecraft? (`1.21.1`, `1.21`, `26.1`, `26.1.2`)
pub fn looks_like_mc(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if !(2..=3).contains(&parts.len())
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return false;
    }
    let first: u32 = parts[0].parse().unwrap_or(0);
    first == 1 || (26..=99).contains(&first)
}

/// Comparación numérica por segmentos ("21.1.10" > "21.1.9").
pub fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let seg = |s: &str| -> Vec<u64> {
        s.split(|c: char| !c.is_ascii_digit())
            .filter_map(|x| x.parse().ok())
            .collect()
    };
    seg(a).cmp(&seg(b)).then_with(|| a.cmp(b))
}

/// Busca en run.bat / run.sh la referencia `@libraries/.../*_args.txt`.
fn args_from_run_script(root: &Path) -> Option<(PathBuf, String)> {
    let script = if cfg!(windows) { "run.bat" } else { "run.sh" };
    let text = std::fs::read_to_string(root.join(script)).ok()?;
    let re = Regex::new(r"@(libraries[/\\][^\s]+?_args\.txt)").unwrap();
    let rel = re.captures(&text)?.get(1)?.as_str().replace('\\', "/");
    Some((PathBuf::from(rel), script.to_string()))
}

fn user_jvm_args(root: &Path) -> Option<PathBuf> {
    root.join("user_jvm_args.txt")
        .is_file()
        .then(|| PathBuf::from("user_jvm_args.txt"))
}

fn detect_modern_forge_like(root: &Path, kind: ServerKind, lib: &str, out: &mut DetectedServer) -> bool {
    let lib_dir = root.join(lib);
    let from_script = args_from_run_script(root).filter(|(rel, _)| rel.starts_with(lib));
    let installed = list_dirs(&lib_dir);
    if from_script.is_none() && installed.is_empty() {
        return false;
    }
    out.kind = kind;
    let (args_rel, version) = if let Some((rel, script)) = from_script {
        let version = rel
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        out.evidence.push(format!("{script} apunta a {}", rel.display()));
        out.confidence = Confidence::Verified;
        (rel, version)
    } else {
        let version = installed.last().cloned().unwrap_or_default();
        out.evidence.push(format!(
            "Carpeta {lib}/{version} encontrada (sin script run de esta plataforma)"
        ));
        out.confidence = Confidence::Inferred;
        (PathBuf::from(lib).join(&version).join(args_file_name()), version)
    };
    if installed.len() > 1 {
        out.warnings.push(format!(
            "Hay varias versiones instaladas en {lib}: {}. Se usará {version}.",
            installed.join(", ")
        ));
    }
    if !root.join(&args_rel).is_file() {
        out.warnings
            .push(format!("Falta el archivo de argumentos {}", args_rel.display()));
    }
    out.mc_version = match kind {
        ServerKind::NeoForge => neoforge_to_mc(&version),
        // Forge moderno: "1.20.1-47.2.0"
        _ => version
            .split('-')
            .next()
            .filter(|s| looks_like_mc(s))
            .map(str::to_string),
    };
    out.loader_version = Some(match kind {
        ServerKind::Forge => version
            .split_once('-')
            .map(|(_, f)| f.to_string())
            .unwrap_or(version.clone()),
        _ => version.clone(),
    });
    out.launch = Some(LaunchTarget::ArgsFile {
        args_file: args_rel,
        user_jvm_args: user_jvm_args(root),
    });
    true
}

/// Analiza una carpeta. No escribe nada.
pub fn detect_server(root: &Path) -> DetectedServer {
    let mut out = DetectedServer {
        kind: ServerKind::Unknown,
        mc_version: None,
        loader_version: None,
        launch: None,
        confidence: Confidence::Unknown,
        evidence: vec![],
        warnings: vec![],
    };

    // Arclight y Mohist descargan librerías del cargador en su primer arranque: su JAR manda
    // sobre esas carpetas. arclight-neoforge-1.21.1-1.0.1-8ec9529.jar
    if let Some(j) = list_files(root).into_iter().rev().find(|f| {
        let l = f.to_ascii_lowercase();
        l.starts_with("arclight-") && l.ends_with(".jar")
    }) {
        let re = Regex::new(r"^arclight-(neoforge|forge|fabric)-([0-9][0-9.]*)-(.+)\.jar$").unwrap();
        let caps = re
            .captures(&j.to_ascii_lowercase())
            .map(|c| (c[1].to_string(), c[2].to_string(), c[3].to_string()));
        out.kind = ServerKind::Arclight;
        if let Some((loader, mc, ver)) = caps {
            out.mc_version = looks_like_mc(&mc).then_some(mc);
            out.loader_version = Some(format!("{loader} {ver}"));
        }
        out.launch = Some(LaunchTarget::Jar { jar: PathBuf::from(&j) });
        out.confidence = Confidence::Inferred;
        out.evidence.push(format!("Archivo {j} encontrado"));
        return out;
    }

    // Mohist descarga librerías de Forge en su primer arranque: su JAR manda sobre esas carpetas.
    if let Some(j) = list_files(root).into_iter().rev().find(|f| {
        let l = f.to_ascii_lowercase();
        l.starts_with("mohist-") && l.ends_with(".jar")
    }) {
        // mohist-1.20.1-123.jar
        let rest = &j["mohist-".len()..j.len() - 4];
        let mut parts = rest.splitn(2, '-');
        out.kind = ServerKind::Mohist;
        out.mc_version = parts.next().filter(|s| looks_like_mc(s)).map(str::to_string);
        out.loader_version = parts.next().map(str::to_string);
        out.launch = Some(LaunchTarget::Jar { jar: PathBuf::from(&j) });
        out.confidence = Confidence::Inferred;
        out.evidence.push(format!("Archivo {j} encontrado"));
        return out;
    }

    if detect_modern_forge_like(root, ServerKind::NeoForge, "libraries/net/neoforged/neoforge", &mut out) {
        return out;
    }
    if detect_modern_forge_like(root, ServerKind::Forge, "libraries/net/minecraftforge/forge", &mut out) {
        return out;
    }

    let files = list_files(root);
    let jars: Vec<&String> = files
        .iter()
        .filter(|f| f.to_ascii_lowercase().ends_with(".jar"))
        .collect();

    let pick = |pred: &dyn Fn(&str) -> bool| -> Vec<String> {
        jars.iter()
            .filter(|j| pred(&j.to_ascii_lowercase()))
            .map(|s| s.to_string())
            .collect()
    };

    let set_jar =
        |out: &mut DetectedServer, kind: ServerKind, jar: &str, mc: Option<String>, loader: Option<String>| {
            out.kind = kind;
            out.mc_version = mc;
            out.loader_version = loader;
            out.launch = Some(LaunchTarget::Jar {
                jar: PathBuf::from(jar),
            });
            out.confidence = Confidence::Inferred;
            out.evidence.push(format!("Archivo {jar} encontrado"));
        };

    let multi_warn = |out: &mut DetectedServer, list: &[String]| {
        if list.len() > 1 {
            out.warnings.push(format!(
                "Hay varios JAR candidatos: {}. Revisa cuál usar.",
                list.join(", ")
            ));
        }
    };

    // Fabric: el lanzador del instalador oficial.
    let fabric =
        pick(&|j| j == "fabric-server-launch.jar" || (j.starts_with("fabric-server-mc.") && j.contains("-launcher.")));
    if let Some(j) = fabric.last() {
        // fabric-server-mc.1.21.1-loader.0.16.5-launcher.1.0.1.jar
        let re = Regex::new(r"fabric-server-mc\.([0-9.]+)-loader\.([0-9.]+)-launcher").unwrap();
        let caps = re.captures(j);
        let mc = caps.as_ref().and_then(|c| c.get(1)).map(|m| m.as_str().to_string());
        let loader = caps.as_ref().and_then(|c| c.get(2)).map(|m| m.as_str().to_string());
        set_jar(&mut out, ServerKind::Fabric, j, mc, loader);
        multi_warn(&mut out, &fabric);
        return out;
    }

    // Quilt: lanzador que crea el instalador oficial (`install server … --download-server`).
    let quilt = pick(&|j| j == "quilt-server-launch.jar");
    if let Some(j) = quilt.last() {
        set_jar(&mut out, ServerKind::Quilt, j, None, None);
        return out;
    }

    for (prefix, kind) in [
        ("paper-", ServerKind::Paper),
        ("purpur-", ServerKind::Purpur),
        ("spigot-", ServerKind::Spigot),
    ] {
        let list = pick(&|j| j.starts_with(prefix));
        if let Some(j) = list.last() {
            // paper-1.21.1-123.jar / spigot-1.21.1.jar
            let rest = &j[prefix.len()..j.len() - 4];
            let mut parts = rest.splitn(2, '-');
            let mc = parts.next().filter(|s| looks_like_mc(s)).map(str::to_string);
            let build = parts.next().map(str::to_string);
            set_jar(&mut out, kind, j, mc, build);
            multi_warn(&mut out, &list);
            return out;
        }
    }

    let vanilla = pick(&|j| j == "server.jar" || j.starts_with("minecraft_server."));
    if let Some(j) = vanilla.last() {
        let mc = j
            .strip_prefix("minecraft_server.")
            .and_then(|s| s.strip_suffix(".jar"))
            .map(str::to_string);
        set_jar(&mut out, ServerKind::Vanilla, j, mc, None);
        if out.mc_version.is_none() {
            out.warnings
                .push("No se pudo leer la versión de Minecraft desde el nombre del JAR; indícala manualmente.".into());
        }
        multi_warn(&mut out, &vanilla);
        return out;
    }

    if !jars.is_empty() {
        out.warnings.push(format!(
            "No reconozco el tipo de servidor. JAR encontrados: {}",
            jars.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        ));
    } else {
        out.warnings
            .push("No hay ningún JAR de servidor en esta carpeta.".into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(root: &Path, rel: &str, content: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    #[test]
    fn neoforge_mapping() {
        assert_eq!(neoforge_to_mc("21.1.77").as_deref(), Some("1.21.1"));
        assert_eq!(neoforge_to_mc("21.0.167").as_deref(), Some("1.21"));
        assert_eq!(neoforge_to_mc("20.4.237").as_deref(), Some("1.20.4"));
        assert_eq!(neoforge_to_mc("47.1.106").as_deref(), Some("1.20.1"));
        assert_eq!(neoforge_to_mc("21.10.64").as_deref(), Some("1.21.10"));
        assert_eq!(neoforge_to_mc("26.1.2.76").as_deref(), Some("26.1.2"));
        assert_eq!(neoforge_to_mc("26.1.0.10-beta").as_deref(), Some("26.1"));
        assert_eq!(neoforge_to_mc("26.2.0.3").as_deref(), Some("26.2"));
        assert_eq!(neoforge_to_mc("basura"), None);
        assert!(looks_like_mc("1.21.1") && looks_like_mc("26.1") && looks_like_mc("26.1.2"));
        assert!(!looks_like_mc("25w14a") && !looks_like_mc("123") && !looks_like_mc("1.21.1.4"));
    }

    #[test]
    fn neoforge_from_run_script() {
        let d = tempfile::tempdir().unwrap();
        let args = args_file_name();
        let script = if cfg!(windows) { "run.bat" } else { "run.sh" };
        touch(
            d.path(),
            script,
            &format!("java @user_jvm_args.txt @libraries/net/neoforged/neoforge/21.1.77/{args} %*\n"),
        );
        touch(
            d.path(),
            &format!("libraries/net/neoforged/neoforge/21.1.77/{args}"),
            "-cp x",
        );
        touch(d.path(), "libraries/net/neoforged/neoforge/21.1.50/placeholder", "");
        touch(d.path(), "user_jvm_args.txt", "# -Xmx4G");
        let r = detect_server(d.path());
        assert_eq!(r.kind, ServerKind::NeoForge);
        assert_eq!(r.confidence, Confidence::Verified);
        assert_eq!(r.mc_version.as_deref(), Some("1.21.1"));
        assert_eq!(r.loader_version.as_deref(), Some("21.1.77"));
        assert!(r.warnings.iter().any(|w| w.contains("varias versiones")));
        match r.launch.unwrap() {
            LaunchTarget::ArgsFile {
                args_file,
                user_jvm_args,
            } => {
                assert_eq!(
                    args_file,
                    PathBuf::from(format!("libraries/net/neoforged/neoforge/21.1.77/{args}"))
                );
                assert_eq!(user_jvm_args, Some(PathBuf::from("user_jvm_args.txt")));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn forge_without_script() {
        let d = tempfile::tempdir().unwrap();
        touch(
            d.path(),
            &format!("libraries/net/minecraftforge/forge/1.20.1-47.2.0/{}", args_file_name()),
            "",
        );
        let r = detect_server(d.path());
        assert_eq!(r.kind, ServerKind::Forge);
        assert_eq!(r.confidence, Confidence::Inferred);
        assert_eq!(r.mc_version.as_deref(), Some("1.20.1"));
        assert_eq!(r.loader_version.as_deref(), Some("47.2.0"));
    }

    #[test]
    fn paper_fabric_vanilla() {
        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "paper-1.21.1-130.jar", "");
        let r = detect_server(d.path());
        assert_eq!(
            (r.kind, r.mc_version.as_deref(), r.loader_version.as_deref()),
            (ServerKind::Paper, Some("1.21.1"), Some("130"))
        );

        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "fabric-server-mc.1.21.1-loader.0.16.5-launcher.1.0.1.jar", "");
        let r = detect_server(d.path());
        assert_eq!(
            (r.kind, r.mc_version.as_deref(), r.loader_version.as_deref()),
            (ServerKind::Fabric, Some("1.21.1"), Some("0.16.5"))
        );

        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "server.jar", "");
        let r = detect_server(d.path());
        assert_eq!(r.kind, ServerKind::Vanilla);
        assert_eq!(r.mc_version, None);
        assert!(!r.warnings.is_empty());
    }

    #[test]
    fn year_based_versions() {
        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "paper-26.1.2-14.jar", "");
        let r = detect_server(d.path());
        assert_eq!((r.kind, r.mc_version.as_deref()), (ServerKind::Paper, Some("26.1.2")));

        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "fabric-server-mc.26.1-loader.0.18.2-launcher.1.1.0.jar", "");
        let r = detect_server(d.path());
        assert_eq!((r.kind, r.mc_version.as_deref()), (ServerKind::Fabric, Some("26.1")));

        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "minecraft_server.26.1.2.jar", "");
        let r = detect_server(d.path());
        assert_eq!((r.kind, r.mc_version.as_deref()), (ServerKind::Vanilla, Some("26.1.2")));
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn arclight_jar() {
        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "arclight-neoforge-1.21.1-1.0.1-8ec9529.jar", "");
        std::fs::create_dir_all(d.path().join("libraries/net/neoforged/neoforge/21.1.192")).unwrap();
        let r = detect_server(d.path());
        assert_eq!(
            (r.kind, r.mc_version.as_deref(), r.loader_version.as_deref()),
            (ServerKind::Arclight, Some("1.21.1"), Some("neoforge 1.0.1-8ec9529"))
        );
    }

    #[test]
    fn quilt_and_mohist() {
        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "quilt-server-launch.jar", "");
        touch(d.path(), "server.jar", "");
        let r = detect_server(d.path());
        assert_eq!(r.kind, ServerKind::Quilt);

        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "mohist-1.20.1-912.jar", "");
        std::fs::create_dir_all(d.path().join("libraries/net/minecraftforge/forge/1.20.1-47.2.0")).unwrap();
        let r = detect_server(d.path());
        assert_eq!(
            (r.kind, r.mc_version.as_deref(), r.loader_version.as_deref()),
            (ServerKind::Mohist, Some("1.20.1"), Some("912"))
        );
    }

    #[test]
    fn empty_folder() {
        let d = tempfile::tempdir().unwrap();
        let r = detect_server(d.path());
        assert_eq!(r.kind, ServerKind::Unknown);
        assert!(r.launch.is_none());
    }

    #[test]
    fn version_ordering() {
        assert_eq!(version_cmp("21.1.10", "21.1.9"), std::cmp::Ordering::Greater);
    }
}
