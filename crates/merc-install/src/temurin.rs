//! Java de Eclipse Temurin (Adoptium): se pregunta a la API oficial por el último JRE de una
//! versión para Windows x64 y se descarga el .zip con su SHA-256 publicado. No se instala nada
//! en Windows: MercHosting lo descomprime en su carpeta de datos.

use crate::{Checksum, InstallError};
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const API_BASE: &str = "https://api.adoptium.net";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JreRelease {
    /// `jdk-21.0.5+11`
    pub release_name: String,
    /// `21.0.5+11`
    pub semver: String,
    pub file_name: String,
    pub link: String,
    pub sha256: String,
    pub size: Option<u64>,
}

impl JreRelease {
    pub fn checksum(&self) -> Checksum {
        Checksum::Sha256(self.sha256.clone())
    }

    /// Carpeta donde queda instalado (dentro de la carpeta de Java de MercHosting).
    pub fn folder_name(&self) -> String {
        format!("{}-jre", self.release_name.replace(['/', '\\', ':'], "_"))
    }
}

pub fn assets_url(base: &str, major: u32) -> String {
    format!(
        "{}/v3/assets/latest/{major}/hotspot?architecture=x64&image_type=jre&os=windows&vendor=eclipse",
        base.trim_end_matches('/')
    )
}

/// Elige el .zip del JRE de Windows x64 en la respuesta de la API.
pub fn parse_latest(v: &Value) -> Result<JreRelease, InstallError> {
    let list = v
        .as_array()
        .ok_or_else(|| InstallError::NotAvailable("respuesta inesperada de Adoptium".into()))?;
    for item in list {
        let b = item.get("binary").unwrap_or(&Value::Null);
        let s = |o: &Value, k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        if s(b, "image_type") != "jre" || s(b, "os") != "windows" || s(b, "architecture") != "x64" {
            continue;
        }
        let p = b.get("package").unwrap_or(&Value::Null);
        let link = s(p, "link");
        let sha256 = s(p, "checksum").to_ascii_lowercase();
        // Solo HTTPS (en compilaciones de desarrollo también un servidor local de pruebas).
        let local_test = cfg!(debug_assertions) && link.starts_with("http://127.0.0.1");
        if !(link.starts_with("https://") || local_test)
            || !link.to_ascii_lowercase().ends_with(".zip")
            || sha256.len() != 64
        {
            continue;
        }
        return Ok(JreRelease {
            release_name: s(item, "release_name"),
            semver: item
                .pointer("/version/semver")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            file_name: s(p, "name"),
            link,
            sha256,
            size: p.get("size").and_then(Value::as_u64),
        });
    }
    Err(InstallError::NotAvailable(
        "Adoptium no publica un JRE para Windows x64 de esa versión".into(),
    ))
}

/// Descomprime el .zip en `dest` (que no debe existir) sin dejar salir ningún archivo de ahí.
/// Devuelve la ruta de `bin/java(.exe)` dentro.
pub fn extract(zip_path: &Path, dest: &Path) -> Result<PathBuf, InstallError> {
    let io = |e: std::io::Error| InstallError::Io(e.to_string());
    let f = std::fs::File::open(zip_path).map_err(io)?;
    let mut zip = zip::ZipArchive::new(f).map_err(|e| InstallError::Io(format!("zip dañado: {e}")))?;
    std::fs::create_dir_all(dest).map_err(io)?;
    for i in 0..zip.len() {
        let mut e = zip.by_index(i).map_err(|e| InstallError::Io(e.to_string()))?;
        // `enclosed_name` rechaza rutas absolutas y `..` (zip slip).
        let Some(rel) = e.enclosed_name() else {
            return Err(InstallError::Io(format!(
                "ruta no permitida en el zip: {}",
                e.name().map(|n| n.into_owned()).unwrap_or_default()
            )));
        };
        let out = dest.join(rel);
        if e.is_dir() {
            std::fs::create_dir_all(&out).map_err(io)?;
            continue;
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p).map_err(io)?;
        }
        let mut w = std::fs::File::create(&out).map_err(io)?;
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = e.read(&mut buf).map_err(io)?;
            if n == 0 {
                break;
            }
            std::io::Write::write_all(&mut w, &buf[..n]).map_err(io)?;
        }
        #[cfg(unix)]
        if let Some(mode) = e.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode & 0o755));
        }
    }
    let exe = merc_core::java::java_exe_name();
    // El zip trae una carpeta raíz (jdk-21.0.5+11-jre/) con bin/ dentro.
    let direct = dest.join("bin").join(exe);
    if direct.is_file() {
        return Ok(direct);
    }
    for entry in std::fs::read_dir(dest).map_err(io)?.flatten() {
        let p = entry.path().join("bin").join(exe);
        if p.is_file() {
            return Ok(p);
        }
    }
    Err(InstallError::Io("el zip no trae bin/java".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    #[test]
    fn picks_the_windows_jre_zip() {
        let v = json!([
            {"binary": {"image_type": "jdk", "os": "windows", "architecture": "x64", "package": {"link": "https://github.com/x/jdk.zip", "checksum": "a".repeat(64), "name": "jdk.zip"}}, "release_name": "jdk-21.0.5+11", "version": {"semver": "21.0.5+11"}},
            {"binary": {"image_type": "jre", "os": "windows", "architecture": "x64",
                "installer": {"link": "https://github.com/x/jre.msi"},
                "package": {"link": "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.5%2B11/OpenJDK21U-jre_x64_windows_hotspot_21.0.5_11.zip",
                    "checksum": "B".repeat(64), "name": "OpenJDK21U-jre_x64_windows_hotspot_21.0.5_11.zip", "size": 47_000_000}},
             "release_name": "jdk-21.0.5+11", "version": {"semver": "21.0.5+11"}}
        ]);
        let r = parse_latest(&v).unwrap();
        assert!(r.link.ends_with("_11.zip"));
        assert_eq!(r.sha256, "b".repeat(64));
        assert_eq!(r.size, Some(47_000_000));
        assert_eq!(r.folder_name(), "jdk-21.0.5+11-jre");
        assert!(parse_latest(&json!([])).is_err());
        assert!(assets_url("https://api.adoptium.net/", 17).contains("/v3/assets/latest/17/hotspot?"));
    }

    #[test]
    fn extracts_safely() {
        let d = tempfile::tempdir().unwrap();
        let z = d.path().join("j.zip");
        {
            let mut w = zip::ZipWriter::new(std::fs::File::create(&z).unwrap());
            let exe = merc_core::java::java_exe_name();
            w.start_file(
                format!("jdk-21-jre/bin/{exe}"),
                zip::write::SimpleFileOptions::default().unix_permissions(0o755),
            )
            .unwrap();
            w.write_all(b"#!/bin/sh\n").unwrap();
            w.finish().unwrap();
        }
        let java = extract(&z, &d.path().join("out")).unwrap();
        assert!(java.ends_with(Path::new("jdk-21-jre/bin").join(merc_core::java::java_exe_name())));
        // Un zip con ../ no sale de su carpeta.
        let bad = d.path().join("bad.zip");
        {
            let mut w = zip::ZipWriter::new(std::fs::File::create(&bad).unwrap());
            w.start_file("../fuera.txt", zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(b"x").unwrap();
            w.finish().unwrap();
        }
        assert!(extract(&bad, &d.path().join("out2")).is_err());
        assert!(!d.path().join("fuera.txt").exists());
    }
}
