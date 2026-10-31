//! `merc`: herramienta de DESARROLLO para hablar con el supervisor.
//!
//! No es la interfaz para usuarios finales (esa será la aplicación de escritorio). Sirve para
//! probar el supervisor con servidores reales mientras se construye la interfaz.

use merc_core::InstanceId;
use merc_ipc::*;
use merc_supervisor::ConsoleStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const AYUDA: &str = "\
merc — herramienta de desarrollo de MercHosting

Uso: merc [--data-dir <carpeta>] <orden> [opciones]

Órdenes:
  ping                                 Comprueba el supervisor (lo arranca si hace falta)
  java                                 Lista las instalaciones de Java encontradas
  detectar <carpeta>                   Analiza una carpeta de servidor sin modificarla
  importar <carpeta> [--nombre N] [--java RUTA] [--ram MB]
                                       Registra un servidor existente (no mueve ni cambia archivos)
  versiones <tipo>                     Versiones publicadas (vanilla, paper, purpur, spigot, fabric,
                                       quilt, forge, neoforge, arclight)
  crear <tipo> <versión> [--nombre N] [--carpeta RUTA] [--java RUTA] [--ram MB] [--puerto P]
        [--acepto-eula]                Descarga e instala un servidor nuevo desde la fuente oficial.
                                       --acepto-eula: aceptas la EULA de Minecraft
                                       (https://aka.ms/MinecraftEULA); si no, se pedirá después.
  lista                                Servidores registrados y su estado
  estado <servidor>
  iniciar <servidor>
  esperar <servidor> <ESTADO> [--max SEG]   Espera a un estado (ONLINE, STOPPED…)
  consola <servidor> [--seguir] [--ultimas N]
  comando <servidor> <texto…>          Envía un comando de Minecraft (no del sistema)
  detener <servidor> [--espera SEG]
  reiniciar <servidor>
  forzar-cierre <servidor> --confirmo  Cierra sin guardar (puede perder datos)
  quitar <servidor>                    Quita de la lista (no borra archivos)
  apagar-supervisor                    Cierra el supervisor si no hay servidores en marcha

<servidor> puede ser el nombre o el comienzo del identificador.";

fn fail(msg: impl std::fmt::Display) -> ! {
    eprintln!("Error: {msg}");
    std::process::exit(1);
}

fn connect(data_dir: &Path) -> Client {
    // El supervisor se busca junto a `merc` (o en MERC_SUPERVISORD_PATH).
    merc_ipc::launcher::connect_or_spawn(data_dir, "merc-cli", Duration::from_secs(15)).unwrap_or_else(|e| fail(e))
}

fn call(c: &mut Client, body: RequestBody) -> ResponseBody {
    c.call(body).unwrap_or_else(|e| fail(e))
}

fn resolve(c: &mut Client, query: &str) -> InstanceId {
    let ResponseBody::Instances(list) = call(c, RequestBody::ListInstances) else {
        fail("respuesta inesperada")
    };
    let q = query.to_lowercase();
    let matches: Vec<_> = list
        .iter()
        .filter(|s| s.instance.name.to_lowercase() == q || s.instance.id.to_string().starts_with(&q))
        .collect();
    match matches.as_slice() {
        [one] => one.instance.id,
        [] => fail(format!("no hay ningún servidor llamado «{query}»")),
        _ => fail(format!(
            "«{query}» coincide con varios servidores; usa el identificador"
        )),
    }
}

fn print_status(s: &merc_supervisor::StatusSnapshot) {
    println!("Estado: {:?} ({})", s.state, s.state.label_es());
    if let Some(pid) = s.pid {
        println!("PID: {pid}{}", if s.has_console { "" } else { " (sin consola)" });
    }
    if let Some(p) = s.port {
        println!("Puerto: {p}");
    }
    if let Some(e) = &s.last_exit {
        println!(
            "Última salida: código {}{}",
            e.code.map(|c| c.to_string()).unwrap_or_else(|| "desconocido".into()),
            if e.forced {
                " (forzada)"
            } else if e.requested {
                " (pedida)"
            } else {
                ""
            }
        );
    }
    if let Some(h) = &s.hint {
        println!("Nota: {h}");
    }
    if let Some(l) = &s.log_file {
        println!("Log: {}", l.display());
    }
}

fn print_lines(lines: &[merc_supervisor::ConsoleLine]) {
    for l in lines {
        match l.stream {
            ConsoleStream::Stdout => println!("{}", l.text),
            ConsoleStream::Stderr => println!("[err] {}", l.text),
            ConsoleStream::System => println!("[MercHosting] {}", l.text),
        }
    }
}

fn opt(args: &mut Vec<String>, flag: &str) -> Option<String> {
    let i = args.iter().position(|a| a == flag)?;
    if i + 1 >= args.len() {
        fail(format!("falta el valor de {flag}"));
    }
    let v = args.remove(i + 1);
    args.remove(i);
    Some(v)
}

fn flag(args: &mut Vec<String>, flag: &str) -> bool {
    match args.iter().position(|a| a == flag) {
        Some(i) => {
            args.remove(i);
            true
        }
        None => false,
    }
}

fn parse_state(s: &str) -> merc_core::RuntimeState {
    serde_json::from_value(serde_json::Value::String(s.to_uppercase()))
        .unwrap_or_else(|_| fail(format!("estado desconocido: {s}")))
}

fn parse_flavor(s: &str) -> merc_install::Flavor {
    use merc_install::Flavor;
    match s.to_lowercase().replace(['-', '_'], "").as_str() {
        "vanilla" => Flavor::Vanilla,
        "paper" => Flavor::Paper,
        "purpur" => Flavor::Purpur,
        "spigot" => Flavor::Spigot,
        "quilt" => Flavor::Quilt,
        "forge" => Flavor::Forge,
        "arclight" => Flavor::Arclight,
        "fabric" => Flavor::Fabric,
        "neoforge" => Flavor::NeoForge,
        _ => fail(format!(
            "tipo desconocido: {s} (usa vanilla, paper, purpur, spigot, fabric, quilt, forge, neoforge o arclight)"
        )),
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let data_dir: PathBuf = opt(&mut args, "--data-dir")
        .map(PathBuf::from)
        .unwrap_or_else(default_data_dir);
    if args.is_empty() || flag(&mut args, "--help") || flag(&mut args, "-h") {
        println!("{AYUDA}");
        return;
    }
    let cmd = args.remove(0);
    let need = |args: &Vec<String>, n: usize| {
        if args.len() < n {
            fail("faltan argumentos. Ejecuta «merc --help»");
        }
    };

    if cmd == "apagar-supervisor" {
        match Client::connect(&data_dir, "merc-cli") {
            Ok(mut c) => {
                call(&mut c, RequestBody::Shutdown);
                println!("Supervisor cerrado.");
            }
            Err(_) => println!("El supervisor no estaba en ejecución."),
        }
        return;
    }

    let mut c = connect(&data_dir);
    match cmd.as_str() {
        "ping" => {
            if let ResponseBody::Pong {
                server_version,
                supervisor_pid,
            } = call(&mut c, RequestBody::Ping)
            {
                println!("Supervisor {server_version} activo (pid {supervisor_pid}).");
                println!("Datos: {}", data_dir.display());
            }
        }
        "java" => {
            if let ResponseBody::JavaCandidates(list) = call(&mut c, RequestBody::FindJava) {
                if list.is_empty() {
                    println!("No se encontró Java. Instala un JDK 21 (por ejemplo, Eclipse Temurin).");
                }
                for j in list {
                    match j.major {
                        Some(m) => println!("Java {m:>2}  {}", j.path.display()),
                        None => println!("??       {}  ({})", j.path.display(), j.error.unwrap_or_default()),
                    }
                }
            }
        }
        "detectar" => {
            need(&args, 1);
            let path = std::fs::canonicalize(&args[0]).unwrap_or_else(|e| fail(e));
            if let ResponseBody::Detected(d) = call(&mut c, RequestBody::Detect { path }) {
                println!("Tipo: {:?}  (confianza: {:?})", d.kind, d.confidence);
                println!("Minecraft: {}", d.mc_version.as_deref().unwrap_or("desconocido"));
                println!("Loader: {}", d.loader_version.as_deref().unwrap_or("-"));
                for e in d.evidence {
                    println!("  · {e}");
                }
                for w in d.warnings {
                    println!("  ! {w}");
                }
            }
        }
        "importar" => {
            let nombre = opt(&mut args, "--nombre");
            let java = opt(&mut args, "--java");
            let ram: u32 = opt(&mut args, "--ram")
                .map(|r| r.parse().unwrap_or_else(|_| fail("--ram debe ser un número en MB")))
                .unwrap_or(4096);
            need(&args, 1);
            let path = std::fs::canonicalize(&args[0]).unwrap_or_else(|e| fail(e));
            let path = merc_core::paths::strip_verbatim(&path);
            let name = nombre.unwrap_or_else(|| {
                path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or("Servidor".into())
            });
            let java = match java {
                Some(j) => PathBuf::from(j),
                None => {
                    let ResponseBody::Detected(det) = call(&mut c, RequestBody::Detect { path: path.clone() }) else {
                        fail("respuesta inesperada")
                    };
                    let ResponseBody::JavaCandidates(list) = call(&mut c, RequestBody::FindJava) else {
                        fail("respuesta inesperada")
                    };
                    let cands: Vec<(PathBuf, u32)> =
                        list.into_iter().filter_map(|j| j.major.map(|m| (j.path, m))).collect();
                    let mc = det.mc_version.as_deref();
                    match merc_core::java::pick_java(&cands, mc) {
                        Some(p) => p.clone(),
                        None => fail(format!(
                            "no hay un Java compatible instalado (se recomienda Java {}); usa --java <ruta a java.exe>",
                            merc_core::java::recommended_java_major(mc)
                        )),
                    }
                }
            };
            println!("Java: {}", java.display());
            let body = RequestBody::ImportInstance {
                name,
                path,
                java,
                min_memory_mb: 0,
                max_memory_mb: ram,
            };
            if let ResponseBody::Instance(i) = call(&mut c, body) {
                println!(
                    "Importado «{}» ({:?} {}) con id {}",
                    i.name,
                    i.kind,
                    i.mc_version.as_deref().unwrap_or("?"),
                    i.id
                );
            }
        }
        "versiones" => {
            need(&args, 1);
            if let ResponseBody::Catalog(list) = call(
                &mut c,
                RequestBody::Catalog {
                    flavor: parse_flavor(&args[0]),
                },
            ) {
                for e in list.iter().take(40) {
                    println!(
                        "{:<10} Java {:>2}{}",
                        e.mc_version,
                        e.java_major,
                        if e.latest { "  ← última" } else { "" }
                    );
                }
                if list.len() > 40 {
                    println!("… y {} más", list.len() - 40);
                }
            }
        }
        "crear" => {
            let nombre = opt(&mut args, "--nombre");
            let carpeta = opt(&mut args, "--carpeta");
            let java = opt(&mut args, "--java");
            let ram: u32 = opt(&mut args, "--ram")
                .map(|r| r.parse().unwrap_or_else(|_| fail("--ram debe ser un número en MB")))
                .unwrap_or(4096);
            let puerto = opt(&mut args, "--puerto");
            let accept_eula = flag(&mut args, "--acepto-eula");
            need(&args, 2);
            let flavor = parse_flavor(&args[0]);
            let mc_version = args[1].clone();
            let ResponseBody::Resolved(r) = call(
                &mut c,
                RequestBody::Resolve {
                    flavor,
                    mc_version: mc_version.clone(),
                    loader_version: None,
                },
            ) else {
                fail("respuesta inesperada")
            };
            println!(
                "{:?} {} {} · Java {} · huella {}",
                r.flavor,
                r.mc_version,
                r.loader_version.as_deref().unwrap_or(""),
                r.java_major,
                r.artifact.checksum.algorithm()
            );
            for n in &r.notes {
                println!("  ⓘ {n}");
            }
            let ResponseBody::CreationDefaults {
                servers_dir,
                suggested_port,
                ..
            } = call(&mut c, RequestBody::CreationDefaults)
            else {
                fail("respuesta inesperada")
            };
            let name = nombre.unwrap_or_else(|| format!("{:?} {}", flavor, mc_version));
            let path = carpeta
                .map(PathBuf::from)
                .unwrap_or_else(|| servers_dir.join(name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "-")));
            let java = match java {
                Some(j) => PathBuf::from(j),
                None => {
                    let ResponseBody::JavaCandidates(list) = call(&mut c, RequestBody::FindJava) else {
                        fail("respuesta inesperada")
                    };
                    let cands: Vec<(PathBuf, u32)> =
                        list.into_iter().filter_map(|j| j.major.map(|m| (j.path, m))).collect();
                    match merc_core::java::pick_java(&cands, Some(&r.mc_version)) {
                        Some(p) => p.clone(),
                        None => fail(format!(
                            "no hay un Java compatible instalado (esta versión pide Java {}); usa --java <ruta>",
                            r.java_major
                        )),
                    }
                }
            };
            let mut properties = std::collections::BTreeMap::new();
            properties.insert(
                "server-port".to_string(),
                puerto.unwrap_or_else(|| suggested_port.to_string()),
            );
            println!("Carpeta: {}\nJava: {}", path.display(), java.display());
            let ResponseBody::Job(job) = call(
                &mut c,
                RequestBody::CreateServer {
                    name,
                    flavor,
                    mc_version,
                    path,
                    java,
                    min_memory_mb: 0,
                    max_memory_mb: ram,
                    properties,
                    accept_eula,
                    modpack_version: None,
                    modpack_source: ContentSource::Modrinth,
                    loader_version: None,
                    game_rules: Default::default(),
                },
            ) else {
                fail("respuesta inesperada")
            };
            let mut after = 0;
            let mut last_pct = None;
            loop {
                let ResponseBody::Job(j) = call(
                    &mut c,
                    RequestBody::Job {
                        job_id: job.job_id.clone(),
                        after,
                    },
                ) else {
                    fail("respuesta inesperada")
                };
                for l in &j.log {
                    println!("{}", l.text);
                }
                after = j.next_after;
                if let Some(d) = &j.download {
                    if let Some(t) = d.total.filter(|t| *t > 0) {
                        let pct = d.done * 100 / t;
                        if last_pct != Some(pct / 10) {
                            last_pct = Some(pct / 10);
                            println!("  {} {pct}%", d.file);
                        }
                    }
                }
                match j.state {
                    JobState::Running => std::thread::sleep(Duration::from_millis(300)),
                    JobState::Succeeded => {
                        println!(
                            "Listo. Id del servidor: {}",
                            j.instance_id.map(|i| i.to_string()).unwrap_or_default()
                        );
                        if !accept_eula {
                            println!("Antes de iniciarlo debes aceptar la EULA de Minecraft desde la aplicación.");
                        }
                        break;
                    }
                    _ => fail(j.error.unwrap_or_else(|| j.message.clone())),
                }
            }
        }
        "lista" => {
            if let ResponseBody::Instances(list) = call(&mut c, RequestBody::ListInstances) {
                if list.is_empty() {
                    println!("No hay servidores registrados.");
                }
                for s in list {
                    let id = s.instance.id.to_string();
                    println!(
                        "{}  {:<24} {:<10} {:<8} {}",
                        &id[..8],
                        s.instance.name,
                        format!("{:?}", s.status.state),
                        s.instance.mc_version.as_deref().unwrap_or("?"),
                        s.instance.root.display()
                    );
                }
            }
        }
        "estado" => {
            need(&args, 1);
            let id = resolve(&mut c, &args[0]);
            if let ResponseBody::Status(s) = call(&mut c, RequestBody::Status { id }) {
                print_status(&s);
            }
        }
        "iniciar" => {
            need(&args, 1);
            let id = resolve(&mut c, &args[0]);
            if let ResponseBody::Status(s) = call(&mut c, RequestBody::Start { id }) {
                print_status(&s);
            }
        }
        "esperar" => {
            need(&args, 2);
            let max: u64 = opt(&mut args, "--max").and_then(|m| m.parse().ok()).unwrap_or(900);
            let id = resolve(&mut c, &args[0]);
            let want = parse_state(&args[1]);
            let deadline = Instant::now() + Duration::from_secs(max);
            loop {
                let ResponseBody::Status(s) = call(&mut c, RequestBody::Status { id }) else {
                    fail("respuesta inesperada")
                };
                if s.state == want {
                    println!("Estado alcanzado: {:?}", s.state);
                    return;
                }
                if matches!(
                    s.state,
                    merc_core::RuntimeState::Crashed | merc_core::RuntimeState::Error
                ) && want != s.state
                {
                    print_status(&s);
                    fail("el servidor no llegó al estado esperado");
                }
                if Instant::now() >= deadline {
                    print_status(&s);
                    fail("tiempo de espera agotado");
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        }
        "consola" => {
            need(&args, 1);
            let seguir = flag(&mut args, "--seguir");
            let ultimas: usize = opt(&mut args, "--ultimas").and_then(|n| n.parse().ok()).unwrap_or(60);
            let id = resolve(&mut c, &args[0]);
            let mut after = 0;
            let mut all = vec![];
            loop {
                let ResponseBody::Console { lines, next_after } =
                    call(&mut c, RequestBody::Console { id, after, limit: 2000 })
                else {
                    fail("respuesta inesperada")
                };
                if lines.is_empty() {
                    break;
                }
                after = next_after;
                all.extend(lines);
            }
            let start = all.len().saturating_sub(ultimas);
            print_lines(&all[start..]);
            if !seguir {
                return;
            }
            loop {
                std::thread::sleep(Duration::from_millis(300));
                let ResponseBody::Console { lines, next_after } =
                    call(&mut c, RequestBody::Console { id, after, limit: 2000 })
                else {
                    fail("respuesta inesperada")
                };
                after = next_after;
                print_lines(&lines);
            }
        }
        "comando" => {
            need(&args, 2);
            let id = resolve(&mut c, &args[0]);
            let command = args[1..].join(" ");
            call(&mut c, RequestBody::SendCommand { id, command });
            println!("Enviado.");
        }
        "detener" => {
            need(&args, 1);
            let espera: u32 = opt(&mut args, "--espera").and_then(|n| n.parse().ok()).unwrap_or(120);
            let id = resolve(&mut c, &args[0]);
            println!("Deteniendo con guardado (hasta {espera} s)…");
            if let ResponseBody::Status(s) = call(
                &mut c,
                RequestBody::Stop {
                    id,
                    timeout_secs: espera,
                },
            ) {
                print_status(&s);
            }
        }
        "reiniciar" => {
            need(&args, 1);
            let id = resolve(&mut c, &args[0]);
            if let ResponseBody::Status(s) = call(
                &mut c,
                RequestBody::Restart {
                    id,
                    stop_timeout_secs: 120,
                },
            ) {
                print_status(&s);
            }
        }
        "forzar-cierre" => {
            need(&args, 1);
            if !flag(&mut args, "--confirmo") {
                fail("el cierre forzado puede perder lo no guardado del mundo. Si estás seguro, añade --confirmo");
            }
            let id = resolve(&mut c, &args[0]);
            call(
                &mut c,
                RequestBody::ForceKill {
                    id,
                    confirm: FORCE_KILL_CONFIRMATION.into(),
                },
            );
            println!("Cierre forzado enviado.");
        }
        "quitar" => {
            need(&args, 1);
            let id = resolve(&mut c, &args[0]);
            call(
                &mut c,
                RequestBody::Unregister {
                    id,
                    delete_files: false,
                },
            );
            println!("Quitado de la lista. Los archivos del servidor no se tocaron.");
        }
        other => fail(format!("orden desconocida «{other}». Ejecuta «merc --help»")),
    }
}
