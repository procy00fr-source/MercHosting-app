//! Ficha de un mod o plugin leída de su propio jar: para qué loader es, qué ID tiene, qué
//! necesita y qué trae dentro. Solo lectura: nunca se modifica ningún archivo.

use serde_json::Value;
use std::collections::BTreeSet;
use std::io::{Cursor, Read, Seek};
use std::path::Path;

/// Para qué loaders trae ficha el jar.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Loaders {
    pub fabric: bool,
    pub quilt: bool,
    pub forge: bool,
    pub neoforge: bool,
    pub plugin: bool,
}

impl Loaders {
    pub fn any(&self) -> bool {
        self.fabric || self.quilt || self.forge || self.neoforge || self.plugin
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModMeta {
    /// Nombre del archivo (`sodium-0.6.jar`).
    pub file: String,
    pub enabled: bool,
    pub loaders: Loaders,
    /// IDs que declara el jar (el suyo y los de sus jars internos o `provides`).
    pub ids: BTreeSet<String>,
    /// ID principal (el primero que declara).
    pub main_id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    /// El mod declara que es solo para el cliente (Fabric/Quilt `environment`; Forge/NeoForge
    /// con sus dependencias de Minecraft y del loader marcadas `side="CLIENT"`).
    pub client_only: bool,
    /// Dependencias obligatorias (solo IDs; los rangos de versión no se comprueban aquí).
    pub requires: BTreeSet<String>,
}

const MAX_ENTRY: u64 = 2 * 1024 * 1024;
const MAX_NESTED: u64 = 64 * 1024 * 1024;

fn read_entry<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, name: &str) -> Option<String> {
    let mut e = zip.by_name(name).ok()?;
    if e.size() > MAX_ENTRY {
        return None;
    }
    let mut s = String::new();
    e.read_to_string(&mut s).ok()?;
    Some(s)
}

fn parse_fabric(m: &mut ModMeta, t: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<Value>(t) else {
        return vec![];
    };
    m.loaders.fabric = true;
    if let Some(id) = v.get("id").and_then(Value::as_str) {
        m.ids.insert(id.to_string());
        m.main_id.get_or_insert_with(|| id.to_string());
    }
    if m.name.is_none() {
        m.name = v.get("name").and_then(Value::as_str).map(str::to_string);
    }
    if m.version.is_none() {
        m.version = v.get("version").and_then(Value::as_str).map(str::to_string);
    }
    if v.get("environment").and_then(Value::as_str) == Some("client") {
        m.client_only = true;
    }
    if let Some(p) = v.get("provides").and_then(Value::as_array) {
        m.ids.extend(p.iter().filter_map(Value::as_str).map(str::to_string));
    }
    if let Some(d) = v.get("depends").and_then(Value::as_object) {
        m.requires.extend(d.keys().cloned());
    }
    v.get("jars")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|j| j.get("file").and_then(Value::as_str).map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_quilt(m: &mut ModMeta, t: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<Value>(t) else {
        return vec![];
    };
    m.loaders.quilt = true;
    let ql = v.get("quilt_loader").cloned().unwrap_or(Value::Null);
    if let Some(id) = ql.get("id").and_then(Value::as_str) {
        m.ids.insert(id.to_string());
        m.main_id.get_or_insert_with(|| id.to_string());
    }
    if m.name.is_none() {
        m.name = ql.pointer("/metadata/name").and_then(Value::as_str).map(str::to_string);
    }
    if m.version.is_none() {
        m.version = ql.get("version").and_then(Value::as_str).map(str::to_string);
    }
    if v.pointer("/minecraft/environment").and_then(Value::as_str) == Some("client") {
        m.client_only = true;
    }
    if let Some(p) = ql.get("provides").and_then(Value::as_array) {
        for x in p {
            if let Some(s) = x.as_str().or_else(|| x.get("id").and_then(Value::as_str)) {
                m.ids.insert(s.to_string());
            }
        }
    }
    if let Some(d) = ql.get("depends").and_then(Value::as_array) {
        for x in d {
            let optional = x.get("optional").and_then(Value::as_bool).unwrap_or(false);
            if let Some(s) = x.as_str().or_else(|| x.get("id").and_then(Value::as_str)) {
                if !optional {
                    m.requires.insert(s.to_string());
                }
            }
        }
    }
    ql.get("jars")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|j| j.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

fn parse_mods_toml(m: &mut ModMeta, t: &str, neo: bool) {
    let Ok(doc) = t.parse::<toml::Table>() else {
        return;
    };
    if neo {
        m.loaders.neoforge = true;
    } else {
        m.loaders.forge = true;
    }
    if let Some(mods) = doc.get("mods").and_then(|x| x.as_array()) {
        for md in mods {
            if let Some(id) = md.get("modId").and_then(|x| x.as_str()) {
                m.ids.insert(id.to_string());
                m.main_id.get_or_insert_with(|| id.to_string());
            }
            if m.name.is_none() {
                m.name = md.get("displayName").and_then(|x| x.as_str()).map(str::to_string);
            }
            if m.version.is_none() {
                m.version = md
                    .get("version")
                    .and_then(|x| x.as_str())
                    .filter(|v| !v.contains("${"))
                    .map(str::to_string);
            }
        }
    }
    // Forge/NeoForge no tienen un campo «solo cliente», pero los mods de cliente declaran su
    // dependencia de Minecraft y del loader con side="CLIENT": si todas son así, el mod solo
    // se carga en el juego.
    if let Some(deps) = doc.get("dependencies").and_then(|x| x.as_table()) {
        let base: Vec<&toml::Value> = deps
            .values()
            .filter_map(|l| l.as_array())
            .flatten()
            .filter(|d| {
                d.get("modId")
                    .and_then(|x| x.as_str())
                    .is_some_and(|id| matches!(id, "minecraft" | "forge" | "neoforge"))
            })
            .collect();
        if !base.is_empty()
            && base.iter().all(|d| {
                d.get("side")
                    .and_then(|x| x.as_str())
                    .is_some_and(|s| s.eq_ignore_ascii_case("CLIENT"))
            })
        {
            m.client_only = true;
        }
    }
    if let Some(deps) = doc.get("dependencies").and_then(|x| x.as_table()) {
        for list in deps.values() {
            for d in list.as_array().into_iter().flatten() {
                let Some(id) = d.get("modId").and_then(|x| x.as_str()) else {
                    continue;
                };
                // NeoForge: type = "required"; Forge: mandatory = true.
                let required = match d.get("type").and_then(|x| x.as_str()) {
                    Some(ty) => ty.eq_ignore_ascii_case("required"),
                    None => d.get("mandatory").and_then(|x| x.as_bool()).unwrap_or(false),
                };
                let client_side = d
                    .get("side")
                    .and_then(|x| x.as_str())
                    .is_some_and(|s| s.eq_ignore_ascii_case("CLIENT"));
                if required && !client_side {
                    m.requires.insert(id.to_string());
                }
            }
        }
    }
}

fn parse_plugin_yml(m: &mut ModMeta, t: &str) {
    m.loaders.plugin = true;
    let mut in_depend = false;
    for line in t.lines() {
        if let Some(v) = line.strip_prefix("name:") {
            let n = v.trim().trim_matches(['"', '\'']).to_string();
            m.ids.insert(n.to_ascii_lowercase());
            m.main_id.get_or_insert_with(|| n.to_ascii_lowercase());
            m.name.get_or_insert(n);
            in_depend = false;
        } else if let Some(v) = line.strip_prefix("version:") {
            m.version
                .get_or_insert_with(|| v.trim().trim_matches(['"', '\'']).to_string());
            in_depend = false;
        } else if let Some(v) = line.strip_prefix("depend:") {
            let v = v.trim();
            if v.starts_with('[') {
                for d in v.trim_matches(['[', ']']).split(',') {
                    let d = d.trim().trim_matches(['"', '\'']);
                    if !d.is_empty() {
                        m.requires.insert(d.to_ascii_lowercase());
                    }
                }
                in_depend = false;
            } else {
                in_depend = v.is_empty();
            }
        } else if in_depend && line.trim_start().starts_with("- ") {
            let d = line.trim_start()[2..].trim().trim_matches(['"', '\'']);
            if !d.is_empty() {
                m.requires.insert(d.to_ascii_lowercase());
            }
        } else if !line.starts_with(' ') && !line.starts_with('\t') {
            in_depend = false;
        }
    }
}

/// `mcmod.info` de los mods de Forge antiguo (1.12.2 y anteriores): una lista JSON de mods
/// (`[{...}]` o `{"modList": [...]}`) con `modid`, `name` y `version`. No trae el lado
/// (cliente/servidor) ni dependencias fiables, así que solo aporta identidad.
fn parse_mcmod_info(m: &mut ModMeta, t: &str) {
    let Ok(v) = serde_json::from_str::<Value>(t.trim_start_matches('\u{feff}')) else {
        return;
    };
    let list = match &v {
        Value::Array(a) => a.clone(),
        Value::Object(o) => o.get("modList").and_then(|l| l.as_array()).cloned().unwrap_or_default(),
        _ => vec![],
    };
    let clean = |x: Option<&Value>| -> Option<String> {
        let s = x?.as_str()?.trim();
        (!s.is_empty() && !s.contains("${") && s.len() <= 128).then(|| s.to_string())
    };
    for (i, e) in list.iter().enumerate() {
        let Some(id) = clean(e.get("modid")).map(|s| s.to_ascii_lowercase()) else {
            continue;
        };
        m.loaders.forge = true;
        if i == 0 || m.main_id.is_none() {
            m.main_id.get_or_insert_with(|| id.clone());
            if m.name.is_none() {
                m.name = clean(e.get("name"));
            }
            if m.version.is_none() {
                m.version = clean(e.get("version"));
            }
        }
        m.ids.insert(id);
    }
}

fn read_meta<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, m: &mut ModMeta, depth: u8) {
    let mut nested: Vec<String> = vec![];
    if let Some(t) = read_entry(zip, "fabric.mod.json") {
        nested.extend(parse_fabric(m, &t));
    }
    if let Some(t) = read_entry(zip, "quilt.mod.json") {
        nested.extend(parse_quilt(m, &t));
    }
    if let Some(t) = read_entry(zip, "META-INF/neoforge.mods.toml") {
        parse_mods_toml(m, &t, true);
    }
    if let Some(t) = read_entry(zip, "META-INF/mods.toml") {
        parse_mods_toml(m, &t, false);
    }
    if !m.loaders.forge && !m.loaders.neoforge {
        if let Some(t) = read_entry(zip, "mcmod.info") {
            parse_mcmod_info(m, &t);
        }
    }
    if let Some(t) = read_entry(zip, "plugin.yml").or_else(|| read_entry(zip, "paper-plugin.yml")) {
        parse_plugin_yml(m, &t);
    }
    if depth > 0 {
        return;
    }
    // Jars incluidos (Fabric `jars`, Forge/NeoForge `META-INF/jarjar/`): aportan sus IDs.
    let jarjar: Vec<String> = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().and_then(|e| e.name().ok().map(|n| n.into_owned())))
        .filter(|n| n.starts_with("META-INF/jarjar/") && n.ends_with(".jar"))
        .collect();
    nested.extend(jarjar);
    for name in nested {
        let Ok(mut e) = zip.by_name(&name) else {
            continue;
        };
        if e.size() > MAX_NESTED {
            continue;
        }
        let mut buf = Vec::with_capacity(e.size() as usize);
        if e.read_to_end(&mut buf).is_err() {
            continue;
        }
        drop(e);
        if let Ok(mut inner) = zip::ZipArchive::new(Cursor::new(buf)) {
            let mut sub = ModMeta::default();
            read_meta(&mut inner, &mut sub, depth + 1);
            m.ids.extend(sub.ids);
        }
    }
}

/// Lee la ficha de un jar (o `.jar.disabled`). `None` si no es un zip legible.
pub fn read_jar(path: &Path) -> Option<ModMeta> {
    let file = path.file_name()?.to_string_lossy().into_owned();
    let lower = file.to_ascii_lowercase();
    let enabled = lower.ends_with(".jar");
    if !enabled && !lower.ends_with(".jar.disabled") {
        return None;
    }
    let f = std::fs::File::open(path).ok()?;
    let mut zip = zip::ZipArchive::new(f).ok()?;
    let mut m = ModMeta {
        file,
        enabled,
        ..Default::default()
    };
    read_meta(&mut zip, &mut m, 0);
    // Un mod no depende de sí mismo (Fabric a veces lo declara para la versión).
    let own = m.ids.clone();
    m.requires.retain(|r| !own.contains(r));
    Some(m)
}

/// Fichas de todos los jars de una carpeta del servidor (`mods` o `plugins`), ordenadas.
pub fn scan(root: &Path, folder: &str) -> Vec<ModMeta> {
    let Ok(rd) = std::fs::read_dir(root.join(folder)) else {
        return vec![];
    };
    let mut out: Vec<ModMeta> = rd
        .flatten()
        .filter(|e| e.path().is_file())
        .filter_map(|e| read_jar(&e.path()))
        .collect();
    out.sort_by(|a, b| a.file.cmp(&b.file));
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;

    pub(crate) fn jar_bytes(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (n, t) in files {
            z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(t).unwrap();
        }
        z.finish().unwrap().into_inner()
    }

    pub(crate) fn jar(path: &Path, files: &[(&str, &[u8])]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, jar_bytes(files)).unwrap();
    }

    #[test]
    fn reads_fabric_with_nested_jars_and_environment() {
        let d = tempfile::tempdir().unwrap();
        let inner = jar_bytes(&[(
            "fabric.mod.json",
            br#"{"id":"fabric-lifecycle-events-v1","version":"2.0"}"#,
        )]);
        jar(
            &d.path().join("mods/fabric-api.jar"),
            &[
                (
                    "fabric.mod.json",
                    br#"{"schemaVersion":1,"id":"fabric-api","version":"0.100","depends":{"fabricloader":">=0.15","minecraft":"1.21.1"},"jars":[{"file":"META-INF/jars/lifecycle.jar"}]}"#,
                ),
                ("META-INF/jars/lifecycle.jar", &inner),
            ],
        );
        jar(
            &d.path().join("mods/zoom.jar.disabled"),
            &[(
                "fabric.mod.json",
                br#"{"id":"zoomify","environment":"client","depends":{"fabric-api":"*"}}"#,
            )],
        );
        let l = scan(d.path(), "mods");
        assert_eq!(l.len(), 2);
        let api = &l[0];
        assert!(api.loaders.fabric && api.enabled);
        assert!(api.ids.contains("fabric-lifecycle-events-v1"));
        assert_eq!(
            api.requires,
            BTreeSet::from(["fabricloader".to_string(), "minecraft".to_string()])
        );
        let zoom = &l[1];
        assert!(zoom.client_only && !zoom.enabled);
    }

    #[test]
    fn reads_neoforge_forge_and_plugins() {
        let d = tempfile::tempdir().unwrap();
        let toml = br#"modLoader="javafml"
[[mods]]
modId="mymod"
version="${file.jarVersion}"
displayName="Mi Mod"
[[dependencies.mymod]]
modId="neoforge"
type="required"
[[dependencies.mymod]]
modId="geckolib"
type="required"
[[dependencies.mymod]]
modId="jei"
type="optional"
[[dependencies.mymod]]
modId="oculus"
type="required"
side="CLIENT"
"#;
        jar(
            &d.path().join("mods/mymod.jar"),
            &[("META-INF/neoforge.mods.toml", toml)],
        );
        let m = read_jar(&d.path().join("mods/mymod.jar")).unwrap();
        assert!(m.loaders.neoforge && !m.loaders.forge);
        assert_eq!(m.name.as_deref(), Some("Mi Mod"));
        assert_eq!(m.version, None);
        assert_eq!(
            m.requires,
            BTreeSet::from(["geckolib".to_string(), "neoforge".to_string()])
        );
        let client = br#"[[mods]]
modId="zoomer"
[[dependencies.zoomer]]
modId="forge"
mandatory=true
side="CLIENT"
[[dependencies.zoomer]]
modId="minecraft"
mandatory=true
side="CLIENT"
"#;
        jar(&d.path().join("mods/zoomer.jar"), &[("META-INF/mods.toml", client)]);
        assert!(read_jar(&d.path().join("mods/zoomer.jar")).unwrap().client_only);
        assert!(!m.client_only, "un mod con dependencias normales no es solo de cliente");
        let forge = br#"[[mods]]
modId="old"
[[dependencies.old]]
modId="curios"
mandatory=true
"#;
        // Forge 1.12.2: mcmod.info (lista o modList) da identidad, sin lado ni dependencias.
        jar(
            &d.path().join("mods/srp.jar"),
            &[(
                "mcmod.info",
                br#"[{"modid": "srparasites", "name": "Scape and Run Parasites", "version": "1.9.21", "requiredMods": ["forge"]}]"#,
            )],
        );
        let srp = read_jar(&d.path().join("mods/srp.jar")).unwrap();
        assert!(srp.loaders.forge && !srp.client_only && srp.requires.is_empty());
        assert_eq!(srp.main_id.as_deref(), Some("srparasites"));
        assert_eq!(srp.name.as_deref(), Some("Scape and Run Parasites"));
        assert_eq!(srp.version.as_deref(), Some("1.9.21"));
        jar(
            &d.path().join("mods/v2.jar"),
            &[(
                "mcmod.info",
                b"{\"modListVersion\": 2, \"modList\": [{\"modid\": \"JEI\", \"version\": \"${version}\"}, {\"modid\": \"jei_extra\"}]}",
            )],
        );
        let v2 = read_jar(&d.path().join("mods/v2.jar")).unwrap();
        assert_eq!(v2.main_id.as_deref(), Some("jei"));
        assert_eq!(v2.version, None);
        assert!(v2.ids.contains("jei_extra"));
        jar(&d.path().join("mods/old.jar"), &[("META-INF/mods.toml", forge)]);
        let o = read_jar(&d.path().join("mods/old.jar")).unwrap();
        assert!(o.loaders.forge && o.requires.contains("curios"));
        jar(
            &d.path().join("plugins/p.jar"),
            &[(
                "plugin.yml",
                b"name: EssentialsChat\nversion: 2.20\ndepend: [Essentials]\nsoftdepend: [Vault]\n",
            )],
        );
        let p = read_jar(&d.path().join("plugins/p.jar")).unwrap();
        assert!(p.loaders.plugin && p.requires.contains("essentials") && !p.requires.contains("vault"));
        jar(
            &d.path().join("plugins/q.jar"),
            &[("plugin.yml", b"name: Q\ndepend:\n  - Vault\n  - LuckPerms\nmain: x\n")],
        );
        let q = read_jar(&d.path().join("plugins/q.jar")).unwrap();
        assert_eq!(
            q.requires,
            BTreeSet::from(["luckperms".to_string(), "vault".to_string()])
        );
        assert!(read_jar(&d.path().join("no.txt")).is_none());
    }
}
