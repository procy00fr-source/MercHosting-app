//! Copias de seguridad comprobadas de un servidor.
//!
//! - Cada copia es un `.zip` con un manifiesto (`merchosting-backup.json`) que lista cada archivo
//!   con su tamaño y su SHA-256. Junto al `.zip` va un `.json` con los mismos datos para listar
//!   sin abrirlo.
//! - Al terminar, la copia se **vuelve a leer entera** y se compara con el manifiesto: una copia
//!   que no pasa la verificación no se guarda.
//! - Alcance: el **mundo activo** (con sus dimensiones Bukkit) o el **servidor completo**.
//! - Restaurar: primero se verifica la copia, se extrae a una carpeta temporal dentro del
//!   servidor y solo entonces se cambian las carpetas (lo anterior se aparta y se devuelve a su
//!   sitio si algo falla). Quien llama hace antes una copia «Antes de restaurar».
//! - La consistencia en caliente (pausar el guardado del juego) la coordina el supervisor; aquí
//!   solo se registra si la copia se hizo con el servidor encendido y si se confirmó el guardado.

use crate::worlds::{is_world, validate_world_name, world_dirs, Progress};
use crate::{io, AdminError};
use merc_core::paths::SandboxRoot;
use serde::{Deserialize, Serialize};
use sha2::Digest as _;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const MANIFEST: &str = "merchosting-backup.json";
const FORMAT: &str = "merchosting-backup";
/// Margen libre que se deja siempre en el disco.
const SPARE_BYTES: u64 = 64 * 1024 * 1024;
const CHUNK: usize = 256 * 1024;
/// Prefijo de las carpetas temporales de MercHosting dentro del servidor (no entran en copias).
pub const TEMP_PREFIX: &str = ".merchosting";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupScope {
    /// El mundo activo y sus dimensiones (Bukkit).
    World,
    /// Toda la carpeta del servidor.
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupReason {
    Manual,
    /// Copia automática antes de restaurar otra.
    BeforeRestore,
    /// Hecha por una tarea programada.
    Scheduled,
    /// Copia automática antes de actualizar el servidor.
    BeforeUpdate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

/// Lo que se sabe de una copia (se guarda en el `.json` de al lado y dentro del `.zip`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupInfo {
    pub id: String,
    pub created_unix_ms: i64,
    pub scope: BackupScope,
    pub reason: BackupReason,
    /// Mundo copiado (alcance «mundo») o mundo activo en ese momento.
    pub world: Option<String>,
    pub mc_version: Option<String>,
    /// Tamaño del `.zip`.
    pub size_bytes: u64,
    /// Suma de los archivos sin comprimir.
    pub content_bytes: u64,
    pub files: u64,
    /// Hecha con el servidor encendido.
    pub hot: bool,
    /// Con el servidor apagado, o encendido con el guardado pausado y confirmado.
    pub consistent: bool,
    /// Resultado de la última verificación (`None` si nunca se verificó).
    pub verified: Option<bool>,
    pub verified_unix_ms: Option<i64>,
    /// Qué falló en la última verificación.
    #[serde(default)]
    pub problems: Vec<String>,
    /// Las protegidas no se borran al limpiar copias viejas.
    #[serde(default)]
    pub protected: bool,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    info: BackupInfo,
    files: Vec<ManifestFile>,
}

/// Datos que pone quien crea la copia.
#[derive(Debug, Clone)]
pub struct NewBackup {
    pub scope: BackupScope,
    pub reason: BackupReason,
    pub level_name: String,
    pub mc_version: Option<String>,
    pub hot: bool,
    pub consistent: bool,
    pub note: Option<String>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Identificador de una copia: solo minúsculas, números y guiones.
pub fn check_id(id: &str) -> Result<&str, AdminError> {
    if id.is_empty()
        || id.len() > 64
        || id.starts_with('-')
        || !id
            .bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase() || b == b'-')
    {
        return Err(AdminError::Invalid(format!("copia no válida: «{id}»")));
    }
    Ok(id)
}

pub fn zip_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.zip"))
}

fn info_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.json"))
}

fn write_info(dir: &Path, info: &BackupInfo) -> Result<(), AdminError> {
    let p = info_path(dir, &info.id);
    let tmp = p.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(info).map_err(|e| AdminError::Io(e.to_string()))?;
    std::fs::write(&tmp, text).map_err(|e| io(&tmp, e))?;
    std::fs::rename(&tmp, &p).map_err(|e| io(&p, e))
}

/// Espacio libre del disco donde está `path` (`None` si no se puede saber).
pub fn free_space(path: &Path) -> Option<u64> {
    disk_space(path).map(|(free, _)| free)
}

/// Espacio libre y total del disco donde está `path`.
pub fn disk_space(path: &Path) -> Option<(u64, u64)> {
    disk_info(path).map(|(f, t, _)| (f, t))
}

/// Espacio libre, total y punto de montaje (`C:\`) del disco donde está `path`.
pub fn disk_info(path: &Path) -> Option<(u64, u64, String)> {
    let mut p = path.to_path_buf();
    while !p.exists() {
        p = p.parent()?.to_path_buf();
    }
    let canon = std::fs::canonicalize(&p).ok()?;
    let s = canon.to_string_lossy().to_string();
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s).to_lowercase();
    let disks = sysinfo::Disks::new_with_refreshed_list().ok()?;
    disks
        .iter()
        .filter(|d| {
            let m = d.mount_point().to_string_lossy().to_lowercase();
            s.starts_with(&m)
        })
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| {
            (
                d.available_space(),
                d.total_space(),
                d.mount_point().to_string_lossy().to_string(),
            )
        })
}

fn need_space(path: &Path, bytes: u64, what: &str) -> Result<(), AdminError> {
    if let Some(free) = free_space(path) {
        let needed = bytes.saturating_add(SPARE_BYTES);
        if free < needed {
            return Err(AdminError::Conflict(format!(
                "No hay espacio suficiente en el disco para {what}: hacen falta unos {} MB y quedan {} MB libres. Libera espacio o borra copias viejas.",
                needed / (1024 * 1024),
                free / (1024 * 1024)
            )));
        }
    }
    Ok(())
}

/// Archivos (y carpetas) a copiar, como rutas relativas a la raíz del servidor.
/// Carpetas (relativas) y archivos (relativo, ruta real) que entran en una copia.
type Collected = (Vec<String>, Vec<(String, PathBuf)>);

fn collect(root: &Path, scope: BackupScope, level: &str) -> Result<Collected, AdminError> {
    let mut dirs = vec![];
    let mut files = vec![];
    let starts: Vec<PathBuf> = match scope {
        BackupScope::World => {
            let name = validate_world_name(level)?;
            if !is_world(&root.join(&name)) {
                return Err(AdminError::NotFound(format!(
                    "el mundo «{name}» (el servidor todavía no lo ha creado: enciéndelo una vez)"
                )));
            }
            world_dirs(root, &name).into_iter().map(|(_, p)| p).collect()
        }
        BackupScope::Full => vec![root.to_path_buf()],
    };
    let mut stack = starts;
    while let Some(dir) = stack.pop() {
        if dir != root {
            let rel = dir
                .strip_prefix(root)
                .unwrap_or(&dir)
                .to_string_lossy()
                .replace('\\', "/");
            dirs.push(rel);
        }
        for e in std::fs::read_dir(&dir).map_err(|e| io(&dir, e))?.flatten() {
            let p = e.path();
            let Ok(meta) = std::fs::symlink_metadata(&p) else {
                continue;
            };
            let name = e.file_name().to_string_lossy().to_string();
            // Enlaces, el candado del mundo y las carpetas temporales de MercHosting no se copian.
            if meta.file_type().is_symlink() || name == "session.lock" || name.starts_with(TEMP_PREFIX) {
                continue;
            }
            if meta.is_dir() {
                stack.push(p);
            } else if meta.is_file() {
                let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                files.push((rel, p));
            }
        }
    }
    dirs.sort();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok((dirs, files))
}

/// Crea, verifica y guarda una copia en `dir`. Si algo falla, no queda nada a medias.
pub fn create(root: &Path, dir: &Path, new: &NewBackup, progress: &Progress) -> Result<BackupInfo, AdminError> {
    let root = SandboxRoot::new(root)?.root().to_path_buf();
    std::fs::create_dir_all(dir).map_err(|e| io(dir, e))?;
    let (dirs, files) = collect(&root, new.scope, &new.level_name)?;
    let total: u64 = files
        .iter()
        .map(|(_, p)| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .sum();
    need_space(dir, total, "la copia")?;
    progress.set_total(total.saturating_mul(2));
    let id = format!(
        "{}-{}",
        crate::stamp(),
        match new.scope {
            BackupScope::World => "mundo",
            BackupScope::Full => "completa",
        }
    );
    let tmp = dir.join(format!("{id}.zip.tmp"));
    let result = (|| {
        let file = std::fs::File::create(&tmp).map_err(|e| io(&tmp, e))?;
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true);
        for d in &dirs {
            zip.add_directory(format!("{d}/"), opts)
                .map_err(|e| AdminError::Io(e.to_string()))?;
        }
        let mut listed = vec![];
        let mut buf = vec![0u8; CHUNK];
        for (rel, p) in &files {
            zip.start_file(rel.as_str(), opts)
                .map_err(|e| AdminError::Io(e.to_string()))?;
            let mut f = std::fs::File::open(p).map_err(|e| {
                AdminError::Io(format!(
                    "no se pudo leer «{rel}» ({e}). Si el servidor está encendido y otro programa lo tiene abierto, detén el servidor y haz la copia en frío."
                ))
            })?;
            let mut h = sha2::Sha256::new();
            let mut size = 0u64;
            loop {
                let n = f.read(&mut buf).map_err(|e| AdminError::Io(format!("{rel}: {e}")))?;
                if n == 0 {
                    break;
                }
                h.update(&buf[..n]);
                zip.write_all(&buf[..n]).map_err(|e| AdminError::Io(e.to_string()))?;
                size += n as u64;
                progress.advance(n as u64)?;
            }
            listed.push(ManifestFile {
                path: rel.clone(),
                size,
                sha256: hex(&h.finalize()),
            });
        }
        let content_bytes = listed.iter().map(|f| f.size).sum();
        let mut info = BackupInfo {
            id: id.clone(),
            created_unix_ms: now_ms(),
            scope: new.scope,
            reason: new.reason,
            world: Some(new.level_name.clone()),
            mc_version: new.mc_version.clone(),
            size_bytes: 0,
            content_bytes,
            files: listed.len() as u64,
            hot: new.hot,
            consistent: new.consistent,
            verified: None,
            verified_unix_ms: None,
            problems: vec![],
            protected: false,
            note: new.note.clone(),
        };
        let manifest = Manifest {
            format: FORMAT.into(),
            version: 1,
            info: info.clone(),
            files: listed,
        };
        zip.start_file(MANIFEST, opts)
            .map_err(|e| AdminError::Io(e.to_string()))?;
        zip.write_all(&serde_json::to_vec_pretty(&manifest).map_err(|e| AdminError::Io(e.to_string()))?)
            .map_err(|e| AdminError::Io(e.to_string()))?;
        let f = zip.finish().map_err(|e| AdminError::Io(e.to_string()))?;
        f.sync_all().map_err(|e| io(&tmp, e))?;
        drop(f);
        // Se vuelve a leer todo: una copia que no se puede comprobar no sirve.
        let problems = check_zip(&tmp, progress)?;
        if !problems.is_empty() {
            return Err(AdminError::Io(format!(
                "la copia no pasó la verificación: {}",
                problems.join("; ")
            )));
        }
        info.verified = Some(true);
        info.verified_unix_ms = Some(now_ms());
        info.size_bytes = std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
        Ok(info)
    })();
    match result {
        Ok(info) => {
            let dest = zip_path(dir, &id);
            std::fs::rename(&tmp, &dest).map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                io(&dest, e)
            })?;
            write_info(dir, &info)?;
            Ok(info)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

fn read_manifest(zip: &mut zip::ZipArchive<std::fs::File>) -> Result<Manifest, AdminError> {
    let mut e = zip
        .by_name(MANIFEST)
        .map_err(|_| AdminError::Invalid("la copia no tiene manifiesto: no es una copia de MercHosting".into()))?;
    if e.size() > 512 * 1024 * 1024 {
        return Err(AdminError::Invalid("manifiesto demasiado grande".into()));
    }
    let mut text = String::new();
    e.read_to_string(&mut text)
        .map_err(|_| AdminError::Invalid("manifiesto ilegible".into()))?;
    let m: Manifest = serde_json::from_str(&text).map_err(|_| AdminError::Invalid("manifiesto dañado".into()))?;
    if m.format != FORMAT || m.version != 1 {
        return Err(AdminError::Invalid("formato de copia desconocido".into()));
    }
    Ok(m)
}

/// Ruta segura de una entrada del `.zip` (relativa, sin `..`, sin unidades ni prefijos raros).
fn safe_entry(name: &str) -> Option<String> {
    let n = name.trim_end_matches('/');
    if n.is_empty() || n.starts_with('/') || n.contains('\\') || n.contains(':') {
        return None;
    }
    let ok = n
        .split('/')
        .all(|c| !c.is_empty() && c != "." && c != ".." && !c.chars().any(char::is_control));
    ok.then(|| n.to_string())
}

/// Compara el `.zip` con su manifiesto. Devuelve la lista de problemas (vacía si está bien).
fn check_zip(path: &Path, progress: &Progress) -> Result<Vec<String>, AdminError> {
    let f = std::fs::File::open(path).map_err(|e| io(path, e))?;
    let mut zip = match zip::ZipArchive::new(f) {
        Ok(z) => z,
        Err(_) => return Ok(vec!["el archivo .zip está dañado o incompleto".into()]),
    };
    let m = match read_manifest(&mut zip) {
        Ok(m) => m,
        Err(e) => return Ok(vec![e.to_string()]),
    };
    let mut expected: BTreeMap<String, &ManifestFile> = m.files.iter().map(|f| (f.path.clone(), f)).collect();
    let mut problems = vec![];
    let mut buf = vec![0u8; CHUNK];
    for i in 0..zip.len() {
        let mut e = match zip.by_index(i) {
            Ok(e) => e,
            Err(err) => {
                problems.push(format!("entrada {i} ilegible ({err})"));
                continue;
            }
        };
        let name = e.name().map(|n| n.into_owned()).unwrap_or_default();
        if name == MANIFEST || e.is_dir() {
            continue;
        }
        let Some(want) = expected.remove(&name) else {
            problems.push(format!("«{name}» no está en el manifiesto"));
            continue;
        };
        let mut h = sha2::Sha256::new();
        let mut size = 0u64;
        loop {
            let n = match e.read(&mut buf) {
                Ok(n) => n,
                Err(err) => {
                    problems.push(format!("«{name}» no se puede leer ({err})"));
                    break;
                }
            };
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
            size += n as u64;
            progress.advance(n as u64)?;
        }
        if size != want.size || hex(&h.finalize()) != want.sha256 {
            problems.push(format!("«{name}» no coincide con su huella"));
        }
        if problems.len() >= 20 {
            break;
        }
    }
    for missing in expected.keys().take(20usize.saturating_sub(problems.len())) {
        problems.push(format!("falta «{missing}»"));
    }
    Ok(problems)
}

/// Copias guardadas en `dir`, de la más nueva a la más vieja.
pub fn list(dir: &Path) -> Vec<BackupInfo> {
    let mut out = vec![];
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(id) = name.strip_suffix(".zip") else { continue };
        if check_id(id).is_err() {
            continue;
        }
        let from_json = std::fs::read_to_string(info_path(dir, id))
            .ok()
            .and_then(|t| serde_json::from_str::<BackupInfo>(&t).ok())
            .filter(|i| i.id == id);
        // Sin su `.json` (por ejemplo, copiada a mano): se lee el manifiesto del `.zip`.
        let info = from_json.or_else(|| {
            let f = std::fs::File::open(e.path()).ok()?;
            let mut z = zip::ZipArchive::new(f).ok()?;
            let mut i = read_manifest(&mut z).ok()?.info;
            i.id = id.to_string();
            i.size_bytes = e.metadata().map(|m| m.len()).unwrap_or(0);
            i.verified = None;
            i.verified_unix_ms = None;
            let _ = write_info(dir, &i);
            Some(i)
        });
        if let Some(i) = info {
            out.push(i);
        }
    }
    out.sort_by(|a, b| b.created_unix_ms.cmp(&a.created_unix_ms).then(b.id.cmp(&a.id)));
    out
}

pub fn get(dir: &Path, id: &str) -> Result<BackupInfo, AdminError> {
    check_id(id)?;
    list(dir)
        .into_iter()
        .find(|i| i.id == id)
        .ok_or_else(|| AdminError::NotFound(format!("la copia «{id}»")))
}

/// Vuelve a leer la copia entera y anota el resultado.
pub fn verify(dir: &Path, id: &str, progress: &Progress) -> Result<BackupInfo, AdminError> {
    let mut info = get(dir, id)?;
    let path = zip_path(dir, id);
    progress.set_total(info.content_bytes);
    let problems = check_zip(&path, progress)?;
    info.verified = Some(problems.is_empty());
    info.verified_unix_ms = Some(now_ms());
    info.problems = problems;
    info.size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(info.size_bytes);
    write_info(dir, &info)?;
    Ok(info)
}

pub fn set_protected(dir: &Path, id: &str, protected: bool) -> Result<BackupInfo, AdminError> {
    let mut info = get(dir, id)?;
    info.protected = protected;
    write_info(dir, &info)?;
    Ok(info)
}

/// Borra una copia (el `.zip` y su `.json`).
pub fn delete(dir: &Path, id: &str) -> Result<(), AdminError> {
    let info = get(dir, id)?;
    if info.protected {
        return Err(AdminError::Conflict(
            "Esta copia está protegida: quita la protección antes de borrarla.".into(),
        ));
    }
    let z = zip_path(dir, id);
    std::fs::remove_file(&z).map_err(|e| io(&z, e))?;
    let _ = std::fs::remove_file(info_path(dir, id));
    Ok(())
}

/// Deja como mucho `keep` copias sin proteger (borra las más viejas). Devuelve las borradas.
/// Nunca borra la copia `keep_id` (la recién hecha).
pub fn apply_retention(dir: &Path, keep: u32, keep_id: &str) -> Vec<String> {
    let keep = keep.max(1) as usize;
    let unprotected: Vec<BackupInfo> = list(dir).into_iter().filter(|i| !i.protected).collect();
    let mut removed = vec![];
    for old in unprotected.iter().skip(keep) {
        if old.id != keep_id && delete(dir, &old.id).is_ok() {
            removed.push(old.id.clone());
        }
    }
    removed
}

/// Carpetas o archivos de primer nivel que una copia contiene (lo que se reemplaza al restaurar).
pub fn top_level(dir: &Path, id: &str) -> Result<Vec<String>, AdminError> {
    check_id(id)?;
    let path = zip_path(dir, id);
    let f = std::fs::File::open(&path).map_err(|e| io(&path, e))?;
    let mut zip = zip::ZipArchive::new(f).map_err(|_| AdminError::Invalid("la copia está dañada".into()))?;
    let m = read_manifest(&mut zip)?;
    let mut tops = BTreeSet::new();
    for f in &m.files {
        if let Some(t) = f.path.split('/').next() {
            tops.insert(t.to_string());
        }
    }
    for i in 0..zip.len() {
        if let Ok(e) = zip.by_index(i) {
            if e.is_dir() {
                if let Some(t) = safe_entry(&e.name().map(|n| n.into_owned()).unwrap_or_default())
                    .and_then(|n| n.split('/').next().map(str::to_string))
                {
                    tops.insert(t);
                }
            }
        }
    }
    Ok(tops.into_iter().collect())
}

/// Restaura una copia en el servidor (que debe estar detenido; lo comprueba quien llama).
/// Verifica la copia, la extrae aparte comprobando cada huella y cambia las carpetas al final.
/// Si algo falla, el servidor queda como estaba.
pub fn restore(root: &Path, dir: &Path, id: &str, progress: &Progress) -> Result<Vec<String>, AdminError> {
    let info = get(dir, id)?;
    let sb = SandboxRoot::new(root)?;
    let root = sb.root().to_path_buf();
    let path = zip_path(dir, id);
    progress.set_total(info.content_bytes.saturating_mul(2));
    let problems = check_zip(&path, progress)?;
    if !problems.is_empty() {
        return Err(AdminError::Conflict(format!(
            "La copia está dañada y no se restauró: {}",
            problems.join("; ")
        )));
    }
    need_space(&root, info.content_bytes, "restaurar la copia")?;
    let staging = root.join(format!("{TEMP_PREFIX}-restaurar-{}", crate::stamp()));
    let fresh = staging.join("nuevo");
    let old = staging.join("anterior");
    std::fs::create_dir_all(&fresh).map_err(|e| io(&fresh, e))?;
    std::fs::create_dir_all(&old).map_err(|e| io(&old, e))?;
    let extracted = (|| {
        let f = std::fs::File::open(&path).map_err(|e| io(&path, e))?;
        let mut zip = zip::ZipArchive::new(f).map_err(|_| AdminError::Invalid("la copia está dañada".into()))?;
        let m = read_manifest(&mut zip)?;
        let want: BTreeMap<String, ManifestFile> = m.files.into_iter().map(|f| (f.path.clone(), f)).collect();
        let mut buf = vec![0u8; CHUNK];
        for i in 0..zip.len() {
            let mut e = zip.by_index(i).map_err(|e| AdminError::Invalid(e.to_string()))?;
            let raw = e.name().map(|n| n.into_owned()).unwrap_or_default();
            if raw == MANIFEST {
                continue;
            }
            let rel = safe_entry(&raw)
                .ok_or_else(|| AdminError::Invalid(format!("ruta no permitida en la copia: «{raw}»")))?;
            let dest = fresh.join(&rel);
            if e.is_dir() {
                std::fs::create_dir_all(&dest).map_err(|er| io(&dest, er))?;
                continue;
            }
            let expected = want
                .get(&rel)
                .ok_or_else(|| AdminError::Invalid(format!("«{rel}» no está en el manifiesto")))?;
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|er| io(parent, er))?;
            }
            let mut out = std::fs::File::create(&dest).map_err(|er| io(&dest, er))?;
            let mut h = sha2::Sha256::new();
            let mut size = 0u64;
            loop {
                let n = e.read(&mut buf).map_err(|er| AdminError::Io(format!("{rel}: {er}")))?;
                if n == 0 {
                    break;
                }
                h.update(&buf[..n]);
                out.write_all(&buf[..n]).map_err(|er| io(&dest, er))?;
                size += n as u64;
                progress.advance(n as u64)?;
            }
            if size != expected.size || hex(&h.finalize()) != expected.sha256 {
                return Err(AdminError::Conflict(format!("«{rel}» no coincide con su huella")));
            }
        }
        Ok::<(), AdminError>(())
    })();
    if let Err(e) = extracted {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    // Cambio de carpetas: se aparta lo actual y se pone lo restaurado. Si algo falla, se deshace.
    let tops: Vec<String> = std::fs::read_dir(&fresh)
        .map_err(|e| io(&fresh, e))?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    let mut moved_old: Vec<String> = vec![];
    let mut placed: Vec<String> = vec![];
    let swap = (|| {
        for t in &tops {
            let cur = sb.resolve(t)?;
            if std::fs::symlink_metadata(&cur).is_ok() {
                std::fs::rename(&cur, old.join(t)).map_err(|e| {
                    AdminError::Io(format!(
                        "no se pudo apartar «{t}» ({e}). ¿Lo tiene abierto otro programa?"
                    ))
                })?;
                moved_old.push(t.clone());
            }
            std::fs::rename(fresh.join(t), &cur).map_err(|e| io(&cur, e))?;
            placed.push(t.clone());
        }
        Ok::<(), AdminError>(())
    })();
    if let Err(e) = swap {
        for t in &placed {
            let _ = std::fs::rename(root.join(t), fresh.join(t));
        }
        for t in &moved_old {
            let _ = std::fs::rename(old.join(t), root.join(t));
        }
        let restored = moved_old.iter().all(|t| root.join(t).exists());
        if restored {
            let _ = std::fs::remove_dir_all(&staging);
        }
        return Err(e);
    }
    let _ = std::fs::remove_dir_all(&staging);
    Ok(tops)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        std::fs::create_dir_all(r.join("world/region")).unwrap();
        std::fs::write(r.join("world/level.dat"), "nivel").unwrap();
        std::fs::write(r.join("world/region/r.0.0.mca"), vec![7u8; 300_000]).unwrap();
        std::fs::write(r.join("world/session.lock"), "x").unwrap();
        std::fs::create_dir_all(r.join("world/datapacks")).unwrap();
        std::fs::create_dir_all(r.join("world_nether/DIM-1")).unwrap();
        std::fs::write(r.join("world_nether/DIM-1/n.mca"), "nether").unwrap();
        std::fs::write(r.join("server.properties"), "level-name=world\n").unwrap();
        std::fs::create_dir_all(r.join("mods")).unwrap();
        std::fs::write(r.join("mods/a.jar"), "mod").unwrap();
        d
    }

    fn new(scope: BackupScope) -> NewBackup {
        NewBackup {
            scope,
            reason: BackupReason::Manual,
            level_name: "world".into(),
            mc_version: Some("1.21.1".into()),
            hot: false,
            consistent: true,
            note: None,
        }
    }

    #[test]
    fn world_backup_is_verified_and_restores() {
        let s = server();
        let b = tempfile::tempdir().unwrap();
        let p = Progress::default();
        let info = create(s.path(), b.path(), &new(BackupScope::World), &p).unwrap();
        assert_eq!(info.verified, Some(true));
        assert_eq!(info.files, 3, "level.dat, región y nether; sin session.lock");
        assert!(info.id.ends_with("-mundo"));
        assert_eq!(list(b.path()).len(), 1);
        assert_eq!(top_level(b.path(), &info.id).unwrap(), ["world", "world_nether"]);

        // Se rompe el mundo y se restaura.
        std::fs::write(s.path().join("world/level.dat"), "roto").unwrap();
        std::fs::write(s.path().join("world/nuevo.txt"), "x").unwrap();
        std::fs::remove_dir_all(s.path().join("world_nether")).unwrap();
        restore(s.path(), b.path(), &info.id, &Progress::default()).unwrap();
        assert_eq!(
            std::fs::read_to_string(s.path().join("world/level.dat")).unwrap(),
            "nivel"
        );
        assert!(!s.path().join("world/nuevo.txt").exists());
        assert!(
            s.path().join("world/datapacks").is_dir(),
            "las carpetas vacías también vuelven"
        );
        assert_eq!(
            std::fs::read_to_string(s.path().join("world_nether/DIM-1/n.mca")).unwrap(),
            "nether"
        );
        // Lo que no está en la copia no se toca.
        assert!(s.path().join("mods/a.jar").is_file());
        assert!(!std::fs::read_dir(s.path())
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with(TEMP_PREFIX)));
    }

    #[test]
    fn damaged_backup_is_detected_and_not_restored() {
        let s = server();
        let b = tempfile::tempdir().unwrap();
        let info = create(s.path(), b.path(), &new(BackupScope::Full), &Progress::default()).unwrap();
        assert!(info.files >= 5);
        // Se cambia un byte dentro del .zip.
        let zp = b.path().join(format!("{}.zip", info.id));
        let mut bytes = std::fs::read(&zp).unwrap();
        let pos = bytes.len() / 3;
        bytes[pos] ^= 0xff;
        std::fs::write(&zp, bytes).unwrap();
        let v = verify(b.path(), &info.id, &Progress::default()).unwrap();
        assert_eq!(v.verified, Some(false));
        assert!(!v.problems.is_empty());
        std::fs::write(s.path().join("world/level.dat"), "actual").unwrap();
        assert!(restore(s.path(), b.path(), &info.id, &Progress::default()).is_err());
        assert_eq!(
            std::fs::read_to_string(s.path().join("world/level.dat")).unwrap(),
            "actual"
        );
    }

    #[test]
    fn retention_keeps_newest_and_protected() {
        let s = server();
        let b = tempfile::tempdir().unwrap();
        let mut ids = vec![];
        for _ in 0..4 {
            ids.push(
                create(s.path(), b.path(), &new(BackupScope::World), &Progress::default())
                    .unwrap()
                    .id,
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        set_protected(b.path(), &ids[0], true).unwrap();
        assert!(delete(b.path(), &ids[0]).is_err(), "protegida");
        let removed = apply_retention(b.path(), 2, &ids[3]);
        assert_eq!(removed, [ids[1].clone()]);
        let left: Vec<String> = list(b.path()).into_iter().map(|i| i.id).collect();
        assert_eq!(left, [ids[3].clone(), ids[2].clone(), ids[0].clone()]);
    }

    #[test]
    fn cancel_leaves_nothing_and_ids_are_checked() {
        let s = server();
        let b = tempfile::tempdir().unwrap();
        let p = Progress::default();
        p.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(matches!(
            create(s.path(), b.path(), &new(BackupScope::World), &p),
            Err(AdminError::Cancelled)
        ));
        assert_eq!(std::fs::read_dir(b.path()).unwrap().count(), 0);
        for bad in ["../x", "A", "", "x/y", "-x"] {
            assert!(check_id(bad).is_err(), "{bad}");
        }
        assert_eq!(safe_entry("a/../b"), None);
        assert_eq!(safe_entry("C:/x"), None);
        assert_eq!(safe_entry("world/level.dat").as_deref(), Some("world/level.dat"));
        // Un mundo que todavía no existe.
        let empty = tempfile::tempdir().unwrap();
        assert!(matches!(
            create(empty.path(), b.path(), &new(BackupScope::World), &Progress::default()),
            Err(AdminError::NotFound(_))
        ));
        assert!(free_space(b.path()).is_some());
    }
}
