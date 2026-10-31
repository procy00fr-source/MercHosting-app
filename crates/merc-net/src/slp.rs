//! Prueba real de conexión: «Server List Ping» de Minecraft Java (lo mismo que hace el juego al
//! mostrar un servidor en la lista). Si responde, se sabe que la dirección llega al servidor;
//! la latencia es la que se mide de verdad desde esta PC (ida y vuelta del paquete de ping).

use crate::NetError;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

const MAX_PACKET: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PingResult {
    /// Host y puerto que se probaron de verdad (tras resolver el registro SRV si lo hay).
    pub tested: String,
    /// Ida y vuelta del paquete de ping (ms).
    pub latency_ms: u32,
    pub version: String,
    pub players_online: u32,
    pub players_max: u32,
    pub motd: String,
}

fn write_varint(out: &mut Vec<u8>, mut v: i32) {
    loop {
        let mut b = (v & 0x7f) as u8;
        v = ((v as u32) >> 7) as i32;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            break;
        }
    }
}

fn read_varint(r: &mut impl Read) -> Result<i32, NetError> {
    let mut value = 0i32;
    for i in 0..5 {
        let mut b = [0u8; 1];
        r.read_exact(&mut b)
            .map_err(|e| NetError::Network(format!("conexión cortada: {e}")))?;
        value |= ((b[0] & 0x7f) as i32) << (7 * i);
        if b[0] & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(NetError::Remote("respuesta que no es de Minecraft".into()))
}

fn packet(id: i32, body: &[u8]) -> Vec<u8> {
    let mut inner = vec![];
    write_varint(&mut inner, id);
    inner.extend_from_slice(body);
    let mut out = vec![];
    write_varint(&mut out, inner.len() as i32);
    out.extend(inner);
    out
}

fn read_packet(r: &mut impl Read) -> Result<(i32, Vec<u8>), NetError> {
    let len = read_varint(r)?;
    if len <= 0 || len as usize > MAX_PACKET {
        return Err(NetError::Remote("respuesta que no es de Minecraft".into()));
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf)
        .map_err(|e| NetError::Network(format!("conexión cortada: {e}")))?;
    let mut cur = std::io::Cursor::new(buf);
    let id = read_varint(&mut cur)?;
    let pos = cur.position() as usize;
    Ok((id, cur.into_inner()[pos..].to_vec()))
}

/// Texto plano de un `description` (cadena o componente con `text` y `extra`), sin códigos §.
fn motd_text(v: &serde_json::Value) -> String {
    fn walk(v: &serde_json::Value, out: &mut String) {
        match v {
            serde_json::Value::String(s) => out.push_str(s),
            serde_json::Value::Object(o) => {
                if let Some(t) = o.get("text") {
                    walk(t, out);
                }
                if let Some(serde_json::Value::Array(a)) = o.get("extra") {
                    for e in a {
                        walk(e, out);
                    }
                }
            }
            serde_json::Value::Array(a) => a.iter().for_each(|e| walk(e, out)),
            _ => {}
        }
    }
    let mut raw = String::new();
    walk(v, &mut raw);
    let mut out = String::new();
    let mut skip = false;
    for c in raw.chars() {
        if skip {
            skip = false;
        } else if c == '§' {
            skip = true;
        } else {
            out.push(c);
        }
    }
    out.trim().chars().take(200).collect()
}

/// Hace el ping a `host:port`.
pub fn ping(host: &str, port: u16, timeout: Duration) -> Result<PingResult, NetError> {
    ping_as(host, port, host, timeout)
}

/// Como [`ping`], pero anunciando `handshake_host` en el saludo, como hace el juego: tras
/// resolver el registro SRV se conecta al destino, pero anuncia el nombre que escribió el
/// jugador (Playit lo usa para saber a qué túnel va la conexión).
pub fn ping_as(host: &str, port: u16, handshake_host: &str, timeout: Duration) -> Result<PingResult, NetError> {
    let addr = (host, port)
        .to_socket_addrs()
        .map_err(|e| NetError::Network(format!("no se encontró «{host}»: {e}")))?
        .next()
        .ok_or_else(|| NetError::Network(format!("no se encontró «{host}»")))?;
    let mut s = TcpStream::connect_timeout(&addr, timeout)
        .map_err(|e| NetError::Network(format!("no se pudo conectar con {host}:{port}: {e}")))?;
    let _ = s.set_read_timeout(Some(timeout));
    let _ = s.set_write_timeout(Some(timeout));
    let mut hs = vec![];
    write_varint(&mut hs, -1); // versión de protocolo: «la que sea»
    write_varint(&mut hs, handshake_host.len() as i32);
    hs.extend_from_slice(handshake_host.as_bytes());
    hs.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut hs, 1); // siguiente estado: status
    let mut out = packet(0, &hs);
    out.extend(packet(0, &[]));
    s.write_all(&out).map_err(|e| NetError::Network(e.to_string()))?;
    let (id, body) = read_packet(&mut s)?;
    if id != 0 {
        return Err(NetError::Remote("respuesta que no es de Minecraft".into()));
    }
    let mut cur = std::io::Cursor::new(body);
    let len = read_varint(&mut cur)?;
    let pos = cur.position() as usize;
    let body = cur.into_inner();
    let json_bytes = body
        .get(pos..pos + len.max(0) as usize)
        .ok_or_else(|| NetError::Remote("respuesta incompleta".into()))?;
    let v: serde_json::Value =
        serde_json::from_slice(json_bytes).map_err(|_| NetError::Remote("respuesta que no es de Minecraft".into()))?;
    let started = Instant::now();
    let token: i64 = 0x4d65_7263_486f_7374; // "MercHost"
    s.write_all(&packet(1, &token.to_be_bytes()))
        .map_err(|e| NetError::Network(e.to_string()))?;
    let (pid, pong) = read_packet(&mut s)?;
    let latency = started.elapsed();
    if pid != 1 || pong.len() != 8 || i64::from_be_bytes(pong[..8].try_into().unwrap_or_default()) != token {
        return Err(NetError::Remote("el servidor no devolvió el ping".into()));
    }
    Ok(PingResult {
        tested: format!("{host}:{port}"),
        latency_ms: latency.as_millis().min(u32::MAX as u128) as u32,
        version: v
            .pointer("/version/name")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .chars()
            .take(80)
            .collect(),
        players_online: v.pointer("/players/online").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
        players_max: v.pointer("/players/max").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
        motd: v.get("description").map(motd_text).unwrap_or_default(),
    })
}

/// Separa `host:puerto`. Sin puerto, devuelve `None` (habrá que mirar el registro SRV).
pub fn split_address(addr: &str) -> Result<(String, Option<u16>), NetError> {
    let a = addr.trim();
    let ok_host = |h: &str| {
        !h.is_empty() && h.len() <= 253 && h.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    };
    if let Some((h, p)) = a.rsplit_once(':') {
        let port: u16 = p
            .parse()
            .map_err(|_| NetError::Invalid(format!("puerto no válido en «{a}»")))?;
        if !ok_host(h) || port == 0 {
            return Err(NetError::Invalid(format!("dirección no válida: «{a}»")));
        }
        return Ok((h.to_string(), Some(port)));
    }
    if !ok_host(a) {
        return Err(NetError::Invalid(format!("dirección no válida: «{a}»")));
    }
    Ok((a.to_string(), None))
}

/// Lee la salida de `nslookup -type=SRV`: `(destino, puerto)`.
pub fn parse_nslookup_srv(out: &str) -> Option<(String, u16)> {
    let mut port = None;
    let mut target = None;
    for line in out.lines() {
        let l = line.trim();
        if let Some((k, v)) = l.split_once('=') {
            let k = k.trim().to_lowercase();
            let v = v.trim();
            if k == "port" {
                port = v.parse().ok();
            } else if k == "svr hostname" {
                target = Some(v.trim_end_matches('.').to_string());
            }
        }
        // Formato de Linux: «… service = 0 5 25565 destino.»
        if let Some((_, rest)) = l.split_once("service = ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() == 4 {
                port = parts[2].parse().ok();
                target = Some(parts[3].trim_end_matches('.').to_string());
            }
        }
        if let (Some(p), Some(t)) = (port, target.clone()) {
            return Some((t, p));
        }
    }
    None
}

/// Registro SRV `_minecraft._tcp.<host>` con la herramienta del sistema (sin consola visible).
pub fn lookup_srv(host: &str) -> Option<(String, u16)> {
    let mut cmd = std::process::Command::new("nslookup");
    cmd.arg("-type=SRV").arg(format!("_minecraft._tcp.{host}"));
    cmd.stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().ok()?;
    parse_nslookup_srv(&String::from_utf8_lossy(&out.stdout))
}

/// Prueba una dirección como la escribiría un jugador (resuelve el SRV si no lleva puerto).
pub fn ping_address(addr: &str, timeout: Duration) -> Result<PingResult, NetError> {
    let (host, port) = split_address(addr)?;
    let (h, p) = match port {
        Some(p) => (host.clone(), p),
        None => lookup_srv(&host).unwrap_or((host.clone(), 25565)),
    };
    ping_as(&h, p, &host, timeout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// Servidor mínimo que responde como Minecraft.
    pub fn fake_server(json: &'static str) -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let _ = read_packet(&mut s).unwrap(); // handshake
            let _ = read_packet(&mut s).unwrap(); // status request
            let mut body = vec![];
            write_varint(&mut body, json.len() as i32);
            body.extend_from_slice(json.as_bytes());
            s.write_all(&packet(0, &body)).unwrap();
            let (id, payload) = read_packet(&mut s).unwrap();
            s.write_all(&packet(id, &payload)).unwrap();
        });
        port
    }

    #[test]
    fn pings_a_minecraft_server() {
        let port = fake_server(
            r#"{"version":{"name":"Paper 1.21.1","protocol":767},"players":{"max":20,"online":3},"description":{"text":"§aHola ","extra":[{"text":"amigos"}]}}"#,
        );
        let r = ping_address(&format!("127.0.0.1:{port}"), Duration::from_secs(5)).unwrap();
        assert_eq!(r.version, "Paper 1.21.1");
        assert_eq!((r.players_online, r.players_max), (3, 20));
        assert_eq!(r.motd, "Hola amigos");
        assert_eq!(r.tested, format!("127.0.0.1:{port}"));
    }

    #[test]
    fn not_minecraft_or_closed() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let _ = s.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n");
        });
        assert!(ping("127.0.0.1", port, Duration::from_secs(3)).is_err());
        let closed = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        assert!(ping("127.0.0.1", closed, Duration::from_secs(3)).is_err());
    }

    #[test]
    fn addresses_and_srv() {
        assert_eq!(split_address("a.joinmc.link").unwrap(), ("a.joinmc.link".into(), None));
        assert_eq!(
            split_address("147.185.221.1:41234").unwrap(),
            ("147.185.221.1".into(), Some(41234))
        );
        for bad in ["", "a b", "x:0", "x:99999", "x;rm"] {
            assert!(split_address(bad).is_err(), "{bad}");
        }
        let windows = "Servidor:  router\nAddress:  192.168.1.1\n\n_minecraft._tcp.a.joinmc.link\tSRV service location:\n          priority       = 0\n          weight         = 5\n          port           = 41234\n          svr hostname   = a.gl.at.ply.gg\n";
        assert_eq!(parse_nslookup_srv(windows), Some(("a.gl.at.ply.gg".into(), 41234)));
        let linux = "_minecraft._tcp.a.joinmc.link\tservice = 0 5 41234 a.gl.at.ply.gg.\n";
        assert_eq!(parse_nslookup_srv(linux), Some(("a.gl.at.ply.gg".into(), 41234)));
        assert_eq!(parse_nslookup_srv("** server can't find"), None);
    }
}
