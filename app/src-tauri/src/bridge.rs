//! Puente entre la interfaz (React) y el supervisor de MercHosting.
//!
//! La ventana no controla procesos Java: solo habla con el supervisor por el canal local
//! autenticado. Cada llamada abre una conexión corta (el canal es local y el saludo es barato),
//! así una operación larga (detener con guardado) no bloquea la consola ni la lista.

use merc_ipc::launcher::{connect_or_spawn, LaunchError};
use merc_ipc::{ClientError, JavaCandidate, RequestBody, ResponseBody};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Mientras se instala una versión nueva, la ventana no vuelve a lanzar el supervisor (el
/// instalador tiene que poder reemplazar su .exe).
pub static UPDATING: AtomicBool = AtomicBool::new(false);

const CLIENT_NAME: &str = concat!("merchosting-app ", env!("CARGO_PKG_VERSION"));

pub use crate::error::UiError;

pub fn ui_error_from_client(e: ClientError) -> UiError {
    match e {
        ClientError::Api(api) => {
            let code = serde_json::to_value(api.code)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string));
            UiError::new(code.as_deref().unwrap_or("internal"), api.message)
        }
        ClientError::Connect(_) | ClientError::Token(_) => UiError::new(
            "unavailable",
            "Se perdió la conexión con el supervisor de MercHosting. Se volverá a intentar automáticamente.",
        ),
        other => UiError::new("internal", other.to_string()),
    }
}

pub fn ui_error_from_launch(e: LaunchError) -> UiError {
    UiError::new("unavailable", e.to_string())
}

/// Carpeta de datos: `MERC_DATA_DIR` (pruebas) o la de siempre (`%LOCALAPPDATA%\MercHosting`).
pub fn data_dir() -> PathBuf {
    std::env::var_os("MERC_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(merc_ipc::default_data_dir)
}

fn blocking_call(body: RequestBody) -> Result<ResponseBody, UiError> {
    if UPDATING.load(Ordering::SeqCst) {
        return Err(UiError::new(
            "unavailable",
            "Instalando la versión nueva de MercHosting…",
        ));
    }
    let mut client =
        connect_or_spawn(&data_dir(), CLIENT_NAME, Duration::from_secs(20)).map_err(ui_error_from_launch)?;
    client.call(body).map_err(ui_error_from_client)
}

async fn call(body: RequestBody) -> Result<ResponseBody, UiError> {
    tauri::async_runtime::spawn_blocking(move || blocking_call(body))
        .await
        .map_err(|e| UiError::new("internal", e.to_string()))?
}

#[tauri::command]
pub async fn supervisor_call(body: RequestBody) -> Result<ResponseBody, UiError> {
    call(body).await
}

#[derive(Debug, Serialize)]
pub struct SupervisorInfo {
    pub server_version: String,
    pub supervisor_pid: u32,
    pub data_dir: PathBuf,
}

#[tauri::command]
pub async fn supervisor_info() -> Result<SupervisorInfo, UiError> {
    match call(RequestBody::Ping).await? {
        ResponseBody::Pong {
            server_version,
            supervisor_pid,
        } => Ok(SupervisorInfo {
            server_version,
            supervisor_pid,
            data_dir: data_dir(),
        }),
        _ => Err(UiError::new("internal", "Respuesta inesperada del supervisor")),
    }
}

#[derive(Debug, Serialize)]
pub struct JavaChoice {
    pub candidates: Vec<JavaCandidate>,
    pub recommended: Option<PathBuf>,
    pub recommended_major: u32,
}

/// Elige el Java recomendado con la misma regla que `merc` (merc_core::java::pick_java).
pub fn choose_java(candidates: Vec<JavaCandidate>, mc_version: Option<&str>) -> JavaChoice {
    let checked: Vec<(PathBuf, u32)> = candidates
        .iter()
        .filter_map(|c| c.major.map(|m| (c.path.clone(), m)))
        .collect();
    JavaChoice {
        recommended: merc_core::java::pick_java(&checked, mc_version).cloned(),
        recommended_major: merc_core::java::recommended_java_major(mc_version),
        candidates,
    }
}

#[tauri::command]
pub async fn java_for(mc_version: Option<String>) -> Result<JavaChoice, UiError> {
    match call(RequestBody::FindJava).await? {
        ResponseBody::JavaCandidates(list) => Ok(choose_java(list, mc_version.as_deref())),
        _ => Err(UiError::new("internal", "Respuesta inesperada del supervisor")),
    }
}

/// Ruta relativa del explorador de archivos, validada: solo nombres de carpeta normales.
pub fn safe_relative(rel: &str) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for part in rel.split(['/', '\\']).filter(|p| !p.is_empty()) {
        if part == "." || part == ".." || part.contains(':') {
            return None;
        }
        out.push(part);
    }
    Some(out)
}

/// Abre en el Explorador la carpeta del servidor (o una subcarpeta suya). La ruta sale del
/// supervisor por el identificador: la interfaz no puede pedir abrir cualquier carpeta.
#[tauri::command]
pub async fn open_server_folder(app: tauri::AppHandle, id: String, rel: Option<String>) -> Result<(), UiError> {
    use tauri_plugin_opener::OpenerExt;
    let list = match call(RequestBody::ListInstances).await? {
        ResponseBody::Instances(list) => list,
        _ => return Err(UiError::new("internal", "Respuesta inesperada del supervisor")),
    };
    let inst = list
        .into_iter()
        .find(|s| s.instance.id.to_string() == id)
        .ok_or_else(|| UiError::new("not_found", "No existe ese servidor"))?
        .instance;
    let sub = rel
        .as_deref()
        .and_then(safe_relative)
        .map(|r| inst.root.join(r))
        .filter(|p| p.is_dir() && !p.is_symlink());
    let target = sub.unwrap_or(inst.root);
    if !target.is_dir() {
        return Err(UiError::new("not_found", "La carpeta del servidor ya no existe"));
    }
    app.opener()
        .open_path(target.to_string_lossy(), None::<&str>)
        .map_err(|e| UiError::new("internal", format!("No se pudo abrir el Explorador: {e}")))
}

/// Abre en el Explorador la carpeta de copias de seguridad de un servidor (la indica el
/// supervisor; si todavía no existe, se crea vacía).
#[tauri::command]
pub async fn open_backups_folder(app: tauri::AppHandle, id: String) -> Result<(), UiError> {
    use tauri_plugin_opener::OpenerExt;
    let id: merc_core::InstanceId = serde_json::from_value(serde_json::Value::String(id))
        .map_err(|_| UiError::new("bad_request", "Servidor no válido"))?;
    let folder = match call(RequestBody::Backups { id }).await? {
        ResponseBody::Backups(l) => PathBuf::from(l.folder),
        _ => return Err(UiError::new("internal", "Respuesta inesperada del supervisor")),
    };
    std::fs::create_dir_all(&folder)
        .map_err(|e| UiError::new("internal", format!("No se pudo crear la carpeta de copias: {e}")))?;
    app.opener()
        .open_path(folder.to_string_lossy(), None::<&str>)
        .map_err(|e| UiError::new("internal", format!("No se pudo abrir el Explorador: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use merc_ipc::{ApiError, ErrorCode};

    #[test]
    fn api_errors_keep_their_code() {
        let e = ui_error_from_client(ClientError::Api(ApiError::new(ErrorCode::NoConsole, "sin consola")));
        assert_eq!(e, UiError::new("no_console", "sin consola"));
        let e = ui_error_from_client(ClientError::Connect(std::io::Error::other("x")));
        assert_eq!(e.code, "unavailable");
    }

    #[test]
    fn relative_paths_cannot_escape() {
        assert_eq!(safe_relative("mods/sub"), Some(PathBuf::from("mods").join("sub")));
        assert_eq!(safe_relative(""), Some(PathBuf::new()));
        assert_eq!(safe_relative("../x"), None);
        assert_eq!(safe_relative("a/../../x"), None);
        assert_eq!(safe_relative("C:/Windows"), None);
    }

    #[test]
    fn java_choice_prefers_recommended() {
        let c = |p: &str, m: Option<u32>| JavaCandidate {
            path: p.into(),
            major: m,
            description: String::new(),
            error: None,
            jdk: false,
        };
        let r = choose_java(
            vec![c("j25", Some(25)), c("j21", Some(21)), c("roto", None)],
            Some("1.21.1"),
        );
        assert_eq!(r.recommended, Some(PathBuf::from("j21")));
        assert_eq!(r.recommended_major, 21);
        assert_eq!(r.candidates.len(), 3);
        let r = choose_java(vec![c("j8", Some(8))], Some("1.21.1"));
        assert_eq!(r.recommended, None);
    }
}
