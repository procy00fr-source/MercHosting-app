//! Diagnóstico de un servidor: busca problemas conocidos en los mods y plugins instalados y en
//! lo que escribió el servidor (consola, `logs/latest.log`, `crash-reports/`).
//!
//! Reglas: cada hallazgo lleva **evidencias** (la línea del registro o el archivo que lo
//! demuestra) y un nivel de confianza; nada se afirma sin evidencia. Los arreglos que se
//! proponen son reversibles (desactivar un mod, cambiar el puerto o la memoria, elegir otro
//! Java) y la persona los confirma antes de aplicarlos.

use crate::modmeta::ModMeta;
use merc_core::detect::ServerKind;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Alta,
    Media,
    Baja,
}

/// Arreglo propuesto (lo aplica la interfaz con las peticiones normales, tras confirmar).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FixAction {
    /// Desactivar (renombrar a `.jar.disabled`, reversible) un mod o plugin.
    DisableContent { kind: String, file: String },
    /// Usar otro puerto en `server.properties`.
    SetPort { port: u16 },
    /// Subir la memoria máxima del servidor.
    SetMemory { max_mb: u32 },
    /// Elegir un Java de esta versión o más nueva.
    ChooseJava { min_major: u32 },
    /// Buscar un mod o plugin en el catálogo.
    SearchContent { kind: String, query: String },
    /// Abrir otra sección (por ejemplo, copias de seguridad).
    OpenTab { tab: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub code: String,
    pub title: String,
    pub confidence: Confidence,
    pub explanation: String,
    /// Líneas o archivos que lo demuestran (recortados).
    pub evidence: Vec<String>,
    /// De dónde sale: `mods/`, `consola`, `crash-reports/…`.
    pub source: String,
    pub actions: Vec<FixAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnosis {
    pub checked_at_unix_ms: i64,
    /// Lo que se revisó (para que la persona sepa de dónde salen las conclusiones).
    pub sources: Vec<String>,
    pub mods_scanned: u32,
    pub plugins_scanned: u32,
    pub findings: Vec<Finding>,
}

pub struct LogSource {
    pub name: String,
    pub text: String,
}

pub struct Inputs<'a> {
    pub kind: ServerKind,
    pub mc_version: Option<&'a str>,
    /// Versión real del Java configurado (si se pudo comprobar).
    pub java_major: Option<u32>,
    pub max_memory_mb: u32,
    /// Memoria total de la PC (para no proponer más de la que hay).
    pub pc_memory_mb: Option<u64>,
    /// Puerto libre que se puede proponer si el actual está ocupado.
    pub free_port: Option<u16>,
    pub mods: &'a [ModMeta],
    pub plugins: &'a [ModMeta],
    pub logs: &'a [LogSource],
}

fn cut(s: &str) -> String {
    let t = s.trim();
    if t.chars().count() > 300 {
        format!("{}…", t.chars().take(300).collect::<String>())
    } else {
        t.to_string()
    }
}

fn label(m: &ModMeta) -> String {
    match (&m.name, &m.main_id) {
        (Some(n), _) => format!("{n} ({})", m.file),
        (None, Some(id)) => format!("{id} ({})", m.file),
        _ => m.file.clone(),
    }
}

/// IDs que el loader o el juego ya aportan (no son mods que falten).
fn builtin(kind: ServerKind, id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    matches!(
        id.as_str(),
        "minecraft" | "java" | "fabricloader" | "quilt_loader" | "forge" | "neoforge" | "javafml" | "mixinextras"
    ) || (matches!(kind, ServerKind::Quilt) && id.starts_with("quilt_"))
}

fn expected_mod_loader(kind: ServerKind) -> Option<&'static str> {
    match kind {
        ServerKind::Fabric => Some("Fabric"),
        ServerKind::Quilt => Some("Quilt"),
        ServerKind::Forge | ServerKind::Mohist => Some("Forge"),
        ServerKind::NeoForge => Some("NeoForge"),
        _ => None,
    }
}

fn fits_loader(kind: ServerKind, m: &ModMeta) -> bool {
    let l = &m.loaders;
    match kind {
        ServerKind::Fabric => l.fabric,
        ServerKind::Quilt => l.quilt || l.fabric,
        ServerKind::Forge | ServerKind::Mohist => l.forge,
        ServerKind::NeoForge => l.neoforge || l.forge,
        ServerKind::Arclight => l.forge || l.neoforge || l.fabric,
        _ => true,
    }
}

fn plugins_supported(kind: ServerKind) -> bool {
    matches!(
        kind,
        ServerKind::Paper | ServerKind::Purpur | ServerKind::Spigot | ServerKind::Mohist | ServerKind::Arclight
    )
}

fn mods_supported(kind: ServerKind) -> bool {
    expected_mod_loader(kind).is_some() || kind == ServerKind::Arclight
}

/// Revisión de lo instalado (no necesita que el servidor haya arrancado).
fn static_checks(i: &Inputs, out: &mut Vec<Finding>) {
    let enabled: Vec<&ModMeta> = i.mods.iter().filter(|m| m.enabled).collect();
    let enabled_plugins: Vec<&ModMeta> = i.plugins.iter().filter(|m| m.enabled).collect();

    // Java demasiado antiguo para esta versión de Minecraft.
    if let (Some(have), Some(req)) = (i.java_major, i.mc_version.and_then(merc_core::java::required_java)) {
        if have < req.min_major {
            out.push(Finding {
                code: "java_old".into(),
                title: format!(
                    "Java {have} es demasiado antiguo para Minecraft {}",
                    i.mc_version.unwrap_or("?")
                ),
                confidence: Confidence::Alta,
                explanation: format!(
                    "Esta versión de Minecraft necesita Java {} o más nuevo. Con Java {have} el servidor no arranca.",
                    req.min_major
                ),
                evidence: vec![format!("Java configurado: versión {have}")],
                source: "Java del servidor".into(),
                actions: vec![FixAction::ChooseJava {
                    min_major: req.min_major,
                }],
            });
        }
    }

    if !mods_supported(i.kind) && !enabled.is_empty() {
        out.push(Finding {
            code: "mods_ignored".into(),
            title: "Este servidor no carga mods".into(),
            confidence: Confidence::Alta,
            explanation: "Los archivos de la carpeta mods/ no se usan en este tipo de servidor. Si querías mods, necesitas un servidor Fabric, Forge o NeoForge.".into(),
            evidence: enabled.iter().take(5).map(|m| format!("mods/{}", m.file)).collect(),
            source: "mods/".into(),
            actions: vec![],
        });
    }

    // Mods para otro loader.
    if let Some(loader) = expected_mod_loader(i.kind) {
        let has_connector = enabled.iter().any(|m| m.ids.contains("connector"));
        for m in &enabled {
            if !m.loaders.any() {
                continue;
            }
            if !fits_loader(i.kind, m) {
                let other = if m.loaders.fabric || m.loaders.quilt {
                    "Fabric/Quilt"
                } else if m.loaders.neoforge {
                    "NeoForge"
                } else if m.loaders.forge {
                    "Forge"
                } else {
                    "plugins (Bukkit/Paper)"
                };
                out.push(Finding {
                    code: "wrong_loader".into(),
                    title: format!("{} es para {other}, no para {loader}", label(m)),
                    confidence: if has_connector && (m.loaders.fabric || m.loaders.quilt) {
                        Confidence::Baja
                    } else {
                        Confidence::Alta
                    },
                    explanation: if has_connector {
                        format!("Tienes Sinytra Connector, que intenta cargar mods de Fabric en {loader}; aun así, este mod puede no funcionar. Si el servidor falla, desactívalo.")
                    } else {
                        format!("Su ficha solo es para {other}. {loader} no lo va a cargar o fallará al arrancar. Busca la versión para {loader} de ese mod.")
                    },
                    evidence: vec![format!("mods/{}: ficha de {other}", m.file)],
                    source: "mods/".into(),
                    actions: vec![
                        FixAction::DisableContent {
                            kind: "mod".into(),
                            file: m.file.clone(),
                        },
                        FixAction::SearchContent {
                            kind: "mod".into(),
                            query: m.name.clone().or(m.main_id.clone()).unwrap_or_default(),
                        },
                    ],
                });
            }
        }
    }

    // Mismo mod dos veces.
    let mut by_id: BTreeMap<&str, Vec<&ModMeta>> = BTreeMap::new();
    for m in &enabled {
        if let Some(id) = m.main_id.as_deref() {
            by_id.entry(id).or_default().push(m);
        }
    }
    for (id, list) in by_id.iter().filter(|(_, l)| l.len() > 1) {
        out.push(Finding {
            code: "duplicate".into(),
            title: format!("El mod «{id}» está {} veces", list.len()),
            confidence: Confidence::Alta,
            explanation: "Dos archivos traen el mismo mod (normalmente dos versiones). El loader no arranca así. Deja solo uno: normalmente el más nuevo.".into(),
            evidence: list.iter().map(|m| format!("mods/{} (versión {})", m.file, m.version.as_deref().unwrap_or("?"))).collect(),
            source: "mods/".into(),
            actions: list
                .iter()
                .map(|m| FixAction::DisableContent {
                    kind: "mod".into(),
                    file: m.file.clone(),
                })
                .collect(),
        });
    }

    // Dependencias obligatorias que faltan (solo se mira que existan, no su versión).
    let provided: BTreeSet<String> = enabled.iter().flat_map(|m| m.ids.iter().cloned()).collect();
    let has_fabric_api = provided.contains("fabric-api") || provided.contains("fabric");
    let mut missing: BTreeMap<String, Vec<&ModMeta>> = BTreeMap::new();
    for m in enabled.iter().filter(|m| fits_loader(i.kind, m)) {
        for r in &m.requires {
            if builtin(i.kind, r) || provided.contains(r) {
                continue;
            }
            // Los módulos de Fabric API vienen dentro de fabric-api.
            if (r == "fabric" || r.starts_with("fabric-")) && has_fabric_api {
                continue;
            }
            missing.entry(r.clone()).or_default().push(m);
        }
    }
    for (dep, who) in missing {
        out.push(Finding {
            code: "missing_dependency".into(),
            title: format!("Falta «{dep}», que necesita{} {}", if who.len() > 1 { "n" } else { "" }, who.iter().map(|m| label(m)).collect::<Vec<_>>().join(", ")),
            confidence: Confidence::Alta,
            explanation: format!("Su ficha dice que «{dep}» es obligatorio y no está en la carpeta mods/. Instálalo (en la versión para tu loader y tu Minecraft) o desactiva el mod que lo pide."),
            evidence: who.iter().map(|m| format!("mods/{}: requiere «{dep}»", m.file)).collect(),
            source: "mods/".into(),
            actions: std::iter::once(FixAction::SearchContent {
                kind: "mod".into(),
                query: dep.clone(),
            })
            .chain(who.iter().map(|m| FixAction::DisableContent {
                kind: "mod".into(),
                file: m.file.clone(),
            }))
            .collect(),
        });
    }

    // Solo para el cliente (lo declara la ficha del propio jar).
    for m in enabled.iter().filter(|m| m.client_only) {
        let fabric_like = m.loaders.fabric || m.loaders.quilt;
        out.push(Finding {
            code: "client_only".into(),
            title: format!("{} es solo para el juego (cliente)", label(m)),
            confidence: Confidence::Alta,
            explanation: if fabric_like {
                "Su ficha dice que es solo para el cliente: el servidor no lo usa. No hace daño, pero puedes desactivarlo o moverlo a la cuarentena con el separador de mods. Los jugadores sí pueden tenerlo en su juego."
            } else {
                "Su ficha dice que solo funciona en el juego (cliente). En Forge y NeoForge un mod así puede impedir que el servidor arranque: desactívalo o muévelo a la cuarentena con el separador de mods. Los jugadores sí pueden tenerlo en su juego."
            }
            .into(),
            evidence: vec![if fabric_like {
                format!("mods/{}: environment = client", m.file)
            } else {
                format!("mods/{}: depende de Minecraft y del loader solo con side=\"CLIENT\"", m.file)
            }],
            source: "mods/".into(),
            actions: vec![FixAction::DisableContent {
                kind: "mod".into(),
                file: m.file.clone(),
            }],
        });
    }

    // Plugins.
    if !plugins_supported(i.kind) && !enabled_plugins.is_empty() {
        out.push(Finding {
            code: "plugins_ignored".into(),
            title: "Este servidor no carga plugins".into(),
            confidence: Confidence::Alta,
            explanation: "Los plugins (Bukkit, Spigot o Paper) necesitan un servidor Paper, Purpur o Spigot (o un híbrido como Mohist o Arclight).".into(),
            evidence: enabled_plugins.iter().take(5).map(|m| format!("plugins/{}", m.file)).collect(),
            source: "plugins/".into(),
            actions: vec![],
        });
    } else if plugins_supported(i.kind) {
        let names: BTreeSet<String> = enabled_plugins.iter().flat_map(|m| m.ids.iter().cloned()).collect();
        for p in &enabled_plugins {
            if !p.loaders.plugin {
                out.push(Finding {
                    code: "not_a_plugin".into(),
                    title: format!("{} no parece un plugin", p.file),
                    confidence: Confidence::Media,
                    explanation: "No trae plugin.yml ni paper-plugin.yml. Si es un mod, va en otro tipo de servidor; si es una librería que pidió otro plugin, ignora este aviso.".into(),
                    evidence: vec![format!("plugins/{}: sin ficha de plugin", p.file)],
                    source: "plugins/".into(),
                    actions: vec![FixAction::DisableContent {
                        kind: "plugin".into(),
                        file: p.file.clone(),
                    }],
                });
                continue;
            }
            for r in p.requires.iter().filter(|r| !names.contains(*r)) {
                out.push(Finding {
                    code: "missing_plugin".into(),
                    title: format!("Falta el plugin «{r}», que necesita {}", label(p)),
                    confidence: Confidence::Alta,
                    explanation: format!(
                        "Su plugin.yml lo marca como obligatorio («depend»). Sin él, {} no se activará.",
                        p.name.as_deref().unwrap_or(&p.file)
                    ),
                    evidence: vec![format!("plugins/{}: depend {r}", p.file)],
                    source: "plugins/".into(),
                    actions: vec![FixAction::SearchContent {
                        kind: "plugin".into(),
                        query: r.clone(),
                    }],
                });
            }
        }
    }
}

struct Patterns {
    class_version: Regex,
    fabric_missing: Regex,
    neo_dep: Regex,
    invalid_dist: Regex,
    suspected: Regex,
    mixin: Regex,
    paren_id_end: Regex,
    paren_id: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        class_version: Regex::new(r"class file version (\d+)(?:\.\d+)?").expect("re"),
        fabric_missing: Regex::new(r"Mod '([^']+)' \(([^)]+)\) \S+ requires (.+?), which is missing").expect("re"),
        neo_dep: Regex::new(
            r"Mod ID: '([^']+)', Requested by: '([^']+)', Expected range: '([^']*)', Actual version: '([^']*)'",
        )
        .expect("re"),
        invalid_dist: Regex::new(r"Attempted to load class (\S+) for invalid dist DEDICATED_SERVER").expect("re"),
        suspected: Regex::new(r"Suspected Mods?: (.+)").expect("re"),
        mixin: Regex::new(r"Mixin apply (?:for mod|failed for) ([A-Za-z0-9_\-]+)").expect("re"),
        paren_id_end: Regex::new(r"\(([A-Za-z0-9_\-.]+)\)\s*$").expect("re"),
        paren_id: Regex::new(r"\(([A-Za-z0-9_\-]+)\)").expect("re"),
    })
}

/// IDs y nombres de archivo de un mod por su ID (para proponer desactivarlo).
fn files_for(i: &Inputs, id: &str) -> Vec<String> {
    let id = id.to_ascii_lowercase();
    i.mods
        .iter()
        .filter(|m| m.enabled && m.main_id.as_deref().map(str::to_ascii_lowercase).as_deref() == Some(id.as_str()))
        .map(|m| m.file.clone())
        .collect()
}

fn log_checks(i: &Inputs, out: &mut Vec<Finding>) {
    let p = patterns();
    for src in i.logs {
        let lines: Vec<&str> = src.text.lines().collect();
        let mut push = |f: Finding| {
            if !out.iter().any(|o| o.code == f.code && o.title == f.title) {
                out.push(f);
            }
        };
        for (n, line) in lines.iter().enumerate() {
            if line.contains("UnsupportedClassVersionError")
                || line.contains("compiled by a more recent version of the Java Runtime")
            {
                let need = p
                    .class_version
                    .captures(line)
                    .and_then(|c| c[1].parse::<u32>().ok())
                    .filter(|v| *v > 44)
                    .map(|v| v - 44);
                push(Finding {
                    code: "java_class_version".into(),
                    title: match need {
                        Some(j) => format!("Algo necesita Java {j} o más nuevo"),
                        None => "Algo necesita un Java más nuevo".into(),
                    },
                    confidence: Confidence::Alta,
                    explanation: "Un archivo del servidor (el propio servidor o un mod) se compiló para un Java más nuevo que el que usa este servidor.".into(),
                    evidence: vec![cut(line)],
                    source: src.name.clone(),
                    actions: need.map(|j| vec![FixAction::ChooseJava { min_major: j }]).unwrap_or_default(),
                });
            }
            if line.contains("java.lang.OutOfMemoryError") {
                let suggested = (i.max_memory_mb.saturating_mul(3) / 2).max(i.max_memory_mb + 1024);
                let cap = i.pc_memory_mb.map(|t| (t as u32).saturating_mul(3) / 4);
                let target = cap.map_or(suggested, |c| suggested.min(c));
                push(Finding {
                    code: "out_of_memory".into(),
                    title: "Al servidor se le acabó la memoria".into(),
                    confidence: Confidence::Alta,
                    explanation: format!(
                        "Java llegó al máximo asignado ({} MB). Con muchos mods o jugadores hace falta más. Sube la memoria sin pasarte de lo que tiene tu PC (deja sitio para Windows y, si juegas aquí, para el juego).",
                        i.max_memory_mb
                    ),
                    evidence: vec![cut(line)],
                    source: src.name.clone(),
                    actions: if target > i.max_memory_mb {
                        vec![FixAction::SetMemory { max_mb: target / 256 * 256 }]
                    } else {
                        vec![]
                    },
                });
            }
            if line.contains("FAILED TO BIND TO PORT") {
                push(Finding {
                    code: "port_in_use".into(),
                    title: "El puerto del servidor ya está ocupado".into(),
                    confidence: Confidence::Alta,
                    explanation: "Otro programa (u otro servidor) ya usa ese puerto. Cierra el otro o usa un puerto distinto. Si cambias el puerto y usas Playit, revisa la Conexión.".into(),
                    evidence: lines[n..lines.len().min(n + 2)].iter().map(|l| cut(l)).collect(),
                    source: src.name.clone(),
                    actions: i.free_port.map(|port| vec![FixAction::SetPort { port }]).unwrap_or_default(),
                });
            }
            if let Some(c) = p.fabric_missing.captures(line) {
                let tail = c[3].to_string();
                // «any version of fabric-api», «version 1.2 or later of mod 'X' (x)»…
                let dep = p
                    .paren_id_end
                    .captures(&tail)
                    .map(|x| x[1].to_string())
                    .or_else(|| tail.rsplit(' ').next().map(|s| s.trim_matches(['\'', '"']).to_string()))
                    .unwrap_or_default();
                push(Finding {
                    code: "log_missing_dependency".into(),
                    title: format!("{} necesita «{dep}»", &c[1]),
                    confidence: Confidence::Alta,
                    explanation: "Fabric lo dijo al arrancar: falta una dependencia (o su versión no vale). Instálala o desactiva el mod que la pide.".into(),
                    evidence: vec![cut(line)],
                    source: src.name.clone(),
                    actions: std::iter::once(FixAction::SearchContent {
                        kind: "mod".into(),
                        query: dep.clone(),
                    })
                    .chain(files_for(i, &c[2]).into_iter().map(|file| FixAction::DisableContent { kind: "mod".into(), file }))
                    .collect(),
                });
            }
            if let Some(c) = p.neo_dep.captures(line) {
                let (dep, by, range, actual) = (&c[1], &c[2], &c[3], &c[4]);
                let missing = actual.contains("MISSING");
                push(Finding {
                    code: "log_missing_dependency".into(),
                    title: if missing {
                        format!("«{by}» necesita «{dep}» y no está")
                    } else {
                        format!("«{by}» necesita otra versión de «{dep}»")
                    },
                    confidence: Confidence::Alta,
                    explanation: if missing {
                        format!("El loader lo dijo al arrancar. Instala «{dep}» (versión {range}) o desactiva «{by}».")
                    } else {
                        format!("Tienes «{dep}» {actual}, pero «{by}» pide {range}. Cambia «{dep}» a una versión compatible o desactiva «{by}».")
                    },
                    evidence: vec![cut(line)],
                    source: src.name.clone(),
                    actions: std::iter::once(FixAction::SearchContent {
                        kind: "mod".into(),
                        query: dep.to_string(),
                    })
                    .chain(files_for(i, by).into_iter().map(|file| FixAction::DisableContent {
                        kind: "mod".into(),
                        file,
                    }))
                    .collect(),
                });
            }
            if let Some(c) = p.invalid_dist.captures(line) {
                push(Finding {
                    code: "client_class_on_server".into(),
                    title: "Un mod de cliente intenta cargarse en el servidor".into(),
                    confidence: Confidence::Media,
                    explanation: "Algún mod usa partes del juego que solo existen en el cliente (pantallas, gráficos). Suele ser un mod solo para el juego que no debería estar en el servidor. Si el registro dice qué mod es (abajo, «Suspected Mods»), desactívalo.".into(),
                    evidence: vec![format!("Clase de cliente: {}", &c[1]), cut(line)],
                    source: src.name.clone(),
                    actions: vec![],
                });
            }
            if let Some(c) = p.suspected.captures(line) {
                let text = c[1].trim();
                if !text.eq_ignore_ascii_case("NONE") && !text.is_empty() {
                    // «Name (modid), Version: x, Issue tracker URL: …»
                    let id = p.paren_id.captures(text).map(|x| x[1].to_string());
                    let files = id.as_deref().map(|i2| files_for(i, i2)).unwrap_or_default();
                    push(Finding {
                        code: "suspected_mod".into(),
                        title: format!("El informe de error señala a {}", text.split(',').next().unwrap_or(text)),
                        confidence: Confidence::Media,
                        explanation: "Es el mod que aparecía en el error. No siempre es el culpable (a veces es otro mod que lo usa), pero es lo primero que conviene probar desactivando.".into(),
                        evidence: vec![cut(line)],
                        source: src.name.clone(),
                        actions: files.into_iter().map(|file| FixAction::DisableContent { kind: "mod".into(), file }).collect(),
                    });
                }
            }
            if let Some(c) = p.mixin.captures(line) {
                let id = c[1].to_string();
                push(Finding {
                    code: "mixin_failed".into(),
                    title: format!("El mod «{id}» no pudo modificar el juego"),
                    confidence: Confidence::Media,
                    explanation: "Suele pasar cuando el mod es para otra versión de Minecraft o choca con otro mod que cambia lo mismo. Comprueba que la versión del mod sea la de tu Minecraft.".into(),
                    evidence: vec![cut(line)],
                    source: src.name.clone(),
                    actions: files_for(i, &id).into_iter().map(|file| FixAction::DisableContent { kind: "mod".into(), file }).collect(),
                });
            }
            if line.contains("A single server tick took") {
                push(Finding {
                    code: "watchdog".into(),
                    title: "El servidor se quedó trabado y se cerró".into(),
                    confidence: Confidence::Media,
                    explanation: "Un ciclo del juego tardó demasiado y el vigilante de Minecraft lo cerró. Suele ser un mod o una granja muy pesada, o poca memoria. Revisa el informe de error y, si se repite, prueba a desactivar el último mod que instalaste.".into(),
                    evidence: vec![cut(line)],
                    source: src.name.clone(),
                    actions: vec![],
                });
            }
            if line.contains("Exception reading") && line.contains(".mca") || line.contains("Failed to read level.dat")
            {
                push(Finding {
                    code: "world_read".into(),
                    title: "No se pudo leer parte del mundo".into(),
                    confidence: Confidence::Baja,
                    explanation: "Un archivo del mundo podría estar dañado (por ejemplo, tras un apagón o un cierre forzado). Si faltan partes del mapa, restaura una copia de seguridad anterior.".into(),
                    evidence: vec![cut(line)],
                    source: src.name.clone(),
                    actions: vec![FixAction::OpenTab { tab: "backups".into() }],
                });
            }
        }
    }
}

pub fn diagnose(i: &Inputs, now_ms: i64) -> Diagnosis {
    let mut findings = vec![];
    static_checks(i, &mut findings);
    log_checks(i, &mut findings);
    findings.sort_by_key(|f| f.confidence);
    let mut sources = vec![];
    if !i.mods.is_empty() {
        sources.push(format!("mods/ ({} archivos)", i.mods.len()));
    }
    if !i.plugins.is_empty() {
        sources.push(format!("plugins/ ({} archivos)", i.plugins.len()));
    }
    sources.extend(i.logs.iter().map(|l| l.name.clone()));
    if i.java_major.is_some() {
        sources.push("versión de Java".into());
    }
    Diagnosis {
        checked_at_unix_ms: now_ms,
        sources,
        mods_scanned: i.mods.len() as u32,
        plugins_scanned: i.plugins.len() as u32,
        findings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modmeta::{Loaders, ModMeta};

    fn m(file: &str, id: &str, l: Loaders, requires: &[&str]) -> ModMeta {
        ModMeta {
            file: file.into(),
            enabled: true,
            loaders: l,
            ids: BTreeSet::from([id.to_string()]),
            main_id: Some(id.into()),
            requires: requires.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }
    fn fabric() -> Loaders {
        Loaders {
            fabric: true,
            ..Default::default()
        }
    }
    fn neo() -> Loaders {
        Loaders {
            neoforge: true,
            ..Default::default()
        }
    }
    fn input<'a>(kind: ServerKind, mods: &'a [ModMeta], logs: &'a [LogSource]) -> Inputs<'a> {
        Inputs {
            kind,
            mc_version: Some("1.21.1"),
            java_major: Some(21),
            max_memory_mb: 4096,
            pc_memory_mb: Some(16384),
            free_port: Some(25566),
            mods,
            plugins: &[],
            logs,
        }
    }
    fn codes(d: &Diagnosis) -> Vec<&str> {
        d.findings.iter().map(|f| f.code.as_str()).collect()
    }

    #[test]
    fn nothing_wrong_means_no_findings() {
        let mods = [
            m("fabric-api.jar", "fabric-api", fabric(), &["fabricloader", "minecraft"]),
            m(
                "sodium.jar",
                "sodium",
                fabric(),
                &["fabric-renderer-api-v1", "minecraft"],
            ),
        ];
        let d = diagnose(&input(ServerKind::Fabric, &mods, &[]), 1);
        assert!(d.findings.is_empty(), "{:?}", d.findings);
        assert_eq!(d.mods_scanned, 2);
    }

    #[test]
    fn static_problems_with_evidence_and_reversible_fixes() {
        let mut client = m("zoom.jar", "zoomify", fabric(), &[]);
        client.client_only = true;
        let mods = [
            m("jei-1.jar", "jei", neo(), &[]),
            m("jei-2.jar", "jei", neo(), &[]),
            m("mymod.jar", "mymod", neo(), &["geckolib", "neoforge"]),
            m("sodium.jar", "sodium", fabric(), &[]),
        ];
        let d = diagnose(&input(ServerKind::NeoForge, &mods, &[]), 1);
        let c = codes(&d);
        assert!(
            c.contains(&"duplicate") && c.contains(&"missing_dependency") && c.contains(&"wrong_loader"),
            "{c:?}"
        );
        let dup = d.findings.iter().find(|f| f.code == "duplicate").unwrap();
        assert_eq!(dup.evidence.len(), 2);
        assert!(dup.actions.contains(&FixAction::DisableContent {
            kind: "mod".into(),
            file: "jei-2.jar".into()
        }));
        let miss = d.findings.iter().find(|f| f.code == "missing_dependency").unwrap();
        assert!(miss.title.contains("geckolib"));
        assert!(miss.actions.contains(&FixAction::SearchContent {
            kind: "mod".into(),
            query: "geckolib".into()
        }));
        let fab = [client];
        assert!(codes(&diagnose(&input(ServerKind::Fabric, &fab, &[]), 1)).contains(&"client_only"));
        // Java viejo para 1.21.1.
        let mut i = input(ServerKind::Fabric, &[], &[]);
        i.java_major = Some(17);
        let d = diagnose(&i, 1);
        assert_eq!(d.findings[0].actions, vec![FixAction::ChooseJava { min_major: 21 }]);
    }

    #[test]
    fn reads_known_errors_from_logs() {
        let mods = [
            m("create.jar", "create", neo(), &[]),
            m("flywheel.jar", "flywheel", neo(), &[]),
        ];
        let logs = [
            LogSource {
                name: "consola".into(),
                text: "[main/ERROR]: Missing or unsupported mandatory dependencies:\n\tMod ID: 'ponder', Requested by: 'create', Expected range: '[1.0,)', Actual version: '[MISSING]'\njava.lang.OutOfMemoryError: Java heap space\n**** FAILED TO BIND TO PORT!\nThe exception was: java.net.BindException: Address already in use\n".into(),
            },
            LogSource {
                name: "crash-reports/crash-2026-10-30_16.00.00-server.txt".into(),
                text: "java.lang.RuntimeException: Attempted to load class net/minecraft/client/gui/screens/Screen for invalid dist DEDICATED_SERVER\nSuspected Mods: Flywheel (flywheel), Version: 1.0\n".into(),
            },
            LogSource {
                name: "logs/latest.log".into(),
                text: "Error: LinkageError occurred while loading main class net.minecraft.server.Main\njava.lang.UnsupportedClassVersionError: net/minecraft/server/Main has been compiled by a more recent version of the Java Runtime (class file version 65.0), this version of the Java Runtime only recognizes class file versions up to 61.0\n- Mod 'Sodium Extra' (sodium-extra) 0.5.4 requires any version of fabric-api, which is missing!\n".into(),
            },
        ];
        let d = diagnose(&input(ServerKind::NeoForge, &mods, &logs), 1);
        let c = codes(&d);
        for want in [
            "log_missing_dependency",
            "out_of_memory",
            "port_in_use",
            "client_class_on_server",
            "suspected_mod",
            "java_class_version",
        ] {
            assert!(c.contains(&want), "falta {want}: {c:?}");
        }
        let java = d.findings.iter().find(|f| f.code == "java_class_version").unwrap();
        assert_eq!(java.actions, vec![FixAction::ChooseJava { min_major: 21 }]);
        let port = d.findings.iter().find(|f| f.code == "port_in_use").unwrap();
        assert_eq!(port.actions, vec![FixAction::SetPort { port: 25566 }]);
        assert_eq!(port.evidence.len(), 2);
        let mem = d.findings.iter().find(|f| f.code == "out_of_memory").unwrap();
        assert_eq!(mem.actions, vec![FixAction::SetMemory { max_mb: 6144 }]);
        let sus = d.findings.iter().find(|f| f.code == "suspected_mod").unwrap();
        assert_eq!(
            sus.actions,
            vec![FixAction::DisableContent {
                kind: "mod".into(),
                file: "flywheel.jar".into()
            }]
        );
        let deps: Vec<&Finding> = d
            .findings
            .iter()
            .filter(|f| f.code == "log_missing_dependency")
            .collect();
        assert!(deps.iter().any(|f| f.title.contains("ponder")
            && f.actions.contains(&FixAction::DisableContent {
                kind: "mod".into(),
                file: "create.jar".into()
            })));
        assert!(deps.iter().any(|f| f.title.contains("fabric-api")));
        // Las evidencias son las líneas reales.
        assert!(java.evidence[0].contains("class file version 65.0"));
    }

    #[test]
    fn plugins_need_their_depends() {
        let p = ModMeta {
            file: "chat.jar".into(),
            enabled: true,
            loaders: Loaders {
                plugin: true,
                ..Default::default()
            },
            ids: BTreeSet::from(["essentialschat".to_string()]),
            main_id: Some("essentialschat".into()),
            name: Some("EssentialsChat".into()),
            requires: BTreeSet::from(["essentials".to_string()]),
            ..Default::default()
        };
        let plugins = [p];
        let mut i = input(ServerKind::Paper, &[], &[]);
        i.plugins = &plugins;
        let d = diagnose(&i, 1);
        assert_eq!(codes(&d), vec!["missing_plugin"]);
        let i2 = Inputs {
            kind: ServerKind::Vanilla,
            plugins: &plugins,
            ..input(ServerKind::Vanilla, &[], &[])
        };
        assert_eq!(codes(&diagnose(&i2, 1)), vec!["plugins_ignored"]);
    }
}
