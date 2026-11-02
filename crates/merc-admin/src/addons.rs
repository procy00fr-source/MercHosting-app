//! Addons de Minecraft Bedrock (behavior packs y resource packs) para Bedrock Dedicated Server.
//!
//! - Un `.mcpack` es un zip con un pack (su `manifest.json` en la raíz o en una carpeta).
//! - Un `.mcaddon` es un zip con varios packs: carpetas con su `manifest.json` o `.mcpack` dentro.
//! - Cada pack tiene `header.uuid`, `header.version` `[a, b, c]` y `modules[].type`
//!   (`data` = behavior pack, `resources` = resource pack; `script` va con `data`).
//! - Se copian a `behavior_packs/<carpeta>/` o `resource_packs/<carpeta>/` y se activan en el
//!   mundo con `worlds/<level-name>/world_behavior_packs.json` / `world_resource_packs.json`
//!   (`[{"pack_id": uuid, "version": [a, b, c]}]`).
//!
//! Nada se borra: quitar un pack lo manda a la papelera de MercHosting (ver [`crate::files`]).

use crate::AdminError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Límites al leer un addon (los reales rondan unos pocos MB).
pub const MAX_ADDON_BYTES: u64 = 1024 * 1024 * 1024;
pub const MAX_ADDON_ENTRIES: usize = 20_000;
/// Un .mcpack metido dentro de un .mcaddon se lee a memoria: como mucho este tamaño.
const MAX_NESTED_PACK: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackKind {
    /// Comportamiento (`behavior_packs/`): entidades, objetos, recetas, scripts.
    Behavior,
    /// Recursos (`resource_packs/`): texturas, modelos, sonidos.
    Resource,
}

impl PackKind {
    pub fn folder(self) -> &'static str {
        match self {
            PackKind::Behavior => "behavior_packs",
            PackKind::Resource => "resource_packs",
        }
    }
    fn world_file(self) -> &'static str {
        match self {
            PackKind::Behavior => "world_behavior_packs.json",
            PackKind::Resource => "world_resource_packs.json",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub uuid: String,
    pub version: [u32; 3],
    pub name: String,
    pub description: String,
    pub kind: PackKind,
    /// Tiene scripts (puede necesitar experimentos o APIs beta activadas en el mundo).
    pub scripts: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddonPack {
    pub uuid: String,
    pub version: [u32; 3],
    pub name: String,
    pub description: String,
    pub kind: PackKind,
    /// Carpeta relativa a la raíz del servidor (`behavior_packs/Dragones_1a2b3c4d`).
    pub folder: String,
    /// Está en la lista de packs del mundo.
    pub active: bool,
    /// Parece venir con el propio servidor de Mojang (carpetas `vanilla*`, `chemistry*`…).
    pub bundled: bool,
    pub scripts: bool,
}

/// Quita los códigos de color `§x` de los nombres de los packs.
fn clean_text(s: &str, max: usize) -> String {
    let mut out = String::new();
    let mut skip = false;
    for c in s.chars() {
        if skip {
            skip = false;
        } else if c == '§' {
            skip = true;
        } else if !c.is_control() {
            out.push(c);
        }
    }
    out.trim().chars().take(max).collect()
}

fn valid_uuid(s: &str) -> bool {
    s.len() == 36
        && s.chars().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == '-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
}

fn version_of(v: &Value) -> Option<[u32; 3]> {
    if let Some(a) = v.as_array() {
        if a.len() == 3 {
            let n: Vec<u32> = a.iter().filter_map(|x| x.as_u64().map(|n| n as u32)).collect();
            if n.len() == 3 {
                return Some([n[0], n[1], n[2]]);
            }
        }
        return None;
    }
    // Manifiestos nuevos (format_version 3) permiten "1.2.3".
    let s = v.as_str()?;
    let parts: Vec<u32> = s.split(['.', '-']).take(3).filter_map(|p| p.parse().ok()).collect();
    (parts.len() == 3).then(|| [parts[0], parts[1], parts[2]])
}

/// Quita comentarios `//` de línea fuera de cadenas (algunos manifiestos los traen).
fn strip_line_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let mut in_str = false;
        let mut esc = false;
        let mut cut = line.len();
        let b = line.as_bytes();
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            if in_str {
                if esc {
                    esc = false;
                } else if c == b'\\' {
                    esc = true;
                } else if c == b'"' {
                    in_str = false;
                }
            } else if c == b'"' {
                in_str = true;
            } else if c == b'/' && b.get(i + 1) == Some(&b'/') {
                cut = i;
                break;
            }
            i += 1;
        }
        out.push_str(&line[..cut]);
        out.push('\n');
    }
    out
}

/// Interpreta un `manifest.json`. Rechaza los que no son behavior ni resource packs
/// (plantillas de mundo, skins…).
pub fn parse_manifest(text: &str) -> Result<Manifest, AdminError> {
    let text = text.trim_start_matches('\u{feff}');
    let v: Value = serde_json::from_str(text)
        .or_else(|_| serde_json::from_str(&strip_line_comments(text)))
        .map_err(|e| AdminError::Invalid(format!("manifest.json no es válido: {e}")))?;
    let h = &v["header"];
    let uuid = h["uuid"].as_str().unwrap_or_default().to_ascii_lowercase();
    if !valid_uuid(&uuid) {
        return Err(AdminError::Invalid(
            "el pack no tiene un identificador (uuid) válido".into(),
        ));
    }
    let version = version_of(&h["version"]).ok_or_else(|| AdminError::Invalid("el pack no tiene versión".into()))?;
    let types: Vec<&str> = v["modules"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m["type"].as_str())
        .collect();
    let kind = if types.contains(&"resources") {
        PackKind::Resource
    } else if types
        .iter()
        .any(|t| *t == "data" || *t == "script" || *t == "client_data")
    {
        PackKind::Behavior
    } else {
        let what = types.first().copied().unwrap_or("desconocido");
        return Err(AdminError::Invalid(format!(
            "no es un behavior pack ni un resource pack (tipo «{what}»)"
        )));
    };
    let name = clean_text(h["name"].as_str().unwrap_or_default(), 80);
    Ok(Manifest {
        uuid,
        version,
        name: if name.is_empty() {
            "Pack sin nombre".into()
        } else {
            name
        },
        description: clean_text(h["description"].as_str().unwrap_or_default(), 300),
        kind,
        scripts: types.contains(&"script"),
    })
}

/// `level-name` de server.properties (por defecto, el de Bedrock: «Bedrock level»).
pub fn level_name(root: &Path) -> String {
    let text = std::fs::read_to_string(root.join("server.properties")).unwrap_or_default();
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .find_map(|l| {
            l.split_once('=')
                .filter(|(k, _)| k.trim() == "level-name")
                .map(|(_, v)| v.trim().to_string())
        })
        .filter(|v| !v.is_empty() && !v.contains(['/', '\\']) && v != "." && v != "..")
        .unwrap_or_else(|| "Bedrock level".into())
}

fn world_dir(root: &Path) -> PathBuf {
    root.join("worlds").join(level_name(root))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct WorldEntry {
    pack_id: String,
    version: [u32; 3],
}

fn read_world_list(path: &Path) -> Vec<WorldEntry> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Vec<Value>>(&t).ok())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|e| {
            Some(WorldEntry {
                pack_id: e["pack_id"].as_str()?.to_ascii_lowercase(),
                version: version_of(&e["version"])?,
            })
        })
        .collect()
}

fn write_world_list(path: &Path, list: &[WorldEntry]) -> Result<(), AdminError> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| AdminError::Io(e.to_string()))?;
    }
    let text = serde_json::to_string_pretty(list).unwrap_or_else(|_| "[]".into());
    let tmp = path.with_extension("json.merchosting-tmp");
    std::fs::write(&tmp, text).map_err(|e| AdminError::Io(e.to_string()))?;
    std::fs::rename(&tmp, path).map_err(|e| AdminError::Io(e.to_string()))
}

fn bundled_name(folder: &str) -> bool {
    let f = folder.to_ascii_lowercase();
    [
        "vanilla",
        "chemistry",
        "experimental",
        "editor",
        "server_editor",
        "server_library",
        "server_ui_library",
    ]
    .iter()
    .any(|p| f.starts_with(p))
}

/// Packs instalados en el servidor y si están activos en el mundo.
pub fn list(root: &Path) -> Vec<AddonPack> {
    let world = world_dir(root);
    let mut out = vec![];
    for kind in [PackKind::Behavior, PackKind::Resource] {
        let active: Vec<String> = read_world_list(&world.join(kind.world_file()))
            .into_iter()
            .map(|e| e.pack_id)
            .collect();
        let Ok(rd) = std::fs::read_dir(root.join(kind.folder())) else {
            continue;
        };
        let mut dirs: Vec<_> = rd.flatten().filter(|e| e.path().is_dir()).collect();
        dirs.sort_by_key(|e| e.file_name());
        for d in dirs {
            let folder = d.file_name().to_string_lossy().into_owned();
            let Ok(text) = std::fs::read_to_string(d.path().join("manifest.json")) else {
                continue;
            };
            let Ok(m) = parse_manifest(&text) else {
                continue;
            };
            out.push(AddonPack {
                active: active.contains(&m.uuid),
                uuid: m.uuid,
                version: m.version,
                name: m.name,
                description: m.description,
                kind,
                folder: format!("{}/{folder}", kind.folder()),
                bundled: bundled_name(&folder),
                scripts: m.scripts,
            });
        }
    }
    out
}

/// Activa o desactiva un pack en el mundo (lo añade o lo quita de su lista).
pub fn set_active(root: &Path, uuid: &str, on: bool) -> Result<(), AdminError> {
    let uuid = uuid.to_ascii_lowercase();
    let pack = list(root)
        .into_iter()
        .find(|p| p.uuid == uuid)
        .ok_or_else(|| AdminError::NotFound(format!("pack {uuid}")))?;
    let path = world_dir(root).join(pack.kind.world_file());
    let mut l = read_world_list(&path);
    l.retain(|e| e.pack_id != uuid);
    if on {
        l.push(WorldEntry {
            pack_id: uuid,
            version: pack.version,
        });
    }
    write_world_list(&path, &l)
}

/// Un pack encontrado dentro del archivo: su manifiesto y sus archivos (ruta dentro del pack).
struct FoundPack {
    manifest: Manifest,
    files: Vec<(String, Vec<u8>)>,
}

fn read_entries(
    bytes: &[u8],
    budget: &mut u64,
    depth: u8,
    out: &mut Vec<FoundPack>,
    notes: &mut Vec<String>,
) -> Result<(), AdminError> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|_| AdminError::Invalid("el archivo está dañado o no es un addon (.mcpack/.mcaddon)".into()))?;
    if z.len() > MAX_ADDON_ENTRIES {
        return Err(AdminError::Invalid("el addon tiene demasiados archivos".into()));
    }
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut nested: Vec<(String, Vec<u8>)> = vec![];
    for i in 0..z.len() {
        let mut e = z
            .by_index(i)
            .map_err(|_| AdminError::Invalid("el addon está dañado".into()))?;
        if e.is_dir() {
            continue;
        }
        if e.is_symlink() {
            return Err(AdminError::Invalid("el addon trae enlaces: no se instala".into()));
        }
        let Some(rel) = e.enclosed_name() else {
            return Err(AdminError::Invalid(
                "el addon trae rutas no permitidas: no se instala".into(),
            ));
        };
        let rel = rel.to_string_lossy().replace('\\', "/");
        let size = e.size();
        if size > *budget {
            return Err(AdminError::Invalid("el addon es demasiado grande".into()));
        }
        let lower = rel.to_ascii_lowercase();
        let pack_inside = depth == 0 && (lower.ends_with(".mcpack") || lower.ends_with(".zip"));
        if pack_inside && size > MAX_NESTED_PACK {
            return Err(AdminError::Invalid(
                "un pack dentro del addon es demasiado grande".into(),
            ));
        }
        let mut buf = Vec::with_capacity(size.min(64 * 1024 * 1024) as usize);
        (&mut e)
            .take(size + 1)
            .read_to_end(&mut buf)
            .map_err(|_| AdminError::Invalid("el addon está dañado".into()))?;
        if buf.len() as u64 > size {
            return Err(AdminError::Invalid("el addon está dañado (tamaños falsos)".into()));
        }
        *budget -= buf.len() as u64;
        if pack_inside {
            nested.push((rel, buf));
        } else {
            files.insert(rel, buf);
        }
    }
    // Cada manifest.json marca la raíz de un pack.
    let roots: Vec<String> = files
        .keys()
        .filter(|k| *k == "manifest.json" || k.ends_with("/manifest.json"))
        .map(|k| k.trim_end_matches("manifest.json").to_string())
        .collect();
    for prefix in &roots {
        // Un manifest.json dentro de otro pack (p. ej. en subpacks) no es un pack aparte.
        if roots
            .iter()
            .any(|r| r != prefix && !r.is_empty() && prefix.starts_with(r.as_str()))
            || (!prefix.is_empty() && roots.iter().any(|r| r.is_empty()))
        {
            continue;
        }
        let text = String::from_utf8_lossy(&files[&format!("{prefix}manifest.json")]).into_owned();
        match parse_manifest(&text) {
            Ok(manifest) => out.push(FoundPack {
                manifest,
                files: files
                    .iter()
                    .filter(|(k, _)| k.starts_with(prefix.as_str()))
                    .map(|(k, v)| (k[prefix.len()..].to_string(), v.clone()))
                    .collect(),
            }),
            Err(e) => notes.push(format!(
                "Se ignoró «{}»: {e}",
                if prefix.is_empty() {
                    "el pack"
                } else {
                    prefix.trim_end_matches('/')
                }
            )),
        }
    }
    for (name, data) in nested {
        read_entries(&data, budget, depth + 1, out, notes).map_err(|e| AdminError::Invalid(format!("{name}: {e}")))?;
    }
    Ok(())
}

/// Nombre de carpeta seguro para el pack: su nombre + el inicio del uuid.
fn folder_name(m: &Manifest) -> String {
    let base: String = m
        .name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .chars()
        .take(40)
        .collect();
    let base = if base.is_empty() { "pack".to_string() } else { base };
    format!("{base}_{}", &m.uuid[..8])
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportResult {
    /// Packs instalados (y activados en el mundo).
    pub installed: Vec<String>,
    /// Packs que ya estaban con la misma versión.
    pub unchanged: Vec<String>,
    /// Packs con otra versión que se reemplazaron (la anterior va a la papelera).
    pub replaced: Vec<String>,
    pub notes: Vec<String>,
}

/// Instala los packs de un `.mcpack`/`.mcaddon` y los activa en el mundo. `trash` recibe la
/// carpeta relativa de una versión anterior para mandarla a la papelera antes de reemplazarla.
pub fn import(
    root: &Path,
    source: &Path,
    trash: &mut dyn FnMut(&str) -> Result<(), AdminError>,
) -> Result<ImportResult, AdminError> {
    let lower = source.to_string_lossy().to_ascii_lowercase();
    if !(lower.ends_with(".mcpack") || lower.ends_with(".mcaddon") || lower.ends_with(".zip")) {
        return Err(AdminError::Invalid("elige un archivo .mcpack o .mcaddon".into()));
    }
    let meta = std::fs::metadata(source).map_err(|e| AdminError::Io(e.to_string()))?;
    if meta.len() > MAX_ADDON_BYTES {
        return Err(AdminError::Invalid("el addon es demasiado grande".into()));
    }
    let bytes = std::fs::read(source).map_err(|e| AdminError::Io(e.to_string()))?;
    let mut found = vec![];
    let mut res = ImportResult {
        installed: vec![],
        unchanged: vec![],
        replaced: vec![],
        notes: vec![],
    };
    let mut budget = MAX_ADDON_BYTES;
    read_entries(&bytes, &mut budget, 0, &mut found, &mut res.notes)?;
    if found.is_empty() {
        return Err(AdminError::Invalid(
            "no se encontró ningún behavior pack ni resource pack dentro".into(),
        ));
    }
    let existing = list(root);
    for p in found {
        let m = &p.manifest;
        let label = format!("{} {}", m.name, m.version.map(|n| n.to_string()).join("."));
        if let Some(old) = existing.iter().find(|e| e.uuid == m.uuid) {
            if old.version == m.version {
                res.unchanged.push(label);
                set_active(root, &m.uuid, true)?;
                continue;
            }
            trash(&old.folder)?;
            res.replaced.push(label.clone());
        }
        let rel = format!("{}/{}", m.kind.folder(), folder_name(m));
        let dest = root.join(&rel);
        if std::fs::symlink_metadata(&dest).is_ok() {
            trash(&rel)?;
        }
        let tmp = root
            .join(m.kind.folder())
            .join(format!(".{}.merchosting-tmp", folder_name(m)));
        let _ = std::fs::remove_dir_all(&tmp);
        let r = (|| {
            for (name, data) in &p.files {
                let out = tmp.join(name);
                if let Some(parent) = out.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&out, data)?;
            }
            std::fs::rename(&tmp, &dest)
        })();
        if let Err(e) = r {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(AdminError::Io(e.to_string()));
        }
        set_active(root, &m.uuid, true)?;
        if m.scripts {
            res.notes.push(format!(
                "«{}» usa scripts: si no funciona, puede necesitar experimentos o APIs beta activados en el mundo.",
                m.name
            ));
        }
        if !res.replaced.contains(&label) {
            res.installed.push(label);
        }
    }
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn manifest(uuid: &str, ver: [u32; 3], name: &str, ty: &str) -> String {
        serde_json::json!({
            "format_version": 2,
            "header": {"name": name, "description": "§aDescripción", "uuid": uuid, "version": ver, "min_engine_version": [1, 21, 0]},
            "modules": [{"type": ty, "uuid": "00000000-0000-0000-0000-000000000000", "version": [1, 0, 0]}]
        })
        .to_string()
    }

    fn zip_bytes(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            for (n, b) in entries {
                z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
                z.write_all(b).unwrap();
            }
            z.finish().unwrap();
        }
        buf.into_inner()
    }

    const BP: &str = "11111111-2222-3333-4444-555555555555";
    const RP: &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";

    #[test]
    fn parses_manifests() {
        let m = parse_manifest(&manifest(BP, [1, 2, 3], "§6Dragones", "data")).unwrap();
        assert_eq!(m.kind, PackKind::Behavior);
        assert_eq!(m.name, "Dragones");
        assert_eq!(m.description, "Descripción");
        assert_eq!(m.version, [1, 2, 3]);
        assert_eq!(
            parse_manifest(&manifest(RP, [1, 0, 0], "Tex", "resources"))
                .unwrap()
                .kind,
            PackKind::Resource
        );
        assert!(parse_manifest(&manifest(BP, [1, 0, 0], "Skin", "skin_pack")).is_err());
        assert!(parse_manifest(&manifest("no-uuid", [1, 0, 0], "x", "data")).is_err());
        let with_comments = "{\n// comentario\n\"header\": {\"uuid\": \"11111111-2222-3333-4444-555555555555\", \"version\": \"2.0.1\", \"name\": \"a // b\"},\n\"modules\": [{\"type\": \"script\"}]}";
        let m = parse_manifest(with_comments).unwrap();
        assert_eq!((m.version, m.name.as_str(), m.scripts), ([2, 0, 1], "a // b", true));
    }

    #[test]
    fn import_mcaddon_activates_and_replaces() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        std::fs::write(root.join("server.properties"), "level-name=Mi mundo\n").unwrap();
        // .mcaddon: una carpeta con un behavior pack y un .mcpack con el resource pack.
        let rp = zip_bytes(&[
            (
                "manifest.json",
                manifest(RP, [1, 0, 0], "Texturas", "resources").into_bytes(),
            ),
            ("textures/a.png", b"png".to_vec()),
        ]);
        let addon = zip_bytes(&[
            (
                "Dragones BP/manifest.json",
                manifest(BP, [1, 0, 0], "Dragones", "data").into_bytes(),
            ),
            ("Dragones BP/entities/dragon.json", b"{}".to_vec()),
            ("Dragones RP.mcpack", rp),
        ]);
        let src = root.join("dragones.mcaddon");
        std::fs::write(&src, &addon).unwrap();
        let mut trashed: Vec<String> = vec![];
        let r = import(root, &src, &mut |rel| {
            trashed.push(rel.to_string());
            Ok(())
        })
        .unwrap();
        assert_eq!(r.installed.len(), 2, "{r:?}");
        let l = list(root);
        assert_eq!(l.len(), 2);
        assert!(l.iter().all(|p| p.active && !p.bundled));
        assert!(root
            .join("behavior_packs/Dragones_11111111/entities/dragon.json")
            .is_file());
        assert!(root.join("resource_packs/Texturas_aaaaaaaa/textures/a.png").is_file());
        let world = std::fs::read_to_string(root.join("worlds/Mi mundo/world_behavior_packs.json")).unwrap();
        assert!(world.contains(BP));

        // Misma versión: no cambia nada. Nueva versión: la anterior va a la papelera.
        let r = import(root, &src, &mut |_| Ok(())).unwrap();
        assert_eq!(r.unchanged.len(), 2);
        let v2 = zip_bytes(&[(
            "manifest.json",
            manifest(BP, [1, 1, 0], "Dragones", "data").into_bytes(),
        )]);
        let src2 = root.join("dragones2.mcpack");
        std::fs::write(&src2, &v2).unwrap();
        let mut moved = vec![];
        let r = import(root, &src2, &mut |rel| {
            moved.push(rel.to_string());
            std::fs::remove_dir_all(root.join(rel)).unwrap();
            Ok(())
        })
        .unwrap();
        assert_eq!(r.replaced, vec!["Dragones 1.1.0".to_string()]);
        assert_eq!(moved, vec!["behavior_packs/Dragones_11111111".to_string()]);
        let p = list(root).into_iter().find(|p| p.uuid == BP).unwrap();
        assert_eq!(p.version, [1, 1, 0]);
        let world = std::fs::read_to_string(root.join("worlds/Mi mundo/world_behavior_packs.json")).unwrap();
        assert!(
            world.contains("1,\n      1,") || world.contains("[\n      1,\n      1,"),
            "{world}"
        );

        // Desactivar y volver a activar.
        set_active(root, RP, false).unwrap();
        assert!(!list(root).iter().find(|p| p.uuid == RP).unwrap().active);
        set_active(root, RP, true).unwrap();
        assert!(list(root).iter().find(|p| p.uuid == RP).unwrap().active);
        assert!(trashed.is_empty());
    }

    #[test]
    fn rejects_bad_archives() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        let src = root.join("x.mcpack");
        std::fs::write(
            &src,
            zip_bytes(&[(
                "../fuera/manifest.json",
                manifest(BP, [1, 0, 0], "x", "data").into_bytes(),
            )]),
        )
        .unwrap();
        assert!(import(root, &src, &mut |_| Ok(())).is_err());
        assert!(!d.path().parent().unwrap().join("fuera").exists());
        std::fs::write(&src, zip_bytes(&[("leeme.txt", b"hola".to_vec())])).unwrap();
        assert!(import(root, &src, &mut |_| Ok(())).is_err());
        std::fs::write(&src, b"no es un zip").unwrap();
        assert!(import(root, &src, &mut |_| Ok(())).is_err());
        assert!(import(root, &root.join("x.jar"), &mut |_| Ok(())).is_err());
    }

    #[test]
    fn bundled_and_level_name() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(level_name(d.path()), "Bedrock level");
        std::fs::write(d.path().join("server.properties"), "level-name=../malo\n").unwrap();
        assert_eq!(level_name(d.path()), "Bedrock level");
        std::fs::create_dir_all(d.path().join("behavior_packs/vanilla_1.21")).unwrap();
        std::fs::write(
            d.path().join("behavior_packs/vanilla_1.21/manifest.json"),
            manifest(BP, [1, 21, 0], "Vanilla", "data"),
        )
        .unwrap();
        assert!(list(d.path())[0].bundled);
    }
}
