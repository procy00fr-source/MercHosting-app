//! Fase 2: servidores Java dentro de ESTE celular (solo Android arm64). Toda la lógica está en
//! `merc_phone` (probada en Linux); aquí solo se pasa la petición y se le pide a Android que
//! arranque el servicio en primer plano (`tauri-plugin-mercphone`).

use crate::error::UiError;
use merc_phone::api::{Api, Host, Op};
use merc_phone::local::Phone;
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Manager, Runtime};

pub struct PhoneState(OnceLock<Arc<Api>>);

impl PhoneState {
    pub fn new() -> Self {
        Self(OnceLock::new())
    }
}

#[cfg(target_os = "android")]
struct AndroidHost<R: Runtime>(AppHandle<R>);

#[cfg(target_os = "android")]
impl<R: Runtime> Host for AndroidHost<R> {
    fn start_service(&self, id: &str, name: &str, spec_json: &str, fifo: &str) -> Result<(), String> {
        self.0
            .state::<tauri_plugin_mercphone::Phone<R>>()
            .start_server(tauri_plugin_mercphone::StartRequest {
                id: id.into(),
                name: name.into(),
                spec: spec_json.into(),
                fifo: fifo.into(),
            })
    }

    fn force_stop(&self) -> Result<(), String> {
        self.0.state::<tauri_plugin_mercphone::Phone<R>>().force_stop()
    }
}

/// En iOS no hay servidor dentro del celular.
#[cfg(not(target_os = "android"))]
struct NoHost;

#[cfg(not(target_os = "android"))]
impl Host for NoHost {
    fn start_service(&self, _: &str, _: &str, _: &str, _: &str) -> Result<(), String> {
        Err("En este sistema no se puede encender un servidor dentro del celular.".into())
    }
    fn force_stop(&self) -> Result<(), String> {
        Ok(())
    }
}

fn api<R: Runtime>(app: &AppHandle<R>) -> Result<Arc<Api>, UiError> {
    let state = app.state::<PhoneState>();
    if let Some(a) = state.0.get() {
        return Ok(a.clone());
    }
    let data = app
        .path()
        .app_data_dir()
        .map_err(|e| UiError::new("internal", e.to_string()))?
        .join("celular");
    #[cfg(target_os = "android")]
    let phone = {
        let info = app
            .state::<tauri_plugin_mercphone::Phone<R>>()
            .device_info()
            .map_err(|e| UiError::new("android", e))?;
        let native = (!info.native_library_dir.is_empty()).then(|| std::path::PathBuf::from(info.native_library_dir));
        Phone::new(data, &info.abi, info.total_ram_mb, native)
    };
    #[cfg(not(target_os = "android"))]
    let phone = Phone::new(data, std::env::consts::ARCH, 0, None);
    let a = Arc::new(Api::new(
        phone,
        merc_install::Http::new(),
        merc_install::Sources::default(),
    ));
    let _ = state.0.set(a.clone());
    Ok(state.0.get().cloned().unwrap_or(a))
}

/// Todo lo de «En este celular» pasa por aquí (`op` dice qué).
#[tauri::command]
pub async fn phone_call<R: Runtime>(app: AppHandle<R>, op: serde_json::Value) -> Result<serde_json::Value, UiError> {
    let op: Op = serde_json::from_value(op).map_err(|e| UiError::new("invalid_request", e.to_string()))?;
    let api = api(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "android")]
        let host = AndroidHost(app.clone());
        #[cfg(not(target_os = "android"))]
        let host = NoHost;
        api.handle(&host, op).map_err(|e| UiError::new(e.code, e.message))
    })
    .await
    .map_err(|e| UiError::new("internal", e.to_string()))?
}

/// Llamada del servicio `ServerService` (proceso `:server`): lanza la JVM con la especificación.
/// Bloquea mientras el servidor corre.
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_app_merchosting_phone_MercNative_launch<'local>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
    spec: jni::objects::JString<'local>,
) -> jni::sys::jint {
    let json: String = match env.get_string(&spec) {
        Ok(s) => s.into(),
        Err(_) => return 97,
    };
    merc_phone::jli::launch_from_json(&json)
}
