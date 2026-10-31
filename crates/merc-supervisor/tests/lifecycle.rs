//! Pruebas de integración del supervisor con un proceso Java REAL (FakeServer.java).
//! FakeServer no es Minecraft: imita su log, su puerto y su respuesta a `stop`.
//! Requiere `java`, `javac` y `jar` en PATH; si faltan, las pruebas fallan con un mensaje claro.

use merc_core::launch::{LaunchProfile, LaunchTarget};
use merc_core::{InstanceId, RuntimeState};
use merc_supervisor::{ConsoleStream, ForceKill, StartSpec, Supervisor, SupervisorError};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

fn which(bin: &str) -> PathBuf {
    let out = Command::new(if cfg!(windows) { "where" } else { "which" })
        .arg(bin)
        .output()
        .expect("which");
    let s = String::from_utf8_lossy(&out.stdout);
    let first = s.lines().next().unwrap_or_else(|| panic!("{bin} no está en PATH"));
    PathBuf::from(first.trim())
}

/// Compila FakeServer.java en un JAR una sola vez por ejecución de tests.
fn fake_jar() -> &'static Path {
    static JAR: OnceLock<PathBuf> = OnceLock::new();
    JAR.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("merc-fake-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/FakeServer.java");
        let ok = Command::new("javac")
            .arg("-encoding")
            .arg("UTF-8")
            .arg("-d")
            .arg(&dir)
            .arg(&src)
            .status()
            .unwrap();
        assert!(ok.success(), "javac falló");
        std::fs::write(dir.join("manifest.txt"), "Main-Class: FakeServer\n").unwrap();
        let jar = dir.join("fake-server.jar");
        let ok = Command::new("jar")
            .current_dir(&dir)
            .args(["cfm", "fake-server.jar", "manifest.txt", "FakeServer.class"])
            .status()
            .unwrap();
        assert!(ok.success(), "jar falló");
        jar
    })
}

fn free_port() -> u16 {
    TcpListener::bind(("0.0.0.0", 0)).unwrap().local_addr().unwrap().port()
}

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    port: u16,
}

fn server_dir(eula: bool) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    // Ruta con tildes y espacios a propósito.
    let root = dir.path().join("Mi Servidor ñandú");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::copy(fake_jar(), root.join("server.jar")).unwrap();
    let port = free_port();
    std::fs::write(
        root.join("server.properties"),
        format!("server-port={port}\nlevel-name=world\n"),
    )
    .unwrap();
    if eula {
        std::fs::write(root.join("eula.txt"), "eula=true\n").unwrap();
    }
    Fixture { _dir: dir, root, port }
}

fn spec(id: InstanceId, root: &Path, mode: &str, readiness: Duration) -> StartSpec {
    StartSpec {
        id,
        root: root.to_path_buf(),
        profile: LaunchProfile {
            java: which("java"),
            min_memory_mb: 0,
            max_memory_mb: 512,
            extra_jvm_args: vec![format!("-Dfake.mode={mode}")],
            target: LaunchTarget::Jar {
                jar: "server.jar".into(),
            },
        },
        mc_version: Some("1.21.1".into()),
        readiness_timeout: readiness,
    }
}

fn sup() -> (tempfile::TempDir, Supervisor) {
    let logs = tempfile::tempdir().unwrap();
    let s = Supervisor::new(logs.path());
    (logs, s)
}

const WAIT: Duration = Duration::from_secs(30);

/// Espera un estado y, si no llega, muestra la consola para diagnosticar.
fn expect(s: &Supervisor, id: InstanceId, want: RuntimeState) {
    let got = s.wait_for(id, WAIT, |st| st == want);
    if got != want {
        let console: Vec<String> = s
            .console_since(id, 0)
            .into_iter()
            .map(|l| format!("{:?} {}", l.stream, l.text))
            .collect();
        panic!(
            "estado {got:?}, se esperaba {want:?}\nhint: {:?}\nconsola:\n{}",
            s.status(id).hint,
            console.join("\n")
        );
    }
}

#[test]
fn start_online_command_stop() {
    let (_l, s) = sup();
    let f = server_dir(true);
    let id = InstanceId::new();
    s.start(spec(id, &f.root, "normal", WAIT)).unwrap();
    expect(&s, id, RuntimeState::Online);
    let st = s.status(id);
    assert!(st.pid.is_some());
    assert_eq!(st.port, Some(f.port));
    assert!(st.online_at_unix_ms.is_some());

    // El puerto lo abrió el proceso Java de verdad.
    assert!(
        TcpListener::bind(("0.0.0.0", f.port)).is_err(),
        "el servidor debería tener el puerto abierto"
    );

    s.send_command(id, "/say hola ñandú").unwrap();
    std::thread::sleep(Duration::from_millis(500));
    let lines = s.console_since(id, 0);
    assert!(
        lines
            .iter()
            .any(|l| l.text.contains("Comando recibido: say hola ñandú")),
        "comando no recibido: {lines:#?}"
    );
    assert!(lines.iter().any(|l| l.text.contains("áéíóú ñ")), "tildes corruptas");

    s.stop(id, WAIT).unwrap();
    let st = s.status(id);
    assert_eq!(st.state, RuntimeState::Stopped);
    let exit = st.last_exit.unwrap();
    assert_eq!(exit.code, Some(0));
    assert!(exit.requested && !exit.forced);
    assert!(s
        .console_since(id, 0)
        .iter()
        .any(|l| l.text.contains("Stopping server")));

    // Log en disco, fuera de la carpeta del servidor.
    let log = std::fs::read_to_string(st.log_file.unwrap()).unwrap();
    assert!(log.contains("Done (1.234s)!"));
    assert!(!f.root.join("console.log").exists());

    // Reconexión incremental: no duplica.
    let all = s.console_since(id, 0);
    let last_seq = all.last().unwrap().seq;
    assert!(s.console_since(id, last_seq).is_empty());
}

#[test]
fn double_start_is_rejected() {
    let (_l, s) = sup();
    let f = server_dir(true);
    let id = InstanceId::new();
    s.start(spec(id, &f.root, "normal", WAIT)).unwrap();
    let err = s.start(spec(id, &f.root, "normal", WAIT)).unwrap_err();
    assert!(matches!(err, SupervisorError::AlreadyRunning(_)), "{err:?}");
    expect(&s, id, RuntimeState::Online);
    assert!(matches!(
        s.start(spec(id, &f.root, "normal", WAIT)),
        Err(SupervisorError::AlreadyRunning(RuntimeState::Online))
    ));
    s.stop(id, WAIT).unwrap();
}

#[test]
fn concurrent_start_only_one_wins() {
    let (_l, s) = sup();
    let f = server_dir(true);
    let id = InstanceId::new();
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let s = s.clone();
            let sp = spec(id, &f.root, "normal", WAIT);
            std::thread::spawn(move || s.start(sp).is_ok())
        })
        .collect();
    let wins = handles.into_iter().map(|h| h.join().unwrap()).filter(|ok| *ok).count();
    assert_eq!(wins, 1, "solo un arranque debe tener éxito");
    expect(&s, id, RuntimeState::Online);
    s.stop(id, WAIT).unwrap();
}

#[test]
fn crash_during_startup() {
    let (_l, s) = sup();
    let f = server_dir(true);
    let id = InstanceId::new();
    s.start(spec(id, &f.root, "crash", WAIT)).unwrap();
    expect(&s, id, RuntimeState::Crashed);
    let st = s.status(id);
    assert_eq!(st.last_exit.as_ref().unwrap().code, Some(1));
    assert!(!st.last_exit.unwrap().requested);
    assert!(st.hint.unwrap().contains("código 1"));
    let lines = s.console_since(id, 0);
    assert!(lines
        .iter()
        .any(|l| l.stream == ConsoleStream::Stderr && l.text.contains("mod defectuoso")));
    // Tras un crash se puede volver a iniciar.
    s.start(spec(id, &f.root, "normal", WAIT)).unwrap();
    expect(&s, id, RuntimeState::Online);
    s.stop(id, WAIT).unwrap();
}

#[test]
fn eula_is_never_accepted_for_the_user() {
    let (_l, s) = sup();
    let f = server_dir(false);
    let id = InstanceId::new();
    assert_eq!(
        s.start(spec(id, &f.root, "normal", WAIT)),
        Err(SupervisorError::EulaNotAccepted)
    );
    assert_eq!(s.status(id).state, RuntimeState::Error);
    assert!(s.status(id).pid.is_none());
    assert!(
        !f.root.join("eula.txt").exists(),
        "MercHosting no debe crear ni modificar eula.txt"
    );
}

#[test]
fn busy_port_is_reported_not_killed() {
    let (_l, s) = sup();
    let f = server_dir(true);
    let _other_program = TcpListener::bind(("0.0.0.0", f.port)).unwrap();
    let id = InstanceId::new();
    assert_eq!(
        s.start(spec(id, &f.root, "normal", WAIT)),
        Err(SupervisorError::PortBusy(f.port))
    );
    assert_eq!(s.status(id).state, RuntimeState::Error);
}

#[test]
fn same_root_or_port_across_instances() {
    let (_l, s) = sup();
    let a = server_dir(true);
    let ida = InstanceId::new();
    s.start(spec(ida, &a.root, "normal", WAIT)).unwrap();
    expect(&s, ida, RuntimeState::Online);

    // Misma carpeta (mundo) desde otra instancia.
    assert_eq!(
        s.start(spec(InstanceId::new(), &a.root, "normal", WAIT)),
        Err(SupervisorError::RootInUse)
    );

    // Otra carpeta con el mismo puerto.
    let b = server_dir(true);
    std::fs::write(b.root.join("server.properties"), format!("server-port={}\n", a.port)).unwrap();
    assert_eq!(
        s.start(spec(InstanceId::new(), &b.root, "normal", WAIT)),
        Err(SupervisorError::PortInUseByInstance(a.port))
    );
    s.stop(ida, WAIT).unwrap();
}

#[test]
fn readiness_timeout_goes_unknown_then_force_kill() {
    let (_l, s) = sup();
    let f = server_dir(true);
    let id = InstanceId::new();
    s.start(spec(id, &f.root, "hang", Duration::from_secs(2))).unwrap();
    expect(&s, id, RuntimeState::Unknown);
    assert!(s.status(id).pid.is_some(), "no debe matarse por no confirmar readiness");
    s.force_kill(id, ForceKill::IUnderstandUnsavedDataMayBeLost).unwrap();
    expect(&s, id, RuntimeState::Stopped);
    assert!(s.status(id).last_exit.unwrap().forced);
}

#[test]
fn stop_timeout_does_not_kill() {
    let (_l, s) = sup();
    let f = server_dir(true);
    let id = InstanceId::new();
    s.start(spec(id, &f.root, "nostop", WAIT)).unwrap();
    expect(&s, id, RuntimeState::Online);
    assert_eq!(s.stop(id, Duration::from_secs(2)), Err(SupervisorError::StopTimeout));
    assert_eq!(s.status(id).state, RuntimeState::Stopping);
    assert!(s.status(id).pid.is_some());
    s.force_kill(id, ForceKill::IUnderstandUnsavedDataMayBeLost).unwrap();
    expect(&s, id, RuntimeState::Stopped);
}

#[test]
fn command_injection_is_rejected() {
    let (_l, s) = sup();
    let f = server_dir(true);
    let id = InstanceId::new();
    s.start(spec(id, &f.root, "normal", WAIT)).unwrap();
    expect(&s, id, RuntimeState::Online);
    assert!(matches!(
        s.send_command(id, "say a\nop atacante"),
        Err(SupervisorError::InvalidCommand(_))
    ));
    assert!(matches!(
        s.send_command(id, "   "),
        Err(SupervisorError::InvalidCommand(_))
    ));
    s.stop(id, WAIT).unwrap();
    assert_eq!(s.send_command(id, "say x"), Err(SupervisorError::NotRunning));
}

#[test]
fn missing_jar_and_bad_java() {
    let (_l, s) = sup();
    let f = server_dir(true);
    std::fs::remove_file(f.root.join("server.jar")).unwrap();
    let id = InstanceId::new();
    assert!(matches!(
        s.start(spec(id, &f.root, "normal", WAIT)),
        Err(SupervisorError::MissingFile(_))
    ));

    let g = server_dir(true);
    let mut sp = spec(InstanceId::new(), &g.root, "normal", WAIT);
    sp.profile.java = "/no/existe/java".into();
    assert!(matches!(s.start(sp), Err(SupervisorError::Java(_))));
}

#[test]
fn restart_is_sequential() {
    let (_l, s) = sup();
    let f = server_dir(true);
    let id = InstanceId::new();
    s.start(spec(id, &f.root, "normal", WAIT)).unwrap();
    expect(&s, id, RuntimeState::Online);
    let pid1 = s.status(id).pid.unwrap();
    s.restart(spec(id, &f.root, "normal", WAIT), WAIT).unwrap();
    expect(&s, id, RuntimeState::Online);
    assert_ne!(s.status(id).pid.unwrap(), pid1);
    s.stop(id, WAIT).unwrap();
}

#[test]
fn real_metrics_and_players() {
    let (_l, s) = sup();
    let f = server_dir(true);
    let id = InstanceId::new();
    s.start(spec(id, &f.root, "normal", WAIT)).unwrap();
    expect(&s, id, RuntimeState::Online);
    s.send_command(id, "fakejoin Steve").unwrap();
    s.send_command(id, "fakejoin Alex").unwrap();
    s.send_command(id, "fakeleave Steve").unwrap();
    s.send_command(id, "say <Troll> Bob joined the game").unwrap();
    std::thread::sleep(Duration::from_millis(4500));
    let (samples, players) = s.metrics(id);
    assert_eq!(players, vec!["Alex".to_string()], "solo Alex sigue conectado");
    assert!(!samples.is_empty(), "debe haber muestras reales tras 4,5 s");
    let last = samples.last().unwrap();
    assert!(
        last.memory_bytes > 10 * 1024 * 1024,
        "un proceso Java usa más de 10 MB: {}",
        last.memory_bytes
    );
    assert!((0.0..=100.0).contains(&last.cpu_percent));
    s.stop(id, WAIT).unwrap();
    let (_, players) = s.metrics(id);
    assert!(players.is_empty(), "al detener no quedan jugadores");
}
