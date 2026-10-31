//! Comunicación local y autenticada entre la interfaz de MercHosting y su supervisor.

pub mod launcher;
pub mod protocol;
pub mod remote;
pub mod transport;

pub use protocol::*;
pub use transport::{default_data_dir, Endpoint};

use interprocess::local_socket::Stream;
use serde::{de::DeserializeOwned, Serialize};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

pub const TOKEN_FILE: &str = "supervisor.token";

// ---------------------------------------------------------------------------------------------
// Token de sesión
// ---------------------------------------------------------------------------------------------

/// Genera un token aleatorio de 256 bits (dos UUID v4, que usan el generador seguro del SO).
pub fn generate_token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

pub fn token_path(data_dir: &Path) -> PathBuf {
    data_dir.join(TOKEN_FILE)
}

/// Escribe el token. En Windows, `%LOCALAPPDATA%` ya es privado del usuario; en Unix se fuerza 0600.
pub fn write_token(data_dir: &Path, token: &str) -> io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    let path = token_path(data_dir);
    let tmp = data_dir.join(format!("{TOKEN_FILE}.tmp"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(token.as_bytes())?;
    }
    #[cfg(not(unix))]
    std::fs::write(&tmp, token)?;
    std::fs::rename(&tmp, &path)
}

pub fn read_token(data_dir: &Path) -> io::Result<String> {
    Ok(std::fs::read_to_string(token_path(data_dir))?.trim().to_string())
}

/// Comparación en tiempo constante para no filtrar el token por diferencias de tiempo.
pub fn token_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

// ---------------------------------------------------------------------------------------------
// Mensajes: una línea JSON con límite de tamaño
// ---------------------------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("conexión cerrada")]
    Closed,
    #[error("mensaje demasiado grande")]
    TooLarge,
    #[error("mensaje no válido: {0}")]
    Invalid(String),
    #[error("error de E/S: {0}")]
    Io(#[from] io::Error),
}

pub fn write_message<W: Write, T: Serialize>(w: &mut W, msg: &T) -> Result<(), FrameError> {
    let mut bytes = serde_json::to_vec(msg).map_err(|e| FrameError::Invalid(e.to_string()))?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(FrameError::TooLarge);
    }
    bytes.push(b'\n');
    w.write_all(&bytes)?;
    w.flush()?;
    Ok(())
}

pub fn read_message<R: BufRead, T: DeserializeOwned>(r: &mut R) -> Result<T, FrameError> {
    let mut buf = Vec::new();
    let n = r
        .by_ref()
        .take(MAX_MESSAGE_BYTES as u64 + 1)
        .read_until(b'\n', &mut buf)?;
    if n == 0 {
        return Err(FrameError::Closed);
    }
    if buf.last() != Some(&b'\n') {
        return Err(if buf.len() > MAX_MESSAGE_BYTES {
            FrameError::TooLarge
        } else {
            FrameError::Closed
        });
    }
    serde_json::from_slice(&buf).map_err(|e| FrameError::Invalid(e.to_string()))
}

// ---------------------------------------------------------------------------------------------
// Cliente
// ---------------------------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("el supervisor no está en ejecución o no se puede conectar ({0})")]
    Connect(io::Error),
    #[error("no se pudo leer el token del supervisor ({0})")]
    Token(io::Error),
    #[error("{0}")]
    Frame(#[from] FrameError),
    #[error("{0}")]
    Api(ApiError),
    #[error("respuesta inesperada del supervisor")]
    Unexpected,
}

pub struct Client {
    reader: BufReader<Stream>,
    seq: u64,
    pub welcome: Welcome,
}

impl Client {
    pub fn connect_with_token(endpoint: &Endpoint, token: &str, client_name: &str) -> Result<Self, ClientError> {
        Self::connect_raw(endpoint, token, PROTOCOL_VERSION, client_name)
    }

    /// Lee el token de la carpeta de datos y se conecta.
    pub fn connect(data_dir: &Path, client_name: &str) -> Result<Self, ClientError> {
        let endpoint = Endpoint::for_data_dir(data_dir);
        let token = read_token(data_dir).map_err(ClientError::Token)?;
        Self::connect_with_token(&endpoint, &token, client_name)
    }

    /// Conexión con versión de protocolo explícita (útil para pruebas de compatibilidad).
    pub fn connect_raw(
        endpoint: &Endpoint,
        token: &str,
        protocol: u32,
        client_name: &str,
    ) -> Result<Self, ClientError> {
        let stream = endpoint.connect().map_err(ClientError::Connect)?;
        let mut reader = BufReader::new(stream);
        write_message(
            reader.get_mut(),
            &Hello {
                protocol,
                token: token.to_string(),
                client: client_name.to_string(),
            },
        )?;
        match read_message::<_, HelloReply>(&mut reader)? {
            HelloReply::Welcome(welcome) => Ok(Self {
                reader,
                seq: 0,
                welcome,
            }),
            HelloReply::Rejected { error } => Err(ClientError::Api(error)),
        }
    }

    pub fn call(&mut self, body: RequestBody) -> Result<ResponseBody, ClientError> {
        self.seq += 1;
        let seq = self.seq;
        write_message(self.reader.get_mut(), &Request { seq, body })?;
        let resp: Response = read_message(&mut self.reader)?;
        if resp.seq != seq {
            return Err(ClientError::Unexpected);
        }
        match resp.result {
            ResponseResult::Ok(b) => Ok(b),
            ResponseResult::Err(e) => Err(ClientError::Api(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_compare() {
        let t = generate_token();
        assert_eq!(t.len(), 64);
        assert!(token_eq(&t, &t.clone()));
        assert!(!token_eq(&t, &generate_token()));
        assert!(!token_eq(&t, &t[..63]));
    }

    #[test]
    fn framing_limits() {
        let mut out = Vec::new();
        write_message(&mut out, &RequestBody::Ping).unwrap();
        let mut r = BufReader::new(&out[..]);
        assert_eq!(read_message::<_, RequestBody>(&mut r).unwrap(), RequestBody::Ping);
        assert!(matches!(
            read_message::<_, RequestBody>(&mut r),
            Err(FrameError::Closed)
        ));

        let huge = vec![b'a'; MAX_MESSAGE_BYTES + 10];
        let mut r = BufReader::new(&huge[..]);
        assert!(matches!(
            read_message::<_, RequestBody>(&mut r),
            Err(FrameError::TooLarge)
        ));

        let mut r = BufReader::new(&b"{basura}\n"[..]);
        assert!(matches!(
            read_message::<_, RequestBody>(&mut r),
            Err(FrameError::Invalid(_))
        ));
    }

    #[test]
    fn token_file_roundtrip() {
        let d = tempfile::tempdir().unwrap();
        write_token(d.path(), "abc").unwrap();
        assert_eq!(read_token(d.path()).unwrap(), "abc");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(token_path(d.path())).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    /// Prueba el canal real del sistema (named pipe en Windows con su ACL; socket Unix aquí).
    #[test]
    fn real_endpoint_listen_and_connect() {
        let d = tempfile::tempdir().unwrap();
        let ep = Endpoint::for_data_dir(d.path());
        let listener = ep.listen().expect("crear canal");
        // Un segundo servidor con el mismo nombre debe fallar (instancia única).
        assert!(ep.listen().is_err(), "dos supervisores no deben compartir canal");
        let t = std::thread::spawn(move || {
            use interprocess::local_socket::traits::Listener as _;
            let conn = listener.accept().unwrap();
            let mut r = BufReader::new(conn);
            let hello: Hello = read_message(&mut r).unwrap();
            write_message(
                r.get_mut(),
                &HelloReply::Welcome(Welcome {
                    protocol: hello.protocol,
                    server_version: "t".into(),
                    supervisor_pid: 1,
                }),
            )
            .unwrap();
            let req: Request = read_message(&mut r).unwrap();
            write_message(
                r.get_mut(),
                &Response {
                    seq: req.seq,
                    result: ResponseResult::Ok(ResponseBody::Ok),
                },
            )
            .unwrap();
        });
        let mut c = Client::connect_with_token(&ep, "tok", "test").unwrap();
        assert_eq!(c.call(RequestBody::Ping).unwrap(), ResponseBody::Ok);
        t.join().unwrap();
    }
}
