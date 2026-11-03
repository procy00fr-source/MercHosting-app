//! Servidores Java dentro del celular: se guardan en la carpeta privada de la app, se instalan
//! con el mismo instalador verificado de escritorio (solo tipos que no necesitan ejecutar un
//! instalador de Java: Vanilla, Paper, Purpur y Fabric) y se encienden en un proceso aparte
//! (`jli`). El estado se deduce del PID del proceso y de lo que escribe el servidor.

use crate::assets::{self, Asset};
use crate::jli::LaunchSpec;
use crate::ram::{self, HeapLimits};
use crate::warnings;
use merc_core::readiness::{classify_line, LogSignal};
use merc_install::{Checksum, Flavor, Http, InstallError, Sources};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

/// Tipos que se pueden crear en el celular (no ejecutan instaladores con Java).
pub const PHONE_FLAVORS: [Flavor; 4] = [Flavor::Vanilla, Flavor::Paper, Flavor::Purpur, Flavor::Fabric];

pub fn flavor_allowed(f: Flavor) -> bool {
    PHONE_FLAVORS.contains(&f)
}

#[derive(Debug, thiserror::Error)]
pub enum PhoneError {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Unsupported(String),
    #[error("{0}")]
    Io(String),
    #[error(transparent)]
    Install(#[from] InstallError),
}

impl From<std::io::Error> for PhoneError {
    fn from(e: std::io::Error) -> Self {
        PhoneError::Io(e.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalServer {
    pub id: String,
    pub name: String,
    pub flavor: Flavor,
    pub mc_version: String,
    pub java_major: u32,
    pub heap_mb: u32,
    /// Ruta del .jar relativa a la carpeta del servidor.
    pub jar: String,
    pub created_unix_ms: i64,
    /// Si Playit se arranca junto con el servidor.
    #[serde(default)]
    pub playit: bool,
    /// Lleva Geyser y Floodgate: entran también jugadores de Minecraft Bedrock.
    #[serde(default)]
    pub bedrock: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalState {
    Stopped,
    Starting,
    Online,
    Stopping,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocalStatus {
    pub state: LocalState,
    pub pid: Option<u32>,
    /// El EULA está aceptado (sin eso no se enciende).
    pub eula_accepted: bool,
    /// Puerto de server.properties.
    pub port: Option<u16>,
    /// Último problema reconocido en la consola (puerto ocupado, EULA…).
    pub problem: Option<String>,
    /// Puerto UDP de Geyser para los jugadores de Bedrock (si Geyser está instalado).
    #[serde(default)]
    pub bedrock_port: Option<u16>,
}

/// Lo que se ve de este celular antes de crear nada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PhoneInfo {
    pub arch: String,
    pub supported: bool,
    /// Por qué no se puede, si no se puede.
    pub reason: Option<String>,
    pub total_ram_mb: u64,
    pub heap: Option<HeapLimits>,
    /// JRE ya descargados (17, 21).
    pub jre_installed: Vec<u32>,
    /// El agente de Playit va dentro de la app.
    pub playit_available: bool,
    pub warnings: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CreateRequest {
    pub name: String,
    pub flavor: Flavor,
    pub mc_version: String,
    pub heap_mb: u32,
    pub accept_eula: bool,
    /// Identificadores de [`warnings::REQUIRED`] que la persona marcó.
    pub warnings_accepted: Vec<String>,
    /// Instalar Geyser y Floodgate (solo Paper o Purpur) para que entren jugadores de Bedrock.
    #[serde(default)]
    pub bedrock: bool,
}

pub struct Phone {
    data: PathBuf,
    arch: String,
    total_ram_mb: u64,
    /// Carpeta de bibliotecas nativas de la app (donde va `libplayit.so`).
    native_dir: Option<PathBuf>,
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Ayudante de diagnóstico (código en `agent/src`, se recompila con `agent/build.sh`): escribe en
/// la consola qué hace cada hilo cuando la persona lo pide (en Android no hay jstack).
pub const AGENT_JAR: &[u8] = include_bytes!("../agent/merc-agent.jar");

/// En Android el transporte de red nativo de Minecraft (epoll de Netty, hecho para Linux de PC)
/// puede aceptar conexiones y no responder nunca: el juego se queda en «Pinging…» y «Joining».
/// Se apaga con la opción oficial `use-native-transport=false` (las demás líneas no se tocan).
fn ensure_android_properties(dir: &Path) -> Result<(), PhoneError> {
    let path = dir.join("server.properties");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut found = false;
    let mut out: Vec<String> = text
        .lines()
        .map(|l| {
            if l.trim_start().starts_with("use-native-transport=") {
                found = true;
                "use-native-transport=false".to_string()
            } else {
                l.to_string()
            }
        })
        .collect();
    if !found {
        out.push("use-native-transport=false".to_string());
    }
    let new = out.join("\n") + "\n";
    if new != text {
        let tmp = dir.join("server.properties.merchosting.tmp");
        std::fs::write(&tmp, new)?;
        std::fs::rename(&tmp, &path)?;
    }
    Ok(())
}

/// Nombre visible válido.
fn clean_name(name: &str) -> Result<String, PhoneError> {
    merc_core::instance::validate_instance_name(name).map_err(|e| PhoneError::Invalid(e.into()))
}

impl Phone {
    pub fn new(data: PathBuf, arch: &str, total_ram_mb: u64, native_dir: Option<PathBuf>) -> Self {
        Self {
            data,
            arch: arch.to_string(),
            total_ram_mb,
            native_dir,
        }
    }

    fn registry(&self) -> PathBuf {
        self.data.join("servidores.json")
    }

    /// Carpeta privada de la app con todos los datos.
    pub(crate) fn data(&self) -> &Path {
        &self.data
    }

    pub fn server_dir(&self, id: &str) -> PathBuf {
        self.data.join("servidores").join(id)
    }

    pub(crate) fn run_dir(&self, id: &str) -> PathBuf {
        self.data.join("ejecucion").join(id)
    }

    pub fn jre_dir(&self, major: u32) -> PathBuf {
        self.data.join("jre").join(major.to_string())
    }

    pub fn backups_dir(&self, id: &str) -> PathBuf {
        self.data.join("copias").join(id)
    }

    pub fn playit_dir(&self) -> PathBuf {
        self.data.join("playit")
    }

    pub fn playit_paths(&self) -> crate::playit::AgentPaths {
        crate::playit::AgentPaths::new(&self.playit_dir())
    }

    /// Ruta del agente de Playit empaquetado, si existe.
    pub fn playit_exe(&self) -> Option<PathBuf> {
        let p = self.native_dir.as_ref()?.join(assets::PLAYIT_LIB_NAME);
        p.is_file().then_some(p)
    }

    pub fn info(&self) -> PhoneInfo {
        let heap = ram::heap_limits(self.total_ram_mb);
        let reason = if !assets::supported_arch(&self.arch) {
            Some(format!(
                "Este celular es {}: el Java para Android verificado es solo para celulares de 64 bits ARM (arm64).",
                self.arch
            ))
        } else if heap.is_none() {
            Some(format!(
                "Este celular tiene {} MB de memoria: se necesitan al menos {} MB para el sistema y un servidor.",
                self.total_ram_mb,
                ram::MIN_DEVICE_MB
            ))
        } else {
            None
        };
        PhoneInfo {
            arch: self.arch.clone(),
            supported: reason.is_none(),
            reason,
            total_ram_mb: self.total_ram_mb,
            heap,
            jre_installed: [17, 21].into_iter().filter(|m| self.jre_ready(*m)).collect(),
            playit_available: self.playit_exe().is_some(),
            warnings: warnings::REQUIRED.to_vec(),
        }
    }

    pub fn list(&self) -> Vec<LocalServer> {
        std::fs::read_to_string(self.registry())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn save(&self, list: &[LocalServer]) -> Result<(), PhoneError> {
        std::fs::create_dir_all(&self.data)?;
        let tmp = self.registry().with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(list).unwrap_or_default())?;
        std::fs::rename(&tmp, self.registry())?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<LocalServer, PhoneError> {
        self.list()
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| PhoneError::Invalid("No existe ese servidor en este celular.".into()))
    }

    fn jre_ready(&self, major: u32) -> bool {
        let d = self.jre_dir(major);
        d.join("lib/libjli.so").is_file() && d.join("lib/server/libjvm.so").is_file()
    }

    /// Descarga (comprobando la huella) y descomprime el JRE si falta.
    pub fn ensure_jre(
        &self,
        http: &Http,
        major: u32,
        progress: &mut dyn FnMut(u64, Option<u64>),
        cancel: &AtomicBool,
    ) -> Result<PathBuf, PhoneError> {
        let dest = self.jre_dir(major);
        if self.jre_ready(major) {
            return Ok(dest);
        }
        let asset: Asset = assets::jre(major, &self.arch)
            .ok_or_else(|| PhoneError::Unsupported(format!("No hay Java {major} verificado para este celular.")))?;
        let cache = self.data.join("descargas");
        std::fs::create_dir_all(&cache)?;
        let file = cache.join(format!("jre{major}.tar.xz"));
        http.download(
            asset.url,
            &file,
            &Checksum::Sha256(asset.sha256.into()),
            Some(asset.size),
            progress,
            cancel,
        )?;
        let tmp = dest.with_extension("nuevo");
        let _ = std::fs::remove_dir_all(&tmp);
        let r = merc_install::temurin::extract_tar_xz(&file, &tmp);
        let _ = std::fs::remove_file(&file);
        r?;
        if !(tmp.join("lib/libjli.so").is_file() && tmp.join("lib/server/libjvm.so").is_file()) {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(PhoneError::Io(
                "El Java descargado no trae libjli.so y libjvm.so.".into(),
            ));
        }
        let _ = std::fs::remove_dir_all(&dest);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&tmp, &dest)
            .map_err(|e| PhoneError::Io(format!("{} → {}: {e}", tmp.display(), dest.display())))?;
        Ok(dest)
    }

    /// Comprueba lo que se pide antes de descargar nada.
    pub fn check_create(&self, req: &CreateRequest) -> Result<(String, u32, u32), PhoneError> {
        let info = self.info();
        if let Some(r) = info.reason {
            return Err(PhoneError::Unsupported(r));
        }
        let missing = warnings::missing(&req.warnings_accepted);
        if !missing.is_empty() {
            return Err(PhoneError::Invalid(
                "Antes de crear un servidor en el celular tienes que aceptar todos los avisos.".into(),
            ));
        }
        if !flavor_allowed(req.flavor) {
            return Err(PhoneError::Unsupported(
                "En el celular se pueden crear Vanilla, Paper, Purpur o Fabric (y Java + Bedrock con Geyser). El servidor oficial de Bedrock solo existe para PC con Windows o Linux, y Forge, NeoForge y los demás necesitan ejecutar su instalador de Java, que aquí no se puede.".into(),
            ));
        }
        if req.bedrock && !matches!(req.flavor, Flavor::Paper | Flavor::Purpur) {
            return Err(PhoneError::Invalid(
                "Para que entren jugadores de Bedrock el servidor tiene que ser Paper o Purpur (Geyser va como plugin).".into(),
            ));
        }
        let name = clean_name(&req.name)?;
        let major = assets::jre_major_for(&req.mc_version).ok_or_else(|| {
            PhoneError::Unsupported(format!(
                "Minecraft {} necesita un Java que no está disponible en el celular (se ofrecen de la 1.18 a la 1.21; las 26.x piden Java 25, que todavía no hay verificado para Android).",
                req.mc_version
            ))
        })?;
        let heap = ram::clamp_heap(req.heap_mb, info.heap.as_ref().expect("comprobado arriba"));
        Ok((name, major, heap))
    }

    /// Crea el servidor: Java verificado + instalador verificado de escritorio.
    pub fn create(
        &self,
        http: &Http,
        src: &Sources,
        catalog: &dyn merc_content::modrinth::Source,
        req: &CreateRequest,
        progress: &mut dyn FnMut(&str, u64, Option<u64>),
        cancel: &AtomicBool,
    ) -> Result<LocalServer, PhoneError> {
        let (name, major, heap) = self.check_create(req)?;
        // Antes de descargar nada: Geyser y Floodgate tienen que existir para esta versión.
        let crossplay = if req.bedrock {
            progress("Buscando Geyser y Floodgate", 0, None);
            Some(crate::content::crossplay_versions(
                catalog,
                req.flavor,
                &req.mc_version,
            )?)
        } else {
            None
        };
        progress("Descargando Java para Android", 0, None);
        self.ensure_jre(
            http,
            major,
            &mut |d, t| progress("Descargando Java para Android", d, t),
            cancel,
        )?;
        let resolved = merc_install::resolve(http, src, req.flavor, &req.mc_version)?;
        if resolved.installer_args.is_some() || resolved.flavor.is_native() {
            return Err(PhoneError::Unsupported(
                "Este tipo necesita ejecutar un instalador y no se puede en el celular.".into(),
            ));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let dir = self.server_dir(&id);
        let mut properties = BTreeMap::new();
        properties.insert("motd".to_string(), name.clone());
        properties.insert("max-players".to_string(), "5".to_string());
        properties.insert("view-distance".to_string(), "6".to_string());
        properties.insert("simulation-distance".to_string(), "5".to_string());
        // Ver `ensure_android_properties`.
        properties.insert("use-native-transport".to_string(), "false".to_string());
        let plan = merc_install::InstallPlan {
            resolved,
            target: dir.clone(),
            java: PathBuf::new(),
            cache_dir: None,
            properties,
            accept_eula: req.accept_eula,
        };
        let mut on_event = |e: merc_install::InstallEvent| match e {
            merc_install::InstallEvent::Download { file, done, total } => progress(&file, done, total),
            merc_install::InstallEvent::Step(_, msg) => progress(&msg, 0, None),
            _ => {}
        };
        let out = merc_install::install(http, &plan, &mut on_event, cancel)?;
        let jar = match out.detected.launch {
            Some(merc_core::launch::LaunchTarget::Jar { jar }) => jar.to_string_lossy().into_owned(),
            _ => {
                let _ = std::fs::remove_dir_all(&dir);
                return Err(PhoneError::Io("No se encontró el .jar del servidor.".into()));
            }
        };
        if let Some(versions) = crossplay {
            progress("Instalando Geyser y Floodgate", 0, None);
            if let Err(e) = crate::content::install_crossplay(
                self,
                http,
                catalog,
                &dir,
                req.flavor,
                &req.mc_version,
                &versions,
                cancel,
            ) {
                // Servidor recién creado y vacío: se quita para no dejar uno a medias.
                let _ = std::fs::remove_dir_all(&dir);
                return Err(e);
            }
        }
        let s = LocalServer {
            id,
            name,
            flavor: req.flavor,
            mc_version: req.mc_version.clone(),
            java_major: major,
            heap_mb: heap,
            jar,
            created_unix_ms: now_ms(),
            playit: false,
            bedrock: req.bedrock,
        };
        let mut list = self.list();
        list.push(s.clone());
        self.save(&list)?;
        Ok(s)
    }

    /// Registra un servidor ya instalado en su carpeta (lo usan las pruebas y la recuperación).
    pub fn register(&self, s: LocalServer) -> Result<(), PhoneError> {
        let mut list = self.list();
        list.retain(|x| x.id != s.id);
        list.push(s);
        self.save(&list)
    }

    pub fn set_playit(&self, id: &str, on: bool) -> Result<LocalServer, PhoneError> {
        let mut list = self.list();
        let s = list
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| PhoneError::Invalid("No existe ese servidor en este celular.".into()))?;
        s.playit = on;
        let out = s.clone();
        self.save(&list)?;
        Ok(out)
    }

    pub fn set_heap(&self, id: &str, heap_mb: u32) -> Result<LocalServer, PhoneError> {
        let limits = ram::heap_limits(self.total_ram_mb)
            .ok_or_else(|| PhoneError::Unsupported("Este celular no tiene memoria suficiente.".into()))?;
        let mut list = self.list();
        let s = list
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| PhoneError::Invalid("No existe ese servidor en este celular.".into()))?;
        s.heap_mb = ram::clamp_heap(heap_mb, &limits);
        let out = s.clone();
        self.save(&list)?;
        Ok(out)
    }

    pub fn eula_accepted(&self, id: &str) -> bool {
        matches!(
            merc_core::properties::read_eula(&self.server_dir(id)),
            Ok(merc_core::properties::EulaStatus::Accepted)
        )
    }

    /// La persona marcó que acepta el EULA de Minecraft.
    pub fn accept_eula(&self, id: &str) -> Result<(), PhoneError> {
        self.get(id)?;
        let dir = self.server_dir(id);
        std::fs::create_dir_all(&dir)?;
        let tmp = dir.join("eula.txt.merchosting.tmp");
        std::fs::write(
            &tmp,
            format!(
                "# Aceptada por la persona desde MercHosting en el celular ({} ms Unix).\n\
                 # EULA de Minecraft: https://aka.ms/MinecraftEULA\n\
                 eula=true\n",
                now_ms()
            ),
        )?;
        std::fs::rename(&tmp, dir.join("eula.txt"))?;
        Ok(())
    }

    /// PID del servidor si sigue encendido: su proceso mantiene un candado sobre el archivo.
    #[cfg(unix)]
    pub(crate) fn pid(&self, id: &str) -> Option<u32> {
        crate::jli::pid_if_running(&self.run_dir(id).join("servidor.pid"))
    }

    #[cfg(not(unix))]
    pub(crate) fn pid(&self, _id: &str) -> Option<u32> {
        None
    }

    pub fn log_path(&self, id: &str) -> PathBuf {
        self.run_dir(id).join("consola.log")
    }

    /// Lo necesario para que el servicio en primer plano encienda el servidor.
    pub fn launch_spec(&self, id: &str) -> Result<LaunchSpec, PhoneError> {
        let s = self.get(id)?;
        if !self.eula_accepted(id) {
            return Err(PhoneError::Invalid(
                "Primero acepta el EULA de Minecraft (https://aka.ms/MinecraftEULA).".into(),
            ));
        }
        if self.pid(id).is_some() {
            return Err(PhoneError::Invalid("Ya está encendido.".into()));
        }
        if !self.jre_ready(s.java_major) {
            return Err(PhoneError::Invalid(format!(
                "Falta Java {} en este celular: vuelve a crearlo o descárgalo de nuevo.",
                s.java_major
            )));
        }
        let run = self.run_dir(id);
        std::fs::create_dir_all(&run)?;
        let work = self.server_dir(id);
        ensure_android_properties(&work)?;
        let tmp = work.join(".tmp");
        std::fs::create_dir_all(&tmp)?;
        let mut args = vec![
            "java".to_string(),
            format!("-Xms{}M", (s.heap_mb / 2).max(512)),
            format!("-Xmx{}M", s.heap_mb),
            // Android no tiene /tmp: sin esto la JVM intenta crear ahí sus datos de rendimiento.
            "-XX:-UsePerfData".to_string(),
            // Netty: solo la red estándar de Java (los transportes nativos son para Linux de PC).
            "-Dio.netty.transport.noNative=true".to_string(),
            // Si la JVM se cae, su informe queda junto a la consola y la app lo muestra.
            format!("-XX:ErrorFile={}", run.join("fallo_java_%p.log").display()),
            format!("-Djava.io.tmpdir={}", tmp.display()),
            format!("-Duser.home={}", work.display()),
            "-Dlog4j2.formatMsgNoLookups=true".to_string(),
        ];
        args.extend(merc_core::launch::ENCODING_ARGS.iter().map(|a| a.to_string()));
        // El Java para Android se identifica como «21.0.1-internal» y Paper (y Purpur, que es
        // un fork) se niega a arrancar con versiones que no son de disponibilidad general. Paper
        // documenta esta propiedad para saltarse ese chequeo (docs.papermc.io, «System
        // properties» › Paper.IgnoreJavaVersion).
        if matches!(s.flavor, Flavor::Paper | Flavor::Purpur) {
            args.push("-DPaper.IgnoreJavaVersion=true".to_string());
            // Paper comprime y cifra la conexión con las bibliotecas nativas de Velocity (hechas
            // para Linux de PC). En Android el jugador se quedaba en «Joining world» con Paper y
            // no con Vanilla: se usan las de Java, como Vanilla (docs.papermc.io, Velocity ›
            // System properties › velocity.natives-disabled).
            args.push("-Dvelocity.natives-disabled=true".to_string());
        }
        let agent = self.ensure_agent()?;
        args.push(format!(
            "-javaagent:{}={}",
            agent.display(),
            run.join("volcado").display()
        ));
        args.extend(["-jar".to_string(), s.jar.clone(), "nogui".to_string()]);
        Ok(LaunchSpec {
            jre: self.jre_dir(s.java_major),
            work,
            log: self.log_path(id),
            fifo: run.join("entrada"),
            pid_file: run.join("servidor.pid"),
            args,
            companion: match (s.playit, self.playit_exe()) {
                (true, Some(exe)) => Some(crate::jli::Companion {
                    exe,
                    args: self.playit_paths().args(),
                    cwd: self.playit_dir(),
                }),
                _ => None,
            },
        })
    }

    /// Últimas líneas de la consola (como mucho `max_bytes` del final).
    pub fn console(&self, id: &str, max_bytes: u64) -> Vec<String> {
        use std::io::{Read, Seek, SeekFrom};
        let Ok(mut f) = std::fs::File::open(self.log_path(id)) else {
            return vec![];
        };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        let start = len.saturating_sub(max_bytes);
        let _ = f.seek(SeekFrom::Start(start));
        let mut buf = String::new();
        let _ = f.take(max_bytes).read_to_string(&mut buf);
        let mut lines: Vec<String> = buf.lines().map(|l| l.to_string()).collect();
        if start > 0 && !lines.is_empty() {
            lines.remove(0);
        }
        lines.extend(self.java_crash_report(id));
        lines
    }

    /// Informes de fallo de la JVM (`fallo_java_<pid>.log`) de este encendido.
    fn crash_reports(&self, id: &str) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = std::fs::read_dir(self.run_dir(id))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("fallo_java_") && n.ends_with(".log"))
            })
            .collect();
        v.sort();
        v
    }

    /// Primeras líneas del informe de fallo de la JVM, para verlo en la consola.
    fn java_crash_report(&self, id: &str) -> Vec<String> {
        let Some(p) = self.crash_reports(id).pop() else {
            return vec![];
        };
        let text = std::fs::read_to_string(&p).unwrap_or_default();
        let mut out =
            vec!["[MercHosting] Java se cerró por un fallo interno. Primeras líneas del informe:".to_string()];
        out.extend(
            text.lines()
                .filter(|l| !l.trim().is_empty())
                .take(30)
                .map(|l| format!("[MercHosting] {l}")),
        );
        out
    }

    /// Copia el ayudante de diagnóstico a la carpeta de la app si falta o cambió.
    fn ensure_agent(&self) -> Result<PathBuf, PhoneError> {
        let dir = self.data.join("agente");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("merc-agent.jar");
        if std::fs::read(&path).ok().as_deref() != Some(AGENT_JAR) {
            let tmp = dir.join("merc-agent.jar.tmp");
            std::fs::write(&tmp, AGENT_JAR)?;
            std::fs::rename(&tmp, &path)?;
        }
        Ok(path)
    }

    /// Pide al ayudante que escriba en la consola qué hace cada hilo (tarda hasta un segundo).
    pub fn thread_dump(&self, id: &str) -> Result<(), PhoneError> {
        self.get(id)?;
        if self.pid(id).is_none() {
            return Err(PhoneError::Invalid("El servidor no está encendido.".into()));
        }
        std::fs::write(self.run_dir(id).join("volcado"), b"")?;
        Ok(())
    }

    /// Borra los informes de fallo de la vez anterior (se llama al encender).
    pub(crate) fn clear_crash_reports(&self, id: &str) {
        for p in self.crash_reports(id) {
            let _ = std::fs::remove_file(p);
        }
    }

    pub fn status(&self, id: &str) -> Result<LocalStatus, PhoneError> {
        self.get(id)?;
        let pid = self.pid(id);
        let lines = self.console(id, 256 * 1024);
        let mut state = LocalState::Starting;
        let mut problem = None;
        for l in &lines {
            match classify_line(l) {
                Some(LogSignal::Ready) => state = LocalState::Online,
                Some(LogSignal::StoppingServer) => state = LocalState::Stopping,
                Some(LogSignal::PortInUse) => {
                    problem = Some("El puerto del servidor está ocupado por otra app.".to_string())
                }
                Some(LogSignal::EulaNotAccepted) => problem = Some("Falta aceptar el EULA de Minecraft.".to_string()),
                _ => {}
            }
        }
        let state = if pid.is_none() { LocalState::Stopped } else { state };
        let port = std::fs::read_to_string(self.server_dir(id).join("server.properties"))
            .ok()
            .and_then(|t| {
                t.lines()
                    .find_map(|l| l.strip_prefix("server-port=").map(|v| v.trim().to_string()))
            })
            .and_then(|v| v.parse().ok())
            .or(Some(25565));
        Ok(LocalStatus {
            state,
            pid,
            eula_accepted: self.eula_accepted(id),
            port,
            problem,
            bedrock_port: crate::content::bedrock_port(&self.server_dir(id)),
        })
    }

    /// Manda un comando de Minecraft por la tubería del servidor.
    #[cfg(unix)]
    pub fn command(&self, id: &str, text: &str) -> Result<(), PhoneError> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let line = text.trim();
        if line.is_empty() || line.chars().any(|c| c.is_control()) || line.len() > 1_000 {
            return Err(PhoneError::Invalid("Comando no válido.".into()));
        }
        if self.pid(id).is_none() {
            return Err(PhoneError::Invalid("El servidor no está encendido.".into()));
        }
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(self.run_dir(id).join("entrada"))?;
        f.write_all(format!("{}\n", line.trim_start_matches('/')).as_bytes())?;
        Ok(())
    }

    /// Apagado limpio (guarda el mundo).
    #[cfg(unix)]
    pub fn stop(&self, id: &str) -> Result<(), PhoneError> {
        self.command(id, "stop")
    }

    /// Copia completa de la carpeta del servidor (con el servidor apagado, para que sea consistente).
    pub fn backup(&self, id: &str) -> Result<merc_admin::backups::BackupInfo, PhoneError> {
        let s = self.get(id)?;
        if self.pid(id).is_some() {
            return Err(PhoneError::Invalid("Apaga el servidor para hacer la copia.".into()));
        }
        let dir = self.backups_dir(id);
        std::fs::create_dir_all(&dir)?;
        let new = merc_admin::backups::NewBackup {
            scope: merc_admin::backups::BackupScope::Full,
            reason: merc_admin::backups::BackupReason::Manual,
            level_name: "world".into(),
            mc_version: Some(s.mc_version),
            hot: false,
            consistent: true,
            note: Some("Copia hecha en el celular".into()),
        };
        let progress = merc_admin::worlds::Progress::default();
        let info = merc_admin::backups::create(&self.server_dir(id), &dir, &new, &progress)
            .map_err(|e| PhoneError::Io(e.to_string()))?;
        merc_admin::backups::apply_retention(&dir, 5, &info.id);
        Ok(info)
    }

    pub fn backups(&self, id: &str) -> Vec<merc_admin::backups::BackupInfo> {
        merc_admin::backups::list(&self.backups_dir(id))
    }

    /// Quita el servidor de la lista. Su carpeta no se borra: se mueve a `papelera/` con una
    /// copia de seguridad hecha antes, para no perder el mundo sin querer.
    pub fn remove(&self, id: &str) -> Result<PathBuf, PhoneError> {
        self.get(id)?;
        if self.pid(id).is_some() {
            return Err(PhoneError::Invalid("Apaga el servidor antes de quitarlo.".into()));
        }
        self.backup(id)?;
        let trash = self.data.join("papelera");
        std::fs::create_dir_all(&trash)?;
        let dest = trash.join(format!("{id}-{}", now_ms()));
        std::fs::rename(self.server_dir(id), &dest)?;
        let mut list = self.list();
        list.retain(|s| s.id != id);
        self.save(&list)?;
        Ok(dest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phone(d: &Path) -> Phone {
        Phone::new(d.to_path_buf(), "aarch64", 8_192, None)
    }

    fn all_warnings() -> Vec<String> {
        warnings::REQUIRED.iter().map(|s| s.to_string()).collect()
    }

    fn req(flavor: Flavor, mc: &str) -> CreateRequest {
        CreateRequest {
            name: "Mi server".into(),
            flavor,
            mc_version: mc.into(),
            heap_mb: 99_999,
            accept_eula: true,
            warnings_accepted: all_warnings(),
            bedrock: false,
        }
    }

    #[test]
    fn agent_jar_has_its_premain() {
        use std::io::Read;
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(AGENT_JAR)).unwrap();
        let mut m = String::new();
        z.by_name("META-INF/MANIFEST.MF")
            .unwrap()
            .read_to_string(&mut m)
            .unwrap();
        assert!(m.contains("Premain-Class: app.merchosting.agent.MercAgent"), "{m}");
        assert!(z.by_name("app/merchosting/agent/MercAgent.class").is_ok());
    }

    #[test]
    fn native_transport_off_keeps_other_lines() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("server.properties"),
            "motd=Hola\nuse-native-transport=true\nserver-port=25565\n",
        )
        .unwrap();
        ensure_android_properties(d.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(d.path().join("server.properties")).unwrap(),
            "motd=Hola\nuse-native-transport=false\nserver-port=25565\n"
        );
        let e = tempfile::tempdir().unwrap();
        ensure_android_properties(e.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(e.path().join("server.properties")).unwrap(),
            "use-native-transport=false\n"
        );
    }

    #[test]
    fn info_reports_why_not() {
        let d = tempfile::tempdir().unwrap();
        let p = Phone::new(d.path().into(), "x86_64", 8_192, None);
        let i = p.info();
        assert!(!i.supported && i.reason.unwrap().contains("arm64"));
        let p = Phone::new(d.path().into(), "aarch64", 2_048, None);
        assert!(p.info().reason.unwrap().contains("memoria"));
        let i = phone(d.path()).info();
        assert!(i.supported && i.jre_installed.is_empty() && !i.playit_available);
        assert_eq!(i.warnings.len(), warnings::REQUIRED.len());
    }

    #[test]
    fn create_checks_before_downloading() {
        let d = tempfile::tempdir().unwrap();
        let p = phone(d.path());
        let mut r = req(Flavor::Paper, "1.21.1");
        r.warnings_accepted.pop();
        assert!(matches!(p.check_create(&r), Err(PhoneError::Invalid(_))));
        assert!(
            matches!(p.check_create(&req(Flavor::Bedrock, "1.21.1")), Err(PhoneError::Unsupported(m)) if m.contains("Bedrock"))
        );
        assert!(matches!(
            p.check_create(&req(Flavor::Forge, "1.20.1")),
            Err(PhoneError::Unsupported(_))
        ));
        assert!(matches!(
            p.check_create(&req(Flavor::Paper, "1.16.5")),
            Err(PhoneError::Unsupported(_))
        ));
        let (name, major, heap) = p.check_create(&req(Flavor::Fabric, "1.21.1")).unwrap();
        assert_eq!((name.as_str(), major, heap), ("Mi server", 21, 3_584));
        assert_eq!(p.check_create(&req(Flavor::Vanilla, "1.20.1")).unwrap().1, 17);
    }

    #[test]
    fn launch_spec_needs_eula_and_jre_and_builds_args() {
        let d = tempfile::tempdir().unwrap();
        let p = phone(d.path());
        let s = LocalServer {
            id: "abc".into(),
            name: "S".into(),
            flavor: Flavor::Paper,
            mc_version: "1.21.1".into(),
            java_major: 21,
            heap_mb: 2_048,
            jar: "paper.jar".into(),
            created_unix_ms: 0,
            playit: false,
            bedrock: false,
        };
        p.register(s).unwrap();
        std::fs::create_dir_all(p.server_dir("abc")).unwrap();
        assert!(p.launch_spec("abc").unwrap_err().to_string().contains("EULA"));
        p.accept_eula("abc").unwrap();
        assert!(p.launch_spec("abc").unwrap_err().to_string().contains("Java 21"));
        std::fs::create_dir_all(p.jre_dir(21).join("lib/server")).unwrap();
        std::fs::write(p.jre_dir(21).join("lib/libjli.so"), b"").unwrap();
        std::fs::write(p.jre_dir(21).join("lib/server/libjvm.so"), b"").unwrap();
        let spec = p.launch_spec("abc").unwrap();
        assert_eq!(spec.args[0], "java");
        assert!(spec.args.contains(&"-Xmx2048M".to_string()));
        assert!(spec.args.contains(&"-Xms1024M".to_string()));
        assert_eq!(&spec.args[spec.args.len() - 3..], ["-jar", "paper.jar", "nogui"]);
        assert_eq!(spec.work, p.server_dir("abc"));
        assert_eq!(spec.jre, p.jre_dir(21));
        assert!(spec.companion.is_none());
        assert!(spec.args.contains(&"-XX:-UsePerfData".to_string()));
        let ja = spec
            .args
            .iter()
            .find(|a| a.starts_with("-javaagent:"))
            .expect("ayudante");
        assert!(ja.ends_with("volcado"), "{ja}");
        let jar_at = spec.args.iter().position(|a| a == "-jar").unwrap();
        assert!(
            spec.args.iter().position(|a| a == ja).unwrap() < jar_at,
            "antes de -jar"
        );
        assert_eq!(
            std::fs::read(d.path().join("agente/merc-agent.jar")).unwrap(),
            AGENT_JAR
        );
        assert!(p.thread_dump("abc").is_err(), "apagado");
        assert!(spec.args.contains(&"-Dio.netty.transport.noNative=true".to_string()));
        let props = std::fs::read_to_string(p.server_dir("abc").join("server.properties")).unwrap();
        assert!(props.lines().any(|l| l == "use-native-transport=false"), "{props}");
        assert!(
            spec.args.contains(&"-DPaper.IgnoreJavaVersion=true".to_string()),
            "Paper con el Java «internal» de Android"
        );
        assert!(spec.args.contains(&"-Dvelocity.natives-disabled=true".to_string()));
        assert!(spec
            .args
            .iter()
            .any(|a| a.starts_with("-XX:ErrorFile=") && a.ends_with("fallo_java_%p.log")));
        // Si la JVM se cae, su informe aparece al final de la consola y se borra al encender.
        std::fs::write(spec.log.clone(), "[Server thread/INFO]: Starting\n").unwrap();
        std::fs::write(
            p.run_dir("abc").join("fallo_java_77.log"),
            "#\n# A fatal error has been detected\n# SIGSEGV\n",
        )
        .unwrap();
        let con = p.console("abc", 64 * 1024);
        assert_eq!(con[0], "[Server thread/INFO]: Starting");
        assert!(con.iter().any(|l| l.contains("fallo interno")));
        assert!(con.iter().any(|l| l.contains("SIGSEGV")));
        p.clear_crash_reports("abc");
        assert_eq!(p.console("abc", 64 * 1024).len(), 1);
        // Con Playit activado y el agente dentro de la app, va como acompañante.
        let native = d.path().join("lib");
        std::fs::create_dir_all(&native).unwrap();
        std::fs::write(native.join(assets::PLAYIT_LIB_NAME), b"").unwrap();
        let p = Phone::new(d.path().into(), "aarch64", 8_192, Some(native.clone()));
        assert!(p.info().playit_available);
        p.set_playit("abc", true).unwrap();
        let c = p.launch_spec("abc").unwrap().companion.unwrap();
        assert_eq!(c.exe, native.join(assets::PLAYIT_LIB_NAME));
        assert_eq!(c.args[0], "--secret-path");
        let st = p.status("abc").unwrap();
        assert_eq!(st.state, LocalState::Stopped);
        assert!(st.eula_accepted);
        assert_eq!(st.port, Some(25565));
    }

    #[test]
    fn status_follows_pid_and_console() {
        let d = tempfile::tempdir().unwrap();
        let p = phone(d.path());
        p.register(LocalServer {
            id: "s1".into(),
            name: "S".into(),
            flavor: Flavor::Vanilla,
            mc_version: "1.21.1".into(),
            java_major: 21,
            heap_mb: 1_024,
            jar: "server.jar".into(),
            created_unix_ms: 0,
            playit: false,
            bedrock: false,
        })
        .unwrap();
        let run = d.path().join("ejecucion/s1");
        std::fs::create_dir_all(&run).unwrap();
        // Encendido = alguien tiene el candado del archivo (aquí, esta prueba).
        let lock = crate::jli::lock_pid_file(&run.join("servidor.pid")).unwrap();
        std::fs::write(
            run.join("consola.log"),
            "[x] [Server thread/INFO]: Starting minecraft server version 1.21.1\n",
        )
        .unwrap();
        assert_eq!(p.status("s1").unwrap().state, LocalState::Starting);
        std::fs::write(
            run.join("consola.log"),
            "[x] [Server thread/INFO]: Done (3.2s)! For help, type \"help\"\n",
        )
        .unwrap();
        assert_eq!(p.status("s1").unwrap().state, LocalState::Online);
        assert_eq!(p.console("s1", 1024).len(), 1);
        // Proceso muerto = candado suelto, aunque el archivo siga ahí.
        drop(lock);
        assert!(run.join("servidor.pid").exists());
        assert_eq!(p.status("s1").unwrap().state, LocalState::Stopped);
    }

    #[cfg(unix)]
    #[test]
    fn commands_go_through_the_pipe() {
        let d = tempfile::tempdir().unwrap();
        let p = phone(d.path());
        p.register(LocalServer {
            id: "s2".into(),
            name: "S".into(),
            flavor: Flavor::Vanilla,
            mc_version: "1.21.1".into(),
            java_major: 21,
            heap_mb: 1_024,
            jar: "server.jar".into(),
            created_unix_ms: 0,
            playit: false,
            bedrock: false,
        })
        .unwrap();
        assert!(p.command("s2", "say hola").is_err(), "apagado");
        let run = d.path().join("ejecucion/s2");
        std::fs::create_dir_all(&run).unwrap();
        let _lock = crate::jli::lock_pid_file(&run.join("servidor.pid")).unwrap();
        p.thread_dump("s2").unwrap();
        assert!(run.join("volcado").is_file());
        let fifo = std::ffi::CString::new(run.join("entrada").to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        // Lector abierto (como el servidor).
        use std::os::unix::fs::OpenOptionsExt;
        let mut r = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(run.join("entrada"))
            .unwrap();
        p.command("s2", "/say hola").unwrap();
        let mut got = String::new();
        std::io::Read::read_to_string(&mut r, &mut got).ok();
        assert_eq!(got, "say hola\n");
        assert!(p.command("s2", "a\nb").is_err());
    }

    #[test]
    fn remove_keeps_a_backup_and_the_folder() {
        let d = tempfile::tempdir().unwrap();
        let p = phone(d.path());
        p.register(LocalServer {
            id: "s3".into(),
            name: "S".into(),
            flavor: Flavor::Vanilla,
            mc_version: "1.21.1".into(),
            java_major: 21,
            heap_mb: 1_024,
            jar: "server.jar".into(),
            created_unix_ms: 0,
            playit: false,
            bedrock: false,
        })
        .unwrap();
        let dir = p.server_dir("s3");
        std::fs::create_dir_all(dir.join("world")).unwrap();
        std::fs::write(dir.join("world/level.dat"), b"mundo").unwrap();
        std::fs::write(dir.join("server.properties"), "level-name=world\n").unwrap();
        let moved = p.remove("s3").unwrap();
        assert!(moved.join("world/level.dat").is_file());
        assert_eq!(p.backups("s3").len(), 1);
        assert!(p.list().is_empty());
    }

    /// De punta a punta en Linux: instala un Vanilla de verdad con el instalador verificado,
    /// lo enciende con el lanzador del celular (dlopen + JLI_Launch en un proceso aparte, la
    /// misma ruta que usa el servicio de Android), espera «Done», manda `stop` por la tubería y
    /// comprueba que se apaga. Necesita internet y el JRE 21 del sistema:
    /// `cargo test -p merc-phone -- --ignored end_to_end`.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore]
    fn end_to_end_vanilla_on_linux() {
        let jre = Path::new("/usr/lib/jvm/java-21-openjdk-amd64");
        assert!(jre.join("lib/libjli.so").is_file(), "falta el JRE 21 del sistema");
        let d = tempfile::tempdir().unwrap();
        let p = phone(d.path());
        std::fs::create_dir_all(d.path().join("jre")).unwrap();
        std::os::unix::fs::symlink(jre, p.jre_dir(21)).unwrap();
        let http = Http::new();
        let src = Sources::default();
        let mut r = req(Flavor::Vanilla, "1.21.1");
        r.heap_mb = 1_024;
        let s = p
            .create(
                &http,
                &src,
                &merc_content::modrinth::Modrinth::new(&http, merc_content::modrinth::MODRINTH_API),
                &r,
                &mut |_, _, _| {},
                &AtomicBool::new(false),
            )
            .unwrap();
        assert_eq!(s.java_major, 21);
        assert!(p.eula_accepted(&s.id));
        // Puerto libre para no chocar con nada.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let props = p.server_dir(&s.id).join("server.properties");
        let text = std::fs::read_to_string(&props).unwrap_or_default();
        std::fs::write(&props, format!("{text}\nserver-port={port}\nonline-mode=false\n")).unwrap();
        let spec = p.launch_spec(&s.id).unwrap();
        let pid = unsafe { libc::fork() };
        if pid == 0 {
            let code = crate::jli::launch_in_process(&spec).unwrap_or(99);
            unsafe { libc::_exit(code) };
        }
        let start = std::time::Instant::now();
        loop {
            let st = p.status(&s.id).unwrap();
            if st.state == LocalState::Online {
                break;
            }
            assert!(
                start.elapsed().as_secs() < 240,
                "no arrancó: {:?}",
                p.console(&s.id, 4096)
            );
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        assert_eq!(p.status(&s.id).unwrap().port, Some(port));
        p.command(&s.id, "say hola desde el celular").unwrap();
        p.stop(&s.id).unwrap();
        let mut status = 0;
        unsafe { libc::waitpid(pid, &mut status, 0) };
        assert_eq!(p.status(&s.id).unwrap().state, LocalState::Stopped);
        let log = p.console(&s.id, 64 * 1024).join("\n");
        assert!(log.contains("hola desde el celular"), "{log}");
        assert!(p.server_dir(&s.id).join("world/level.dat").is_file());
        assert!(!p.backup(&s.id).unwrap().id.is_empty());
    }

    /// Lo mismo sin internet: un «servidor» Java mínimo que imprime la línea «Done» de Minecraft,
    /// lee comandos de la entrada y se apaga con `stop`. Comprueba el lanzador, la tubería de
    /// comandos, la consola y el estado. Se salta si no hay JDK 21 en el sistema.
    #[cfg(target_os = "linux")]
    #[test]
    fn fake_server_runs_and_stops() {
        let jdk = Path::new("/usr/lib/jvm/java-21-openjdk-amd64");
        if !jdk.join("bin/javac").is_file() || !jdk.join("lib/libjli.so").is_file() {
            eprintln!("sin JDK 21: se salta");
            return;
        }
        let d = tempfile::tempdir().unwrap();
        let p = phone(d.path());
        p.register(LocalServer {
            id: "f1".into(),
            name: "Falso".into(),
            flavor: Flavor::Vanilla,
            mc_version: "1.21.1".into(),
            java_major: 21,
            heap_mb: 1_024,
            jar: "falso.jar".into(),
            created_unix_ms: 0,
            playit: false,
            bedrock: false,
        })
        .unwrap();
        let dir = p.server_dir("f1");
        let build = d.path().join("build");
        std::fs::create_dir_all(&build).unwrap();
        std::fs::write(
            build.join("Falso.java"),
            r#"import java.io.*;
public class Falso {
  public static void main(String[] a) throws Exception {
    System.out.println("[00:00:00] [Server thread/INFO]: Starting minecraft server version 1.21.1");
    new File("world").mkdirs();
    try (FileOutputStream f = new FileOutputStream("world/level.dat")) { f.write(1); }
    System.out.println("[00:00:01] [Server thread/INFO]: Done (1.000s)! For help, type \"help\"");
    System.out.flush();
    BufferedReader in = new BufferedReader(new InputStreamReader(System.in));
    String l;
    while ((l = in.readLine()) != null) {
      System.out.println("[00:00:02] [Server thread/INFO]: comando: " + l);
      System.out.flush();
      if (l.equals("stop")) { System.out.println("[00:00:03] [Server thread/INFO]: Stopping server"); System.out.flush(); System.exit(0); }
    }
  }
}
"#,
        )
        .unwrap();
        let ok = std::process::Command::new(jdk.join("bin/javac"))
            .arg("-d")
            .arg(&build)
            .arg(build.join("Falso.java"))
            .status()
            .unwrap()
            .success();
        assert!(ok);
        std::fs::write(build.join("m.txt"), "Main-Class: Falso\n").unwrap();
        let ok = std::process::Command::new(jdk.join("bin/jar"))
            .current_dir(&build)
            .arg("cfm")
            .arg(dir.join("falso.jar"))
            .arg("m.txt")
            .arg("Falso.class")
            .status()
            .unwrap()
            .success();
        assert!(ok);
        p.accept_eula("f1").unwrap();
        std::fs::create_dir_all(d.path().join("jre")).unwrap();
        std::os::unix::fs::symlink(jdk, p.jre_dir(21)).unwrap();
        let spec = p.launch_spec("f1").unwrap();
        let pid = unsafe { libc::fork() };
        if pid == 0 {
            let code = crate::jli::launch_in_process(&spec).unwrap_or(99);
            unsafe { libc::_exit(code) };
        }
        let start = std::time::Instant::now();
        while p.status("f1").unwrap().state != LocalState::Online {
            assert!(
                start.elapsed().as_secs() < 60,
                "no arrancó: {:?}",
                p.console("f1", 4096)
            );
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        assert!(p.launch_spec("f1").unwrap_err().to_string().contains("encendido"));
        assert!(p.backup("f1").is_err(), "copia con el servidor encendido");
        p.command("f1", "/say hola").unwrap();
        p.stop("f1").unwrap();
        let mut status = 0;
        unsafe { libc::waitpid(pid, &mut status, 0) };
        assert!(libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0, "{status}");
        assert_eq!(p.status("f1").unwrap().state, LocalState::Stopped);
        let log = p.console("f1", 64 * 1024).join("\n");
        assert!(
            log.contains("comando: say hola") && log.contains("comando: stop"),
            "{log}"
        );
        assert!(dir.join("world/level.dat").is_file());
        assert_eq!(p.backups("f1").len(), 0);
        p.backup("f1").unwrap();
        assert_eq!(p.backups("f1").len(), 1);
    }
}
