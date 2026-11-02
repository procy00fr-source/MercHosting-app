//! Creación de servidores de extremo a extremo: supervisor real + fuente "oficial" falsa
//! servida en 127.0.0.1. El JAR "descargado" es FakeServer, que imita a Minecraft.

use merc_core::RuntimeState;
use merc_install::Flavor;
use merc_ipc::*;
use sha2::Digest as _;
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn java() -> PathBuf {
    merc_core::java::find_java_candidates()
        .into_iter()
        .next()
        .expect("hace falta Java para estas pruebas")
}

fn fake_jar(dir: &Path) -> Vec<u8> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../merc-supervisor/tests/fixtures/FakeServer.java");
    assert!(Command::new("javac")
        .args(["-encoding", "UTF-8", "-d"])
        .arg(dir)
        .arg(&src)
        .status()
        .unwrap()
        .success());
    std::fs::write(dir.join("manifest.txt"), "Main-Class: FakeServer\n").unwrap();
    assert!(Command::new("jar")
        .current_dir(dir)
        .args(["cfm", "fake.jar", "manifest.txt", "FakeServer.class"])
        .status()
        .unwrap()
        .success());
    std::fs::read(dir.join("fake.jar")).unwrap()
}

type Routes = Arc<Mutex<HashMap<String, Vec<u8>>>>;

fn fake_http(routes: Routes) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let routes = routes.clone();
            std::thread::spawn(move || {
                let mut r = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                let _ = r.read_line(&mut line);
                let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
                loop {
                    let mut h = String::new();
                    if r.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                        break;
                    }
                }
                let body = routes.lock().unwrap().get(&path).cloned();
                let mut s = stream;
                match body {
                    Some(b) => {
                        let _ = write!(
                            s,
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            b.len()
                        );
                        let _ = s.write_all(&b);
                    }
                    None => {
                        let _ = write!(s, "HTTP/1.1 404 No\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    }
                }
            });
        }
    });
    base
}

struct Daemon(Child);
impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
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

fn wait_job(c: &mut Client, id: &str) -> JobSnapshot {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut log = vec![];
    let mut after = 0;
    loop {
        let ResponseBody::Job(j) = c
            .call(RequestBody::Job {
                job_id: id.into(),
                after,
            })
            .unwrap()
        else {
            panic!()
        };
        after = j.next_after;
        log.extend(j.log.iter().map(|l| l.text.clone()));
        if j.state != JobState::Running {
            let mut j = j;
            j.log = log
                .into_iter()
                .enumerate()
                .map(|(i, text)| JobLogLine {
                    seq: i as u64 + 1,
                    text,
                })
                .collect();
            return j;
        }
        assert!(Instant::now() < deadline, "el trabajo no terminó");
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn create_paper_server_then_start_it() {
    let tmp = tempfile::tempdir().unwrap();
    let jar = fake_jar(tmp.path());
    let routes: Routes = Default::default();
    let base = fake_http(routes.clone());
    let sha = sha2::Sha256::digest(&jar)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    {
        let mut r = routes.lock().unwrap();
        r.insert(
            "/paper/v3/projects/paper".into(),
            br#"{"project":{"id":"paper"},"versions":{"26.1":["26.1.2"],"1.21":["1.21.11","1.21.10"]}}"#.to_vec(),
        );
        r.insert(
            "/paper/v3/projects/paper/versions/1.21.11/builds".into(),
            serde_json::json!([{"id": 42, "channel": "STABLE", "downloads": {"server:default": {
                "name": "paper-1.21.11-42.jar", "url": format!("{base}/dl/paper.jar"),
                "checksums": {"sha256": sha}, "size": jar.len()}}}])
            .to_string()
            .into_bytes(),
        );
        r.insert(
            "/paper/v3/projects/paper/versions/26.1.2/builds".into(),
            serde_json::json!([{"id": 3, "channel": "STABLE", "downloads": {"server:default": {
                "name": "paper-26.1.2-3.jar", "url": format!("{base}/dl/paper.jar"),
                "checksums": {"sha256": sha}, "size": jar.len()}}}])
            .to_string()
            .into_bytes(),
        );
        r.insert("/dl/paper.jar".into(), jar.clone());
    }
    let data = tmp.path().join("datos");
    let servers = tmp.path().join("Mis servidores");
    let _d = Daemon(
        Command::new(env!("CARGO_BIN_EXE_merc-supervisord"))
            .arg("--data-dir")
            .arg(&data)
            .env("MERC_SOURCE_PAPER", format!("{base}/paper"))
            .env("MERC_SERVERS_DIR", &servers)
            .env("MERC_HEALTH_TICK_MS", "100")
            .env("MERC_UPDATE_CHECK_MS", "300")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut c = client(&data);

    let ResponseBody::Catalog(list) = c.call(RequestBody::Catalog { flavor: Flavor::Paper }).unwrap() else {
        panic!()
    };
    assert_eq!(list[0].mc_version, "26.1.2");
    assert_eq!(list[0].java_major, 25);
    let ResponseBody::CreationDefaults {
        servers_dir,
        suggested_port,
        ..
    } = c.call(RequestBody::CreationDefaults).unwrap()
    else {
        panic!()
    };
    assert_eq!(servers_dir, servers);
    assert!(suggested_port >= 25565);

    let port = TcpListener::bind(("0.0.0.0", 0)).unwrap().local_addr().unwrap().port();
    let mut properties = BTreeMap::new();
    properties.insert("server-port".to_string(), port.to_string());
    properties.insert("motd".to_string(), "Servidor de Ñandú".to_string());
    let target = servers.join("Mi primer server");
    let create = |c: &mut Client, mc: &str, path: PathBuf, props: BTreeMap<String, String>| {
        c.call(RequestBody::CreateServer {
            name: "Mi primer server".into(),
            flavor: Flavor::Paper,
            mc_version: mc.into(),
            path,
            java: java(),
            min_memory_mb: 0,
            max_memory_mb: 512,
            properties: props,
            accept_eula: true,
            modpack_version: None,
            modpack_source: ContentSource::Modrinth,
            loader_version: None,
            game_rules: [("keep_inventory".to_string(), "true".to_string())].into(),
        })
    };

    // Una opción fuera de la lista permitida se rechaza antes de crear nada.
    let mut bad = properties.clone();
    bad.insert("enable-rcon".into(), "true".into());
    assert!(
        matches!(create(&mut c, "1.21.11", target.clone(), bad), Err(ClientError::Api(e)) if e.code == ErrorCode::BadRequest)
    );
    assert!(!target.exists());

    // Java 21 no sirve para 26.1.2 (pide Java 25): falla sin dejar nada.
    let ResponseBody::Job(j) = create(&mut c, "26.1.2", target.clone(), properties.clone()).unwrap() else {
        panic!()
    };
    let j = wait_job(&mut c, &j.job_id);
    if merc_core::java::probe_java(&java(), Duration::from_secs(10))
        .unwrap()
        .major
        < 25
    {
        assert_eq!(j.state, JobState::Failed);
        assert!(j.error.as_deref().unwrap().contains("Java 25"), "{:?}", j.error);
        assert!(!target.exists());
    }

    let ResponseBody::Job(j) = create(&mut c, "1.21.11", target.clone(), properties).unwrap() else {
        panic!()
    };
    let j = wait_job(&mut c, &j.job_id);
    assert_eq!(j.state, JobState::Succeeded, "{:?}\n{:#?}", j.error, j.log);
    assert!(j.log.iter().any(|l| l.text.contains("SHA-256")));
    let id = j.instance_id.unwrap();

    let ResponseBody::Instances(list) = c.call(RequestBody::ListInstances).unwrap() else {
        panic!()
    };
    let s = list.iter().find(|s| s.instance.id == id).unwrap();
    assert_eq!(s.instance.kind, merc_core::detect::ServerKind::Paper);
    assert_eq!(s.instance.mc_version.as_deref(), Some("1.21.11"));
    assert!(!s.instance.imported);
    assert!(s.eula_accepted);
    assert_eq!(s.configured_port, Some(port));

    // Ya no está vacía: crear otra vez ahí se rechaza.
    assert!(matches!(
        c.call(RequestBody::CheckTarget { path: target.clone() }),
        Err(ClientError::Api(e)) if e.code == ErrorCode::Conflict
    ));

    c.call(RequestBody::Start { id }).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let ResponseBody::Status(st) = c.call(RequestBody::Status { id }).unwrap() else {
            panic!()
        };
        if st.state == RuntimeState::Online {
            break;
        }
        assert!(Instant::now() < deadline, "{st:?}");
        std::thread::sleep(Duration::from_millis(100));
    }
    // La regla elegida al crear se aplica la primera vez que queda en línea (y solo esa vez).
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let ResponseBody::Activity(a) = c.call(RequestBody::Activity { id, limit: 50 }).unwrap() else {
            panic!()
        };
        if let Some(e) = a.iter().find(|e| e.action.starts_with("world.gamerule")) {
            assert_eq!(e.action, "world.gamerule", "{e:?}");
            assert!(
                e.detail.starts_with("gamerule keep_inventory true"),
                "1.21.11 usa los nombres nuevos: {e:?}"
            );
            break;
        }
        assert!(Instant::now() < deadline, "no se aplicó la regla pendiente");
        std::thread::sleep(Duration::from_millis(100));
    }
    c.call(RequestBody::Stop { id, timeout_secs: 30 }).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let ResponseBody::Status(st) = c.call(RequestBody::Status { id }).unwrap() else {
            panic!()
        };
        if st.state == RuntimeState::Stopped {
            break;
        }
        assert!(Instant::now() < deadline, "{st:?}");
        std::thread::sleep(Duration::from_millis(100));
    }

    // Centro de actualizaciones: sale la compilación 50 de la misma versión.
    let ResponseBody::ServerUpdate(u) = c.call(RequestBody::ServerUpdateCheck { id }).unwrap() else {
        panic!()
    };
    assert!(u.supported && !u.newer, "{u:?}");
    assert_eq!(u.current.as_deref(), Some("42"));
    routes.lock().unwrap().insert(
        "/paper/v3/projects/paper/versions/1.21.11/builds".into(),
        serde_json::json!([{"id": 50, "channel": "STABLE", "downloads": {"server:default": {
            "name": "paper-1.21.11-50.jar", "url": format!("{base}/dl/paper.jar"),
            "checksums": {"sha256": sha}, "size": jar.len()}}}])
        .to_string()
        .into_bytes(),
    );
    let ResponseBody::ServerUpdate(u) = c.call(RequestBody::ServerUpdateCheck { id }).unwrap() else {
        panic!()
    };
    assert!(u.newer, "{u:?}");
    assert_eq!(u.latest.as_deref(), Some("50"));
    // La vigilancia de actualizaciones avisa una sola vez de la compilación 50.
    let alerts_of = |c: &mut merc_ipc::Client| {
        let ResponseBody::Alerts(a) = c.call(RequestBody::Alerts).unwrap() else {
            panic!()
        };
        a.items
            .into_iter()
            .filter(|x| x.kind == merc_admin::health::AlertKind::UpdateAvailable)
            .collect::<Vec<_>>()
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    while alerts_of(&mut c).is_empty() {
        assert!(Instant::now() < deadline, "no avisó de la actualización");
        std::thread::sleep(Duration::from_millis(100));
    }
    std::thread::sleep(Duration::from_millis(1000));
    let ups = alerts_of(&mut c);
    assert_eq!(ups.len(), 1, "{ups:?}");
    assert!(ups[0].message.starts_with("42 → 50"), "{:?}", ups[0]);
    // Pedir otra versión que la ofrecida se rechaza sin tocar nada.
    let ResponseBody::Job(j) = c
        .call(RequestBody::ApplyServerUpdate {
            id,
            loader_version: "49".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(wait_job(&mut c, &j.job_id).state, JobState::Failed);
    let ResponseBody::Job(j) = c
        .call(RequestBody::ApplyServerUpdate {
            id,
            loader_version: "50".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    let j = wait_job(&mut c, &j.job_id);
    assert_eq!(j.state, JobState::Succeeded, "{:?}\n{:#?}", j.error, j.log);
    assert!(target.join("paper-1.21.11-50.jar").is_file());
    assert!(
        !target.join("paper-1.21.11-42.jar").exists(),
        "el jar viejo va a la papelera"
    );
    let ResponseBody::Backups(b) = c.call(RequestBody::Backups { id }).unwrap() else {
        panic!()
    };
    assert!(b
        .items
        .iter()
        .any(|b| b.reason == merc_admin::backups::BackupReason::BeforeUpdate));
    let ResponseBody::ServerUpdate(u) = c.call(RequestBody::ServerUpdateCheck { id }).unwrap() else {
        panic!()
    };
    assert_eq!(u.current.as_deref(), Some("50"));
    assert!(!u.newer);

    // Icono: una foto JPG de 640×360 queda como server-icon.png de 64×64.
    let photo = tmp.path().join("foto.jpg");
    image::RgbImage::from_pixel(640, 360, image::Rgb([20, 120, 220]))
        .save(&photo)
        .unwrap();
    let ResponseBody::IconPreview(p) = c
        .call(RequestBody::PreviewServerIcon {
            source: photo.to_string_lossy().into(),
            fit: Default::default(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!((p.source_width, p.source_height, p.format.as_str()), (640, 360, "JPG"));
    assert!(
        !target.join("server-icon.png").exists(),
        "la vista previa no guarda nada"
    );
    for _ in 0..2 {
        let ResponseBody::ServerIcon(i) = c
            .call(RequestBody::SetServerIcon {
                id,
                source: photo.to_string_lossy().into(),
                fit: merc_admin::icon::IconFit::Contain,
            })
            .unwrap()
        else {
            panic!()
        };
        assert!(i.valid && i.width == Some(64) && i.png_base64.is_some(), "{i:?}");
    }
    let saved = image::open(target.join("server-icon.png")).unwrap();
    assert_eq!((saved.width(), saved.height()), (64, 64));
    let ResponseBody::ServerIcon(i) = c.call(RequestBody::RemoveServerIcon { id }).unwrap() else {
        panic!()
    };
    assert!(!i.present);
    assert!(matches!(
        c.call(RequestBody::PreviewServerIcon { source: "relativa.png".into(), fit: Default::default() }),
        Err(ClientError::Api(e)) if e.code == ErrorCode::BadRequest
    ));

    // Clonar: copia completa en una carpeta nueva, con otro puerto y sin registros del original.
    std::fs::create_dir_all(target.join("logs")).unwrap();
    std::fs::write(target.join("logs/latest.log"), "viejo").unwrap();
    let ResponseBody::Job(j) = c
        .call(RequestBody::CloneServer {
            id,
            name: "Mi primer server (copia)".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    let j = wait_job(&mut c, &j.job_id);
    assert_eq!(j.state, JobState::Succeeded, "{:?}\n{:#?}", j.error, j.log);
    let copy_id = j.instance_id.unwrap();
    let ResponseBody::Instances(list) = c.call(RequestBody::ListInstances).unwrap() else {
        panic!()
    };
    let copy = list.iter().find(|s| s.instance.id == copy_id).unwrap();
    assert_eq!(copy.instance.root, servers.join("Mi primer server (copia)"));
    assert!(copy.instance.root.join("paper-1.21.11-50.jar").is_file());
    assert!(!copy.instance.root.join("logs").exists());
    assert_ne!(copy.configured_port, Some(port), "la copia usa otro puerto");
    assert_eq!(copy.instance.loader_version.as_deref(), Some("50"));

    // Archivar: no se puede encender hasta desarchivar.
    c.call(RequestBody::SetArchived {
        id: copy_id,
        archived: true,
    })
    .unwrap();
    assert!(matches!(
        c.call(RequestBody::Start { id: copy_id }),
        Err(ClientError::Api(e)) if e.code == ErrorCode::InvalidState
    ));
    let ResponseBody::Instances(list) = c.call(RequestBody::ListInstances).unwrap() else {
        panic!()
    };
    assert!(list.iter().find(|s| s.instance.id == copy_id).unwrap().archived);
    c.call(RequestBody::SetArchived {
        id: copy_id,
        archived: false,
    })
    .unwrap();
    // Una regla inventada al crear se rechaza antes de empezar.
    assert!(matches!(
        c.call(RequestBody::CreateServer {
            name: "x".into(),
            flavor: Flavor::Paper,
            mc_version: "1.21.11".into(),
            path: servers.join("otro"),
            java: java(),
            min_memory_mb: 0,
            max_memory_mb: 512,
            properties: BTreeMap::new(),
            accept_eula: true,
            modpack_version: None,
            modpack_source: ContentSource::Modrinth,
            loader_version: None,
            game_rules: [("borrar_mundo".to_string(), "true".to_string())].into(),
        }),
        Err(ClientError::Api(e)) if e.code == ErrorCode::BadRequest
    ));
}

#[test]
fn offline_source_and_cancel() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("datos");
    // Nada escucha en ese puerto: simula estar sin Internet.
    let dead = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let _d = Daemon(
        Command::new(env!("CARGO_BIN_EXE_merc-supervisord"))
            .arg("--data-dir")
            .arg(&data)
            .env("MERC_SOURCE_FABRIC", format!("http://127.0.0.1:{dead}/fabric"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut c = client(&data);
    assert!(matches!(
        c.call(RequestBody::Catalog { flavor: Flavor::Fabric }),
        Err(ClientError::Api(e)) if e.code == ErrorCode::Network
    ));
    let target = tmp.path().join("nuevo");
    let ResponseBody::Job(j) = c
        .call(RequestBody::CreateServer {
            name: "X".into(),
            flavor: Flavor::Fabric,
            mc_version: "1.21.11".into(),
            path: target.clone(),
            java: java(),
            min_memory_mb: 0,
            max_memory_mb: 1024,
            properties: BTreeMap::new(),
            accept_eula: false,
            modpack_version: None,
            modpack_source: ContentSource::Modrinth,
            loader_version: None,
            game_rules: Default::default(),
        })
        .unwrap()
    else {
        panic!()
    };
    let _ = c.call(RequestBody::CancelJob {
        job_id: j.job_id.clone(),
    });
    let j = wait_job(&mut c, &j.job_id);
    assert!(matches!(j.state, JobState::Failed | JobState::Cancelled), "{j:?}");
    assert!(!target.exists());
    assert!(matches!(
        c.call(RequestBody::Job { job_id: "no-existe".into(), after: 0 }),
        Err(ClientError::Api(e)) if e.code == ErrorCode::NotFound
    ));
}

fn sha1_hex(b: &[u8]) -> String {
    use sha1::Digest as _;
    sha1::Sha1::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn create_from_modrinth_modpack() {
    use serde_json::json;
    use std::io::Write as _;
    let tmp = tempfile::tempdir().unwrap();
    let jar = fake_jar(tmp.path());
    let routes: Routes = Default::default();
    let base = fake_http(routes.clone());
    let modjar = b"mod del modpack".to_vec();
    let index = json!({
        "formatVersion": 1, "game": "minecraft", "versionId": "1.0", "name": "Aventura",
        "dependencies": { "minecraft": "1.21.1", "fabric-loader": "0.16.5" },
        "files": [
            { "path": "mods/a.jar", "hashes": { "sha1": sha1_hex(&modjar) }, "env": { "server": "required" },
              "downloads": [format!("{base}/dl/a.jar")], "fileSize": modjar.len() },
            { "path": "mods/solo-cliente.jar", "hashes": { "sha1": "b".repeat(40) }, "env": { "server": "unsupported" },
              "downloads": [format!("{base}/dl/no.jar")], "fileSize": 3 }
        ]
    });
    let mut pack = vec![];
    {
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut pack));
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("modrinth.index.json", o).unwrap();
        z.write_all(index.to_string().as_bytes()).unwrap();
        z.start_file("overrides/config/x.toml", o).unwrap();
        z.write_all(b"valor = 1").unwrap();
        z.start_file("overrides/eula.txt", o).unwrap();
        z.write_all(b"eula=true").unwrap();
        z.finish().unwrap();
    }
    {
        let mut r = routes.lock().unwrap();
        r.insert(
            "/fabric/v2/versions/installer".into(),
            br#"[{"version":"1.1.0","stable":true}]"#.to_vec(),
        );
        r.insert(
            "/fabric/v2/versions/loader/1.21.1/0.16.5/1.1.0/server/jar".into(),
            jar.clone(),
        );
        r.insert(
            "/mr/v2/version/packv1".into(),
            json!({ "id": "packv1", "project_id": "PACK", "name": "Aventura 1.0", "version_number": "1.0",
                "version_type": "release", "game_versions": ["1.21.1"], "loaders": ["fabric"],
                "files": [{ "url": format!("{base}/dl/aventura.mrpack"), "filename": "aventura-1.0.mrpack",
                            "primary": true, "size": pack.len(), "hashes": { "sha1": sha1_hex(&pack) } }] })
            .to_string()
            .into_bytes(),
        );
        r.insert("/dl/aventura.mrpack".into(), pack.clone());
        r.insert("/dl/a.jar".into(), modjar.clone());
    }
    let data = tmp.path().join("datos");
    let _d = Daemon(
        Command::new(env!("CARGO_BIN_EXE_merc-supervisord"))
            .arg("--data-dir")
            .arg(&data)
            .env("MERC_SOURCE_FABRIC", format!("{base}/fabric"))
            .env("MERC_SOURCE_MODRINTH", format!("{base}/mr/v2"))
            .env("MERC_TEST_LOOPBACK", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut c = client(&data);
    let ResponseBody::ModpackSummary(s) = c
        .call(RequestBody::ModpackInspect {
            source: ContentSource::Modrinth,
            version_id: "packv1".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!((s.flavor, s.mc_version.as_str()), (Flavor::Fabric, "1.21.1"));
    assert_eq!((s.server_files, s.client_only_files), (1, 1));
    let ResponseBody::Resolved(r) = c
        .call(RequestBody::Resolve {
            flavor: Flavor::Fabric,
            mc_version: "1.21.1".into(),
            loader_version: s.loader_version.clone(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(r.loader_version.as_deref(), Some("0.16.5"));

    let target = tmp.path().join("Aventura");
    let ResponseBody::Job(job) = c
        .call(RequestBody::CreateServer {
            name: "Aventura".into(),
            flavor: Flavor::Fabric,
            mc_version: "1.21.1".into(),
            path: target.clone(),
            java: java(),
            min_memory_mb: 0,
            max_memory_mb: 1024,
            properties: BTreeMap::new(),
            accept_eula: false,
            modpack_version: Some("packv1".into()),
            modpack_source: ContentSource::Modrinth,
            loader_version: None,
            game_rules: Default::default(),
        })
        .unwrap()
    else {
        panic!()
    };
    let done = wait_job(&mut c, &job.job_id);
    assert_eq!(done.state, JobState::Succeeded, "{:?}", done.log);
    assert_eq!(std::fs::read(target.join("mods/a.jar")).unwrap(), modjar);
    assert!(!target.join("mods/solo-cliente.jar").exists());
    assert_eq!(
        std::fs::read_to_string(target.join("config/x.toml")).unwrap(),
        "valor = 1"
    );
    assert!(
        !std::fs::read_to_string(target.join("eula.txt"))
            .unwrap_or_default()
            .contains("eula=true"),
        "la EULA del modpack no se copia"
    );
    assert!(done.log.iter().any(|l| l.text.contains("Modpack «Aventura»")));
    assert!(!data
        .join("cache")
        .join("modpack-staging")
        .read_dir()
        .map(|mut d| d.next().is_some())
        .unwrap_or(false));
}

#[test]
fn create_from_curseforge_modpack() {
    use serde_json::json;
    use std::io::Write as _;
    let tmp = tempfile::tempdir().unwrap();
    let jar = fake_jar(tmp.path());
    let routes: Routes = Default::default();
    let base = fake_http(routes.clone());
    let modjar = b"mod de curseforge".to_vec();
    let manifest = json!({
        "manifestType": "minecraftModpack", "manifestVersion": 1, "name": "Pack CF", "version": "3.0",
        "minecraft": { "version": "1.21.1", "modLoaders": [{ "id": "fabric-0.16.5", "primary": true }] },
        "files": [
            { "projectID": 100, "fileID": 1001, "required": true },
            { "projectID": 200, "fileID": 2001, "required": true },
            { "projectID": 300, "fileID": 3001, "required": true }
        ],
        "overrides": "overrides"
    });
    let mut pack = vec![];
    {
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut pack));
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("manifest.json", o).unwrap();
        z.write_all(manifest.to_string().as_bytes()).unwrap();
        z.start_file("overrides/config/cf.toml", o).unwrap();
        z.write_all(b"cf = true").unwrap();
        z.finish().unwrap();
    }
    let file = |id: u64, mod_id: u64, name: &str, data: &[u8], url: bool| {
        json!({ "id": id, "modId": mod_id, "displayName": name, "fileName": name, "releaseType": 1,
            "fileDate": "2026-10-01T00:00:00Z", "fileLength": data.len(),
            "downloadUrl": if url { json!(format!("{base}/dl/{name}")) } else { json!(null) },
            "gameVersions": ["1.21.1", "Fabric", "Server"], "hashes": [{ "value": sha1_hex(data), "algo": 1 }] })
    };
    let module = |id: u64, class: u64, allow: bool| {
        json!({ "id": id, "name": format!("Proyecto {id}"), "slug": format!("p{id}"), "classId": class,
            "allowModDistribution": allow, "links": { "websiteUrl": format!("https://www.curseforge.com/minecraft/mc-mods/p{id}") } })
    };
    {
        let mut r = routes.lock().unwrap();
        r.insert(
            "/fabric/v2/versions/installer".into(),
            br#"[{"version":"1.1.0","stable":true}]"#.to_vec(),
        );
        r.insert(
            "/fabric/v2/versions/loader/1.21.1/0.16.5/1.1.0/server/jar".into(),
            jar.clone(),
        );
        // La API falsa no mira el cuerpo: devuelve todos los archivos y el cliente busca el suyo.
        r.insert(
            "/cf/v1/mods/files".into(),
            json!({ "data": [
                file(9001, 900, "pack-cf-3.0.zip", &pack, true),
                file(1001, 100, "bueno.jar", &modjar, true),
                file(2001, 200, "restringido.jar", b"x", false),
                file(3001, 300, "texturas.zip", b"y", true)
            ] })
            .to_string()
            .into_bytes(),
        );
        r.insert(
            "/cf/v1/mods".into(),
            json!({ "data": [module(100, 6, true), module(200, 6, false), module(300, 12, true)] })
                .to_string()
                .into_bytes(),
        );
        r.insert("/dl/pack-cf-3.0.zip".into(), pack.clone());
        r.insert("/dl/bueno.jar".into(), modjar.clone());
    }
    let data = tmp.path().join("datos");
    let _d = Daemon(
        Command::new(env!("CARGO_BIN_EXE_merc-supervisord"))
            .arg("--data-dir")
            .arg(&data)
            .env("MERC_SOURCE_FABRIC", format!("{base}/fabric"))
            .env("MERC_SOURCE_CURSEFORGE", format!("{base}/cf"))
            .env("MERC_CURSEFORGE_API_KEY", "clave-de-prueba")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut c = client(&data);
    let ResponseBody::CreationDefaults { curseforge, .. } = c.call(RequestBody::CreationDefaults).unwrap() else {
        panic!()
    };
    assert!(curseforge);
    let ResponseBody::ModpackSummary(s) = c
        .call(RequestBody::ModpackInspect {
            source: ContentSource::Curseforge,
            version_id: "9001".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(
        (s.flavor, s.loader_version.as_deref()),
        (Flavor::Fabric, Some("0.16.5"))
    );
    assert_eq!(s.server_files, 1);
    assert!(s.warnings[0].contains("Proyecto 200"), "{:?}", s.warnings);
    assert!(s.warnings.iter().any(|w| w.contains("resource packs")));

    let target = tmp.path().join("PackCF");
    let ResponseBody::Job(job) = c
        .call(RequestBody::CreateServer {
            name: "Pack CF".into(),
            flavor: Flavor::Fabric,
            mc_version: "1.21.1".into(),
            path: target.clone(),
            java: java(),
            min_memory_mb: 0,
            max_memory_mb: 1024,
            properties: BTreeMap::new(),
            accept_eula: false,
            modpack_version: Some("9001".into()),
            modpack_source: ContentSource::Curseforge,
            loader_version: None,
            game_rules: Default::default(),
        })
        .unwrap()
    else {
        panic!()
    };
    let done = wait_job(&mut c, &job.job_id);
    assert_eq!(done.state, JobState::Succeeded, "{:?}", done.log);
    assert_eq!(std::fs::read(target.join("mods/bueno.jar")).unwrap(), modjar);
    assert!(!target.join("mods/restringido.jar").exists());
    assert!(!target.join("mods/texturas.zip").exists());
    assert_eq!(
        std::fs::read_to_string(target.join("config/cf.toml")).unwrap(),
        "cf = true"
    );
    assert!(done
        .log
        .iter()
        .any(|l| l.text.contains("⚠") && l.text.contains("Proyecto 200")));
}

/// Crea un servidor desde un modpack y lo actualiza a la versión siguiente sin pisar lo que
/// cambió la persona.
#[test]
fn update_modpack_respects_user_changes() {
    use serde_json::json;
    use std::io::Write as _;
    let tmp = tempfile::tempdir().unwrap();
    let jar = fake_jar(tmp.path());
    let routes: Routes = Default::default();
    let base = fake_http(routes.clone());
    let (a1, a2, quitado, nuevo) = (
        b"mod a v1".to_vec(),
        b"mod a v2".to_vec(),
        b"mod que quito".to_vec(),
        b"mod nuevo".to_vec(),
    );
    let entry = |path: &str, data: &[u8], dl: &str| {
        json!({ "path": path, "hashes": { "sha1": sha1_hex(data) }, "env": { "server": "required" },
                "downloads": [format!("{base}/dl/{dl}")], "fileSize": data.len() })
    };
    let mrpack = |version: &str, files: Vec<serde_json::Value>, cfg: &[(&str, &str)]| {
        let index = json!({
            "formatVersion": 1, "game": "minecraft", "versionId": version, "name": "Aventura",
            "dependencies": { "minecraft": "1.21.1", "fabric-loader": "0.16.5" }, "files": files
        });
        let mut pack = vec![];
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut pack));
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("modrinth.index.json", o).unwrap();
        z.write_all(index.to_string().as_bytes()).unwrap();
        for (p, c) in cfg {
            z.start_file(format!("overrides/{p}"), o).unwrap();
            z.write_all(c.as_bytes()).unwrap();
        }
        z.finish().unwrap();
        pack
    };
    let p1 = mrpack(
        "1.0",
        vec![
            entry("mods/a-1.jar", &a1, "a1.jar"),
            entry("mods/quitado.jar", &quitado, "q.jar"),
        ],
        &[("config/editado.toml", "x = 1"), ("config/igual.toml", "y = 1")],
    );
    let p2 = mrpack(
        "2.0",
        vec![
            entry("mods/a-2.jar", &a2, "a2.jar"),
            entry("mods/quitado.jar", &quitado, "q.jar"),
            entry("mods/nuevo.jar", &nuevo, "n.jar"),
        ],
        &[("config/editado.toml", "x = 2"), ("config/igual.toml", "y = 2")],
    );
    let version = |id: &str, num: &str, date: &str, file: &str, pack: &[u8]| {
        json!({ "id": id, "project_id": "PACK", "name": format!("Aventura {num}"), "version_number": num,
            "version_type": "release", "game_versions": ["1.21.1"], "loaders": ["fabric"], "date_published": date,
            "files": [{ "url": format!("{base}/dl/{file}"), "filename": file, "primary": true,
                        "size": pack.len(), "hashes": { "sha1": sha1_hex(pack) } }] })
    };
    {
        let mut r = routes.lock().unwrap();
        r.insert(
            "/fabric/v2/versions/installer".into(),
            br#"[{"version":"1.1.0","stable":true}]"#.to_vec(),
        );
        r.insert(
            "/fabric/v2/versions/loader/1.21.1/0.16.5/1.1.0/server/jar".into(),
            jar.clone(),
        );
        let v1 = version("packv1", "1.0", "2026-09-01T00:00:00Z", "av-1.mrpack", &p1);
        let v2 = version("packv2", "2.0", "2026-10-01T00:00:00Z", "av-2.mrpack", &p2);
        r.insert("/mr/v2/version/packv1".into(), v1.to_string().into_bytes());
        r.insert("/mr/v2/version/packv2".into(), v2.to_string().into_bytes());
        r.insert(
            "/mr/v2/project/PACK/version".into(),
            json!([v2, v1]).to_string().into_bytes(),
        );
        r.insert("/dl/av-1.mrpack".into(), p1.clone());
        r.insert("/dl/av-2.mrpack".into(), p2.clone());
        r.insert("/dl/a1.jar".into(), a1.clone());
        r.insert("/dl/a2.jar".into(), a2.clone());
        r.insert("/dl/q.jar".into(), quitado.clone());
        r.insert("/dl/n.jar".into(), nuevo.clone());
    }
    let data = tmp.path().join("datos");
    let _d = Daemon(
        Command::new(env!("CARGO_BIN_EXE_merc-supervisord"))
            .arg("--data-dir")
            .arg(&data)
            .env("MERC_SOURCE_FABRIC", format!("{base}/fabric"))
            .env("MERC_SOURCE_MODRINTH", format!("{base}/mr/v2"))
            .env("MERC_TEST_LOOPBACK", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut c = client(&data);
    let target = tmp.path().join("Aventura");
    let ResponseBody::Job(job) = c
        .call(RequestBody::CreateServer {
            name: "Aventura".into(),
            flavor: Flavor::Fabric,
            mc_version: "1.21.1".into(),
            path: target.clone(),
            java: java(),
            min_memory_mb: 0,
            max_memory_mb: 1024,
            properties: BTreeMap::new(),
            accept_eula: false,
            modpack_version: Some("packv1".into()),
            modpack_source: ContentSource::Modrinth,
            loader_version: None,
            game_rules: Default::default(),
        })
        .unwrap()
    else {
        panic!()
    };
    let done = wait_job(&mut c, &job.job_id);
    assert_eq!(done.state, JobState::Succeeded, "{:?}", done.log);
    let id = done.instance_id.unwrap();

    // La persona edita un config, quita un mod y añade uno suyo.
    std::fs::write(target.join("config/editado.toml"), "x = 99 # mío").unwrap();
    std::fs::remove_file(target.join("mods/quitado.jar")).unwrap();
    std::fs::write(target.join("mods/propio.jar"), "mío").unwrap();

    let ResponseBody::ModpackStatus(st) = c.call(RequestBody::ModpackStatus { id }).unwrap() else {
        panic!()
    };
    let linked = st.linked.as_ref().unwrap();
    assert_eq!(
        (linked.version_number.as_str(), linked.title.as_str()),
        ("1.0", "Aventura")
    );
    let v2 = st.versions.iter().find(|v| v.version_id == "packv2").unwrap();
    assert!(v2.newer && v2.compatible && !v2.current, "{st:?}");

    let ResponseBody::ModpackUpdatePlan(plan) = c
        .call(RequestBody::ModpackUpdatePreview {
            id,
            version_id: "packv2".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    use merc_content::packupdate::ChangeKind as K;
    let kind = |p: &str| plan.changes.iter().find(|c| c.path == p).map(|c| c.kind);
    assert_eq!(kind("mods/a-1.jar"), Some(K::Remove));
    assert_eq!(kind("mods/a-2.jar"), Some(K::Add));
    assert_eq!(kind("mods/nuevo.jar"), Some(K::Add));
    assert_eq!(kind("mods/quitado.jar"), Some(K::SkipRemoved));
    assert_eq!(kind("config/editado.toml"), Some(K::KeepModified));
    assert_eq!(kind("config/igual.toml"), Some(K::Replace));
    assert_eq!(kind("mods/propio.jar"), None);
    assert!(!plan.loader_mismatch);

    let ResponseBody::Job(job) = c
        .call(RequestBody::ModpackUpdateApply {
            id,
            version_id: "packv2".into(),
            accept_loader_change: false,
        })
        .unwrap()
    else {
        panic!()
    };
    let done = wait_job(&mut c, &job.job_id);
    assert_eq!(done.state, JobState::Succeeded, "{:?}", done.log);
    assert!(!target.join("mods/a-1.jar").exists());
    assert_eq!(std::fs::read(target.join("mods/a-2.jar")).unwrap(), a2);
    assert_eq!(std::fs::read(target.join("mods/nuevo.jar")).unwrap(), nuevo);
    assert!(
        !target.join("mods/quitado.jar").exists(),
        "lo que quitó la persona no vuelve"
    );
    assert_eq!(std::fs::read_to_string(target.join("mods/propio.jar")).unwrap(), "mío");
    assert_eq!(
        std::fs::read_to_string(target.join("config/editado.toml")).unwrap(),
        "x = 99 # mío"
    );
    assert_eq!(
        std::fs::read_to_string(target.join("config/igual.toml")).unwrap(),
        "y = 2"
    );
    // Hay copia previa y lo viejo está en la papelera.
    let ResponseBody::Backups(b) = c.call(RequestBody::Backups { id }).unwrap() else {
        panic!()
    };
    assert!(
        b.items
            .iter()
            .any(|x| x.note.as_deref().unwrap_or("").contains("modpack")),
        "{b:?}"
    );
    let ResponseBody::SavedItems(items) = c.call(RequestBody::SavedItems { id }).unwrap() else {
        panic!()
    };
    assert!(items.iter().any(|i| i.path == "modpack-anterior-1.0"), "{items:?}");
    let ResponseBody::ModpackStatus(st) = c.call(RequestBody::ModpackStatus { id }).unwrap() else {
        panic!()
    };
    assert_eq!(st.linked.unwrap().version_number, "2.0");
}

/// Crear un servidor Bedrock: enlaces de minecraft.net, zip oficial, sin Java, EULA marcada.
#[cfg(unix)]
#[test]
fn create_bedrock_server_without_java() {
    use std::io::Write;
    let tmp = tempfile::tempdir().unwrap();
    let mut zbuf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut zbuf);
        let opts = zip::write::SimpleFileOptions::default().unix_permissions(0o755);
        z.start_file("bedrock_server", opts).unwrap();
        z.write_all(b"#!/bin/sh\necho '[2026-11-01 10:00:01:000 INFO] Server started.'\nwhile read l; do [ \"$l\" = stop ] && exit 0; done\n")
            .unwrap();
        z.start_file("server.properties", zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(b"server-name=Dedicated Server\nserver-port=19132\nlevel-name=Bedrock level\n")
            .unwrap();
        z.finish().unwrap();
    }
    let routes: Routes = Default::default();
    let base = fake_http(routes.clone());
    {
        let mut r = routes.lock().unwrap();
        r.insert(
            "/bedrock/links".into(),
            serde_json::json!({"result": {"links": [
                {"downloadType": "serverBedrockLinux", "downloadUrl": format!("{base}/bedrock/bedrock-server-1.21.124.2.zip")}
            ]}})
            .to_string()
            .into_bytes(),
        );
        r.insert("/bedrock/bedrock-server-1.21.124.2.zip".into(), zbuf.into_inner());
    }
    let data = tmp.path().join("datos");
    let _d = Daemon(
        Command::new(env!("CARGO_BIN_EXE_merc-supervisord"))
            .arg("--data-dir")
            .arg(&data)
            .env("MERC_SOURCE_BEDROCK", format!("{base}/bedrock/links"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut c = client(&data);
    let ResponseBody::Catalog(list) = c
        .call(RequestBody::Catalog {
            flavor: Flavor::Bedrock,
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].mc_version, "1.21.124.2");
    let port = std::net::UdpSocket::bind(("0.0.0.0", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let target = tmp.path().join("Bedrock de prueba");
    let create = |c: &mut Client, props: BTreeMap<String, String>, rules: BTreeMap<String, String>| {
        c.call(RequestBody::CreateServer {
            name: "Bedrock de prueba".into(),
            flavor: Flavor::Bedrock,
            mc_version: "1.21.124.2".into(),
            path: target.clone(),
            java: PathBuf::new(),
            min_memory_mb: 0,
            max_memory_mb: 0,
            properties: props,
            accept_eula: true,
            modpack_version: None,
            modpack_source: ContentSource::Modrinth,
            loader_version: None,
            game_rules: rules,
        })
    };
    // Claves de Java y reglas de Java se rechazan antes de crear nada.
    assert!(matches!(
        create(&mut c, [("motd".to_string(), "x".to_string())].into(), BTreeMap::new()),
        Err(ClientError::Api(e)) if e.code == ErrorCode::BadRequest
    ));
    assert!(matches!(
        create(&mut c, BTreeMap::new(), [("keep_inventory".to_string(), "true".to_string())].into()),
        Err(ClientError::Api(e)) if e.code == ErrorCode::BadRequest
    ));
    let props: BTreeMap<String, String> = [
        ("server-port".to_string(), port.to_string()),
        ("server-name".to_string(), "Server de Ñandú".to_string()),
    ]
    .into();
    let ResponseBody::Job(j) = create(&mut c, props, BTreeMap::new()).unwrap() else {
        panic!()
    };
    let j = wait_job(&mut c, &j.job_id);
    assert_eq!(j.state, JobState::Succeeded, "{:?} {:?}", j.error, j.log);
    let id = j.instance_id.unwrap();
    let text = std::fs::read_to_string(target.join("server.properties")).unwrap();
    assert!(text.contains("server-name=Server de Ñandú") && text.contains(&format!("server-port={port}")));
    assert!(target.join("merchosting-bedrock-sha256.txt").is_file());
    c.call(RequestBody::Start { id }).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let ResponseBody::Status(s) = c.call(RequestBody::Status { id }).unwrap() else {
            panic!()
        };
        if s.state == RuntimeState::Online {
            break;
        }
        assert!(Instant::now() < deadline, "{s:?}");
        std::thread::sleep(Duration::from_millis(100));
    }
    c.call(RequestBody::Stop { id, timeout_secs: 20 }).unwrap();
}

#[test]
fn legacy_forge_needs_exactly_java_8() {
    let j = java();
    let major = merc_core::java::probe_java(&j, Duration::from_secs(15)).unwrap().major;
    if major == 8 {
        // Esta comprobación necesita un Java distinto de 8 en la máquina de pruebas.
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let routes: Routes = Default::default();
    let base = fake_http(routes.clone());
    {
        let mut r = routes.lock().unwrap();
        r.insert(
            "/forge/promotions_slim.json".into(),
            br#"{"promos":{"1.12.2-recommended":"14.23.5.2859"}}"#.to_vec(),
        );
        r.insert(
            "/forgemaven/net/minecraftforge/forge/1.12.2-14.23.5.2859/forge-1.12.2-14.23.5.2859-installer.jar".into(),
            b"instalador".to_vec(),
        );
    }
    let data = tmp.path().join("datos");
    let _d = Daemon(
        Command::new(env!("CARGO_BIN_EXE_merc-supervisord"))
            .arg("--data-dir")
            .arg(&data)
            .env("MERC_SOURCE_FORGE_PROMOS", format!("{base}/forge/promotions_slim.json"))
            .env("MERC_SOURCE_FORGE", format!("{base}/forgemaven"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut c = client(&data);
    let ResponseBody::Catalog(list) = c.call(RequestBody::Catalog { flavor: Flavor::Forge }).unwrap() else {
        panic!()
    };
    assert_eq!(list[0].mc_version, "1.12.2");
    assert_eq!(list[0].java_major, 8);
    let target = tmp.path().join("srp");
    let ResponseBody::Job(job) = c
        .call(RequestBody::CreateServer {
            name: "SRP".into(),
            flavor: Flavor::Forge,
            mc_version: "1.12.2".into(),
            path: target.clone(),
            java: j,
            min_memory_mb: 0,
            max_memory_mb: 2048,
            properties: BTreeMap::new(),
            accept_eula: true,
            modpack_version: None,
            modpack_source: ContentSource::Modrinth,
            loader_version: None,
            game_rules: Default::default(),
        })
        .unwrap()
    else {
        panic!()
    };
    let job = wait_job(&mut c, &job.job_id);
    assert_eq!(job.state, JobState::Failed, "{job:?}");
    assert!(
        job.error
            .as_deref()
            .is_some_and(|e| e.contains("solo funciona con Java 8")),
        "{job:?}"
    );
    assert!(!target.exists());
}

/// Servidor casero: el código de control total sale de `merc` (terminal) y otra PC crea un
/// servidor desde lejos. La carpeta y el Java los pone el anfitrión, no la otra PC.
#[cfg(unix)]
#[test]
fn home_server_full_control_from_cli() {
    use std::io::Write;
    let tmp = tempfile::tempdir().unwrap();
    let mut zbuf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut zbuf);
        let opts = zip::write::SimpleFileOptions::default().unix_permissions(0o755);
        z.start_file("bedrock_server", opts).unwrap();
        z.write_all(b"#!/bin/sh\necho '[2026-11-01 10:00:01:000 INFO] Server started.'\nwhile read l; do [ \"$l\" = stop ] && exit 0; done\n")
            .unwrap();
        z.start_file("server.properties", zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(b"server-name=Dedicated Server\nserver-port=19132\nlevel-name=Bedrock level\n")
            .unwrap();
        z.finish().unwrap();
    }
    let routes: Routes = Default::default();
    let base = fake_http(routes.clone());
    {
        let mut r = routes.lock().unwrap();
        r.insert(
            "/bedrock/links".into(),
            serde_json::json!({"result": {"links": [
                {"downloadType": "serverBedrockLinux", "downloadUrl": format!("{base}/bedrock/bedrock-server-1.21.124.2.zip")}
            ]}})
            .to_string()
            .into_bytes(),
        );
        r.insert("/bedrock/bedrock-server-1.21.124.2.zip".into(), zbuf.into_inner());
    }
    let host_data = tmp.path().join("casero");
    let guest_data = tmp.path().join("mi-pc");
    let servers_dir = tmp.path().join("Servidores del casero");
    let spawn = |data: &Path| {
        Daemon(
            Command::new(env!("CARGO_BIN_EXE_merc-supervisord"))
                .arg("--data-dir")
                .arg(data)
                .env("MERC_SOURCE_BEDROCK", format!("{base}/bedrock/links"))
                .env("MERC_SERVERS_DIR", &servers_dir)
                .env("MERC_REMOTE_LOCAL", "1")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        )
    };
    let _h = spawn(&host_data);
    let _g = spawn(&guest_data);
    let mut host = client(&host_data);
    let mut guest = client(&guest_data);

    // En el casero, desde la terminal.
    let merc = |args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_merc"))
            .arg("--data-dir")
            .arg(&host_data)
            .args(args)
            .env("MERC_SUPERVISORD_PATH", env!("CARGO_BIN_EXE_merc-supervisord"))
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let out = merc(&["compañeros", "invitar", "--control-total", "--minutos", "30"]);
    assert!(out.contains("CONTROL TOTAL"), "{out}");
    let code = out
        .lines()
        .find(|l| l.starts_with("MH1-"))
        .expect("el código")
        .trim()
        .to_string();
    let ResponseBody::Companions(info) = host.call(RequestBody::Companions).unwrap() else {
        panic!()
    };
    assert!(info.enabled);
    assert!(info.invites.iter().any(|i| i.host_admin && i.server.is_none()));
    // El secreto no se guarda en claro.
    let saved = std::fs::read_to_string(host_data.join("companeros/anfitrion.json")).unwrap();
    let secret = merc_remote::code::Invite::decode(&code).unwrap().secret;
    assert!(!saved.contains(&merc_remote::hex(&secret)));

    // En la PC principal: se conecta con el código.
    let ResponseBody::RemoteHosts(hosts) = guest
        .call(RequestBody::JoinWithCode {
            code: code.clone(),
            name: "Amir".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(hosts.len(), 1);
    assert!(hosts[0].host_admin);
    assert!(hosts[0].servers.is_empty());
    let h = hosts[0].host.clone();
    let remote = |c: &mut Client, r: RequestBody| {
        c.call(RequestBody::RemoteCall {
            host: h.clone(),
            request: Box::new(r),
        })
    };
    let forbidden =
        |r: Result<ResponseBody, ClientError>| matches!(r, Err(ClientError::Api(e)) if e.code == ErrorCode::Forbidden);

    // Sigue sin poder tocar cosas de la PC: rutas, apagar, compañeros, Playit.
    for r in [
        RequestBody::CheckTarget { path: "/etc".into() },
        RequestBody::Detect { path: "/etc".into() },
        RequestBody::Shutdown,
        RequestBody::Companions,
        RequestBody::CreateHostInvite { minutes: 30 },
    ] {
        assert!(forbidden(remote(&mut guest, r.clone())), "{r:?}");
    }
    // Con control total sí ve Playit de esa PC (para activarlo y vincularlo desde el celular).
    assert!(matches!(
        remote(&mut guest, RequestBody::PlayitStatus),
        Ok(ResponseBody::Playit(_))
    ));

    // Crea un servidor: la ruta y el Java que manda se ignoran.
    let ResponseBody::Job(j) = remote(
        &mut guest,
        RequestBody::CreateServer {
            name: "Casero ../../x".into(),
            flavor: Flavor::Bedrock,
            mc_version: "1.21.124.2".into(),
            path: "/tmp/no-aqui".into(),
            java: "/bin/sh".into(),
            min_memory_mb: 0,
            max_memory_mb: 0,
            properties: BTreeMap::new(),
            accept_eula: true,
            modpack_version: None,
            modpack_source: ContentSource::Modrinth,
            loader_version: None,
            game_rules: BTreeMap::new(),
        },
    )
    .unwrap() else {
        panic!()
    };
    // El trabajo lo sigue quien lo empezó.
    let deadline = Instant::now() + Duration::from_secs(60);
    let id = loop {
        let ResponseBody::Job(s) = remote(
            &mut guest,
            RequestBody::Job {
                job_id: j.job_id.clone(),
                after: 0,
            },
        )
        .unwrap() else {
            panic!()
        };
        if s.state != JobState::Running {
            assert_eq!(s.state, JobState::Succeeded, "{:?}", s.error);
            break s.instance_id.unwrap();
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(150));
    };
    assert!(!Path::new("/tmp/no-aqui").exists());
    let made = servers_dir.join("Casero-x");
    assert!(
        made.join("bedrock_server").is_file(),
        "{:?}",
        std::fs::read_dir(&servers_dir).unwrap().collect::<Vec<_>>()
    );

    // Ve el servidor nuevo con todos los permisos, y sin rutas del casero.
    let ResponseBody::Instances(list) = remote(&mut guest, RequestBody::ListInstances).unwrap() else {
        panic!()
    };
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].instance.root, PathBuf::new());
    assert!(matches!(
        remote(&mut guest, RequestBody::Status { id }).unwrap(),
        ResponseBody::Status(_)
    ));
    let ResponseBody::RemoteHosts(hosts) = guest.call(RequestBody::RefreshHost { host: h.clone() }).unwrap() else {
        panic!()
    };
    assert_eq!(hosts[0].servers.len(), 1);
    assert_eq!(hosts[0].servers[0].perms.len(), merc_ipc::remote::Perm::ALL.len());

    // La terminal lo muestra y lo puede quitar.
    let out = merc(&["compañeros", "lista"]);
    assert!(out.contains("Amir") && out.contains("CONTROL TOTAL"), "{out}");
    merc(&["compañeros", "quitar", "Amir"]);
    assert!(forbidden(remote(&mut guest, RequestBody::Status { id })));
}
