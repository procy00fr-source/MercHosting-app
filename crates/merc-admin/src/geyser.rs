//! Geyser y Floodgate (GeyserMC): dejan entrar a jugadores de Minecraft Bedrock (celular,
//! consola, Windows) a un servidor Java. Aquí solo se detectan y se lee su configuración real:
//! el puerto UDP en el que Geyser escucha a los jugadores de Bedrock.
//!
//! Identificadores (de la ficha de cada jar, no del nombre del archivo):
//! - Geyser: plugin `Geyser-Spigot`, mod `geyser-fabric` / `geyser_neoforge`.
//! - Floodgate: plugin y mod `floodgate`.

use std::io::Read;
use std::path::Path;

/// Puerto por defecto de Geyser para Bedrock (solo si todavía no creó su configuración).
pub const DEFAULT_PORT: u16 = 19132;

/// Carpetas de configuración que crea Geyser según la plataforma.
pub const CONFIG_PATHS: [&str; 3] = [
    "plugins/Geyser-Spigot/config.yml",
    "config/Geyser-Fabric/config.yml",
    "config/Geyser-NeoForge/config.yml",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    Geyser,
    Floodgate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Jar {
    /// Ruta relativa (`plugins/Geyser-Spigot.jar`).
    pub rel: String,
    /// `false` si está desactivado (`.jar.disabled`).
    pub enabled: bool,
}

/// Identificadores que declara un jar (ids de mod o nombre de plugin).
fn jar_ids(path: &Path) -> Vec<String> {
    let mut ids = vec![];
    let Ok(f) = std::fs::File::open(path) else {
        return ids;
    };
    let Ok(mut zip) = zip::ZipArchive::new(f) else {
        return ids;
    };
    let mut read = |name: &str| -> Option<String> {
        let mut e = zip.by_name(name).ok()?;
        if e.size() > 512 * 1024 {
            return None;
        }
        let mut s = String::new();
        e.read_to_string(&mut s).ok()?;
        Some(s)
    };
    if let Some(t) = read("fabric.mod.json") {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) {
            if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
                ids.push(id.to_string());
            }
        }
    }
    let re = regex::Regex::new(r#"(?m)^\s*modId\s*=\s*["']([^"']+)["']"#).expect("regex");
    for toml in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
        if let Some(t) = read(toml) {
            ids.extend(re.captures_iter(&t).map(|c| c[1].to_string()));
        }
    }
    for yml in ["plugin.yml", "paper-plugin.yml"] {
        if let Some(t) = read(yml) {
            if let Some(n) = t.lines().find_map(|l| {
                l.strip_prefix("name:")
                    .map(|v| v.trim().trim_matches(['"', '\'']).to_string())
            }) {
                ids.push(n);
            }
        }
    }
    ids
}

/// ¿Qué es este jar, si es de GeyserMC?
pub fn classify(path: &Path) -> Option<Which> {
    let ids = jar_ids(path);
    if ids.iter().any(|i| i.eq_ignore_ascii_case("floodgate")) {
        return Some(Which::Floodgate);
    }
    if ids.iter().any(|i| {
        let l = i.to_ascii_lowercase();
        l == "geyser" || l.starts_with("geyser-") || l.starts_with("geyser_")
    }) {
        return Some(Which::Geyser);
    }
    None
}

/// Busca Geyser y Floodgate en `mods/` y `plugins/` (primero los activos).
pub fn find(root: &Path) -> (Option<Jar>, Option<Jar>) {
    let mut geyser: Vec<Jar> = vec![];
    let mut flood: Vec<Jar> = vec![];
    for dir in ["mods", "plugins"] {
        let Ok(rd) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        let mut entries: Vec<_> = rd.flatten().filter(|e| e.path().is_file()).collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let name = e.file_name().to_string_lossy().into_owned();
            let lower = name.to_ascii_lowercase();
            let enabled = lower.ends_with(".jar");
            if !enabled && !lower.ends_with(".jar.disabled") {
                continue;
            }
            let jar = Jar {
                rel: format!("{dir}/{name}"),
                enabled,
            };
            match classify(&e.path()) {
                Some(Which::Geyser) => geyser.push(jar),
                Some(Which::Floodgate) => flood.push(jar),
                None => {}
            }
        }
    }
    geyser.sort_by_key(|j| !j.enabled);
    flood.sort_by_key(|j| !j.enabled);
    (geyser.into_iter().next(), flood.into_iter().next())
}

/// Lee `bedrock: port:` de la configuración de Geyser (YAML simple, sin dependencias).
pub fn parse_port(text: &str) -> Option<u16> {
    let mut in_bedrock = false;
    for line in text.lines() {
        let no_comment = line.split('#').next().unwrap_or("");
        if no_comment.trim().is_empty() {
            continue;
        }
        let indented = no_comment.starts_with(' ') || no_comment.starts_with('\t');
        if !indented {
            in_bedrock = no_comment.trim_end() == "bedrock:";
            continue;
        }
        if in_bedrock {
            if let Some(v) = no_comment.trim().strip_prefix("port:") {
                return v.trim().trim_matches(['"', '\'']).parse().ok().filter(|p| *p > 0);
            }
        }
    }
    None
}

/// Configuración de Geyser que exista: `(ruta relativa, puerto)`.
pub fn read_config(root: &Path) -> Option<(String, Option<u16>)> {
    CONFIG_PATHS.iter().find_map(|rel| {
        let text = std::fs::read_to_string(root.join(rel)).ok()?;
        Some((rel.to_string(), parse_port(&text)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn jar(path: &Path, entries: &[(&str, &str)]) {
        let f = std::fs::File::create(path).unwrap();
        let mut z = zip::ZipWriter::new(f);
        for (name, body) in entries {
            z.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(body.as_bytes()).unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn finds_plugins_and_mods_by_their_metadata() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("plugins")).unwrap();
        std::fs::create_dir_all(d.path().join("mods")).unwrap();
        jar(
            &d.path().join("plugins/Geyser-Spigot.jar"),
            &[("plugin.yml", "name: Geyser-Spigot\nversion: 2.9\n")],
        );
        jar(
            &d.path().join("plugins/cualquiera.jar.disabled"),
            &[("plugin.yml", "name: floodgate\n")],
        );
        jar(
            &d.path().join("plugins/geyser-falso.jar"),
            &[("plugin.yml", "name: OtroPlugin\n")],
        );
        let (g, f) = find(d.path());
        assert_eq!(
            g,
            Some(Jar {
                rel: "plugins/Geyser-Spigot.jar".into(),
                enabled: true
            })
        );
        assert_eq!(
            f,
            Some(Jar {
                rel: "plugins/cualquiera.jar.disabled".into(),
                enabled: false
            })
        );

        jar(
            &d.path().join("mods/a.jar"),
            &[("fabric.mod.json", r#"{"id":"geyser-fabric"}"#)],
        );
        assert_eq!(classify(&d.path().join("mods/a.jar")), Some(Which::Geyser));
        jar(
            &d.path().join("mods/b.jar"),
            &[("META-INF/neoforge.mods.toml", "[[mods]]\nmodId=\"geyser_neoforge\"\n")],
        );
        assert_eq!(classify(&d.path().join("mods/b.jar")), Some(Which::Geyser));
        jar(
            &d.path().join("mods/c.jar"),
            &[("META-INF/neoforge.mods.toml", "[[mods]]\nmodId=\"floodgate\"\n")],
        );
        assert_eq!(classify(&d.path().join("mods/c.jar")), Some(Which::Floodgate));
        jar(
            &d.path().join("mods/d.jar"),
            &[("fabric.mod.json", r#"{"id":"geyserish"}"#)],
        );
        assert_eq!(classify(&d.path().join("mods/d.jar")), None);
    }

    #[test]
    fn reads_bedrock_port_only_from_its_section() {
        let cfg = "java:\n  port: 25565\nbedrock:\n  # el puerto de Bedrock\n  address: 0.0.0.0\n  port: 19200 # comentario\n  clone-remote-port: false\nremote:\n  port: 25565\n";
        assert_eq!(parse_port(cfg), Some(19200));
        assert_eq!(parse_port("bedrock:\n  address: 0.0.0.0\nremote:\n  port: 1\n"), None);
        assert_eq!(parse_port("bedrock:\n  port: \"19133\"\n"), Some(19133));
        let d = tempfile::tempdir().unwrap();
        assert_eq!(read_config(d.path()), None);
        std::fs::create_dir_all(d.path().join("config/Geyser-Fabric")).unwrap();
        std::fs::write(d.path().join("config/Geyser-Fabric/config.yml"), cfg).unwrap();
        assert_eq!(
            read_config(d.path()),
            Some(("config/Geyser-Fabric/config.yml".into(), Some(19200)))
        );
    }
}
