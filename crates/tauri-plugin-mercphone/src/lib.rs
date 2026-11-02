//! Parte nativa de Android para correr un servidor Java dentro del celular.
//!
//! - `deviceInfo`: memoria total, arquitectura (ABI) y carpeta de bibliotecas nativas (donde va
//!   el agente de Playit, `libplayit.so`).
//! - `startServer`: arranca `ServerService`, un servicio en primer plano en su propio proceso
//!   (`:server`) con notificación permanente y botón «Detener». Ese proceso carga la biblioteca
//!   de la app y llama a `merc_phone::jli::launch_from_json` por JNI (`MercNative.launch`).
//! - `forceStop`: cierra ese proceso si el servidor no responde (puede perder lo no guardado).
//!
//! En escritorio este plugin no hace nada.

use serde::{Deserialize, Serialize};
use tauri::{plugin::TauriPlugin, Runtime};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "app.merchosting.phone";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub total_ram_mb: u64,
    pub abi: String,
    pub native_library_dir: String,
    pub sdk: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRequest {
    pub id: String,
    pub name: String,
    /// `LaunchSpec` en JSON (lo entiende `merc_phone::jli`).
    pub spec: String,
    /// Tubería de comandos (para el botón «Detener» de la notificación).
    pub fifo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Empty {}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Running {
    pub running: bool,
}

#[cfg(target_os = "android")]
pub struct Phone<R: Runtime>(tauri::plugin::PluginHandle<R>);

#[cfg(target_os = "android")]
impl<R: Runtime> Phone<R> {
    pub fn device_info(&self) -> Result<DeviceInfo, String> {
        self.0
            .run_mobile_plugin("deviceInfo", Empty {})
            .map_err(|e| e.to_string())
    }

    pub fn start_server(&self, req: StartRequest) -> Result<(), String> {
        self.0
            .run_mobile_plugin::<Empty>("startServer", req)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn force_stop(&self) -> Result<(), String> {
        self.0
            .run_mobile_plugin::<Empty>("forceStop", Empty {})
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn service_running(&self) -> Result<bool, String> {
        self.0
            .run_mobile_plugin::<Running>("serviceRunning", Empty {})
            .map(|r| r.running)
            .map_err(|e| e.to_string())
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("mercphone")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                use tauri::Manager;
                let handle = _api.register_android_plugin(PLUGIN_IDENTIFIER, "MercPhonePlugin")?;
                _app.manage(Phone(handle));
            }
            Ok(())
        })
        .build()
}
