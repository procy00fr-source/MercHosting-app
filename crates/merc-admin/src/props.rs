//! Edición de `server.properties` respetando el formato `.properties` de Java.
//!
//! Se conservan comentarios, orden y líneas que no se tocan. Los valores se leen
//! desescapados (`é` → `é`) y se escriben escapados, como hace Minecraft.

use crate::AdminError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropEntry {
    pub key: String,
    pub value: String,
}

/// Una entrada lógica: clave, valor y las líneas físicas que ocupa (por continuaciones `\`).
#[derive(Debug, Clone)]
struct Logical {
    key: String,
    value: String,
    first: usize,
    last: usize,
}

#[derive(Debug, Clone)]
pub struct PropsDoc {
    lines: Vec<String>,
    entries: Vec<Logical>,
}

fn is_comment(line: &str) -> bool {
    let t = line.trim_start();
    t.is_empty() || t.starts_with('#') || t.starts_with('!')
}

/// ¿La línea termina en un número impar de barras invertidas? (continúa en la siguiente)
fn continues(line: &str) -> bool {
    line.chars().rev().take_while(|c| *c == '\\').count() % 2 == 1
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    let mut pending_high: Option<u16> = None;
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('f') => out.push('\u{c}'),
            Some('u') => {
                let hex: String = (0..4).filter_map(|_| it.next()).collect();
                match u16::from_str_radix(&hex, 16) {
                    Ok(u) if (0xD800..0xDC00).contains(&u) => pending_high = Some(u),
                    Ok(u) if (0xDC00..0xE000).contains(&u) => {
                        if let Some(h) = pending_high.take() {
                            if let Some(ch) = char::decode_utf16([h, u]).next().and_then(|r| r.ok()) {
                                out.push(ch);
                            }
                        }
                    }
                    Ok(u) => out.push(char::from_u32(u as u32).unwrap_or('\u{fffd}')),
                    Err(_) => {
                        out.push_str("\\u");
                        out.push_str(&hex);
                    }
                }
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// Escapa como `java.util.Properties.store`: no-ASCII como `\uXXXX`, `\`, y espacios iniciales.
pub fn escape_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for (i, ch) in v.chars().enumerate() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            ' ' if i == 0 => out.push_str("\\ "),
            c if c.is_ascii() && !c.is_ascii_control() => out.push(c),
            c => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{u:04x}"));
                }
            }
        }
    }
    out
}

/// Separa clave y valor de una línea lógica (`clave=valor`, `clave: valor` o `clave valor`).
fn split_kv(logical: &str) -> (String, String) {
    let s = logical.trim_start();
    let mut key_end = s.len();
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        if c == '=' || c == ':' || c.is_whitespace() {
            key_end = i;
            break;
        }
    }
    let key = unescape(&s[..key_end]);
    let mut rest = s[key_end..].trim_start_matches([' ', '\t', '\u{c}']);
    if let Some(r) = rest.strip_prefix(['=', ':']) {
        rest = r.trim_start_matches([' ', '\t', '\u{c}']);
    }
    (key, unescape(rest))
}

impl PropsDoc {
    pub fn parse(text: &str) -> Self {
        let lines: Vec<String> = text.lines().map(str::to_string).collect();
        let mut entries = vec![];
        let mut i = 0;
        while i < lines.len() {
            if is_comment(&lines[i]) {
                i += 1;
                continue;
            }
            let first = i;
            let mut logical = String::new();
            loop {
                let l = &lines[i];
                let piece = if i == first { l.as_str() } else { l.trim_start() };
                if continues(l) && i + 1 < lines.len() {
                    logical.push_str(&piece[..piece.len() - 1]);
                    i += 1;
                } else {
                    logical.push_str(piece);
                    break;
                }
            }
            let (key, value) = split_kv(&logical);
            entries.push(Logical {
                key,
                value,
                first,
                last: i,
            });
            i += 1;
        }
        Self { lines, entries }
    }

    /// Entradas en el orden del archivo (si una clave se repite, gana la última, como en Java).
    pub fn entries(&self) -> Vec<PropEntry> {
        let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
        let mut out: Vec<PropEntry> = vec![];
        for e in &self.entries {
            match seen.get(e.key.as_str()) {
                Some(&idx) => out[idx].value = e.value.clone(),
                None => {
                    seen.insert(&e.key, out.len());
                    out.push(PropEntry {
                        key: e.key.clone(),
                        value: e.value.clone(),
                    });
                }
            }
        }
        out
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|e| e.key == key)
            .map(|e| e.value.as_str())
    }

    /// Cambia (o añade al final) una clave. Las repeticiones anteriores se eliminan.
    pub fn set(&mut self, key: &str, value: &str) {
        let line = format!("{key}={}", escape_value(value));
        let positions: Vec<(usize, usize)> = self
            .entries
            .iter()
            .filter(|e| e.key == key)
            .map(|e| (e.first, e.last))
            .collect();
        match positions.split_last() {
            Some((&(first, last), earlier)) => {
                self.lines.splice(first..=last, [line]);
                for &(f, l) in earlier.iter().rev() {
                    self.lines.drain(f..=l);
                }
            }
            None => self.lines.push(line),
        }
        *self = Self::parse(&self.render());
    }

    pub fn render(&self) -> String {
        let mut s = self.lines.join("\n");
        s.push('\n');
        s
    }
}

/// Tipo de valor de las claves conocidas de Minecraft (Java Edition).
enum Kind {
    Bool,
    Int(i64, i64),
    Choice(&'static [&'static str]),
    Text(usize),
}

fn known(key: &str) -> Option<Kind> {
    use Kind::*;
    Some(match key {
        "allow-flight"
        | "allow-nether"
        | "enable-command-block"
        | "enforce-whitelist"
        | "hardcore"
        | "online-mode"
        | "pvp"
        | "spawn-monsters"
        | "spawn-animals"
        | "spawn-npcs"
        | "white-list"
        | "generate-structures"
        | "force-gamemode"
        | "enforce-secure-profile"
        | "hide-online-players"
        | "require-resource-pack"
        | "enable-status"
        | "enable-query"
        | "enable-rcon"
        | "prevent-proxy-connections"
        | "use-native-transport"
        | "sync-chunk-writes"
        | "broadcast-console-to-ops"
        | "broadcast-rcon-to-ops"
        | "log-ips"
        | "accepts-transfers"
        | "enable-jmx-monitoring" => Bool,
        "server-port" | "query.port" | "rcon.port" => Int(1, 65535),
        "max-players" => Int(0, 1_000_000),
        "view-distance" => Int(2, 32),
        "simulation-distance" => Int(2, 32),
        "spawn-protection" => Int(0, 1_000_000),
        "max-world-size" => Int(1, 29_999_984),
        "player-idle-timeout" => Int(0, 1_000_000),
        "op-permission-level" | "function-permission-level" => Int(0, 4),
        "entity-broadcast-range-percentage" => Int(10, 1000),
        "network-compression-threshold" => Int(-1, 1_000_000),
        "rate-limit" => Int(0, 1_000_000),
        "max-tick-time" => Int(-1, i64::MAX),
        "max-chained-neighbor-updates" => Int(-1, i64::MAX),
        "gamemode" => Choice(&["survival", "creative", "adventure", "spectator"]),
        "difficulty" => Choice(&["peaceful", "easy", "normal", "hard"]),
        "motd" => Text(1024),
        "level-seed" | "level-name" | "level-type" | "server-ip" => Text(256),
        _ => return None,
    })
}

/// Valida un cambio. Las claves desconocidas se aceptan como texto de una sola línea.
pub fn validate(key: &str, value: &str) -> Result<(), AdminError> {
    let bad = |why: &str| Err(AdminError::Invalid(format!("{key}: {why}")));
    if key.is_empty() || key.len() > 128 || !key.bytes().all(|b| b.is_ascii_graphic() && !b"=:#!\\".contains(&b)) {
        return bad("nombre de opción no válido");
    }
    // El MOTD admite dos líneas (Minecraft muestra solo dos en la lista de servidores).
    let newlines = value.matches('\n').count();
    if value.contains('\r') || (newlines > 0 && key != "motd") {
        return bad("no puede tener saltos de línea");
    }
    if newlines > 1 {
        return bad("como mucho dos líneas");
    }
    match known(key) {
        Some(Kind::Bool) if value != "true" && value != "false" => bad("debe ser true o false"),
        Some(Kind::Int(lo, hi)) => match value.parse::<i64>() {
            Ok(n) if (lo..=hi).contains(&n) => Ok(()),
            _ => bad(&format!("debe ser un número entre {lo} y {hi}")),
        },
        Some(Kind::Choice(opts)) if !opts.contains(&value) => bad(&format!("debe ser uno de: {}", opts.join(", "))),
        Some(Kind::Text(max)) if value.chars().count() > max => bad(&format!("máximo {max} caracteres")),
        None if value.chars().count() > 4096 => bad("demasiado largo"),
        _ => Ok(()),
    }
}

/// Las claves `level-name` y `server-port` afectan a qué mundo y puerto usa el servidor;
/// `level-name` además no puede salir de la carpeta del servidor.
pub fn validate_level_name(value: &str) -> Result<(), AdminError> {
    merc_core::paths::SandboxRoot::normalize_relative(value)
        .map(|_| ())
        .map_err(|e| AdminError::Invalid(format!("level-name: {e}")))
}

/// Aplica cambios validados a un texto `server.properties` y devuelve el texto nuevo.
pub fn apply_changes(text: &str, changes: &BTreeMap<String, String>) -> Result<String, AdminError> {
    if changes.is_empty() {
        return Err(AdminError::Invalid("no hay cambios que guardar".into()));
    }
    for (k, v) in changes {
        validate(k, v)?;
        if k == "level-name" {
            validate_level_name(v)?;
        }
    }
    let mut doc = PropsDoc::parse(text);
    for (k, v) in changes {
        doc.set(k, v);
    }
    Ok(doc.render())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "#Minecraft server properties\n#Thu Oct 30 2026\nmotd=Un servidor \\u00f1and\\u00fa\nserver-port=25565\n# nota\nwhite-list=false\nlevel-seed=\nmax-players=20\n";

    #[test]
    fn reads_unescaped_values_in_order() {
        let d = PropsDoc::parse(SAMPLE);
        let e = d.entries();
        assert_eq!(
            e[0],
            PropEntry {
                key: "motd".into(),
                value: "Un servidor ñandú".into()
            }
        );
        assert_eq!(
            e.iter().map(|x| x.key.as_str()).collect::<Vec<_>>(),
            ["motd", "server-port", "white-list", "level-seed", "max-players"]
        );
        assert_eq!(d.get("level-seed"), Some(""));
    }

    #[test]
    fn set_keeps_comments_and_other_lines() {
        let mut d = PropsDoc::parse(SAMPLE);
        d.set("max-players", "8");
        d.set("motd", "Hola 🎮");
        d.set("pvp", "false");
        let out = d.render();
        assert!(out.starts_with("#Minecraft server properties\n#Thu Oct 30 2026\nmotd=Hola \\ud83c\\udfae\n"));
        assert!(out.contains("# nota\nwhite-list=false\n"));
        assert!(out.contains("max-players=8\n"));
        assert!(out.ends_with("pvp=false\n"));
        assert_eq!(PropsDoc::parse(&out).get("motd"), Some("Hola 🎮"));
    }

    #[test]
    fn continuation_and_separators() {
        let d = PropsDoc::parse("a = uno \\\n    dos\nb:tres\nc cuatro\nd\\=x=5\n");
        assert_eq!(d.get("a"), Some("uno dos"));
        assert_eq!(d.get("b"), Some("tres"));
        assert_eq!(d.get("c"), Some("cuatro"));
        assert_eq!(d.get("d=x"), Some("5"));
        let mut d = d;
        d.set("a", "nuevo");
        assert_eq!(d.render(), "a=nuevo\nb:tres\nc cuatro\nd\\=x=5\n");
    }

    #[test]
    fn duplicates_collapse_to_last() {
        let mut d = PropsDoc::parse("x=1\ny=2\nx=3\n");
        assert_eq!(d.get("x"), Some("3"));
        d.set("x", "4");
        assert_eq!(d.render(), "y=2\nx=4\n");
    }

    #[test]
    fn motd_two_lines_round_trip() {
        let mut ch = BTreeMap::new();
        ch.insert("motd".to_string(), "\u{a7}6\u{a7}lHola\n\u{a7}7Línea dos".to_string());
        let out = apply_changes(SAMPLE, &ch).unwrap();
        assert!(
            out.contains("motd=\\u00a76\\u00a7lHola\\n\\u00a77L\\u00ednea dos\n"),
            "{out}"
        );
        assert_eq!(
            PropsDoc::parse(&out).get("motd"),
            Some("\u{a7}6\u{a7}lHola\n\u{a7}7Línea dos")
        );
        assert!(validate("motd", "a\nb\nc").is_err());
        assert!(validate("level-name", "a\nb").is_err());
    }

    #[test]
    fn validation() {
        assert!(validate("server-port", "25566").is_ok());
        assert!(validate("server-port", "70000").is_err());
        assert!(validate("pvp", "si").is_err());
        assert!(validate("difficulty", "hard").is_ok());
        assert!(validate("server-ip", "a\nb").is_err());
        assert!(validate("mod-custom-thing", "lo que sea").is_ok());
        assert!(validate("bad=key", "1").is_err());
        assert!(validate_level_name("world").is_ok());
        assert!(validate_level_name("../fuera").is_err());
    }
}
