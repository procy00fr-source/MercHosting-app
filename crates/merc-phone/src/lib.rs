//! MercHosting dentro del celular (Android): servidores de Minecraft **Java** que corren en el
//! propio teléfono. El servidor oficial de Bedrock no existe para Android; para jugadores de
//! Bedrock se crea un Paper con Geyser y Floodgate.
//!
//! - [`assets`]: JRE para Android (fijado con huella) y el código fijado del agente de Playit.
//! - [`ram`]: cuánta memoria se le da al servidor según el celular.
//! - [`warnings`]: avisos de riesgo que hay que aceptar.
//! - [`content`]: mods y plugins de Modrinth, y Geyser/Floodgate para Bedrock.
//! - [`jli`]: lanzador de la JVM en el mismo proceso (dlopen + JLI_Launch).
//! - [`local`]: los servidores del celular (crear, encender, consola, copias).
//! - [`playit`]: canal local con el agente de Playit y vinculación con la cuenta.
//! - [`api`]: las peticiones de la interfaz en un solo punto (la app solo las pasa).

pub mod api;
pub mod assets;
pub mod content;
pub mod jli;
pub mod local;
pub mod playit;
pub mod ram;
pub mod warnings;

/// Pruebas que hacen `fork` y pruebas que toman el candado del PID se turnan: el hijo de un
/// `fork` hereda los archivos abiertos (también el candado de otra prueba) y lo mantiene tomado.
#[cfg(test)]
pub(crate) static FORK_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());
