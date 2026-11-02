//! MercHosting dentro del celular (Android): servidores de Minecraft **Java** que corren en el
//! propio teléfono. Bedrock no: Mojang no publica su servidor para Android.
//!
//! - [`assets`]: JRE para Android (fijado con huella) y el código fijado del agente de Playit.
//! - [`ram`]: cuánta memoria se le da al servidor según el celular.
//! - [`warnings`]: avisos de riesgo que hay que aceptar.
//! - [`jli`]: lanzador de la JVM en el mismo proceso (dlopen + JLI_Launch).
//! - [`local`]: los servidores del celular (crear, encender, consola, copias).
//! - [`playit`]: canal local con el agente de Playit y vinculación con la cuenta.
//! - [`api`]: las peticiones de la interfaz en un solo punto (la app solo las pasa).

pub mod api;
pub mod assets;
pub mod jli;
pub mod local;
pub mod playit;
pub mod ram;
pub mod warnings;
