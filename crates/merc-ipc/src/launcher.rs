//! Arranque del supervisor desde la aplicación o la herramienta `merc`, y conexión con espera.
//!
//! Se comparte para que todos los clientes lancen el supervisor de la misma forma (con las
//! correcciones específicas de Windows probadas en el incremento 2).

use crate::{Client, ClientError};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Nombre del ejecutable del supervisor en esta plataforma.
pub fn supervisor_exe_name() -> &'static str {
    if cfg!(windows) {
        "merc-supervisord.exe"
    } else {
        "merc-supervisord"
    }
}

/// Busca el supervisor: variable `MERC_SUPERVISORD_PATH` (desarrollo) o junto al ejecutable actual.
pub fn find_supervisor_exe() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("MERC_SUPERVISORD_PATH") {
        let p = PathBuf::from(p);
        return p.is_file().then_some(p);
    }
    let me = std::env::current_exe().ok()?;
    let p = me.parent()?.join(supervisor_exe_name());
    p.is_file().then_some(p)
}

/// En Windows, `CreateProcess` hereda TODOS los handles heredables del proceso padre, no solo los
/// de stdio. Si el cliente tiene su salida conectada a un tubo (PowerShell, scripts), el
/// supervisor heredaría ese tubo y lo mantendría abierto para siempre. Se marcan los handles
/// estándar propios como no heredables antes de lanzar.
#[cfg(windows)]
fn stop_inheriting_std_handles() {
    use windows_sys::Win32::Foundation::{SetHandleInformation, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE};
    for which in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        // SAFETY: GetStdHandle no tiene precondiciones; SetHandleInformation solo cambia el
        // indicador de herencia de un handle válido que pertenece a este proceso.
        unsafe {
            let h = GetStdHandle(which);
            if !h.is_null() && h != INVALID_HANDLE_VALUE {
                SetHandleInformation(h, HANDLE_FLAG_INHERIT, 0);
            }
        }
    }
}

/// Lanza el supervisor separado del proceso actual: sobrevive al cierre de la ventana o terminal.
pub fn spawn_supervisor(exe: &Path, data_dir: &Path) -> io::Result<()> {
    let mut cmd = Command::new(exe);
    cmd.arg("--data-dir")
        .arg(data_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        stop_inheriting_std_handles();
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        // Salir del Job Object del proceso actual (si Windows lo permite) para que cerrarlo no
        // termine también el supervisor.
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB);
        if cmd.spawn().is_ok() {
            return Ok(());
        }
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn().map(|_| ())
}

#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("no se encontró el supervisor de MercHosting ({0}). Reinstala la aplicación")]
    MissingExe(String),
    #[error("no se pudo iniciar el supervisor: {0}")]
    Spawn(io::Error),
    #[error("el supervisor no respondió a tiempo: {0}")]
    Timeout(ClientError),
}

/// Se conecta al supervisor; si no está en marcha, lo inicia y espera hasta `timeout`.
pub fn connect_or_spawn(data_dir: &Path, client_name: &str, timeout: Duration) -> Result<Client, LaunchError> {
    if let Ok(c) = Client::connect(data_dir, client_name) {
        return Ok(c);
    }
    let exe = find_supervisor_exe().ok_or_else(|| LaunchError::MissingExe(supervisor_exe_name().into()))?;
    spawn_supervisor(&exe, data_dir).map_err(LaunchError::Spawn)?;
    let deadline = Instant::now() + timeout;
    loop {
        match Client::connect(data_dir, client_name) {
            Ok(c) => return Ok(c),
            Err(e) if Instant::now() >= deadline => return Err(LaunchError::Timeout(e)),
            Err(_) => std::thread::sleep(Duration::from_millis(150)),
        }
    }
}
