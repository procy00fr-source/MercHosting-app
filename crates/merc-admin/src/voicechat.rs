//! Simple Voice Chat (henkelmax): detectar el mod o plugin, leer su configuración real y
//! entender lo que dice la consola. El chat de voz usa su propio puerto UDP (por defecto
//! 24454, pero se lee siempre del archivo) y `voice_host` es la dirección que el servidor
//! anuncia a los clientes para conectar la voz.

use crate::props::PropsDoc;
use std::io::Read;
use std::path::Path;

/// Dónde crea su configuración: mods (Fabric, Forge, NeoForge, Quilt) o plugins (Paper…).
pub const CONFIG_PATHS: [&str; 2] = [
    "config/voicechat/voicechat-server.properties",
    "plugins/voicechat/voicechat-server.properties",
];

/// Identificador del mod y nombre del plugin.
pub const MOD_ID: &str = "voicechat";

/// Puerto por defecto del mod (solo para explicar; nunca se supone sin leer el archivo).
pub const DEFAULT_PORT: u16 = 24454;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceJar {
    /// Ruta relativa a la carpeta del servidor (`mods/voicechat-fabric-….jar`).
    pub rel: String,
    /// `false` si está desactivado (`.jar.disabled`).
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VoiceConfig {
    /// Ruta relativa del archivo.
    pub rel: String,
    /// Valor de `port` tal como está (`-1` = el mismo de Minecraft). `None` si falta o no es número.
    pub port: Option<i64>,
    pub bind_address: String,
    pub voice_host: String,
}

impl VoiceConfig {
    /// Puerto UDP en el que escuchará la voz.
    pub fn effective_port(&self, minecraft_port: Option<u16>) -> Option<u16> {
        match self.port? {
            -1 => minecraft_port,
            p if (1..=65535).contains(&p) => Some(p as u16),
            _ => None,
        }
    }
}

/// ¿Este jar es Simple Voice Chat? Se mira su ficha (no solo el nombre del archivo).
pub fn jar_is_voicechat(path: &Path) -> bool {
    let Ok(f) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(mut zip) = zip::ZipArchive::new(f) else {
        return false;
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
            if v.get("id").and_then(|x| x.as_str()) == Some(MOD_ID) {
                return true;
            }
        }
    }
    if let Some(t) = read("quilt.mod.json") {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) {
            if v.pointer("/quilt_loader/id").and_then(|x| x.as_str()) == Some(MOD_ID) {
                return true;
            }
        }
    }
    let re = regex::Regex::new(r#"(?m)^\s*modId\s*=\s*["']voicechat["']"#).expect("regex");
    for toml in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
        if let Some(t) = read(toml) {
            if re.is_match(&t) {
                return true;
            }
        }
    }
    for yml in ["plugin.yml", "paper-plugin.yml"] {
        if let Some(t) = read(yml) {
            let named = t.lines().any(|l| {
                l.strip_prefix("name:")
                    .map(|v| v.trim().trim_matches(['"', '\'']).eq_ignore_ascii_case(MOD_ID))
                    .unwrap_or(false)
            });
            if named {
                return true;
            }
        }
    }
    false
}

/// Busca Simple Voice Chat en `mods/` y `plugins/` (primero uno activo).
pub fn find_jar(root: &Path) -> Option<VoiceJar> {
    let mut found: Vec<VoiceJar> = vec![];
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
            if jar_is_voicechat(&e.path()) {
                found.push(VoiceJar {
                    rel: format!("{dir}/{name}"),
                    enabled,
                });
            }
        }
    }
    found.sort_by_key(|j| !j.enabled);
    found.into_iter().next()
}

/// Interpreta el texto de `voicechat-server.properties`.
pub fn parse_config(rel: &str, text: &str) -> VoiceConfig {
    let doc = PropsDoc::parse(text);
    VoiceConfig {
        rel: rel.to_string(),
        port: doc.get("port").and_then(|v| v.trim().parse().ok()),
        bind_address: doc.get("bind_address").unwrap_or("").trim().to_string(),
        voice_host: doc.get("voice_host").unwrap_or("").trim().to_string(),
    }
}

/// Lee la configuración que exista (la primera de [`CONFIG_PATHS`]).
pub fn read_config(root: &Path) -> Option<(VoiceConfig, String)> {
    CONFIG_PATHS.iter().find_map(|rel| {
        let text = std::fs::read_to_string(root.join(rel)).ok()?;
        Some((parse_config(rel, &text), text))
    })
}

/// Cambia claves en el texto conservando el resto (comentarios incluidos).
pub fn apply(text: &str, voice_host: Option<&str>, port: Option<u16>) -> String {
    let mut doc = PropsDoc::parse(text);
    if let Some(h) = voice_host {
        doc.set("voice_host", h.trim());
    }
    if let Some(p) = port {
        doc.set("port", &p.to_string());
    }
    doc.render()
}

/// ¿`bind_address` puede impedir que lleguen jugadores de fuera? (vacío, `*` y `0.0.0.0` van bien)
pub fn bind_address_is_restrictive(bind: &str) -> bool {
    let b = bind.trim();
    !(b.is_empty() || b == "*" || b == "0.0.0.0" || b == "::")
}

/// ¿`voice_host` apunta a esta dirección pública? (sin distinguir mayúsculas)
pub fn host_matches(voice_host: &str, address: &str) -> bool {
    let a = voice_host.trim().trim_end_matches('.');
    let b = address.trim().trim_end_matches('.');
    !a.is_empty() && a.eq_ignore_ascii_case(b)
}

/// Lo que dice la consola del chat de voz en esta sesión.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogSignal {
    /// «Voice chat server started at 0.0.0.0:24454» (o «Server started at port 24454»).
    Started { port: u16 },
    /// No pudo abrir su puerto (la línea, recortada).
    Failed(String),
}

fn trailing_port(s: &str) -> Option<u16> {
    let digits: String = s
        .trim_end()
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    digits.parse().ok()
}

/// Busca la última señal del chat de voz en líneas de consola.
pub fn scan_log<'a>(lines: impl IntoIterator<Item = &'a str>) -> Option<LogSignal> {
    let mut last = None;
    for line in lines {
        let lower = line.to_ascii_lowercase();
        let voice = lower.contains("voice");
        if voice && lower.contains("started at") {
            if let Some(port) = trailing_port(line) {
                last = Some(LogSignal::Started { port });
            }
        } else if lower.contains("server started at port") {
            // Versiones antiguas: «[VoiceChatServerThread/INFO]: Server started at port 24454».
            if let Some(port) = trailing_port(line) {
                last = Some(LogSignal::Started { port });
            }
        } else if voice
            && (lower.contains("failed to bind")
                || lower.contains("address already in use")
                || lower.contains("bindexception"))
        {
            let t: String = line.trim().chars().take(240).collect();
            last = Some(LogSignal::Failed(t));
        }
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const SAMPLE: &str = "# Simple Voice Chat server config v2.5.32\n\n# The port number to use for the voice chat communication.\nport=50127\n# The server IP address to bind the voice chat to\nbind_address=\n# The hostname that clients should use to connect to the voice chat\n# This may also include a port, e.g. 'example.com:24454' or just a port, e.g. '24454'\nvoice_host=\nmax_voice_distance=48.0\n";

    fn jar(path: &Path, files: &[(&str, &str)]) {
        let f = std::fs::File::create(path).unwrap();
        let mut z = zip::ZipWriter::new(f);
        for (n, t) in files {
            z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(t.as_bytes()).unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn reads_real_port_and_keeps_comments_when_writing() {
        let c = parse_config(CONFIG_PATHS[0], SAMPLE);
        assert_eq!(c.port, Some(50127));
        assert_eq!(c.effective_port(Some(25565)), Some(50127));
        assert_eq!(c.voice_host, "");
        let out = apply(SAMPLE, Some("147.185.221.181:25732"), None);
        assert!(out.contains("voice_host=147.185.221.181:25732\n"));
        assert!(out.contains("# This may also include a port"));
        assert!(out.contains("port=50127\n"));
        assert_eq!(parse_config("x", &out).voice_host, "147.185.221.181:25732");
        let out = apply(&out, None, Some(24460));
        assert_eq!(parse_config("x", &out).port, Some(24460));
    }

    #[test]
    fn minus_one_uses_the_minecraft_port() {
        let c = parse_config("x", "port=-1\n");
        assert_eq!(c.effective_port(Some(25570)), Some(25570));
        assert_eq!(parse_config("x", "port=abc\n").effective_port(Some(1)), None);
        assert_eq!(parse_config("x", "").port, None);
    }

    #[test]
    fn detects_mod_and_plugin_by_metadata() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("mods")).unwrap();
        std::fs::create_dir_all(d.path().join("plugins")).unwrap();
        // Un addon con «voicechat» en el nombre no es el mod.
        jar(
            &d.path().join("mods/voicechat-addon.jar"),
            &[("fabric.mod.json", r#"{"id":"voicechat_addon"}"#)],
        );
        assert_eq!(find_jar(d.path()), None);
        jar(
            &d.path().join("mods/vc.jar.disabled"),
            &[("fabric.mod.json", r#"{"schemaVersion":1,"id":"voicechat"}"#)],
        );
        assert_eq!(
            find_jar(d.path()),
            Some(VoiceJar {
                rel: "mods/vc.jar.disabled".into(),
                enabled: false
            })
        );
        jar(
            &d.path().join("plugins/x.jar"),
            &[(
                "plugin.yml",
                "main: de.maxhenkel.voicechat.Voicechat\nname: voicechat\nversion: 2.5.32\n",
            )],
        );
        assert_eq!(find_jar(d.path()).unwrap().rel, "plugins/x.jar");
        let neo = d.path().join("n.jar");
        jar(
            &neo,
            &[("META-INF/neoforge.mods.toml", "[[mods]]\nmodId=\"voicechat\"\n")],
        );
        assert!(jar_is_voicechat(&neo));
        assert!(!jar_is_voicechat(&d.path().join("no-existe.jar")));
    }

    #[test]
    fn reads_the_console() {
        let lines = [
            "[02:56:36 INFO]: [voicechat] Using server-ip as bind address: 0.0.0.0",
            "[02:56:36 INFO]: [voicechat] Voice chat server started at 0.0.0.0:50127",
        ];
        assert_eq!(scan_log(lines), Some(LogSignal::Started { port: 50127 }));
        assert_eq!(
            scan_log(["[VoiceChatServerThread/INFO]: Server started at port 38429"]),
            Some(LogSignal::Started { port: 38429 })
        );
        let failed = scan_log([
            "[voicechat] Voice chat server started at 0.0.0.0:24454",
            "[VoiceChatServerThread/ERROR]: [voicechat] Failed to bind: java.net.BindException: Address already in use",
        ]);
        assert!(matches!(failed, Some(LogSignal::Failed(_))));
        assert_eq!(scan_log(["Done (5.630s)! For help, type \"help\""]), None);
    }

    #[test]
    fn hosts_and_bind_address() {
        assert!(host_matches("Abc.ply.gg:123", "abc.ply.gg:123"));
        assert!(!host_matches("", "abc.ply.gg:123"));
        assert!(!host_matches("abc.ply.gg:124", "abc.ply.gg:123"));
        assert!(!bind_address_is_restrictive(""));
        assert!(!bind_address_is_restrictive("*"));
        assert!(bind_address_is_restrictive("192.168.1.5"));
    }
}
