//! Pruebas de extremo a extremo del supervisor persistente: se ejecuta el binario real
//! `merc-supervisord`, se habla con él por el canal real (named pipe en Windows) y se usa
//! un proceso Java real (FakeServer, que imita a Minecraft; no es Minecraft).

use merc_core::{InstanceId, RuntimeState};
use merc_ipc::*;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

fn java() -> PathBuf {
    let found = merc_core::java::find_java_candidates();
    found.into_iter().next().expect("hace falta Java para estas pruebas")
}

fn fake_jar() -> &'static Path {
    static JAR: OnceLock<PathBuf> = OnceLock::new();
    JAR.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("merc-fake-daemon-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../merc-supervisor/tests/fixtures/FakeServer.java");
        assert!(Command::new("javac")
            .args(["-encoding", "UTF-8", "-d"])
            .arg(&dir)
            .arg(&src)
            .status()
            .unwrap()
            .success());
        std::fs::write(dir.join("manifest.txt"), "Main-Class: FakeServer\n").unwrap();
        assert!(Command::new("jar")
            .current_dir(&dir)
            .args(["cfm", "fake-server.jar", "manifest.txt", "FakeServer.class"])
            .status()
            .unwrap()
            .success());
        dir.join("fake-server.jar")
    })
}

struct Env {
    _tmp: tempfile::TempDir,
    data: PathBuf,
    server: PathBuf,
    port: u16,
}

fn env() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("datos");
    let server = tmp.path().join("Servidor de prueba ñ");
    std::fs::create_dir_all(&server).unwrap();
    std::fs::copy(fake_jar(), server.join("server.jar")).unwrap();
    let port = TcpListener::bind(("0.0.0.0", 0)).unwrap().local_addr().unwrap().port();
    std::fs::write(server.join("server.properties"), format!("server-port={port}\n")).unwrap();
    std::fs::write(server.join("eula.txt"), "eula=true\n").unwrap();
    Env {
        _tmp: tmp,
        data,
        server,
        port,
    }
}

struct Daemon(Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn_daemon(data: &Path) -> Daemon {
    spawn_daemon_with(data, &[])
}

fn spawn_daemon_with(data: &Path, vars: &[(&str, &Path)]) -> Daemon {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_merc-supervisord"));
    for (k, v) in vars {
        cmd.env(k, v);
    }
    let child = cmd
        .arg("--data-dir")
        .arg(data)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    Daemon(child)
}

fn client(data: &Path) -> Client {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match Client::connect(data, "pruebas") {
            Ok(c) => return c,
            Err(e) if Instant::now() > deadline => panic!("no se pudo conectar: {e}"),
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

fn import(c: &mut Client, e: &Env) -> InstanceId {
    let body = RequestBody::ImportInstance {
        name: "Prueba".into(),
        path: e.server.clone(),
        java: java(),
        min_memory_mb: 0,
        max_memory_mb: 512,
    };
    match c.call(body).unwrap() {
        ResponseBody::Instance(i) => i.id,
        other => panic!("{other:?}"),
    }
}

fn status(c: &mut Client, id: InstanceId) -> merc_supervisor::StatusSnapshot {
    match c.call(RequestBody::Status { id }).unwrap() {
        ResponseBody::Status(s) => s,
        other => panic!("{other:?}"),
    }
}

fn wait_state(c: &mut Client, id: InstanceId, want: RuntimeState) -> merc_supervisor::StatusSnapshot {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let s = status(c, id);
        if s.state == want {
            return s;
        }
        if Instant::now() > deadline {
            panic!("estado {:?}, se esperaba {want:?}: {s:?}", s.state);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn api_code(r: Result<ResponseBody, ClientError>) -> ErrorCode {
    match r {
        Err(ClientError::Api(e)) => e.code,
        other => panic!("se esperaba un error del supervisor: {other:?}"),
    }
}

#[test]
fn auth_and_version_are_enforced() {
    let e = env();
    let _d = spawn_daemon(&e.data);
    let mut c = client(&e.data);
    assert!(matches!(c.call(RequestBody::Ping).unwrap(), ResponseBody::Pong { .. }));

    let ep = Endpoint::for_data_dir(&e.data);
    match Client::connect_with_token(&ep, "token-falso", "atacante") {
        Err(ClientError::Api(err)) => assert_eq!(err.code, ErrorCode::AuthFailed),
        other => panic!("{:?}", other.err()),
    }
    let token = read_token(&e.data).unwrap();
    match Client::connect_raw(&ep, &token, PROTOCOL_VERSION + 1, "futuro") {
        Err(ClientError::Api(err)) => assert_eq!(err.code, ErrorCode::VersionMismatch),
        other => panic!("{:?}", other.err()),
    }
    // El rechazo no tumba al supervisor.
    assert!(matches!(c.call(RequestBody::Ping).unwrap(), ResponseBody::Pong { .. }));
}

#[test]
fn single_supervisor_per_data_dir() {
    let e = env();
    let _d = spawn_daemon(&e.data);
    let mut c = client(&e.data);
    let ResponseBody::Pong { supervisor_pid, .. } = c.call(RequestBody::Ping).unwrap() else {
        panic!()
    };
    let mut second = spawn_daemon(&e.data);
    let deadline = Instant::now() + Duration::from_secs(10);
    let code = loop {
        if let Some(st) = second.0.try_wait().unwrap() {
            break st;
        }
        assert!(Instant::now() < deadline, "el segundo supervisor debería salir solo");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(code.success(), "sale sin error al ver que ya hay uno");
    let mut c2 = client(&e.data);
    let ResponseBody::Pong {
        supervisor_pid: pid2, ..
    } = c2.call(RequestBody::Ping).unwrap()
    else {
        panic!()
    };
    assert_eq!(supervisor_pid, pid2, "sigue atendiendo el primero");
}

/// Cerrar la interfaz (desconectar el cliente) no afecta al servidor; al reconectar se
/// recupera el estado y la consola acumulada.
#[test]
fn server_survives_client_disconnect() {
    let e = env();
    let _d = spawn_daemon(&e.data);
    let id = {
        let mut c = client(&e.data);
        let id = import(&mut c, &e);
        c.call(RequestBody::Start { id }).unwrap();
        wait_state(&mut c, id, RuntimeState::Online);
        c.call(RequestBody::SendCommand {
            id,
            command: "say antes de cerrar".into(),
        })
        .unwrap();
        id
        // `c` se destruye aquí: la "interfaz" se cierra.
    };
    std::thread::sleep(Duration::from_millis(500));
    let mut c = client(&e.data);
    let s = status(&mut c, id);
    assert_eq!(s.state, RuntimeState::Online);
    assert!(s.has_console);
    let ResponseBody::Console { lines, .. } = c
        .call(RequestBody::Console {
            id,
            after: 0,
            limit: 2000,
        })
        .unwrap()
    else {
        panic!()
    };
    assert!(lines
        .iter()
        .any(|l| l.text.contains("Comando recibido: say antes de cerrar")));
    assert_eq!(
        api_code(c.call(RequestBody::Shutdown)),
        ErrorCode::Conflict,
        "no se cierra con servidores en marcha"
    );
    assert_eq!(
        api_code(c.call(RequestBody::Unregister {
            id,
            delete_files: false
        })),
        ErrorCode::InvalidState
    );
    c.call(RequestBody::Stop { id, timeout_secs: 30 }).unwrap();
    assert_eq!(status(&mut c, id).state, RuntimeState::Stopped);
    // Persistencia: la instancia sigue registrada tras reiniciar el supervisor.
    c.call(RequestBody::Shutdown).unwrap();
}

/// Si el supervisor muere de golpe, el servidor sigue vivo. El supervisor nuevo lo adopta
/// (UNKNOWN, sin consola), impide arrancar otro encima y detecta cuando termina.
#[test]
fn supervisor_crash_adopts_orphan() {
    let e = env();
    let mut d = spawn_daemon(&e.data);
    let mut c = client(&e.data);
    let id = import(&mut c, &e);
    c.call(RequestBody::Start { id }).unwrap();
    let s = wait_state(&mut c, id, RuntimeState::Online);
    let java_pid = s.pid.unwrap();
    drop(c);

    // Caída abrupta del supervisor.
    d.0.kill().unwrap();
    d.0.wait().unwrap();
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        TcpListener::bind(("0.0.0.0", e.port)).is_err(),
        "el servidor sigue vivo con su puerto"
    );

    let _d2 = spawn_daemon(&e.data);
    let mut c = client(&e.data);
    let s = status(&mut c, id);
    assert_eq!(s.state, RuntimeState::Unknown, "{s:?}");
    assert_eq!(s.pid, Some(java_pid));
    assert!(!s.has_console);
    assert!(s.hint.as_deref().unwrap_or("").contains("siguió funcionando"));

    assert_eq!(
        api_code(c.call(RequestBody::Start { id })),
        ErrorCode::Conflict,
        "no se arranca otro proceso sobre el mismo mundo"
    );
    assert_eq!(
        api_code(c.call(RequestBody::SendCommand {
            id,
            command: "say x".into()
        })),
        ErrorCode::NoConsole
    );
    assert_eq!(
        api_code(c.call(RequestBody::Stop { id, timeout_secs: 5 })),
        ErrorCode::NoConsole
    );
    assert_eq!(
        api_code(c.call(RequestBody::ForceKill {
            id,
            confirm: "si".into()
        })),
        ErrorCode::BadRequest,
        "el cierre forzado exige la confirmación exacta"
    );

    // El servidor se detiene "desde dentro" (equivale a /stop de un operador en el juego).
    std::fs::write(e.server.join("stop.flag"), "").unwrap();
    let s = wait_state(&mut c, id, RuntimeState::Stopped);
    assert!(s.hint.unwrap().contains("terminó"));
    std::fs::remove_file(e.server.join("stop.flag")).unwrap();

    // Ya se puede volver a arrancar con consola completa.
    c.call(RequestBody::Start { id }).unwrap();
    let s = wait_state(&mut c, id, RuntimeState::Online);
    assert!(s.has_console);
    c.call(RequestBody::Stop { id, timeout_secs: 30 }).unwrap();
    c.call(RequestBody::Shutdown).unwrap();
}

#[test]
fn orphan_force_kill_with_confirmation() {
    let e = env();
    let mut d = spawn_daemon(&e.data);
    let mut c = client(&e.data);
    let id = import(&mut c, &e);
    c.call(RequestBody::Start { id }).unwrap();
    wait_state(&mut c, id, RuntimeState::Online);
    drop(c);
    d.0.kill().unwrap();
    d.0.wait().unwrap();

    let _d2 = spawn_daemon(&e.data);
    let mut c = client(&e.data);
    wait_state(&mut c, id, RuntimeState::Unknown);
    c.call(RequestBody::ForceKill {
        id,
        confirm: FORCE_KILL_CONFIRMATION.into(),
    })
    .unwrap();
    let s = wait_state(&mut c, id, RuntimeState::Stopped);
    assert!(s.last_exit.unwrap().forced);
    std::thread::sleep(Duration::from_millis(300));
    assert!(TcpListener::bind(("0.0.0.0", e.port)).is_ok(), "el puerto quedó libre");
    c.call(RequestBody::Shutdown).unwrap();
}

#[test]
fn import_validation() {
    let e = env();
    let _d = spawn_daemon(&e.data);
    let mut c = client(&e.data);
    import(&mut c, &e);
    // Misma carpeta otra vez.
    let dup = RequestBody::ImportInstance {
        name: "Otra".into(),
        path: e.server.clone(),
        java: java(),
        min_memory_mb: 0,
        max_memory_mb: 512,
    };
    assert_eq!(api_code(c.call(dup)), ErrorCode::Conflict);
    // Carpeta sin servidor.
    let empty = e.data.join("vacia");
    std::fs::create_dir_all(&empty).unwrap();
    let bad = RequestBody::ImportInstance {
        name: "X".into(),
        path: empty,
        java: java(),
        min_memory_mb: 0,
        max_memory_mb: 512,
    };
    assert_eq!(api_code(c.call(bad)), ErrorCode::BadRequest);
    // Ruta relativa.
    let rel = RequestBody::Detect {
        path: "relativa".into(),
    };
    assert_eq!(api_code(c.call(rel)), ErrorCode::BadRequest);
    // La importación no modificó la carpeta del servidor.
    let mut names: Vec<_> = std::fs::read_dir(&e.server)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    names.sort();
    assert_eq!(names, ["eula.txt", "server.jar", "server.properties"]);
    c.call(RequestBody::Shutdown).unwrap();
}

#[test]
fn eula_only_with_explicit_consent() {
    let e = env();
    std::fs::remove_file(e.server.join("eula.txt")).unwrap();
    let _d = spawn_daemon(&e.data);
    let mut c = client(&e.data);
    let id = import(&mut c, &e);
    let ResponseBody::Instances(list) = c.call(RequestBody::ListInstances).unwrap() else {
        panic!()
    };
    assert!(!list[0].eula_accepted);
    assert_eq!(list[0].configured_port, Some(e.port));
    assert!(!e.server.join("eula.txt").exists(), "importar no crea eula.txt");

    assert_eq!(
        api_code(c.call(RequestBody::Start { id })),
        ErrorCode::BadRequest,
        "sin EULA no arranca"
    );
    assert_eq!(
        api_code(c.call(RequestBody::AcceptEula { id, accepted: false })),
        ErrorCode::BadRequest
    );
    assert!(!e.server.join("eula.txt").exists());

    c.call(RequestBody::AcceptEula { id, accepted: true }).unwrap();
    let text = std::fs::read_to_string(e.server.join("eula.txt")).unwrap();
    assert!(text.lines().any(|l| l == "eula=true"), "{text}");
    let ResponseBody::Instances(list) = c.call(RequestBody::ListInstances).unwrap() else {
        panic!()
    };
    assert!(list[0].eula_accepted);

    c.call(RequestBody::Start { id }).unwrap();
    wait_state(&mut c, id, RuntimeState::Online);
    assert_eq!(
        api_code(c.call(RequestBody::AcceptEula { id, accepted: true })),
        ErrorCode::InvalidState,
        "no se toca eula.txt con el servidor en marcha"
    );
    c.call(RequestBody::Stop { id, timeout_secs: 30 }).unwrap();
    c.call(RequestBody::Shutdown).unwrap();
}

#[test]
fn metrics_and_system_info() {
    let e = env();
    let _d = spawn_daemon(&e.data);
    let mut c = client(&e.data);
    let ResponseBody::System {
        total_memory_bytes,
        logical_cpus,
        ..
    } = c.call(RequestBody::SystemInfo).unwrap()
    else {
        panic!()
    };
    assert!(total_memory_bytes.unwrap() > 0);
    assert!(logical_cpus >= 1);
    let id = import(&mut c, &e);
    std::fs::write(
        e.server.join("server.properties"),
        format!("server-port={}\nmax-players=8\n", e.port),
    )
    .unwrap();
    c.call(RequestBody::Start { id }).unwrap();
    wait_state(&mut c, id, RuntimeState::Online);
    c.call(RequestBody::SendCommand {
        id,
        command: "fakejoin Steve".into(),
    })
    .unwrap();
    std::thread::sleep(Duration::from_millis(4500));
    let ResponseBody::Metrics {
        samples,
        players,
        max_players,
    } = c.call(RequestBody::Metrics { id }).unwrap()
    else {
        panic!()
    };
    assert_eq!(players, vec!["Steve".to_string()]);
    assert_eq!(max_players, Some(8));
    assert!(!samples.is_empty() && samples[0].memory_bytes > 0);
    c.call(RequestBody::Stop { id, timeout_secs: 30 }).unwrap();
    c.call(RequestBody::Shutdown).unwrap();
}

/// Administración: server.properties con copia previa, archivos confinados, papelera,
/// bloqueo mientras está encendido y acciones de jugadores por comando oficial.
#[test]
fn local_administration() {
    use merc_admin::players::PlayerAction;
    let e = env();
    std::fs::write(
        e.server.join("server.properties"),
        format!(
            "#Minecraft server properties\nserver-port={}\nmotd=A Minecraft Server\n",
            e.port
        ),
    )
    .unwrap();
    std::fs::write(e.server.join("ops.json"), r#"[{"uuid":"u","name":"Steve","level":4}]"#).unwrap();
    let _d = spawn_daemon(&e.data);
    let mut c = client(&e.data);
    let id = import(&mut c, &e);

    let ResponseBody::Properties {
        exists,
        entries,
        modified_unix_ms,
    } = c.call(RequestBody::Properties { id }).unwrap()
    else {
        panic!()
    };
    assert!(exists);
    assert_eq!(entries[1].value, "A Minecraft Server");
    let changes = [("motd".to_string(), "Servidor de Ñandú".to_string())]
        .into_iter()
        .collect();
    let ResponseBody::Properties { entries, .. } = c
        .call(RequestBody::SetProperties {
            id,
            changes,
            expected_modified: modified_unix_ms,
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(entries[1].value, "Servidor de Ñandú");
    let text = std::fs::read_to_string(e.server.join("server.properties")).unwrap();
    assert!(text.starts_with("#Minecraft server properties\n"));
    assert!(text.contains("motd=Servidor de \\u00d1and\\u00fa"));
    // Valor inválido y marca de tiempo vieja: rechazados.
    let bad = [("server-port".to_string(), "99999".to_string())].into_iter().collect();
    assert_eq!(
        api_code(c.call(RequestBody::SetProperties {
            id,
            changes: bad,
            expected_modified: None
        })),
        ErrorCode::BadRequest
    );
    let stale = [("pvp".to_string(), "false".to_string())].into_iter().collect();
    assert_eq!(
        api_code(c.call(RequestBody::SetProperties {
            id,
            changes: stale,
            expected_modified: Some(1)
        })),
        ErrorCode::Conflict
    );
    // Hay copia previa fuera de la carpeta del servidor.
    assert!(e.data.join("instancias").join(id.to_string()).join("copias").is_dir());

    let ResponseBody::Files { entries, .. } = c
        .call(RequestBody::ListFiles {
            id,
            path: String::new(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert!(entries.iter().any(|f| f.name == "server.jar"));
    assert_eq!(
        api_code(c.call(RequestBody::ListFiles { id, path: "../".into() })),
        ErrorCode::BadRequest
    );
    c.call(RequestBody::WriteFile {
        id,
        path: "notas.txt".into(),
        content: "hola".into(),
        expected_modified: None,
    })
    .unwrap();
    let ResponseBody::FileDone { path } = c
        .call(RequestBody::TrashFile {
            id,
            path: "notas.txt".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert!(Path::new(&path).is_file() && !e.server.join("notas.txt").exists());

    // Jugadores: sin encender, las acciones se rechazan.
    let ResponseBody::Players { lists, can_act, .. } = c.call(RequestBody::Players { id }).unwrap() else {
        panic!()
    };
    assert_eq!(lists.ops[0].name, "Steve");
    assert!(!can_act);
    let op = |name: &str| RequestBody::PlayerAction {
        id,
        action: PlayerAction::Op,
        name: name.into(),
        reason: None,
    };
    assert_eq!(api_code(c.call(op("Alex"))), ErrorCode::InvalidState);

    c.call(RequestBody::Start { id }).unwrap();
    wait_state(&mut c, id, RuntimeState::Online);
    assert_eq!(api_code(c.call(op("Alex; stop"))), ErrorCode::BadRequest);
    c.call(op("Alex")).unwrap();
    // Con el servidor encendido no se tocan archivos.
    assert_eq!(
        api_code(c.call(RequestBody::WriteFile {
            id,
            path: "x.txt".into(),
            content: "x".into(),
            expected_modified: None
        })),
        ErrorCode::InvalidState
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let ResponseBody::Console { lines, .. } = c
            .call(RequestBody::Console {
                id,
                after: 0,
                limit: 500,
            })
            .unwrap()
        else {
            panic!()
        };
        if lines.iter().any(|l| l.text.contains("Comando recibido: op Alex")) {
            break;
        }
        assert!(Instant::now() < deadline, "el comando no llegó al servidor");
        std::thread::sleep(Duration::from_millis(100));
    }
    c.call(RequestBody::Stop { id, timeout_secs: 30 }).unwrap();
    let out = e.data.join("consola-exportada.log");
    c.call(RequestBody::ExportConsole { id, dest: out.clone() }).unwrap();
    assert!(std::fs::read_to_string(out)
        .unwrap()
        .contains("Comando recibido: op Alex"));
}

/// Mundos, papelera, reglas del juego, actividad y reinicio con aviso.
#[test]
fn worlds_rules_activity_and_notice() {
    let e = env();
    for (p, c) in [
        ("world/level.dat", "W"),
        ("world_nether/level.dat", "N"),
        ("world/region/r.mca", "R"),
    ] {
        let path = e.server.join(p);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, c).unwrap();
    }
    let _d = spawn_daemon(&e.data);
    let mut c = client(&e.data);
    let id = import(&mut c, &e);

    let ResponseBody::Worlds { worlds, level_name } = c.call(RequestBody::Worlds { id }).unwrap() else {
        panic!()
    };
    assert_eq!(level_name, "world");
    assert_eq!(worlds.len(), 1);
    assert!(worlds[0].active && worlds[0].companions == ["world_nether"]);

    c.call(RequestBody::DuplicateWorld {
        id,
        name: "world".into(),
        new_name: "copia".into(),
    })
    .unwrap();
    let zip = e.data.join("mundo.zip");
    c.call(RequestBody::ExportWorld {
        id,
        name: "world".into(),
        dest: zip.clone(),
    })
    .unwrap();
    c.call(RequestBody::ImportWorld {
        id,
        source: zip,
        new_name: "importado".into(),
    })
    .unwrap();
    let ResponseBody::Worlds { worlds, .. } = c
        .call(RequestBody::SetActiveWorld {
            id,
            name: "copia".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(
        worlds.iter().map(|w| w.name.as_str()).collect::<Vec<_>>(),
        ["copia", "importado", "world"]
    );
    assert!(worlds.iter().find(|w| w.name == "copia").unwrap().active);
    assert!(std::fs::read_to_string(e.server.join("server.properties"))
        .unwrap()
        .contains("level-name=copia"));
    // El activo no se puede mandar a la papelera.
    assert_eq!(
        api_code(c.call(RequestBody::TrashWorld {
            id,
            name: "copia".into()
        })),
        ErrorCode::Conflict
    );
    c.call(RequestBody::TrashWorld {
        id,
        name: "world".into(),
    })
    .unwrap();
    assert!(!e.server.join("world").exists() && !e.server.join("world_nether").exists());

    let ResponseBody::SavedItems(items) = c.call(RequestBody::SavedItems { id }).unwrap() else {
        panic!()
    };
    let world_item = items.iter().find(|i| i.path == "world").unwrap().id.clone();
    c.call(RequestBody::RestoreSaved {
        id,
        item_id: world_item,
    })
    .unwrap();
    assert!(e.server.join("world/level.dat").is_file());

    // Reglas: necesitan el servidor encendido.
    assert_eq!(api_code(c.call(RequestBody::GameRules { id })), ErrorCode::InvalidState);
    c.call(RequestBody::Start { id }).unwrap();
    wait_state(&mut c, id, RuntimeState::Online);
    let ResponseBody::GameRules { rules, new_names } = c.call(RequestBody::GameRules { id }).unwrap() else {
        panic!()
    };
    assert!(new_names, "sin versión conocida se usan los nombres nuevos");
    let keep = rules.iter().find(|r| r.key == "keep_inventory").unwrap();
    assert_eq!(keep.value.as_deref(), Some("false"));
    assert_eq!(
        rules
            .iter()
            .find(|r| r.key == "random_tick_speed")
            .unwrap()
            .value
            .as_deref(),
        Some("3")
    );
    c.call(RequestBody::SetGameRule {
        id,
        key: "keep_inventory".into(),
        value: "true".into(),
    })
    .unwrap();
    assert_eq!(
        api_code(c.call(RequestBody::SetGameRule {
            id,
            key: "keep_inventory".into(),
            value: "true; stop".into()
        })),
        ErrorCode::BadRequest
    );

    // Reinicio con aviso: avisa por el chat y vuelve a estar en línea.
    c.call(RequestBody::RestartWithNotice { id, seconds: 10 }).unwrap();
    assert_eq!(
        api_code(c.call(RequestBody::RestartWithNotice { id, seconds: 10 })),
        ErrorCode::Conflict
    );
    let deadline = Instant::now() + Duration::from_secs(40);
    let mut saw_notice = false;
    let mut restarted = false;
    while Instant::now() < deadline && !(saw_notice && restarted) {
        let ResponseBody::Console { lines, .. } = c
            .call(RequestBody::Console {
                id,
                after: 0,
                limit: 2000,
            })
            .unwrap()
        else {
            panic!()
        };
        saw_notice |= lines.iter().any(|l| l.text.contains("se reiniciará en 10 s"));
        restarted |= lines.iter().filter(|l| l.text.contains("Done (")).count() >= 2;
        std::thread::sleep(Duration::from_millis(250));
    }
    assert!(saw_notice && restarted, "aviso {saw_notice}, reinicio {restarted}");
    wait_state(&mut c, id, RuntimeState::Online);

    let ResponseBody::Activity(act) = c.call(RequestBody::Activity { id, limit: 0 }).unwrap() else {
        panic!()
    };
    let actions: Vec<&str> = act.iter().map(|a| a.action.as_str()).collect();
    for want in [
        "world.duplicate",
        "world.activate",
        "world.trash",
        "file.restore",
        "world.gamerule",
        "server.restart.notice",
    ] {
        assert!(actions.contains(&want), "falta {want} en {actions:?}");
    }
    c.call(RequestBody::Stop { id, timeout_secs: 30 }).unwrap();
}

fn console_has(c: &mut Client, id: InstanceId, text: &str) -> bool {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        let ResponseBody::Console { lines, .. } = c
            .call(RequestBody::Console {
                id,
                after: 0,
                limit: 2000,
            })
            .unwrap()
        else {
            panic!()
        };
        if lines.iter().any(|l| l.text.contains(text)) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    false
}

fn admin(c: &mut Client, id: InstanceId) -> AdminState {
    match c.call(RequestBody::AdminState { id }).unwrap() {
        ResponseBody::AdminState(a) => a,
        other => panic!("{other:?}"),
    }
}

#[test]
fn maintenance_cancel_restart_and_delete_folder() {
    let e = env();
    let recycle = e.data.parent().unwrap().join("papelera-de-windows");
    let _d = spawn_daemon_with(&e.data, &[("MERC_RECYCLE_DIR", &recycle)]);
    let mut c = client(&e.data);
    let id = import(&mut c, &e);

    // Mantenimiento: pide el servidor encendido.
    assert_eq!(
        api_code(c.call(RequestBody::SetMaintenance { id, on: true })),
        ErrorCode::InvalidState
    );
    assert_eq!(api_code(c.call(RequestBody::CancelWorldOp { id })), ErrorCode::NotFound);
    c.call(RequestBody::Start { id }).unwrap();
    wait_state(&mut c, id, RuntimeState::Online);
    let a = admin(&mut c, id);
    assert!(a.maintenance.is_none() && a.restart_in_seconds.is_none() && a.world_op.is_none());
    assert!(!a.whitelist_on);

    c.call(RequestBody::SetMaintenance { id, on: true }).unwrap();
    assert!(console_has(&mut c, id, "whitelist on"));
    assert!(console_has(&mut c, id, "Servidor en mantenimiento"));
    let m = admin(&mut c, id).maintenance.expect("mantenimiento activo");
    assert!(!m.previous_whitelist);

    // Reinicio avisado y cancelado: no se reinicia.
    c.call(RequestBody::RestartWithNotice { id, seconds: 30 }).unwrap();
    let left = admin(&mut c, id).restart_in_seconds.expect("reinicio pendiente");
    assert!((25..=30).contains(&left), "{left}");
    c.call(RequestBody::CancelRestart { id }).unwrap();
    assert!(console_has(&mut c, id, "Reinicio cancelado"));
    std::thread::sleep(Duration::from_millis(500));
    assert!(admin(&mut c, id).restart_in_seconds.is_none());
    assert_eq!(api_code(c.call(RequestBody::CancelRestart { id })), ErrorCode::NotFound);
    assert_eq!(status(&mut c, id).state, RuntimeState::Online);

    c.call(RequestBody::SetMaintenance { id, on: false }).unwrap();
    assert!(console_has(&mut c, id, "whitelist off"));
    assert!(admin(&mut c, id).maintenance.is_none());

    c.call(RequestBody::Stop { id, timeout_secs: 30 }).unwrap();
    wait_state(&mut c, id, RuntimeState::Stopped);

    // Una carpeta que no parece de un servidor no se borra, y el servidor sigue en la lista.
    let saved: Vec<(PathBuf, Vec<u8>)> = ["server.jar", "server.properties", "eula.txt"]
        .iter()
        .map(|f| (e.server.join(f), std::fs::read(e.server.join(f)).unwrap()))
        .collect();
    for (p, _) in &saved {
        std::fs::remove_file(p).unwrap();
    }
    assert_eq!(
        api_code(c.call(RequestBody::Unregister { id, delete_files: true })),
        ErrorCode::Conflict
    );
    assert!(e.server.is_dir());
    for (p, bytes) in &saved {
        std::fs::write(p, bytes).unwrap();
    }

    // Con la casilla marcada, la carpeta va a la papelera y el servidor sale de la lista.
    c.call(RequestBody::Unregister { id, delete_files: true }).unwrap();
    assert!(!e.server.exists());
    let recycled: Vec<PathBuf> = std::fs::read_dir(&recycle)
        .unwrap()
        .flatten()
        .flat_map(|stamp| std::fs::read_dir(stamp.path()).unwrap().flatten().map(|x| x.path()))
        .collect();
    assert!(recycled.iter().any(|p| p.join("server.jar").is_file()), "{recycled:?}");
    let ResponseBody::Instances(list) = c.call(RequestBody::ListInstances).unwrap() else {
        panic!()
    };
    assert!(list.is_empty());
}

/// Servidor HTTP mínimo que responde como la API de Modrinth (rutas por «MÉTODO /ruta»).
/// `make` recibe la dirección base para poder escribir URLs de descarga que apunten a él.
fn fake_modrinth(make: impl FnOnce(&str) -> std::collections::HashMap<String, Vec<u8>>) -> String {
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let routes = std::sync::Arc::new(make(&base));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let routes = routes.clone();
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() {
                    return;
                }
                let mut parts = line.split_whitespace();
                let method = parts.next().unwrap_or("GET").to_string();
                let path = parts.next().unwrap_or("/").split('?').next().unwrap().to_string();
                let mut len = 0usize;
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                        break;
                    }
                    if let Some(v) = h.to_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0u8; len];
                let _ = reader.read_exact(&mut body);
                let mut s = stream;
                match routes.get(&format!("{method} {path}")) {
                    Some(b) => {
                        let _ = write!(
                            s,
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            b.len()
                        );
                        let _ = s.write_all(b);
                    }
                    None => {
                        let _ = write!(s, "HTTP/1.1 404 X\r\nContent-Length: 2\r\nConnection: close\r\n\r\nno");
                    }
                }
            });
        }
    });
    base
}

fn sha1_hex(b: &[u8]) -> String {
    use sha1::Digest as _;
    sha1::Sha1::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn mods_from_modrinth_install_toggle_remove() {
    use serde_json::json;
    let e = env();
    std::fs::rename(
        e.server.join("server.jar"),
        e.server
            .join("fabric-server-mc.1.21.1-loader.0.16.5-launcher.1.0.1.jar"),
    )
    .unwrap();
    let jar = b"un mod de prueba".to_vec();
    let lib = b"su dependencia".to_vec();
    let base = fake_modrinth(|base| {
        let v = |id: &str, project: &str, data: &[u8], deps: serde_json::Value| {
            json!({
                "id": id, "project_id": project, "name": id, "version_number": format!("{id}-1"),
                "version_type": "release", "game_versions": ["1.21.1"], "loaders": ["fabric"],
                "date_published": "2026-10-01T00:00:00Z", "downloads": 1, "changelog": null, "dependencies": deps,
                "files": [{ "hashes": { "sha1": sha1_hex(data) }, "url": format!("{base}/cdn/{id}.jar"),
                            "filename": format!("{id}.jar"), "primary": true, "size": data.len() }]
            })
        };
        let p = |id: &str| {
            json!({ "id": id, "slug": id.to_lowercase(), "title": format!("Proyecto {id}"), "client_side": "optional",
                    "server_side": "required", "license": { "id": "MIT" } })
        };
        let vm = v(
            "modv1",
            "MODA",
            &jar,
            json!([{ "project_id": "LIBB", "dependency_type": "required" }]),
        );
        let vl = v("libv1", "LIBB", &lib, json!([]));
        let mut r = std::collections::HashMap::new();
        let mut put = |k: &str, body: Vec<u8>| {
            r.insert(k.to_string(), body);
        };
        put("GET /v2/version/modv1", vm.to_string().into_bytes());
        put("GET /v2/project/MODA", p("MODA").to_string().into_bytes());
        put("GET /v2/project/LIBB", p("LIBB").to_string().into_bytes());
        put("GET /v2/project/LIBB/version", json!([vl]).to_string().into_bytes());
        put("GET /v2/project/MODA/version", json!([vm]).to_string().into_bytes());
        put(
            "GET /v2/search",
            json!({ "hits": [{ "project_id": "MODA", "slug": "moda", "title": "Proyecto MODA", "author": "x",
                "description": "d", "downloads": 5, "server_side": "required", "client_side": "optional" }],
                "offset": 0, "total_hits": 1 })
            .to_string()
            .into_bytes(),
        );
        put(
            "POST /v2/version_files",
            json!({ sha1_hex(&jar): vm, sha1_hex(&lib): vl })
                .to_string()
                .into_bytes(),
        );
        put("POST /v2/version_files/update", b"{}".to_vec());
        put(
            "GET /v2/projects",
            json!([p("MODA"), p("LIBB")]).to_string().into_bytes(),
        );
        put("GET /cdn/modv1.jar", jar.clone());
        put("GET /cdn/libv1.jar", lib.clone());
        r
    });
    let mr = PathBuf::from(format!("{base}/v2"));
    let _d = spawn_daemon_with(&e.data, &[("MERC_SOURCE_MODRINTH", &mr)]);
    let mut c = client(&e.data);
    let id = import(&mut c, &e);

    let ResponseBody::ContentPlatforms(p) = c.call(RequestBody::ContentPlatforms { id }).unwrap() else {
        panic!()
    };
    assert_eq!(p.len(), 2);
    assert_eq!((p[0].kind, p[0].folder.as_str()), (ContentKind::Mod, "mods"));
    assert_eq!(
        (p[1].kind, p[1].folder.as_str()),
        (ContentKind::Datapack, "world/datapacks")
    );
    // Sin clave de CurseForge, solo Modrinth, y pedir CurseForge se rechaza.
    assert_eq!(p[0].sources, [ContentSource::Modrinth]);
    assert_eq!(
        api_code(c.call(RequestBody::ContentSearch {
            id,
            kind: ContentKind::Mod,
            source: ContentSource::Curseforge,
            query: String::new(),
            sort: SearchSort::Relevance,
            offset: 0,
        })),
        ErrorCode::BadRequest
    );
    // Con la clave en la carpeta de datos, CurseForge aparece (sin reiniciar el supervisor).
    std::fs::write(e.data.join("curseforge-api-key.txt"), "clave-de-prueba\n").unwrap();
    let ResponseBody::ContentPlatforms(p2) = c.call(RequestBody::ContentPlatforms { id }).unwrap() else {
        panic!()
    };
    assert_eq!(p2[0].sources, [ContentSource::Modrinth, ContentSource::Curseforge]);
    std::fs::remove_file(e.data.join("curseforge-api-key.txt")).unwrap();
    assert_eq!(
        api_code(c.call(RequestBody::ContentInstalled {
            id,
            kind: ContentKind::Plugin
        })),
        ErrorCode::BadRequest
    );

    let ResponseBody::ContentSearch(page) = c
        .call(RequestBody::ContentSearch {
            id,
            kind: ContentKind::Mod,
            source: ContentSource::Modrinth,
            query: "mapa".into(),
            sort: SearchSort::Relevance,
            offset: 0,
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(page.hits[0].project_id, "MODA");
    let ResponseBody::ContentDetails { versions, .. } = c
        .call(RequestBody::ContentProject {
            id,
            kind: ContentKind::Mod,
            source: ContentSource::Modrinth,
            project: "MODA".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(versions[0].id, "modv1");
    let ResponseBody::ContentPlan(plan) = c
        .call(RequestBody::ContentPlan {
            id,
            kind: ContentKind::Mod,
            source: ContentSource::Modrinth,
            version_id: "modv1".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(plan.items.len(), 2, "{plan:?}");

    // Con el servidor encendido no se instala nada.
    c.call(RequestBody::Start { id }).unwrap();
    wait_state(&mut c, id, RuntimeState::Online);
    let install = RequestBody::ContentInstall {
        id,
        kind: ContentKind::Mod,
        source: ContentSource::Modrinth,
        version_id: "modv1".into(),
    };
    assert_eq!(api_code(c.call(install.clone())), ErrorCode::InvalidState);
    c.call(RequestBody::Stop { id, timeout_secs: 30 }).unwrap();
    wait_state(&mut c, id, RuntimeState::Stopped);

    let ResponseBody::ContentList(list) = c.call(install.clone()).unwrap() else {
        panic!()
    };
    assert!(list.online);
    let names: Vec<&str> = list.items.iter().map(|i| i.file_name.as_str()).collect();
    assert_eq!(names, ["libv1.jar", "modv1.jar"]);
    assert_eq!(list.items[1].title.as_deref(), Some("Proyecto MODA"));
    assert_eq!(std::fs::read(e.server.join("mods/modv1.jar")).unwrap(), jar);
    // La misma versión otra vez: bloqueado.
    assert_eq!(api_code(c.call(install)), ErrorCode::Conflict);

    let ResponseBody::ContentList(list) = c
        .call(RequestBody::ContentToggle {
            id,
            kind: ContentKind::Mod,
            file_name: "modv1.jar".into(),
            enabled: false,
        })
        .unwrap()
    else {
        panic!()
    };
    assert!(list
        .items
        .iter()
        .any(|i| i.file_name == "modv1.jar.disabled" && !i.enabled));
    let ResponseBody::ContentList(list) = c
        .call(RequestBody::ContentRemove {
            id,
            kind: ContentKind::Mod,
            file_name: "modv1.jar.disabled".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(list.items.len(), 1);
    let ResponseBody::SavedItems(saved) = c.call(RequestBody::SavedItems { id }).unwrap() else {
        panic!()
    };
    assert!(saved.iter().any(|s| s.path == "mods/modv1.jar.disabled"));
    let ResponseBody::Activity(act) = c.call(RequestBody::Activity { id, limit: 0 }).unwrap() else {
        panic!()
    };
    assert!(act
        .iter()
        .any(|a| a.action == "content.install" && a.detail.contains("Proyecto MODA")));
}

#[test]
fn resource_pack_is_checked_hashed_and_written() {
    use std::io::Write as _;
    let e = env();
    let mut pack = vec![];
    {
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut pack));
        z.start_file("pack.mcmeta", zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(br#"{"pack":{"pack_format":34,"description":"Texturas"}}"#)
            .unwrap();
        z.finish().unwrap();
    }
    let sha = sha1_hex(&pack);
    let p2 = pack.clone();
    let base = fake_modrinth(move |_| {
        let mut r = std::collections::HashMap::new();
        r.insert("GET /pack.zip".to_string(), p2);
        r.insert("GET /web.zip".to_string(), b"<html>no</html>".to_vec());
        r
    });
    let _d = spawn_daemon_with(&e.data, &[("MERC_TEST_LOOPBACK", Path::new("1"))]);
    let mut c = client(&e.data);
    let id = import(&mut c, &e);
    let value = |c: &mut Client, k: &str| {
        let ResponseBody::Properties { entries, .. } = c.call(RequestBody::Properties { id }).unwrap() else {
            panic!()
        };
        entries.into_iter().find(|p| p.key == k).map(|p| p.value)
    };
    // Una página web en lugar de un .zip se rechaza y no cambia nada.
    assert_eq!(
        api_code(c.call(RequestBody::SetResourcePack {
            id,
            url: format!("{base}/web.zip"),
            required: false
        })),
        ErrorCode::BadRequest
    );
    assert_eq!(value(&mut c, "resource-pack"), None);
    c.call(RequestBody::SetResourcePack {
        id,
        url: format!("{base}/pack.zip"),
        required: true,
    })
    .unwrap();
    assert_eq!(value(&mut c, "resource-pack"), Some(format!("{base}/pack.zip")));
    assert_eq!(value(&mut c, "resource-pack-sha1"), Some(sha));
    assert_eq!(value(&mut c, "require-resource-pack").as_deref(), Some("true"));
    // Quitarlo deja las opciones vacías.
    c.call(RequestBody::SetResourcePack {
        id,
        url: String::new(),
        required: true,
    })
    .unwrap();
    assert_eq!(value(&mut c, "resource-pack").as_deref(), Some(""));
    assert_eq!(value(&mut c, "require-resource-pack").as_deref(), Some("false"));
}

#[test]
fn backups_cold_hot_verify_restore() {
    let e = env();
    std::fs::create_dir_all(e.server.join("world/region")).unwrap();
    std::fs::write(e.server.join("world/level.dat"), "original").unwrap();
    std::fs::write(e.server.join("world/region/r.0.0.mca"), vec![3u8; 50_000]).unwrap();
    let _d = spawn_daemon(&e.data);
    let mut c = client(&e.data);
    let id = import(&mut c, &e);
    let list = |c: &mut Client| {
        let ResponseBody::Backups(l) = c.call(RequestBody::Backups { id }).unwrap() else {
            panic!()
        };
        l
    };
    assert!(list(&mut c).items.is_empty());
    assert_eq!(list(&mut c).keep, 10);

    // En frío: consistente y verificada.
    let ResponseBody::Backups(l) = c
        .call(RequestBody::CreateBackup {
            id,
            scope: BackupScope::World,
            note: Some("antes del evento".into()),
        })
        .unwrap()
    else {
        panic!()
    };
    let cold = l.items[0].clone();
    assert!(!cold.hot && cold.consistent && cold.verified == Some(true));
    assert_eq!(cold.note.as_deref(), Some("antes del evento"));

    // En caliente: pausa el guardado, espera la confirmación y lo reanuda.
    c.call(RequestBody::Start { id }).unwrap();
    wait_state(&mut c, id, RuntimeState::Online);
    std::fs::write(e.server.join("world/level.dat"), "segunda").unwrap();
    let ResponseBody::Backups(l) = c
        .call(RequestBody::CreateBackup {
            id,
            scope: BackupScope::World,
            note: None,
        })
        .unwrap()
    else {
        panic!()
    };
    let hot = l.items[0].clone();
    assert!(hot.hot && hot.consistent, "{hot:?}");
    let ResponseBody::Console { lines, .. } = c
        .call(RequestBody::Console {
            id,
            after: 0,
            limit: 2000,
        })
        .unwrap()
    else {
        panic!()
    };
    let text: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
    let pos = |s: &str| {
        text.iter()
            .position(|t| t.contains(s))
            .unwrap_or_else(|| panic!("falta «{s}»"))
    };
    assert!(pos("Automatic saving is now disabled") < pos("Saved the game"));
    assert!(pos("Saved the game") < pos("Automatic saving is now enabled"));
    // Restaurar con el servidor encendido no se permite.
    assert_eq!(
        api_code(c.call(RequestBody::RestoreBackup {
            id,
            backup: cold.id.clone()
        })),
        ErrorCode::InvalidState
    );
    c.call(RequestBody::Stop { id, timeout_secs: 30 }).unwrap();

    std::fs::write(e.server.join("world/level.dat"), "estropeado").unwrap();
    let ResponseBody::Backups(l) = c
        .call(RequestBody::RestoreBackup {
            id,
            backup: cold.id.clone(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(l.restored.as_deref(), Some(cold.id.as_str()));
    assert_eq!(
        std::fs::read_to_string(e.server.join("world/level.dat")).unwrap(),
        "original"
    );
    // Se guardó lo que había antes de restaurar.
    let safety = l
        .items
        .iter()
        .find(|i| i.reason == BackupReason::BeforeRestore)
        .unwrap();
    assert_eq!(l.items.len(), 3);
    assert!(safety.note.as_deref().unwrap().contains(&cold.id));

    // Verificar, proteger (no se puede borrar) y ajustes.
    let ResponseBody::Backups(l) = c
        .call(RequestBody::VerifyBackup {
            id,
            backup: hot.id.clone(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(l.items.iter().find(|i| i.id == hot.id).unwrap().verified, Some(true));
    c.call(RequestBody::ProtectBackup {
        id,
        backup: hot.id.clone(),
        protected: true,
    })
    .unwrap();
    assert_eq!(
        api_code(c.call(RequestBody::DeleteBackup {
            id,
            backup: hot.id.clone()
        })),
        ErrorCode::Conflict
    );
    c.call(RequestBody::DeleteBackup {
        id,
        backup: cold.id.clone(),
    })
    .unwrap();
    assert_eq!(
        api_code(c.call(RequestBody::SetBackupSettings { id, keep: 0 })),
        ErrorCode::BadRequest
    );
    let ResponseBody::Backups(l) = c.call(RequestBody::SetBackupSettings { id, keep: 1 }).unwrap() else {
        panic!()
    };
    assert_eq!(l.keep, 1);
    // Con «conservar 1», la siguiente copia borra las viejas sin proteger, no la protegida.
    let ResponseBody::Backups(l) = c
        .call(RequestBody::CreateBackup {
            id,
            scope: BackupScope::Full,
            note: None,
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(
        l.items.len(),
        2,
        "{:?}",
        l.items.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
    assert!(l.items.iter().any(|i| i.id == hot.id && i.protected));
    assert!(l.items.iter().any(|i| i.scope == BackupScope::Full));
    assert!(l.items.iter().all(|i| i.reason != BackupReason::BeforeRestore));
    let ResponseBody::Activity(act) = c.call(RequestBody::Activity { id, limit: 100 }).unwrap() else {
        panic!()
    };
    assert!(act.iter().any(|a| a.action == "backup.restore"));
}

#[test]
fn scheduled_tasks_run_skip_and_miss() {
    use merc_admin::schedule::*;
    let e = env();
    std::fs::create_dir_all(e.server.join("world")).unwrap();
    std::fs::write(e.server.join("world/level.dat"), "nivel").unwrap();
    let _d = spawn_daemon_with(&e.data, &[("MERC_SCHEDULER_TICK_MS", Path::new("200"))]);
    let mut c = client(&e.data);
    let id = import(&mut c, &e);
    let save = |c: &mut Client, action: TaskAction, schedule: Schedule| {
        let ResponseBody::Tasks(l) = c
            .call(RequestBody::SaveTask {
                id,
                task: TaskDraft {
                    id: None,
                    enabled: true,
                    action,
                    schedule,
                },
            })
            .unwrap()
        else {
            panic!()
        };
        l.items.last().unwrap().id.clone()
    };
    let every = Schedule::Every { minutes: 60 };
    // Validación: «stop» no se programa como comando.
    assert_eq!(
        api_code(c.call(RequestBody::SaveTask {
            id,
            task: TaskDraft {
                id: None,
                enabled: true,
                action: TaskAction::Command { command: "stop".into() },
                schedule: every.clone(),
            },
        })),
        ErrorCode::BadRequest
    );
    let start = save(&mut c, TaskAction::Start, every.clone());
    let missed = save(
        &mut c,
        TaskAction::Announce { message: "hola".into() },
        Schedule::Daily {
            times: vec!["04:00".into()],
            days: vec![],
        },
    );
    let restart = save(
        &mut c,
        TaskAction::Restart {
            notice_seconds: 0,
            if_players: PlayersPolicy::Skip,
        },
        every.clone(),
    );
    let backup = save(
        &mut c,
        TaskAction::Backup {
            scope: BackupScope::World,
        },
        every.clone(),
    );
    let cmd = save(
        &mut c,
        TaskAction::Command {
            command: "/fakejoin Steve".into(),
        },
        every.clone(),
    );

    // Se adelanta la hora de dos tareas: una toca ya y otra «pasó hace una hora».
    let file = e.data.join("instancias").join(id.to_string()).join("tareas.json");
    let mut tasks: Vec<ScheduledTask> = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    for t in &mut tasks {
        if t.id == start {
            t.next_run_unix_ms = Some(now - 1000);
        }
        if t.id == missed {
            t.next_run_unix_ms = Some(now - 3_600_000);
        }
    }
    std::fs::write(&file, serde_json::to_string(&tasks).unwrap()).unwrap();

    let find = |c: &mut Client, tid: &str| -> ScheduledTask {
        let ResponseBody::Tasks(l) = c.call(RequestBody::Tasks { id }).unwrap() else {
            panic!()
        };
        l.items.into_iter().find(|t| t.id == tid).unwrap()
    };
    let wait_run = |c: &mut Client, tid: &str| -> TaskRun {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let t = find(c, tid);
            if let (Some(r), false) = (t.last_run.clone(), t.running) {
                return r;
            }
            assert!(Instant::now() < deadline, "la tarea {tid} no se ejecutó");
            std::thread::sleep(Duration::from_millis(100));
        }
    };
    // El programador enciende el servidor a su hora.
    let r = wait_run(&mut c, &start);
    assert_eq!(r.outcome, RunOutcome::Ok, "{r:?}");
    assert!(!r.manual);
    assert!(find(&mut c, &start).next_run_unix_ms.unwrap() > now + 50 * 60_000);
    wait_state(&mut c, id, RuntimeState::Online);
    // La que se pasó de hora no se ejecuta tarde.
    let r = wait_run(&mut c, &missed);
    assert_eq!(r.outcome, RunOutcome::Skipped);
    assert!(r.message.contains("No se ejecutó"), "{r:?}");

    // Ejecutar ahora: un comando (hace entrar a un jugador de prueba)…
    c.call(RequestBody::RunTaskNow {
        id,
        task_id: cmd.clone(),
    })
    .unwrap();
    assert_eq!(wait_run(&mut c, &cmd).outcome, RunOutcome::Ok);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let ResponseBody::Metrics { players, .. } = c.call(RequestBody::Metrics { id }).unwrap() else {
            panic!()
        };
        if players.contains(&"Steve".to_string()) {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(100));
    }
    // …y con un jugador dentro, el reinicio «si no hay nadie» se omite.
    c.call(RequestBody::RunTaskNow {
        id,
        task_id: restart.clone(),
    })
    .unwrap();
    let r = wait_run(&mut c, &restart);
    assert_eq!(r.outcome, RunOutcome::Skipped);
    assert!(r.message.contains("Steve") && r.manual, "{r:?}");
    assert_eq!(status(&mut c, id).state, RuntimeState::Online);
    // Copia programada en caliente.
    c.call(RequestBody::RunTaskNow {
        id,
        task_id: backup.clone(),
    })
    .unwrap();
    let r = wait_run(&mut c, &backup);
    assert_eq!(r.outcome, RunOutcome::Ok, "{r:?}");
    let ResponseBody::Backups(l) = c.call(RequestBody::Backups { id }).unwrap() else {
        panic!()
    };
    assert_eq!(l.items[0].reason, BackupReason::Scheduled);
    assert!(l.items[0].hot && l.items[0].consistent);
    // Si la copia programada falla, avisa como «copia fallida» (y no además como tarea fallida).
    let bdir = e.data.join("instancias").join(id.to_string()).join("backups");
    let aside = bdir.with_file_name("backups-aparte");
    std::fs::rename(&bdir, &aside).unwrap();
    std::fs::write(&bdir, b"esto no es una carpeta").unwrap();
    c.call(RequestBody::RunTaskNow {
        id,
        task_id: backup.clone(),
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let t = find(&mut c, &backup);
        if !t.running && t.last_run.as_ref().is_some_and(|r| r.outcome == RunOutcome::Failed) {
            break;
        }
        assert!(Instant::now() < deadline, "la copia debía fallar");
        std::thread::sleep(Duration::from_millis(100));
    }
    let ResponseBody::Alerts(a) = c.call(RequestBody::Alerts).unwrap() else {
        panic!()
    };
    use merc_admin::health::AlertKind;
    assert!(
        a.items
            .iter()
            .any(|x| x.kind == AlertKind::BackupFailed && x.title.contains("copia programada")),
        "{:?}",
        a.items
    );
    assert!(!a.items.iter().any(|x| x.kind == AlertKind::TaskFailed));
    std::fs::remove_file(&bdir).unwrap();
    std::fs::rename(&aside, &bdir).unwrap();

    // Desactivar y borrar.
    let mut t = find(&mut c, &backup);
    t.enabled = false;
    let ResponseBody::Tasks(l) = c
        .call(RequestBody::SaveTask {
            id,
            task: TaskDraft {
                id: Some(t.id.clone()),
                enabled: false,
                action: t.action.clone(),
                schedule: t.schedule.clone(),
            },
        })
        .unwrap()
    else {
        panic!()
    };
    let t = l.items.iter().find(|x| x.id == backup).unwrap();
    assert!(!t.enabled && t.next_run_unix_ms.is_none());
    c.call(RequestBody::DeleteTask {
        id,
        task_id: backup.clone(),
    })
    .unwrap();
    let ResponseBody::Tasks(l) = c.call(RequestBody::Tasks { id }).unwrap() else {
        panic!()
    };
    assert_eq!(l.items.len(), 4);
    let ResponseBody::Activity(act) = c.call(RequestBody::Activity { id, limit: 200 }).unwrap() else {
        panic!()
    };
    for a in [
        "task.create",
        "task.run",
        "task.missed",
        "task.skipped",
        "task.run_now",
        "task.delete",
    ] {
        assert!(act.iter().any(|x| x.action == a), "falta {a}");
    }
    c.call(RequestBody::Stop { id, timeout_secs: 30 }).unwrap();
}

#[test]
fn health_tps_and_crash_alerts() {
    let e = env();
    std::fs::rename(e.server.join("server.jar"), e.server.join("paper-1.21.1-130.jar")).unwrap();
    let _d = spawn_daemon_with(&e.data, &[("MERC_HEALTH_TICK_MS", Path::new("200"))]);
    let mut c = client(&e.data);
    let id = import(&mut c, &e);
    let ResponseBody::Health(h) = c.call(RequestBody::Health { id }).unwrap() else {
        panic!()
    };
    assert_eq!(h.tps_command.as_deref(), Some("tps"));
    assert!(h.disks.iter().any(|d| d.what == "server" && d.total_bytes > 0));
    assert!(h.max_memory_mb >= 512);
    // Apagado: no se puede medir.
    assert_eq!(
        api_code(c.call(RequestBody::MeasureTps { id })),
        ErrorCode::InvalidState
    );
    c.call(RequestBody::Start { id }).unwrap();
    wait_state(&mut c, id, RuntimeState::Online);
    let ResponseBody::Tps(t) = c.call(RequestBody::MeasureTps { id }).unwrap() else {
        panic!()
    };
    assert_eq!(
        t.tps,
        vec![
            ("1m".to_string(), 20.0),
            ("5m".to_string(), 19.9),
            ("15m".to_string(), 19.95)
        ]
    );
    // La PC ya se midió.
    let ResponseBody::Health(h) = c.call(RequestBody::Health { id }).unwrap() else {
        panic!()
    };
    assert!(h.pc_memory_total_bytes.unwrap() > 0);

    // Ajustes: sin alertas de discos (para que esta prueba no dependa del disco del equipo).
    let ResponseBody::Alerts(a) = c.call(RequestBody::Alerts).unwrap() else {
        panic!()
    };
    let mut s = a.settings.clone();
    s.disk = false;
    s.resources = false;
    assert_eq!(
        api_code(c.call(RequestBody::SetAlertSettings {
            settings: AlertSettings {
                disk_min_gb: 0,
                ..s.clone()
            }
        })),
        ErrorCode::BadRequest
    );
    c.call(RequestBody::SetAlertSettings { settings: s }).unwrap();
    c.call(RequestBody::MarkAlertsRead { ids: vec![] }).unwrap();

    // Caída: el servidor se cierra solo y queda una alerta crítica.
    c.call(RequestBody::SendCommand {
        id,
        command: "fakecrash".into(),
    })
    .unwrap();
    wait_state(&mut c, id, RuntimeState::Crashed);
    let deadline = Instant::now() + Duration::from_secs(10);
    let alert = loop {
        let ResponseBody::Alerts(a) = c.call(RequestBody::Alerts).unwrap() else {
            panic!()
        };
        if let Some(x) = a.items.iter().find(|x| x.kind == AlertKind::Crash) {
            assert_eq!(a.unread, a.items.iter().filter(|x| !x.read).count() as u32);
            break x.clone();
        }
        assert!(Instant::now() < deadline, "no llegó la alerta de caída");
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(alert.level, AlertLevel::Critical);
    assert_eq!(alert.instance_id.as_deref(), Some(id.to_string().as_str()));
    assert!(!alert.read);
    let ResponseBody::Alerts(a) = c
        .call(RequestBody::MarkAlertsRead {
            ids: vec![alert.id.clone()],
        })
        .unwrap()
    else {
        panic!()
    };
    assert!(a.items.iter().find(|x| x.id == alert.id).unwrap().read);
}

/// Agente de Playit falso: habla el canal local (líneas JSON, versión 2) en un socket.
fn fake_playit_agent(path: &Path, port: u16) -> std::sync::Arc<std::sync::Mutex<Option<String>>> {
    use interprocess::local_socket::{prelude::*, GenericFilePath, ListenerOptions};
    use std::io::{BufRead, BufReader, Write};
    let secret: std::sync::Arc<std::sync::Mutex<Option<String>>> = Default::default();
    let listener = ListenerOptions::new()
        .name(path.to_fs_name::<GenericFilePath>().unwrap())
        .create_sync()
        .unwrap();
    let s2 = secret.clone();
    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            let secret = s2.clone();
            std::thread::spawn(move || {
                let conn = std::sync::Arc::new(conn);
                let mut w = &*conn;
                let _ = writeln!(
                    w,
                    "{}",
                    serde_json::json!({"message_kind":"hello","data":{"protocol":{"ipc_version":2,"capabilities":[]}}})
                );
                let mut r = BufReader::new(&*conn);
                loop {
                    let mut line = String::new();
                    if r.read_line(&mut line).unwrap_or(0) == 0 {
                        return;
                    }
                    let req: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
                    let id = req["request_id"].as_u64().unwrap();
                    let kind = req["request"]["type"].as_str().unwrap().to_string();
                    let response = match kind.as_str() {
                        "get_state" => {
                            let state = if secret.lock().unwrap().is_some() {
                                serde_json::json!({"state":"running","data":{"version":"1.2.0","agent_id":"a","account_status":"verified","start_time":1,
                                    "tunnels":[{"display_address":"127.0.0.1:9","destination":format!("127.0.0.1:{port}"),"is_disabled":false,"disabled_reason":null},
                                               {"display_address":"otro.joinmc.link","destination":"127.0.0.1:1","is_disabled":false,"disabled_reason":null},
                                               {"display_address":"147.185.221.181:25732","destination":"127.0.0.1:24499","is_disabled":false,"disabled_reason":null}],
                                    "pending_tunnels":[],"notices":[],"login_link":null}})
                            } else {
                                serde_json::json!({"state":"waiting_for_secret"})
                            };
                            serde_json::json!({"type":"state","data":state})
                        }
                        "set_secret" => {
                            *secret.lock().unwrap() = Some(req["request"]["secret"].as_str().unwrap().into());
                            serde_json::json!({"type":"set_secret","data":{"accepted":true,"message":null}})
                        }
                        "reset_secret" => {
                            *secret.lock().unwrap() = None;
                            serde_json::json!({"type":"reset_secret","data":{"accepted":true,"message":null}})
                        }
                        "get_account_login_url" => {
                            serde_json::json!({"type":"account_login_url","data":{"login_url":"https://playit.gg/login/guest-account/abc"}})
                        }
                        _ => {
                            serde_json::json!({"type":"error","data":{"code":"invalid_request","message":"no","retryable":false}})
                        }
                    };
                    let _ = writeln!(
                        w,
                        "{}",
                        serde_json::json!({"message_kind":"response","data":{"ipc_version":2,"request_id":id,"response":response}})
                    );
                }
            });
        }
    });
    secret
}

/// API de vinculación falsa: la primera consulta dice «esperando», luego «aprobado».
fn fake_playit_api() -> String {
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let calls = calls.clone();
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
                let mut len = 0;
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                        break;
                    }
                    if let Some(v) = h.to_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0u8; len];
                let _ = reader.read_exact(&mut body);
                let n = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let resp = match path.as_str() {
                    "/claim/setup" if n == 0 => r#"{"status":"success","data":"WaitingForUser"}"#,
                    "/claim/setup" => r#"{"status":"success","data":"UserAccepted"}"#,
                    "/claim/exchange" => r#"{"status":"success","data":{"secret_key":"clave-secreta-de-prueba"}}"#,
                    _ => r#"{"status":"error","data":{}}"#,
                };
                let mut s = stream;
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    resp.len(),
                    resp
                );
            });
        }
    });
    base
}

#[test]
fn playit_link_tunnels_and_address_test() {
    let e = env();
    let sock = e.data.parent().unwrap().join("playit-falso.sock");
    let secret = fake_playit_agent(&sock, e.port);
    let api_base = PathBuf::from(fake_playit_api());
    let _d = spawn_daemon_with(
        &e.data,
        &[("MERC_PLAYIT_SOCKET", &sock), ("MERC_PLAYIT_API", &api_base)],
    );
    let mut c = client(&e.data);
    let id = import(&mut c, &e);
    let info = |c: &mut Client| {
        let ResponseBody::Playit(p) = c.call(RequestBody::PlayitStatus).unwrap() else {
            panic!()
        };
        p
    };
    let p = info(&mut c);
    assert!(p.supported && p.running);
    assert_eq!(p.agent.unwrap().phase, AgentPhase::WaitingForSecret);

    // Vincular: enlace oficial, aprobación y el secreto llega al agente.
    let ResponseBody::Playit(p) = c.call(RequestBody::PlayitLink).unwrap() else {
        panic!()
    };
    let url = p.claim.unwrap().url;
    assert!(
        url.starts_with("https://playit.gg/claim/") && url.len() == "https://playit.gg/claim/".len() + 10,
        "{url}"
    );
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let p = info(&mut c);
        if p.claim.as_ref().is_some_and(|c| c.done) {
            assert_eq!(p.agent.unwrap().phase, AgentPhase::Running);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "no se completó la vinculación: {:?}",
            p.claim
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    assert_eq!(secret.lock().unwrap().as_deref(), Some("clave-secreta-de-prueba"));

    // Conexión del servidor: solo el túnel que lleva a su puerto.
    let ResponseBody::Connection(conn) = c.call(RequestBody::Connection { id }).unwrap() else {
        panic!()
    };
    assert_eq!(conn.port, Some(e.port));
    assert_eq!(conn.tunnels.len(), 1);
    assert_eq!(conn.tunnels[0].display_address, "127.0.0.1:9");
    // Probar la dirección: no hay Minecraft detrás, se anota el fallo (sin inventar éxito).
    let ResponseBody::Connection(conn) = c
        .call(RequestBody::TestAddress {
            id,
            address: "127.0.0.1:9".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert!(conn.tests[0].result.is_none() && conn.tests[0].error.is_some());
    assert_eq!(
        api_code(c.call(RequestBody::TestAddress {
            id,
            address: "no válida;".into()
        })),
        ErrorCode::BadRequest
    );

    // Simple Voice Chat: se detecta por su ficha y se lee su puerto real del archivo.
    assert!(conn.voice.is_none());
    {
        use std::io::Write;
        std::fs::create_dir_all(e.server.join("mods")).unwrap();
        let f = std::fs::File::create(e.server.join("mods/voz.jar")).unwrap();
        let mut z = zip::ZipWriter::new(f);
        z.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(br#"{"schemaVersion":1,"id":"voicechat"}"#).unwrap();
        z.finish().unwrap();
        std::fs::create_dir_all(e.server.join("config/voicechat")).unwrap();
        std::fs::write(
            e.server.join("config/voicechat/voicechat-server.properties"),
            "# Simple Voice Chat server config\nport=24499\nbind_address=\nvoice_host=\n",
        )
        .unwrap();
    }
    let ResponseBody::Connection(conn) = c.call(RequestBody::Connection { id }).unwrap() else {
        panic!()
    };
    let v = conn.voice.clone().unwrap();
    assert!(v.installed && v.enabled);
    assert_eq!(v.port, Some(24499));
    assert_eq!(v.tunnels.len(), 1);
    assert_eq!(v.tunnels[0].display_address, "147.185.221.181:25732");
    let codes: Vec<&str> = v.problems.iter().map(|p| p.code.as_str()).collect();
    assert!(
        codes.contains(&"host_missing") && codes.contains(&"client_mod"),
        "{codes:?}"
    );
    let layer = |conn: &ConnectionInfo, k: &str| conn.layers.iter().find(|l| l.key == k).map(|l| l.state);
    assert_eq!(layer(&conn, "server"), Some(LayerState::Caido));
    assert_eq!(layer(&conn, "agent"), Some(LayerState::Operativo));
    assert_eq!(layer(&conn, "tunnel"), Some(LayerState::Operativo));
    assert_eq!(layer(&conn, "external"), Some(LayerState::Caido));
    assert_eq!(layer(&conn, "voice"), Some(LayerState::Desconocido));
    // Solo se acepta la dirección de un túnel de voz detectado.
    assert_eq!(
        api_code(c.call(RequestBody::SetVoiceConfig {
            id,
            voice_host: Some("inventado.example:1".into()),
            port: None,
        })),
        ErrorCode::BadRequest
    );
    assert_eq!(
        api_code(c.call(RequestBody::SetVoiceConfig {
            id,
            voice_host: None,
            port: Some(e.port),
        })),
        ErrorCode::BadRequest
    );
    let ResponseBody::Connection(conn) = c
        .call(RequestBody::SetVoiceConfig {
            id,
            voice_host: Some("147.185.221.181:25732".into()),
            port: None,
        })
        .unwrap()
    else {
        panic!()
    };
    let text = std::fs::read_to_string(e.server.join("config/voicechat/voicechat-server.properties")).unwrap();
    assert!(text.contains("voice_host=147.185.221.181:25732\n") && text.starts_with("# Simple Voice Chat"));
    let v = conn.voice.unwrap();
    assert!(!v.problems.iter().any(|p| p.level == "error"), "{:?}", v.problems);
    // Quedó una copia del archivo anterior en el historial.
    let ResponseBody::SavedItems(h) = c.call(RequestBody::SavedItems { id }).unwrap() else {
        panic!()
    };
    assert!(
        h.iter().any(|x| x.path.contains("voicechat-server.properties")),
        "{h:?}"
    );

    let ResponseBody::PlayitLoginUrl(u) = c.call(RequestBody::PlayitLoginUrl).unwrap() else {
        panic!()
    };
    assert!(u.starts_with("https://playit.gg/"));
    c.call(RequestBody::PlayitUnlink).unwrap();
    assert!(secret.lock().unwrap().is_none());
    assert_eq!(info(&mut c).agent.unwrap().phase, AgentPhase::WaitingForSecret);
}

/// Dos PCs (dos supervisores) por el canal de compañeros, sin relés (modo local de pruebas).
/// Incluye los intentos de abuso de la especificación: todos fallan en la PC anfitriona.
#[test]
fn companions_between_two_pcs() {
    use merc_ipc::remote::Perm;
    let host_env = env();
    let guest_env = env();
    let local = Path::new("1");
    let _h = spawn_daemon_with(&host_env.data, &[("MERC_REMOTE_LOCAL", local)]);
    let _g = spawn_daemon_with(&guest_env.data, &[("MERC_REMOTE_LOCAL", local)]);
    let mut host = client(&host_env.data);
    let mut guest = client(&guest_env.data);
    let id = import(&mut host, &host_env);
    // Otro servidor del anfitrión al que el compañero NO tiene acceso.
    let other = match host
        .call(RequestBody::ImportInstance {
            name: "Privado".into(),
            path: guest_env.server.clone(),
            java: java(),
            min_memory_mb: 0,
            max_memory_mb: 512,
        })
        .unwrap()
    {
        ResponseBody::Instance(i) => i.id,
        o => panic!("{o:?}"),
    };

    // Sin activar no se puede invitar.
    assert_eq!(
        api_code(host.call(RequestBody::CreateInvite {
            id,
            perms: vec![Perm::Status],
            minutes: 30
        })),
        ErrorCode::InvalidState
    );
    let ResponseBody::Companions(info) = host.call(RequestBody::SetCompanionsEnabled { enabled: true }).unwrap() else {
        panic!()
    };
    assert!(info.enabled && info.fingerprint.is_some());
    let ResponseBody::Invite(inv) = host
        .call(RequestBody::CreateInvite {
            id,
            perms: vec![Perm::Status, Perm::ConsoleRead],
            minutes: 30,
        })
        .unwrap()
    else {
        panic!()
    };
    assert!(inv.code.starts_with("MH1-"));
    // El secreto no se guarda en claro.
    let saved = std::fs::read_to_string(host_env.data.join("companeros/anfitrion.json")).unwrap();
    let secret_part = merc_remote::code::Invite::decode(&inv.code).unwrap().secret;
    assert!(!saved.contains(&merc_remote::hex(&secret_part)));

    // Código mal copiado.
    assert_eq!(
        api_code(guest.call(RequestBody::JoinWithCode {
            code: "MH1-AAAA".into(),
            name: "Shandé".into()
        })),
        ErrorCode::BadRequest
    );
    let ResponseBody::RemoteHosts(hosts) = guest
        .call(RequestBody::JoinWithCode {
            code: inv.code.clone(),
            name: "Shandé".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(hosts.len(), 1);
    let h = hosts[0].host.clone();
    assert_eq!(hosts[0].servers.len(), 1);
    assert_eq!(hosts[0].servers[0].id, id);
    // Un solo uso.
    assert_eq!(
        api_code(guest.call(RequestBody::JoinWithCode {
            code: inv.code.clone(),
            name: "Shandé".into()
        })),
        ErrorCode::Forbidden
    );

    let remote = |c: &mut Client, r: RequestBody| {
        c.call(RequestBody::RemoteCall {
            host: h.clone(),
            request: Box::new(r),
        })
    };
    // Permitido: estado y lista (solo su servidor y sin rutas de la PC del anfitrión).
    assert!(matches!(
        remote(&mut guest, RequestBody::Status { id }).unwrap(),
        ResponseBody::Status(_)
    ));
    let ResponseBody::Instances(list) = remote(&mut guest, RequestBody::ListInstances).unwrap() else {
        panic!()
    };
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].instance.id, id);
    assert_eq!(list[0].instance.root, PathBuf::new());

    // Abusos: sin permiso, otro servidor, IDs inventados, acciones del dueño, rutas raras.
    for r in [
        RequestBody::SendCommand {
            id,
            command: "op Shande".into(),
        },
        RequestBody::Start { id },
        RequestBody::Status { id: other },
        RequestBody::Status { id: InstanceId::new() },
        RequestBody::Shutdown,
        RequestBody::Unregister { id, delete_files: true },
        RequestBody::AcceptEula { id, accepted: true },
        RequestBody::ReadFile {
            id,
            path: "../datos/companeros/llave-dispositivo".into(),
        },
        RequestBody::Companions,
        RequestBody::CreateInvite {
            id,
            perms: vec![Perm::FilesEdit],
            minutes: 30,
        },
        RequestBody::RemoteCall {
            host: h.clone(),
            request: Box::new(RequestBody::Ping),
        },
        RequestBody::Job {
            job_id: "x".into(),
            after: 0,
        },
        RequestBody::SetLaunch {
            id,
            java: Some("C:/Windows/System32/cmd.exe".into()),
            min_memory_mb: None,
            max_memory_mb: None,
        },
    ] {
        assert_eq!(api_code(remote(&mut guest, r.clone())), ErrorCode::Forbidden, "{r:?}");
    }

    // Ampliar permisos: lectura de archivos, pero siempre dentro de su servidor.
    let ResponseBody::Companions(info) = host.call(RequestBody::Companions).unwrap() else {
        panic!()
    };
    let comp = info.companions[0].companion.clone();
    assert_eq!(info.companions[0].name, "Shandé");
    host.call(RequestBody::SetCompanionPerms {
        companion: comp.clone(),
        id,
        perms: vec![Perm::Status, Perm::FilesRead],
    })
    .unwrap();
    assert!(matches!(
        remote(
            &mut guest,
            RequestBody::ReadFile {
                id,
                path: "server.properties".into()
            }
        )
        .unwrap(),
        ResponseBody::TextFile(_)
    ));
    let escape = api_code(remote(
        &mut guest,
        RequestBody::ReadFile {
            id,
            path: "../datos/companeros/llave-dispositivo".into(),
        },
    ));
    assert_ne!(escape, ErrorCode::Forbidden);
    assert!(
        matches!(escape, ErrorCode::BadRequest | ErrorCode::NotFound),
        "{escape:?}"
    );
    // Descargar a esta PC: llega entero y comprobado.
    let dest = guest_env.data.parent().unwrap().join("bajado.properties");
    let ResponseBody::FileDone { path } = guest
        .call(RequestBody::RemoteDownload {
            host: h.clone(),
            item: merc_ipc::remote::RemoteItem::File {
                id,
                path: "server.properties".into(),
            },
            dest: dest.clone(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(PathBuf::from(path), dest);
    assert_eq!(
        std::fs::read(&dest).unwrap(),
        std::fs::read(host_env.server.join("server.properties")).unwrap()
    );
    // Sin permiso de mundos ni de copias, ni archivos fuera del servidor.
    for item in [
        merc_ipc::remote::RemoteItem::World {
            id,
            name: "world".into(),
        },
        merc_ipc::remote::RemoteItem::Backup { id, backup: "x".into() },
        merc_ipc::remote::RemoteItem::File {
            id: other,
            path: "server.properties".into(),
        },
    ] {
        assert_eq!(
            api_code(guest.call(RequestBody::RemoteDownload {
                host: h.clone(),
                item,
                dest: dest.clone()
            })),
            ErrorCode::Forbidden
        );
    }
    assert_ne!(
        api_code(guest.call(RequestBody::RemoteDownload {
            host: h.clone(),
            item: merc_ipc::remote::RemoteItem::File {
                id,
                path: "../datos/companeros/llave-dispositivo".into()
            },
            dest: dest.clone()
        })),
        ErrorCode::Forbidden
    );
    // Subir sin permiso de edición: rechazado.
    let big = guest_env.data.parent().unwrap().join("mapa grande.bin");
    let content: Vec<u8> = (0..2_621_440u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(&big, &content).unwrap();
    assert_eq!(
        api_code(guest.call(RequestBody::RemoteUpload {
            host: h.clone(),
            id,
            source: big.clone(),
            dest_dir: String::new()
        })),
        ErrorCode::Forbidden
    );
    host.call(RequestBody::SetCompanionPerms {
        companion: comp.clone(),
        id,
        perms: vec![Perm::Status, Perm::FilesRead, Perm::FilesEdit],
    })
    .unwrap();
    // Varios trozos de 1 MiB; llega al servidor del amigo y se puede volver a bajar igual.
    guest
        .call(RequestBody::RemoteUpload {
            host: h.clone(),
            id,
            source: big.clone(),
            dest_dir: String::new(),
        })
        .unwrap();
    assert_eq!(std::fs::read(host_env.server.join("mapa grande.bin")).unwrap(), content);
    let back = guest_env.data.parent().unwrap().join("vuelta.bin");
    guest
        .call(RequestBody::RemoteDownload {
            host: h.clone(),
            item: merc_ipc::remote::RemoteItem::File {
                id,
                path: "mapa grande.bin".into(),
            },
            dest: back.clone(),
        })
        .unwrap();
    assert_eq!(std::fs::read(&back).unwrap(), content);
    // Nada queda a medias en la carpeta de datos del anfitrión.
    let left = |d: &str| {
        std::fs::read_dir(host_env.data.join("companeros").join(d))
            .map(|r| r.count())
            .unwrap_or(0)
    };
    assert_eq!(left("salida"), 0);
    assert_eq!(left("entrada"), 0);
    host.call(RequestBody::SetCompanionPerms {
        companion: comp.clone(),
        id,
        perms: vec![Perm::Status, Perm::FilesRead],
    })
    .unwrap();

    // El permiso viejo (consola) ya no vale.
    assert_eq!(
        api_code(remote(
            &mut guest,
            RequestBody::Console {
                id,
                after: 0,
                limit: 10
            }
        )),
        ErrorCode::Forbidden
    );

    // Historial: quién, qué y el resultado (también lo denegado).
    let ResponseBody::Activity(act) = host.call(RequestBody::Activity { id, limit: 200 }).unwrap() else {
        panic!()
    };
    assert!(act
        .iter()
        .any(|a| a.actor == "companero:Shandé" && a.action == "companero.denegado"));
    assert!(act
        .iter()
        .any(|a| a.actor == "companero:Shandé" && a.action == "companero.unido"));

    // Quitar al compañero: la sesión deja de valer al momento.
    host.call(RequestBody::RemoveCompanion { companion: comp }).unwrap();
    assert_eq!(
        api_code(remote(&mut guest, RequestBody::Status { id })),
        ErrorCode::Forbidden
    );

    // Pausar el acceso: ya no se atiende a nadie.
    host.call(RequestBody::SetCompanionsEnabled { enabled: false }).unwrap();
    assert!(guest
        .call(RequestBody::RemoteCall {
            host: h.clone(),
            request: Box::new(RequestBody::Status { id })
        })
        .is_err());
    let ResponseBody::RemoteHosts(hosts) = guest.call(RequestBody::ForgetHost { host: h }).unwrap() else {
        panic!()
    };
    assert!(hosts.is_empty());
}

#[test]
fn diagnose_and_set_launch() {
    use merc_admin::diagnose::FixAction;
    use std::io::Write;
    let e = env();
    let _d = spawn_daemon(&e.data);
    let mut c = client(&e.data);
    let id = import(&mut c, &e);
    let jar = |name: &str, meta: &str, body: &str| {
        std::fs::create_dir_all(e.server.join("mods")).unwrap();
        let f = std::fs::File::create(e.server.join("mods").join(name)).unwrap();
        let mut z = zip::ZipWriter::new(f);
        z.start_file(meta, zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(body.as_bytes()).unwrap();
        z.finish().unwrap();
    };
    // El servidor de prueba no es de mods: los mods se ignoran (y se dice).
    jar("a.jar", "fabric.mod.json", r#"{"id":"a"}"#);
    std::fs::create_dir_all(e.server.join("crash-reports")).unwrap();
    std::fs::write(
        e.server.join("crash-reports/crash-2026-10-30_16.00.00-server.txt"),
        "---- Minecraft Crash Report ----\njava.lang.OutOfMemoryError: Java heap space\n",
    )
    .unwrap();
    let ResponseBody::Diagnosis(d) = c.call(RequestBody::Diagnose { id }).unwrap() else {
        panic!()
    };
    let codes: Vec<&str> = d.findings.iter().map(|f| f.code.as_str()).collect();
    assert!(codes.contains(&"out_of_memory"), "{codes:?}");
    let oom = d.findings.iter().find(|f| f.code == "out_of_memory").unwrap();
    assert!(oom
        .source
        .starts_with("crash-reports/crash-2026-10-30_16.00.00-server.txt"));
    assert!(oom.evidence[0].contains("OutOfMemoryError"));
    assert!(d.sources.iter().any(|s| s.starts_with("mods/")));

    // Memoria: se guarda; Java que no existe o no es absoluto: rechazado.
    let ResponseBody::Instance(i) = c
        .call(RequestBody::SetLaunch {
            id,
            java: None,
            min_memory_mb: None,
            max_memory_mb: Some(1024),
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(i.launch.max_memory_mb, 1024);
    if let Some(FixAction::SetMemory { max_mb }) = oom.actions.first() {
        assert!(*max_mb > 512);
    }
    assert_eq!(
        api_code(c.call(RequestBody::SetLaunch {
            id,
            java: Some("java".into()),
            min_memory_mb: None,
            max_memory_mb: None
        })),
        ErrorCode::BadRequest
    );
    assert_eq!(
        api_code(c.call(RequestBody::SetLaunch {
            id,
            java: None,
            min_memory_mb: None,
            max_memory_mb: Some(100)
        })),
        ErrorCode::BadRequest
    );
    let ResponseBody::Instance(i) = c
        .call(RequestBody::SetLaunch {
            id,
            java: Some(java()),
            min_memory_mb: None,
            max_memory_mb: None,
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(i.launch.java, java());
}

#[test]
fn security_report_uses_real_data() {
    use merc_admin::security::Level;
    let e = env();
    std::fs::write(
        e.server.join("server.properties"),
        format!(
            "server-port={}\nonline-mode=false\nenable-rcon=true\nrcon.password=\n",
            e.port
        ),
    )
    .unwrap();
    std::fs::write(e.server.join("playit.toml"), "secret_key = \"x\"").unwrap();
    let _d = spawn_daemon(&e.data);
    let mut c = client(&e.data);
    let id = import(&mut c, &e);
    let ResponseBody::SecurityReport(r) = c.call(RequestBody::SecurityReport { id }).unwrap() else {
        panic!()
    };
    let level = |code: &str| r.checks.iter().find(|x| x.code == code).map(|x| x.level);
    assert_eq!(level("online_mode"), Some(Level::Peligro));
    assert_eq!(level("rcon"), Some(Level::Peligro));
    assert_eq!(level("secrets"), Some(Level::Peligro));
    assert_eq!(level("backups"), Some(Level::Aviso));
    assert_eq!(level("companions"), Some(Level::Bien));
    assert!(r.public_addresses.is_empty());
    // Con una copia hecha, el aviso de copias desaparece.
    c.call(RequestBody::CreateBackup {
        id,
        scope: merc_admin::backups::BackupScope::Full,
        note: None,
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let ResponseBody::SecurityReport(r) = c.call(RequestBody::SecurityReport { id }).unwrap() else {
            panic!()
        };
        if r.checks.iter().any(|x| x.code == "backups" && x.level == Level::Bien) {
            break;
        }
        assert!(Instant::now() < deadline, "no apareció la copia");
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// Instalador de Java: API de Adoptium falsa y un «JRE» que responde como Java 21.
#[cfg(unix)]
#[test]
fn install_java_from_temurin() {
    use sha2::{Digest, Sha256};
    use std::io::Write;
    let mut zip_bytes = std::io::Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut zip_bytes);
        let opts = zip::write::SimpleFileOptions::default().unix_permissions(0o755);
        w.start_file("jdk-21.0.5+11-jre/bin/java", opts).unwrap();
        w.write_all(b"#!/bin/sh\necho 'openjdk version \"21.0.5\" 2024-10-15' >&2\n")
            .unwrap();
        w.start_file("jdk-21.0.5+11-jre/release", opts).unwrap();
        w.write_all(b"JAVA_VERSION=\"21.0.5\"\n").unwrap();
        w.finish().unwrap();
    }
    let zip_bytes = zip_bytes.into_inner();
    let sha = Sha256::digest(&zip_bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let api = fake_modrinth(|base| {
        let json = format!(
            r#"[{{"binary":{{"image_type":"jre","os":"windows","architecture":"x64","package":{{"link":"{base}/jre.zip","checksum":"{sha}","name":"OpenJDK21U-jre_x64_windows_hotspot_21.0.5_11.zip","size":{}}}}},"release_name":"jdk-21.0.5+11","version":{{"semver":"21.0.5+11"}}}}]"#,
            zip_bytes.len()
        );
        std::collections::HashMap::from([
            ("GET /v3/assets/latest/21/hotspot".to_string(), json.into_bytes()),
            ("GET /jre.zip".to_string(), zip_bytes.clone()),
        ])
    });
    let e = env();
    let api_path = PathBuf::from(api);
    let _d = spawn_daemon_with(&e.data, &[("MERC_TEMURIN_API", &api_path)]);
    let mut c = client(&e.data);
    let ResponseBody::Job(job) = c.call(RequestBody::InstallJava { major: 21 }).unwrap() else {
        panic!()
    };
    let deadline = Instant::now() + Duration::from_secs(30);
    let snap = loop {
        let ResponseBody::Job(s) = c
            .call(RequestBody::Job {
                job_id: job.job_id.clone(),
                after: 0,
            })
            .unwrap()
        else {
            panic!()
        };
        if s.state != JobState::Running {
            break s;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(200));
    };
    assert_eq!(snap.state, JobState::Succeeded, "{:?}", snap);
    assert!(snap.log.iter().any(|l| l.text.contains("SHA-256 correcta")));
    let java = e.data.join("java/jdk-21.0.5+11-jre/bin/java");
    assert!(java.is_file());
    assert!(!e.data.join("java/descargas").read_dir().unwrap().any(|_| true));
    // Aparece entre los Java de la PC, con su versión real.
    let ResponseBody::JavaCandidates(list) = c.call(RequestBody::FindJava).unwrap() else {
        panic!()
    };
    assert!(list.iter().any(|j| j.path == java && j.major == Some(21)), "{list:?}");
    assert_eq!(
        api_code(c.call(RequestBody::InstallJava { major: 3 })),
        ErrorCode::BadRequest
    );
}

fn automation(c: &mut Client, body: RequestBody) -> AutomationInfo {
    match c.call(body).unwrap() {
        ResponseBody::Automation(a) => a,
        other => panic!("{other:?}"),
    }
}

fn activity_has(c: &mut Client, id: InstanceId, action: &str) -> usize {
    match c.call(RequestBody::Activity { id, limit: 200 }).unwrap() {
        ResponseBody::Activity(a) => a.iter().filter(|e| e.action == action).count(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn auto_restart_after_crash_with_limit() {
    let e = env();
    let tick = Path::new("100");
    let wait = Path::new("200");
    let _d = spawn_daemon_with(&e.data, &[("MERC_HEALTH_TICK_MS", tick), ("MERC_AUTORESTART_MS", wait)]);
    let mut c = client(&e.data);
    let id = import(&mut c, &e);

    // Apagado por defecto: una caída no se reinicia sola.
    let a = automation(&mut c, RequestBody::Automation { id });
    assert!(!a.auto_restart && a.max_restarts == 3);
    c.call(RequestBody::Start { id }).unwrap();
    wait_state(&mut c, id, RuntimeState::Online);
    c.call(RequestBody::SendCommand {
        id,
        command: "fakecrash".into(),
    })
    .unwrap();
    wait_state(&mut c, id, RuntimeState::Crashed);
    std::thread::sleep(Duration::from_millis(800));
    assert_eq!(status(&mut c, id).state, RuntimeState::Crashed);

    let a = automation(
        &mut c,
        RequestBody::SetAutomation {
            id,
            auto_restart: Some(true),
            autostart: None,
        },
    );
    assert!(a.auto_restart);
    c.call(RequestBody::Start { id }).unwrap();
    for n in 1..=3 {
        wait_state(&mut c, id, RuntimeState::Online);
        c.call(RequestBody::SendCommand {
            id,
            command: "fakecrash".into(),
        })
        .unwrap();
        wait_state(&mut c, id, RuntimeState::Crashed);
        // Se vuelve a encender solo tras la espera.
        wait_state(&mut c, id, RuntimeState::Online);
        assert_eq!(activity_has(&mut c, id, "server.auto_restart"), n);
    }
    // Cuarta caída en 10 minutos: deja de reintentar.
    c.call(RequestBody::SendCommand {
        id,
        command: "fakecrash".into(),
    })
    .unwrap();
    wait_state(&mut c, id, RuntimeState::Crashed);
    std::thread::sleep(Duration::from_millis(1000));
    assert_eq!(status(&mut c, id).state, RuntimeState::Crashed);
    let a = automation(&mut c, RequestBody::Automation { id });
    assert!(a.gave_up && a.recent_restarts == 3, "{a:?}");
    let alerts = match c.call(RequestBody::Alerts).unwrap() {
        ResponseBody::Alerts(l) => l,
        other => panic!("{other:?}"),
    };
    assert!(
        alerts
            .items
            .iter()
            .any(|al| al.message.contains("ya no se reinicia solo")),
        "{:#?}",
        alerts.items
    );
    // Encenderlo a mano lo rearma.
    c.call(RequestBody::Start { id }).unwrap();
    wait_state(&mut c, id, RuntimeState::Online);
    assert!(!automation(&mut c, RequestBody::Automation { id }).gave_up);
    c.call(RequestBody::Stop { id, timeout_secs: 20 }).unwrap();
    wait_state(&mut c, id, RuntimeState::Stopped);
}

#[test]
fn start_with_windows_turns_on_marked_servers() {
    let e = env();
    let key = e.data.parent().unwrap().join("run-key.txt");
    let id;
    {
        let _d = spawn_daemon_with(&e.data, &[("MERC_FAKE_RUN_KEY", &key)]);
        let mut c = client(&e.data);
        id = import(&mut c, &e);
        let a = automation(&mut c, RequestBody::Automation { id });
        assert!(a.startup_available && !a.startup_enabled && !a.autostart);
        c.call(RequestBody::SetStartup { enabled: true }).unwrap();
        let cmd = std::fs::read_to_string(&key).unwrap();
        assert!(
            cmd.contains("--al-iniciar-windows") && cmd.contains("--data-dir"),
            "{cmd}"
        );
        let a = automation(
            &mut c,
            RequestBody::SetAutomation {
                id,
                auto_restart: None,
                autostart: Some(true),
            },
        );
        assert!(a.startup_enabled && a.autostart);
        c.call(RequestBody::Shutdown).ok();
    }
    std::thread::sleep(Duration::from_millis(500));
    // Windows lo arranca al iniciar sesión: el servidor marcado se enciende sin pedirlo.
    let child = Command::new(env!("CARGO_BIN_EXE_merc-supervisord"))
        .arg("--data-dir")
        .arg(&e.data)
        .arg("--al-iniciar-windows")
        .env("MERC_FAKE_RUN_KEY", &key)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let _d = Daemon(child);
    let mut c = client(&e.data);
    wait_state(&mut c, id, RuntimeState::Online);
    assert_eq!(activity_has(&mut c, id, "server.autostart"), 1);
    c.call(RequestBody::SetStartup { enabled: false }).unwrap();
    assert!(!key.exists());
    c.call(RequestBody::Stop { id, timeout_secs: 20 }).unwrap();
    wait_state(&mut c, id, RuntimeState::Stopped);
}

fn history(c: &mut Client, body: RequestBody) -> Vec<merc_admin::players::PlayerRecord> {
    match c.call(body).unwrap() {
        ResponseBody::PlayerHistory(h) => h,
        other => panic!("{other:?}"),
    }
}

#[test]
fn player_history_notes_and_commands() {
    use merc_admin::players::{GameMode, PlayerCommand};
    let e = env();
    let tick = Path::new("100");
    let _d = spawn_daemon_with(&e.data, &[("MERC_HEALTH_TICK_MS", tick)]);
    let mut c = client(&e.data);
    let id = import(&mut c, &e);
    // Con el servidor apagado no se manda nada.
    assert_eq!(
        api_code(c.call(RequestBody::PlayerCommand {
            id,
            name: "Alex".into(),
            command: PlayerCommand::GameMode {
                mode: GameMode::Creative
            },
        })),
        ErrorCode::InvalidState
    );
    c.call(RequestBody::Start { id }).unwrap();
    wait_state(&mut c, id, RuntimeState::Online);
    c.call(RequestBody::SendCommand {
        id,
        command: "fakejoin Alex".into(),
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let h = history(&mut c, RequestBody::PlayerHistory { id });
        if h.first()
            .is_some_and(|r| r.name == "Alex" && r.online_since_unix.is_some())
        {
            assert_eq!(h[0].sessions, 1);
            break;
        }
        assert!(Instant::now() < deadline, "no se registró la entrada");
        std::thread::sleep(Duration::from_millis(100));
    }
    // Acciones con un jugador conectado: comandos oficiales, validados.
    c.call(RequestBody::PlayerCommand {
        id,
        name: "Alex".into(),
        command: PlayerCommand::GameMode {
            mode: GameMode::Creative,
        },
    })
    .unwrap();
    c.call(RequestBody::PlayerCommand {
        id,
        name: "Alex".into(),
        command: PlayerCommand::Message { text: "secreto".into() },
    })
    .unwrap();
    assert_eq!(
        api_code(c.call(RequestBody::PlayerCommand {
            id,
            name: "Nadie".into(),
            command: PlayerCommand::TeleportCoords { x: 0, y: 64, z: 0 },
        })),
        ErrorCode::InvalidState,
        "no está conectado"
    );
    let acts = match c.call(RequestBody::Activity { id, limit: 50 }).unwrap() {
        ResponseBody::Activity(a) => a,
        other => panic!("{other:?}"),
    };
    assert!(acts.iter().any(|a| a.detail == "gamemode creative Alex"));
    assert!(
        acts.iter().all(|a| !a.detail.contains("secreto")),
        "el texto del mensaje no se guarda"
    );

    c.call(RequestBody::SendCommand {
        id,
        command: "fakeleave Alex".into(),
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let h = history(&mut c, RequestBody::PlayerHistory { id });
        if h[0].online_since_unix.is_none() {
            break;
        }
        assert!(Instant::now() < deadline, "no se registró la salida");
        std::thread::sleep(Duration::from_millis(100));
    }
    // Notas privadas.
    let h = history(
        &mut c,
        RequestBody::SetPlayerNote {
            id,
            name: "Alex".into(),
            note: "Construyó el castillo".into(),
        },
    );
    assert_eq!(h[0].note.as_deref(), Some("Construyó el castillo"));
    assert_eq!(
        api_code(c.call(RequestBody::SetPlayerNote {
            id,
            name: "mal nombre".into(),
            note: "x".into()
        })),
        ErrorCode::BadRequest
    );
    assert!(history(&mut c, RequestBody::ClearPlayerHistory { id }).is_empty());
    c.call(RequestBody::Stop { id, timeout_secs: 20 }).unwrap();
    wait_state(&mut c, id, RuntimeState::Stopped);
}

fn lp_info(c: &mut Client, id: merc_core::InstanceId) -> merc_ipc::LuckPermsInfo {
    match c.call(RequestBody::LuckPerms { id }).unwrap() {
        ResponseBody::LuckPerms(i) => *i,
        other => panic!("{other:?}"),
    }
}

#[test]
fn luckperms_reads_previews_applies_and_verifies() {
    use merc_admin::luckperms::{LpChange, LpTarget, LpTargetKind};
    let e = env();
    let _d = spawn_daemon(&e.data);
    let mut c = client(&e.data);
    let id = import(&mut c, &e);
    // Sin LuckPerms: se dice, sin inventar nada.
    let i = lp_info(&mut c, id);
    assert!(i.install.is_none() && i.snapshot.is_none());

    std::fs::create_dir_all(e.server.join("plugins/LuckPerms")).unwrap();
    std::fs::write(e.server.join("plugins/LuckPerms-Bukkit-5.5.17.jar"), b"").unwrap();
    let i = lp_info(&mut c, id);
    assert_eq!(i.install.as_ref().unwrap().data_dir, "plugins/LuckPerms");
    assert!(i.snapshot.is_none(), "apagado no se puede leer");
    let vip = LpChange::CreateGroup { group: "vip".into() };
    assert_eq!(
        api_code(c.call(RequestBody::LuckPermsApply {
            id,
            change: vip.clone()
        })),
        ErrorCode::InvalidState
    );

    c.call(RequestBody::Start { id }).unwrap();
    wait_state(&mut c, id, RuntimeState::Online);
    let snap = lp_info(&mut c, id).snapshot.expect("datos de LuckPerms");
    assert_eq!(snap.groups[0].name, "admin");
    assert_eq!(snap.groups[0].prefix.as_deref(), Some("&c[Admin] "));
    assert_eq!(snap.users[0].username.as_deref(), Some("Notch"));
    assert_eq!(snap.users[0].parents, vec!["admin"]);
    // El export temporal no queda en la carpeta de LuckPerms.
    assert_eq!(
        std::fs::read_dir(e.server.join("plugins/LuckPerms")).unwrap().count(),
        0
    );

    // Vista previa: comando exacto y explicación, sin cambiar nada.
    let notch = LpTarget {
        kind: LpTargetKind::User,
        name: "069a79f4-44e9-4726-a5be-fca90e38aaf5".into(),
    };
    match c
        .call(RequestBody::LuckPermsPreview {
            id,
            change: LpChange::RemoveParent {
                target: notch.clone(),
                group: "admin".into(),
            },
        })
        .unwrap()
    {
        ResponseBody::LuckPermsPreview(p) => {
            assert_eq!(
                p.command,
                "luckperms user 069a79f4-44e9-4726-a5be-fca90e38aaf5 parent remove admin"
            );
            assert_eq!(p.summary, "Sacar a Notch del grupo «admin».");
        }
        other => panic!("{other:?}"),
    }
    assert!(lp_info(&mut c, id).backups.is_empty());

    let apply = |c: &mut Client, change: LpChange| match c.call(RequestBody::LuckPermsApply { id, change }).unwrap() {
        ResponseBody::LuckPermsApplied(a) => *a,
        other => panic!("{other:?}"),
    };
    let a = apply(&mut c, vip.clone());
    assert!(a.verified, "el export posterior muestra el grupo");
    assert!(a.snapshot.group("vip").is_some());
    let a = apply(
        &mut c,
        LpChange::SetPermission {
            target: LpTarget {
                kind: LpTargetKind::Group,
                name: "vip".into(),
            },
            node: "essentials.fly".into(),
            value: true,
        },
    );
    assert!(a.verified);
    let a = apply(
        &mut c,
        LpChange::AddParent {
            target: notch.clone(),
            group: "vip".into(),
        },
    );
    assert!(a.verified);
    assert_eq!(a.snapshot.user(&notch.name).unwrap().parents, vec!["admin", "vip"]);
    // Sin sentido con los datos actuales: no se manda.
    assert_eq!(
        api_code(c.call(RequestBody::LuckPermsApply { id, change: vip })),
        ErrorCode::Conflict
    );
    assert_eq!(
        api_code(c.call(RequestBody::LuckPermsApply {
            id,
            change: LpChange::CreateGroup {
                group: "x op Steve".into()
            }
        })),
        ErrorCode::BadRequest
    );
    // Una copia antes de cada cambio, legible como export.
    let info = lp_info(&mut c, id);
    assert_eq!(info.backups.len(), 3);
    let first = e
        .data
        .join("instancias")
        .join(id.to_string())
        .join("luckperms-copias")
        .join(info.backups.last().unwrap());
    let before = merc_admin::luckperms::parse_export(&std::fs::read(first).unwrap()).unwrap();
    assert!(
        before.group("vip").is_none(),
        "la primera copia es de antes de crear vip"
    );
    let acts = match c.call(RequestBody::Activity { id, limit: 50 }).unwrap() {
        ResponseBody::Activity(a) => a,
        other => panic!("{other:?}"),
    };
    assert_eq!(acts.iter().filter(|a| a.action == "luckperms.change").count(), 3);
    c.call(RequestBody::Stop { id, timeout_secs: 20 }).unwrap();
    wait_state(&mut c, id, RuntimeState::Stopped);
}

#[test]
fn friend_pack_lists_official_links_and_writes_mrpack() {
    use serde_json::json;
    let e = env();
    std::fs::rename(
        e.server.join("server.jar"),
        e.server
            .join("fabric-server-mc.1.21.1-loader.0.16.5-launcher.1.0.1.jar"),
    )
    .unwrap();
    std::fs::create_dir_all(e.server.join("mods")).unwrap();
    let sodium = b"sodium de prueba".to_vec();
    let lithium = b"lithium de prueba".to_vec();
    std::fs::write(e.server.join("mods/sodium.jar"), &sodium).unwrap();
    std::fs::write(e.server.join("mods/lithium.jar"), &lithium).unwrap();
    std::fs::write(e.server.join("mods/casero.jar"), b"hecho a mano").unwrap();
    let base = fake_modrinth(|_| {
        let v = |id: &str, project: &str, data: &[u8]| {
            json!({
                "id": id, "project_id": project, "name": id, "version_number": "1",
                "version_type": "release", "game_versions": ["1.21.1"], "loaders": ["fabric"],
                "date_published": "2026-10-01T00:00:00Z", "downloads": 1, "changelog": null, "dependencies": [],
                "files": [{ "hashes": { "sha1": sha1_hex(data), "sha512": "ab".repeat(64) },
                            "url": format!("https://cdn.modrinth.com/data/{project}/versions/{id}/{id}.jar"),
                            "filename": format!("{id}.jar"), "primary": true, "size": data.len() }]
            })
        };
        let p = |id: &str, client: &str| {
            json!({ "id": id, "slug": id.to_lowercase(), "title": id, "client_side": client,
                    "server_side": "optional", "license": { "id": "MIT" } })
        };
        let mut r = std::collections::HashMap::new();
        r.insert(
            "POST /v2/version_files".to_string(),
            json!({ sha1_hex(&sodium): v("sod1", "Sodium", &sodium), sha1_hex(&lithium): v("lit1", "Lithium", &lithium) })
                .to_string()
                .into_bytes(),
        );
        r.insert("POST /v2/version_files/update".to_string(), b"{}".to_vec());
        r.insert(
            "GET /v2/projects".to_string(),
            json!([p("Sodium", "required"), p("Lithium", "unsupported")])
                .to_string()
                .into_bytes(),
        );
        r
    });
    let mr = PathBuf::from(format!("{base}/v2"));
    let _d = spawn_daemon_with(&e.data, &[("MERC_SOURCE_MODRINTH", &mr)]);
    let mut c = client(&e.data);
    let id = import(&mut c, &e);

    let plan = match c.call(RequestBody::FriendPack { id }).unwrap() {
        ResponseBody::FriendPack(p) => *p,
        other => panic!("{other:?}"),
    };
    assert_eq!(plan.blocked, None);
    assert_eq!(plan.loader, Some(("fabric-loader".into(), "0.16.5".into())));
    assert_eq!(plan.included.len(), 1);
    assert_eq!(plan.included[0].title, "Sodium");
    assert_eq!(plan.server_only, vec!["Lithium"]);
    assert_eq!(plan.manual.len(), 1);
    assert_eq!(plan.manual[0].file_name, "casero.jar");

    assert_eq!(
        api_code(c.call(RequestBody::ExportFriendPack {
            id,
            dest: "relativo.mrpack".into()
        })),
        ErrorCode::BadRequest
    );
    let dest = e.data.parent().unwrap().join("Para mis amigos.mrpack");
    c.call(RequestBody::ExportFriendPack { id, dest: dest.clone() })
        .unwrap();
    let idx = merc_content::mrpack::read_index(&dest).unwrap();
    assert_eq!(idx.mc_version, "1.21.1");
    assert_eq!(idx.loader.as_deref(), Some("fabric"));
    assert_eq!(idx.loader_version.as_deref(), Some("0.16.5"));
    assert_eq!(idx.files.len(), 1);
    assert_eq!(idx.files[0].path, "mods/sod1.jar");
    assert!(idx.files[0].urls[0].starts_with("https://cdn.modrinth.com/"));
    // Solo el índice: ningún archivo de mod dentro del pack.
    let z = zip::ZipArchive::new(std::fs::File::open(&dest).unwrap()).unwrap();
    assert_eq!(z.len(), 1);
}

#[test]
fn tunnel_down_alert_after_grace_and_recovery() {
    use merc_admin::health::{AlertKind, AlertLevel};
    let e = env();
    let sock = e.data.parent().unwrap().join("playit-tunel.sock");
    let secret = fake_playit_agent(&sock, e.port);
    *secret.lock().unwrap() = Some("vinculado".into());
    let tick = Path::new("100");
    let grace = Path::new("600");
    let _d = spawn_daemon_with(
        &e.data,
        &[
            ("MERC_HEALTH_TICK_MS", tick),
            ("MERC_TUNNEL_GRACE_MS", grace),
            ("MERC_PLAYIT_SOCKET", sock.as_path()),
        ],
    );
    let mut c = client(&e.data);
    let id = import(&mut c, &e);
    let tunnel_alerts = |c: &mut Client| {
        let ResponseBody::Alerts(a) = c.call(RequestBody::Alerts).unwrap() else {
            panic!()
        };
        a.items
            .into_iter()
            .filter(|x| x.kind == AlertKind::TunnelDown)
            .collect::<Vec<_>>()
    };
    c.call(RequestBody::Start { id }).unwrap();
    wait_state(&mut c, id, RuntimeState::Online);
    std::thread::sleep(Duration::from_millis(500));
    assert!(tunnel_alerts(&mut c).is_empty(), "funcionando: sin alertas");
    // Playit deja de estar vinculado: tras la espera, una sola alerta.
    *secret.lock().unwrap() = None;
    let deadline = Instant::now() + Duration::from_secs(10);
    while tunnel_alerts(&mut c).is_empty() {
        assert!(Instant::now() < deadline, "no avisó del túnel caído");
        std::thread::sleep(Duration::from_millis(100));
    }
    std::thread::sleep(Duration::from_millis(800));
    let a = tunnel_alerts(&mut c);
    assert_eq!(a.len(), 1, "{a:?}");
    assert_eq!(a[0].level, AlertLevel::Warning);
    assert!(a[0].message.contains("vinculado"), "{a:?}");
    // Vuelve: aviso de que volvió.
    *secret.lock().unwrap() = Some("vinculado".into());
    let deadline = Instant::now() + Duration::from_secs(10);
    while tunnel_alerts(&mut c).len() < 2 {
        assert!(Instant::now() < deadline, "no avisó de que volvió");
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(tunnel_alerts(&mut c)[0].level, AlertLevel::Info);
    c.call(RequestBody::Stop { id, timeout_secs: 20 }).unwrap();
    wait_state(&mut c, id, RuntimeState::Stopped);
}

fn jar_with(path: &Path, name: &str, body: &str) {
    use std::io::Write;
    let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    z.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
    z.write_all(body.as_bytes()).unwrap();
    z.finish().unwrap();
}

#[test]
fn mod_sides_classify_and_quarantine_reversibly() {
    use merc_admin::modsides::ModSide;
    use serde_json::json;
    let e = env();
    std::fs::rename(
        e.server.join("server.jar"),
        e.server
            .join("fabric-server-mc.1.21.1-loader.0.16.5-launcher.1.0.1.jar"),
    )
    .unwrap();
    let mods = e.server.join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    // Solo cliente según su ficha; solo cliente según Modrinth; y uno normal que lo usa todo.
    jar_with(
        &mods.join("zoomify.jar"),
        "fabric.mod.json",
        r#"{"id":"zoomify","environment":"client"}"#,
    );
    jar_with(&mods.join("minimapa.jar"), "fabric.mod.json", r#"{"id":"minimapa"}"#);
    jar_with(&mods.join("lithium.jar"), "fabric.mod.json", r#"{"id":"lithium"}"#);
    let sha = |p: &Path| sha1_hex(&std::fs::read(p).unwrap());
    let (map_sha, lith_sha) = (sha(&mods.join("minimapa.jar")), sha(&mods.join("lithium.jar")));
    let base = fake_modrinth(move |_| {
        let v = |id: &str, project: &str, h: &str| {
            json!({ "id": id, "project_id": project, "name": id, "version_number": "1", "version_type": "release",
                "game_versions": ["1.21.1"], "loaders": ["fabric"], "date_published": "2026-10-01T00:00:00Z",
                "downloads": 1, "changelog": null, "dependencies": [],
                "files": [{ "hashes": { "sha1": h }, "url": "https://cdn.modrinth.com/x.jar", "filename": "x.jar", "primary": true, "size": 1 }] })
        };
        let p = |id: &str, client: &str, server: &str| json!({ "id": id, "slug": id.to_lowercase(), "title": id, "client_side": client, "server_side": server, "license": { "id": "MIT" } });
        let mut r = std::collections::HashMap::new();
        r.insert(
            "POST /v2/version_files".to_string(),
            json!({ map_sha.clone(): v("m1", "Minimapa", &map_sha), lith_sha.clone(): v("l1", "Lithium", &lith_sha) })
                .to_string()
                .into_bytes(),
        );
        r.insert("POST /v2/version_files/update".to_string(), b"{}".to_vec());
        r.insert(
            "GET /v2/projects".to_string(),
            json!([
                p("Minimapa", "required", "unsupported"),
                p("Lithium", "optional", "required")
            ])
            .to_string()
            .into_bytes(),
        );
        r
    });
    let mr = PathBuf::from(format!("{base}/v2"));
    let _d = spawn_daemon_with(&e.data, &[("MERC_SOURCE_MODRINTH", &mr)]);
    let mut c = client(&e.data);
    let id = import(&mut c, &e);
    let info = |r: ResponseBody| match r {
        ResponseBody::ModSides(i) => *i,
        other => panic!("{other:?}"),
    };
    let i = info(c.call(RequestBody::ModSides { id }).unwrap());
    let side = |f: &str| {
        i.items
            .iter()
            .find(|x| x.file == f)
            .map(|x| (x.side, x.suggested))
            .unwrap()
    };
    assert_eq!(side("zoomify.jar"), (ModSide::ClientOnly, true));
    assert_eq!(side("minimapa.jar"), (ModSide::ClientOnly, true));
    assert_eq!(side("lithium.jar"), (ModSide::Both, false));

    let files = vec!["zoomify.jar".to_string(), "minimapa.jar".to_string()];
    let i = info(
        c.call(RequestBody::QuarantineMods {
            id,
            files: files.clone(),
        })
        .unwrap(),
    );
    assert_eq!(i.quarantined.len(), 2);
    assert!(i.quarantined.iter().all(|q| q.reason == "Solo cliente"));
    assert!(!mods.join("zoomify.jar").exists());
    assert!(e.server.join("mods-cuarentena/zoomify.jar").is_file());
    assert_eq!(i.items.len(), 1, "en mods solo queda lithium");

    // Con el servidor encendido no se mueve nada.
    c.call(RequestBody::Start { id }).unwrap();
    wait_state(&mut c, id, RuntimeState::Online);
    assert_eq!(
        api_code(c.call(RequestBody::RestoreMods {
            id,
            files: vec!["zoomify.jar".into()]
        })),
        ErrorCode::InvalidState
    );
    c.call(RequestBody::Stop { id, timeout_secs: 20 }).unwrap();
    wait_state(&mut c, id, RuntimeState::Stopped);

    let i = info(c.call(RequestBody::RestoreMods { id, files }).unwrap());
    assert!(i.quarantined.is_empty());
    assert!(mods.join("zoomify.jar").is_file() && mods.join("minimapa.jar").is_file());
    let acts = match c.call(RequestBody::Activity { id, limit: 50 }).unwrap() {
        ResponseBody::Activity(a) => a,
        other => panic!("{other:?}"),
    };
    assert!(acts.iter().any(|a| a.action == "mods.quarantine"));
    assert!(acts.iter().any(|a| a.action == "mods.restore"));
}
