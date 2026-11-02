//! `merc`: herramienta de terminal para hablar con el supervisor.
//!
//! La interfaz normal es la aplicación de escritorio. `merc` sirve para probar el supervisor y
//! para el **servidor casero sin pantalla**: dejarlo como servicio (`merc servicio …`) y
//! emparejarlo con la app de otra PC (`merc compañeros …`).

use merc_core::InstanceId;
use merc_ipc::*;
use merc_supervisor::ConsoleStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const AYUDA: &str = "\
merc — MercHosting desde la terminal (pruebas y servidor casero sin pantalla)

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

Servidor casero sin pantalla:
  servicio instalar                    Deja MercHosting funcionando en segundo plano (Linux:
                                       servicio de usuario de systemd; Windows: al iniciar sesión)
  servicio quitar                      Lo quita (no borra servidores ni datos)
  servicio estado
  compañeros activar | pausar          Permite (o no) que otras PCs se conecten con un código
  compañeros invitar --control-total [--minutos N]
                                       Código para manejar ESTA PC entera desde tu MercHosting
                                       de otra PC (todos los servidores y crear nuevos)
  compañeros invitar --servidor <servidor> [--rol observador|moderador|operador|administrador]
                     [--minutos N]     Código para un amigo, solo para ese servidor
  compañeros lista                     Quién tiene acceso, invitaciones abiertas y la huella
  compañeros quitar <nombre o huella>  Le quita el acceso al momento
  playit activar                       Descarga el agente oficial de Playit (verificado) y lo arranca
  playit vincular                      Muestra el enlace de playit.gg para vincular tu cuenta (ábrelo
                                       en el navegador de cualquier dispositivo) y espera la aprobación
  playit estado                        Estado del agente y sus túneles (direcciones para tus amigos)
  playit desvincular | desactivar      Quita la vinculación de esta PC o apaga el agente

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
        "bedrock" => Flavor::Bedrock,
        _ => fail(format!(
            "tipo desconocido: {s} (usa vanilla, paper, purpur, spigot, fabric, quilt, forge, neoforge, arclight o bedrock)"
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

    if cmd == "servicio" {
        need(&args, 1);
        servicio(&args[0], &data_dir);
        return;
    }

    let mut c = connect(&data_dir);
    match cmd.as_str() {
        "compañeros" | "companeros" => {
            need(&args, 1);
            let sub = args.remove(0);
            companeros(&mut c, &sub, &mut args);
        }
        "playit" => {
            need(&args, 1);
            playit(&mut c, &args[0]);
        }
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

fn print_invite(inv: &remote::NewInvite) {
    let minutes = (inv.invite.expires_at_unix_ms - inv.invite.created_at_unix_ms) / 60_000;
    println!();
    println!("{}", inv.code);
    println!();
    if inv.invite.host_admin {
        println!("Código de CONTROL TOTAL de esta PC. Vale {minutes} min y sirve una sola vez.");
        println!("En tu MercHosting de la otra PC: Compañeros › Conectar a mi servidor casero › pega el código.");
        println!("Quien lo use podrá administrar todos los servidores de esta PC y crear nuevos.");
        println!("No lo compartas con nadie más.");
    } else {
        println!(
            "Código para «{}». Vale {minutes} min y sirve una sola vez.",
            inv.invite.server_name
        );
        println!("Tu amigo lo pega en Compañeros › Unirme con un código.");
    }
}

fn parse_role(s: &str) -> remote::Role {
    use remote::Role;
    match s.to_lowercase().as_str() {
        "observador" => Role::Observador,
        "moderador" => Role::Moderador,
        "operador" => Role::Operador,
        "administrador" => Role::Administrador,
        _ => fail(format!(
            "rol desconocido: {s} (usa observador, moderador, operador o administrador)"
        )),
    }
}

fn playit_info(c: &mut Client, body: RequestBody) -> PlayitInfo {
    match call(c, body) {
        ResponseBody::Playit(p) => *p,
        other => fail(format!("respuesta inesperada: {other:?}")),
    }
}

fn print_playit(p: &PlayitInfo) {
    if !p.supported {
        println!("{}", merc_net::playit::LINUX_NOT_VERIFIED);
        return;
    }
    if !p.enabled {
        println!("Playit está desactivado. Actívalo con «merc playit activar».");
        return;
    }
    println!(
        "Agente oficial de Playit {}: {}",
        p.agent_version,
        if p.running { "en marcha" } else { "sin responder" }
    );
    if let Some(e) = &p.agent_error {
        println!("  ! {e}");
    }
    if let Some(a) = &p.agent {
        println!("Estado: {:?}", a.phase);
        if a.tunnels.is_empty() {
            println!(
                "Sin túneles todavía: créalos en playit.gg (Minecraft Java o Bedrock, hacia el puerto del servidor)."
            );
        }
        for t in &a.tunnels {
            println!(
                "  {}  →  {}{}",
                t.display_address,
                t.destination,
                if t.is_disabled {
                    "  (desactivado en playit.gg)"
                } else {
                    ""
                }
            );
        }
    }
}

fn playit(c: &mut Client, sub: &str) {
    match sub {
        "activar" => {
            let p = playit_info(c, RequestBody::PlayitEnable);
            print_playit(&p);
            if p.agent
                .as_ref()
                .is_some_and(|a| a.phase == merc_net::playit::AgentPhase::WaitingForSecret)
            {
                println!("Siguiente paso: «merc playit vincular».");
            }
        }
        "vincular" => {
            let p = playit_info(c, RequestBody::PlayitLink);
            let Some(claim) = p.claim.clone() else {
                print_playit(&p);
                return;
            };
            println!("Abre este enlace en el navegador (puede ser tu celular) y aprueba «MercHosting»:");
            println!();
            println!("  {}", claim.url);
            println!();
            println!("Esperando la aprobación… (Ctrl+C para salir; puedes volver a intentarlo luego)");
            let deadline = Instant::now() + Duration::from_secs(15 * 60);
            loop {
                std::thread::sleep(Duration::from_secs(2));
                let p = playit_info(c, RequestBody::PlayitStatus);
                match &p.claim {
                    Some(cl) if cl.done => {
                        println!("Vinculado. Ahora crea el túnel en playit.gg y míralo con «merc playit estado».");
                        print_playit(&p);
                        return;
                    }
                    Some(cl) if cl.error.is_some() => fail(cl.error.clone().unwrap_or_default()),
                    None if p
                        .agent
                        .as_ref()
                        .is_some_and(|a| a.phase == merc_net::playit::AgentPhase::Running) =>
                    {
                        println!("Vinculado.");
                        print_playit(&p);
                        return;
                    }
                    _ => {}
                }
                if Instant::now() > deadline {
                    fail("se acabó el tiempo de espera. Ejecuta «merc playit vincular» otra vez.");
                }
            }
        }
        "estado" => print_playit(&playit_info(c, RequestBody::PlayitStatus)),
        "desvincular" => {
            print_playit(&playit_info(c, RequestBody::PlayitUnlink));
            println!("Esta PC ya no está vinculada a tu cuenta de Playit.");
        }
        "desactivar" => {
            playit_info(c, RequestBody::PlayitDisable);
            println!("Playit desactivado. Tus amigos ya no podrán entrar por la dirección de Playit.");
        }
        _ => fail("usa: merc playit activar | vincular | estado | desvincular | desactivar"),
    }
}

fn companeros(c: &mut Client, sub: &str, args: &mut Vec<String>) {
    match sub {
        "activar" | "pausar" => {
            call(
                c,
                RequestBody::SetCompanionsEnabled {
                    enabled: sub == "activar",
                },
            );
            println!(
                "{}",
                if sub == "activar" {
                    "Compañeros activado: esta PC acepta conexiones con código."
                } else {
                    "Compañeros en pausa: nadie puede conectarse hasta que lo actives."
                }
            );
        }
        "invitar" => {
            let minutes: u32 = opt(args, "--minutos")
                .map(|m| m.parse().unwrap_or_else(|_| fail("--minutos debe ser un número")))
                .unwrap_or(30);
            let total = flag(args, "--control-total");
            let servidor = opt(args, "--servidor");
            let rol = opt(args, "--rol");
            if total == servidor.is_some() {
                fail("elige --control-total (tu servidor casero) o --servidor <servidor> (un amigo)");
            }
            let ResponseBody::Companions(info) = call(c, RequestBody::Companions) else {
                fail("respuesta inesperada")
            };
            if !info.enabled {
                call(c, RequestBody::SetCompanionsEnabled { enabled: true });
                println!("Compañeros activado.");
            }
            let body = if total {
                RequestBody::CreateHostInvite { minutes }
            } else {
                let id = resolve(c, servidor.as_deref().unwrap_or_default());
                let perms = parse_role(rol.as_deref().unwrap_or("observador")).perms();
                RequestBody::CreateInvite { id, perms, minutes }
            };
            if let ResponseBody::Invite(inv) = call(c, body) {
                print_invite(&inv);
            }
        }
        "lista" => {
            let ResponseBody::Companions(info) = call(c, RequestBody::Companions) else {
                fail("respuesta inesperada")
            };
            println!(
                "Compañeros: {}{}",
                if info.enabled { "activado" } else { "en pausa" },
                info.fingerprint
                    .as_deref()
                    .map(|f| format!(" · huella de esta PC: {f}"))
                    .unwrap_or_default()
            );
            if let Some(e) = &info.error {
                println!("Problema del canal: {e}");
            }
            if info.companions.is_empty() {
                println!("Nadie tiene acceso todavía.");
            }
            for comp in &info.companions {
                let what = if comp.host_admin {
                    "CONTROL TOTAL".to_string()
                } else {
                    comp.servers
                        .iter()
                        .map(|s| format!("{} ({} permisos)", s.name, s.perms.len()))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                println!(
                    "· {} · PC {}{} · {what}",
                    comp.name,
                    comp.fingerprint,
                    if comp.connected { " · conectado" } else { "" }
                );
            }
            for i in &info.invites {
                println!(
                    "Invitación abierta {} · {}",
                    i.invite_id,
                    if i.host_admin {
                        "control total".to_string()
                    } else {
                        i.server_name.clone()
                    }
                );
            }
        }
        "quitar" => {
            if args.is_empty() {
                fail("indica el nombre o la huella del compañero");
            }
            let q = args.join(" ").to_lowercase();
            let ResponseBody::Companions(info) = call(c, RequestBody::Companions) else {
                fail("respuesta inesperada")
            };
            let found: Vec<_> = info
                .companions
                .iter()
                .filter(|x| x.name.to_lowercase() == q || x.fingerprint.to_lowercase() == q)
                .collect();
            match found.as_slice() {
                [one] => {
                    call(
                        c,
                        RequestBody::RemoveCompanion {
                            companion: one.companion.clone(),
                        },
                    );
                    println!("Acceso quitado a {} (PC {}).", one.name, one.fingerprint);
                }
                [] => fail(format!("no hay ningún compañero «{q}». Mira «merc compañeros lista»")),
                _ => fail("hay varios con ese nombre: usa la huella"),
            }
        }
        other => fail(format!(
            "orden desconocida «compañeros {other}» (usa activar, pausar, invitar, lista o quitar)"
        )),
    }
}

fn servicio(sub: &str, data_dir: &Path) {
    #[cfg(windows)]
    {
        // En Windows se usa el mismo «Arrancar con el sistema» de la app (inicio de sesión).
        let mut c = connect(data_dir);
        match sub {
            "instalar" | "quitar" => {
                call(
                    &mut c,
                    RequestBody::SetStartup {
                        enabled: sub == "instalar",
                    },
                );
                println!(
                    "{}",
                    if sub == "instalar" {
                        "Listo: MercHosting arrancará en segundo plano al iniciar sesión en Windows. Para que arranque sin que nadie inicie sesión, configura el inicio de sesión automático de Windows."
                    } else {
                        "Quitado del inicio de sesión. Tus servidores y datos siguen igual."
                    }
                );
            }
            "estado" => {
                if let ResponseBody::Pong { server_version, .. } = call(&mut c, RequestBody::Ping) {
                    println!("Supervisor {server_version} en marcha. El arranque al iniciar sesión se ve en la app (Ajustes de un servidor).");
                }
            }
            other => fail(format!(
                "orden desconocida «servicio {other}» (usa instalar, quitar o estado)"
            )),
        }
    }
    #[cfg(not(windows))]
    {
        use merc_supervisord::service as svc;
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| fail("no se encontró la carpeta personal (HOME)"));
        let xdg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
        let dir = svc::unit_dir(xdg.as_deref(), &home);
        let unit = dir.join(svc::UNIT_NAME);
        let user = svc::current_user();
        let systemctl = |a: &[&str]| -> Option<(bool, String)> {
            let mut all = vec!["--user"];
            all.extend_from_slice(a);
            svc::run("systemctl", &all)
        };
        let linger =
            || svc::run("loginctl", &["show-user", &user, "-p", "Linger"]).map(|(_, out)| svc::linger_on(&out));
        match sub {
            "instalar" => {
                let exe = merc_ipc::launcher::find_supervisor_exe()
                    .unwrap_or_else(|| fail("no se encontró merc-supervisord junto a merc. Reinstala MercHosting"));
                // Desde un AppImage el archivo desaparece al cerrarlo: se usa una copia estable.
                let exe = if std::env::var_os("APPIMAGE").is_some() {
                    merc_ipc::launcher::stable_copy(&exe, data_dir).unwrap_or_else(|e| fail(e))
                } else {
                    exe
                };
                std::fs::create_dir_all(&dir).unwrap_or_else(|e| fail(e));
                let tmp = unit.with_extension("service.nuevo");
                std::fs::write(&tmp, svc::unit_text(&exe, data_dir)).unwrap_or_else(|e| fail(e));
                std::fs::rename(&tmp, &unit).unwrap_or_else(|e| fail(e));
                println!("Servicio escrito en {}", unit.display());
                // Si ya hay un supervisor abierto a mano, se cierra para que lo lleve systemd.
                if let Ok(mut c) = Client::connect(data_dir, "merc-cli") {
                    match c.call(RequestBody::Shutdown) {
                        Ok(_) => std::thread::sleep(Duration::from_millis(800)),
                        Err(e) => println!(
                            "Aviso: el supervisor actual sigue abierto ({e}). El servicio tomará el control cuando lo cierres o reinicies la PC."
                        ),
                    }
                }
                match systemctl(&["daemon-reload"]).and_then(|_| systemctl(&["enable", "--now", svc::UNIT_NAME])) {
                    Some((true, _)) => {
                        println!("Servicio activado: MercHosting funciona en segundo plano y arranca con el sistema.")
                    }
                    Some((false, out)) => {
                        println!("systemd no pudo activarlo:\n{}", out.trim());
                        println!("Prueba a mano: systemctl --user enable --now {}", svc::UNIT_NAME);
                    }
                    None => {
                        println!("No se encontró systemctl. En este sistema arranca MercHosting a mano o con tu gestor de servicios:");
                        println!(
                            "  {} --data-dir \"{}\" --al-iniciar-sesion",
                            exe.display(),
                            data_dir.display()
                        );
                    }
                }
                match linger() {
                    Some(true) => println!("El servicio sigue funcionando aunque cierres sesión (linger activado)."),
                    _ => {
                        println!();
                        println!("Para que MercHosting funcione también sin que nadie inicie sesión (por ejemplo tras un corte");
                        println!("de luz), un administrador de esta PC tiene que ejecutar UNA vez:");
                        println!("  sudo loginctl enable-linger {user}");
                        println!("(MercHosting no lo ejecuta por ti porque necesita permisos de administrador.)");
                    }
                }
                println!();
                println!("Siguiente paso: merc compañeros invitar --control-total");
            }
            "quitar" => {
                if unit.exists() {
                    if let Some((false, out)) = systemctl(&["disable", "--now", svc::UNIT_NAME]) {
                        println!("Aviso de systemd: {}", out.trim());
                    }
                    std::fs::remove_file(&unit).unwrap_or_else(|e| fail(e));
                    let _ = systemctl(&["daemon-reload"]);
                    println!("Servicio quitado. Tus servidores y datos siguen igual.");
                    println!("Si tenías servidores encendidos, siguen funcionando: deténlos desde la app o con «merc detener».");
                } else {
                    println!("No hay servicio instalado ({}).", unit.display());
                }
            }
            "estado" => {
                println!(
                    "Servicio: {}",
                    if unit.exists() {
                        unit.display().to_string()
                    } else {
                        "no instalado".into()
                    }
                );
                if let Some((_, out)) = systemctl(&["is-enabled", svc::UNIT_NAME]) {
                    println!("Arranca con el sistema: {}", out.trim());
                }
                if let Some((_, out)) = systemctl(&["is-active", svc::UNIT_NAME]) {
                    println!("Activo: {}", out.trim());
                }
                match linger() {
                    Some(true) => println!("Sin iniciar sesión: sí (linger)"),
                    Some(false) => println!("Sin iniciar sesión: no (sudo loginctl enable-linger {user})"),
                    None => println!("Sin iniciar sesión: no se pudo comprobar (falta loginctl)"),
                }
                match Client::connect(data_dir, "merc-cli") {
                    Ok(mut c) => {
                        if let Ok(ResponseBody::Pong { server_version, .. }) = c.call(RequestBody::Ping) {
                            println!("Supervisor {server_version} en marcha.");
                        }
                    }
                    Err(_) => println!("El supervisor no está en marcha."),
                }
            }
            other => fail(format!(
                "orden desconocida «servicio {other}» (usa instalar, quitar o estado)"
            )),
        }
    }
}
