//! Error que recibe la interfaz (común al escritorio y al celular).

use serde::Serialize;

/// Error que recibe la interfaz: `code` coincide con `ErrorCode` del protocolo, o `unavailable`
/// si no se pudo hablar con el supervisor.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct UiError {
    pub code: String,
    pub message: String,
}

impl UiError {
    pub(crate) fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl From<merc_ipc::ApiError> for UiError {
    fn from(e: merc_ipc::ApiError) -> Self {
        let code = serde_json::to_value(e.code)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string));
        UiError::new(code.as_deref().unwrap_or("internal"), e.message)
    }
}
