//! Ajustes de creación de procesos específicos de la plataforma.

use std::process::Command;

/// En Windows, evita que se abra una ventana de consola (CMD) para el proceso hijo.
#[cfg(windows)]
pub fn no_console_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub fn no_console_window(_cmd: &mut Command) {}
