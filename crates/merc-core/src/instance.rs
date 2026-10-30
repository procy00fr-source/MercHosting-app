//! Contrato de una instancia registrada en MercHosting.

use crate::detect::ServerKind;
use crate::launch::LaunchProfile;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Identificador interno inmutable. El nombre visible puede cambiar; el id no.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct InstanceId(pub Uuid);

impl InstanceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for InstanceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for InstanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instance {
    pub id: InstanceId,
    pub name: String,
    /// Carpeta del servidor. Para instancias importadas es la carpeta original del usuario,
    /// que MercHosting no mueve ni reorganiza.
    pub root: PathBuf,
    pub kind: ServerKind,
    pub mc_version: Option<String>,
    pub loader_version: Option<String>,
    pub launch: LaunchProfile,
    /// `true` si se registró una carpeta existente (importación sin copiar).
    pub imported: bool,
    pub created_at_unix: i64,
}

/// Valida un nombre visible de instancia.
pub fn validate_instance_name(name: &str) -> Result<String, &'static str> {
    let n = name.trim();
    if n.is_empty() {
        return Err("El nombre no puede estar vacío");
    }
    if n.chars().count() > 64 {
        return Err("El nombre puede tener como máximo 64 caracteres");
    }
    if n.chars().any(|c| c.is_control()) {
        return Err("El nombre contiene caracteres no válidos");
    }
    Ok(n.to_string())
}
