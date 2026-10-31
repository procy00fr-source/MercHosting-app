//! Tareas programadas de un servidor: qué hacer, cuándo y qué pasó la última vez.
//!
//! Las horas son las del reloj de esta PC. El cálculo de la próxima ejecución es puro (recibe
//! «ahora») para poder probarlo; quien ejecuta es el supervisor.

use crate::backups::BackupScope;
use crate::AdminError;
use chrono::{DateTime, Datelike, Duration, NaiveTime, TimeZone, Timelike};
use serde::{Deserialize, Serialize};

pub const MAX_TASKS: usize = 50;
const MAX_TIMES: usize = 24;
/// Intervalo mínimo entre ejecuciones de una tarea «cada…».
pub const MIN_EVERY_MINUTES: u32 = 5;
const MAX_EVERY_MINUTES: u32 = 7 * 24 * 60;
pub const MAX_NOTICE_SECONDS: u32 = 600;
/// Si la hora ya pasó hace más que esto (PC apagada o dormida), no se ejecuta tarde: se omite.
pub const LATE_TOLERANCE_MS: i64 = 10 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Schedule {
    /// A ciertas horas (`HH:MM`) de ciertos días (1 = lunes … 7 = domingo; vacío = todos).
    Daily { times: Vec<String>, days: Vec<u8> },
    /// Cada N minutos.
    Every { minutes: u32 },
}

/// Qué hacer si hay jugadores conectados a la hora de reiniciar o apagar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlayersPolicy {
    /// No hacerlo esta vez.
    #[default]
    Skip,
    /// Avisar en el chat con la cuenta atrás y hacerlo igual.
    Notify,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TaskAction {
    Backup {
        scope: BackupScope,
    },
    Restart {
        notice_seconds: u32,
        #[serde(default)]
        if_players: PlayersPolicy,
    },
    Stop {
        notice_seconds: u32,
        #[serde(default)]
        if_players: PlayersPolicy,
    },
    Start,
    /// Un comando de consola de Minecraft (sin la barra).
    Command {
        command: String,
    },
    /// Un mensaje en el chat (`say`).
    Announce {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRun {
    pub at_unix_ms: i64,
    /// `ok`, `skipped` (no hacía falta o la política lo impidió) o `failed`.
    pub outcome: RunOutcome,
    pub message: String,
    /// La lanzó una persona con «Ejecutar ahora».
    #[serde(default)]
    pub manual: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunOutcome {
    Ok,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: String,
    pub enabled: bool,
    pub action: TaskAction,
    pub schedule: Schedule,
    pub created_unix_ms: i64,
    pub next_run_unix_ms: Option<i64>,
    pub last_run: Option<TaskRun>,
    /// Se está ejecutando ahora mismo.
    #[serde(default)]
    pub running: bool,
}

fn parse_time(t: &str) -> Option<NaiveTime> {
    let (h, m) = t.trim().split_once(':')?;
    if h.len() > 2 || m.len() != 2 {
        return None;
    }
    NaiveTime::from_hms_opt(h.parse().ok()?, m.parse().ok()?, 0)
}

/// Comprueba y normaliza (horas ordenadas `HH:MM`, días ordenados sin repetir).
pub fn normalize_schedule(s: &Schedule) -> Result<Schedule, AdminError> {
    let bad = |m: &str| Err(AdminError::Invalid(m.to_string()));
    match s {
        Schedule::Daily { times, days } => {
            if times.is_empty() {
                return bad("Elige al menos una hora.");
            }
            if times.len() > MAX_TIMES {
                return bad("Demasiadas horas: como mucho 24.");
            }
            let mut ts = vec![];
            for t in times {
                match parse_time(t) {
                    Some(n) => ts.push(n.format("%H:%M").to_string()),
                    None => return Err(AdminError::Invalid(format!("Hora no válida: «{t}» (usa HH:MM)."))),
                }
            }
            ts.sort();
            ts.dedup();
            let mut ds = days.clone();
            if ds.iter().any(|d| !(1..=7).contains(d)) {
                return bad("Día no válido.");
            }
            ds.sort();
            ds.dedup();
            if ds.len() == 7 {
                ds.clear();
            }
            Ok(Schedule::Daily { times: ts, days: ds })
        }
        Schedule::Every { minutes } => {
            if !(MIN_EVERY_MINUTES..=MAX_EVERY_MINUTES).contains(minutes) {
                return Err(AdminError::Invalid(format!(
                    "El intervalo tiene que estar entre {MIN_EVERY_MINUTES} minutos y 7 días."
                )));
            }
            Ok(s.clone())
        }
    }
}

pub fn normalize_action(a: &TaskAction) -> Result<TaskAction, AdminError> {
    let text = |t: &str, max: usize, what: &str| -> Result<String, AdminError> {
        let t = t.trim().trim_start_matches('/').trim().to_string();
        if t.is_empty() {
            return Err(AdminError::Invalid(format!("Escribe {what}.")));
        }
        if t.chars().count() > max || t.chars().any(char::is_control) {
            return Err(AdminError::Invalid(format!(
                "{what}: como mucho {max} caracteres y en una sola línea."
            )));
        }
        Ok(t)
    };
    Ok(match a {
        TaskAction::Command { command } => {
            let c = text(command, 256, "el comando")?;
            let first = c.split_whitespace().next().unwrap_or("").to_lowercase();
            // Apagar o cambiar el guardado se hace con sus propias tareas, no a ciegas.
            if ["stop", "save-off", "save-on", "end", "restart"].contains(&first.as_str()) {
                return Err(AdminError::Invalid(format!(
                    "«{first}» no se programa como comando: usa las tareas de apagar, reiniciar o copia."
                )));
            }
            TaskAction::Command { command: c }
        }
        TaskAction::Announce { message } => TaskAction::Announce {
            message: text(message, 200, "el mensaje")?,
        },
        TaskAction::Restart {
            notice_seconds,
            if_players,
        } => TaskAction::Restart {
            notice_seconds: (*notice_seconds).min(MAX_NOTICE_SECONDS),
            if_players: *if_players,
        },
        TaskAction::Stop {
            notice_seconds,
            if_players,
        } => TaskAction::Stop {
            notice_seconds: (*notice_seconds).min(MAX_NOTICE_SECONDS),
            if_players: *if_players,
        },
        other => other.clone(),
    })
}

/// Próxima ejecución estrictamente después de `after`.
pub fn next_after<Tz: TimeZone>(s: &Schedule, after: &DateTime<Tz>) -> Option<DateTime<Tz>> {
    match s {
        Schedule::Every { minutes } => Some(after.clone() + Duration::minutes((*minutes).max(1) as i64)),
        Schedule::Daily { times, days } => {
            let times: Vec<NaiveTime> = times.iter().filter_map(|t| parse_time(t)).collect();
            if times.is_empty() {
                return None;
            }
            let tz = after.timezone();
            let start = after.date_naive();
            for offset in 0..=8 {
                let date = start + Duration::days(offset);
                let wd = date.weekday().number_from_monday() as u8;
                if !days.is_empty() && !days.contains(&wd) {
                    continue;
                }
                let mut best: Option<DateTime<Tz>> = None;
                for t in &times {
                    // En un cambio de hora, una hora que no existe se salta y una repetida usa la primera.
                    let Some(dt) = tz.from_local_datetime(&date.and_time(*t)).earliest() else {
                        continue;
                    };
                    if dt > *after && best.as_ref().is_none_or(|b| dt < *b) {
                        best = Some(dt);
                    }
                }
                if best.is_some() {
                    return best;
                }
            }
            None
        }
    }
}

/// Descripción corta de cuándo se ejecuta, para el registro de Actividad.
pub fn describe(s: &Schedule) -> String {
    match s {
        Schedule::Every { minutes } if minutes % 60 == 0 => format!("cada {} h", minutes / 60),
        Schedule::Every { minutes } => format!("cada {minutes} min"),
        Schedule::Daily { times, days } => {
            const D: [&str; 7] = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"];
            let when = if days.is_empty() {
                "todos los días".to_string()
            } else {
                days.iter()
                    .map(|d| D[(*d as usize).clamp(1, 7) - 1])
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            format!("{when} a las {}", times.join(", "))
        }
    }
}

/// Qué toca hacer con una tarea en este momento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Due {
    NotYet,
    Run,
    /// La hora pasó hace demasiado (la PC estaba apagada o dormida): se omite esta vez.
    Missed,
}

pub fn due(task: &ScheduledTask, now_ms: i64) -> Due {
    match task.next_run_unix_ms {
        Some(n) if task.enabled && !task.running && now_ms >= n => {
            if now_ms - n > LATE_TOLERANCE_MS {
                Due::Missed
            } else {
                Due::Run
            }
        }
        _ => Due::NotYet,
    }
}

/// Minuto exacto (sin segundos) para comparar horas.
pub fn minute_of<Tz: TimeZone>(d: &DateTime<Tz>) -> (u32, u32) {
    (d.hour(), d.minute())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    fn lima(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<FixedOffset> {
        FixedOffset::west_opt(5 * 3600)
            .unwrap()
            .with_ymd_and_hms(y, mo, d, h, mi, 0)
            .unwrap()
    }

    #[test]
    fn daily_next_run() {
        let s = normalize_schedule(&Schedule::Daily {
            times: vec!["4:00".into(), "16:30".into(), "04:00".into()],
            days: vec![],
        })
        .unwrap();
        assert_eq!(
            s,
            Schedule::Daily {
                times: vec!["04:00".into(), "16:30".into()],
                days: vec![]
            }
        );
        // Viernes 30 de octubre de 2026, 13:31 en Lima.
        let now = lima(2026, 10, 30, 13, 31);
        assert_eq!(next_after(&s, &now).unwrap(), lima(2026, 10, 30, 16, 30));
        assert_eq!(
            next_after(&s, &lima(2026, 10, 30, 16, 30)).unwrap(),
            lima(2026, 10, 31, 4, 0)
        );
        // Solo lunes y miércoles: el próximo es el lunes 2 de noviembre.
        let s2 = normalize_schedule(&Schedule::Daily {
            times: vec!["03:15".into()],
            days: vec![3, 1, 1],
        })
        .unwrap();
        assert_eq!(next_after(&s2, &now).unwrap(), lima(2026, 11, 2, 3, 15));
        assert_eq!(describe(&s2), "lun, mié a las 03:15");
        assert_eq!(minute_of(&lima(2026, 11, 2, 3, 15)), (3, 15));
        // Los 7 días equivalen a «todos».
        let all = normalize_schedule(&Schedule::Daily {
            times: vec!["12:00".into()],
            days: vec![1, 2, 3, 4, 5, 6, 7],
        })
        .unwrap();
        assert_eq!(describe(&all), "todos los días a las 12:00");
    }

    #[test]
    fn every_and_validation() {
        let s = Schedule::Every { minutes: 90 };
        let now = lima(2026, 10, 30, 23, 0);
        assert_eq!(next_after(&s, &now).unwrap(), lima(2026, 10, 31, 0, 30));
        assert_eq!(describe(&Schedule::Every { minutes: 120 }), "cada 2 h");
        assert!(normalize_schedule(&Schedule::Every { minutes: 1 }).is_err());
        for bad in ["25:00", "12:60", "1200", "12:5", "x"] {
            assert!(
                normalize_schedule(&Schedule::Daily {
                    times: vec![bad.into()],
                    days: vec![]
                })
                .is_err(),
                "{bad}"
            );
        }
        assert!(normalize_schedule(&Schedule::Daily {
            times: vec!["10:00".into()],
            days: vec![8]
        })
        .is_err());
        assert_eq!(
            normalize_action(&TaskAction::Command {
                command: " /weather clear ".into()
            })
            .unwrap(),
            TaskAction::Command {
                command: "weather clear".into()
            }
        );
        for bad in ["stop", "save-off", "", "say a\nop yo"] {
            assert!(
                normalize_action(&TaskAction::Command { command: bad.into() }).is_err(),
                "{bad:?}"
            );
        }
        assert_eq!(
            normalize_action(&TaskAction::Restart {
                notice_seconds: 9999,
                if_players: PlayersPolicy::Notify
            })
            .unwrap(),
            TaskAction::Restart {
                notice_seconds: 600,
                if_players: PlayersPolicy::Notify
            }
        );
    }

    #[test]
    fn due_and_missed() {
        let mut t = ScheduledTask {
            id: "t1".into(),
            enabled: true,
            action: TaskAction::Start,
            schedule: Schedule::Every { minutes: 30 },
            created_unix_ms: 0,
            next_run_unix_ms: Some(1_000_000),
            last_run: None,
            running: false,
        };
        assert_eq!(due(&t, 999_999), Due::NotYet);
        assert_eq!(due(&t, 1_000_000), Due::Run);
        assert_eq!(due(&t, 1_000_000 + LATE_TOLERANCE_MS + 1), Due::Missed);
        t.enabled = false;
        assert_eq!(due(&t, 1_000_000), Due::NotYet);
    }
}
