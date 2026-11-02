//! Reconocimiento de la salida del servidor para avanzar estados.
//!
//! Patrones probados contra el formato de log de Vanilla/Paper/Fabric/NeoForge (Log4j):
//! `[12:00:00] [Server thread/INFO]: Done (5.123s)! For help, type "help"`
//! NeoForge/Forge usan `[12:00:00] [Server thread/INFO] [minecraft/DedicatedServer]: Done (...)`.
//! El detector es tolerante a prefijos pero exige el texto de Mojang.
//!
//! Bedrock Dedicated Server usa otro formato, siempre con la marca de tiempo y el nivel al
//! principio: `[2024-02-23 14:18:27:338 INFO] Server started.`. Se exige la línea completa.

use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogSignal {
    /// Primera línea reconocible de arranque del servidor.
    Loading,
    /// El servidor terminó de cargar y acepta conexiones.
    Ready,
    /// El servidor informa que el puerto está ocupado.
    PortInUse,
    /// El servidor se detiene por EULA no aceptada.
    EulaNotAccepted,
    /// Comienza el apagado ("Stopping server").
    StoppingServer,
}

/// Prefijo de cada línea de Bedrock Dedicated Server: `[AAAA-MM-DD hh:mm:ss:mmm NIVEL] `.
const BDS_PREFIX: &str = r"^\[\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}(?::\d+)? (?:INFO|WARN|ERROR)\] ";

fn bds_re(rest: &str) -> Regex {
    Regex::new(&format!("{BDS_PREFIX}{rest}")).unwrap()
}

fn bds_ready() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| bds_re(r"Server started\.$"))
}

fn bds_loading() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| bds_re(r"(?:Starting Server|Version: [0-9.]+)$"))
}

fn bds_port() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| bds_re(r"Port \[\d+\] may be in use by another process"))
}

fn bds_stopping() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| bds_re(r"(?:Server stop requested\.|Stopping server\.\.\.)$"))
}

fn bds_player() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Los gamertags pueden llevar espacios, pero no comas; el xuid son solo dígitos.
    RE.get_or_init(|| bds_re(r"Player (connected|disconnected): ([^,\]\[]{1,32}), xuid: \d*$"))
}

fn done_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"\]: Done \(\d+(?:[.,]\d+)?s\)! For help, type "help""#).unwrap())
}

/// Entrada o salida de un jugador según el registro del servidor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerEvent {
    Joined(String),
    Left(String),
}

fn player_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // El nombre va justo después de "]: ". Un mensaje de chat empieza por "<Nombre>", un /say por
    // "[Server]" y un /me por "*", así que ningún jugador puede simular la entrada de otro.
    RE.get_or_init(|| Regex::new(r"\]: ([A-Za-z0-9_]{1,16}) (joined|left) the game$").unwrap())
}

/// Bedrock: gamertag y XUID de «Player connected: Nombre, xuid: 123». Solo la entrada lleva un
/// XUID que valga la pena recordar (para dar permisos sin pedirlo a mano).
pub fn bedrock_xuid(line: &str) -> Option<(String, String)> {
    let c = bds_player().captures(line.trim_end())?;
    if &c[1] != "connected" {
        return None;
    }
    let name = c.get(2)?.as_str().trim().to_string();
    let xuid = line.trim_end().rsplit("xuid: ").next()?.trim().to_string();
    if name.is_empty() || xuid.is_empty() || xuid.len() > 20 || !xuid.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some((name, xuid))
}

/// Detecta "Nombre joined the game" / "Nombre left the game" (Vanilla, Paper, Fabric, NeoForge).
pub fn player_event(line: &str) -> Option<PlayerEvent> {
    if let Some(c) = bds_player().captures(line.trim_end()) {
        let name = c.get(2)?.as_str().trim().to_string();
        if name.is_empty() {
            return None;
        }
        return Some(if &c[1] == "connected" {
            PlayerEvent::Joined(name)
        } else {
            PlayerEvent::Left(name)
        });
    }
    let c = player_re().captures(line.trim_end())?;
    let name = c.get(1)?.as_str().to_string();
    Some(if &c[2] == "joined" {
        PlayerEvent::Joined(name)
    } else {
        PlayerEvent::Left(name)
    })
}

/// Analiza una línea de consola. Devuelve la señal si la línea es significativa.
pub fn classify_line(line: &str) -> Option<LogSignal> {
    if done_re().is_match(line) {
        return Some(LogSignal::Ready);
    }
    let t = line.trim_end();
    if bds_ready().is_match(t) {
        return Some(LogSignal::Ready);
    }
    if bds_port().is_match(t) {
        return Some(LogSignal::PortInUse);
    }
    if bds_stopping().is_match(t) {
        return Some(LogSignal::StoppingServer);
    }
    if bds_loading().is_match(t) {
        return Some(LogSignal::Loading);
    }
    if line.contains("**** FAILED TO BIND TO PORT!")
        || line.contains("Perhaps a server is already running on that port?")
    {
        return Some(LogSignal::PortInUse);
    }
    if line.contains("You need to agree to the EULA in order to run the server") {
        return Some(LogSignal::EulaNotAccepted);
    }
    if line.contains("]: Stopping server") {
        return Some(LogSignal::StoppingServer);
    }
    if line.contains("]: Starting minecraft server version")
        || line.contains("]: Loading properties")
        || line.contains("]: Preparing level")
        || line.contains("ModLauncher running")
        || line.contains("Loading Minecraft")
    // Fabric
    {
        return Some(LogSignal::Loading);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_done() {
        let l = r#"[18:04:11] [Server thread/INFO]: Done (3.812s)! For help, type "help""#;
        assert_eq!(classify_line(l), Some(LogSignal::Ready));
    }

    #[test]
    fn neoforge_done() {
        let l = r#"[18:04:11] [Server thread/INFO] [minecraft/DedicatedServer]: Done (21.004s)! For help, type "help""#;
        assert_eq!(classify_line(l), Some(LogSignal::Ready));
    }

    #[test]
    fn chat_cannot_fake_ready() {
        // Un jugador que escribe el texto en el chat no debe poner el servidor en línea:
        // el chat aparece como "<Nombre> Done (1s)! ..." y no tiene "]: Done (".
        let l = r#"[18:05:00] [Server thread/INFO]: <Troll> Done (1s)! For help, type "help""#;
        assert_eq!(classify_line(l), None);
    }

    #[test]
    fn port_and_eula() {
        assert_eq!(
            classify_line("[x] [Server thread/WARN]: **** FAILED TO BIND TO PORT!"),
            Some(LogSignal::PortInUse)
        );
        assert_eq!(
            classify_line("[x] [main/INFO]: You need to agree to the EULA in order to run the server. Go to eula.txt for more info."),
            Some(LogSignal::EulaNotAccepted)
        );
    }

    #[test]
    fn players() {
        assert_eq!(
            player_event("[18:00:01] [Server thread/INFO]: Steve joined the game"),
            Some(PlayerEvent::Joined("Steve".into()))
        );
        assert_eq!(
            player_event("[18:00:01] [Server thread/INFO] [minecraft/MinecraftServer]: Alex_99 left the game"),
            Some(PlayerEvent::Left("Alex_99".into()))
        );
        // Intentos de falsificar desde el chat, /say o /me
        assert_eq!(
            player_event("[x] [Server thread/INFO]: <Troll> Steve joined the game"),
            None
        );
        assert_eq!(
            player_event("[x] [Server thread/INFO]: [Server] Steve joined the game"),
            None
        );
        assert_eq!(
            player_event("[x] [Server thread/INFO]: * Troll Steve joined the game"),
            None
        );
        assert_eq!(
            player_event("[x] [Server thread/INFO]: Steve joined the game, really"),
            None
        );
    }

    #[test]
    fn loading() {
        assert_eq!(
            classify_line("[x] [Server thread/INFO]: Starting minecraft server version 1.21.1"),
            Some(LogSignal::Loading)
        );
    }

    #[test]
    fn bedrock_lines() {
        assert_eq!(
            classify_line("[2024-02-23 14:18:27:338 INFO] Starting Server"),
            Some(LogSignal::Loading)
        );
        assert_eq!(
            classify_line("[2024-02-23 14:18:27:338 INFO] Version: 1.20.62.02"),
            Some(LogSignal::Loading)
        );
        assert_eq!(
            classify_line("[2024-02-23 14:18:30:001 INFO] Server started.\r"),
            Some(LogSignal::Ready)
        );
        assert_eq!(
            classify_line("[2024-02-23 14:18:28:142 ERROR] Port [19132] may be in use by another process. Free up port and re-run program"),
            Some(LogSignal::PortInUse)
        );
        assert_eq!(
            player_event("[2025-03-01 08:49:08:469 INFO] Player connected: Steve Craft, xuid: 2345678909876543"),
            Some(PlayerEvent::Joined("Steve Craft".into()))
        );
        assert_eq!(
            player_event("[2025-03-01 08:59:08:469 INFO] Player disconnected: Steve Craft, xuid: 2345678909876543"),
            Some(PlayerEvent::Left("Steve Craft".into()))
        );
    }

    #[test]
    fn bedrock_xuid_from_join() {
        assert_eq!(
            bedrock_xuid("[2025-03-01 08:49:08:469 INFO] Player connected: Steve Craft, xuid: 2345678909876543"),
            Some(("Steve Craft".into(), "2345678909876543".into()))
        );
        assert_eq!(
            bedrock_xuid("[2025-03-01 08:49:08:469 INFO] Player disconnected: Steve, xuid: 1"),
            None
        );
        // Sin XUID (servidor sin conexión a Xbox Live): no hay nada que recordar.
        assert_eq!(
            bedrock_xuid("[2025-03-01 08:49:08:469 INFO] Player connected: Steve, xuid: "),
            None
        );
        assert_eq!(
            bedrock_xuid("[x] [Server thread/INFO]: <Troll> Player connected: Steve, xuid: 1"),
            None
        );
    }

    #[test]
    fn bedrock_cannot_be_faked() {
        // Sin la marca de tiempo al principio no cuenta (p. ej. texto dentro de otra línea).
        assert_eq!(
            classify_line("<Troll> [2024-02-23 14:18:30:001 INFO] Server started."),
            None
        );
        assert_eq!(
            classify_line("[2024-02-23 14:18:30:001 INFO] Server started. de verdad"),
            None
        );
        assert_eq!(
            player_event("[x] [Server thread/INFO]: <Troll> Player connected: Steve, xuid: 1"),
            None
        );
        assert_eq!(
            player_event("[2025-03-01 08:49:08:469 INFO] Player connected: A, xuid: 1, xuid: 2"),
            None
        );
    }
}
