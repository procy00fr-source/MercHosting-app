//! Reconocimiento de la salida del servidor para avanzar estados.
//!
//! Patrones probados contra el formato de log de Vanilla/Paper/Fabric/NeoForge (Log4j):
//! `[12:00:00] [Server thread/INFO]: Done (5.123s)! For help, type "help"`
//! NeoForge/Forge usan `[12:00:00] [Server thread/INFO] [minecraft/DedicatedServer]: Done (...)`.
//! El detector es tolerante a prefijos pero exige el texto de Mojang.

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

/// Detecta "Nombre joined the game" / "Nombre left the game" (Vanilla, Paper, Fabric, NeoForge).
pub fn player_event(line: &str) -> Option<PlayerEvent> {
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
}
