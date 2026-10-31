//! Trabajos de creación de servidores: corren en su propio hilo para que la interfaz pueda
//! consultar el progreso (paso, descarga, registro) y cancelar en cualquier momento.

use merc_install::{InstallEvent, Step};
use merc_ipc::{JobDownload, JobLogLine, JobSnapshot, JobState};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MAX_LOG_LINES: usize = 600;
/// Trabajos terminados que se recuerdan para que la interfaz lea el resultado.
const KEEP_FINISHED: Duration = Duration::from_secs(30 * 60);
pub const MAX_RUNNING_JOBS: usize = 2;

pub struct Job {
    pub cancel: AtomicBool,
    pub target: std::path::PathBuf,
    state: Mutex<Inner>,
}

struct Inner {
    snap: JobSnapshot,
    seq: u64,
    finished_at: Option<Instant>,
}

impl Job {
    fn new(id: String, name: String, target: std::path::PathBuf) -> Self {
        Self {
            cancel: AtomicBool::new(false),
            target,
            state: Mutex::new(Inner {
                snap: JobSnapshot {
                    job_id: id,
                    name,
                    state: JobState::Running,
                    step: None,
                    message: "En cola".into(),
                    download: None,
                    log: vec![],
                    next_after: 0,
                    instance_id: None,
                    error: None,
                },
                seq: 0,
                finished_at: None,
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn log(&self, text: impl Into<String>) {
        let mut g = self.lock();
        g.seq += 1;
        let seq = g.seq;
        g.snap.log.push(JobLogLine { seq, text: text.into() });
        if g.snap.log.len() > MAX_LOG_LINES {
            let drop = g.snap.log.len() - MAX_LOG_LINES;
            g.snap.log.drain(..drop);
        }
    }

    pub fn event(&self, e: InstallEvent) {
        match e {
            InstallEvent::Step(step, msg) => {
                {
                    let mut g = self.lock();
                    g.snap.step = Some(step);
                    g.snap.message = msg.clone();
                    if step != Step::Downloading {
                        g.snap.download = None;
                    }
                }
                self.log(format!("▸ {msg}"));
            }
            InstallEvent::Download { file, done, total } => {
                self.lock().snap.download = Some(JobDownload { file, done, total });
            }
            InstallEvent::Log(line) => self.log(line),
        }
    }

    pub fn set_step(&self, step: Step, msg: &str) {
        self.event(InstallEvent::Step(step, msg.to_string()));
    }

    pub fn finish(&self, state: JobState, error: Option<String>, instance: Option<merc_core::InstanceId>) {
        let msg = match state {
            JobState::Succeeded => "Servidor creado".to_string(),
            JobState::Cancelled => "Creación cancelada; se deshicieron los cambios".to_string(),
            _ => "No se pudo crear el servidor".to_string(),
        };
        self.finish_with(state, error, instance, msg);
    }

    /// Como `finish`, con el mensaje final propio del trabajo.
    pub fn finish_with(
        &self,
        state: JobState,
        error: Option<String>,
        instance: Option<merc_core::InstanceId>,
        msg: String,
    ) {
        if let Some(e) = &error {
            self.log(format!("✖ {e}"));
        } else {
            self.log(format!("✔ {msg}"));
        }
        let mut g = self.lock();
        g.snap.state = state;
        g.snap.message = msg;
        g.snap.error = error;
        g.snap.instance_id = instance;
        g.snap.download = None;
        g.finished_at = Some(Instant::now());
    }

    pub fn is_running(&self) -> bool {
        self.lock().snap.state == JobState::Running
    }

    pub fn snapshot(&self, after: u64) -> JobSnapshot {
        let g = self.lock();
        let mut s = g.snap.clone();
        s.log.retain(|l| l.seq > after);
        s.next_after = s.log.last().map(|l| l.seq).unwrap_or(after);
        s
    }

    fn expired(&self) -> bool {
        self.lock().finished_at.is_some_and(|t| t.elapsed() > KEEP_FINISHED)
    }
}

#[derive(Default)]
pub struct Jobs {
    map: Mutex<HashMap<String, Arc<Job>>>,
}

impl Jobs {
    pub fn create(&self, name: String, target: std::path::PathBuf) -> Result<Arc<Job>, String> {
        let mut m = self.map.lock().unwrap_or_else(|p| p.into_inner());
        m.retain(|_, j| !j.expired());
        if m.values().any(|j| j.is_running() && j.target == target) {
            return Err("Ya se está creando un servidor en esa carpeta".into());
        }
        if m.values().filter(|j| j.is_running()).count() >= MAX_RUNNING_JOBS {
            return Err("Ya se están creando varios servidores; espera a que termine uno".into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        let job = Arc::new(Job::new(id.clone(), name, target));
        m.insert(id, job.clone());
        Ok(job)
    }

    pub fn get(&self, id: &str) -> Option<Arc<Job>> {
        self.map.lock().unwrap_or_else(|p| p.into_inner()).get(id).cloned()
    }

    pub fn any_running(&self) -> bool {
        self.map
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .values()
            .any(|j| j.is_running())
    }

    pub fn cancel(&self, id: &str) -> bool {
        match self.get(id) {
            Some(j) => {
                j.cancel.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_is_incremental_and_bounded() {
        let jobs = Jobs::default();
        let j = jobs.create("A".into(), "/a".into()).unwrap();
        for i in 0..700 {
            j.log(format!("línea {i}"));
        }
        let s = j.snapshot(0);
        assert_eq!(s.log.len(), MAX_LOG_LINES);
        assert_eq!(s.next_after, 700);
        assert!(j.snapshot(700).log.is_empty());
        assert_eq!(j.snapshot(698).log.len(), 2);
        assert!(jobs.create("A2".into(), "/a".into()).is_err(), "misma carpeta");
        jobs.create("B".into(), "/b".into()).unwrap();
        assert!(
            jobs.create("C".into(), "/c".into()).is_err(),
            "máximo de trabajos simultáneos"
        );
        j.finish(JobState::Succeeded, None, None);
        assert!(jobs.create("C".into(), "/c".into()).is_ok());
    }
}
