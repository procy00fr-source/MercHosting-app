//! Paquete de recursos del servidor: Minecraft lo ofrece a cada jugador al entrar.
//!
//! En `server.properties` van la dirección (`resource-pack`) y su SHA-1
//! (`resource-pack-sha1`); con la huella, el juego no vuelve a descargarlo si ya lo tiene y
//! rechaza un archivo distinto. MercHosting descarga el paquete una vez para comprobar que es un
//! paquete de recursos de verdad (un `.zip` con `pack.mcmeta`) y calcular esa huella.

use crate::local::sha1_file;
use crate::ContentError;
use merc_install::{Checksum, Http};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// Límite que acepta el juego para un paquete del servidor.
pub const MAX_PACK_BYTES: u64 = 250 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourcePackInfo {
    pub url: String,
    pub sha1: String,
    pub size: u64,
    /// Descripción de `pack.mcmeta`, si es texto simple.
    pub description: Option<String>,
}

/// La dirección tiene que ser HTTPS y directa a un archivo (sin espacios ni saltos de línea).
pub fn check_url(url: &str) -> Result<&str, ContentError> {
    let u = url.trim();
    // Solo en compilaciones de desarrollo y si se pide expresamente (pruebas con un servidor local).
    let test_loopback = cfg!(debug_assertions)
        && std::env::var_os("MERC_TEST_LOOPBACK").is_some()
        && u.starts_with("http://127.0.0.1:");
    let ok = u.len() <= 2000
        && (test_loopback
            || u.strip_prefix("https://")
                .is_some_and(|r| !r.is_empty() && !r.starts_with('/')))
        && !u.chars().any(|c| c.is_whitespace() || c.is_control());
    if !ok {
        return Err(ContentError::Invalid(
            "La dirección del paquete debe empezar por https:// y apuntar directamente al archivo .zip.".into(),
        ));
    }
    Ok(u)
}

/// Comprueba que el archivo es un paquete de recursos y lee su descripción.
pub fn read_pack(path: &Path) -> Result<Option<String>, ContentError> {
    let f = std::fs::File::open(path).map_err(|e| ContentError::Io(e.to_string()))?;
    let mut zip = zip::ZipArchive::new(f).map_err(|_| {
        ContentError::Invalid("El archivo no es un .zip. Usa el enlace de descarga directa del paquete.".into())
    })?;
    let mut entry = zip.by_name("pack.mcmeta").map_err(|_| {
        ContentError::Invalid(
            "El .zip no tiene pack.mcmeta en la raíz: no es un paquete de recursos (o está dentro de otra carpeta)."
                .into(),
        )
    })?;
    if entry.size() > 1024 * 1024 {
        return Err(ContentError::Invalid("pack.mcmeta es demasiado grande".into()));
    }
    let mut text = String::new();
    std::io::Read::read_to_string(&mut entry, &mut text)
        .map_err(|_| ContentError::Invalid("pack.mcmeta no se puede leer como texto".into()))?;
    let v: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|_| ContentError::Invalid("pack.mcmeta no es JSON válido".into()))?;
    let pack = v
        .get("pack")
        .ok_or_else(|| ContentError::Invalid("pack.mcmeta no tiene la sección «pack»".into()))?;
    Ok(pack
        .get("description")
        .and_then(|d| d.as_str())
        .map(|d| d.chars().take(200).collect()))
}

/// Descarga el paquete a `staging` (se borra al terminar), lo comprueba y calcula su SHA-1.
pub fn inspect(http: &Http, url: &str, staging: &Path) -> Result<ResourcePackInfo, ContentError> {
    let url = check_url(url)?.to_string();
    std::fs::create_dir_all(staging).map_err(|e| ContentError::Io(e.to_string()))?;
    let dest = staging.join("paquete.zip");
    let too_big = AtomicBool::new(false);
    let result = (|| {
        let mut cb = |done: u64, total: Option<u64>| {
            if done > MAX_PACK_BYTES || total.is_some_and(|t| t > MAX_PACK_BYTES) {
                too_big.store(true, Ordering::Relaxed);
            }
        };
        match http.download(&url, &dest, &Checksum::None, None, &mut cb, &too_big) {
            Ok(_) => {}
            Err(merc_install::InstallError::Cancelled) => {
                return Err(ContentError::Invalid(
                    "El paquete pasa de 250 MB, el máximo que acepta Minecraft.".into(),
                ))
            }
            Err(e) => return Err(e.into()),
        }
        let size = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
        if size > MAX_PACK_BYTES {
            return Err(ContentError::Invalid(
                "El paquete pasa de 250 MB, el máximo que acepta Minecraft.".into(),
            ));
        }
        let description = read_pack(&dest)?;
        Ok(ResourcePackInfo {
            sha1: sha1_file(&dest)?,
            url: url.clone(),
            size,
            description,
        })
    })();
    let _ = std::fs::remove_dir_all(staging);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(path: &Path, files: &[(&str, &str)]) {
        let f = std::fs::File::create(path).unwrap();
        let mut z = zip::ZipWriter::new(f);
        for (n, c) in files {
            z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(c.as_bytes()).unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn urls_and_packs_are_checked() {
        assert!(check_url("https://ejemplo.dev/pack.zip").is_ok());
        for bad in [
            "http://ejemplo.dev/p.zip",
            "https://",
            "https://a b",
            "ftp://x",
            "https:///x",
        ] {
            assert!(check_url(bad).is_err(), "{bad}");
        }
        let d = tempfile::tempdir().unwrap();
        let ok = d.path().join("ok.zip");
        make_zip(
            &ok,
            &[(
                "pack.mcmeta",
                r#"{"pack":{"pack_format":34,"description":"Mi paquete"}}"#,
            )],
        );
        assert_eq!(read_pack(&ok).unwrap().as_deref(), Some("Mi paquete"));
        let nested = d.path().join("nested.zip");
        make_zip(&nested, &[("carpeta/pack.mcmeta", r#"{"pack":{}}"#)]);
        assert!(read_pack(&nested).is_err());
        let notzip = d.path().join("x.zip");
        std::fs::write(&notzip, "<html>").unwrap();
        assert!(read_pack(&notzip).is_err());
    }
}
