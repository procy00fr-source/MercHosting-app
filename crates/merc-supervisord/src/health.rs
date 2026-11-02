//! Salud y alertas en el supervisor.
//!
//! Un hilo revisa cada poco: si un servidor se cerró solo (caída), si queda poco espacio en los
//! discos de los servidores o de las copias, y si la memoria de la PC está casi llena. Las tareas
//! programadas que fallan también dejan alerta. Las alertas se guardan en `alertas.json`; la
//! ventana las muestra y, si está abierta, lanza la notificación de Windows.

use super::{api, Daemon};
use merc_admin::health::{self, Alert, AlertKind, AlertLevel, AlertSettings};
use merc_core::{InstanceId, RuntimeState};
use merc_ipc::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const GB: u64 = 1024 * 1024 * 1024;
/// Memoria de la PC a partir de la que se avisa (si se mantiene varias mediciones seguidas).
const HIGH_MEMORY: f64 = 0.92;
/// Mediciones seguidas (una cada 5 s) por encima del límite antes de avisar.
const HIGH_MEMORY_SAMPLES: u32 = 6;
/// Cada cuánto se mide la CPU y la memoria de la PC.
const PC_SAMPLE: Duration = Duration::from_secs(2);
const DISK_REPEAT: Duration = Duration::from_secs(6 * 3600);
const MEMORY_REPEAT: Duration = Duration::from_secs(3600);
const TPS_TIMEOUT: Duration = Duration::from_secs(10);

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Última medición de la PC: (CPU %, memoria usada, memoria total).
pub(crate) type PcLoad = Option<(f32, u64, u64)>;

impl Daemon {
    fn alerts_file(&self) -> PathBuf {
        self.data_dir.join("alertas.json")
    }

    fn alert_settings_file(&self) -> PathBuf {
        self.data_dir.join("alertas-ajustes.json")
    }

    pub(crate) fn alert_settings(&self) -> AlertSettings {
        std::fs::read_to_string(self.alert_settings_file())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Registra una alerta (si su tipo está activado en los ajustes).
    pub(crate) fn push_alert(
        &self,
        kind: AlertKind,
        level: AlertLevel,
        instance: Option<(InstanceId, String)>,
        title: String,
        message: String,
    ) {
        if !self.alert_settings().wants(kind) {
            return;
        }
        let _g = self.alerts_lock.lock().unwrap_or_else(|p| p.into_inner());
        let mut list = health::load_alerts(&self.alerts_file());
        health::push_alert(
            &mut list,
            Alert {
                id: uuid::Uuid::new_v4().simple().to_string()[..12].to_string(),
                at_unix_ms: now_ms(),
                level,
                kind,
                instance_id: instance.as_ref().map(|(i, _)| i.to_string()),
                server_name: instance.map(|(_, n)| n),
                title: title.clone(),
                message: message.clone(),
                read: false,
            },
        );
        if let Err(e) = health::save_alerts(&self.alerts_file(), &list) {
            self.log.line(&format!("no se pudo guardar una alerta: {e}"));
        }
        self.log.line(&format!("alerta: {title} — {message}"));
    }

    fn alert_list(&self) -> AlertList {
        let items = health::load_alerts(&self.alerts_file());
        AlertList {
            unread: items.iter().filter(|a| !a.read).count() as u32,
            items,
            settings: self.alert_settings(),
        }
    }

    /// Bucle de vigilancia (caídas, discos y memoria de la PC).
    /// Mide la CPU y la memoria de la PC cada 2 s (lo que muestra la pestaña Salud).
    pub(crate) fn pc_sampler_loop(self: Arc<Self>) {
        let Ok(mut sys) = sysinfo::System::new() else {
            self.log
                .line("no se pudo medir la PC (sysinfo): sin CPU ni memoria de la PC");
            return;
        };
        loop {
            sys.refresh_cpu_usage();
            sys.refresh_memory();
            *self.pc_load.lock().unwrap_or_else(|p| p.into_inner()) =
                Some((sys.global_cpu_usage(), sys.used_memory(), sys.total_memory()));
            std::thread::sleep(PC_SAMPLE);
        }
    }

    pub(crate) fn health_loop(self: Arc<Self>) {
        let mut tick = Duration::from_secs(5);
        if cfg!(debug_assertions) {
            if let Some(ms) = std::env::var("MERC_HEALTH_TICK_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
            {
                tick = Duration::from_millis(ms.max(50));
            }
        }
        let mut states: HashMap<InstanceId, RuntimeState> = HashMap::new();
        let mut disk_warned: HashMap<String, Instant> = HashMap::new();
        let mut high_mem_count = 0u32;
        let mut mem_warned: Option<Instant> = None;
        loop {
            let (used, total) = self
                .pc_load
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .map(|(_, u, t)| (u, t))
                .unwrap_or((0, 0));

            let instances = self
                .store
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .list_instances()
                .unwrap_or_default();
            let settings = self.alert_settings();
            // Caídas: el servidor pasó a «Crashed» desde otro estado.
            for inst in &instances {
                let st = self.sup.status(inst.id);
                let prev = states.insert(inst.id, st.state);
                self.track_players(inst, st.state);
                if st.state == RuntimeState::Online && prev != Some(RuntimeState::Online) {
                    self.apply_pending_rules(inst);
                }
                if st.state == RuntimeState::Crashed && prev.is_some_and(|p| p != RuntimeState::Crashed) {
                    let detail = st
                        .hint
                        .clone()
                        .or_else(|| {
                            st.last_exit
                                .as_ref()
                                .and_then(|e| e.code)
                                .map(|c| format!("Código de salida {c}."))
                        })
                        .unwrap_or_else(|| "Mira la consola para ver el error.".into());
                    let detail = match self.on_crash(inst.id) {
                        Some(extra) => format!("{detail}{extra}"),
                        None => detail,
                    };
                    self.push_alert(
                        AlertKind::Crash,
                        AlertLevel::Critical,
                        Some((inst.id, inst.name.clone())),
                        format!("«{}» se cerró inesperadamente", inst.name),
                        detail,
                    );
                }
            }
            self.run_due_restarts();
            self.keep_awake_tick();
            self.tunnel_watch();
            // Discos: el de cada servidor y el de las copias de seguridad.
            let mut paths: Vec<(PathBuf, String)> =
                instances.iter().map(|i| (i.root.clone(), i.name.clone())).collect();
            paths.push((self.data_dir.clone(), "las copias de seguridad".into()));
            let limit = settings.disk_min_gb as u64 * GB;
            let mut seen_disks: HashMap<String, (u64, Vec<String>)> = HashMap::new();
            for (p, who) in &paths {
                if let Some((free, _, mount)) = merc_admin::backups::disk_info(p) {
                    // Se agrupan por disco para avisar una sola vez.
                    let e = seen_disks.entry(mount).or_insert((free, vec![]));
                    e.1.push(who.clone());
                }
            }
            for (key, (free, who)) in seen_disks {
                if free < limit {
                    let recent = disk_warned.get(&key).is_some_and(|t| t.elapsed() < DISK_REPEAT);
                    if !recent {
                        disk_warned.insert(key.clone(), Instant::now());
                        self.push_alert(
                            AlertKind::LowDisk,
                            AlertLevel::Warning,
                            None,
                            format!(
                                "Queda poco espacio en el disco {} ({:.1} GB libres)",
                                key.trim_end_matches(['\\', '/']),
                                free as f64 / GB as f64
                            ),
                            format!(
                                "Lo usan {}. Si se llena, el mundo o las copias pueden quedar incompletos. Borra copias viejas o libera espacio.",
                                who.join(", ")
                            ),
                        );
                    }
                } else {
                    disk_warned.remove(&key);
                }
            }
            // Memoria de la PC casi llena durante varias mediciones seguidas.
            if total > 0 && used as f64 / total as f64 >= HIGH_MEMORY {
                high_mem_count += 1;
            } else {
                high_mem_count = 0;
            }
            if high_mem_count >= HIGH_MEMORY_SAMPLES && mem_warned.is_none_or(|t| t.elapsed() > MEMORY_REPEAT) {
                mem_warned = Some(Instant::now());
                self.push_alert(
                    AlertKind::HighMemory,
                    AlertLevel::Warning,
                    None,
                    format!(
                        "La memoria de la PC está casi llena ({:.0} %)",
                        used as f64 * 100.0 / total as f64
                    ),
                    "Puede ir lento todo: el juego, el servidor y el sistema. Cierra programas que no uses o baja la memoria asignada al servidor."
                        .into(),
                );
            }
            std::thread::sleep(tick);
        }
    }

    /// Manda el comando de TPS del servidor y lee la respuesta en la consola.
    fn measure_tps(&self, id: InstanceId) -> Result<TpsReading, ApiError> {
        let inst = self.instance(id)?;
        let cmd = health::tps_command(inst.kind).ok_or_else(|| {
            api(
                ErrorCode::BadRequest,
                "Este tipo de servidor no informa su TPS (Vanilla y Fabric no tienen ese comando).",
            )
        })?;
        let st = self.sup.status(id);
        if st.state != RuntimeState::Online || !st.has_console {
            return Err(api(ErrorCode::InvalidState, "Enciende el servidor para medir su TPS."));
        }
        let after = self.sup.console_since(id, 0).last().map(|l| l.seq).unwrap_or(0);
        self.sup.send_command(id, cmd).map_err(super::sup_err)?;
        let deadline = Instant::now() + TPS_TIMEOUT;
        while Instant::now() < deadline {
            for l in self.sup.console_since(id, after) {
                if let Some(r) = health::parse_tps(&l.text, cmd) {
                    return Ok(r);
                }
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        Err(api(
            ErrorCode::Conflict,
            format!("El servidor no respondió a «{cmd}» en 10 s (puede que un mod o plugin lo cambie)."),
        ))
    }

    pub(crate) fn handle_health(&self, client: &str, body: RequestBody) -> Result<ResponseBody, ApiError> {
        use RequestBody as R;
        Ok(match body {
            R::Health { id } => {
                let inst = self.instance(id)?;
                let load = *self.pc_load.lock().unwrap_or_else(|p| p.into_inner());
                let mut disks = vec![];
                for (what, p) in [("server", inst.root.clone()), ("backups", self.backups_dir(id))] {
                    if let Some((free, total)) = merc_admin::backups::disk_space(&p) {
                        disks.push(DiskSpace {
                            what: what.into(),
                            path: merc_core::paths::strip_verbatim(&p).display().to_string(),
                            free_bytes: free,
                            total_bytes: total,
                        });
                    }
                }
                ResponseBody::Health(HealthReport {
                    pc_cpu_percent: load.map(|l| l.0),
                    pc_memory_used_bytes: load.map(|l| l.1),
                    pc_memory_total_bytes: load.map(|l| l.2).or_else(merc_supervisor::metrics::total_memory_bytes),
                    disks,
                    max_memory_mb: inst.launch.max_memory_mb,
                    tps_command: health::tps_command(inst.kind).map(str::to_string),
                })
            }
            R::MeasureTps { id } => ResponseBody::Tps(self.measure_tps(id)?),
            R::Alerts => ResponseBody::Alerts(self.alert_list()),
            R::MarkAlertsRead { ids } => {
                {
                    let _g = self.alerts_lock.lock().unwrap_or_else(|p| p.into_inner());
                    let mut list = health::load_alerts(&self.alerts_file());
                    for a in &mut list {
                        if ids.is_empty() || ids.contains(&a.id) {
                            a.read = true;
                        }
                    }
                    health::save_alerts(&self.alerts_file(), &list)
                        .map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
                }
                ResponseBody::Alerts(self.alert_list())
            }
            R::SetAlertSettings { settings } => {
                if !(1..=1000).contains(&settings.disk_min_gb) {
                    return Err(api(ErrorCode::BadRequest, "El aviso de disco va de 1 a 1000 GB."));
                }
                let text =
                    serde_json::to_string_pretty(&settings).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
                std::fs::write(self.alert_settings_file(), text)
                    .map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
                self.audit(client, None, "alerts.settings", &format!("{settings:?}"));
                ResponseBody::Alerts(self.alert_list())
            }
            _ => return Err(api(ErrorCode::BadRequest, "Petición no válida")),
        })
    }
}
