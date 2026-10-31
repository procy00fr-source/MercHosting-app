//! Canal local entre procesos del mismo usuario.
//!
//! - Windows: named pipe `\\.\pipe\MercHosting.Supervisor.v1.<usuario>.<hash>`, que solo acepta
//!   clientes locales (`PIPE_REJECT_REMOTE_CLIENTS`). Su ACL concede acceso solo al
//!   SYSTEM y al propietario (el usuario que arrancó el supervisor).
//! - Linux/macOS (desarrollo y pruebas): socket de dominio Unix dentro de la carpeta de datos.
//!
//! Además, cada conexión debe presentar el token de `supervisor.token` (ver [`crate::token`]).
//! Nunca se abre un puerto TCP ni nada accesible desde la red.

use interprocess::local_socket::{prelude::*, Listener, ListenerOptions, Stream};
use std::io;
use std::path::{Path, PathBuf};

/// Dirección del supervisor derivada de la carpeta de datos: dos carpetas de datos distintas
/// (por ejemplo, una de pruebas) nunca comparten canal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    #[cfg(windows)]
    pipe_name: String,
    #[cfg(not(windows))]
    socket_path: PathBuf,
}

/// FNV-1a de 64 bits: estable entre versiones y plataformas (no criptográfico; solo separa nombres).
fn fnv1a(text: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in text.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn sanitize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(32)
        .collect()
}

impl Endpoint {
    pub fn for_data_dir(data_dir: &Path) -> Self {
        #[cfg(windows)]
        {
            let key = data_dir
                .to_string_lossy()
                .replace('/', "\\")
                .trim_end_matches('\\')
                .to_lowercase();
            let user = sanitize(&std::env::var("USERNAME").unwrap_or_default());
            Self {
                pipe_name: format!("MercHosting.Supervisor.v1.{user}.{:016x}", fnv1a(&key)),
            }
        }
        #[cfg(not(windows))]
        {
            let _ = (fnv1a, sanitize);
            Self {
                socket_path: data_dir.join("supervisor.sock"),
            }
        }
    }

    pub fn describe(&self) -> String {
        #[cfg(windows)]
        {
            format!(r"\\.\pipe\{}", self.pipe_name)
        }
        #[cfg(not(windows))]
        {
            self.socket_path.display().to_string()
        }
    }

    pub fn connect(&self) -> io::Result<Stream> {
        #[cfg(windows)]
        {
            use interprocess::local_socket::GenericNamespaced;
            Stream::connect(self.pipe_name.as_str().to_ns_name::<GenericNamespaced>()?)
        }
        #[cfg(not(windows))]
        {
            use interprocess::local_socket::GenericFilePath;
            Stream::connect(self.socket_path.as_path().to_fs_name::<GenericFilePath>()?)
        }
    }

    /// Crea el canal de escucha. Falla si otro proceso ya lo posee.
    pub fn listen(&self) -> io::Result<Listener> {
        #[cfg(windows)]
        {
            use interprocess::local_socket::GenericNamespaced;
            use interprocess::os::windows::local_socket::ListenerOptionsExt;
            use interprocess::os::windows::security_descriptor::SecurityDescriptor;
            // D:P = DACL protegida (sin herencia). GA = acceso total.
            // SY = LocalSystem; OW = "Owner Rights", el propietario del pipe (quien lo crea).
            // Nadie más (ni otros usuarios ni "Everyone") puede abrirlo.
            let sddl = widestring::U16CString::from_str("D:P(A;;GA;;;SY)(A;;GA;;;OW)").map_err(io::Error::other)?;
            let sd = SecurityDescriptor::deserialize(&sddl)?;
            ListenerOptions::new()
                .name(self.pipe_name.as_str().to_ns_name::<GenericNamespaced>()?)
                .security_descriptor(sd)
                .create_sync()
        }
        #[cfg(not(windows))]
        {
            use interprocess::local_socket::GenericFilePath;
            let l = ListenerOptions::new()
                .name(self.socket_path.as_path().to_fs_name::<GenericFilePath>()?)
                .create_sync()?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&self.socket_path, std::fs::Permissions::from_mode(0o600))?;
            }
            Ok(l)
        }
    }

    /// Elimina un socket Unix abandonado (no aplica a Windows: el pipe desaparece con su proceso).
    pub fn remove_stale(&self) {
        #[cfg(not(windows))]
        {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }

    #[cfg(not(windows))]
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

/// Carpeta de datos por defecto de MercHosting.
/// Windows: `%LOCALAPPDATA%\MercHosting` (privada del usuario por los permisos del perfil).
pub fn default_data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(d) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(d).join("MercHosting");
        }
    }
    #[cfg(not(windows))]
    {
        if let Some(d) = std::env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(d).join("MercHosting");
        }
        if let Some(h) = std::env::var_os("HOME") {
            return PathBuf::from(h).join(".local/share/MercHosting");
        }
    }
    std::env::temp_dir().join("MercHosting")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_depends_on_data_dir() {
        let a = Endpoint::for_data_dir(Path::new("/datos/a"));
        let b = Endpoint::for_data_dir(Path::new("/datos/b"));
        assert_ne!(a, b);
        assert_eq!(a, Endpoint::for_data_dir(Path::new("/datos/a")));
    }

    #[test]
    fn fnv_is_stable() {
        assert_eq!(fnv1a(""), 0xcbf29ce484222325);
        assert_eq!(fnv1a("a"), 0xaf63dc4c8601ec8c);
    }
}
