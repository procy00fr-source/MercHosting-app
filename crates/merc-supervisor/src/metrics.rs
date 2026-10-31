//! Medición real del proceso del servidor: CPU y memoria según el sistema operativo.
//!
//! Solo se mide lo que el SO informa del proceso Java. TPS, entidades, etc. necesitarán un
//! puente dentro del servidor (fase posterior) y no se inventan aquí.

use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// Intervalo entre muestras.
pub const SAMPLE_INTERVAL_MS: u64 = 2000;
/// Muestras guardadas por servidor (6 minutos con el intervalo actual).
pub const MAX_SAMPLES: usize = 180;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricSample {
    pub at_unix_ms: u64,
    /// Porcentaje de CPU de TODA la PC (0–100), no de un solo núcleo.
    pub cpu_percent: f32,
    /// Memoria residente del proceso (bytes).
    pub memory_bytes: u64,
}

/// Mide un proceso con un `System` propio (sysinfo necesita dos lecturas para calcular la CPU).
pub struct ProcessMeter {
    sys: System,
    pid: Pid,
    cpus: f32,
}

impl ProcessMeter {
    pub fn new(pid: u32) -> Option<Self> {
        let sys = System::new().ok()?;
        let cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f32;
        let mut m = Self {
            sys,
            pid: Pid::from_u32(pid),
            cpus,
        };
        m.refresh();
        Some(m)
    }

    fn refresh(&mut self) -> bool {
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[self.pid]),
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        self.sys.process(self.pid).is_some()
    }

    /// Toma una muestra. `None` si el proceso ya no existe.
    pub fn sample(&mut self, now_ms: u64) -> Option<MetricSample> {
        if !self.refresh() {
            return None;
        }
        let p = self.sys.process(self.pid)?;
        // sysinfo da el % respecto a un núcleo (puede pasar de 100); se normaliza a toda la PC.
        let cpu = (p.cpu_usage() / self.cpus).clamp(0.0, 100.0);
        Some(MetricSample {
            at_unix_ms: now_ms,
            cpu_percent: cpu,
            memory_bytes: p.memory(),
        })
    }
}

/// Memoria física total de la PC (bytes), si el sistema la informa.
pub fn total_memory_bytes() -> Option<u64> {
    let mut sys = System::new().ok()?;
    sys.refresh_memory();
    let t = sys.total_memory();
    (t > 0).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_own_process() {
        let mut m = ProcessMeter::new(std::process::id()).unwrap();
        // Trabajo para que haya algo de CPU.
        let mut x = 0u64;
        for i in 0..20_000_000u64 {
            x = x.wrapping_add(i * i);
        }
        assert!(x > 0);
        std::thread::sleep(std::time::Duration::from_millis(250));
        let s = m.sample(1).expect("el proceso actual existe");
        assert!(
            s.memory_bytes > 1024 * 1024,
            "memoria real del proceso: {}",
            s.memory_bytes
        );
        assert!((0.0..=100.0).contains(&s.cpu_percent));
        assert!(total_memory_bytes().unwrap() > s.memory_bytes);
    }

    #[test]
    fn missing_process() {
        // PID muy alto que no debería existir.
        if let Some(mut m) = ProcessMeter::new(u32::MAX - 7) {
            assert!(m.sample(1).is_none());
        }
    }
}
