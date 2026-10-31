//! Mundos de un servidor: listar, duplicar, exportar a .zip, importar desde .zip y elegir el activo.
//!
//! Un mundo es una carpeta de la raíz del servidor con `level.dat`. Los servidores de la familia
//! Bukkit (Paper, Spigot, Purpur…) guardan el Nether y el End en carpetas hermanas
//! (`world_nether`, `world_the_end`): se tratan como parte del mismo mundo.

use crate::{io, AdminError};
use merc_core::paths::SandboxRoot;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

const COMPANIONS: [&str; 2] = ["_nether", "_the_end"];
/// Límites al importar: protegen de .zip enormes o «bombas» de compresión.
const MAX_IMPORT_BYTES: u64 = 32 * 1024 * 1024 * 1024;
const MAX_IMPORT_ENTRIES: usize = 500_000;

/// Progreso compartido de una operación larga (duplicar, exportar, importar): la interfaz lo
/// consulta mientras tanto y puede pedir cancelar.
#[derive(Debug, Default)]
pub struct Progress {
    pub done: AtomicU64,
    pub total: AtomicU64,
    pub cancel: AtomicBool,
}

impl Progress {
    pub(crate) fn set_total(&self, n: u64) {
        self.total.store(n, Ordering::Relaxed);
    }

    pub(crate) fn advance(&self, n: u64) -> Result<(), AdminError> {
        self.done.fetch_add(n, Ordering::Relaxed);
        self.check()
    }

    pub(crate) fn check(&self) -> Result<(), AdminError> {
        if self.cancel.load(Ordering::Relaxed) {
            Err(AdminError::Cancelled)
        } else {
            Ok(())
        }
    }
}

const CHUNK: usize = 256 * 1024;

/// Copia `r` en `w` por bloques, contando el progreso y atendiendo la cancelación.
fn pump(r: &mut impl Read, w: &mut impl Write, progress: &Progress) -> Result<u64, AdminError> {
    let mut buf = vec![0u8; CHUNK];
    let mut n_total = 0u64;
    loop {
        let n = r.read(&mut buf).map_err(|e| AdminError::Io(e.to_string()))?;
        if n == 0 {
            return Ok(n_total);
        }
        w.write_all(&buf[..n]).map_err(|e| AdminError::Io(e.to_string()))?;
        n_total += n as u64;
        progress.advance(n as u64)?;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldInfo {
    pub name: String,
    pub size_bytes: u64,
    pub modified_unix_ms: Option<i64>,
    /// Es el mundo de `level-name` (el que carga el servidor).
    pub active: bool,
    /// Carpetas hermanas de dimensiones (Bukkit), si existen.
    pub companions: Vec<String>,
}

fn dir_size(p: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(p) else {
        return 0;
    };
    if meta.file_type().is_symlink() {
        return 0;
    }
    if meta.is_file() {
        return meta.len();
    }
    std::fs::read_dir(p)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| dir_size(&e.path()))
        .sum()
}

fn modified_ms(p: &Path) -> Option<i64> {
    std::fs::metadata(p)
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as i64)
}

/// Nombre de mundo válido: una sola carpeta, sin rutas ni nombres reservados.
pub fn validate_world_name(name: &str) -> Result<String, AdminError> {
    let n = name.trim();
    if n.contains(['/', '\\']) || n.chars().count() > 64 {
        return Err(AdminError::Invalid(
            "el nombre del mundo no puede llevar carpetas (máximo 64 caracteres)".into(),
        ));
    }
    SandboxRoot::normalize_relative(n).map_err(|e| AdminError::Invalid(format!("nombre de mundo no válido: {e}")))?;
    Ok(n.to_string())
}

pub(crate) fn is_world(dir: &Path) -> bool {
    dir.join("level.dat").is_file()
}

pub fn list_worlds(root: &Path, level_name: &str) -> Result<Vec<WorldInfo>, AdminError> {
    let mut names: Vec<String> = std::fs::read_dir(root)
        .map_err(|e| io(root, e))?
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| is_world(&root.join(n)))
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    let is_companion = |n: &str, all: &[String]| {
        COMPANIONS
            .iter()
            .any(|suf| n.strip_suffix(suf).is_some_and(|base| all.iter().any(|x| x == base)))
    };
    let mut out = vec![];
    for n in names.iter().filter(|n| !is_companion(n, &names)) {
        let companions: Vec<String> = COMPANIONS
            .iter()
            .map(|suf| format!("{n}{suf}"))
            .filter(|c| root.join(c).is_dir())
            .collect();
        let size_bytes = dir_size(&root.join(n)) + companions.iter().map(|c| dir_size(&root.join(c))).sum::<u64>();
        out.push(WorldInfo {
            name: n.clone(),
            size_bytes,
            modified_unix_ms: modified_ms(&root.join(n).join("level.dat")),
            active: n == level_name,
            companions,
        });
    }
    Ok(out)
}

fn copy_tree(from: &Path, to: &Path, progress: &Progress) -> Result<(), AdminError> {
    let meta = std::fs::symlink_metadata(from).map_err(|e| io(from, e))?;
    if meta.file_type().is_symlink() {
        return Ok(());
    }
    if meta.is_dir() {
        std::fs::create_dir_all(to).map_err(|e| io(to, e))?;
        for e in std::fs::read_dir(from).map_err(|e| io(from, e))? {
            let e = e.map_err(|e| io(from, e))?;
            // El candado de sesión es del proceso que tenía el mundo abierto: no se copia.
            if e.file_name() == "session.lock" {
                continue;
            }
            copy_tree(&e.path(), &to.join(e.file_name()), progress)?;
        }
    } else {
        let mut r = std::fs::File::open(from).map_err(|e| io(from, e))?;
        let mut w = std::fs::File::create(to).map_err(|e| io(to, e))?;
        pump(&mut r, &mut w, progress)?;
        if let Ok(t) = meta.modified() {
            let _ = w.set_modified(t);
        }
    }
    Ok(())
}

/// Carpetas que no se copian al clonar un servidor: registros e informes de errores del original.
const CLONE_SKIP: [&str; 2] = ["logs", "crash-reports"];

/// Copia la carpeta de un servidor en `to` (que no debe existir) para clonarlo. No sigue enlaces,
/// no copia candados de sesión, registros ni informes de errores. Si falla o se cancela, borra lo
/// copiado a medias.
pub fn clone_server(from: &Path, to: &Path, progress: &Progress) -> Result<(), AdminError> {
    if to.exists() {
        return Err(AdminError::Conflict(format!("{} ya existe", to.display())));
    }
    progress.set_total(dir_size(from));
    let run = || -> Result<(), AdminError> {
        std::fs::create_dir_all(to).map_err(|e| io(to, e))?;
        for e in std::fs::read_dir(from).map_err(|e| io(from, e))? {
            let e = e.map_err(|e| io(from, e))?;
            if CLONE_SKIP.iter().any(|s| e.file_name() == *s) {
                continue;
            }
            copy_tree(&e.path(), &to.join(e.file_name()), progress)?;
        }
        Ok(())
    };
    let r = run();
    if r.is_err() {
        let _ = std::fs::remove_dir_all(to);
    }
    r
}

pub(crate) fn world_dirs(root: &Path, name: &str) -> Vec<(String, PathBuf)> {
    std::iter::once(String::new())
        .chain(COMPANIONS.iter().map(|s| s.to_string()))
        .map(|suf| (suf.clone(), root.join(format!("{name}{suf}"))))
        .filter(|(_, p)| p.is_dir())
        .collect()
}

/// Copia un mundo (y sus dimensiones Bukkit) con otro nombre.
pub fn duplicate(root: &Path, name: &str, new_name: &str, progress: &Progress) -> Result<(), AdminError> {
    let name = validate_world_name(name)?;
    let new_name = validate_world_name(new_name)?;
    let sb = SandboxRoot::new(root)?;
    let src = sb.resolve(&name)?;
    if !is_world(&src) {
        return Err(AdminError::NotFound(format!("el mundo «{name}»")));
    }
    for suf in std::iter::once("").chain(COMPANIONS) {
        if sb.resolve(&format!("{new_name}{suf}"))?.exists() {
            return Err(AdminError::Conflict(format!("ya existe una carpeta «{new_name}{suf}»")));
        }
    }
    let dirs = world_dirs(sb.root(), &name);
    progress.set_total(dirs.iter().map(|(_, d)| dir_size(d)).sum());
    let mut done = vec![];
    for (suf, from) in dirs {
        let to = sb.resolve(&format!("{new_name}{suf}"))?;
        if let Err(e) = copy_tree(&from, &to, progress) {
            // Deshacer lo copiado: son carpetas nuevas que creamos nosotros.
            let _ = std::fs::remove_dir_all(&to);
            for d in &done {
                let _ = std::fs::remove_dir_all(d);
            }
            return Err(e);
        }
        done.push(to);
    }
    Ok(())
}

fn add_dir_to_zip<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    base: &Path,
    prefix: &str,
    opts: zip::write::SimpleFileOptions,
    progress: &Progress,
) -> Result<(), AdminError> {
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).map_err(|e| io(&dir, e))?.flatten() {
            let p = e.path();
            let Ok(meta) = std::fs::symlink_metadata(&p) else {
                continue;
            };
            if meta.file_type().is_symlink() || e.file_name() == "session.lock" {
                continue;
            }
            let rel = p.strip_prefix(base).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            let name = format!("{prefix}/{rel}");
            if meta.is_dir() {
                zip.add_directory(format!("{name}/"), opts)
                    .map_err(|e| AdminError::Io(e.to_string()))?;
                stack.push(p);
            } else {
                zip.start_file(name, opts).map_err(|e| AdminError::Io(e.to_string()))?;
                let mut f = std::fs::File::open(&p).map_err(|e| io(&p, e))?;
                pump(&mut f, zip, progress)?;
            }
        }
    }
    Ok(())
}

/// Exporta el mundo (con sus dimensiones Bukkit) a un .zip elegido por el usuario.
pub fn export_zip(root: &Path, name: &str, dest: &Path, progress: &Progress) -> Result<u64, AdminError> {
    let name = validate_world_name(name)?;
    let sb = SandboxRoot::new(root)?;
    if !is_world(&sb.resolve(&name)?) {
        return Err(AdminError::NotFound(format!("el mundo «{name}»")));
    }
    if !dest.is_absolute() {
        return Err(AdminError::Invalid("destino no válido".into()));
    }
    let tmp = dest.with_extension("zip.merchosting-tmp");
    let result = (|| {
        let file = std::fs::File::create(&tmp).map_err(|e| io(&tmp, e))?;
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true);
        let dirs = world_dirs(sb.root(), &name);
        progress.set_total(dirs.iter().map(|(_, d)| dir_size(d)).sum());
        for (suf, dir) in dirs {
            add_dir_to_zip(&mut zip, &dir, &format!("{name}{suf}"), opts, progress)?;
        }
        zip.finish().map_err(|e| AdminError::Io(e.to_string()))?;
        Ok::<(), AdminError>(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, dest).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        io(dest, e)
    })?;
    Ok(std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0))
}

/// Importa un mundo desde un .zip como una carpeta nueva `new_name`. Busca el `level.dat` menos
/// profundo del archivo y extrae solo esa carpeta, con todas las rutas comprobadas.
pub fn import_zip(root: &Path, zip_path: &Path, new_name: &str, progress: &Progress) -> Result<(), AdminError> {
    let new_name = validate_world_name(new_name)?;
    let sb = SandboxRoot::new(root)?;
    let target = sb.resolve(&new_name)?;
    if target.exists() {
        return Err(AdminError::Conflict(format!("ya existe una carpeta «{new_name}»")));
    }
    let file = std::fs::File::open(zip_path).map_err(|e| io(zip_path, e))?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|_| AdminError::Invalid("el archivo no es un .zip válido".into()))?;
    if zip.len() > MAX_IMPORT_ENTRIES {
        return Err(AdminError::Invalid("el .zip tiene demasiados archivos".into()));
    }
    // Carpeta del mundo dentro del .zip: la del level.dat menos profundo.
    let mut prefix: Option<PathBuf> = None;
    for i in 0..zip.len() {
        let f = zip.by_index(i).map_err(|e| AdminError::Invalid(e.to_string()))?;
        let Some(p) = f.enclosed_name() else { continue };
        if p.file_name().is_some_and(|n| n == "level.dat") {
            let parent = p.parent().map(Path::to_path_buf).unwrap_or_default();
            if prefix
                .as_ref()
                .is_none_or(|cur| parent.components().count() < cur.components().count())
            {
                prefix = Some(parent);
            }
        }
    }
    let prefix = prefix
        .ok_or_else(|| AdminError::Invalid("el .zip no contiene un mundo de Minecraft (falta level.dat)".into()))?;
    // Total aproximado para la barra (tamaños declarados en el .zip; el límite real se mide abajo).
    let mut declared: u64 = 0;
    for i in 0..zip.len() {
        if let Ok(f) = zip.by_index(i) {
            if f.enclosed_name().is_some_and(|p| p.starts_with(&prefix)) {
                declared = declared.saturating_add(f.size());
            }
        }
    }
    progress.set_total(declared.min(MAX_IMPORT_BYTES));
    std::fs::create_dir_all(&target).map_err(|e| io(&target, e))?;
    let result = (|| {
        let mut total: u64 = 0;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|e| AdminError::Invalid(e.to_string()))?;
            // enclosed_name descarta rutas absolutas y con «..».
            let Some(p) = f.enclosed_name() else { continue };
            let Ok(rel) = p.strip_prefix(&prefix) else { continue };
            if rel.as_os_str().is_empty() {
                continue;
            }
            let rel_s = rel.to_string_lossy().replace('\\', "/");
            let out = sb.resolve(&format!("{new_name}/{rel_s}"))?;
            if f.is_dir() {
                std::fs::create_dir_all(&out).map_err(|e| io(&out, e))?;
                continue;
            }
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
            }
            let mut w = std::fs::File::create(&out).map_err(|e| io(&out, e))?;
            let mut buf = [0u8; 64 * 1024];
            loop {
                let n = f
                    .read(&mut buf)
                    .map_err(|e| AdminError::Invalid(format!("el .zip está dañado: {e}")))?;
                if n == 0 {
                    break;
                }
                total += n as u64;
                if total > MAX_IMPORT_BYTES {
                    return Err(AdminError::Invalid("el mundo descomprimido es demasiado grande".into()));
                }
                w.write_all(&buf[..n]).map_err(|e| io(&out, e))?;
                progress.advance(n as u64)?;
            }
        }
        Ok::<(), AdminError>(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&target);
        return Err(e);
    }
    if !is_world(&target) {
        let _ = std::fs::remove_dir_all(&target);
        return Err(AdminError::Invalid("el mundo importado no tiene level.dat".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_skips_logs_and_locks_and_cleans_up_on_cancel() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("a");
        std::fs::create_dir_all(src.join("world")).unwrap();
        std::fs::create_dir_all(src.join("logs")).unwrap();
        std::fs::write(src.join("server.properties"), "server-port=25565\n").unwrap();
        std::fs::write(src.join("world/level.dat"), "x").unwrap();
        std::fs::write(src.join("world/session.lock"), "l").unwrap();
        std::fs::write(src.join("logs/latest.log"), "log").unwrap();
        let dst = tmp.path().join("b");
        clone_server(&src, &dst, &Progress::default()).unwrap();
        assert!(dst.join("world/level.dat").is_file());
        assert!(dst.join("server.properties").is_file());
        assert!(!dst.join("logs").exists());
        assert!(!dst.join("world/session.lock").exists());
        assert!(
            clone_server(&src, &dst, &Progress::default()).is_err(),
            "no pisa una carpeta existente"
        );
        let p = Progress::default();
        p.cancel.store(true, Ordering::Relaxed);
        let dst2 = tmp.path().join("c");
        assert!(clone_server(&src, &dst2, &p).is_err());
        assert!(!dst2.exists(), "lo copiado a medias se borra");
    }

    fn server() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for (p, c) in [
            ("world/level.dat", "W"),
            ("world/region/r.0.0.mca", "region"),
            ("world/session.lock", "lock"),
            ("world_nether/level.dat", "N"),
            ("world_nether/DIM-1/r.mca", "nether"),
            ("world_the_end/level.dat", "E"),
            ("creativo/level.dat", "C"),
            ("plugins/x.jar", "j"),
        ] {
            let path = d.path().join(p);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, c).unwrap();
        }
        d
    }

    #[test]
    fn lists_worlds_with_bukkit_dimensions() {
        let d = server();
        let w = list_worlds(d.path(), "world").unwrap();
        assert_eq!(
            w.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(),
            ["creativo", "world"]
        );
        let main = &w[1];
        assert!(main.active);
        assert_eq!(main.companions, ["world_nether", "world_the_end"]);
        assert!(main.size_bytes > 10);
    }

    #[test]
    fn duplicate_copies_dimensions_without_lock() {
        let d = server();
        duplicate(d.path(), "world", "copia", &Progress::default()).unwrap();
        assert!(d.path().join("copia/level.dat").is_file());
        assert!(d.path().join("copia_nether/DIM-1/r.mca").is_file());
        assert!(!d.path().join("copia/session.lock").exists());
        assert!(matches!(
            duplicate(d.path(), "world", "creativo", &Progress::default()),
            Err(AdminError::Conflict(_))
        ));
        assert!(duplicate(d.path(), "world", "../fuera", &Progress::default()).is_err());
        assert!(duplicate(d.path(), "plugins", "x", &Progress::default()).is_err());
    }

    #[test]
    fn export_then_import_roundtrip() {
        let d = server();
        let out = tempfile::tempdir().unwrap();
        let zip_path = out.path().join("mundo.zip");
        assert!(export_zip(d.path(), "world", &zip_path, &Progress::default()).unwrap() > 0);
        import_zip(d.path(), &zip_path, "importado", &Progress::default()).unwrap();
        assert_eq!(
            std::fs::read_to_string(d.path().join("importado/level.dat")).unwrap(),
            "W"
        );
        assert!(d.path().join("importado/region/r.0.0.mca").is_file());
        assert!(matches!(
            import_zip(d.path(), &zip_path, "importado", &Progress::default()),
            Err(AdminError::Conflict(_))
        ));
    }

    #[test]
    fn progress_counts_and_cancel_undoes() {
        let d = server();
        let p = Progress::default();
        duplicate(d.path(), "world", "copia", &p).unwrap();
        let total = p.total.load(Ordering::Relaxed);
        assert!(total > 0);
        assert_eq!(p.done.load(Ordering::Relaxed), total - "lock".len() as u64);

        let p = Progress::default();
        p.cancel.store(true, Ordering::Relaxed);
        assert_eq!(duplicate(d.path(), "world", "otra", &p), Err(AdminError::Cancelled));
        assert!(!d.path().join("otra").exists() && !d.path().join("otra_nether").exists());

        let out = tempfile::tempdir().unwrap();
        let zip_path = out.path().join("m.zip");
        assert_eq!(export_zip(d.path(), "world", &zip_path, &p), Err(AdminError::Cancelled));
        assert!(std::fs::read_dir(out.path()).unwrap().next().is_none(), "sin restos");
        export_zip(d.path(), "world", &zip_path, &Progress::default()).unwrap();
        assert_eq!(import_zip(d.path(), &zip_path, "imp", &p), Err(AdminError::Cancelled));
        assert!(!d.path().join("imp").exists());
    }

    #[test]
    fn import_rejects_non_worlds_and_traversal() {
        let d = server();
        let out = tempfile::tempdir().unwrap();
        let zip_path = out.path().join("malo.zip");
        {
            let mut z = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            z.start_file("../../escapado.txt", o).unwrap();
            z.write_all(b"x").unwrap();
            z.start_file("descarga/mi mundo/level.dat", o).unwrap();
            z.write_all(b"L").unwrap();
            z.start_file("descarga/mi mundo/data/x.dat", o).unwrap();
            z.write_all(b"D").unwrap();
            z.finish().unwrap();
        }
        import_zip(d.path(), &zip_path, "nuevo", &Progress::default()).unwrap();
        assert_eq!(std::fs::read_to_string(d.path().join("nuevo/level.dat")).unwrap(), "L");
        assert!(d.path().join("nuevo/data/x.dat").is_file());
        assert!(!out.path().join("escapado.txt").exists() && !d.path().join("escapado.txt").exists());

        let not_world = out.path().join("nada.zip");
        {
            let mut z = zip::ZipWriter::new(std::fs::File::create(&not_world).unwrap());
            z.start_file("hola.txt", zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(b"x").unwrap();
            z.finish().unwrap();
        }
        assert!(matches!(
            import_zip(d.path(), &not_world, "otro", &Progress::default()),
            Err(AdminError::Invalid(_))
        ));
        assert!(!d.path().join("otro").exists());
    }
}
