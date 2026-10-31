//! Conectividad de MercHosting.
//!
//! - [`playit`]: agente oficial de Playit.gg (descarga verificada, vinculación oficial, estado).
//! - [`slp`]: prueba real de que una dirección llega a un servidor de Minecraft.

pub mod playit;
pub mod slp;

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum NetError {
    #[error("{0}")]
    Invalid(String),
    #[error("sin conexión: {0}")]
    Network(String),
    #[error("{0}")]
    Remote(String),
    #[error("agente de Playit: {0}")]
    Agent(String),
}
