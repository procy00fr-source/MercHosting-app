// En las versiones finales no se abre una consola junto a la ventana.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_update;
mod bridge;

fn main() {
    let builder = tauri::Builder::default();
    #[cfg(feature = "updater")]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            bridge::supervisor_call,
            bridge::supervisor_info,
            bridge::java_for,
            bridge::open_server_folder,
            bridge::open_backups_folder,
            app_update::app_update_check,
            app_update::app_update_install
        ])
        .run(tauri::generate_context!())
        .expect("no se pudo iniciar la ventana de MercHosting");
}
