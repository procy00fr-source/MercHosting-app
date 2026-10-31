//! Salud y alertas: tipos de las alertas, su archivo, sus ajustes y la lectura del TPS que
//! informan los propios servidores (Paper, Spigot, Purpur, Forge y NeoForge).
//!
//! Regla: solo se muestra lo medido. Si un servidor no informa TPS (Vanilla, Fabric), se dice
//! que no está disponible; no se calcula ni se estima.

use merc_core::detect::ServerKind;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Alertas que se guardan (las más viejas se descartan).
pub const MAX_ALERTS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertKind {
    /// El servidor se cerró sin que se lo pidieran.
    Crash,
    /// Poco espacio en el disco de un servidor o de las copias.
    LowDisk,
    /// La memoria de la PC está casi llena.
    HighMemory,
    /// Una tarea programada (o su copia) falló.
    TaskFailed,
    /// Un compañero se unió (o pasó algo con el acceso de compañeros).
    Companion,
    /// El túnel de Playit dejó de funcionar (o volvió) con un servidor encendido.
    TunnelDown,
    /// Falló una copia de seguridad automática (programada o antes de actualizar/restaurar).
    BackupFailed,
    /// Hay una versión más nueva del software del servidor (misma versión de Minecraft).
    UpdateAvailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub at_unix_ms: i64,
    pub level: AlertLevel,
    pub kind: AlertKind,
    pub instance_id: Option<String>,
    pub server_name: Option<String>,
    pub title: String,
    pub message: String,
    pub read: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertSettings {
    /// Mostrar notificaciones de Windows (si la ventana de MercHosting está abierta o minimizada).
    pub notify: bool,
    pub crash: bool,
    pub disk: bool,
    pub resources: bool,
    pub tasks: bool,
    /// Avisar cuando queden menos de estos GB libres.
    pub disk_min_gb: u32,
    #[serde(default = "yes")]
    pub tunnel: bool,
    #[serde(default = "yes")]
    pub backups: bool,
    #[serde(default = "yes")]
    pub updates: bool,
}

fn yes() -> bool {
    true
}

impl Default for AlertSettings {
    fn default() -> Self {
        Self {
            notify: true,
            crash: true,
            disk: true,
            resources: true,
            tasks: true,
            disk_min_gb: 5,
            tunnel: true,
            backups: true,
            updates: true,
        }
    }
}

impl AlertSettings {
    pub fn wants(&self, kind: AlertKind) -> bool {
        match kind {
            AlertKind::Crash => self.crash,
            AlertKind::LowDisk => self.disk,
            AlertKind::HighMemory => self.resources,
            AlertKind::TaskFailed => self.tasks,
            AlertKind::Companion => true,
            AlertKind::TunnelDown => self.tunnel,
            AlertKind::BackupFailed => self.backups,
            AlertKind::UpdateAvailable => self.updates,
        }
    }
}

pub fn load_alerts(path: &Path) -> Vec<Alert> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save_alerts(path: &Path, list: &[Alert]) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(list).unwrap_or_default())?;
    std::fs::rename(&tmp, path)
}

/// Añade una alerta al principio y recorta la lista.
pub fn push_alert(list: &mut Vec<Alert>, alert: Alert) {
    list.insert(0, alert);
    list.truncate(MAX_ALERTS);
}

/// Comando que hace que el servidor informe su TPS, si lo tiene.
pub fn tps_command(kind: ServerKind) -> Option<&'static str> {
    match kind {
        ServerKind::Paper | ServerKind::Purpur | ServerKind::Spigot | ServerKind::Mohist | ServerKind::Arclight => {
            Some("tps")
        }
        ServerKind::NeoForge => Some("neoforge tps"),
        ServerKind::Forge => Some("forge tps"),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TpsReading {
    /// Pares (periodo, TPS) tal como los informa el servidor: «1m», «5m», «15m» o «media».
    pub tps: Vec<(String, f64)>,
    /// Milisegundos por tick, si el servidor lo informa.
    pub mspt: Option<f64>,
    /// Comando usado.
    pub command: String,
    /// Línea original de la consola.
    pub raw: String,
}

/// Quita los códigos de color de Minecraft (`§a`) y los asteriscos que Paper pone a más de 20.
fn clean(line: &str) -> String {
    let mut out = String::new();
    let mut skip = false;
    for c in line.chars() {
        if skip {
            skip = false;
            continue;
        }
        if c == '§' {
            skip = true;
            continue;
        }
        if c != '*' {
            out.push(c);
        }
    }
    out
}

fn num(s: &str) -> Option<f64> {
    s.trim()
        .trim_end_matches(',')
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

/// Lee el TPS de una línea de consola, si la hay.
/// - Paper/Spigot: `TPS from last 1m, 5m, 15m: 20.0, 20.0, 20.0` (Purpur añade `5s`).
/// - Forge: `Overall : Mean tick time: 1.234 ms. Mean TPS: 20.000`.
/// - NeoForge: `Overall: 20.000 TPS (1.234 ms/tick)`.
pub fn parse_tps(line: &str, command: &str) -> Option<TpsReading> {
    let l = clean(line);
    if let Some(i) = l.find("TPS from last ") {
        let rest = &l[i + "TPS from last ".len()..];
        let (labels, values) = rest.split_once(':')?;
        let labels: Vec<String> = labels.split(',').map(|s| s.trim().to_string()).collect();
        let values: Vec<f64> = values.split(',').filter_map(num).collect();
        if labels.is_empty() || labels.len() != values.len() {
            return None;
        }
        return Some(TpsReading {
            tps: labels.into_iter().zip(values).collect(),
            mspt: None,
            command: command.into(),
            raw: l.trim().to_string(),
        });
    }
    if l.contains("Overall") {
        let re_mean = regex::Regex::new(r"Mean TPS:\s*([0-9.]+)").ok()?;
        let re_tps = regex::Regex::new(r"([0-9.]+)\s*TPS").ok()?;
        let re_ms = regex::Regex::new(r"([0-9.]+)\s*ms").ok()?;
        let tps = re_mean
            .captures(&l)
            .or_else(|| re_tps.captures(&l))
            .and_then(|c| num(&c[1]))?;
        let mspt = re_ms.captures(&l).and_then(|c| num(&c[1]));
        return Some(TpsReading {
            tps: vec![("media".into(), tps)],
            mspt,
            command: command.into(),
            raw: l.trim().to_string(),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tps_lines() {
        let p = parse_tps(
            "[12:00:00 INFO]: §6TPS from last 1m, 5m, 15m: §a*20.0, §a19.85, §e17.2",
            "tps",
        )
        .unwrap();
        assert_eq!(
            p.tps,
            vec![("1m".into(), 20.0), ("5m".into(), 19.85), ("15m".into(), 17.2)]
        );
        let purpur = parse_tps("TPS from last 5s, 1m, 5m, 15m: 20.0, 20.0, 20.0, 19.9", "tps").unwrap();
        assert_eq!(purpur.tps.len(), 4);
        let forge = parse_tps(
            "[Server thread/INFO]: Overall : Mean tick time: 3.456 ms. Mean TPS: 20.000",
            "forge tps",
        )
        .unwrap();
        assert_eq!(forge.tps, vec![("media".into(), 20.0)]);
        assert_eq!(forge.mspt, Some(3.456));
        let neo = parse_tps("Overall: 18.500 TPS (54.054 ms/tick)", "neoforge tps").unwrap();
        assert_eq!((neo.tps[0].1, neo.mspt), (18.5, Some(54.054)));
        assert!(parse_tps("Done (1.2s)! For help, type \"help\"", "tps").is_none());
        assert!(parse_tps("TPS from last 1m, 5m: 20.0", "tps").is_none());
        assert_eq!(tps_command(ServerKind::Vanilla), None);
        assert_eq!(tps_command(ServerKind::Fabric), None);
        assert_eq!(tps_command(ServerKind::Purpur), Some("tps"));
    }

    #[test]
    fn alert_store_is_capped() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("alertas.json");
        let mut list = load_alerts(&p);
        for i in 0..(MAX_ALERTS + 5) {
            push_alert(
                &mut list,
                Alert {
                    id: format!("a{i}"),
                    at_unix_ms: i as i64,
                    level: AlertLevel::Warning,
                    kind: AlertKind::LowDisk,
                    instance_id: None,
                    server_name: None,
                    title: "t".into(),
                    message: "m".into(),
                    read: false,
                },
            );
        }
        save_alerts(&p, &list).unwrap();
        let back = load_alerts(&p);
        assert_eq!(back.len(), MAX_ALERTS);
        assert_eq!(back[0].id, format!("a{}", MAX_ALERTS + 4));
        assert!(AlertSettings::default().wants(AlertKind::Crash));
        // Ajustes guardados antes de que existieran las alertas nuevas: se leen con ellas activadas.
        let old: AlertSettings = serde_json::from_str(
            r#"{"notify":true,"crash":false,"disk":true,"resources":true,"tasks":true,"disk_min_gb":5}"#,
        )
        .unwrap();
        assert!(!old.crash && old.tunnel && old.backups && old.updates);
        let off = AlertSettings {
            updates: false,
            ..Default::default()
        };
        assert!(!off.wants(AlertKind::UpdateAvailable) && off.wants(AlertKind::TunnelDown));
    }
}
