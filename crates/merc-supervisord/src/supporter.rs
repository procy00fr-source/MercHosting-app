//! Supporter Edition (US$8, pago único, solo cosmético).
//!
//! La compra se hace en la página oficial de Gumroad: MercHosting no toca tarjetas ni datos de
//! pago. Gumroad envía al comprador una clave de licencia; aquí se comprueba con la API pública
//! de licencias de Gumroad (`POST /v2/licenses/verify`, sin clave de API) y se guarda en esta PC.
//! Un reembolso, contracargo o disputa perdida desactiva los extras en la siguiente comprobación.
//! Sin red se conserva el último estado conocido: nunca se quita nada por estar desconectado.

use super::{api, Daemon};
use merc_ipc::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// ID del producto en Gumroad (Productos › el producto › Contenido › Clave de licencia). No es
/// secreto. Vacío mientras no exista el producto: la app muestra «Próximamente».
pub const GUMROAD_PRODUCT_ID: &str = "rDOphkWlktS3vhO61DHacw==";
/// Página oficial de compra en Gumroad. Vacía mientras no exista el producto.
pub const CHECKOUT_URL: &str = "";
pub const PRICE_USD: u32 = 8;
const VERIFY_URL: &str = "https://api.gumroad.com/v2/licenses/verify";
/// Cada cuánto conviene volver a comprobar la clave.
const STALE_MS: i64 = 7 * 24 * 3600 * 1000;

static LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Saved {
    license_key: String,
    active: bool,
    email_hint: Option<String>,
    purchased: Option<String>,
    checked_at_unix_ms: i64,
    revoked_reason: Option<String>,
    last_error: Option<String>,
}

/// Lo que se concluye de una respuesta de Gumroad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Verdict {
    Valid {
        email_hint: Option<String>,
        purchased: Option<String>,
    },
    /// La compra existe pero ya no da derecho (reembolso, contracargo…).
    Revoked(String),
    /// Gumroad dice que la clave no existe o está desactivada.
    Invalid(String),
}

fn mask_email(e: &str) -> Option<String> {
    let (user, domain) = e.split_once('@')?;
    let first: String = user.chars().take(1).collect();
    Some(format!("{first}•••@{domain}"))
}

pub(crate) fn key_ok(k: &str) -> bool {
    (8..=64).contains(&k.len()) && k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn key_hint(k: &str) -> String {
    let tail: String = k.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    format!("…{tail}")
}

/// Interpreta la respuesta de `/v2/licenses/verify`.
pub(crate) fn evaluate(status: u16, v: &serde_json::Value, product_id: &str) -> Verdict {
    let msg = v["message"].as_str().unwrap_or("").to_string();
    if !v["success"].as_bool().unwrap_or(false) || !(200..300).contains(&status) {
        return Verdict::Invalid(if msg.is_empty() {
            "Gumroad no reconoce esta clave para MercHosting Supporter.".into()
        } else {
            format!("Gumroad dice: {msg}")
        });
    }
    let p = &v["purchase"];
    if let Some(pid) = p["product_id"].as_str() {
        if !product_id.is_empty() && pid != product_id {
            return Verdict::Invalid("Esa clave es de otro producto de Gumroad, no de MercHosting Supporter.".into());
        }
    }
    let flag = |k: &str| p[k].as_bool().unwrap_or(false);
    if flag("refunded") {
        return Verdict::Revoked("La compra se reembolsó.".into());
    }
    if flag("chargebacked") {
        return Verdict::Revoked("La compra tuvo un contracargo.".into());
    }
    if flag("disputed") && !flag("dispute_won") {
        return Verdict::Revoked("La compra está en disputa con el banco.".into());
    }
    Verdict::Valid {
        email_hint: p["email"].as_str().and_then(mask_email),
        purchased: p["sale_timestamp"].as_str().map(|s| s.chars().take(10).collect()),
    }
}

impl Daemon {
    fn supporter_file(&self) -> PathBuf {
        self.data_dir.join("supporter.json")
    }

    fn product_id() -> String {
        if cfg!(debug_assertions) {
            if let Ok(v) = std::env::var("MERC_GUMROAD_PRODUCT") {
                return v;
            }
        }
        GUMROAD_PRODUCT_ID.to_string()
    }

    fn verify_url() -> String {
        if cfg!(debug_assertions) {
            if let Ok(v) = std::env::var("MERC_GUMROAD_API") {
                if merc_install::http::url_allowed(&v) {
                    return v;
                }
            }
        }
        VERIFY_URL.to_string()
    }

    fn checkout_url() -> Option<String> {
        let v = if cfg!(debug_assertions) {
            std::env::var("MERC_SUPPORTER_CHECKOUT").unwrap_or_else(|_| CHECKOUT_URL.to_string())
        } else {
            CHECKOUT_URL.to_string()
        };
        v.starts_with("https://").then_some(v)
    }

    fn load_supporter(&self) -> Option<Saved> {
        serde_json::from_str(&std::fs::read_to_string(self.supporter_file()).ok()?).ok()
    }

    fn save_supporter(&self, s: &Saved) -> Result<(), ApiError> {
        let f = self.supporter_file();
        let tmp = f.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(s).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        std::fs::write(&tmp, text).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        std::fs::rename(&tmp, &f).map_err(|e| api(ErrorCode::Internal, e.to_string()))
    }

    fn supporter_info(&self) -> SupporterInfo {
        let product = Self::product_id();
        let saved = self.load_supporter();
        let now = super::playit::now_ms();
        SupporterInfo {
            available: !product.is_empty(),
            checkout_url: Self::checkout_url(),
            price_usd: PRICE_USD,
            active: saved.as_ref().is_some_and(|s| s.active),
            email_hint: saved.as_ref().and_then(|s| s.email_hint.clone()),
            key_hint: saved.as_ref().map(|s| key_hint(&s.license_key)),
            purchased: saved.as_ref().and_then(|s| s.purchased.clone()),
            checked_at_unix_ms: saved.as_ref().map(|s| s.checked_at_unix_ms),
            stale: saved
                .as_ref()
                .is_some_and(|s| s.active && now - s.checked_at_unix_ms > STALE_MS),
            revoked_reason: saved.as_ref().and_then(|s| s.revoked_reason.clone()),
            last_error: saved.as_ref().and_then(|s| s.last_error.clone()),
        }
    }

    fn verify(&self, key: &str, first_time: bool) -> Result<Verdict, String> {
        let product = Self::product_id();
        let (status, v) = self
            .http
            .post_form(
                &Self::verify_url(),
                &[
                    ("product_id", product.as_str()),
                    ("license_key", key),
                    // Solo la activación cuenta como un uso nuevo; las revisiones no.
                    ("increment_uses_count", if first_time { "true" } else { "false" }),
                ],
            )
            .map_err(|e| e.to_string())?;
        if status >= 500 || status == 429 {
            return Err(format!("Gumroad no respondió bien (HTTP {status}). Prueba más tarde."));
        }
        Ok(evaluate(status, &v, &product))
    }

    pub(crate) fn handle_supporter(
        self: &Arc<Self>,
        client: &str,
        body: RequestBody,
    ) -> Result<ResponseBody, ApiError> {
        let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
        match body {
            RequestBody::Supporter => {}
            RequestBody::ActivateSupporter { license_key } => {
                if Self::product_id().is_empty() {
                    return Err(api(
                        ErrorCode::InvalidState,
                        "Supporter Edition todavía no está a la venta.",
                    ));
                }
                let key = license_key.trim().to_string();
                if !key_ok(&key) {
                    return Err(api(
                        ErrorCode::BadRequest,
                        "Esa no parece una clave de licencia. Cópiala tal cual del correo de Gumroad.",
                    ));
                }
                match self.verify(&key, true) {
                    Err(e) => return Err(api(ErrorCode::Network, format!("No se pudo comprobar la clave: {e}"))),
                    Ok(Verdict::Invalid(why)) | Ok(Verdict::Revoked(why)) => {
                        return Err(api(ErrorCode::BadRequest, why))
                    }
                    Ok(Verdict::Valid { email_hint, purchased }) => {
                        self.save_supporter(&Saved {
                            license_key: key.clone(),
                            active: true,
                            email_hint,
                            purchased,
                            checked_at_unix_ms: super::playit::now_ms(),
                            revoked_reason: None,
                            last_error: None,
                        })?;
                        self.audit(client, None, "supporter.activate", &format!("clave {}", key_hint(&key)));
                    }
                }
            }
            RequestBody::RefreshSupporter => {
                let Some(mut s) = self.load_supporter() else {
                    return Err(api(ErrorCode::NotFound, "No hay ninguna clave guardada en esta PC."));
                };
                match self.verify(&s.license_key, false) {
                    // Sin red o Gumroad caído: se conserva el estado y se anota el problema.
                    Err(e) => s.last_error = Some(e),
                    Ok(Verdict::Valid { email_hint, purchased }) => {
                        if !s.active {
                            self.audit(
                                client,
                                None,
                                "supporter.restore",
                                &format!("clave {}", key_hint(&s.license_key)),
                            );
                        }
                        s.active = true;
                        s.email_hint = email_hint.or(s.email_hint);
                        s.purchased = purchased.or(s.purchased);
                        s.revoked_reason = None;
                        s.last_error = None;
                        s.checked_at_unix_ms = super::playit::now_ms();
                    }
                    Ok(Verdict::Revoked(why)) | Ok(Verdict::Invalid(why)) => {
                        if s.active {
                            self.audit(client, None, "supporter.revoked", &why);
                        }
                        s.active = false;
                        s.revoked_reason = Some(why);
                        s.last_error = None;
                        s.checked_at_unix_ms = super::playit::now_ms();
                    }
                }
                self.save_supporter(&s)?;
            }
            RequestBody::ForgetSupporter => {
                if let Some(s) = self.load_supporter() {
                    let _ = std::fs::remove_file(self.supporter_file());
                    self.audit(
                        client,
                        None,
                        "supporter.forget",
                        &format!("clave {}", key_hint(&s.license_key)),
                    );
                }
            }
            _ => return Err(api(ErrorCode::BadRequest, "Petición no válida")),
        }
        Ok(ResponseBody::Supporter(self.supporter_info()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ok(p: serde_json::Value) -> serde_json::Value {
        json!({"success": true, "uses": 1, "purchase": p})
    }

    #[test]
    fn valid_purchase_hides_most_of_the_email() {
        let v = ok(
            json!({"product_id": "P1", "email": "amir@gmail.com", "sale_timestamp": "2026-10-30T20:00:00Z", "refunded": false, "chargebacked": false}),
        );
        assert_eq!(
            evaluate(200, &v, "P1"),
            Verdict::Valid {
                email_hint: Some("a•••@gmail.com".into()),
                purchased: Some("2026-10-30".into())
            }
        );
    }

    #[test]
    fn refunds_chargebacks_and_lost_disputes_revoke() {
        assert!(matches!(
            evaluate(200, &ok(json!({"refunded": true})), "P1"),
            Verdict::Revoked(_)
        ));
        assert!(matches!(
            evaluate(200, &ok(json!({"chargebacked": true})), "P1"),
            Verdict::Revoked(_)
        ));
        assert!(matches!(
            evaluate(200, &ok(json!({"disputed": true})), "P1"),
            Verdict::Revoked(_)
        ));
        assert!(matches!(
            evaluate(200, &ok(json!({"disputed": true, "dispute_won": true})), "P1"),
            Verdict::Valid { .. }
        ));
    }

    #[test]
    fn unknown_disabled_or_foreign_keys_are_invalid() {
        let v = json!({"success": false, "message": "That license does not exist for the provided product."});
        assert_eq!(
            evaluate(404, &v, "P1"),
            Verdict::Invalid("Gumroad dice: That license does not exist for the provided product.".into())
        );
        assert!(matches!(
            evaluate(200, &ok(json!({"product_id": "OTRO"})), "P1"),
            Verdict::Invalid(_)
        ));
    }

    #[test]
    fn key_format() {
        assert!(key_ok("ABCD1234-EFGH5678-IJKL9012-MNOP3456"));
        assert!(!key_ok("corta"));
        assert!(!key_ok("con espacios y cosas raras!!"));
        assert_eq!(key_hint("ABCD1234-MNOP3456"), "…3456");
    }
}
