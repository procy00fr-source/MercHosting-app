//! Cliente HTTP mínimo: solo HTTPS (HTTP únicamente hacia 127.0.0.1 para las pruebas),
//! límites de tamaño, descarga a archivo temporal con hash verificado y cancelación.

use crate::{Checksum, InstallError};
use sha1::Digest as _;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Identifica a MercHosting ante los servicios oficiales (PaperMC lo exige).
pub const USER_AGENT: &str = concat!(
    "MercHosting/",
    env!("CARGO_PKG_VERSION"),
    " (app de escritorio libre GPL-3.0 para crear servidores de Minecraft)"
);

/// Tope para respuestas de catálogo (JSON/XML).
const MAX_TEXT_BYTES: u64 = 32 * 1024 * 1024;
/// Tope para un archivo descargado (un JAR de servidor ronda 50–80 MB).
pub const MAX_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;

pub struct Http {
    agent: ureq::Agent,
}

impl Default for Http {
    fn default() -> Self {
        Self::new()
    }
}

/// ¿Se permite pedir esta URL? HTTPS siempre; HTTP solo hacia el propio equipo (pruebas).
pub fn url_allowed(url: &str) -> bool {
    if let Some(rest) = url.strip_prefix("https://") {
        return !rest.is_empty() && !rest.starts_with('/');
    }
    if let Some(rest) = url.strip_prefix("http://") {
        let host = rest.split(['/', '?']).next().unwrap_or("");
        let host = host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host);
        return host == "127.0.0.1" || host == "localhost";
    }
    false
}

impl Http {
    pub fn new() -> Self {
        let tls = ureq::tls::TlsConfig::builder()
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .build();
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .user_agent(USER_AGENT)
            .tls_config(tls)
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(20)))
            .timeout_recv_response(Some(Duration::from_secs(60)))
            .timeout_recv_body(Some(Duration::from_secs(30 * 60)))
            .build()
            .into();
        Self { agent }
    }

    fn open(&self, url: &str) -> Result<ureq::http::Response<ureq::Body>, InstallError> {
        self.open_with(url, &[])
    }

    fn open_with(&self, url: &str, headers: &[(&str, &str)]) -> Result<ureq::http::Response<ureq::Body>, InstallError> {
        if !url_allowed(url) {
            return Err(InstallError::UrlNotAllowed(url.to_string()));
        }
        let mut req = self.agent.get(url);
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let resp = req.call().map_err(|e| InstallError::Network(format!("{url}: {e}")))?;
        let status = resp.status().as_u16();
        if status == 404 {
            return Err(InstallError::NotFound(url.to_string()));
        }
        if !(200..300).contains(&status) {
            return Err(InstallError::Network(format!("{url}: respuesta HTTP {status}")));
        }
        Ok(resp)
    }

    /// ¿La dirección entrega al menos un byte? Pide solo el primero (`Range`); si el servidor
    /// ignora el rango, se lee un byte y se corta. Sirve para detectar fuentes que responden
    /// «OK» con archivos vacíos antes de que el usuario empiece a crear.
    pub fn delivers_content(&self, url: &str) -> bool {
        if !url_allowed(url) {
            return false;
        }
        let Ok(mut resp) = self.agent.get(url).header("Range", "bytes=0-0").call() else {
            return false;
        };
        if !(200..300).contains(&resp.status().as_u16()) {
            return false;
        }
        let mut one = [0u8; 1];
        matches!(resp.body_mut().as_reader().read(&mut one), Ok(1))
    }

    pub fn get_text(&self, url: &str) -> Result<String, InstallError> {
        let mut resp = self.open(url)?;
        resp.body_mut()
            .with_config()
            .limit(MAX_TEXT_BYTES)
            .read_to_string()
            .map_err(|e| InstallError::Network(format!("{url}: {e}")))
    }

    /// POST con cuerpo JSON y respuesta JSON (consultas por lotes, p. ej. huellas en Modrinth).
    pub fn post_json(&self, url: &str, body: &serde_json::Value) -> Result<serde_json::Value, InstallError> {
        self.post_json_with(url, body, &[])
    }

    /// Como [`Self::post_json`], con cabeceras extra (p. ej. la clave de API de CurseForge).
    pub fn post_json_with(
        &self,
        url: &str,
        body: &serde_json::Value,
        headers: &[(&str, &str)],
    ) -> Result<serde_json::Value, InstallError> {
        if !url_allowed(url) {
            return Err(InstallError::UrlNotAllowed(url.to_string()));
        }
        let mut req = self.agent.post(url).header("Content-Type", "application/json");
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let mut resp = req
            .send(body.to_string())
            .map_err(|e| InstallError::Network(format!("{url}: {e}")))?;
        let status = resp.status().as_u16();
        if status == 404 {
            return Err(InstallError::NotFound(url.to_string()));
        }
        if !(200..300).contains(&status) {
            return Err(InstallError::Network(format!("{url}: respuesta HTTP {status}")));
        }
        let text = resp
            .body_mut()
            .with_config()
            .limit(MAX_TEXT_BYTES)
            .read_to_string()
            .map_err(|e| InstallError::Network(format!("{url}: {e}")))?;
        serde_json::from_str(&text).map_err(|e| InstallError::BadResponse(format!("{url}: JSON no válido ({e})")))
    }

    /// POST de formulario (`application/x-www-form-urlencoded`). Devuelve el código HTTP y el
    /// JSON aunque no sea 2xx: algunas APIs (p. ej. licencias de Gumroad) explican el rechazo en
    /// el cuerpo de un 404.
    pub fn post_form(&self, url: &str, fields: &[(&str, &str)]) -> Result<(u16, serde_json::Value), InstallError> {
        if !url_allowed(url) {
            return Err(InstallError::UrlNotAllowed(url.to_string()));
        }
        let mut resp = self
            .agent
            .post(url)
            .header("Accept", "application/json")
            .send_form(fields.iter().copied())
            .map_err(|e| InstallError::Network(format!("{url}: {e}")))?;
        let status = resp.status().as_u16();
        let text = resp
            .body_mut()
            .with_config()
            .limit(MAX_TEXT_BYTES)
            .read_to_string()
            .map_err(|e| InstallError::Network(format!("{url}: {e}")))?;
        match serde_json::from_str(&text) {
            Ok(v) => Ok((status, v)),
            Err(_) if !(200..300).contains(&status) => {
                Err(InstallError::Network(format!("{url}: respuesta HTTP {status}")))
            }
            Err(e) => Err(InstallError::BadResponse(format!("{url}: JSON no válido ({e})"))),
        }
    }

    /// GET de JSON con cabeceras extra.
    pub fn get_json_with(&self, url: &str, headers: &[(&str, &str)]) -> Result<serde_json::Value, InstallError> {
        let mut resp = self.open_with(url, headers)?;
        let text = resp
            .body_mut()
            .with_config()
            .limit(MAX_TEXT_BYTES)
            .read_to_string()
            .map_err(|e| InstallError::Network(format!("{url}: {e}")))?;
        serde_json::from_str(&text).map_err(|e| InstallError::BadResponse(format!("{url}: JSON no válido ({e})")))
    }

    pub fn get_json(&self, url: &str) -> Result<serde_json::Value, InstallError> {
        let text = self.get_text(url)?;
        serde_json::from_str(&text).map_err(|e| InstallError::BadResponse(format!("{url}: JSON no válido ({e})")))
    }

    /// Descarga `url` a `dest`. Escribe primero en `dest.part`, comprueba tamaño y hash y
    /// solo entonces renombra. Si algo falla, el temporal se borra.
    pub fn download(
        &self,
        url: &str,
        dest: &Path,
        checksum: &Checksum,
        expected_size: Option<u64>,
        progress: &mut dyn FnMut(u64, Option<u64>),
        cancel: &AtomicBool,
    ) -> Result<u64, InstallError> {
        let part = dest.with_extension("part");
        let result = self.download_inner(url, &part, checksum, expected_size, progress, cancel);
        match result {
            Ok(n) => {
                std::fs::rename(&part, dest).map_err(|e| InstallError::Io(format!("{}: {e}", dest.display())))?;
                Ok(n)
            }
            Err(e) => {
                let _ = std::fs::remove_file(&part);
                Err(e)
            }
        }
    }

    fn download_inner(
        &self,
        url: &str,
        part: &Path,
        checksum: &Checksum,
        expected_size: Option<u64>,
        progress: &mut dyn FnMut(u64, Option<u64>),
        cancel: &AtomicBool,
    ) -> Result<u64, InstallError> {
        if expected_size.is_some_and(|s| s > MAX_DOWNLOAD_BYTES) {
            return Err(InstallError::TooLarge(url.to_string()));
        }
        let mut resp = self.open(url)?;
        let header_len = resp
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        let total = expected_size.or(header_len);
        // Para explicar una respuesta vacía en el registro.
        let status = resp.status().as_u16();
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("sin tipo")
            .to_string();
        if total.is_some_and(|s| s > MAX_DOWNLOAD_BYTES) {
            return Err(InstallError::TooLarge(url.to_string()));
        }
        let mut reader = resp.body_mut().with_config().limit(MAX_DOWNLOAD_BYTES + 1).reader();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(part)
            .map_err(|e| InstallError::Io(format!("{}: {e}", part.display())))?;
        let mut md5 = md5::Md5::new();
        let mut sha1 = sha1::Sha1::new();
        let mut sha256 = sha2::Sha256::new();
        let mut buf = vec![0u8; 64 * 1024];
        let mut done: u64 = 0;
        let mut last_report = 0u64;
        progress(0, total);
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(InstallError::Cancelled);
            }
            let n = reader
                .read(&mut buf)
                .map_err(|e| InstallError::Network(format!("{url}: descarga interrumpida ({e})")))?;
            if n == 0 {
                break;
            }
            done += n as u64;
            if done > MAX_DOWNLOAD_BYTES {
                return Err(InstallError::TooLarge(url.to_string()));
            }
            match checksum {
                Checksum::Md5(_) => md5.update(&buf[..n]),
                Checksum::Sha1(_) => sha1.update(&buf[..n]),
                Checksum::Sha256(_) => sha256.update(&buf[..n]),
                Checksum::None => {}
            }
            file.write_all(&buf[..n])
                .map_err(|e| InstallError::Io(format!("{}: {e}", part.display())))?;
            if done - last_report >= 256 * 1024 {
                last_report = done;
                progress(done, total);
            }
        }
        if done == 0 {
            // Hubo conexión y respuesta correcta, pero sin archivo: es un fallo de la fuente.
            return Err(InstallError::BadResponse(format!(
                "la fuente entregó un archivo vacío (HTTP {status}, {content_type}) desde {url}. \
                 No es un problema de tu conexión; prueba más tarde"
            )));
        }
        file.sync_all()
            .map_err(|e| InstallError::Io(format!("{}: {e}", part.display())))?;
        drop(file);
        progress(done, total);
        if let Some(size) = expected_size {
            if size != done {
                return Err(InstallError::Integrity(format!(
                    "tamaño distinto al publicado ({done} bytes, se esperaban {size})"
                )));
            }
        }
        let actual = match checksum {
            Checksum::Md5(_) => Some(hex(&md5.finalize())),
            Checksum::Sha1(_) => Some(hex(&sha1.finalize())),
            Checksum::Sha256(_) => Some(hex(&sha256.finalize())),
            Checksum::None => None,
        };
        if let (Some(actual), Some(expected)) = (actual, checksum.expected()) {
            if !actual.eq_ignore_ascii_case(expected) {
                return Err(InstallError::Integrity(format!(
                    "la huella del archivo no coincide con la publicada ({actual} ≠ {expected})"
                )));
            }
        }
        Ok(done)
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_or_loopback() {
        assert!(url_allowed("https://piston-meta.mojang.com/x"));
        assert!(url_allowed("http://127.0.0.1:8080/x"));
        assert!(url_allowed("http://localhost/x"));
        assert!(!url_allowed("http://piston-meta.mojang.com/x"));
        assert!(!url_allowed("http://127.0.0.1.evil.com/x"));
        assert!(!url_allowed("file:///etc/passwd"));
        assert!(!url_allowed("https:///x"));
    }
}
