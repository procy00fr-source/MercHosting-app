//! Automatización: reinicio automático tras una caída (con espera creciente y un tope para no
//! entrar en bucle) y arrancar con Windows (el supervisor se registra en el inicio de sesión del
//! usuario y enciende los servidores marcados). Ambas cosas son opcionales y vienen apagadas.

use super::{api, Daemon};
use merc_core::{InstanceId, RuntimeState};
use merc_ipc::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Como mucho tantos reinicios automáticos…
pub const MAX_AUTO_RESTARTS: usize = 3;
/// …dentro de esta ventana. Si se pasa, se deja de reintentar hasta que alguien lo encienda.
const WINDOW: Duration = Duration::from_secs(10 * 60);
/// Espera antes de cada reintento (crece para no martillar la PC).
const DELAYS: [Duration; MAX_AUTO_RESTARTS] = [
    Duration::from_secs(10),
    Duration::from_secs(30),
    Duration::from_secs(60),
];
/// Nombre del valor en `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
const RUN_VALUE: &str = "MercHosting";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Settings {
    #[serde(default)]
    auto_restart: bool,
    #[serde(default)]
    autostart: bool,
}

/// Reinicios automáticos en curso (en memoria: un reinicio del supervisor empieza de cero).
#[derive(Default)]
pub(crate) struct RestartBook {
    attempts: HashMap<InstanceId, Vec<Instant>>,
    due: HashMap<InstanceId, Instant>,
    gave_up: HashSet<InstanceId>,
}

fn delays() -> [Duration; MAX_AUTO_RESTARTS] {
    if cfg!(debug_assertions) {
        if let Ok(v) = std::env::var("MERC_AUTORESTART_MS") {
            if let Ok(ms) = v.parse::<u64>() {
                return [Duration::from_millis(ms); MAX_AUTO_RESTARTS];
            }
        }
    }
    DELAYS
}

impl Daemon {
    fn automation_file(&self, id: InstanceId) -> PathBuf {
        self.store_dir(id).join("automatizacion.json")
    }

    fn automation(&self, id: InstanceId) -> Settings {
        std::fs::read_to_string(self.automation_file(id))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn save_automation(&self, id: InstanceId, s: &Settings) -> Result<(), ApiError> {
        std::fs::create_dir_all(self.store_dir(id)).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        std::fs::write(
            self.automation_file(id),
            serde_json::to_string_pretty(s).unwrap_or_default(),
        )
        .map_err(|e| api(ErrorCode::Internal, e.to_string()))
    }

    /// Una caída: si el reinicio automático está activado y no se pasó del tope, lo programa.
    /// Devuelve el texto que se añade a la alerta de caída.
    pub(crate) fn on_crash(&self, id: InstanceId) -> Option<String> {
        if !self.automation(id).auto_restart || self.is_archived(id) {
            return None;
        }
        let mut book = self.restarts.lock().unwrap_or_else(|p| p.into_inner());
        let list = book.attempts.entry(id).or_default();
        list.retain(|t| t.elapsed() < WINDOW);
        let n = list.len();
        if n >= MAX_AUTO_RESTARTS {
            book.due.remove(&id);
            book.gave_up.insert(id);
            return Some(format!(
                " Se reinició solo {MAX_AUTO_RESTARTS} veces en 10 minutos y volvió a caer: ya no se reinicia solo hasta que lo enciendas tú. Revisa el Diagnóstico."
            ));
        }
        let wait = delays()[n];
        book.due.insert(id, Instant::now() + wait);
        Some(format!(
            " Se reiniciará solo en {} s (intento {} de {MAX_AUTO_RESTARTS}).",
            wait.as_secs().max(1),
            n + 1
        ))
    }

    /// Enciende los reinicios que ya tocan (si nadie lo encendió ni lo cambió mientras tanto).
    pub(crate) fn run_due_restarts(&self) {
        let ready: Vec<InstanceId> = {
            let mut book = self.restarts.lock().unwrap_or_else(|p| p.into_inner());
            let now = Instant::now();
            let ids: Vec<InstanceId> = book.due.iter().filter(|(_, t)| **t <= now).map(|(id, _)| *id).collect();
            for id in &ids {
                book.due.remove(id);
            }
            ids
        };
        for id in ready {
            let Ok(inst) = self.instance(id) else { continue };
            if self.sup.status(id).state != RuntimeState::Crashed
                || !self.automation(id).auto_restart
                || self.is_archived(id)
            {
                continue;
            }
            self.restarts
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .attempts
                .entry(id)
                .or_default()
                .push(Instant::now());
            match self.sup.start(Self::spec(&inst)) {
                Ok(_) => self.audit(
                    "supervisor",
                    Some(id),
                    "server.auto_restart",
                    "reinicio automático tras una caída",
                ),
                Err(e) => self.audit("supervisor", Some(id), "server.auto_restart.failed", &e.to_string()),
            }
        }
    }

    /// Alguien lo encendió a mano: se olvida el «dejó de reintentar».
    pub(crate) fn clear_gave_up(&self, id: InstanceId) {
        let mut book = self.restarts.lock().unwrap_or_else(|p| p.into_inner());
        book.gave_up.remove(&id);
        book.attempts.remove(&id);
        book.due.remove(&id);
    }

    /// Al arrancar con Windows: enciende los servidores marcados (sin EULA o archivados, no).
    pub(crate) fn start_at_login(self: &Arc<Self>) {
        let instances = self
            .store
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .list_instances()
            .unwrap_or_default();
        for inst in instances {
            if !self.automation(inst.id).autostart {
                continue;
            }
            if self.is_archived(inst.id) {
                self.audit(
                    "supervisor",
                    Some(inst.id),
                    "server.autostart.skipped",
                    "está archivado",
                );
                continue;
            }
            if !matches!(
                merc_core::properties::read_eula(&inst.root),
                Ok(merc_core::properties::EulaStatus::Accepted)
            ) {
                self.audit(
                    "supervisor",
                    Some(inst.id),
                    "server.autostart.skipped",
                    "falta aceptar la EULA",
                );
                continue;
            }
            match self.sup.start(Self::spec(&inst)) {
                Ok(_) => self.audit(
                    "supervisor",
                    Some(inst.id),
                    "server.autostart",
                    "encendido al iniciar Windows",
                ),
                Err(e) => self.audit("supervisor", Some(inst.id), "server.autostart.failed", &e.to_string()),
            }
        }
    }

    fn run_command(&self) -> Result<String, String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        Ok(format!(
            "\"{}\" --data-dir \"{}\" --al-iniciar-windows",
            exe.display(),
            self.data_dir.display()
        ))
    }

    fn automation_info(&self, id: InstanceId) -> AutomationInfo {
        let s = self.automation(id);
        let book = self.restarts.lock().unwrap_or_else(|p| p.into_inner());
        let recent = book
            .attempts
            .get(&id)
            .map_or(0, |v| v.iter().filter(|t| t.elapsed() < WINDOW).count());
        let (available, enabled) = match (run_key::get(&self.data_dir, RUN_VALUE), self.run_command()) {
            (Err(_), _) => (false, false),
            (Ok(cur), Ok(cmd)) => (true, cur.as_deref() == Some(cmd.as_str())),
            (Ok(_), Err(_)) => (true, false),
        };
        AutomationInfo {
            auto_restart: s.auto_restart,
            max_restarts: MAX_AUTO_RESTARTS as u32,
            recent_restarts: recent as u32,
            gave_up: book.gave_up.contains(&id),
            autostart: s.autostart,
            startup_available: available,
            startup_enabled: enabled,
        }
    }

    pub(crate) fn handle_automation(
        self: &Arc<Self>,
        client: &str,
        body: RequestBody,
    ) -> Result<ResponseBody, ApiError> {
        match body {
            RequestBody::Automation { id } => {
                self.instance(id)?;
                Ok(ResponseBody::Automation(self.automation_info(id)))
            }
            RequestBody::SetAutomation {
                id,
                auto_restart,
                autostart,
            } => {
                self.instance(id)?;
                let mut s = self.automation(id);
                if let Some(v) = auto_restart {
                    s.auto_restart = v;
                    if !v {
                        self.clear_gave_up(id);
                    }
                    self.audit(
                        client,
                        Some(id),
                        "server.auto_restart.set",
                        if v { "activado" } else { "desactivado" },
                    );
                }
                if let Some(v) = autostart {
                    s.autostart = v;
                    self.audit(
                        client,
                        Some(id),
                        "server.autostart.set",
                        if v { "activado" } else { "desactivado" },
                    );
                }
                self.save_automation(id, &s)?;
                Ok(ResponseBody::Automation(self.automation_info(id)))
            }
            RequestBody::SetStartup { enabled } => {
                let r = if enabled {
                    let cmd = self.run_command().map_err(|e| api(ErrorCode::Internal, e))?;
                    run_key::set(&self.data_dir, RUN_VALUE, &cmd)
                } else {
                    run_key::delete(&self.data_dir, RUN_VALUE)
                };
                r.map_err(|e| api(ErrorCode::InvalidState, e))?;
                self.audit(
                    client,
                    None,
                    "app.startup",
                    if enabled {
                        "arrancar con Windows: sí"
                    } else {
                        "arrancar con Windows: no"
                    },
                );
                Ok(ResponseBody::Ok)
            }
            _ => Err(api(ErrorCode::BadRequest, "Petición no válida")),
        }
    }
}

/// Valor de inicio de sesión del usuario (`HKCU\…\Run`). En pruebas (compilación de depuración)
/// `MERC_FAKE_RUN_KEY` lo sustituye por un archivo para poder comprobarlo fuera de Windows.
mod run_key {
    use std::path::Path;

    fn fake() -> Option<std::path::PathBuf> {
        if cfg!(debug_assertions) {
            std::env::var_os("MERC_FAKE_RUN_KEY").map(Into::into)
        } else {
            None
        }
    }

    pub fn get(_data: &Path, name: &str) -> Result<Option<String>, String> {
        if let Some(f) = fake() {
            return Ok(std::fs::read_to_string(f).ok());
        }
        imp::get(name)
    }

    pub fn set(_data: &Path, name: &str, value: &str) -> Result<(), String> {
        if let Some(f) = fake() {
            return std::fs::write(f, value).map_err(|e| e.to_string());
        }
        imp::set(name, value)
    }

    pub fn delete(_data: &Path, name: &str) -> Result<(), String> {
        if let Some(f) = fake() {
            let _ = std::fs::remove_file(f);
            return Ok(());
        }
        imp::delete(name)
    }

    #[cfg(windows)]
    mod imp {
        use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
        use windows_sys::Win32::System::Registry::{
            RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
            KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
        };

        const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

        fn wide(s: &str) -> Vec<u16> {
            s.encode_utf16().chain(std::iter::once(0)).collect()
        }

        fn open(access: u32) -> Result<HKEY, String> {
            let mut key: HKEY = std::ptr::null_mut();
            let path = wide(RUN);
            // SAFETY: argumentos válidos; la clave se cierra con RegCloseKey.
            let r = unsafe {
                RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    path.as_ptr(),
                    0,
                    std::ptr::null(),
                    REG_OPTION_NON_VOLATILE,
                    access,
                    std::ptr::null(),
                    &mut key,
                    std::ptr::null_mut(),
                )
            };
            if r == ERROR_SUCCESS {
                Ok(key)
            } else {
                Err(format!("No se pudo abrir la lista de inicio de Windows (error {r})"))
            }
        }

        pub fn get(name: &str) -> Result<Option<String>, String> {
            let key = open(KEY_QUERY_VALUE)?;
            let n = wide(name);
            let mut buf = vec![0u16; 2048];
            let mut size = (buf.len() * 2) as u32;
            let mut ty = 0u32;
            // SAFETY: el búfer tiene `size` bytes.
            let r = unsafe {
                RegQueryValueExW(
                    key,
                    n.as_ptr(),
                    std::ptr::null(),
                    &mut ty,
                    buf.as_mut_ptr().cast(),
                    &mut size,
                )
            };
            // SAFETY: clave abierta arriba.
            unsafe { RegCloseKey(key) };
            if r == ERROR_FILE_NOT_FOUND {
                return Ok(None);
            }
            if r != ERROR_SUCCESS || ty != REG_SZ {
                return Ok(None);
            }
            let len = (size as usize / 2).min(buf.len());
            let s = String::from_utf16_lossy(&buf[..len]);
            Ok(Some(s.trim_end_matches('\0').to_string()))
        }

        pub fn set(name: &str, value: &str) -> Result<(), String> {
            let key = open(KEY_SET_VALUE)?;
            let n = wide(name);
            let v = wide(value);
            // SAFETY: `v` incluye el cero final; el tamaño va en bytes.
            let r = unsafe { RegSetValueExW(key, n.as_ptr(), 0, REG_SZ, v.as_ptr().cast(), (v.len() * 2) as u32) };
            // SAFETY: clave abierta arriba.
            unsafe { RegCloseKey(key) };
            if r == ERROR_SUCCESS {
                Ok(())
            } else {
                Err(format!("No se pudo guardar en el inicio de Windows (error {r})"))
            }
        }

        pub fn delete(name: &str) -> Result<(), String> {
            let key = open(KEY_SET_VALUE)?;
            let n = wide(name);
            // SAFETY: clave abierta arriba.
            let r = unsafe { RegDeleteValueW(key, n.as_ptr()) };
            // SAFETY: clave abierta arriba.
            unsafe { RegCloseKey(key) };
            if r == ERROR_SUCCESS || r == ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                Err(format!("No se pudo quitar del inicio de Windows (error {r})"))
            }
        }
    }

    #[cfg(not(windows))]
    mod imp {
        const NO: &str = "Arrancar con Windows solo está disponible en Windows.";
        pub fn get(_: &str) -> Result<Option<String>, String> {
            Err(NO.into())
        }
        pub fn set(_: &str, _: &str) -> Result<(), String> {
            Err(NO.into())
        }
        pub fn delete(_: &str) -> Result<(), String> {
            Err(NO.into())
        }
    }
}
