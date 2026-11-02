//! Instalación de un servidor nuevo en una carpeta nueva o vacía.
//!
//! Regla de oro: solo se toca la carpeta destino, y únicamente si no existía o estaba
//! vacía. Si algo falla o se cancela, se borra lo que creamos y nada más.

use crate::catalog::{Flavor, Resolved};
use crate::http::Http;
use crate::InstallError;
use merc_core::detect::{detect_server, DetectedServer};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Tiempo máximo para un instalador oficial (descarga librerías; BuildTools además compila).
const INSTALLER_TIMEOUT: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone)]
pub struct InstallPlan {
    pub resolved: Resolved,
    /// Carpeta del servidor. Debe no existir o estar vacía.
    pub target: PathBuf,
    /// Java con el que se ejecutará el instalador (Forge, NeoForge, Quilt, Spigot).
    pub java: PathBuf,
    /// Carpeta de trabajo reutilizable para instaladores que compilan (BuildTools de Spigot).
    /// Debe estar fuera de la carpeta del servidor.
    pub cache_dir: Option<PathBuf>,
    /// Claves de server.properties elegidas por el usuario. Minecraft completa el resto.
    pub properties: BTreeMap<String, String>,
    /// El usuario marcó expresamente que acepta el EULA de Minecraft.
    pub accept_eula: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Preparing,
    Downloading,
    Installing,
    Configuring,
    Finishing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallEvent {
    Step(Step, String),
    Download {
        file: String,
        done: u64,
        total: Option<u64>,
    },
    Log(String),
}

#[derive(Debug, Clone)]
pub struct InstallOutcome {
    pub detected: DetectedServer,
}

/// Qué creamos para poder deshacerlo sin tocar nada ajeno.
#[derive(Debug)]
enum Prepared {
    /// Creamos esta carpeta (la primera que no existía en la ruta).
    Created(PathBuf),
    /// La carpeta ya existía y estaba vacía: todo lo de dentro es nuestro.
    WasEmpty(PathBuf),
}

impl Prepared {
    fn rollback(&self) {
        match self {
            Prepared::Created(top) => {
                let _ = std::fs::remove_dir_all(top);
            }
            Prepared::WasEmpty(dir) => {
                if let Ok(rd) = std::fs::read_dir(dir) {
                    for e in rd.flatten() {
                        let p = e.path();
                        let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                        let _ = if is_dir {
                            std::fs::remove_dir_all(&p)
                        } else {
                            std::fs::remove_file(&p)
                        };
                    }
                }
            }
        }
    }
}

fn io(path: &Path, e: std::io::Error) -> InstallError {
    InstallError::Io(format!("{}: {e}", path.display()))
}

/// Comprueba la carpeta destino sin escribir nada.
pub fn check_target(target: &Path) -> Result<(), InstallError> {
    if !target.is_absolute() {
        return Err(InstallError::InvalidInput(
            "la carpeta debe ser una ruta completa".into(),
        ));
    }
    match std::fs::symlink_metadata(target) {
        Ok(m) if m.file_type().is_symlink() => Err(InstallError::TargetNotUsable(
            "la carpeta es un acceso directo o enlace; elige una carpeta normal".into(),
        )),
        Ok(m) if !m.is_dir() => Err(InstallError::TargetNotUsable(
            "ya existe un archivo con ese nombre".into(),
        )),
        Ok(_) => {
            let mut rd = std::fs::read_dir(target).map_err(|e| io(target, e))?;
            if rd.next().is_some() {
                Err(InstallError::TargetNotEmpty(target.display().to_string()))
            } else {
                Ok(())
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io(target, e)),
    }
}

fn prepare_target(target: &Path) -> Result<Prepared, InstallError> {
    check_target(target)?;
    if target.is_dir() {
        return Ok(Prepared::WasEmpty(target.to_path_buf()));
    }
    // La carpeta más alta que vamos a crear: es la que se borraría al deshacer.
    let mut top = target.to_path_buf();
    while let Some(parent) = top.parent() {
        if parent.exists() {
            break;
        }
        top = parent.to_path_buf();
    }
    std::fs::create_dir_all(target).map_err(|e| io(target, e))?;
    Ok(Prepared::Created(top))
}

fn valid_property_key(k: &str) -> bool {
    !k.is_empty()
        && k.len() <= 64
        && k.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-._".contains(&b))
}

/// Escapa un valor al formato `.properties` (las tildes y emojis van como `\uXXXX`,
/// que Minecraft lee igual en todas las versiones).
pub fn escape_property_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for (i, ch) in v.chars().enumerate() {
        match ch {
            '\\' => out.push_str("\\\\"),
            ' ' if i == 0 => out.push_str("\\ "),
            c if c.is_ascii() && !c.is_ascii_control() => out.push(c),
            c => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{u:04x}"));
                }
            }
        }
    }
    out
}

/// Opciones de server.properties que el asistente puede fijar, con su validación.
/// Cualquier otra clave se rechaza: el resto lo decide Minecraft con sus valores por defecto.
pub fn validate_user_properties(props: &BTreeMap<String, String>) -> Result<(), InstallError> {
    let bad = |k: &str, why: &str| Err(InstallError::InvalidInput(format!("{k}: {why}")));
    for (k, v) in props {
        let int = |lo: i64, hi: i64| v.parse::<i64>().ok().filter(|n| (lo..=hi).contains(n)).is_some();
        let ok = match k.as_str() {
            "server-port" => int(1024, 65535),
            "max-players" => int(1, 1000),
            "view-distance" => int(3, 32),
            "simulation-distance" => int(3, 32),
            "spawn-protection" => int(0, 1000),
            "gamemode" => ["survival", "creative", "adventure", "spectator"].contains(&v.as_str()),
            "difficulty" => ["peaceful", "easy", "normal", "hard"].contains(&v.as_str()),
            "online-mode" | "white-list" | "enforce-whitelist" | "pvp" | "hardcore" | "allow-flight" => {
                v == "true" || v == "false"
            }
            "level-seed" => v.chars().count() <= 64,
            "motd" => v.chars().count() <= 120,
            _ => return bad(k, "esta opción no se puede fijar desde el asistente"),
        };
        if !ok {
            return bad(k, "valor no válido");
        }
    }
    Ok(())
}

/// Opciones de server.properties de Bedrock Dedicated Server que el asistente puede fijar.
/// Las claves de Bedrock son otras (server-name, allow-list, tick-distance…).
pub fn validate_bedrock_properties(props: &BTreeMap<String, String>) -> Result<(), InstallError> {
    let bad = |k: &str, why: &str| Err(InstallError::InvalidInput(format!("{k}: {why}")));
    for (k, v) in props {
        let int = |lo: i64, hi: i64| v.parse::<i64>().ok().filter(|n| (lo..=hi).contains(n)).is_some();
        let ok = match k.as_str() {
            "server-port" | "server-portv6" => int(1024, 65535),
            "max-players" => int(1, 1000),
            "view-distance" => int(5, 96),
            "tick-distance" => int(4, 12),
            "gamemode" => ["survival", "creative", "adventure"].contains(&v.as_str()),
            "difficulty" => ["peaceful", "easy", "normal", "hard"].contains(&v.as_str()),
            "online-mode" | "allow-list" | "allow-cheats" => v == "true" || v == "false",
            "level-seed" => v.chars().count() <= 64,
            "server-name" | "level-name" => {
                let n = v.chars().count();
                (1..=64).contains(&n) && !v.contains(['/', '\\', ';'])
            }
            _ => return bad(k, "esta opción no se puede fijar desde el asistente"),
        };
        if !ok || v.contains(['\n', '\r']) {
            return bad(k, "valor no válido");
        }
    }
    Ok(())
}

/// Opciones válidas según el tipo de servidor.
pub fn validate_properties_for(flavor: Flavor, props: &BTreeMap<String, String>) -> Result<(), InstallError> {
    if flavor.is_native() {
        validate_bedrock_properties(props)
    } else {
        validate_user_properties(props)
    }
}

/// Cambia (o añade) claves en un server.properties existente conservando el resto, comentarios
/// incluidos. Bedrock lee el archivo como texto UTF-8, así que los valores van tal cual.
pub fn merge_properties(text: &str, props: &BTreeMap<String, String>) -> Result<String, InstallError> {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    for (k, v) in props {
        if !valid_property_key(k) || v.contains(['\n', '\r']) || v.len() > 1024 {
            return Err(InstallError::InvalidInput(format!("opción no válida: {k:?}")));
        }
        let new = format!("{k}={v}");
        match lines.iter().position(|l| {
            let t = l.trim_start();
            !t.starts_with('#') && t.split_once('=').is_some_and(|(key, _)| key.trim() == k)
        }) {
            Some(i) => lines[i] = new,
            None => lines.push(new),
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    Ok(out)
}

/// Archivo con la huella SHA-256 del .zip oficial que se instaló (Mojang no la publica).
pub const BEDROCK_SHA_FILE: &str = "merchosting-bedrock-sha256.txt";
/// Límites al extraer el .zip oficial de Bedrock (ronda 100 MB y unos cientos de archivos).
const BEDROCK_MAX_ENTRIES: usize = 20_000;
const BEDROCK_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;

fn sha256_file(path: &Path) -> Result<String, InstallError> {
    use sha2::Digest as _;
    let mut f = std::fs::File::open(path).map_err(|e| io(path, e))?;
    let mut h = sha2::Sha256::new();
    std::io::copy(&mut f, &mut h).map_err(|e| io(path, e))?;
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Extrae el .zip oficial en `dir` con rutas seguras, sin enlaces y con límites.
pub fn extract_bedrock_zip(zip_path: &Path, dir: &Path, cancel: &AtomicBool) -> Result<usize, InstallError> {
    let f = std::fs::File::open(zip_path).map_err(|e| io(zip_path, e))?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| InstallError::Integrity(format!("el .zip no es válido: {e}")))?;
    if z.len() > BEDROCK_MAX_ENTRIES {
        return Err(InstallError::Integrity("el .zip tiene demasiados archivos".into()));
    }
    let mut total: u64 = 0;
    let mut files = 0;
    for i in 0..z.len() {
        if cancel.load(Ordering::Relaxed) {
            return Err(InstallError::Cancelled);
        }
        let mut e = z
            .by_index(i)
            .map_err(|e| InstallError::Integrity(format!("el .zip está dañado: {e}")))?;
        let Some(rel) = e.enclosed_name() else {
            return Err(InstallError::Integrity(format!(
                "ruta no permitida en el .zip: {}",
                e.name().map(|n| n.into_owned()).unwrap_or_default()
            )));
        };
        if e.is_symlink() {
            return Err(InstallError::Integrity(format!(
                "el .zip trae un enlace: {}",
                e.name().map(|n| n.into_owned()).unwrap_or_default()
            )));
        }
        let out = dir.join(&rel);
        if e.is_dir() {
            std::fs::create_dir_all(&out).map_err(|er| io(&out, er))?;
            continue;
        }
        total = total.saturating_add(e.size());
        if total > BEDROCK_MAX_BYTES {
            return Err(InstallError::TooLarge("el contenido del .zip de Bedrock".into()));
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p).map_err(|er| io(p, er))?;
        }
        let mut w = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&out)
            .map_err(|er| io(&out, er))?;
        std::io::copy(&mut e, &mut w).map_err(|er| io(&out, er))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = e.unix_mode().unwrap_or(0o644) & 0o755;
            let exe = rel
                .file_name()
                .is_some_and(|n| n == merc_core::detect::bedrock_exe_name());
            let _ = std::fs::set_permissions(
                &out,
                std::fs::Permissions::from_mode(if exe { 0o755 } else { mode | 0o600 }),
            );
        }
        files += 1;
    }
    Ok(files)
}

pub fn render_properties(props: &BTreeMap<String, String>) -> Result<String, InstallError> {
    let mut text =
        String::from("#Creado por MercHosting. Minecraft agregara el resto de opciones en el primer arranque.\n");
    for (k, v) in props {
        if !valid_property_key(k) {
            return Err(InstallError::InvalidInput(format!("opción no válida: {k:?}")));
        }
        if v.contains(['\n', '\r']) || v.len() > 1024 {
            return Err(InstallError::InvalidInput(format!("valor no válido para {k}")));
        }
        text.push_str(&format!("{k}={}\n", escape_property_value(v)));
    }
    Ok(text)
}

fn write_new(path: &Path, text: &str) -> Result<(), InstallError> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| io(path, e))?;
    f.write_all(text.as_bytes()).map_err(|e| io(path, e))?;
    f.sync_all().map_err(|e| io(path, e))
}

/// ¿El Java elegido es un JDK? (tiene `javac` al lado)
pub fn has_javac(java: &Path) -> bool {
    let name = if cfg!(windows) { "javac.exe" } else { "javac" };
    java.parent().is_some_and(|d| d.join(name).is_file())
}

fn flavor_name(f: Flavor) -> &'static str {
    match f {
        Flavor::Vanilla => "Minecraft",
        Flavor::Paper => "Paper",
        Flavor::Purpur => "Purpur",
        Flavor::Spigot => "Spigot (BuildTools)",
        Flavor::Fabric => "Fabric",
        Flavor::Quilt => "Quilt",
        Flavor::Forge => "Forge",
        Flavor::NeoForge => "NeoForge",
        Flavor::Arclight => "Arclight",
        Flavor::Bedrock => "Bedrock Dedicated Server",
    }
}

/// Ejecuta `java -jar instalador <args>` en `dir`, sin shell.
fn run_installer(
    java: &Path,
    installer: &Path,
    args: &[String],
    dir: &Path,
    events: &mut dyn FnMut(InstallEvent),
    cancel: &AtomicBool,
) -> Result<(), InstallError> {
    let mut cmd = Command::new(java);
    cmd.arg("-jar")
        .arg(installer)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    merc_core::process_flags::no_console_window(&mut cmd);
    let mut child = cmd
        .spawn()
        .map_err(|e| InstallError::InstallerFailed(format!("no se pudo ejecutar Java ({}): {e}", java.display())))?;
    let (tx, rx) = mpsc::channel::<String>();
    let mut readers = vec![];
    for stream in [
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || {
            let mut r = BufReader::new(stream);
            let mut buf = Vec::new();
            while r.read_until(b'\n', &mut buf).map(|n| n > 0).unwrap_or(false) {
                let line = String::from_utf8_lossy(&buf).trim_end().to_string();
                if !line.is_empty() && tx.send(line).is_err() {
                    break;
                }
                buf.clear();
            }
        }));
    }
    drop(tx);
    let started = Instant::now();
    let mut tail: Vec<String> = vec![];
    let status = loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => {
                tail.push(line.clone());
                if tail.len() > 12 {
                    tail.remove(0);
                }
                events(InstallEvent::Log(line));
                continue;
            }
            Err(mpsc::RecvTimeoutError::Timeout) | Err(mpsc::RecvTimeoutError::Disconnected) => {}
        }
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(InstallError::Cancelled);
        }
        if started.elapsed() > INSTALLER_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err(InstallError::InstallerFailed(
                "el instalador tardó demasiado y se detuvo".into(),
            ));
        }
        if let Some(st) = child
            .try_wait()
            .map_err(|e| InstallError::InstallerFailed(e.to_string()))?
        {
            break st;
        }
    };
    for r in readers {
        let _ = r.join();
    }
    for line in rx.try_iter() {
        events(InstallEvent::Log(line));
    }
    if !status.success() {
        return Err(InstallError::InstallerFailed(format!(
            "el instalador terminó con error ({status}). Últimas líneas:\n{}",
            tail.join("\n")
        )));
    }
    Ok(())
}

pub fn install(
    http: &Http,
    plan: &InstallPlan,
    events: &mut dyn FnMut(InstallEvent),
    cancel: &AtomicBool,
) -> Result<InstallOutcome, InstallError> {
    // Validar todo lo que no depende de la red antes de crear nada.
    let props_text = if plan.resolved.flavor.is_native() {
        validate_bedrock_properties(&plan.properties)?;
        String::new()
    } else {
        render_properties(&plan.properties)?
    };
    if !crate::catalog::valid_download_name(plan.resolved.flavor, &plan.resolved.artifact.file_name) {
        return Err(InstallError::InvalidInput("nombre de archivo no válido".into()));
    }
    events(InstallEvent::Step(
        Step::Preparing,
        "Preparando la carpeta del servidor".into(),
    ));
    let prepared = prepare_target(&plan.target)?;
    match install_inner(http, plan, &props_text, events, cancel) {
        Ok(o) => Ok(o),
        Err(e) => {
            prepared.rollback();
            Err(e)
        }
    }
}

fn install_inner(
    http: &Http,
    plan: &InstallPlan,
    props_text: &str,
    events: &mut dyn FnMut(InstallEvent),
    cancel: &AtomicBool,
) -> Result<InstallOutcome, InstallError> {
    let r = &plan.resolved;
    let dir = &plan.target;
    let art = &r.artifact;
    if r.needs_jdk && !has_javac(&plan.java) {
        return Err(InstallError::InvalidInput(
            "Spigot se compila en tu PC y necesita un JDK (Java con compilador), no solo Java. \
             Instala un JDK (por ejemplo, Eclipse Temurin JDK) y elígelo."
                .into(),
        ));
    }
    let dest = dir.join(&art.file_name);
    events(InstallEvent::Step(
        Step::Downloading,
        format!("Descargando {}", art.file_name),
    ));
    let file = art.file_name.clone();
    let sources: Vec<&String> = std::iter::once(&art.url).chain(art.mirrors.iter()).collect();
    let mut last_err = None;
    for (i, url) in sources.iter().enumerate() {
        events(InstallEvent::Log(format!("Origen: {url}")));
        let r = http.download(
            url,
            &dest,
            &art.checksum,
            art.size,
            &mut |done, total| {
                events(InstallEvent::Download {
                    file: file.clone(),
                    done,
                    total,
                })
            },
            cancel,
        );
        match r {
            Ok(_) => {
                last_err = None;
                break;
            }
            Err(InstallError::Cancelled) => return Err(InstallError::Cancelled),
            Err(e) => {
                if i + 1 < sources.len() {
                    events(InstallEvent::Log(format!(
                        "Ese origen no sirvió ({e}); se prueba el siguiente"
                    )));
                }
                last_err = Some(e);
            }
        }
    }
    if let Some(e) = last_err {
        return Err(e);
    }
    events(InstallEvent::Log(match art.checksum.expected() {
        Some(_) => format!("{} verificado ({})", art.file_name, art.checksum.algorithm()),
        None => format!("{} descargado por HTTPS (la fuente no publica huella)", art.file_name),
    }));

    if r.flavor.is_native() {
        events(InstallEvent::Step(
            Step::Installing,
            "Extrayendo Bedrock Dedicated Server".into(),
        ));
        let sha = sha256_file(&dest)?;
        events(InstallEvent::Log(format!("SHA-256 del .zip descargado: {sha}")));
        let n = extract_bedrock_zip(&dest, dir, cancel)?;
        events(InstallEvent::Log(format!("{n} archivos extraídos")));
        let _ = std::fs::remove_file(&dest);
        write_new(
            &dir.join(merc_core::detect::BEDROCK_VERSION_FILE),
            &format!("{}\n", r.mc_version),
        )?;
        write_new(
            &dir.join(BEDROCK_SHA_FILE),
            &format!(
                "# Huella del archivo oficial descargado por MercHosting (Mojang no la publica).\n{sha}  {}\n{}\n",
                art.file_name, art.url
            ),
        )?;
    }
    if let Some(args) = &r.installer_args {
        events(InstallEvent::Step(
            Step::Installing,
            format!("Ejecutando el instalador oficial de {}", flavor_name(r.flavor)),
        ));
        let target = dir.to_string_lossy().into_owned();
        let args: Vec<String> = args.iter().map(|a| a.replace("{server_dir}", &target)).collect();
        let work = if r.installer_in_cache {
            let c = plan
                .cache_dir
                .clone()
                .ok_or_else(|| InstallError::InvalidInput("falta la carpeta de trabajo del instalador".into()))?;
            std::fs::create_dir_all(&c).map_err(|e| io(&c, e))?;
            events(InstallEvent::Log(format!("Carpeta de trabajo: {}", c.display())));
            c
        } else {
            dir.clone()
        };
        run_installer(&plan.java, &dest, &args, &work, events, cancel)?;
        // El instalador ya no hace falta; su registro (.log) se conserva.
        let _ = std::fs::remove_file(&dest);
        if r.flavor == Flavor::Forge && crate::catalog::forge_is_legacy(&r.mc_version) {
            let jar = check_legacy_forge(dir)?;
            events(InstallEvent::Log(format!("JAR del servidor de Forge: {jar}")));
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(InstallError::Cancelled);
    }

    events(InstallEvent::Step(
        Step::Configuring,
        "Guardando la configuración elegida".into(),
    ));
    let props_path = dir.join("server.properties");
    if r.flavor.is_native() {
        // El .zip de Bedrock trae su server.properties con comentarios: se cambian solo las
        // claves elegidas.
        let text = std::fs::read_to_string(&props_path).unwrap_or_default();
        let merged = merge_properties(&text, &plan.properties)?;
        std::fs::write(&props_path, merged).map_err(|e| io(&props_path, e))?;
    } else if props_path.exists() {
        // Ningún instalador oficial la crea; si apareciera, no la pisamos.
        return Err(InstallError::InstallerFailed(
            "el instalador creó server.properties inesperadamente; revisa la carpeta".into(),
        ));
    }
    if !r.flavor.is_native() {
        write_new(&props_path, props_text)?;
    }
    if plan.accept_eula {
        let text = if r.flavor.is_native() {
            "#Aceptados expresamente por el usuario en MercHosting: EULA y Politica de privacidad de Minecraft (https://aka.ms/MinecraftEULA).\n\
             #Bedrock Dedicated Server no lee este archivo; MercHosting lo usa para no encenderlo sin tu aceptacion.\neula=true\n"
        } else {
            "#Aceptado expresamente por el usuario en MercHosting (https://aka.ms/MinecraftEULA)\neula=true\n"
        };
        write_new(&dir.join("eula.txt"), text)?;
    }

    events(InstallEvent::Step(Step::Finishing, "Comprobando la instalación".into()));
    let detected = detect_server(dir);
    if detected.kind != r.flavor.kind() || detected.launch.is_none() {
        return Err(InstallError::InstallerFailed(format!(
            "la carpeta no quedó como un servidor {:?} válido ({})",
            r.flavor,
            detected.warnings.join("; ")
        )));
    }
    Ok(InstallOutcome { detected })
}

/// Forge antiguo: el instalador deja `forge-<mc>-<build>[-universal].jar` en la carpeta. Se
/// busca (no se supone su nombre) y se comprueba que es ejecutable (`Main-Class` en su
/// manifiesto); si no, el error lo dice en vez de dejar un servidor que no arranca.
pub fn check_legacy_forge(dir: &Path) -> Result<String, InstallError> {
    let (jar, _, _) = merc_core::detect::legacy_forge_jar(dir).ok_or_else(|| {
        InstallError::InstallerFailed(
            "el instalador de Forge terminó, pero no dejó el JAR del servidor (forge-<versión>.jar). \
             Revisa el archivo .log del instalador en la carpeta"
                .into(),
        )
    })?;
    if !jar_has_main_class(&dir.join(&jar)) {
        return Err(InstallError::InstallerFailed(format!(
            "{jar} no es un JAR ejecutable (no tiene Main-Class en su manifiesto)"
        )));
    }
    Ok(jar)
}

fn jar_has_main_class(path: &Path) -> bool {
    use std::io::Read;
    let Ok(f) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(mut zip) = zip::ZipArchive::new(f) else {
        return false;
    };
    let Ok(mut m) = zip.by_name("META-INF/MANIFEST.MF") else {
        return false;
    };
    if m.size() > 1024 * 1024 {
        return false;
    }
    let mut text = String::new();
    if m.read_to_string(&mut text).is_err() {
        return false;
    }
    text.lines()
        .any(|l| l.strip_prefix("Main-Class:").is_some_and(|v| !v.trim().is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jar(path: &Path, manifest: &str) {
        use std::io::Write;
        let f = std::fs::File::create(path).unwrap();
        let mut z = zip::ZipWriter::new(f);
        z.start_file("META-INF/MANIFEST.MF", zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(manifest.as_bytes()).unwrap();
        z.finish().unwrap();
    }

    #[test]
    fn legacy_forge_jar_must_be_executable() {
        let d = tempfile::tempdir().unwrap();
        assert!(matches!(check_legacy_forge(d.path()), Err(InstallError::InstallerFailed(m)) if m.contains(".log")));
        jar(
            &d.path().join("forge-1.12.2-14.23.5.2860.jar"),
            "Manifest-Version: 1.0\r\n",
        );
        assert!(
            matches!(check_legacy_forge(d.path()), Err(InstallError::InstallerFailed(m)) if m.contains("Main-Class"))
        );
        jar(
            &d.path().join("forge-1.12.2-14.23.5.2860.jar"),
            "Manifest-Version: 1.0\r\nMain-Class: net.minecraftforge.fml.relauncher.ServerLaunchWrapper\r\n",
        );
        assert_eq!(check_legacy_forge(d.path()).unwrap(), "forge-1.12.2-14.23.5.2860.jar");
        std::fs::write(d.path().join("roto.jar"), "no es zip").unwrap();
        assert!(!jar_has_main_class(&d.path().join("roto.jar")));
    }

    #[test]
    fn properties_escape() {
        assert_eq!(escape_property_value("Hola mundo"), "Hola mundo");
        assert_eq!(escape_property_value("Café ñ"), "Caf\\u00e9 \\u00f1");
        assert_eq!(escape_property_value("a\\b"), "a\\\\b");
        assert_eq!(escape_property_value(" x"), "\\ x");
        assert_eq!(escape_property_value("🎮"), "\\ud83c\\udfae");
        let mut p = BTreeMap::new();
        p.insert("server-port".to_string(), "25566".to_string());
        p.insert("motd".to_string(), "Mi server".to_string());
        let t = render_properties(&p).unwrap();
        assert!(t.contains("motd=Mi server\nserver-port=25566\n"));
        p.insert("bad key".into(), "x".into());
        assert!(render_properties(&p).is_err());
        let mut p = BTreeMap::new();
        p.insert("motd".to_string(), "a\nonline-mode=false".to_string());
        assert!(render_properties(&p).is_err());
    }

    #[test]
    fn user_properties_whitelist() {
        let mk = |k: &str, v: &str| BTreeMap::from([(k.to_string(), v.to_string())]);
        assert!(validate_user_properties(&mk("server-port", "25566")).is_ok());
        assert!(validate_user_properties(&mk("server-port", "80")).is_err());
        assert!(validate_user_properties(&mk("gamemode", "creative")).is_ok());
        assert!(validate_user_properties(&mk("gamemode", "god")).is_err());
        assert!(validate_user_properties(&mk("online-mode", "false")).is_ok());
        assert!(validate_user_properties(&mk("enable-rcon", "true")).is_err());
        assert!(validate_user_properties(&mk("motd", "Hola ñandú")).is_ok());
    }

    #[test]
    fn bedrock_properties() {
        let mk = |k: &str, v: &str| BTreeMap::from([(k.to_string(), v.to_string())]);
        assert!(validate_bedrock_properties(&mk("server-name", "Mi server ñ")).is_ok());
        assert!(validate_bedrock_properties(&mk("gamemode", "spectator")).is_err());
        assert!(validate_bedrock_properties(&mk("motd", "x")).is_err());
        assert!(validate_bedrock_properties(&mk("allow-list", "true")).is_ok());
        assert!(validate_properties_for(Flavor::Bedrock, &mk("white-list", "true")).is_err());
        assert!(validate_properties_for(Flavor::Paper, &mk("white-list", "true")).is_ok());
        let text = "# comentario\nserver-name=Dedicated Server\n# server-port=1\nserver-port=19132\n";
        let mut p = BTreeMap::new();
        p.insert("server-port".to_string(), "19140".to_string());
        p.insert("level-seed".to_string(), "123".to_string());
        let out = merge_properties(text, &p).unwrap();
        assert_eq!(
            out,
            "# comentario\nserver-name=Dedicated Server\n# server-port=1\nserver-port=19140\nlevel-seed=123\n"
        );
    }

    #[test]
    fn bedrock_zip_extraction_is_safe() {
        use std::io::Write;
        let d = tempfile::tempdir().unwrap();
        let zp = d.path().join("b.zip");
        let mk = |entries: &[(&str, &str)]| {
            let f = std::fs::File::create(&zp).unwrap();
            let mut z = zip::ZipWriter::new(f);
            for (n, b) in entries {
                z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
                z.write_all(b.as_bytes()).unwrap();
            }
            z.finish().unwrap();
        };
        mk(&[
            (merc_core::detect::bedrock_exe_name(), "exe"),
            ("server.properties", "server-port=19132\n"),
            ("behavior_packs/vanilla/manifest.json", "{}"),
        ]);
        let out = d.path().join("srv");
        std::fs::create_dir(&out).unwrap();
        assert_eq!(extract_bedrock_zip(&zp, &out, &AtomicBool::new(false)).unwrap(), 3);
        assert!(out.join("behavior_packs/vanilla/manifest.json").is_file());
        mk(&[("../fuera.txt", "x")]);
        let out2 = d.path().join("srv2");
        std::fs::create_dir(&out2).unwrap();
        assert!(extract_bedrock_zip(&zp, &out2, &AtomicBool::new(false)).is_err());
        assert!(!d.path().join("fuera.txt").exists());
    }

    #[test]
    fn target_rules() {
        let d = tempfile::tempdir().unwrap();
        assert!(check_target(d.path()).is_ok());
        assert!(check_target(&d.path().join("nuevo/sub")).is_ok());
        std::fs::write(d.path().join("algo.txt"), "x").unwrap();
        assert!(matches!(check_target(d.path()), Err(InstallError::TargetNotEmpty(_))));
        assert!(matches!(
            check_target(&d.path().join("algo.txt")),
            Err(InstallError::TargetNotUsable(_))
        ));
        assert!(check_target(Path::new("relativa")).is_err());
    }

    #[test]
    fn rollback_only_removes_what_we_created() {
        let d = tempfile::tempdir().unwrap();
        let target = d.path().join("a/b/servidor");
        let p = prepare_target(&target).unwrap();
        std::fs::write(target.join("x.jar"), "x").unwrap();
        p.rollback();
        assert!(!d.path().join("a").exists());
        assert!(d.path().exists());

        let empty = d.path().join("vacia");
        std::fs::create_dir(&empty).unwrap();
        let p = prepare_target(&empty).unwrap();
        std::fs::create_dir_all(empty.join("libraries/x")).unwrap();
        std::fs::write(empty.join("y.jar"), "y").unwrap();
        p.rollback();
        assert!(empty.is_dir());
        assert_eq!(std::fs::read_dir(&empty).unwrap().count(), 0);
    }
}
