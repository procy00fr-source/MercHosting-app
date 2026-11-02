//! Actualizar MercHosting (la propia app) con el actualizador oficial de Tauri.
//!
//! - Solo existe en las versiones publicadas (característica `updater`): la clave pública y la
//!   dirección de `latest.json` (GitHub Releases) las pone `scripts/publicar-version.mjs`.
//! - Cada instalador va firmado con la clave privada del desarrollador; el actualizador rechaza
//!   lo que no esté firmado con ella o lo que diga una versión distinta a la firmada.
//! - Antes de instalar se cierra el supervisor (que se niega si hay servidores en marcha), así
//!   el instalador puede reemplazar su .exe y ningún mundo queda a medio guardar.

use crate::error::UiError;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AppUpdateCheck {
    /// `false` en compilaciones de desarrollo (sin actualizador).
    pub enabled: bool,
    pub current: String,
    pub available: bool,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub date: Option<String>,
    /// La app se puede actualizar sola: en Windows (instalador) y en Linux solo con AppImage.
    /// Con el paquete .deb se descarga la versión nueva a mano.
    pub self_install: bool,
}

/// En Linux el actualizador de Tauri reemplaza el AppImage; un .deb instalado no se toca.
pub fn self_install_supported() -> bool {
    cfg!(windows) || std::env::var_os("APPIMAGE").is_some()
}

#[cfg(not(feature = "updater"))]
#[tauri::command]
pub async fn app_update_check() -> Result<AppUpdateCheck, UiError> {
    Ok(AppUpdateCheck {
        enabled: false,
        current: env!("CARGO_PKG_VERSION").into(),
        available: false,
        version: None,
        notes: None,
        date: None,
        self_install: self_install_supported(),
    })
}

#[cfg(not(feature = "updater"))]
#[tauri::command]
pub async fn app_update_install() -> Result<(), UiError> {
    Err(UiError::new(
        "bad_request",
        "Esta es una compilación de desarrollo: se actualiza compilando de nuevo.",
    ))
}

#[cfg(feature = "updater")]
pub use real::*;

#[cfg(feature = "updater")]
mod real {
    use super::*;
    use crate::bridge::{data_dir, UPDATING};
    use merc_ipc::{Client, ClientError, RequestBody};
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};
    use tauri::{AppHandle, Emitter};
    use tauri_plugin_updater::UpdaterExt;

    fn err(e: impl std::fmt::Display) -> UiError {
        UiError::new("network", format!("No se pudo revisar las actualizaciones: {e}"))
    }

    #[tauri::command]
    pub async fn app_update_check(app: AppHandle) -> Result<AppUpdateCheck, UiError> {
        let current = app.package_info().version.to_string();
        let update = app.updater().map_err(err)?.check().await.map_err(err)?;
        Ok(AppUpdateCheck {
            enabled: true,
            current,
            available: update.is_some(),
            version: update.as_ref().map(|u| u.version.clone()),
            notes: update.as_ref().and_then(|u| u.body.clone()),
            date: update.as_ref().and_then(|u| u.date.map(|d| d.to_string())),
            self_install: self_install_supported(),
        })
    }

    /// Cierra el supervisor si está en marcha. Falla (con su mensaje) si hay servidores encendidos.
    fn stop_supervisor() -> Result<(), UiError> {
        let dir = data_dir();
        let mut c = match Client::connect(&dir, "merchosting-app (actualización)") {
            Ok(c) => c,
            Err(_) => return Ok(()), // No está en marcha.
        };
        c.call(RequestBody::Shutdown)
            .map_err(crate::bridge::ui_error_from_client)?;
        drop(c);
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            match Client::connect(&dir, "merchosting-app (actualización)") {
                Err(ClientError::Connect(_)) | Err(ClientError::Token(_)) => return Ok(()),
                _ => std::thread::sleep(Duration::from_millis(250)),
            }
        }
        Err(UiError::new(
            "internal",
            "El supervisor no terminó de cerrarse. Inténtalo otra vez.",
        ))
    }

    #[derive(Clone, Serialize)]
    struct Progress {
        downloaded: u64,
        total: Option<u64>,
    }

    #[tauri::command]
    pub async fn app_update_install(app: AppHandle) -> Result<(), UiError> {
        if !self_install_supported() {
            return Err(UiError::new(
                "bad_request",
                "Esta instalación (.deb) no se actualiza sola: descarga la versión nueva desde la página de releases.",
            ));
        }
        let update = app
            .updater()
            .map_err(err)?
            .check()
            .await
            .map_err(err)?
            .ok_or_else(|| UiError::new("not_found", "Ya tienes la versión más nueva."))?;
        UPDATING.store(true, Ordering::SeqCst);
        let stopped = tauri::async_runtime::spawn_blocking(stop_supervisor)
            .await
            .map_err(|e| UiError::new("internal", e.to_string()))
            .and_then(|r| r);
        if let Err(e) = stopped {
            UPDATING.store(false, Ordering::SeqCst);
            return Err(e);
        }
        let mut downloaded = 0u64;
        let r = update
            .download_and_install(
                |chunk, total| {
                    downloaded += chunk as u64;
                    let _ = app.emit("app-update-progress", Progress { downloaded, total });
                },
                || {},
            )
            .await;
        // En Windows el instalador ya tomó el control y la app se cerró antes de llegar aquí.
        UPDATING.store(false, Ordering::SeqCst);
        match r {
            Ok(()) => {
                app.restart();
            }
            Err(e) => Err(UiError::new(
                "network",
                format!("No se pudo instalar la actualización (no se cambió nada): {e}"),
            )),
        }
    }
}
