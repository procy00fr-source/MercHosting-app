//! Canal entre PCs con MercHosting para que un compañero administre servidores de un amigo.
//!
//! - Conexión con [iroh](https://iroh.computer): QUIC cifrado de punta a punta (TLS 1.3) y
//!   autenticado por la llave de cada PC. Intenta conexión directa (atravesando el router) y, si
//!   no puede, usa los relés públicos gratuitos de n0, que solo reenvían bytes cifrados.
//! - Es un canal propio de MercHosting: nunca usa el túnel de Minecraft ni Playit.
//! - Aquí solo hay transporte, códigos y mensajes. Qué se permite lo decide el anfitrión (el
//!   supervisor) en cada petición.

pub mod code;
pub mod guest;
pub mod wire;

use iroh::endpoint::{presets, Connection};
use iroh::{Endpoint, EndpointAddr, PublicKey, RelayMode, SecretKey};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use wire::{WireRequest, WireResponse};

/// Protocolo de la aplicación dentro de iroh (cambia si cambia el formato de los mensajes).
pub const ALPN: &[u8] = b"merchosting/companeros/1";

#[derive(Debug, thiserror::Error)]
pub enum RemoteError {
    #[error("no se pudo preparar el canal de compañeros: {0}")]
    Bind(String),
    #[error("no se pudo conectar con la PC de tu amigo: {0}")]
    Connect(String),
    #[error("se cortó la comunicación: {0}")]
    Io(String),
    #[error("la PC de tu amigo no respondió a tiempo")]
    Timeout,
    #[error("código no válido: {0}")]
    Code(String),
}

/// Cómo sale a la red.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Normal: relés y búsqueda de direcciones públicos de n0.
    Internet,
    /// Solo direcciones locales, sin relés ni búsquedas (pruebas automáticas).
    Local,
}

/// Pistas para encontrar a la otra PC (además de buscarla por su llave).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hints {
    pub addrs: Vec<SocketAddr>,
    pub relay: Option<String>,
}

/// Responde a lo que pide un compañero (`llave del compañero`, petición).
pub type Handler = Arc<dyn Fn([u8; 32], WireRequest) -> WireResponse + Send + Sync>;
/// ¿Se aceptan conexiones de compañeros ahora?
pub type Gate = Arc<dyn Fn() -> bool + Send + Sync>;

/// Llave nueva para esta PC (se guarda en la carpeta de datos de MercHosting).
pub fn new_device_key() -> [u8; 32] {
    SecretKey::generate().to_bytes()
}

/// Llave pública que corresponde a una llave privada.
pub fn public_of(secret: &[u8; 32]) -> [u8; 32] {
    *SecretKey::from_bytes(secret).public().as_bytes()
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn parse_hex32(s: &str) -> Option<[u8; 32]> {
    let s = s.trim();
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

/// Huella corta y legible de una llave pública (para comprobarla de palabra).
pub fn fingerprint(public: &[u8; 32]) -> String {
    use sha2::{Digest, Sha256};
    let h = Sha256::digest(public);
    let s = code::base32(&h[..8]).to_uppercase();
    format!("{}-{}-{}", &s[0..4], &s[4..8], &s[8..12])
}

pub struct Node {
    rt: tokio::runtime::Runtime,
    ep: Endpoint,
    mode: Mode,
    /// Conexiones entrantes abiertas, por compañero.
    incoming: Arc<Mutex<HashMap<[u8; 32], Vec<Connection>>>>,
    /// Conexiones salientes reutilizables, por anfitrión.
    outgoing: Mutex<HashMap<[u8; 32], Connection>>,
}

impl Node {
    pub fn start(secret: [u8; 32], mode: Mode) -> Result<Node, RemoteError> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("merc-companeros")
            .enable_all()
            .build()
            .map_err(|e| RemoteError::Bind(e.to_string()))?;
        let ep = rt
            .block_on(async {
                let b = match mode {
                    Mode::Internet => Endpoint::builder(presets::N0),
                    Mode::Local => Endpoint::builder(presets::Minimal).relay_mode(RelayMode::Disabled),
                };
                b.secret_key(SecretKey::from_bytes(&secret))
                    .alpns(vec![ALPN.to_vec()])
                    .bind()
                    .await
            })
            .map_err(|e| RemoteError::Bind(e.to_string()))?;
        Ok(Node {
            rt,
            ep,
            mode,
            incoming: Arc::default(),
            outgoing: Mutex::default(),
        })
    }

    pub fn public(&self) -> [u8; 32] {
        *self.ep.id().as_bytes()
    }

    /// Direcciones para incluir en el código (solo en modo local; en Internet se buscan solas).
    pub fn hint_addrs(&self) -> Vec<SocketAddr> {
        if self.mode != Mode::Local {
            return vec![];
        }
        self.ep
            .bound_sockets()
            .into_iter()
            .filter(|a| a.is_ipv4())
            .map(|mut a| {
                if a.ip().is_unspecified() {
                    a.set_ip(std::net::Ipv4Addr::LOCALHOST.into());
                }
                a
            })
            .collect()
    }

    /// Relé de esta PC (si ya se conectó a uno), para incluirlo en los códigos.
    pub fn relay_url(&self) -> Option<String> {
        if self.mode != Mode::Internet {
            return None;
        }
        let ep = self.ep.clone();
        self.rt.block_on(async move {
            let _ = tokio::time::timeout(Duration::from_secs(5), ep.online()).await;
            ep.addr().relay_urls().next().map(|u| u.to_string())
        })
    }

    /// Compañeros conectados ahora mismo.
    pub fn connected(&self) -> Vec<[u8; 32]> {
        let mut m = self.incoming.lock().unwrap_or_else(|p| p.into_inner());
        m.retain(|_, v| {
            v.retain(|c| c.close_reason().is_none());
            !v.is_empty()
        });
        m.keys().copied().collect()
    }

    /// Corta al momento las conexiones de un compañero (al revocarlo).
    pub fn disconnect(&self, peer: &[u8; 32]) {
        if let Some(list) = self.incoming.lock().unwrap_or_else(|p| p.into_inner()).remove(peer) {
            for c in list {
                c.close(1u32.into(), b"acceso revocado");
            }
        }
    }

    /// Atiende a compañeros: cada petición va en su propio flujo y se responde con `handler`.
    pub fn serve(&self, gate: Gate, handler: Handler) {
        let ep = self.ep.clone();
        let incoming = self.incoming.clone();
        self.rt.spawn(async move {
            while let Some(inc) = ep.accept().await {
                let gate = gate.clone();
                let handler = handler.clone();
                let incoming = incoming.clone();
                tokio::spawn(async move {
                    if !gate() {
                        inc.refuse();
                        return;
                    }
                    let Ok(conn) = inc.await else {
                        return;
                    };
                    let peer = *conn.remote_id().as_bytes();
                    incoming
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .entry(peer)
                        .or_default()
                        .push(conn.clone());
                    while let Ok((mut send, mut recv)) = conn.accept_bi().await {
                        if !gate() {
                            conn.close(2u32.into(), b"compartir desactivado");
                            break;
                        }
                        let handler = handler.clone();
                        tokio::spawn(async move {
                            let req = match wire::read_frame::<WireRequest>(&mut recv).await {
                                Ok(r) => r,
                                Err(e) => WireRequest::Invalid(e.to_string()),
                            };
                            let resp = match req {
                                WireRequest::Invalid(e) => WireResponse::Denied(format!("Mensaje no válido: {e}")),
                                req => tokio::task::spawn_blocking(move || handler(peer, req))
                                    .await
                                    .unwrap_or_else(|_| WireResponse::Denied("Error interno del anfitrión".into())),
                            };
                            let _ = wire::write_frame(&mut send, &resp).await;
                            let _ = send.finish();
                            // Espera a que el compañero lea la respuesta antes de soltar el flujo.
                            let _ = tokio::time::timeout(Duration::from_secs(30), send.stopped()).await;
                        });
                    }
                });
            }
        });
    }

    /// Envía una petición a la PC de un amigo y espera la respuesta.
    pub fn call(
        &self,
        host: [u8; 32],
        hints: &Hints,
        req: &WireRequest,
        timeout: Duration,
    ) -> Result<WireResponse, RemoteError> {
        self.rt.block_on(async {
            match tokio::time::timeout(timeout, self.call_async(host, hints, req, false)).await {
                Ok(Err(_)) => tokio::time::timeout(timeout, self.call_async(host, hints, req, true))
                    .await
                    .map_err(|_| RemoteError::Timeout)?,
                Ok(r) => r,
                Err(_) => Err(RemoteError::Timeout),
            }
        })
    }

    async fn conn_to(&self, host: [u8; 32], hints: &Hints, fresh: bool) -> Result<Connection, RemoteError> {
        if !fresh {
            if let Some(c) = self.outgoing.lock().unwrap_or_else(|p| p.into_inner()).get(&host) {
                if c.close_reason().is_none() {
                    return Ok(c.clone());
                }
            }
        }
        let pk = PublicKey::from_bytes(&host).map_err(|e| RemoteError::Connect(e.to_string()))?;
        let mut addr = EndpointAddr::new(pk);
        for h in &hints.addrs {
            addr = addr.with_ip_addr(*h);
        }
        if let Some(r) = hints.relay.as_deref().and_then(|r| r.parse::<iroh::RelayUrl>().ok()) {
            addr = addr.with_relay_url(r);
        }
        let c = self
            .ep
            .connect(addr, ALPN)
            .await
            .map_err(|e| RemoteError::Connect(e.to_string()))?;
        self.outgoing
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(host, c.clone());
        Ok(c)
    }

    async fn call_async(
        &self,
        host: [u8; 32],
        hints: &Hints,
        req: &WireRequest,
        fresh: bool,
    ) -> Result<WireResponse, RemoteError> {
        let conn = self.conn_to(host, hints, fresh).await?;
        let (mut send, mut recv) = conn.open_bi().await.map_err(|e| RemoteError::Io(e.to_string()))?;
        wire::write_frame(&mut send, req)
            .await
            .map_err(|e| RemoteError::Io(e.to_string()))?;
        send.finish().map_err(|e| RemoteError::Io(e.to_string()))?;
        wire::read_frame::<WireResponse>(&mut recv)
            .await
            .map_err(|e| RemoteError::Io(e.to_string()))
    }

    /// Olvida la conexión saliente a un anfitrión.
    pub fn drop_host(&self, host: &[u8; 32]) {
        if let Some(c) = self.outgoing.lock().unwrap_or_else(|p| p.into_inner()).remove(host) {
            c.close(0u32.into(), b"");
        }
    }

    pub fn shutdown(self) {
        let ep = self.ep.clone();
        self.rt.block_on(async move { ep.close().await });
        self.rt.shutdown_timeout(Duration::from_secs(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use merc_ipc::remote::RemoteServer;

    #[test]
    fn two_pcs_talk_over_an_encrypted_channel() {
        let host = Node::start(new_device_key(), Mode::Local).unwrap();
        let guest_key = new_device_key();
        let guest_pub = public_of(&guest_key);
        let guest = Node::start(guest_key, Mode::Local).unwrap();
        let enabled = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let e2 = enabled.clone();
        host.serve(
            Arc::new(move || e2.load(std::sync::atomic::Ordering::SeqCst)),
            Arc::new(move |peer, req| {
                assert_eq!(peer, guest_pub, "el anfitrión ve la llave real del compañero");
                match req {
                    WireRequest::Servers => WireResponse::Servers {
                        host_name: "PC de Amir".into(),
                        servers: vec![RemoteServer {
                            id: merc_core::InstanceId::new(),
                            name: "Survival".into(),
                            perms: vec![],
                        }],
                        host_admin: false,
                    },
                    _ => WireResponse::Denied("no".into()),
                }
            }),
        );
        let hints = Hints {
            addrs: host.hint_addrs(),
            relay: host.relay_url(),
        };
        assert!(!hints.addrs.is_empty() && hints.relay.is_none());
        let r = guest
            .call(host.public(), &hints, &WireRequest::Servers, Duration::from_secs(20))
            .unwrap();
        assert!(matches!(r, WireResponse::Servers { ref host_name, .. } if host_name == "PC de Amir"));
        // Segunda petición por la misma conexión.
        let r = guest
            .call(
                host.public(),
                &hints,
                &WireRequest::Pair {
                    secret: "x".into(),
                    name: "y".into(),
                },
                Duration::from_secs(20),
            )
            .unwrap();
        assert!(matches!(r, WireResponse::Denied(_)));
        assert_eq!(host.connected(), vec![guest_pub]);
        // Al revocar se corta la conexión; al desactivar ya no se atiende.
        host.disconnect(&guest_pub);
        enabled.store(false, std::sync::atomic::Ordering::SeqCst);
        assert!(guest
            .call(host.public(), &hints, &WireRequest::Servers, Duration::from_secs(5))
            .is_err());
        guest.shutdown();
        host.shutdown();
    }

    #[test]
    fn hex_and_fingerprint() {
        let k = public_of(&new_device_key());
        assert_eq!(parse_hex32(&hex(&k)), Some(k));
        assert_eq!(parse_hex32("zz"), None);
        let f = fingerprint(&k);
        assert_eq!(f.len(), 14);
        assert_eq!(f, fingerprint(&k));
    }
}
