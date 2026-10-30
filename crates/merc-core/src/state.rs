//! Estados reales del ciclo de vida de un servidor.
//!
//! Regla principal: ONLINE solo se alcanza con una señal de readiness verificable
//! (ver [`crate::readiness`]). Un proceso vivo no basta.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RuntimeState {
    /// No hay proceso controlado por MercHosting.
    Stopped,
    /// Comprobando Java, EULA, puertos, archivos y bloqueos antes de lanzar.
    Validating,
    /// El proceso Java fue creado; todavía no hay salida reconocible.
    Starting,
    /// El servidor está cargando (se reconoció salida de arranque).
    Loading,
    /// El servidor confirmó que acepta conexiones ("Done (...)! For help").
    Online,
    /// Se envió `stop` y se espera a que guarde y termine.
    Stopping,
    /// El proceso terminó sin que se lo pidiéramos.
    Crashed,
    /// La validación o el lanzamiento fallaron; no hay proceso.
    Error,
    /// Reconciliando tras reiniciar el supervisor.
    Recovering,
    /// El proceso existe pero no se puede confirmar su estado (p. ej. readiness no detectada a tiempo).
    Unknown,
}

impl RuntimeState {
    /// Hay (o puede haber) un proceso vivo asociado.
    pub fn has_process(self) -> bool {
        matches!(
            self,
            Self::Starting | Self::Loading | Self::Online | Self::Stopping | Self::Unknown | Self::Recovering
        )
    }

    /// Se puede solicitar un arranque desde este estado.
    pub fn can_start(self) -> bool {
        matches!(self, Self::Stopped | Self::Crashed | Self::Error)
    }

    /// Transiciones permitidas. Cualquier otra es un error de programación o un evento fuera de orden.
    #[allow(clippy::match_like_matches_macro)] // la tabla explícita se lee mejor
    pub fn can_transition_to(self, to: RuntimeState) -> bool {
        use RuntimeState::*;
        if self == to {
            return false;
        }
        match (self, to) {
            (Stopped | Crashed | Error, Validating) => true,
            (Validating, Starting | Error | Stopped) => true,
            (Starting, Loading | Online | Stopping | Crashed | Unknown) => true,
            (Loading, Online | Stopping | Crashed | Unknown) => true,
            (Online, Stopping | Crashed) => true,
            (Unknown, Online | Loading | Stopping | Crashed) => true,
            (Stopping, Stopped | Crashed | Unknown) => true,
            (Recovering, Online | Unknown | Stopped | Crashed) => true,
            (Stopped | Unknown, Recovering) => true,
            _ => false,
        }
    }

    /// Texto breve para la interfaz en español.
    pub fn label_es(self) -> &'static str {
        match self {
            Self::Stopped => "Detenido",
            Self::Validating => "Validando",
            Self::Starting => "Iniciando",
            Self::Loading => "Cargando",
            Self::Online => "En línea",
            Self::Stopping => "Deteniendo",
            Self::Crashed => "Se cerró inesperadamente",
            Self::Error => "Error",
            Self::Recovering => "Recuperando",
            Self::Unknown => "Estado no confirmado",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeState::*;

    #[test]
    fn online_requires_process_path() {
        assert!(!Stopped.can_transition_to(Online));
        assert!(!Validating.can_transition_to(Online));
        assert!(Loading.can_transition_to(Online));
    }

    #[test]
    fn cannot_start_while_running() {
        for s in [Starting, Loading, Online, Stopping, Unknown, Recovering, Validating] {
            assert!(!s.can_start(), "{s:?} no debería permitir arrancar");
        }
        assert!(Stopped.can_start());
        assert!(Crashed.can_start());
    }

    #[test]
    fn serializes_screaming() {
        assert_eq!(serde_json::to_string(&Online).unwrap(), "\"ONLINE\"");
    }
}
