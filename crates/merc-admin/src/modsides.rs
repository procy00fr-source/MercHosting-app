//! Separador de mods cliente/servidor: clasifica cada mod de la carpeta `mods` con la
//! evidencia que hay (catálogo y ficha del propio jar) y mueve los que no hacen falta en el
//! servidor a una cuarentena reversible (`mods-cuarentena/`). Nunca borra nada.
//!
//! Por qué importa: los modpacks de cliente y los servidores copiados desde un juego suelen
//! traer mods de solo cliente (minimapas, shaders, menús). En Fabric el servidor los ignora,
//! pero en Forge y NeoForge muchos impiden que el servidor arranque.

use crate::modmeta::ModMeta;
use crate::AdminError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const QUARANTINE_DIR: &str = "mods-cuarentena";
const MANIFEST: &str = "merchosting-cuarentena.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModSide {
    /// Solo se usa en el juego de cada jugador.
    ClientOnly,
    /// Solo en el servidor (los jugadores no lo necesitan).
    ServerOnly,
    /// En los dos.
    Both,
    /// No hay datos suficientes (o se contradicen).
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SideConfidence {
    Alta,
    Media,
    Baja,
}

/// Lo que dice un catálogo sobre el lado del mod (`required`, `optional`, `unsupported`,
/// `unknown`, como Modrinth).
#[derive(Debug, Clone, Default)]
pub struct CatalogSides {
    pub source: Option<String>,
    pub title: Option<String>,
    pub client: Option<String>,
    pub server: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModSideReport {
    pub file: String,
    pub name: String,
    pub enabled: bool,
    pub side: ModSide,
    pub confidence: SideConfidence,
    pub evidence: Vec<String>,
    /// Otros mods activos (no de cliente) que lo necesitan: moverlo los rompería.
    pub needed_by: Vec<String>,
    /// Si conviene marcarlo para la cuarentena (solo cliente, seguro y sin dependientes).
    pub suggested: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuarantinedMod {
    pub file: String,
    pub moved_at_unix: i64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModSidesInfo {
    pub items: Vec<ModSideReport>,
    pub quarantined: Vec<QuarantinedMod>,
    /// Carpeta de la cuarentena, relativa a la del servidor.
    pub folder: String,
    /// Si se pudo consultar el catálogo (sin él, solo cuenta la ficha del jar).
    pub catalog_ok: bool,
}

fn is(v: &Option<String>, what: &str) -> bool {
    v.as_deref() == Some(what)
}

/// Clasifica un mod con lo que dicen el catálogo y su propio jar.
pub fn classify(meta: Option<&ModMeta>, cat: &CatalogSides) -> (ModSide, SideConfidence, Vec<String>) {
    let mut ev = vec![];
    let src = cat.source.clone().unwrap_or_else(|| "El catálogo".into());
    let cat_client_only = is(&cat.server, "unsupported");
    let cat_server_only = is(&cat.client, "unsupported") && !cat_client_only;
    let cat_server_ok = is(&cat.server, "required") || is(&cat.server, "optional");
    let cat_both = cat_server_ok && (is(&cat.client, "required") || is(&cat.client, "optional"));
    let jar_client = meta.is_some_and(|m| m.client_only);

    if cat_client_only {
        ev.push(format!("{src}: el autor indica que no funciona en servidores."));
    } else if cat_server_only {
        ev.push(format!("{src}: el autor indica que los jugadores no lo necesitan."));
    } else if cat_both {
        ev.push(format!("{src}: funciona en el servidor y en el juego."));
    } else if cat_server_ok {
        ev.push(format!("{src}: funciona en el servidor."));
    }
    if jar_client {
        let m = meta.unwrap();
        ev.push(if m.loaders.fabric || m.loaders.quilt {
            "Su ficha (fabric.mod.json / quilt.mod.json) dice environment = client.".into()
        } else {
            "Su ficha (mods.toml) depende de Minecraft y del loader solo con side=\"CLIENT\".".into()
        });
    }

    let (side, conf) = match (cat_client_only, jar_client, cat_server_ok) {
        (true, true, _) => (ModSide::ClientOnly, SideConfidence::Alta),
        (true, false, _) => (ModSide::ClientOnly, SideConfidence::Alta),
        (false, true, false) => (ModSide::ClientOnly, SideConfidence::Alta),
        // El catálogo dice que va en el servidor, pero el jar dice cliente: revisar a mano.
        (false, true, true) => {
            ev.push("Las dos fuentes no coinciden: revísalo antes de moverlo.".into());
            (ModSide::Unknown, SideConfidence::Baja)
        }
        (false, false, _) if cat_server_only => (ModSide::ServerOnly, SideConfidence::Alta),
        (false, false, _) if cat_both => (ModSide::Both, SideConfidence::Alta),
        (false, false, true) => (ModSide::Both, SideConfidence::Media),
        _ => {
            if meta.is_none_or(|m| !m.loaders.any()) {
                ev.push("No se pudo leer su ficha y el catálogo no lo reconoce.".into());
            } else {
                ev.push("Ni su ficha ni el catálogo dicen de qué lado es.".into());
            }
            (ModSide::Unknown, SideConfidence::Baja)
        }
    };
    (side, conf, ev)
}

/// Clasificación de cada mod (sin mirar dependencias).
fn report(entries: Vec<(String, bool, Option<ModMeta>, CatalogSides)>) -> Vec<ModSideReport> {
    entries
        .into_iter()
        .map(|(file, enabled, meta, cat)| {
            let (side, confidence, evidence) = classify(meta.as_ref(), &cat);
            let name = cat
                .title
                .clone()
                .or_else(|| meta.as_ref().and_then(|m| m.name.clone()))
                .unwrap_or_else(|| file.clone());
            ModSideReport {
                file,
                name,
                enabled,
                side,
                confidence,
                evidence,
                needed_by: vec![],
                suggested: false,
            }
        })
        .collect()
}

/// Informe de todos los mods: clasificación, quién depende de quién y cuáles sugerir.
pub fn report_with_deps(entries: Vec<(String, bool, Option<ModMeta>, CatalogSides)>) -> Vec<ModSideReport> {
    let reqs: Vec<(String, bool, BTreeSet<String>)> = entries
        .iter()
        .map(|(_, enabled, meta, _)| {
            (
                String::new(),
                *enabled,
                meta.as_ref().map(|m| m.requires.clone()).unwrap_or_default(),
            )
        })
        .collect();
    let ids: Vec<BTreeSet<String>> = entries
        .iter()
        .map(|(_, _, meta, _)| meta.as_ref().map(|m| m.ids.clone()).unwrap_or_default())
        .collect();
    let mut out = report(entries);
    for i in 0..out.len() {
        if ids[i].is_empty() {
            continue;
        }
        let needed: Vec<String> = (0..out.len())
            .filter(|&j| j != i && reqs[j].1 && out[j].side != ModSide::ClientOnly)
            .filter(|&j| reqs[j].2.iter().any(|r| ids[i].contains(r)))
            .map(|j| out[j].name.clone())
            .collect();
        out[i].needed_by = needed;
    }
    for r in &mut out {
        r.suggested = r.side == ModSide::ClientOnly && r.confidence == SideConfidence::Alta && r.needed_by.is_empty();
    }
    out.sort_by(|a, b| {
        let rank = |s: ModSide| match s {
            ModSide::ClientOnly => 0,
            ModSide::Unknown => 1,
            ModSide::ServerOnly => 2,
            ModSide::Both => 3,
        };
        rank(a.side)
            .cmp(&rank(b.side))
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

// ---------- Cuarentena ----------

fn qdir(root: &Path) -> PathBuf {
    root.join(QUARANTINE_DIR)
}

fn load_manifest(root: &Path) -> Vec<QuarantinedMod> {
    std::fs::read_to_string(qdir(root).join(MANIFEST))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_manifest(root: &Path, list: &[QuarantinedMod]) -> Result<(), AdminError> {
    let p = qdir(root).join(MANIFEST);
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(list).unwrap_or_default())
        .map_err(|e| AdminError::Io(e.to_string()))?;
    std::fs::rename(&tmp, &p).map_err(|e| AdminError::Io(e.to_string()))
}

/// Un nombre de archivo de mod, sin carpetas.
fn plain_jar(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    !name.is_empty()
        && !name.contains(['/', '\\', ':'])
        && name != "."
        && name != ".."
        && (l.ends_with(".jar") || l.ends_with(".jar.disabled"))
}

/// Mods en la cuarentena (los que siguen ahí de verdad).
pub fn quarantined(root: &Path) -> Vec<QuarantinedMod> {
    let dir = qdir(root);
    let mut list = load_manifest(root);
    list.retain(|q| dir.join(&q.file).is_file());
    // Archivos que alguien dejó a mano en la carpeta también se muestran.
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if plain_jar(&n) && !list.iter().any(|q| q.file == n) {
                list.push(QuarantinedMod {
                    file: n,
                    moved_at_unix: 0,
                    reason: String::new(),
                });
            }
        }
    }
    list.sort_by_key(|a| a.file.to_lowercase());
    list
}

fn twin(name: &str) -> String {
    match name.strip_suffix(".disabled") {
        Some(base) => base.to_string(),
        None => format!("{name}.disabled"),
    }
}

/// Mueve mods de `mods/` a la cuarentena. Todo o nada: si alguno no se puede, no se mueve
/// ninguno.
pub fn move_to_quarantine(root: &Path, files: &[(String, String)], now: i64) -> Result<(), AdminError> {
    if files.is_empty() {
        return Err(AdminError::Invalid("no elegiste ningún mod".into()));
    }
    let mods = root.join("mods");
    let dir = qdir(root);
    for (f, _) in files {
        if !plain_jar(f) {
            return Err(AdminError::Invalid(format!("«{f}» no es un mod de la carpeta mods")));
        }
        if !mods.join(f).is_file() {
            return Err(AdminError::NotFound(format!("mods/{f}")));
        }
        if dir.join(f).exists() || dir.join(twin(f)).exists() {
            return Err(AdminError::Conflict(format!(
                "ya hay un «{f}» en la cuarentena: devuélvelo o bórralo tú antes"
            )));
        }
    }
    std::fs::create_dir_all(&dir).map_err(|e| AdminError::Io(e.to_string()))?;
    let mut manifest = load_manifest(root);
    let mut done: Vec<String> = vec![];
    for (f, reason) in files {
        if let Err(e) = std::fs::rename(mods.join(f), dir.join(f)) {
            // Se deshace lo movido para no dejar la mitad.
            for d in &done {
                let _ = std::fs::rename(dir.join(d), mods.join(d));
            }
            return Err(AdminError::Io(format!("{f}: {e}")));
        }
        done.push(f.clone());
        manifest.retain(|q| &q.file != f);
        manifest.push(QuarantinedMod {
            file: f.clone(),
            moved_at_unix: now,
            reason: reason.clone(),
        });
    }
    save_manifest(root, &manifest)
}

/// Devuelve mods de la cuarentena a `mods/`. No pisa nada: si ya hay un archivo con ese
/// nombre (activo o desactivado), no se mueve.
pub fn restore(root: &Path, files: &[String]) -> Result<(), AdminError> {
    if files.is_empty() {
        return Err(AdminError::Invalid("no elegiste ningún mod".into()));
    }
    let mods = root.join("mods");
    let dir = qdir(root);
    for f in files {
        if !plain_jar(f) {
            return Err(AdminError::Invalid(format!("«{f}» no es un mod")));
        }
        if !dir.join(f).is_file() {
            return Err(AdminError::NotFound(format!("{QUARANTINE_DIR}/{f}")));
        }
        if mods.join(f).exists() || mods.join(twin(f)).exists() {
            return Err(AdminError::Conflict(format!("ya hay un «{f}» en mods")));
        }
    }
    std::fs::create_dir_all(&mods).map_err(|e| AdminError::Io(e.to_string()))?;
    let mut manifest = load_manifest(root);
    for f in files {
        std::fs::rename(dir.join(f), mods.join(f)).map_err(|e| AdminError::Io(format!("{f}: {e}")))?;
        manifest.retain(|q| &q.file != f);
    }
    save_manifest(root, &manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modmeta::tests::jar;
    use crate::modmeta::{read_jar, Loaders};

    fn cat(server: &str, client: &str) -> CatalogSides {
        CatalogSides {
            source: Some("Modrinth".into()),
            title: None,
            client: Some(client.into()),
            server: Some(server.into()),
        }
    }

    #[test]
    fn classifies_with_evidence() {
        let none = CatalogSides::default();
        let (s, c, _) = classify(None, &cat("unsupported", "required"));
        assert_eq!((s, c), (ModSide::ClientOnly, SideConfidence::Alta));
        let (s, _, _) = classify(None, &cat("required", "unsupported"));
        assert_eq!(s, ModSide::ServerOnly);
        let (s, _, _) = classify(None, &cat("required", "required"));
        assert_eq!(s, ModSide::Both);
        let fabric_client = ModMeta {
            client_only: true,
            loaders: Loaders {
                fabric: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let (s, c, ev) = classify(Some(&fabric_client), &none);
        assert_eq!((s, c), (ModSide::ClientOnly, SideConfidence::Alta));
        assert!(ev[0].contains("environment = client"));
        // Fuentes que se contradicen: a revisar, nunca se sugiere.
        let (s, c, ev) = classify(Some(&fabric_client), &cat("required", "required"));
        assert_eq!((s, c), (ModSide::Unknown, SideConfidence::Baja));
        assert!(ev.iter().any(|e| e.contains("no coinciden")));
        let (s, _, _) = classify(Some(&ModMeta::default()), &none);
        assert_eq!(s, ModSide::Unknown);
    }

    #[test]
    fn a_needed_client_mod_is_not_suggested() {
        let mut lib = ModMeta {
            client_only: true,
            loaders: Loaders {
                fabric: true,
                ..Default::default()
            },
            ..Default::default()
        };
        lib.ids.insert("clientlib".into());
        let mut user = ModMeta {
            loaders: Loaders {
                fabric: true,
                ..Default::default()
            },
            ..Default::default()
        };
        user.requires.insert("clientlib".into());
        let mut zoom = ModMeta {
            client_only: true,
            loaders: Loaders {
                fabric: true,
                ..Default::default()
            },
            ..Default::default()
        };
        zoom.ids.insert("zoomify".into());
        let r = report_with_deps(vec![
            ("lib.jar".into(), true, Some(lib), CatalogSides::default()),
            ("user.jar".into(), true, Some(user), cat("required", "required")),
            ("zoom.jar".into(), true, Some(zoom), CatalogSides::default()),
        ]);
        let get = |f: &str| r.iter().find(|x| x.file == f).unwrap();
        assert!(get("zoom.jar").suggested);
        assert!(!get("lib.jar").suggested);
        assert_eq!(get("lib.jar").needed_by, vec!["user.jar"]);
        assert_eq!(r[0].side, ModSide::ClientOnly, "los de cliente van primero");
    }

    #[test]
    fn quarantine_is_reversible_and_never_overwrites() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        jar(
            &root.join("mods/zoom.jar"),
            &[("fabric.mod.json", br#"{"id":"z","environment":"client"}"#)],
        );
        jar(&root.join("mods/sodium.jar"), &[("fabric.mod.json", br#"{"id":"s"}"#)]);
        assert!(read_jar(&root.join("mods/zoom.jar")).unwrap().client_only);
        // Nombres raros o que no existen: no se mueve nada.
        assert!(move_to_quarantine(root, &[("../x.jar".into(), String::new())], 1).is_err());
        assert!(move_to_quarantine(
            root,
            &[("zoom.jar".into(), String::new()), ("no.jar".into(), String::new())],
            1
        )
        .is_err());
        assert!(root.join("mods/zoom.jar").is_file(), "todo o nada");
        move_to_quarantine(root, &[("zoom.jar".into(), "solo cliente".into())], 100).unwrap();
        assert!(!root.join("mods/zoom.jar").exists());
        let q = quarantined(root);
        assert_eq!(q.len(), 1);
        assert_eq!(
            (q[0].file.as_str(), q[0].moved_at_unix, q[0].reason.as_str()),
            ("zoom.jar", 100, "solo cliente")
        );
        // Si vuelve a aparecer otro con el mismo nombre en mods, no se pisa.
        std::fs::write(root.join("mods/zoom.jar.disabled"), b"otro").unwrap();
        assert!(restore(root, &["zoom.jar".into()]).is_err());
        std::fs::remove_file(root.join("mods/zoom.jar.disabled")).unwrap();
        restore(root, &["zoom.jar".into()]).unwrap();
        assert!(root.join("mods/zoom.jar").is_file());
        assert!(quarantined(root).is_empty());
        assert!(root.join("mods/sodium.jar").is_file());
    }
}
