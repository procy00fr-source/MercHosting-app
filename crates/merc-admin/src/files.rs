//! Explorador de archivos confinado a la carpeta de un servidor.
//!
//! Todas las rutas pasan por [`SandboxRoot`]: no se puede salir de la carpeta, ni seguir
//! enlaces o junctions. Antes de sobrescribir un archivo se guarda una copia; «borrar»
//! mueve a una papelera fuera de la carpeta del servidor. Nada se elimina de verdad.

use crate::{io, stamp, AdminError};
use merc_core::paths::SandboxRoot;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Tamaño máximo para ver o editar como texto.
pub const MAX_TEXT_BYTES: u64 = 2 * 1024 * 1024;
/// Tamaño máximo de un archivo subido.
pub const MAX_UPLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_LIST: usize = 5000;
/// Límites al extraer un zip (evitan «bombas» de compresión).
pub const MAX_ZIP_ENTRIES: usize = 50_000;
pub const MAX_ZIP_BYTES: u64 = 8 * 1024 * 1024 * 1024;

/// Resultado de extraer un zip.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ZipExtracted {
    /// Carpeta nueva (relativa al servidor).
    pub folder: String,
    pub files: u64,
    pub bytes: u64,
    /// Entradas omitidas por inseguras (rutas fuera, enlaces, nombres no válidos).
    pub skipped: Vec<String>,
}

fn extract_into(zip: &mut zip::ZipArchive<std::fs::File>, dest: &Path) -> Result<(u64, u64, Vec<String>), AdminError> {
    use std::io::{Read, Write};
    std::fs::create_dir_all(dest).map_err(|e| io(dest, e))?;
    let (mut files, mut total) = (0u64, 0u64);
    let mut skipped = vec![];
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|_| AdminError::Invalid("el zip está dañado".into()))?;
        let raw = entry.name().map(|n| n.into_owned()).unwrap_or_default();
        let is_link = entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000);
        let rel = match (is_link, SandboxRoot::normalize_relative(&raw)) {
            (false, Ok(r)) => r,
            (_, Err(_)) if raw.trim_matches('/').is_empty() => continue,
            _ => {
                if skipped.len() < 50 {
                    skipped.push(raw);
                }
                continue;
            }
        };
        let target = dest.join(&rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| io(&target, e))?;
            continue;
        }
        if let Some(p) = target.parent() {
            std::fs::create_dir_all(p).map_err(|e| io(p, e))?;
        }
        if target.exists() {
            // Dos entradas con el mismo nombre (p. ej. mayúsculas en Windows): no pisar.
            if skipped.len() < 50 {
                skipped.push(raw);
            }
            continue;
        }
        let mut out = std::fs::File::create(&target).map_err(|e| io(&target, e))?;
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = entry
                .read(&mut buf)
                .map_err(|_| AdminError::Invalid("el zip está dañado".into()))?;
            if n == 0 {
                break;
            }
            total += n as u64;
            if total > MAX_ZIP_BYTES {
                return Err(AdminError::Invalid(
                    "el contenido del zip es demasiado grande (máximo 8 GB)".into(),
                ));
            }
            out.write_all(&buf[..n]).map_err(|e| io(&target, e))?;
        }
        files += 1;
    }
    Ok((files, total, skipped))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub modified_unix_ms: Option<i64>,
    /// Enlace simbólico o junction: se muestra pero no se puede abrir.
    pub link: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextFile {
    pub path: String,
    pub content: String,
    pub size: u64,
    pub modified_unix_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedKind {
    /// Movido a la papelera.
    Trash,
    /// Copia de un archivo antes de sobrescribirlo.
    Copy,
}

impl SavedKind {
    fn dir(self) -> &'static str {
        match self {
            SavedKind::Trash => "papelera",
            SavedKind::Copy => "copias",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedItem {
    /// `papelera/<marca>` o `copias/<marca>`.
    pub id: String,
    pub kind: SavedKind,
    /// Ruta original dentro de la carpeta del servidor.
    pub path: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub saved_unix_ms: Option<i64>,
}

const MANIFEST: &str = ".merchosting.json";

/// Marca de tiempo que no exista ya en `base` (dos operaciones en el mismo milisegundo).
fn fresh_stamp(base: &Path) -> String {
    let st = stamp();
    if !base.join(&st).exists() {
        return st;
    }
    (2..)
        .map(|n| format!("{st}-{n}"))
        .find(|c| !base.join(c).exists())
        .unwrap_or(st)
}

fn write_manifest(dir: &Path, rel: &str) {
    let _ = std::fs::write(dir.join(MANIFEST), serde_json::json!({ "original": rel }).to_string());
}

fn read_manifest(dir: &Path) -> Option<String> {
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join(MANIFEST)).ok()?).ok()?;
    let rel = v["original"].as_str()?.to_string();
    // La ruta guardada se revalida como cualquier otra.
    SandboxRoot::normalize_relative(&rel).ok()?;
    Some(rel)
}

pub struct Explorer {
    sb: SandboxRoot,
    /// Copias de archivos antes de sobrescribirlos.
    history: PathBuf,
    /// Papelera: lo «borrado» se mueve aquí.
    trash: PathBuf,
}

fn modified_ms(m: &std::fs::Metadata) -> Option<i64> {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
}

/// Copia recursiva sin seguir enlaces (los enlaces se omiten).
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(from)?;
    if meta.file_type().is_symlink() {
        return Ok(());
    }
    if meta.is_dir() {
        std::fs::create_dir_all(to)?;
        for e in std::fs::read_dir(from)? {
            let e = e?;
            copy_tree(&e.path(), &to.join(e.file_name()))?;
        }
    } else {
        if let Some(p) = to.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::copy(from, to)?;
    }
    Ok(())
}

impl Explorer {
    /// `store` es la carpeta de datos de MercHosting para esta instancia (fuera del servidor).
    pub fn new(root: &Path, store: &Path) -> Result<Self, AdminError> {
        Ok(Self {
            sb: SandboxRoot::new(root)?,
            history: store.join("copias"),
            trash: store.join("papelera"),
        })
    }

    pub fn trash_dir(&self) -> &Path {
        &self.trash
    }

    fn path(&self, rel: &str) -> Result<PathBuf, AdminError> {
        if rel.trim().is_empty() || rel.trim() == "." || rel.trim() == "/" {
            return Ok(self.sb.root().to_path_buf());
        }
        Ok(self.sb.resolve(rel)?)
    }

    fn rel_clean(rel: &str) -> Result<String, AdminError> {
        Ok(SandboxRoot::normalize_relative(rel)?
            .to_string_lossy()
            .replace('\\', "/"))
    }

    pub fn list(&self, rel: &str) -> Result<Vec<FileEntry>, AdminError> {
        let dir = self.path(rel)?;
        let rd = std::fs::read_dir(&dir).map_err(|e| io(&dir, e))?;
        let mut out = vec![];
        for e in rd.flatten().take(MAX_LIST) {
            let Ok(meta) = std::fs::symlink_metadata(e.path()) else {
                continue;
            };
            let link = meta.file_type().is_symlink();
            out.push(FileEntry {
                name: e.file_name().to_string_lossy().into_owned(),
                is_dir: meta.is_dir(),
                size: (!meta.is_dir() && !link).then_some(meta.len()),
                modified_unix_ms: modified_ms(&meta),
                link,
            });
        }
        out.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(out)
    }

    pub fn read_text(&self, rel: &str) -> Result<TextFile, AdminError> {
        let p = self.path(rel)?;
        let meta = std::fs::metadata(&p).map_err(|e| io(&p, e))?;
        if meta.is_dir() {
            return Err(AdminError::Invalid("es una carpeta".into()));
        }
        if meta.len() > MAX_TEXT_BYTES {
            return Err(AdminError::Invalid(format!(
                "el archivo es demasiado grande para verlo aquí (máximo {} MB); descárgalo",
                MAX_TEXT_BYTES / 1024 / 1024
            )));
        }
        let bytes = std::fs::read(&p).map_err(|e| io(&p, e))?;
        if bytes.contains(&0) {
            return Err(AdminError::Invalid(
                "no es un archivo de texto; descárgalo para abrirlo".into(),
            ));
        }
        let content = String::from_utf8(bytes)
            .map_err(|_| AdminError::Invalid("el archivo no está en UTF-8; descárgalo para abrirlo".into()))?;
        Ok(TextFile {
            path: Self::rel_clean(rel)?,
            size: meta.len(),
            modified_unix_ms: modified_ms(&meta),
            content,
        })
    }

    /// Guarda un archivo de texto. Si ya existía: comprueba que no cambió desde que se leyó
    /// (`expected_modified`), guarda una copia y escribe con archivo temporal + renombrado.
    pub fn write_text(&self, rel: &str, content: &str, expected_modified: Option<i64>) -> Result<TextFile, AdminError> {
        if content.len() as u64 > MAX_TEXT_BYTES {
            return Err(AdminError::Invalid("el texto es demasiado grande".into()));
        }
        let p = self.path(rel)?;
        if p == self.sb.root() {
            return Err(AdminError::Invalid("indica un archivo".into()));
        }
        let clean = Self::rel_clean(rel)?;
        match std::fs::metadata(&p) {
            Ok(meta) => {
                if meta.is_dir() {
                    return Err(AdminError::Invalid("es una carpeta".into()));
                }
                if expected_modified.is_some() && modified_ms(&meta) != expected_modified {
                    return Err(AdminError::Conflict(
                        "el archivo cambió desde que lo abriste (quizá lo modificó el servidor). Vuelve a abrirlo"
                            .into(),
                    ));
                }
                let st = fresh_stamp(&self.history);
                let backup = self.history.join(&st).join(&clean);
                copy_tree(&p, &backup).map_err(|e| io(&backup, e))?;
                write_manifest(&self.history.join(&st), &clean);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if expected_modified.is_some() {
                    return Err(AdminError::Conflict("el archivo ya no existe".into()));
                }
            }
            Err(e) => return Err(io(&p, e)),
        }
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let tmp = p.with_file_name(format!(".{name}.merchosting-tmp"));
        std::fs::write(&tmp, content.as_bytes()).map_err(|e| io(&tmp, e))?;
        if let Err(e) = std::fs::rename(&tmp, &p) {
            let _ = std::fs::remove_file(&tmp);
            return Err(io(&p, e));
        }
        self.read_text(&clean)
    }

    pub fn mkdir(&self, rel: &str) -> Result<(), AdminError> {
        let p = self.path(rel)?;
        if p.exists() {
            return Err(AdminError::Conflict("ya existe algo con ese nombre".into()));
        }
        std::fs::create_dir_all(&p).map_err(|e| io(&p, e))
    }

    /// Cambia el nombre dentro de la misma carpeta.
    pub fn rename(&self, rel: &str, new_name: &str) -> Result<String, AdminError> {
        if new_name.contains(['/', '\\']) {
            return Err(AdminError::Invalid("el nombre nuevo no puede llevar carpetas".into()));
        }
        let clean = Self::rel_clean(rel)?;
        let from = self.path(&clean)?;
        if !from.exists() {
            return Err(AdminError::NotFound(clean));
        }
        let parent_rel = Path::new(&clean)
            .parent()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        let to_rel = if parent_rel.is_empty() {
            new_name.to_string()
        } else {
            format!("{parent_rel}/{new_name}")
        };
        let to = self.path(&to_rel)?;
        if to.exists() && !from.to_string_lossy().eq_ignore_ascii_case(&to.to_string_lossy()) {
            return Err(AdminError::Conflict("ya existe algo con ese nombre".into()));
        }
        std::fs::rename(&from, &to).map_err(|e| io(&from, e))?;
        Self::rel_clean(&to_rel)
    }

    /// Mueve a la papelera de MercHosting. Devuelve dónde quedó.
    pub fn trash(&self, rel: &str) -> Result<PathBuf, AdminError> {
        let clean = Self::rel_clean(rel)?;
        let from = self.path(&clean)?;
        std::fs::symlink_metadata(&from).map_err(|e| io(&from, e))?;
        let st = fresh_stamp(&self.trash);
        let dest = self.trash.join(&st).join(&clean);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
        }
        if std::fs::rename(&from, &dest).is_err() {
            // Otra unidad: copiar y, solo si la copia salió bien, quitar el original.
            copy_tree(&from, &dest).map_err(|e| io(&dest, e))?;
            let r = if from.is_dir() {
                std::fs::remove_dir_all(&from)
            } else {
                std::fs::remove_file(&from)
            };
            r.map_err(|e| io(&from, e))?;
        }
        write_manifest(&self.trash.join(&st), &clean);
        Ok(dest)
    }

    /// Mueve varios archivos a la papelera juntos, dentro de una carpeta `group` (al restaurar
    /// vuelve esa carpeta entera al servidor, con los archivos en sus rutas originales dentro).
    /// Si uno falla, los ya movidos vuelven a su sitio.
    pub fn trash_group(&self, group: &str, rels: &[String]) -> Result<PathBuf, AdminError> {
        let group = Self::rel_clean(group)?;
        if group.contains('/') {
            return Err(AdminError::Invalid("nombre de grupo no válido".into()));
        }
        let st = fresh_stamp(&self.trash);
        let base = self.trash.join(&st).join(&group);
        let mut moved: Vec<(PathBuf, PathBuf)> = vec![];
        for rel in rels {
            let clean = Self::rel_clean(rel)?;
            let from = self.path(&clean)?;
            if !from.is_file() {
                continue;
            }
            let dest = base.join(&clean);
            let r = (|| {
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                if std::fs::rename(&from, &dest).is_err() {
                    std::fs::copy(&from, &dest)?;
                    std::fs::remove_file(&from)?;
                }
                Ok::<(), std::io::Error>(())
            })();
            if let Err(e) = r {
                for (f, d) in moved.iter().rev() {
                    if std::fs::rename(d, f).is_err() && std::fs::copy(d, f).is_ok() {
                        let _ = std::fs::remove_file(d);
                    }
                }
                let _ = std::fs::remove_dir_all(self.trash.join(&st));
                return Err(io(&from, e));
            }
            moved.push((from, dest));
        }
        if moved.is_empty() {
            let _ = std::fs::remove_dir_all(self.trash.join(&st));
        } else {
            write_manifest(&self.trash.join(&st), &group);
        }
        Ok(base)
    }

    /// Lo que hay en la papelera y en las copias previas, del más reciente al más antiguo.
    pub fn saved_items(&self) -> Vec<SavedItem> {
        let mut out = vec![];
        for (kind, base) in [(SavedKind::Trash, &self.trash), (SavedKind::Copy, &self.history)] {
            for e in std::fs::read_dir(base).into_iter().flatten().flatten() {
                let dir = e.path();
                let Some(stamp) = e.file_name().to_str().map(str::to_string) else {
                    continue;
                };
                let Some(rel) = read_manifest(&dir) else { continue };
                let item = dir.join(&rel);
                let Ok(meta) = std::fs::symlink_metadata(&item) else {
                    continue;
                };
                out.push(SavedItem {
                    id: format!("{}/{stamp}", kind.dir()),
                    kind,
                    path: rel,
                    is_dir: meta.is_dir(),
                    size_bytes: if meta.is_dir() {
                        crate::worlds_size(&item)
                    } else {
                        meta.len()
                    },
                    saved_unix_ms: modified_ms(&std::fs::metadata(dir.join(MANIFEST)).unwrap_or(meta)),
                });
            }
        }
        out.sort_by(|a, b| b.id.rsplit('/').next().cmp(&a.id.rsplit('/').next()));
        out
    }

    fn saved_dir(&self, id: &str) -> Result<(SavedKind, PathBuf, String), AdminError> {
        let (kind_s, st) = id
            .split_once('/')
            .ok_or_else(|| AdminError::Invalid("identificador no válido".into()))?;
        let kind = match kind_s {
            "papelera" => SavedKind::Trash,
            "copias" => SavedKind::Copy,
            _ => return Err(AdminError::Invalid("identificador no válido".into())),
        };
        if st.is_empty() || !st.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
            return Err(AdminError::Invalid("identificador no válido".into()));
        }
        let base = if kind == SavedKind::Trash {
            &self.trash
        } else {
            &self.history
        };
        let dir = base.join(st);
        let rel = read_manifest(&dir).ok_or_else(|| AdminError::NotFound(id.to_string()))?;
        Ok((kind, dir, rel))
    }

    /// Devuelve a su sitio algo de la papelera, o vuelve a poner una copia previa (guardando
    /// antes la versión actual, que a su vez queda en las copias).
    pub fn restore(&self, id: &str) -> Result<String, AdminError> {
        let (kind, dir, rel) = self.saved_dir(id)?;
        let item = dir.join(&rel);
        let target = self.path(&rel)?;
        match kind {
            SavedKind::Trash => {
                if std::fs::symlink_metadata(&target).is_ok() {
                    return Err(AdminError::Conflict(format!(
                        "ya hay algo en «{rel}». Renómbralo o muévelo a la papelera antes de restaurar"
                    )));
                }
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
                }
                if std::fs::rename(&item, &target).is_err() {
                    copy_tree(&item, &target).map_err(|e| io(&target, e))?;
                    let _ = if item.is_dir() {
                        std::fs::remove_dir_all(&item)
                    } else {
                        std::fs::remove_file(&item)
                    };
                }
                let _ = std::fs::remove_dir_all(&dir);
            }
            SavedKind::Copy => {
                if target.is_dir() {
                    return Err(AdminError::Conflict(format!("«{rel}» ahora es una carpeta")));
                }
                if target.exists() {
                    let st = fresh_stamp(&self.history);
                    let backup = self.history.join(&st).join(&rel);
                    copy_tree(&target, &backup).map_err(|e| io(&backup, e))?;
                    write_manifest(&self.history.join(&st), &rel);
                }
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
                }
                let name = target
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let tmp = target.with_file_name(format!(".{name}.merchosting-tmp"));
                std::fs::copy(&item, &tmp).map_err(|e| io(&tmp, e))?;
                std::fs::rename(&tmp, &target).map_err(|e| {
                    let _ = std::fs::remove_file(&tmp);
                    io(&target, e)
                })?;
            }
        }
        Ok(rel)
    }

    /// Borra definitivamente un elemento guardado (el usuario lo confirma en la interfaz).
    pub fn purge(&self, id: &str) -> Result<(), AdminError> {
        let (_, dir, _) = self.saved_dir(id)?;
        std::fs::remove_dir_all(&dir).map_err(|e| io(&dir, e))
    }

    /// Copia un archivo elegido por el usuario (en su PC) dentro de una carpeta del servidor.
    pub fn upload(&self, src: &Path, dest_dir_rel: &str) -> Result<String, AdminError> {
        let meta = std::fs::metadata(src).map_err(|e| io(src, e))?;
        if !meta.is_file() {
            return Err(AdminError::Invalid("solo se pueden subir archivos".into()));
        }
        if meta.len() > MAX_UPLOAD_BYTES {
            return Err(AdminError::Invalid(
                "el archivo es demasiado grande (máximo 2 GB)".into(),
            ));
        }
        let name = src
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or_else(|| AdminError::Invalid("archivo sin nombre".into()))?;
        let dir = self.path(dest_dir_rel)?;
        if !dir.is_dir() {
            return Err(AdminError::NotFound(dest_dir_rel.to_string()));
        }
        let rel = if dest_dir_rel.trim().is_empty() {
            name.clone()
        } else {
            format!("{}/{name}", Self::rel_clean(dest_dir_rel)?)
        };
        let dest = self.path(&rel)?;
        if dest.exists() {
            return Err(AdminError::Conflict(format!("ya existe «{name}» en esa carpeta")));
        }
        let tmp = dest.with_file_name(format!(".{name}.merchosting-tmp"));
        std::fs::copy(src, &tmp).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            io(src, e)
        })?;
        std::fs::rename(&tmp, &dest).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            io(&dest, e)
        })?;
        Self::rel_clean(&rel)
    }

    /// Ruta relativa de `name` dentro de `dir_rel` (vacío = raíz).
    fn child_rel(dir_rel: &str, name: &str) -> Result<String, AdminError> {
        if dir_rel.trim().is_empty() || dir_rel.trim() == "/" {
            Self::rel_clean(name)
        } else {
            Self::rel_clean(&format!("{}/{name}", Self::rel_clean(dir_rel)?))
        }
    }

    fn file_name_of(clean: &str) -> Result<String, AdminError> {
        Path::new(clean)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or_else(|| AdminError::Invalid("ruta sin nombre".into()))
    }

    /// Comprueba origen y carpeta destino para mover o copiar. Devuelve (origen, destino, rel destino).
    fn transfer_paths(&self, rel: &str, dest_dir_rel: &str) -> Result<(PathBuf, PathBuf, String), AdminError> {
        let clean = Self::rel_clean(rel)?;
        let from = self.path(&clean)?;
        let meta = std::fs::symlink_metadata(&from).map_err(|e| io(&from, e))?;
        if meta.file_type().is_symlink() {
            return Err(AdminError::Invalid("los enlaces no se pueden mover ni copiar".into()));
        }
        let dir = self.path(dest_dir_rel)?;
        if !dir.is_dir() {
            return Err(AdminError::NotFound(dest_dir_rel.to_string()));
        }
        let name = Self::file_name_of(&clean)?;
        let to_rel = Self::child_rel(dest_dir_rel, &name)?;
        if to_rel == clean {
            return Err(AdminError::Invalid("ya está en esa carpeta".into()));
        }
        if meta.is_dir() && (to_rel.starts_with(&format!("{clean}/"))) {
            return Err(AdminError::Invalid(
                "no se puede meter una carpeta dentro de sí misma".into(),
            ));
        }
        let to = self.path(&to_rel)?;
        if std::fs::symlink_metadata(&to).is_ok() {
            return Err(AdminError::Conflict(format!("ya existe «{name}» en esa carpeta")));
        }
        Ok((from, to, to_rel))
    }

    /// Mueve un archivo o carpeta a otra carpeta del servidor. Nunca sobrescribe.
    pub fn move_to(&self, rel: &str, dest_dir_rel: &str) -> Result<String, AdminError> {
        let (from, to, to_rel) = self.transfer_paths(rel, dest_dir_rel)?;
        std::fs::rename(&from, &to).map_err(|e| io(&from, e))?;
        Ok(to_rel)
    }

    /// Copia un archivo o carpeta (sin seguir enlaces) a otra carpeta del servidor. Nunca sobrescribe;
    /// si la copia falla a medias, se quita lo copiado.
    pub fn copy_to(&self, rel: &str, dest_dir_rel: &str) -> Result<String, AdminError> {
        let (from, to, to_rel) = self.transfer_paths(rel, dest_dir_rel)?;
        let name = Self::file_name_of(&to_rel)?;
        let tmp = to.with_file_name(format!(".{name}.merchosting-tmp"));
        let clean_tmp = |t: &Path| {
            let _ = if t.is_dir() {
                std::fs::remove_dir_all(t)
            } else {
                std::fs::remove_file(t)
            };
        };
        clean_tmp(&tmp);
        if let Err(e) = copy_tree(&from, &tmp) {
            clean_tmp(&tmp);
            return Err(io(&to, e));
        }
        std::fs::rename(&tmp, &to).map_err(|e| {
            clean_tmp(&tmp);
            io(&to, e)
        })?;
        Ok(to_rel)
    }

    /// Extrae un .zip del servidor en una carpeta NUEVA junto a él (con el nombre del zip).
    /// Seguro: rechaza rutas que salgan de la carpeta, absolutas o con nombres no válidos en
    /// Windows; omite enlaces; limita cantidad y tamaño; extrae a una carpeta temporal y solo
    /// la deja si todo salió bien. Nunca sobrescribe.
    pub fn extract_zip(&self, rel: &str) -> Result<ZipExtracted, AdminError> {
        let clean = Self::rel_clean(rel)?;
        let src = self.path(&clean)?;
        if !src.is_file() || !clean.to_lowercase().ends_with(".zip") {
            return Err(AdminError::Invalid("solo se pueden extraer archivos .zip".into()));
        }
        let name = Self::file_name_of(&clean)?;
        let stem = name[..name.len() - 4].trim().to_string();
        if stem.is_empty() {
            return Err(AdminError::Invalid("el zip no tiene nombre".into()));
        }
        let parent = Path::new(&clean)
            .parent()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        let out_rel = Self::child_rel(&parent, &stem)?;
        let out = self.path(&out_rel)?;
        if std::fs::symlink_metadata(&out).is_ok() {
            return Err(AdminError::Conflict(format!(
                "ya existe una carpeta «{stem}»: renómbrala o muévela antes de extraer"
            )));
        }
        let f = std::fs::File::open(&src).map_err(|e| io(&src, e))?;
        let mut zip =
            zip::ZipArchive::new(f).map_err(|_| AdminError::Invalid("el zip está dañado o no es un zip".into()))?;
        if zip.len() > MAX_ZIP_ENTRIES {
            return Err(AdminError::Invalid(format!(
                "el zip tiene demasiados archivos (más de {MAX_ZIP_ENTRIES})"
            )));
        }
        let tmp = out.with_file_name(format!(".{stem}.merchosting-tmp"));
        let _ = std::fs::remove_dir_all(&tmp);
        let r = extract_into(&mut zip, &tmp);
        match r {
            Ok((files, bytes, skipped)) => {
                std::fs::rename(&tmp, &out).map_err(|e| {
                    let _ = std::fs::remove_dir_all(&tmp);
                    io(&out, e)
                })?;
                Ok(ZipExtracted {
                    folder: out_rel,
                    files,
                    bytes,
                    skipped,
                })
            }
            Err(e) => {
                let _ = std::fs::remove_dir_all(&tmp);
                Err(e)
            }
        }
    }

    /// Copia un archivo del servidor a la ruta que eligió el usuario en el diálogo de guardar.
    pub fn download(&self, rel: &str, dest: &Path) -> Result<u64, AdminError> {
        let p = self.path(rel)?;
        if !p.is_file() {
            return Err(AdminError::Invalid("solo se pueden descargar archivos".into()));
        }
        if !dest.is_absolute() {
            return Err(AdminError::Invalid("destino no válido".into()));
        }
        std::fs::copy(&p, dest).map_err(|e| io(dest, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, Explorer) {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("servidor");
        std::fs::create_dir_all(root.join("world/region")).unwrap();
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::write(root.join("server.properties"), "motd=hola\n").unwrap();
        std::fs::write(root.join("world/level.dat"), [0u8, 1, 2]).unwrap();
        let ex = Explorer::new(&root, &d.path().join("datos/inst")).unwrap();
        (d, ex)
    }

    #[test]
    fn list_dirs_first() {
        let (_d, ex) = setup();
        let l = ex.list("").unwrap();
        assert_eq!(
            l.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["config", "world", "server.properties"]
        );
        assert_eq!(l[2].size, Some(10));
        assert!(ex.list("../").is_err());
    }

    #[test]
    fn edit_with_backup_and_conflict_detection() {
        let (d, ex) = setup();
        let f = ex.read_text("server.properties").unwrap();
        assert_eq!(f.content, "motd=hola\n");
        let f2 = ex
            .write_text("server.properties", "motd=chau\n", f.modified_unix_ms)
            .unwrap();
        assert_eq!(f2.content, "motd=chau\n");
        // La copia previa existe con el contenido anterior.
        let copies: Vec<_> = std::fs::read_dir(d.path().join("datos/inst/copias"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(
            std::fs::read_to_string(copies[0].path().join("server.properties")).unwrap(),
            "motd=hola\n"
        );
        // Con una marca de tiempo vieja se rechaza.
        assert!(matches!(
            ex.write_text("server.properties", "x", Some(1)),
            Err(AdminError::Conflict(_))
        ));
        // Binario: no se abre como texto.
        assert!(ex.read_text("world/level.dat").is_err());
        // Crear nuevo.
        ex.write_text("config/nuevo.toml", "a = 1\n", None).unwrap();
        assert!(ex.write_text("../fuera.txt", "x", None).is_err());
    }

    #[test]
    fn rename_trash_upload_download() {
        let (d, ex) = setup();
        assert_eq!(ex.rename("config", "configuracion").unwrap(), "configuracion");
        assert!(ex.rename("server.properties", "../x").is_err());
        assert!(matches!(
            ex.rename("world", "configuracion"),
            Err(AdminError::Conflict(_))
        ));

        let where_ = ex.trash("world").unwrap();
        assert!(where_.join("level.dat").is_file());
        assert!(!d.path().join("servidor/world").exists());
        assert!(ex.trash("").is_err());

        let src = d.path().join("mod.jar");
        std::fs::write(&src, b"jar").unwrap();
        ex.mkdir("mods").unwrap();
        assert_eq!(ex.upload(&src, "mods").unwrap(), "mods/mod.jar");
        assert!(matches!(ex.upload(&src, "mods"), Err(AdminError::Conflict(_))));
        let out = d.path().join("descarga.properties");
        assert_eq!(ex.download("server.properties", &out).unwrap(), 10);
        assert!(ex.download("mods", &out).is_err());
    }

    #[test]
    fn trash_and_copies_can_be_restored() {
        let (d, ex) = setup();
        ex.write_text("server.properties", "motd=v2\n", None).unwrap();
        ex.trash("world").unwrap();
        let items = ex.saved_items();
        assert_eq!(items.len(), 2);
        let trash = items.iter().find(|i| i.kind == SavedKind::Trash).unwrap();
        assert_eq!((trash.path.as_str(), trash.is_dir), ("world", true));
        let copy = items.iter().find(|i| i.kind == SavedKind::Copy).unwrap();
        assert_eq!(copy.path, "server.properties");

        // Restaurar la copia previa guarda la versión actual y vuelve a la anterior.
        ex.restore(&copy.id).unwrap();
        assert_eq!(
            std::fs::read_to_string(d.path().join("servidor/server.properties")).unwrap(),
            "motd=hola\n"
        );
        assert_eq!(ex.saved_items().iter().filter(|i| i.kind == SavedKind::Copy).count(), 2);

        // La papelera no pisa nada que exista.
        std::fs::create_dir(d.path().join("servidor/world")).unwrap();
        assert!(matches!(ex.restore(&trash.id), Err(AdminError::Conflict(_))));
        std::fs::remove_dir(d.path().join("servidor/world")).unwrap();
        assert_eq!(ex.restore(&trash.id).unwrap(), "world");
        assert!(d.path().join("servidor/world/level.dat").is_file());
        assert!(ex.saved_items().iter().all(|i| i.kind == SavedKind::Copy));

        let c = ex.saved_items()[0].id.clone();
        ex.purge(&c).unwrap();
        assert!(ex.restore("papelera/../../x").is_err());
        assert!(ex.purge("otra/123").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn links_are_listed_but_not_followed() {
        let (d, ex) = setup();
        std::os::unix::fs::symlink("/etc", d.path().join("servidor/fuera")).unwrap();
        let l = ex.list("").unwrap();
        assert!(l.iter().any(|e| e.name == "fuera" && e.link));
        assert!(ex.list("fuera").is_err());
        assert!(ex.read_text("fuera/passwd").is_err());
    }

    #[test]
    fn move_and_copy_never_overwrite() {
        let (d, ex) = setup();
        let root = d.path().join("servidor");
        assert_eq!(
            ex.copy_to("server.properties", "config").unwrap(),
            "config/server.properties"
        );
        assert!(root.join("server.properties").exists());
        assert!(matches!(
            ex.copy_to("server.properties", "config"),
            Err(AdminError::Conflict(_))
        ));
        assert_eq!(ex.copy_to("world", "config").unwrap(), "config/world");
        assert!(root.join("config/world/level.dat").exists());
        assert!(ex.move_to("world", "world/region").is_err());
        assert!(ex.move_to("world", "").is_err());
        assert!(matches!(
            ex.move_to("server.properties", "config"),
            Err(AdminError::Conflict(_))
        ));
        std::fs::create_dir_all(root.join("viejo")).unwrap();
        assert_eq!(ex.move_to("config/world", "viejo").unwrap(), "viejo/world");
        assert!(!root.join("config/world").exists());
        assert!(ex.move_to("../x", "").is_err());
        assert!(ex.copy_to("server.properties", "../fuera").is_err());
    }

    #[test]
    fn extract_zip_is_safe() {
        use std::io::Write;
        let (d, ex) = setup();
        let root = d.path().join("servidor");
        let mut z = zip::ZipWriter::new(std::fs::File::create(root.join("pack.zip")).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        z.add_directory("data/", o).unwrap();
        z.start_file("data/a.txt", o).unwrap();
        z.write_all(b"hola").unwrap();
        z.start_file("../escapa.txt", o).unwrap();
        z.write_all(b"x").unwrap();
        z.start_file("con.txt", o).unwrap();
        z.write_all(b"x").unwrap();
        z.start_file("/abs.txt", o).unwrap();
        z.write_all(b"x").unwrap();
        z.finish().unwrap();

        let r = ex.extract_zip("pack.zip").unwrap();
        assert_eq!(r.folder, "pack");
        assert_eq!(r.files, 1);
        assert_eq!(r.bytes, 4);
        assert_eq!(r.skipped.len(), 3);
        assert_eq!(std::fs::read_to_string(root.join("pack/data/a.txt")).unwrap(), "hola");
        assert!(!d.path().join("escapa.txt").exists());
        assert!(!root.join("escapa.txt").exists());
        // Ya existe la carpeta: no se pisa.
        assert!(matches!(ex.extract_zip("pack.zip"), Err(AdminError::Conflict(_))));
        assert!(ex.extract_zip("server.properties").is_err());
        std::fs::write(root.join("roto.zip"), b"no es zip").unwrap();
        assert!(ex.extract_zip("roto.zip").is_err());
        assert!(!root.join("roto").exists());
        assert!(!root.join(".roto.merchosting-tmp").exists());
    }

    #[test]
    fn trash_group_moves_together_and_restores_as_folder() {
        let (d, ex) = setup();
        let root = d.path().join("servidor");
        std::fs::create_dir_all(root.join("mods")).unwrap();
        std::fs::write(root.join("mods/a.jar"), "a").unwrap();
        ex.trash_group(
            "modpack-anterior-1.0",
            &["mods/a.jar".into(), "config/no-existe.toml".into()],
        )
        .unwrap();
        assert!(!root.join("mods/a.jar").exists());
        let items = ex.saved_items();
        let it = items.iter().find(|i| i.path == "modpack-anterior-1.0").unwrap();
        ex.restore(&it.id).unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("modpack-anterior-1.0/mods/a.jar")).unwrap(),
            "a"
        );
    }
}
