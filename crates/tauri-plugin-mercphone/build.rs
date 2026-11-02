// Las órdenes se llaman desde Rust (la app), no desde la interfaz: no se exponen comandos.
const COMMANDS: &[&str] = &[];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).android_path("android").build();
}
