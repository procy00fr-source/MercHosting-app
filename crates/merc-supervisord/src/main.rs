// En Windows el supervisor no abre ninguna ventana ni consola.
#![cfg_attr(windows, windows_subsystem = "windows")]

//! `merc-supervisord [--data-dir <carpeta>]`
//!
//! Lo arranca la aplicación (o la herramienta `merc`); no está pensado para que el usuario lo
//! ejecute a mano. Su registro de actividad está en `<carpeta de datos>/supervisor.log`.

fn main() {
    let mut args = std::env::args().skip(1);
    let mut data_dir = merc_ipc::default_data_dir();
    let mut at_login = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--data-dir" => match args.next() {
                Some(d) => data_dir = d.into(),
                None => std::process::exit(2),
            },
            // Lo pone el arranque con el sistema al iniciar sesión (registro de Windows o el
            // .desktop de ~/.config/autostart en Linux).
            "--al-iniciar-windows" | "--al-iniciar-sesion" => at_login = true,
            "--version" => {
                println!("merc-supervisord {}", merc_supervisord::VERSION);
                return;
            }
            _ => std::process::exit(2),
        }
    }
    match merc_supervisord::run_with(data_dir, at_login) {
        Ok(merc_supervisord::RunOutcome::AlreadyRunning) => {}
        Err(e) => {
            eprintln!("merc-supervisord: {e}");
            std::process::exit(1);
        }
    }
}
