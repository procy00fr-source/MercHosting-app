//! Tareas programadas: se guardan por servidor, el programador las revisa cada pocos segundos y
//! cada ejecución queda en su `last_run` y en Actividad.
//!
//! Reglas:
//! - Las horas son las del reloj de esta PC.
//! - Una tarea no se solapa consigo misma.
//! - Si la PC o el supervisor estaban apagados a la hora, no se ejecuta tarde: se anota como
//!   omitida y se espera a la siguiente.
//! - Reiniciar o apagar con jugadores conectados solo si la tarea lo permite («avisar y seguir»);
//!   si no, se omite esa vez.

use super::{admin_err, api, Daemon};
use chrono::{Local, Offset, TimeZone};
use merc_admin::backups::BackupReason;
use merc_admin::schedule::{self, Due, PlayersPolicy, RunOutcome, ScheduledTask, TaskAction, TaskRun, MAX_TASKS};
use merc_core::{Instance, InstanceId, RuntimeState};
use merc_ipc::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const ACTOR: &str = "programador";
/// Aviso mínimo cuando hay jugadores y la tarea dice «avisar y seguir».
const MIN_NOTICE_WITH_PLAYERS: u32 = 30;

fn now_ms() -> i64 {
    Local::now().timestamp_millis()
}

fn next_from(s: &schedule::Schedule, after_ms: i64) -> Option<i64> {
    let after = Local.timestamp_millis_opt(after_ms).single()?;
    schedule::next_after(s, &after).map(|d| d.timestamp_millis())
}

fn hhmm(ms: i64) -> String {
    Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|d| d.format("%d/%m %H:%M").to_string())
        .unwrap_or_default()
}

pub(crate) fn describe_action(a: &TaskAction) -> String {
    match a {
        TaskAction::Backup { scope } => match scope {
            BackupScope::World => "copia del mundo".into(),
            BackupScope::Full => "copia completa".into(),
        },
        TaskAction::Restart { notice_seconds, .. } => format!("reiniciar (aviso {notice_seconds} s)"),
        TaskAction::Stop { notice_seconds, .. } => format!("apagar (aviso {notice_seconds} s)"),
        TaskAction::Start => "encender".into(),
        TaskAction::Command { command } => format!("comando «{command}»"),
        TaskAction::Announce { message } => format!("mensaje «{message}»"),
    }
}

impl Daemon {
    fn tasks_file(&self, id: InstanceId) -> PathBuf {
        self.store_dir(id).join("tareas.json")
    }

    pub(crate) fn load_tasks(&self, id: InstanceId) -> Vec<ScheduledTask> {
        let mut list: Vec<ScheduledTask> = std::fs::read_to_string(self.tasks_file(id))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        let running = self.running_tasks.lock().unwrap_or_else(|p| p.into_inner());
        for t in &mut list {
            t.running = running.contains(&(id, t.id.clone()));
        }
        list
    }

    fn save_tasks(&self, id: InstanceId, list: &[ScheduledTask]) -> Result<(), ApiError> {
        let f = self.tasks_file(id);
        if let Some(p) = f.parent() {
            std::fs::create_dir_all(p).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        }
        let tmp = f.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(list).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        std::fs::write(&tmp, text).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        std::fs::rename(&tmp, &f).map_err(|e| api(ErrorCode::Internal, e.to_string()))
    }

    fn task_list(&self, id: InstanceId) -> TaskList {
        let now = Local::now();
        TaskList {
            items: self.load_tasks(id),
            now_unix_ms: now.timestamp_millis(),
            utc_offset_minutes: now.offset().fix().local_minus_utc() / 60,
        }
    }

    /// Bucle del programador: revisa todas las tareas cada pocos segundos.
    pub(crate) fn scheduler_loop(self: Arc<Self>) {
        let mut tick = Duration::from_secs(15);
        if cfg!(debug_assertions) {
            if let Some(ms) = std::env::var("MERC_SCHEDULER_TICK_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
            {
                tick = Duration::from_millis(ms.max(50));
            }
        }
        loop {
            self.scheduler_tick();
            std::thread::sleep(tick);
        }
    }

    fn scheduler_tick(self: &Arc<Self>) {
        let instances = self
            .store
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .list_instances()
            .unwrap_or_default();
        for inst in instances {
            if !self.tasks_file(inst.id).exists() {
                continue;
            }
            let mut to_run = vec![];
            {
                let _g = self.tasks_lock.lock().unwrap_or_else(|p| p.into_inner());
                let mut list = self.load_tasks(inst.id);
                let now = now_ms();
                let mut changed = false;
                for t in &mut list {
                    if t.enabled && t.next_run_unix_ms.is_none() {
                        t.next_run_unix_ms = next_from(&t.schedule, now);
                        changed = true;
                    }
                    match schedule::due(t, now) {
                        Due::NotYet => {}
                        Due::Missed => {
                            let at = t.next_run_unix_ms.unwrap_or(now);
                            let msg = format!(
                                "No se ejecutó a las {}: la PC o el supervisor no estaban en marcha a esa hora.",
                                hhmm(at)
                            );
                            t.last_run = Some(TaskRun {
                                at_unix_ms: now,
                                outcome: RunOutcome::Skipped,
                                message: msg.clone(),
                                manual: false,
                            });
                            t.next_run_unix_ms = next_from(&t.schedule, now);
                            self.audit(
                                ACTOR,
                                Some(inst.id),
                                "task.missed",
                                &format!("{}: {msg}", describe_action(&t.action)),
                            );
                            changed = true;
                        }
                        Due::Run => {
                            t.next_run_unix_ms = next_from(&t.schedule, now);
                            changed = true;
                            to_run.push(t.clone());
                        }
                    }
                }
                if changed {
                    if let Err(e) = self.save_tasks(inst.id, &list) {
                        self.log
                            .line(&format!("tareas de {}: no se pudieron guardar: {}", inst.id, e.message));
                        continue;
                    }
                }
            }
            for t in to_run {
                self.spawn_task(inst.clone(), t, false);
            }
        }
    }

    /// Ejecuta la tarea en su propio hilo (sin solaparse consigo misma).
    fn spawn_task(self: &Arc<Self>, inst: Instance, task: ScheduledTask, manual: bool) -> bool {
        let key = (inst.id, task.id.clone());
        if !self
            .running_tasks
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(key.clone())
        {
            return false;
        }
        let d = self.clone();
        std::thread::spawn(move || {
            let (outcome, message) = match d.execute(&inst, &task.action) {
                Ok((o, m)) => (o, m),
                Err(e) => (RunOutcome::Failed, e.message),
            };
            let run = TaskRun {
                at_unix_ms: now_ms(),
                outcome,
                message: message.clone(),
                manual,
            };
            {
                let _g = d.tasks_lock.lock().unwrap_or_else(|p| p.into_inner());
                let mut list = d.load_tasks(inst.id);
                if let Some(t) = list.iter_mut().find(|t| t.id == task.id) {
                    t.last_run = Some(run);
                    let _ = d.save_tasks(inst.id, &list);
                }
            }
            d.running_tasks.lock().unwrap_or_else(|p| p.into_inner()).remove(&key);
            // Una copia fallida ya avisa con su propia alerta (BackupFailed).
            if outcome == RunOutcome::Failed && !matches!(task.action, TaskAction::Backup { .. }) {
                d.push_alert(
                    merc_admin::health::AlertKind::TaskFailed,
                    merc_admin::health::AlertLevel::Warning,
                    Some((inst.id, inst.name.clone())),
                    format!("Falló una tarea de «{}»: {}", inst.name, describe_action(&task.action)),
                    message.clone(),
                );
            }
            d.audit(
                if manual { "programador (ejecutar ahora)" } else { ACTOR },
                Some(inst.id),
                match outcome {
                    RunOutcome::Ok => "task.run",
                    RunOutcome::Skipped => "task.skipped",
                    RunOutcome::Failed => "task.failed",
                },
                &format!("{}: {message}", describe_action(&task.action)),
            );
        });
        true
    }

    /// Hace lo que pide la tarea. `Ok` también para «omitida» (con su motivo).
    fn execute(self: &Arc<Self>, inst: &Instance, action: &TaskAction) -> Result<(RunOutcome, String), ApiError> {
        let id = inst.id;
        let st = self.sup.status(id);
        let online = st.state == RuntimeState::Online && st.has_console;
        let skip = |m: &str| Ok((RunOutcome::Skipped, m.to_string()));
        match action {
            TaskAction::Backup { scope } => {
                let info = self.make_backup(
                    ACTOR,
                    inst,
                    *scope,
                    BackupReason::Scheduled,
                    Some("Tarea programada".into()),
                )?;
                Ok((
                    RunOutcome::Ok,
                    if info.hot && !info.consistent {
                        "Copia hecha, pero el juego no confirmó que guardó.".into()
                    } else {
                        format!("Copia hecha y comprobada ({} archivos).", info.files)
                    },
                ))
            }
            TaskAction::Start => {
                if st.state.has_process() {
                    return skip("Ya estaba encendido.");
                }
                if self.world_op_running(id) {
                    return Err(api(
                        ErrorCode::Conflict,
                        "Había una operación de mundos o copias en marcha.",
                    ));
                }
                self.require_not_archived(inst.id)?;
                self.sup.start(Self::spec(inst)).map_err(super::sup_err)?;
                Ok((RunOutcome::Ok, "Encendido.".into()))
            }
            TaskAction::Restart {
                notice_seconds,
                if_players,
            }
            | TaskAction::Stop {
                notice_seconds,
                if_players,
            } => {
                let stop_only = matches!(action, TaskAction::Stop { .. });
                if !st.state.has_process() {
                    return skip("El servidor está apagado.");
                }
                if !online {
                    return skip("El servidor no está en línea (encendiéndose, apagándose o sin consola).");
                }
                let players = self.sup.metrics(id).1;
                if !players.is_empty() && *if_players == PlayersPolicy::Skip {
                    return Ok((
                        RunOutcome::Skipped,
                        format!(
                            "Había {} jugador(es) conectado(s) ({}): esta tarea no {} con gente jugando.",
                            players.len(),
                            players.join(", "),
                            if stop_only { "apaga" } else { "reinicia" }
                        ),
                    ));
                }
                let notice = if players.is_empty() {
                    *notice_seconds
                } else {
                    (*notice_seconds).max(MIN_NOTICE_WITH_PLAYERS)
                };
                if notice == 0 {
                    if stop_only {
                        self.sup.stop(id, Duration::from_secs(180)).map_err(super::sup_err)?;
                        Ok((RunOutcome::Ok, "Apagado.".into()))
                    } else {
                        self.sup
                            .restart(Self::spec(inst), Duration::from_secs(180))
                            .map_err(super::sup_err)?;
                        Ok((RunOutcome::Ok, "Reiniciado.".into()))
                    }
                } else {
                    self.countdown(ACTOR, inst.clone(), notice, stop_only)?;
                    Ok((
                        RunOutcome::Ok,
                        format!(
                            "Aviso en el chat y {} en {} s.",
                            if stop_only { "apagado" } else { "reinicio" },
                            notice.clamp(10, schedule::MAX_NOTICE_SECONDS)
                        ),
                    ))
                }
            }
            TaskAction::Command { command } => {
                if !online {
                    return skip("El servidor no está en línea.");
                }
                self.sup.send_command(id, command).map_err(super::sup_err)?;
                Ok((RunOutcome::Ok, format!("Comando enviado: {command}")))
            }
            TaskAction::Announce { message } => {
                if !online {
                    return skip("El servidor no está en línea.");
                }
                self.sup
                    .send_command(id, &format!("say {message}"))
                    .map_err(super::sup_err)?;
                Ok((RunOutcome::Ok, "Mensaje enviado al chat.".into()))
            }
        }
    }

    pub(crate) fn handle_tasks(self: &Arc<Self>, client: &str, body: RequestBody) -> Result<ResponseBody, ApiError> {
        use RequestBody as R;
        Ok(match body {
            R::Tasks { id } => {
                self.instance(id)?;
                ResponseBody::Tasks(self.task_list(id))
            }
            R::SaveTask { id, task } => {
                self.instance(id)?;
                let action = schedule::normalize_action(&task.action).map_err(admin_err)?;
                let sched = schedule::normalize_schedule(&task.schedule).map_err(admin_err)?;
                let now = now_ms();
                let _g = self.tasks_lock.lock().unwrap_or_else(|p| p.into_inner());
                let mut list = self.load_tasks(id);
                let next = if task.enabled { next_from(&sched, now) } else { None };
                let detail = format!("{} · {}", describe_action(&action), schedule::describe(&sched));
                match &task.id {
                    Some(tid) => {
                        let t = list
                            .iter_mut()
                            .find(|t| &t.id == tid)
                            .ok_or_else(|| api(ErrorCode::NotFound, "Esa tarea ya no existe"))?;
                        t.enabled = task.enabled;
                        t.action = action;
                        t.schedule = sched;
                        t.next_run_unix_ms = next;
                        self.audit(client, Some(id), "task.update", &detail);
                    }
                    None => {
                        if list.len() >= MAX_TASKS {
                            return Err(api(
                                ErrorCode::Conflict,
                                format!("Como mucho {MAX_TASKS} tareas por servidor."),
                            ));
                        }
                        list.push(ScheduledTask {
                            id: uuid::Uuid::new_v4().simple().to_string()[..12].to_string(),
                            enabled: task.enabled,
                            action,
                            schedule: sched,
                            created_unix_ms: now,
                            next_run_unix_ms: next,
                            last_run: None,
                            running: false,
                        });
                        self.audit(client, Some(id), "task.create", &detail);
                    }
                }
                self.save_tasks(id, &list)?;
                drop(_g);
                ResponseBody::Tasks(self.task_list(id))
            }
            R::DeleteTask { id, task_id } => {
                self.instance(id)?;
                let _g = self.tasks_lock.lock().unwrap_or_else(|p| p.into_inner());
                let mut list = self.load_tasks(id);
                let before = list.len();
                let removed = list
                    .iter()
                    .find(|t| t.id == task_id)
                    .map(|t| describe_action(&t.action));
                list.retain(|t| t.id != task_id);
                if list.len() == before {
                    return Err(api(ErrorCode::NotFound, "Esa tarea ya no existe"));
                }
                self.save_tasks(id, &list)?;
                drop(_g);
                self.audit(client, Some(id), "task.delete", &removed.unwrap_or_default());
                ResponseBody::Tasks(self.task_list(id))
            }
            R::RunTaskNow { id, task_id } => {
                let inst = self.instance(id)?;
                let task = self
                    .load_tasks(id)
                    .into_iter()
                    .find(|t| t.id == task_id)
                    .ok_or_else(|| api(ErrorCode::NotFound, "Esa tarea ya no existe"))?;
                let what = describe_action(&task.action);
                if !self.spawn_task(inst, task, true) {
                    return Err(api(ErrorCode::Conflict, "Esa tarea ya se está ejecutando."));
                }
                self.audit(client, Some(id), "task.run_now", &what);
                ResponseBody::Tasks(self.task_list(id))
            }
            _ => return Err(api(ErrorCode::BadRequest, "Petición no válida")),
        })
    }
}
