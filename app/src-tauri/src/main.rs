// En las versiones finales no se abre una consola junto a la ventana.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    merchosting_app_lib::run()
}
