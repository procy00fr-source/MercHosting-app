//! Prueba real de una dirección de Minecraft Bedrock: el «Unconnected Ping» de RakNet que hace
//! el juego para mostrar un servidor en su lista. Sirve tanto para un servidor Bedrock de verdad
//! como para Geyser (que habla Bedrock delante de un servidor Java).

use crate::slp::{split_address, PingResult};
use crate::NetError;
use std::net::{ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

/// Puerto por defecto de Bedrock (UDP).
pub const DEFAULT_PORT: u16 = 19132;

/// «Magic» fijo de los paquetes sin conexión de RakNet.
const MAGIC: [u8; 16] = [
    0x00, 0xff, 0xff, 0x00, 0xfe, 0xfe, 0xfe, 0xfe, 0xfd, 0xfd, 0xfd, 0xfd, 0x12, 0x34, 0x56, 0x78,
];
const PING: u8 = 0x01;
const PONG: u8 = 0x1c;

/// Paquete de ping con la marca de tiempo `time` y un GUID de cliente fijo.
pub fn ping_packet(time: u64) -> Vec<u8> {
    let mut p = Vec::with_capacity(33);
    p.push(PING);
    p.extend_from_slice(&time.to_be_bytes());
    p.extend_from_slice(&MAGIC);
    p.extend_from_slice(&0x4d65_7263_4265_6472u64.to_be_bytes()); // "MercBedr"
    p
}

/// Datos que anuncia el servidor (`MCPE;motd;protocolo;versión;jugadores;máximo;…`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pong {
    pub time: u64,
    pub motd: String,
    pub version: String,
    pub players_online: u32,
    pub players_max: u32,
}

/// Lee la respuesta. `None` si no es un pong de RakNet válido.
pub fn parse_pong(b: &[u8]) -> Option<Pong> {
    if b.len() < 35 || b[0] != PONG || b[17..33] != MAGIC {
        return None;
    }
    let time = u64::from_be_bytes(b[1..9].try_into().ok()?);
    let len = u16::from_be_bytes([b[33], b[34]]) as usize;
    let text = std::str::from_utf8(b.get(35..35 + len)?).ok()?;
    let f: Vec<&str> = text.split(';').collect();
    if f.len() < 6 || !(f[0] == "MCPE" || f[0] == "MCEE") {
        return None;
    }
    let clean = |s: &str| -> String {
        let mut out = String::new();
        let mut skip = false;
        for c in s.chars() {
            if skip {
                skip = false;
            } else if c == '§' {
                skip = true;
            } else {
                out.push(c);
            }
        }
        out.trim().chars().take(200).collect()
    };
    Some(Pong {
        time,
        motd: clean(f[1]),
        version: format!("Bedrock {}", f[3].chars().take(40).collect::<String>()),
        players_online: f[4].trim().parse().unwrap_or(0),
        players_max: f[5].trim().parse().unwrap_or(0),
    })
}

/// Hace el ping a `host:port` por UDP. Reintenta un par de veces (UDP puede perder paquetes).
pub fn ping(host: &str, port: u16, timeout: Duration) -> Result<PingResult, NetError> {
    let addr = (host, port)
        .to_socket_addrs()
        .map_err(|e| NetError::Network(format!("no se encontró «{host}»: {e}")))?
        .next()
        .ok_or_else(|| NetError::Network(format!("no se encontró «{host}»")))?;
    let bind = if addr.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" };
    let sock = UdpSocket::bind(bind).map_err(|e| NetError::Network(e.to_string()))?;
    let tries = 3u32;
    let per_try = timeout / tries;
    let _ = sock.set_read_timeout(Some(per_try.max(Duration::from_millis(200))));
    let mut buf = [0u8; 2048];
    for n in 0..tries {
        let token = 0x4d48_0000u64 + n as u64;
        let started = Instant::now();
        sock.send_to(&ping_packet(token), addr)
            .map_err(|e| NetError::Network(format!("no se pudo enviar a {host}:{port}: {e}")))?;
        loop {
            match sock.recv_from(&mut buf) {
                Ok((len, from)) if from == addr => {
                    if let Some(p) = parse_pong(&buf[..len]) {
                        if p.time == token {
                            return Ok(PingResult {
                                tested: format!("{host}:{port}"),
                                latency_ms: started.elapsed().as_millis().min(u32::MAX as u128) as u32,
                                version: p.version,
                                players_online: p.players_online,
                                players_max: p.players_max,
                                motd: p.motd,
                            });
                        }
                    }
                }
                Ok(_) => continue,
                Err(_) => break,
            }
        }
    }
    Err(NetError::Network(format!(
        "{host}:{port} no respondió como un servidor Bedrock (UDP)"
    )))
}

/// Prueba una dirección como la escribiría un jugador de Bedrock (sin puerto = 19132).
pub fn ping_address(addr: &str, timeout: Duration) -> Result<PingResult, NetError> {
    let (host, port) = split_address(addr)?;
    ping(&host, port.unwrap_or(DEFAULT_PORT), timeout)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn pong(time: u64, text: &str) -> Vec<u8> {
        let mut p = vec![PONG];
        p.extend_from_slice(&time.to_be_bytes());
        p.extend_from_slice(&7u64.to_be_bytes());
        p.extend_from_slice(&MAGIC);
        p.extend_from_slice(&(text.len() as u16).to_be_bytes());
        p.extend_from_slice(text.as_bytes());
        p
    }

    /// Servidor Bedrock mínimo que contesta el ping.
    pub fn fake_server(text: &'static str) -> u16 {
        let s = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = s.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let mut buf = [0u8; 2048];
            while let Ok((n, from)) = s.recv_from(&mut buf) {
                if n >= 33 && buf[0] == PING {
                    let t = u64::from_be_bytes(buf[1..9].try_into().unwrap());
                    let _ = s.send_to(&pong(t, text), from);
                }
            }
        });
        port
    }

    #[test]
    fn packet_shape() {
        let p = ping_packet(5);
        assert_eq!(p.len(), 33);
        assert_eq!(p[0], 0x01);
        assert_eq!(&p[9..25], &MAGIC);
    }

    #[test]
    fn parses_pong() {
        let p = parse_pong(&pong(
            9,
            "MCPE;§aMi server;712;1.21.40;3;10;123;Mundo;Survival;1;19132;19133;",
        ))
        .unwrap();
        assert_eq!(p.time, 9);
        assert_eq!(p.motd, "Mi server");
        assert_eq!(p.version, "Bedrock 1.21.40");
        assert_eq!((p.players_online, p.players_max), (3, 10));
        assert!(parse_pong(&pong(9, "nada")).is_none());
        assert!(parse_pong(&[0x1c, 1, 2]).is_none());
        let mut bad = pong(9, "MCPE;a;1;1;1;1");
        bad[18] = 0;
        assert!(parse_pong(&bad).is_none());
    }

    #[test]
    fn real_ping_over_udp() {
        let port = fake_server("MCPE;Geyser;712;1.21.40;0;20;1;Geyser;Survival;1;19132;19133;");
        let r = ping_address(&format!("127.0.0.1:{port}"), Duration::from_secs(3)).unwrap();
        assert_eq!(r.version, "Bedrock 1.21.40");
        assert_eq!(r.players_max, 20);
        assert_eq!(r.tested, format!("127.0.0.1:{port}"));
    }

    #[test]
    fn silent_port_fails() {
        // Un puerto UDP sin nadie escuchando: no hay respuesta.
        let s = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = s.local_addr().unwrap().port();
        drop(s);
        assert!(ping("127.0.0.1", port, Duration::from_millis(900)).is_err());
    }
}
