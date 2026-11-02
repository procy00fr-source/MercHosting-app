//! «No suspender la PC mientras haya un servidor encendido»: para jugar 24/7, el sistema no debe
//! dormir la PC y cortar el servidor. En Windows se pide con `SetThreadExecutionState`
//! (`ES_SYSTEM_REQUIRED`); en Linux con `systemd-inhibit` (bloqueo «sleep:idle»). Solo evita la
//! suspensión: la pantalla se puede apagar y apagar la PC sigue funcionando. Diferencia: en
//! Windows suspender a mano sigue funcionando; en Linux el bloqueo también frena la suspensión
//! pedida desde el escritorio (que suele avisar de que MercHosting la impide). Viene apagado.
//!
//! La petición vale mientras viva el hilo que la hizo, así que vive en un hilo propio.

use super::Daemon;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Saved {
    #[serde(default)]
    enabled: bool,
}

/// Windows confirmó la petición (y sigue vigente).
static ACTIVE: AtomicBool = AtomicBool::new(false);
static WORKER: OnceLock<Mutex<Sender<bool>>> = OnceLock::new();

fn worker() -> &'static Mutex<Sender<bool>> {
    WORKER.get_or_init(|| {
        let (tx, rx) = channel::<bool>();
        std::thread::Builder::new()
            .name("no-suspender".into())
            .spawn(move || {
                let mut cur = false;
                while let Ok(want) = rx.recv() {
                    if want != cur {
                        let ok = imp::request(want);
                        ACTIVE.store(want && ok, Ordering::SeqCst);
                        cur = want;
                    }
                }
                imp::request(false);
                ACTIVE.store(false, Ordering::SeqCst);
            })
            .ok();
        Mutex::new(tx)
    })
}

pub(crate) fn active() -> bool {
    ACTIVE.load(Ordering::SeqCst)
}

impl Daemon {
    fn awake_file(&self) -> PathBuf {
        self.data_dir.join("no-suspender.json")
    }

    pub(crate) fn keep_awake_enabled(&self) -> bool {
        std::fs::read_to_string(self.awake_file())
            .ok()
            .and_then(|t| serde_json::from_str::<Saved>(&t).ok())
            .map(|s| s.enabled)
            .unwrap_or(false)
    }

    pub(crate) fn set_keep_awake(&self, enabled: bool) -> Result<(), String> {
        std::fs::create_dir_all(&self.data_dir).map_err(|e| e.to_string())?;
        std::fs::write(
            self.awake_file(),
            serde_json::to_string_pretty(&Saved { enabled }).unwrap_or_default(),
        )
        .map_err(|e| e.to_string())?;
        self.keep_awake_tick();
        Ok(())
    }

    /// Se llama en cada vuelta del vigilante: pide no suspender solo si está activado y hay
    /// algún servidor encendido; si no, lo suelta.
    pub(crate) fn keep_awake_tick(&self) {
        let want = self.keep_awake_enabled() && self.any_server_running();
        if let Ok(tx) = worker().lock() {
            let _ = tx.send(want);
        }
    }

    fn any_server_running(&self) -> bool {
        let instances = self
            .store
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .list_instances()
            .unwrap_or_default();
        instances.iter().any(|i| {
            use merc_core::RuntimeState as S;
            matches!(
                self.sup.status(i.id).state,
                S::Validating | S::Starting | S::Loading | S::Online | S::Stopping
            )
        })
    }
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED};

    pub fn request(on: bool) -> bool {
        let flags = if on {
            ES_CONTINUOUS | ES_SYSTEM_REQUIRED
        } else {
            ES_CONTINUOUS
        };
        // SAFETY: llamada sin punteros; devuelve 0 si falla.
        unsafe { SetThreadExecutionState(flags) != 0 }
    }
}

/// Linux: `systemd-inhibit` (de systemd, presente en casi todas las distribuciones de
/// escritorio) toma un bloqueo «sleep:idle» mientras vive su proceso hijo. Al soltarlo se termina
/// ese proceso y el bloqueo desaparece. Si el sistema no tiene `systemd-inhibit`, se informa como
/// no vigente.
#[cfg(not(windows))]
mod imp {
    use std::process::{Child, Command, Stdio};
    use std::sync::Mutex;

    static HOLD: Mutex<Option<Child>> = Mutex::new(None);

    fn release(slot: &mut Option<Child>) {
        if let Some(mut c) = slot.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    pub fn request(on: bool) -> bool {
        let mut slot = HOLD.lock().unwrap_or_else(|p| p.into_inner());
        release(&mut slot);
        if !on {
            return true;
        }
        let child = Command::new("systemd-inhibit")
            .args([
                "--what=sleep:idle",
                "--who=MercHosting",
                "--why=Hay un servidor de Minecraft encendido",
                "--mode=block",
                "sleep",
                "infinity",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut c) = child else {
            return false;
        };
        // Si systemd-inhibit no consigue el bloqueo (sin logind, sin permiso…) termina enseguida.
        std::thread::sleep(std::time::Duration::from_millis(300));
        match c.try_wait() {
            Ok(None) => {
                *slot = Some(c);
                true
            }
            _ => false,
        }
    }
}
