//! Códigos de invitación: `MH1-` + base32 de (llave pública del anfitrión, secreto de un solo
//! uso y, solo en pruebas locales, direcciones). El secreto no se guarda en claro en el
//! anfitrión: solo su SHA-256.

use super::RemoteError;
use std::net::{IpAddr, SocketAddr};

const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
pub const PREFIX: &str = "MH1-";

pub fn base32(data: &[u8]) -> String {
    let mut out = String::new();
    let mut buf: u32 = 0;
    let mut bits = 0;
    for &b in data {
        buf = (buf << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buf >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

pub fn unbase32(s: &str) -> Option<Vec<u8>> {
    let mut out = vec![];
    let mut buf: u32 = 0;
    let mut bits = 0;
    for c in s.chars() {
        let v = ALPHABET.iter().position(|a| *a as char == c.to_ascii_lowercase())? as u32;
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invite {
    pub host: [u8; 32],
    pub secret: [u8; 16],
    pub addrs: Vec<SocketAddr>,
    /// Relé de la PC anfitriona (así no hace falta buscarla primero).
    pub relay: Option<String>,
}

impl Invite {
    pub fn encode(&self) -> String {
        let mut b = Vec::with_capacity(64);
        b.extend_from_slice(&self.host);
        b.extend_from_slice(&self.secret);
        for a in &self.addrs {
            match a.ip() {
                IpAddr::V4(ip) => {
                    b.push(4);
                    b.extend_from_slice(&ip.octets());
                }
                IpAddr::V6(ip) => {
                    b.push(6);
                    b.extend_from_slice(&ip.octets());
                }
            }
            b.extend_from_slice(&a.port().to_be_bytes());
        }
        if let Some(r) = self.relay.as_ref().filter(|r| r.len() < 200) {
            b.push(b'r');
            b.push(r.len() as u8);
            b.extend_from_slice(r.as_bytes());
        }
        format!("{PREFIX}{}", base32(&b).to_uppercase())
    }

    /// Acepta el código con espacios, guiones extra o en minúsculas (copiado de un chat).
    pub fn decode(text: &str) -> Result<Invite, RemoteError> {
        let t: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let body = t
            .strip_prefix(PREFIX)
            .or_else(|| t.strip_prefix(&PREFIX.to_lowercase()))
            .ok_or_else(|| RemoteError::Code("debe empezar por MH1-".into()))?;
        let body: String = body.chars().filter(|c| *c != '-').collect();
        let b = unbase32(&body).ok_or_else(|| RemoteError::Code("tiene letras que no corresponden".into()))?;
        if b.len() < 48 {
            return Err(RemoteError::Code("está incompleto".into()));
        }
        let mut host = [0u8; 32];
        host.copy_from_slice(&b[..32]);
        let mut secret = [0u8; 16];
        secret.copy_from_slice(&b[32..48]);
        let mut addrs = vec![];
        let mut relay = None;
        let mut rest = &b[48..];
        while !rest.is_empty() {
            if rest[0] == b'r' {
                let len = *rest.get(1).ok_or_else(bad)? as usize;
                let url = rest.get(2..2 + len).ok_or_else(bad)?;
                let url = std::str::from_utf8(url).map_err(|_| bad())?;
                if !url.starts_with("https://") {
                    return Err(bad());
                }
                relay = Some(url.to_string());
                rest = &rest[2 + len..];
                continue;
            }
            let (ip, used): (IpAddr, usize) = match rest[0] {
                4 if rest.len() >= 7 => (
                    <[u8; 4]>::try_from(&rest[1..5])
                        .ok()
                        .map(IpAddr::from)
                        .ok_or_else(bad)?,
                    5,
                ),
                6 if rest.len() >= 19 => (
                    <[u8; 16]>::try_from(&rest[1..17])
                        .ok()
                        .map(IpAddr::from)
                        .ok_or_else(bad)?,
                    17,
                ),
                _ => return Err(bad()),
            };
            let port = u16::from_be_bytes([rest[used], rest[used + 1]]);
            addrs.push(SocketAddr::new(ip, port));
            rest = &rest[used + 2..];
        }
        Ok(Invite {
            host,
            secret,
            addrs,
            relay,
        })
    }
}

fn bad() -> RemoteError {
    RemoteError::Code("está dañado".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_tolerant_parsing() {
        let i = Invite {
            host: [7; 32],
            secret: [9; 16],
            addrs: vec![],
            relay: None,
        };
        let c = i.encode();
        assert!(c.starts_with("MH1-"));
        assert_eq!(Invite::decode(&c).unwrap(), i);
        assert_eq!(Invite::decode(&format!("  {}\n", c.to_lowercase())).unwrap(), i);
        let with = Invite {
            addrs: vec!["127.0.0.1:4242".parse().unwrap(), "[::1]:9".parse().unwrap()],
            ..i.clone()
        };
        assert_eq!(Invite::decode(&with.encode()).unwrap(), with);
        let relay = Invite {
            relay: Some("https://use1-1.relay.n0.iroh-canary.iroh.link./".into()),
            ..i.clone()
        };
        assert_eq!(Invite::decode(&relay.encode()).unwrap(), relay);
        assert!(Invite::decode("hola").is_err());
        assert!(Invite::decode("MH1-ABC").is_err());
        assert!(Invite::decode(&format!("{}1", &c[..c.len() - 1])).is_err());
        assert_eq!(unbase32(&base32(b"MercHosting")).unwrap(), b"MercHosting");
    }
}
