//! Java de Eclipse Temurin (Adoptium): se pregunta a la API oficial por el último JRE de una
//! versión para este sistema (Windows x64: .zip; Linux x64: .tar.gz) y se descarga con su
//! SHA-256 publicado. No se instala nada en el sistema: MercHosting lo descomprime en su carpeta
//! de datos.

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

/// Sistema tal como lo nombra la API de Adoptium.
pub fn adoptium_os() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else {
        "linux"
    }
}

/// Extensión del paquete del JRE en ese sistema.
fn package_ext(os: &str) -> &'static str {
    if os == "windows" {
        ".zip"
    } else {
        ".tar.gz"
    }
}

pub fn assets_url(base: &str, major: u32) -> String {
    assets_url_for(base, major, adoptium_os())
}

pub fn assets_url_for(base: &str, major: u32, os: &str) -> String {
    format!(
        "{}/v3/assets/latest/{major}/hotspot?architecture=x64&image_type=jre&os={os}&vendor=eclipse",
        base.trim_end_matches('/')
    )
}

/// Elige el paquete del JRE x64 de este sistema en la respuesta de la API.
pub fn parse_latest(v: &Value) -> Result<JreRelease, InstallError> {
    parse_latest_for(v, adoptium_os())
}

/// Como [`parse_latest`], para un sistema concreto (`windows` o `linux`).
pub fn parse_latest_for(v: &Value, os: &str) -> Result<JreRelease, InstallError> {
    let ext = package_ext(os);
    let list = v
        .as_array()
        .ok_or_else(|| InstallError::NotAvailable("respuesta inesperada de Adoptium".into()))?;
    for item in list {
        let b = item.get("binary").unwrap_or(&Value::Null);
        let s = |o: &Value, k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        if s(b, "image_type") != "jre" || s(b, "os") != os || s(b, "architecture") != "x64" {
            continue;
        }
        let p = b.get("package").unwrap_or(&Value::Null);
        let link = s(p, "link");
        let sha256 = s(p, "checksum").to_ascii_lowercase();
        // Solo HTTPS (en compilaciones de desarrollo también un servidor local de pruebas).
        let local_test = cfg!(debug_assertions) && link.starts_with("http://127.0.0.1");
        if !(link.starts_with("https://") || local_test)
            || !link.to_ascii_lowercase().ends_with(ext)
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
    Err(InstallError::NotAvailable(format!(
        "Adoptium no publica un JRE para {} x64 de esa versión",
        if os == "windows" { "Windows" } else { "Linux" }
    )))
}

/// Descomprime el paquete descargado (.zip o .tar.gz según su nombre) en `dest`.
pub fn extract_package(file: &Path, dest: &Path) -> Result<PathBuf, InstallError> {
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        extract_tar_gz(file, dest)
    } else {
        extract(file, dest)
    }
}

/// Descomprime un .tar.gz en `dest` (que no debe existir) sin dejar salir nada de ahí: se
/// rechazan rutas absolutas o con `..`, y los enlaces solo se aceptan si apuntan dentro del
/// propio paquete (el JRE de Linux trae algunos, como `legal/`). Devuelve `bin/java`.
pub fn extract_tar_gz(file: &Path, dest: &Path) -> Result<PathBuf, InstallError> {
    let f = std::fs::File::open(file).map_err(|e| InstallError::Io(e.to_string()))?;
    extract_tar(flate2::read::GzDecoder::new(f), dest)?;
    find_java(dest)
}

/// Como [`extract_tar_gz`] pero para .tar.xz (el JRE para Android). El .xz se descomprime
/// primero a un .tar temporal junto a `dest` (decodificador XZ en Rust puro, lzma-rs) y luego se
/// extrae con las mismas reglas. No busca `bin/java`: quien llama comprueba lo que necesita.
pub fn extract_tar_xz(file: &Path, dest: &Path) -> Result<(), InstallError> {
    let io = |e: std::io::Error| InstallError::Io(e.to_string());
    let tmp = dest.with_extension("tar.tmp");
    let result = (|| {
        let mut input = std::io::BufReader::new(std::fs::File::open(file).map_err(io)?);
        let mut out = std::io::BufWriter::new(std::fs::File::create(&tmp).map_err(io)?);
        lzma_rs::xz_decompress(&mut input, &mut out)
            .map_err(|e| InstallError::Io(format!("paquete .xz dañado: {e:?}")))?;
        std::io::Write::flush(&mut out).map_err(io)?;
        drop(out);
        extract_tar(std::fs::File::open(&tmp).map_err(io)?, dest)
    })();
    let _ = std::fs::remove_file(&tmp);
    result
}

/// Extrae un tar en `dest` con las reglas de seguridad de los JRE (sin rutas fuera, enlaces solo
/// hacia dentro del paquete, sin enlaces duros).
pub fn extract_tar<R: std::io::Read>(reader: R, dest: &Path) -> Result<(), InstallError> {
    use std::path::Component;
    let io = |e: std::io::Error| InstallError::Io(e.to_string());
    let mut ar = tar::Archive::new(reader);
    std::fs::create_dir_all(dest).map_err(io)?;
    let safe = |p: &Path| {
        p.components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
    };
    for entry in ar.entries().map_err(io)? {
        let mut e = entry.map_err(io)?;
        let rel = e.path().map_err(io)?.into_owned();
        if !safe(&rel) {
            return Err(InstallError::Io(format!(
                "ruta no permitida en el paquete: {}",
                rel.display()
            )));
        }
        let kind = e.header().entry_type();
        if kind.is_symlink() || kind.is_hard_link() {
            let target = e.link_name().map_err(io)?.map(|t| t.into_owned()).unwrap_or_default();
            // El destino del enlace, visto desde su carpeta, no puede salir del paquete.
            let base = rel.parent().unwrap_or(Path::new(""));
            let mut depth: i64 = base.components().count() as i64;
            let mut ok = !target.is_absolute();
            for c in target.components() {
                match c {
                    Component::ParentDir => depth -= 1,
                    Component::Normal(_) => depth += 1,
                    Component::CurDir => {}
                    _ => ok = false,
                }
                if depth < 0 {
                    ok = false;
                }
            }
            if !ok || kind.is_hard_link() {
                return Err(InstallError::Io(format!(
                    "enlace no permitido en el paquete: {}",
                    rel.display()
                )));
            }
        } else if !(kind.is_file() || kind.is_dir()) {
            continue;
        }
        // `unpack_in` vuelve a comprobar que nada se escribe fuera de `dest`.
        if !e.unpack_in(dest).map_err(io)? {
            return Err(InstallError::Io(format!(
                "ruta no permitida en el paquete: {}",
                rel.display()
            )));
        }
    }
    Ok(())
}

fn find_java(dest: &Path) -> Result<PathBuf, InstallError> {
    let io = |e: std::io::Error| InstallError::Io(e.to_string());
    let exe = merc_core::java::java_exe_name();
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
    Err(InstallError::Io("el paquete no trae bin/java".into()))
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
    // El zip trae una carpeta raíz (jdk-21.0.5+11-jre/) con bin/ dentro.
    find_java(dest)
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
        let r = parse_latest_for(&v, "windows").unwrap();
        assert!(r.link.ends_with("_11.zip"));
        assert_eq!(r.sha256, "b".repeat(64));
        assert_eq!(r.size, Some(47_000_000));
        assert_eq!(r.folder_name(), "jdk-21.0.5+11-jre");
        assert!(parse_latest(&json!([])).is_err());
        assert!(assets_url("https://api.adoptium.net/", 17).contains("/v3/assets/latest/17/hotspot?"));
        // El paquete de Windows no sirve en Linux.
        assert!(parse_latest_for(&v, "linux").is_err());
    }

    #[test]
    fn picks_the_linux_jre_tar_gz() {
        let v = json!([
            {"binary": {"image_type": "jre", "os": "linux", "architecture": "x64",
                "package": {"link": "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.5%2B11/OpenJDK21U-jre_x64_linux_hotspot_21.0.5_11.tar.gz",
                    "checksum": "c".repeat(64), "name": "OpenJDK21U-jre_x64_linux_hotspot_21.0.5_11.tar.gz", "size": 48_000_000}},
             "release_name": "jdk-21.0.5+11", "version": {"semver": "21.0.5+11"}}
        ]);
        let r = parse_latest_for(&v, "linux").unwrap();
        assert!(r.file_name.ends_with(".tar.gz"));
        assert!(parse_latest_for(&v, "windows").is_err());
        assert!(assets_url_for("https://api.adoptium.net", 21, "linux").contains("&os=linux&"));
    }

    fn tar_gz(path: &Path, entries: &[(&str, Option<&str>)]) {
        let gz = flate2::write::GzEncoder::new(std::fs::File::create(path).unwrap(), flate2::Compression::fast());
        let mut b = tar::Builder::new(gz);
        for (name, link) in entries {
            let mut h = tar::Header::new_gnu();
            match link {
                None => {
                    h.set_entry_type(tar::EntryType::Regular);
                    h.set_size(10);
                    h.set_mode(0o755);
                    h.set_cksum();
                    b.append_data(&mut h, name, &b"#!/bin/sh\n"[..]).unwrap();
                }
                Some(t) => {
                    h.set_entry_type(tar::EntryType::Symlink);
                    h.set_size(0);
                    b.append_link(&mut h, name, t).unwrap();
                }
            }
        }
        b.into_inner().unwrap().finish().unwrap();
    }

    #[test]
    fn extracts_linux_tar_gz_safely() {
        let d = tempfile::tempdir().unwrap();
        let ok = d.path().join("jre.tar.gz");
        tar_gz(
            &ok,
            &[
                ("jdk-21-jre/bin/java", None),
                ("jdk-21-jre/legal/java.base/LICENSE", Some("../../LICENSE")),
            ],
        );
        let java = extract_package(&ok, &d.path().join("out")).unwrap();
        assert!(java.ends_with("jdk-21-jre/bin/java"));
        // Un enlace que sale del paquete se rechaza.
        let bad = d.path().join("bad.tar.gz");
        tar_gz(
            &bad,
            &[("jdk/bin/java", None), ("jdk/escape", Some("../../../etc/passwd"))],
        );
        assert!(extract_package(&bad, &d.path().join("out2")).is_err());
        let abs = d.path().join("abs.tar.gz");
        tar_gz(&abs, &[("jdk/bin/java", None), ("jdk/abs", Some("/etc/passwd"))]);
        assert!(extract_package(&abs, &d.path().join("out3")).is_err());
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
