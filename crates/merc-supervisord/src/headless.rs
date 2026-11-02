//! Servidor casero: crear servidores desde otra PC con «control total».
//!
//! La PC que pide solo elige qué crear (tipo, versión, nombre, ajustes, si acepta la EULA).
//! Dónde se guarda y con qué Java se arranca lo decide SIEMPRE esta PC:
//! - la carpeta va dentro de su carpeta de servidores, con un nombre sacado del nombre visible;
//! - el Java se elige entre los que tiene instalados (el que pide la versión de Minecraft).
//!
//! Así una PC remota nunca escribe en una ruta que elija ni ejecuta un programa que elija.

use super::{api, default_servers_dir, Daemon};
use merc_install::Flavor;
use merc_ipc::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Nombre de carpeta seguro a partir del nombre visible («Mi Survival» → «Mi-Survival»).
pub(crate) fn folder_name(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
            out.push(c);
        } else if (c == ' ' || c == '.') && !out.ends_with('-') {
            out.push('-');
        } else if c.is_alphanumeric() {
            // Letras con tilde u otras: se dejan fuera para que la ruta sea válida en cualquier sistema.
            let base = match c {
                'á' | 'à' | 'ä' | 'â' | 'Á' => 'a',
                'é' | 'è' | 'ë' | 'ê' | 'É' => 'e',
                'í' | 'ì' | 'ï' | 'î' | 'Í' => 'i',
                'ó' | 'ò' | 'ö' | 'ô' | 'Ó' => 'o',
                'ú' | 'ù' | 'ü' | 'û' | 'Ú' => 'u',
                'ñ' | 'Ñ' => 'n',
                _ => continue,
            };
            out.push(base);
        }
        if out.len() >= 40 {
            break;
        }
    }
    let t = out.trim_matches(['-', '_']).to_string();
    if t.is_empty() {
        "servidor".into()
    } else {
        t
    }
}

/// Primera carpeta libre `<dir>/<nombre>`, `<nombre>-2`, `<nombre>-3`…
pub(crate) fn free_folder(dir: &Path, name: &str) -> PathBuf {
    let base = folder_name(name);
    let mut p = dir.join(&base);
    let mut n = 2;
    while p.exists() {
        p = dir.join(format!("{base}-{n}"));
        n += 1;
    }
    p
}

/// ¿Sirve un Java de versión `major` para lo que se crea?
pub(crate) fn java_fits(major: u32, min_major: u32, exact_8: bool) -> bool {
    if exact_8 || min_major <= 8 {
        major == 8
    } else {
        major >= min_major
    }
}

/// Elige el Java de esta PC: el más bajo que cumpla (lo más compatible con mods de la época).
fn pick_java(d: &Arc<Daemon>, flavor: Flavor, mc_version: &str) -> Result<PathBuf, ApiError> {
    let req = merc_core::java::required_java(mc_version)
        .map(|r| r.min_major)
        .unwrap_or(21);
    let exact_8 = flavor == Flavor::Forge && merc_install::catalog::forge_is_legacy(mc_version);
    let needs_jdk = flavor == Flavor::Spigot;
    let mut found = merc_core::java::find_java_candidates();
    for p in d.managed_java() {
        if !found.contains(&p) {
            found.push(p);
        }
    }
    let mut best: Option<(u32, PathBuf)> = None;
    for p in found {
        let Ok(probe) = merc_core::java::probe_java(&p, Duration::from_secs(10)) else {
            continue;
        };
        if !java_fits(probe.major, req, exact_8) || (needs_jdk && !merc_install::install::has_javac(&p)) {
            continue;
        }
        if best.as_ref().is_none_or(|(m, _)| probe.major < *m) {
            best = Some((probe.major, p));
        }
    }
    best.map(|(_, p)| p).ok_or_else(|| {
        let need = if exact_8 || req <= 8 { 8 } else { req };
        api(
            ErrorCode::InvalidState,
            if needs_jdk {
                format!("El servidor casero no tiene un JDK {need} (Spigot se compila allí). Instálalo en esa PC y vuelve a intentarlo.")
            } else {
                format!("El servidor casero no tiene Java {need}. Descárgalo desde aquí con «Descargar Java {need} (Eclipse Temurin)» y vuelve a intentarlo.")
            },
        )
    })
}

/// Rehace la petición de crear: carpeta y Java los pone esta PC.
pub(crate) fn remote_create(d: &Arc<Daemon>, body: RequestBody) -> Result<RequestBody, ApiError> {
    let RequestBody::CreateServer {
        name,
        flavor,
        mc_version,
        path: _,
        java: _,
        min_memory_mb,
        max_memory_mb,
        properties,
        accept_eula,
        modpack_version,
        modpack_source,
        loader_version,
        game_rules,
    } = body
    else {
        return Ok(body);
    };
    let dir = default_servers_dir();
    std::fs::create_dir_all(&dir).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
    let path = free_folder(&dir, &name);
    let java = if flavor.is_native() {
        PathBuf::new()
    } else {
        pick_java(d, flavor, &mc_version)?
    };
    Ok(RequestBody::CreateServer {
        name,
        flavor,
        mc_version,
        path,
        java,
        min_memory_mb,
        max_memory_mb,
        properties,
        accept_eula,
        modpack_version,
        modpack_source,
        loader_version,
        game_rules,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_names_are_safe() {
        assert_eq!(folder_name("Mi Survival"), "Mi-Survival");
        assert_eq!(folder_name("Creativo de Amir ñ"), "Creativo-de-Amir-n");
        assert_eq!(folder_name("../../etc/passwd"), "etcpasswd");
        assert!(!folder_name("../../etc").contains(['/', '\\', '.']));
        assert_eq!(folder_name("///"), "servidor");
        assert_eq!(folder_name("🙂"), "servidor");
        assert!(folder_name(&"a".repeat(100)).len() <= 40);
    }

    #[test]
    fn free_folder_does_not_reuse() {
        let d = tempfile::tempdir().unwrap();
        let a = free_folder(d.path(), "Survival");
        assert_eq!(a, d.path().join("Survival"));
        std::fs::create_dir(&a).unwrap();
        assert_eq!(free_folder(d.path(), "Survival"), d.path().join("Survival-2"));
    }

    #[test]
    fn java_rules() {
        assert!(java_fits(21, 21, false));
        assert!(java_fits(25, 21, false));
        assert!(!java_fits(17, 21, false));
        assert!(java_fits(8, 8, false));
        assert!(!java_fits(17, 8, false));
        assert!(!java_fits(11, 16, true));
    }
}
