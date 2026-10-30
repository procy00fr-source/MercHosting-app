//! Núcleo de dominio de MercHosting. Sin E/S de red; la E/S de disco se limita a
//! lectura para detección y a la verificación de rutas.

pub mod detect;
pub mod instance;
pub mod java;
pub mod launch;
pub mod paths;
pub mod process_flags;
pub mod properties;
pub mod readiness;
pub mod state;

pub use instance::{Instance, InstanceId};
pub use state::RuntimeState;
