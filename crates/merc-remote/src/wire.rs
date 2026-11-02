//! Mensajes entre PCs: longitud (u32, big-endian) + JSON.

use merc_core::InstanceId;
use merc_ipc::remote::{RemoteItem, RemoteServer};
use merc_ipc::{RequestBody, ResponseResult};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Tope de un mensaje (los textos y listas del servidor caben de sobra).
pub const MAX_FRAME: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WireRequest {
    /// Usar un código de invitación (`secret` en hex) y presentarse con un nombre.
    Pair { secret: String, name: String },
    /// Qué servidores y permisos tengo en esta PC.
    Servers,
    /// Una petición del protocolo normal; el anfitrión decide si está permitida.
    Call { request: Box<RequestBody> },
    /// Preparar algo para llevárselo: el anfitrión lo deja listo y responde `Prepared`.
    Prepare { item: RemoteItem },
    /// Un trozo de lo preparado (desde `offset`).
    Chunk { token: String, offset: u64 },
    /// Un trozo de un archivo que se sube. El primero va sin `upload_id`; el último lleva
    /// `sha256` y entonces el anfitrión lo comprueba y lo coloca en el servidor.
    Upload {
        upload_id: Option<String>,
        id: InstanceId,
        dest_dir: String,
        name: String,
        size: u64,
        offset: u64,
        /// Bytes en base64.
        data: String,
        sha256: Option<String>,
    },
    /// (Solo interno) el mensaje recibido no se pudo leer.
    #[serde(skip)]
    Invalid(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum WireResponse {
    Servers {
        host_name: String,
        servers: Vec<RemoteServer>,
        /// Esta PC tiene control total del anfitrión (servidor casero). Las versiones
        /// anteriores no lo mandan: se entiende `false`.
        #[serde(default)]
        host_admin: bool,
    },
    Result(Box<ResponseResult>),
    Prepared {
        token: String,
        name: String,
        size: u64,
        sha256: String,
    },
    Chunk {
        /// Bytes en base64.
        data: String,
    },
    UploadProgress {
        upload_id: String,
        received: u64,
    },
    /// Rechazado (sin permiso, código usado o caducado…), con el motivo para la persona.
    Denied(String),
}

pub async fn write_frame<W: AsyncWrite + Unpin, T: Serialize>(w: &mut W, msg: &T) -> std::io::Result<()> {
    let body = serde_json::to_vec(msg).map_err(std::io::Error::other)?;
    if body.len() > MAX_FRAME {
        return Err(std::io::Error::other("mensaje demasiado grande"));
    }
    w.write_all(&(body.len() as u32).to_be_bytes()).await?;
    w.write_all(&body).await?;
    w.flush().await
}

pub async fn read_frame<T: DeserializeOwned>(r: &mut (impl AsyncRead + Unpin)) -> std::io::Result<T> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len).await?;
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err(std::io::Error::other("mensaje demasiado grande"));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).await?;
    serde_json::from_slice(&buf).map_err(std::io::Error::other)
}
