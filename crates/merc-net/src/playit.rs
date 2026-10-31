//! Playit.gg mediante su agente oficial (`playitd`, licencia BSD-2-Clause,
//! <https://github.com/playit-cloud/playit-agent>).
//!
//! - **Agente**: se descarga la versión fijada de la publicación oficial de GitHub y se comprueba
//!   su SHA-256 antes de usarla. MercHosting lo ejecuta en segundo plano con su propio canal
//!   privado (`--socket-path`) y su propio archivo de secreto (`--secret-path`), sin tocar una
//!   instalación de Playit que ya tenga la persona.
//! - **Vinculación**: el flujo oficial de «claim» del propio agente (`playit-cli claim`): se
//!   genera un código, la persona abre `https://playit.gg/claim/<código>` con SU cuenta, lo
//!   aprueba, y el secreto que devuelve Playit se entrega al agente por su canal local
//!   (`set_secret`). El agente lo guarda; MercHosting no lo escribe en ningún otro sitio.
//! - **Estado**: se lee del agente por su canal local (`get_state`): fase, túneles con su
//!   dirección pública y su destino local, túneles pendientes y avisos.
//! - **Túneles**: la API pública documentada de Playit (docs.playit.gg) es de pago y no incluye
//!   crear túneles, así que MercHosting NO los crea por su cuenta: guía a la persona para crearlo
//!   en playit.gg y lo reconoce en cuanto el agente lo recibe.

use crate::NetError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

/// Versión fijada del agente oficial y su huella (calculada de la publicación oficial).
pub const AGENT_VERSION: &str = "1.2.0";
pub const AGENT_URL: &str =
    "https://github.com/playit-cloud/playit-agent/releases/download/v1.2.0/playitd-windows-x86_64.exe";
pub const AGENT_SHA256: &str = "ecd1c6c236b378c3f27ae2038e6eb69726f378235f50163b0ea29fd3fca83331";
pub const AGENT_SIZE: u64 = 5_251_584;
pub const API_BASE: &str = "https://api.playit.gg";
pub const CLAIM_BASE: &str = "https://playit.gg/claim/";
/// Versión del canal local del agente que entiende MercHosting.
pub const IPC_VERSION: u32 = 2;

/// Código de vinculación: 5 bytes aleatorios en hexadecimal (como `playit-cli claim generate`).
pub fn claim_code(random: [u8; 5]) -> String {
    random.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn claim_url(code: &str) -> Result<String, NetError> {
    if code.len() != 10 || !code.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(NetError::Invalid("código de vinculación no válido".into()));
    }
    Ok(format!("{CLAIM_BASE}{code}"))
}

/// Estado de la vinculación en playit.gg.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimState {
    /// Falta que la persona abra el enlace.
    WaitingForVisit,
    /// Abrió el enlace; falta que apruebe.
    WaitingForApproval,
    Approved,
    Rejected,
    Expired,
}

fn api_result(v: &Value) -> Result<Result<Value, Value>, NetError> {
    match v.get("status").and_then(Value::as_str) {
        Some("success") => Ok(Ok(v.get("data").cloned().unwrap_or(Value::Null))),
        Some("fail") => Ok(Err(v.get("data").cloned().unwrap_or(Value::Null))),
        Some("error") => Err(NetError::Remote(format!(
            "Playit respondió con un error: {}",
            v.get("data").map(|d| d.to_string()).unwrap_or_default()
        ))),
        _ => Err(NetError::Remote("respuesta inesperada de Playit".into())),
    }
}

/// Cliente del flujo oficial de vinculación (`/claim/setup` y `/claim/exchange`).
pub struct ClaimClient<'a> {
    http: &'a merc_install::Http,
    base: String,
}

impl<'a> ClaimClient<'a> {
    pub fn new(http: &'a merc_install::Http, base: impl Into<String>) -> Self {
        Self {
            http,
            base: base.into(),
        }
    }

    fn call(&self, path: &str, body: Value) -> Result<Result<Value, Value>, NetError> {
        let v = self
            .http
            .post_json(&format!("{}{path}", self.base), &body)
            .map_err(|e| NetError::Network(e.to_string()))?;
        api_result(&v)
    }

    /// Pregunta en qué punto está la aprobación del código.
    pub fn setup(&self, code: &str) -> Result<ClaimState, NetError> {
        claim_url(code)?;
        match self.call(
            "/claim/setup",
            json!({ "code": code, "agent_type": "assignable", "version": format!("MercHosting (playit {AGENT_VERSION})") }),
        )? {
            Ok(Value::String(s)) => Ok(match s.as_str() {
                "WaitingForUserVisit" => ClaimState::WaitingForVisit,
                "WaitingForUser" => ClaimState::WaitingForApproval,
                "UserAccepted" => ClaimState::Approved,
                "UserRejected" => ClaimState::Rejected,
                other => return Err(NetError::Remote(format!("estado de vinculación desconocido: {other}"))),
            }),
            Ok(other) => Err(NetError::Remote(format!("respuesta inesperada: {other}"))),
            Err(Value::String(s)) if s == "CodeExpired" => Ok(ClaimState::Expired),
            Err(f) => Err(NetError::Remote(format!("Playit rechazó el código: {f}"))),
        }
    }

    /// Cambia el código aprobado por el secreto del agente (solo cuando ya está aprobado).
    pub fn exchange(&self, code: &str) -> Result<Option<String>, NetError> {
        claim_url(code)?;
        match self.call("/claim/exchange", json!({ "code": code }))? {
            Ok(v) => {
                let key = v
                    .get("secret_key")
                    .and_then(Value::as_str)
                    .filter(|k| !k.trim().is_empty() && k.len() < 1024)
                    .ok_or_else(|| NetError::Remote("Playit no devolvió la clave del agente".into()))?;
                Ok(Some(key.trim().to_string()))
            }
            // Todavía no aprobado.
            Err(Value::String(s)) if s == "NotAccepted" || s == "NotSetup" => Ok(None),
            Err(f) => Err(NetError::Remote(format!("no se pudo terminar la vinculación: {f}"))),
        }
    }
}

// ---------- Canal local del agente ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Tunnel {
    /// Dirección pública que se da a los amigos (`nombre.joinmc.link` o `host:puerto`).
    pub display_address: String,
    /// Destino en esta PC (`127.0.0.1:25565`).
    pub destination: String,
    #[serde(default)]
    pub is_disabled: bool,
    #[serde(default)]
    pub disabled_reason: Option<String>,
}

impl Tunnel {
    /// Puerto local al que lleva el túnel.
    pub fn local_port(&self) -> Option<u16> {
        self.destination.rsplit_once(':')?.1.trim().parse().ok()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Notice {
    pub priority: String,
    pub message: String,
    pub resolve_link: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentPhase {
    /// Funciona, pero falta vincularlo a una cuenta.
    WaitingForSecret,
    InvalidSecret,
    /// La cuenta superó el límite de agentes de su plan.
    OverLimit,
    Starting,
    Running,
    Stopping,
    Error,
}

/// Lo que informa el agente (subconjunto de su `AgentLifecycle`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentStatus {
    pub phase: AgentPhase,
    pub message: Option<String>,
    pub version: Option<String>,
    /// `guest`, `email_not_verified` o `verified`.
    pub account_status: Option<String>,
    pub tunnels: Vec<Tunnel>,
    /// Túneles que Playit todavía está preparando (su mensaje).
    pub pending: Vec<String>,
    pub notices: Vec<Notice>,
}

/// Interpreta la respuesta `state` del agente (tolerante a campos nuevos).
pub fn parse_lifecycle(v: &Value) -> Result<AgentStatus, NetError> {
    let state = v
        .get("state")
        .and_then(Value::as_str)
        .ok_or_else(|| NetError::Remote("estado del agente ilegible".into()))?;
    let data = v.get("data").cloned().unwrap_or(Value::Null);
    let msg = |d: &Value| d.get("message").and_then(Value::as_str).map(str::to_string);
    let mut out = AgentStatus {
        phase: AgentPhase::Starting,
        message: None,
        version: None,
        account_status: None,
        tunnels: vec![],
        pending: vec![],
        notices: vec![],
    };
    match state {
        "waiting_for_secret" => out.phase = AgentPhase::WaitingForSecret,
        "has_invalid_secret" => {
            out.phase = AgentPhase::InvalidSecret;
            out.message = msg(&data);
        }
        "disabled_over_limit" => {
            out.phase = AgentPhase::OverLimit;
            out.message = msg(&data);
        }
        "starting" => out.phase = AgentPhase::Starting,
        "stopping" => out.phase = AgentPhase::Stopping,
        "error" => {
            out.phase = AgentPhase::Error;
            out.message = msg(&data);
        }
        "running" => {
            out.phase = AgentPhase::Running;
            out.version = data.get("version").and_then(Value::as_str).map(str::to_string);
            out.account_status = data.get("account_status").and_then(Value::as_str).map(str::to_string);
            out.tunnels = data
                .get("tunnels")
                .cloned()
                .and_then(|t| serde_json::from_value(t).ok())
                .unwrap_or_default();
            out.pending = data
                .get("pending_tunnels")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|p| p.get("status_msg").and_then(Value::as_str).map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            out.notices = data
                .get("notices")
                .cloned()
                .and_then(|t| serde_json::from_value(t).ok())
                .unwrap_or_default();
        }
        other => {
            out.phase = AgentPhase::Error;
            out.message = Some(format!("estado desconocido del agente: {other}"));
        }
    }
    Ok(out)
}

/// Conexión al canal local del agente: líneas JSON (`playit-ipc`, versión 2).
/// Lectura sobre un `Stream` compartido (`&Stream` implementa `Read` y `Write`).
struct Shared(std::sync::Arc<interprocess::local_socket::Stream>);

impl std::io::Read for Shared {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        (&*self.0).read(buf)
    }
}

pub struct AgentIpc {
    reader: BufReader<Shared>,
    writer: std::sync::Arc<interprocess::local_socket::Stream>,
    next_id: u64,
}

impl AgentIpc {
    /// `socket` es una ruta de archivo (Linux) o de tubería (`\\.\pipe\…`, Windows).
    pub fn connect(socket: &str, timeout: Duration) -> Result<Self, NetError> {
        use interprocess::local_socket::{prelude::*, GenericFilePath, Stream};
        let name = std::path::Path::new(socket)
            .to_fs_name::<GenericFilePath>()
            .map_err(|e| NetError::Agent(format!("canal no válido: {e}")))?;
        let stream = Stream::connect(name).map_err(|e| NetError::Agent(format!("el agente no responde: {e}")))?;
        let _ = stream.set_recv_timeout(Some(timeout));
        let stream = std::sync::Arc::new(stream);
        let mut c = Self {
            reader: BufReader::new(Shared(stream.clone())),
            writer: stream,
            next_id: 1,
        };
        let hello = c.read()?;
        if hello.get("message_kind").and_then(Value::as_str) != Some("hello") {
            return Err(NetError::Agent("el agente no saludó como se esperaba".into()));
        }
        let v = hello
            .pointer("/data/protocol/ipc_version")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if v != IPC_VERSION as u64 {
            return Err(NetError::Agent(format!(
                "versión del canal del agente no compatible ({v}, se esperaba {IPC_VERSION})"
            )));
        }
        Ok(c)
    }

    fn read(&mut self) -> Result<Value, NetError> {
        let mut line = String::new();
        let n = self
            .reader
            .read_line(&mut line)
            .map_err(|e| NetError::Agent(format!("sin respuesta del agente: {e}")))?;
        if n == 0 {
            return Err(NetError::Agent("el agente cerró la conexión".into()));
        }
        serde_json::from_str(line.trim()).map_err(|_| NetError::Agent("respuesta ilegible del agente".into()))
    }

    /// Envía una petición y devuelve `response` (`{type, data}`), saltando eventos.
    pub fn request(&mut self, request: Value) -> Result<Value, NetError> {
        let id = self.next_id;
        self.next_id += 1;
        let env = json!({ "ipc_version": IPC_VERSION, "request_id": id, "request": request });
        let mut text = env.to_string();
        text.push('\n');
        (&*self.writer)
            .write_all(text.as_bytes())
            .map_err(|e| NetError::Agent(e.to_string()))?;
        loop {
            let msg = self.read()?;
            if msg.get("message_kind").and_then(Value::as_str) != Some("response") {
                continue;
            }
            let data = msg.get("data").cloned().unwrap_or(Value::Null);
            if data.get("request_id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            let resp = data.get("response").cloned().unwrap_or(Value::Null);
            if resp.get("type").and_then(Value::as_str) == Some("error") {
                let m = resp
                    .pointer("/data/message")
                    .and_then(Value::as_str)
                    .unwrap_or("error del agente");
                return Err(NetError::Agent(m.to_string()));
            }
            return Ok(resp);
        }
    }

    pub fn state(&mut self) -> Result<AgentStatus, NetError> {
        let r = self.request(json!({ "type": "get_state" }))?;
        parse_lifecycle(r.get("data").unwrap_or(&Value::Null))
    }

    /// Entrega el secreto al agente (que lo guarda en su archivo).
    pub fn set_secret(&mut self, secret: &str) -> Result<(), NetError> {
        let r = self.request(json!({ "type": "set_secret", "secret": secret }))?;
        accepted(&r)
    }

    /// Olvida la vinculación (el agente vuelve a esperar un secreto).
    pub fn reset_secret(&mut self) -> Result<(), NetError> {
        let r = self.request(json!({ "type": "reset_secret" }))?;
        accepted(&r)
    }

    /// Enlace para entrar en la cuenta de playit.gg vinculada.
    pub fn login_url(&mut self) -> Result<String, NetError> {
        let r = self.request(json!({ "type": "get_account_login_url" }))?;
        r.pointer("/data/login_url")
            .and_then(Value::as_str)
            .filter(|u| u.starts_with("https://"))
            .map(str::to_string)
            .ok_or_else(|| NetError::Agent("el agente no dio un enlace de acceso".into()))
    }

    pub fn stop(&mut self) -> Result<(), NetError> {
        let r = self.request(json!({ "type": "stop" }))?;
        accepted(&r)
    }
}

fn accepted(r: &Value) -> Result<(), NetError> {
    if r.pointer("/data/accepted").and_then(Value::as_bool) == Some(true) {
        Ok(())
    } else {
        Err(NetError::Agent(
            r.pointer("/data/message")
                .and_then(Value::as_str)
                .unwrap_or("el agente no aceptó la orden")
                .to_string(),
        ))
    }
}

/// Cómo está el túnel para los servidores encendidos (por su puerto local).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelCheck {
    /// Algún servidor encendido tiene un túnel activo.
    Serving,
    /// Hay un problema (el agente no responde, falló o el túnel está desactivado).
    Problem(String),
    /// Ningún servidor encendido tiene túnel: no hay nada que vigilar.
    NotRelevant,
}

/// Comprueba el túnel con lo que informa el agente. `None` = el agente no responde.
pub fn tunnel_check(agent: Option<&AgentStatus>, online_ports: &[u16]) -> TunnelCheck {
    if online_ports.is_empty() {
        return TunnelCheck::NotRelevant;
    }
    let Some(a) = agent else {
        return TunnelCheck::Problem("El agente de Playit no responde.".into());
    };
    if a.phase != AgentPhase::Running {
        let why = match a.phase {
            AgentPhase::WaitingForSecret => "Playit dejó de estar vinculado a una cuenta.".to_string(),
            AgentPhase::InvalidSecret => "Playit rechazó la clave de esta PC.".to_string(),
            AgentPhase::OverLimit => "La cuenta de Playit superó el límite de agentes.".to_string(),
            AgentPhase::Starting => "El agente de Playit se está reiniciando.".to_string(),
            AgentPhase::Stopping => "El agente de Playit se está deteniendo.".to_string(),
            _ => "El agente de Playit informó un error.".to_string(),
        };
        return TunnelCheck::Problem(match &a.message {
            Some(m) if !m.trim().is_empty() => format!("{why} ({})", m.trim()),
            _ => why,
        });
    }
    let mine: Vec<&Tunnel> = a
        .tunnels
        .iter()
        .filter(|t| t.local_port().is_some_and(|p| online_ports.contains(&p)))
        .collect();
    if mine.is_empty() {
        return TunnelCheck::NotRelevant;
    }
    if mine.iter().any(|t| !t.is_disabled) {
        return TunnelCheck::Serving;
    }
    let t = mine[0];
    TunnelCheck::Problem(format!(
        "Playit desactivó el túnel {}{}.",
        t.display_address,
        t.disabled_reason
            .as_deref()
            .map(|r| format!(": {r}"))
            .unwrap_or_default()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_urls() {
        let c = claim_code([0xab, 0x01, 0xff, 0x10, 0x00]);
        assert_eq!(c, "ab01ff1000");
        assert_eq!(claim_url(&c).unwrap(), "https://playit.gg/claim/ab01ff1000");
        assert!(claim_url("../x").is_err());
        assert!(claim_url("ab01ff100").is_err());
    }

    #[test]
    fn lifecycle_parsing() {
        let running = json!({ "state": "running", "data": {
            "version": "1.2.0", "agent_id": "x", "account_status": "verified", "start_time": 1,
            "tunnels": [
                { "display_address": "dark-sea.joinmc.link", "destination": "127.0.0.1:25565", "is_disabled": false, "disabled_reason": null },
                { "display_address": "147.185.221.1:41234", "destination": "127.0.0.1:24454", "is_disabled": true, "disabled_reason": "over limit" }
            ],
            "pending_tunnels": [{ "id": "p", "status_msg": "Asignando puerto" }],
            "notices": [{ "priority": "High", "message": "Verifica tu correo", "resolve_link": "https://playit.gg/account" }],
            "login_link": null, "campo_nuevo": 1
        }});
        let s = parse_lifecycle(&running).unwrap();
        assert_eq!(s.phase, AgentPhase::Running);
        assert_eq!(s.tunnels[0].local_port(), Some(25565));
        assert!(s.tunnels[1].is_disabled);
        assert_eq!(s.pending, ["Asignando puerto"]);
        assert_eq!(s.notices[0].message, "Verifica tu correo");
        assert_eq!(s.account_status.as_deref(), Some("verified"));
        assert_eq!(
            parse_lifecycle(&json!({ "state": "waiting_for_secret" }))
                .unwrap()
                .phase,
            AgentPhase::WaitingForSecret
        );
        let bad = parse_lifecycle(&json!({ "state": "has_invalid_secret", "data": { "code": "invalid_secret", "message": "clave inválida", "retryable": false } })).unwrap();
        assert_eq!(
            (bad.phase, bad.message.as_deref()),
            (AgentPhase::InvalidSecret, Some("clave inválida"))
        );
    }

    #[test]
    fn api_results() {
        assert_eq!(
            api_result(&json!({"status":"success","data":"UserAccepted"})).unwrap(),
            Ok(json!("UserAccepted"))
        );
        assert_eq!(
            api_result(&json!({"status":"fail","data":"NotAccepted"})).unwrap(),
            Err(json!("NotAccepted"))
        );
        assert!(api_result(&json!({"status":"error","data":{}})).is_err());
    }

    #[test]
    fn tunnel_check_by_port() {
        let t = |port: u16, disabled: bool| Tunnel {
            display_address: format!("x{port}.joinmc.link"),
            destination: format!("127.0.0.1:{port}"),
            is_disabled: disabled,
            disabled_reason: disabled.then(|| "uso indebido".into()),
        };
        let st = |phase, tunnels| AgentStatus {
            phase,
            message: None,
            version: None,
            account_status: None,
            tunnels,
            pending: vec![],
            notices: vec![],
        };
        let ok = st(AgentPhase::Running, vec![t(25565, false)]);
        assert_eq!(tunnel_check(Some(&ok), &[]), TunnelCheck::NotRelevant);
        assert_eq!(tunnel_check(Some(&ok), &[25565]), TunnelCheck::Serving);
        assert_eq!(tunnel_check(Some(&ok), &[25566]), TunnelCheck::NotRelevant);
        assert!(matches!(tunnel_check(None, &[25565]), TunnelCheck::Problem(_)));
        let off = st(AgentPhase::Running, vec![t(25565, true)]);
        assert_eq!(
            tunnel_check(Some(&off), &[25565]),
            TunnelCheck::Problem("Playit desactivó el túnel x25565.joinmc.link: uso indebido.".into())
        );
        let unlinked = st(AgentPhase::WaitingForSecret, vec![]);
        assert!(matches!(tunnel_check(Some(&unlinked), &[25565]), TunnelCheck::Problem(m) if m.contains("vinculado")));
    }
}
