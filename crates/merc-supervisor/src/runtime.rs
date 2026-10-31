//! Registro en disco de los procesos Java que el supervisor lanzó.
//!
//! Si el supervisor se cierra de forma inesperada, el proceso Java del servidor puede seguir
//! vivo (Minecraft sigue funcionando aunque se cierre su consola). Al volver a arrancar, el
//! supervisor lee estos registros y comprueba la identidad del proceso por PID **y** hora de
//! inicio, para no confundirlo con otro programa que haya recibido el mismo PID.
//!
//! Límite honesto: el supervisor nuevo no puede recuperar stdin/stdout de un proceso que no
//! creó. Un servidor "huérfano" se muestra como UNKNOWN, se impide arrancar otro sobre la misma
//! carpeta y se ofrece el cierre forzado con confirmación. No se envía `stop` porque no hay consola.

use merc_core::InstanceId;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeRecord {
    pub instance: InstanceId,
    pub pid: u32,
    /// Segundos Unix de inicio del proceso según el sistema operativo.
    pub process_start_unix: u64,
    pub root: PathBuf,
    pub port: Option<u16>,
    pub started_at_unix_ms: u64,
}

pub struct RuntimeDir {
    dir: PathBuf,
}

impl RuntimeDir {
    pub fn new(dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    fn path(&self, id: InstanceId) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    /// Escritura atómica: archivo temporal + renombrado.
    pub fn write(&self, rec: &RuntimeRecord) -> std::io::Result<()> {
        let tmp = self.dir.join(format!("{}.json.tmp", rec.instance));
        std::fs::write(&tmp, serde_json::to_vec_pretty(rec).map_err(std::io::Error::other)?)?;
        std::fs::rename(&tmp, self.path(rec.instance))
    }

    pub fn remove(&self, id: InstanceId) {
        let _ = std::fs::remove_file(self.path(id));
    }

    pub fn list(&self) -> Vec<RuntimeRecord> {
        let mut out = vec![];
        for entry in std::fs::read_dir(&self.dir).into_iter().flatten().flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(bytes) = std::fs::read(&p) {
                if let Ok(rec) = serde_json::from_slice::<RuntimeRecord>(&bytes) {
                    out.push(rec);
                }
            }
        }
        out
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

/// Hora de inicio del proceso según el SO, o `None` si no existe.
pub fn process_start_time(pid: u32) -> Option<u64> {
    let mut sys = System::new().ok()?;
    let p = Pid::from_u32(pid);
    sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[p]), true, ProcessRefreshKind::nothing());
    sys.process(p).map(|pr| pr.start_time())
}

/// ¿Sigue vivo el mismo proceso? Tolerancia de 2 s por redondeos del SO.
pub fn same_process_alive(pid: u32, start_unix: u64) -> bool {
    match process_start_time(pid) {
        Some(t) => t.abs_diff(start_unix) <= 2,
        None => false,
    }
}

/// Cierra a la fuerza un proceso solo si su identidad coincide (PID + hora de inicio).
pub fn kill_verified(pid: u32, start_unix: u64) -> bool {
    let Ok(mut sys) = System::new() else { return false };
    let p = Pid::from_u32(pid);
    sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[p]), true, ProcessRefreshKind::nothing());
    match sys.process(p) {
        Some(pr) if pr.start_time().abs_diff(start_unix) <= 2 => pr.kill(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_process_is_alive_and_wrong_start_is_not() {
        let me = std::process::id();
        let t = process_start_time(me).expect("el proceso actual debe existir");
        assert!(same_process_alive(me, t));
        assert!(
            !same_process_alive(me, t.saturating_sub(3600)),
            "otro inicio = otro proceso"
        );
        assert!(
            !kill_verified(me, t.saturating_sub(3600)),
            "no debe matar si no coincide la identidad"
        );
    }

    #[test]
    fn records_roundtrip() {
        let d = tempfile::tempdir().unwrap();
        let rd = RuntimeDir::new(d.path()).unwrap();
        let rec = RuntimeRecord {
            instance: InstanceId::new(),
            pid: 42,
            process_start_unix: 7,
            root: "x".into(),
            port: Some(25565),
            started_at_unix_ms: 1,
        };
        rd.write(&rec).unwrap();
        assert_eq!(rd.list(), vec![rec.clone()]);
        rd.remove(rec.instance);
        assert!(rd.list().is_empty());
    }
}
