//! MercHosting en el celular: no hay supervisor ni servidores propios. La app es un compañero
//! más (canal iroh cifrado, código de invitación) que administra los servidores de tu PC o de
//! tu servidor casero. Lo que se permite lo decide siempre esa PC; aquí no se abre ningún puerto
//! ni se atienden conexiones entrantes.

use crate::error::UiError;
use merc_ipc::{RequestBody, ResponseBody};
use merc_remote::guest::Guest;
use merc_remote::Mode;
use std::sync::Arc;
use tauri::{Manager, State};

pub struct MobileState(pub Arc<Guest>);

/// Prepara el compañero con la carpeta privada de la app (Android la protege del resto de apps).
pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let dir = app.path().app_data_dir()?.join("companeros");
    app.manage(MobileState(Arc::new(Guest::new(dir, Mode::Internet))));
    Ok(())
}

/// Lo que la interfaz pide (unirse, PCs guardadas, peticiones a una PC).
#[tauri::command]
pub async fn mobile_call(state: State<'_, MobileState>, body: RequestBody) -> Result<ResponseBody, UiError> {
    let g = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || g.handle(body))
        .await
        .map_err(|e| UiError::new("internal", e.to_string()))?
        .map_err(UiError::from)
}

/// Huella de este celular (la que aparece en la lista de compañeros de la PC).
#[tauri::command]
pub async fn mobile_identity(state: State<'_, MobileState>) -> Result<String, UiError> {
    let g = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || g.fingerprint())
        .await
        .map_err(|e| UiError::new("internal", e.to_string()))?
        .map_err(UiError::from)
}
