//! Aplicación de MercHosting (Tauri 2): escritorio (Windows y Linux) y celular (Android).
//!
//! - Escritorio: la ventana habla con el supervisor local (`bridge`) y se actualiza sola
//!   (`app_update`, solo en versiones publicadas).
//! - Celular: no hay supervisor; la app es un compañero que administra tu PC o tu servidor
//!   casero por el canal de compañeros (`mobile`), y en Android puede además encender un
//!   servidor Java dentro del propio celular (`phone`).

mod error;

#[cfg(desktop)]
mod app_update;
#[cfg(desktop)]
mod bridge;
#[cfg(mobile)]
mod mobile;
#[cfg(mobile)]
mod phone;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(all(desktop, feature = "updater"))]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    let builder = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init());
    #[cfg(desktop)]
    let builder = builder.invoke_handler(tauri::generate_handler![
        bridge::supervisor_call,
        bridge::supervisor_info,
        bridge::java_for,
        bridge::open_server_folder,
        bridge::open_backups_folder,
        app_update::app_update_check,
        app_update::app_update_install
    ]);
    #[cfg(mobile)]
    let builder = builder
        .plugin(tauri_plugin_mercphone::init())
        .manage(phone::PhoneState::new())
        .setup(mobile::setup)
        .invoke_handler(tauri::generate_handler![
            mobile::mobile_call,
            mobile::mobile_identity,
            phone::phone_call
        ]);
    builder
        .run(tauri::generate_context!())
        .expect("no se pudo iniciar la ventana de MercHosting");
}
