//! Servidor casero sin pantalla: dejar el supervisor como servicio de usuario de systemd.
//!
//! `merc servicio instalar` escribe `~/.config/systemd/user/merchosting.service` y lo activa con
//! `systemctl --user` (no necesita permisos de administrador). Para que siga funcionando sin
//! que nadie inicie sesión hace falta «lingering», que sí pide sudo: no se ejecuta a escondidas,
//! se imprime el comando para que la persona lo ejecute si quiere.

use std::path::{Path, PathBuf};
use std::process::Command;

pub const UNIT_NAME: &str = "merchosting.service";

/// Carpeta de servicios de usuario: `$XDG_CONFIG_HOME/systemd/user` o `~/.config/systemd/user`.
pub fn unit_dir(xdg_config_home: Option<&Path>, home: &Path) -> PathBuf {
    xdg_config_home
        .filter(|p| p.is_absolute())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| home.join(".config"))
        .join("systemd")
        .join("user")
}

/// Un argumento de `ExecStart` entre comillas, con `\`, `"`, `%` y `$` escapados (systemd.service).
pub fn quote(arg: &str) -> String {
    let mut s = String::from('"');
    for c in arg.chars() {
        match c {
            '\\' => s.push_str("\\\\"),
            '"' => s.push_str("\\\""),
            '%' => s.push_str("%%"),
            '$' => s.push_str("$$"),
            c => s.push(c),
        }
    }
    s.push('"');
    s
}

/// Texto del servicio. `Restart=on-failure`: si el supervisor se cae, systemd lo vuelve a abrir.
/// `--al-iniciar-sesion` enciende los servidores marcados con «Encender al iniciar».
pub fn unit_text(supervisord: &Path, data_dir: &Path) -> String {
    format!(
        "# Creado por «merc servicio instalar» (MercHosting). Quítalo con «merc servicio quitar».\n\
[Unit]\n\
Description=MercHosting (supervisor de servidores de Minecraft)\n\
After=network-online.target\n\
Wants=network-online.target\n\
\n\
[Service]\n\
Type=simple\n\
ExecStart={} --data-dir {} --al-iniciar-sesion\n\
Restart=on-failure\n\
RestartSec=10\n\
# Al parar el servicio, el supervisor recibe SIGTERM; los servidores siguen su propio cierre.\n\
KillMode=process\n\
\n\
[Install]\n\
WantedBy=default.target\n",
        quote(&supervisord.to_string_lossy()),
        quote(&data_dir.to_string_lossy()),
    )
}

/// `Linger=yes` en la salida de `loginctl show-user <usuario> -p Linger`.
pub fn linger_on(loginctl_output: &str) -> bool {
    loginctl_output.lines().any(|l| l.trim() == "Linger=yes")
}

/// Nombre del usuario actual (para los mensajes y `loginctl`).
pub fn current_user() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "tu-usuario".into())
}

/// Ejecuta un programa y devuelve (¿salió bien?, salida estándar + error).
pub fn run(program: &str, args: &[&str]) -> Option<(bool, String)> {
    let out = Command::new(program).args(args).output().ok()?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    Some((out.status.success(), text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_paths() {
        assert_eq!(
            unit_dir(None, Path::new("/home/amir")),
            PathBuf::from("/home/amir/.config/systemd/user")
        );
        assert_eq!(
            unit_dir(Some(Path::new("/x/cfg")), Path::new("/home/amir")),
            PathBuf::from("/x/cfg/systemd/user")
        );
        // Un XDG_CONFIG_HOME relativo no vale (la especificación lo manda ignorar).
        assert_eq!(
            unit_dir(Some(Path::new("cfg")), Path::new("/home/amir")),
            PathBuf::from("/home/amir/.config/systemd/user")
        );
    }

    #[test]
    fn unit_quotes_paths() {
        let t = unit_text(
            Path::new("/home/a b/MercHosting/merc-supervisord"),
            Path::new("/home/a b/.local/share/MercHosting 100%"),
        );
        assert!(t.contains(
            "ExecStart=\"/home/a b/MercHosting/merc-supervisord\" --data-dir \"/home/a b/.local/share/MercHosting 100%%\" --al-iniciar-sesion"
        ));
        assert!(t.contains("Restart=on-failure"));
        assert!(t.contains("WantedBy=default.target"));
        assert_eq!(quote("a\"b$c\\"), "\"a\\\"b$$c\\\\\"");
    }

    #[test]
    fn linger_detection() {
        assert!(linger_on("Linger=yes\n"));
        assert!(!linger_on("Linger=no\n"));
        assert!(!linger_on(""));
    }
}
