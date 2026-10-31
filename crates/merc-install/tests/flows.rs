//! Flujos completos contra un servidor HTTP local que imita las fuentes oficiales.

use merc_core::detect::ServerKind;
use merc_install::{install, list_versions, resolve, Flavor, Http, InstallError, InstallEvent, InstallPlan, Sources};
use sha1::Digest as _;
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

type Routes = Arc<Mutex<HashMap<String, (u16, Vec<u8>)>>>;

struct Fake {
    base: String,
    routes: Routes,
}

impl Fake {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let routes: Routes = Default::default();
        let r = routes.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let r = r.clone();
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_err() {
                        return;
                    }
                    let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
                    loop {
                        let mut h = String::new();
                        if reader.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                            break;
                        }
                    }
                    let (status, body) = r.lock().unwrap().get(&path).cloned().unwrap_or((404, b"no".to_vec()));
                    let mut s = stream;
                    let _ = write!(
                        s,
                        "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = s.write_all(&body);
                });
            }
        });
        Self { base, routes }
    }
    fn route(&self, path: &str, body: impl Into<Vec<u8>>) {
        self.routes.lock().unwrap().insert(path.to_string(), (200, body.into()));
    }
    fn sources(&self) -> Sources {
        Sources {
            mojang_manifest: format!("{}/mojang/manifest.json", self.base),
            paper_api: format!("{}/paper", self.base),
            fabric_meta: format!("{}/fabric", self.base),
            neoforge_maven: format!("{}/maven", self.base),
            purpur_api: format!("{}/purpur", self.base),
            forge_promotions: format!("{}/forge/promotions_slim.json", self.base),
            forge_maven: format!("{}/forgemaven", self.base),
            quilt_meta: format!("{}/quilt", self.base),
            quilt_maven: format!("{}/quiltmaven", self.base),
            arclight_releases: format!("{}/arclight/releases", self.base),
            spigot_versions: format!("{}/spigot/versions", self.base),
            buildtools_url: format!("{}/spigot/BuildTools.jar", self.base),
        }
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn plan(resolved: merc_install::Resolved, target: PathBuf, eula: bool) -> InstallPlan {
    let mut properties = BTreeMap::new();
    properties.insert("server-port".to_string(), "25570".to_string());
    properties.insert("motd".to_string(), "Servidor de prueba".to_string());
    InstallPlan {
        resolved,
        target,
        java: PathBuf::from("java"),
        properties,
        accept_eula: eula,
        cache_dir: None,
    }
}

fn collect() -> (Arc<Mutex<Vec<InstallEvent>>>, impl FnMut(InstallEvent)) {
    let ev: Arc<Mutex<Vec<InstallEvent>>> = Default::default();
    let e2 = ev.clone();
    (ev, move |e| e2.lock().unwrap().push(e))
}

#[test]
fn vanilla_end_to_end() {
    let f = Fake::start();
    let jar = b"jar de servidor falso".repeat(50_000);
    let sha1 = hex(&sha1::Sha1::digest(&jar));
    f.route(
        "/mojang/manifest.json",
        serde_json::json!({
            "latest": {"release": "26.1.2"},
            "versions": [{"id": "26.1.2", "type": "release", "url": format!("{}/mojang/26.1.2.json", f.base)}]
        })
        .to_string(),
    );
    f.route(
        "/mojang/26.1.2.json",
        serde_json::json!({
            "javaVersion": {"majorVersion": 25},
            "downloads": {"server": {"url": format!("{}/mojang/server.jar", f.base), "sha1": sha1, "size": jar.len()}}
        })
        .to_string(),
    );
    f.route("/mojang/server.jar", jar.clone());
    let http = Http::new();
    let list = list_versions(&http, &f.sources(), Flavor::Vanilla).unwrap();
    assert_eq!(list[0].mc_version, "26.1.2");
    let r = resolve(&http, &f.sources(), Flavor::Vanilla, "26.1.2").unwrap();
    assert_eq!(r.java_major, 25);

    let d = tempfile::tempdir().unwrap();
    let target = d.path().join("Servidores/Mi server");
    let (ev, mut sink) = collect();
    let out = install(
        &http,
        &plan(r, target.clone(), true),
        &mut sink,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(out.detected.kind, ServerKind::Vanilla);
    assert_eq!(out.detected.mc_version.as_deref(), Some("26.1.2"));
    assert_eq!(std::fs::read(target.join("minecraft_server.26.1.2.jar")).unwrap(), jar);
    let props = std::fs::read_to_string(target.join("server.properties")).unwrap();
    assert!(props.contains("server-port=25570") && props.contains("motd=Servidor de prueba"));
    assert!(std::fs::read_to_string(target.join("eula.txt"))
        .unwrap()
        .contains("eula=true"));
    let ev = ev.lock().unwrap();
    assert!(ev
        .iter()
        .any(|e| matches!(e, InstallEvent::Download { done, total: Some(t), .. } if done == t)));
    assert!(ev
        .iter()
        .any(|e| matches!(e, InstallEvent::Log(l) if l.contains("SHA-1"))));
}

#[test]
fn eula_not_written_without_consent() {
    let f = Fake::start();
    let jar = b"x".repeat(1000);
    let sha256 = hex(&sha2::Sha256::digest(&jar));
    f.route(
        "/paper/v3/projects/paper/versions/1.21.11/builds",
        serde_json::json!([{"id": 5, "channel": "STABLE", "downloads": {"server:default": {
            "name": "paper-1.21.11-5.jar", "url": format!("{}/paper/dl", f.base), "checksums": {"sha256": sha256}, "size": 1000}}}])
        .to_string(),
    );
    f.route("/paper/dl", jar);
    let http = Http::new();
    let r = resolve(&http, &f.sources(), Flavor::Paper, "1.21.11").unwrap();
    let d = tempfile::tempdir().unwrap();
    let out = install(
        &http,
        &plan(r, d.path().to_path_buf(), false),
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(out.detected.kind, ServerKind::Paper);
    assert!(!d.path().join("eula.txt").exists());
}

#[test]
fn corrupted_download_is_rejected_and_cleaned() {
    let f = Fake::start();
    f.route(
        "/paper/v3/projects/paper/versions/1.21.11/builds",
        serde_json::json!([{"id": 5, "channel": "STABLE", "downloads": {"server:default": {
            "name": "paper-1.21.11-5.jar", "url": format!("{}/paper/dl", f.base), "checksums": {"sha256": "00".repeat(32)}}}}])
        .to_string(),
    );
    f.route("/paper/dl", b"contenido alterado".to_vec());
    let http = Http::new();
    let r = resolve(&http, &f.sources(), Flavor::Paper, "1.21.11").unwrap();
    let d = tempfile::tempdir().unwrap();
    let target = d.path().join("nuevo");
    let err = install(
        &http,
        &plan(r, target.clone(), true),
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap_err();
    assert!(matches!(err, InstallError::Integrity(_)), "{err:?}");
    assert!(!target.exists(), "debe borrar la carpeta que creó");
}

#[test]
fn missing_version_is_reported() {
    let f = Fake::start();
    let http = Http::new();
    let err = resolve(&http, &f.sources(), Flavor::Paper, "1.21.11").unwrap_err();
    assert!(matches!(err, InstallError::NotFound(_)), "{err:?}");
    let err = resolve(&http, &f.sources(), Flavor::Paper, "../../x").unwrap_err();
    assert!(matches!(err, InstallError::InvalidInput(_)));
}

#[test]
fn non_empty_folder_is_never_touched() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("world.dat"), "mundo").unwrap();
    let r = merc_install::catalog::fabric_resolved("http://127.0.0.1:1", "1.21.11", "0.18.2", "1.1.0");
    let err = install(
        &Http::new(),
        &plan(r, d.path().to_path_buf(), true),
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap_err();
    assert!(matches!(err, InstallError::TargetNotEmpty(_)));
    assert_eq!(std::fs::read_to_string(d.path().join("world.dat")).unwrap(), "mundo");
}

#[test]
fn fabric_and_cancel() {
    let f = Fake::start();
    f.route(
        "/fabric/v2/versions/loader",
        r#"[{"version":"0.19.0","stable":false},{"version":"0.18.2","stable":true}]"#,
    );
    f.route(
        "/fabric/v2/versions/installer",
        r#"[{"version":"1.1.0","stable":true}]"#,
    );
    f.route(
        "/fabric/v2/versions/loader/26.1.2/0.18.2/1.1.0/server/jar",
        b"fabric".to_vec(),
    );
    let http = Http::new();
    let r = resolve(&http, &f.sources(), Flavor::Fabric, "26.1.2").unwrap();
    assert_eq!(r.loader_version.as_deref(), Some("0.18.2"));

    let d = tempfile::tempdir().unwrap();
    let cancelled = install(
        &http,
        &plan(r.clone(), d.path().join("a"), true),
        &mut |_| {},
        &AtomicBool::new(true),
    );
    assert_eq!(cancelled.unwrap_err(), InstallError::Cancelled);
    assert!(!d.path().join("a").exists());

    let out = install(
        &http,
        &plan(r, d.path().join("b"), true),
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(out.detected.kind, ServerKind::Fabric);
    assert_eq!(out.detected.mc_version.as_deref(), Some("26.1.2"));
}

/// El "Java" es un script que imita al instalador oficial: crea run.sh y los argumentos.
#[cfg(unix)]
fn fake_java(dir: &std::path::Path, fail: bool) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let body = if fail {
        "#!/bin/sh\necho 'Descargando librerías...'\necho 'Error: sin conexión' >&2\nexit 3\n".to_string()
    } else {
        "#!/bin/sh\n[ \"$3\" = \"--installServer\" ] || exit 9\nV=26.1.2.76\n\
         mkdir -p libraries/net/neoforged/neoforge/$V\n\
         echo '-cp x' > libraries/net/neoforged/neoforge/$V/unix_args.txt\n\
         echo 'java @user_jvm_args.txt @libraries/net/neoforged/neoforge/26.1.2.76/unix_args.txt \"$@\"' > run.sh\n\
         echo '# Xmx' > user_jvm_args.txt\necho 'The server installed successfully'\n"
            .to_string()
    };
    let p = dir.join(if fail { "java-falla" } else { "java-ok" });
    std::fs::write(&p, body).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

#[cfg(unix)]
#[test]
fn neoforge_runs_installer_and_verifies() {
    let f = Fake::start();
    let installer = b"instalador".to_vec();
    f.route(
        "/maven/net/neoforged/neoforge/maven-metadata.xml",
        "<metadata><versioning><versions><version>26.1.2.70-beta</version><version>26.1.2.76</version><version>21.1.259</version></versions></versioning></metadata>",
    );
    let jar_path = "/maven/net/neoforged/neoforge/26.1.2.76/neoforge-26.1.2.76-installer.jar";
    f.route(jar_path, installer.clone());
    f.route(&format!("{jar_path}.sha256"), hex(&sha2::Sha256::digest(&installer)));
    let http = Http::new();
    let list = list_versions(&http, &f.sources(), Flavor::NeoForge).unwrap();
    assert_eq!(
        list.iter().map(|e| e.mc_version.as_str()).collect::<Vec<_>>(),
        ["26.1.2", "1.21.1"]
    );
    let r = resolve(&http, &f.sources(), Flavor::NeoForge, "26.1.2").unwrap();
    assert_eq!(r.loader_version.as_deref(), Some("26.1.2.76"));
    assert!(matches!(r.artifact.checksum, merc_install::Checksum::Sha256(_)));
    assert_eq!(r.java_major, 25);

    let tools = tempfile::tempdir().unwrap();
    let d = tempfile::tempdir().unwrap();
    // Falla: se limpia todo y se muestra la salida del instalador.
    let mut p = plan(r.clone(), d.path().join("falla"), true);
    p.java = fake_java(tools.path(), true);
    let (ev, mut sink) = collect();
    let err = install(&http, &p, &mut sink, &AtomicBool::new(false)).unwrap_err();
    assert!(
        matches!(&err, InstallError::InstallerFailed(m) if m.contains("sin conexión")),
        "{err:?}"
    );
    assert!(!d.path().join("falla").exists());
    assert!(ev
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, InstallEvent::Log(l) if l.contains("librerías"))));

    let mut p = plan(r, d.path().join("ok"), true);
    p.java = fake_java(tools.path(), false);
    let out = install(&http, &p, &mut |_| {}, &AtomicBool::new(false)).unwrap();
    assert_eq!(out.detected.kind, ServerKind::NeoForge);
    assert_eq!(out.detected.mc_version.as_deref(), Some("26.1.2"));
    assert!(!d.path().join("ok/neoforge-26.1.2.76-installer.jar").exists());
}

#[test]
fn purpur_md5_and_forge() {
    let f = Fake::start();
    let jar = b"purpur".repeat(100);
    let md5 = hex(&md5::Md5::digest(&jar));
    f.route(
        "/purpur/purpur",
        r#"{"project":"purpur","versions":["1.20.4","1.21.11","26.1.2"]}"#,
    );
    f.route(
        "/purpur/purpur/1.21.11",
        r#"{"project":"purpur","version":"1.21.11","builds":{"latest":"2500","all":["2499","2500"]}}"#,
    );
    f.route(
        "/purpur/purpur/1.21.11/2500",
        format!(r#"{{"build":"2500","md5":"{md5}","result":"SUCCESS"}}"#),
    );
    f.route("/purpur/purpur/1.21.11/2500/download", jar.clone());
    let http = Http::new();
    let list = list_versions(&http, &f.sources(), Flavor::Purpur).unwrap();
    assert_eq!(list[0].mc_version, "26.1.2");
    let r = resolve(&http, &f.sources(), Flavor::Purpur, "1.21.11").unwrap();
    assert_eq!(r.artifact.checksum, merc_install::Checksum::Md5(md5));
    let d = tempfile::tempdir().unwrap();
    let out = install(
        &http,
        &plan(r, d.path().join("p"), true),
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(out.detected.kind, ServerKind::Purpur);

    // MD5 alterado: rechazado.
    f.route(
        "/purpur/purpur/1.21.11/2500",
        r#"{"build":"2500","md5":"00000000000000000000000000000000"}"#,
    );
    let r = resolve(&http, &f.sources(), Flavor::Purpur, "1.21.11").unwrap();
    let err = install(
        &http,
        &plan(r, d.path().join("q"), true),
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap_err();
    assert!(matches!(err, InstallError::Integrity(_)));

    f.route(
        "/forge/promotions_slim.json",
        r#"{"promos":{"1.12.2-recommended":"14.23.5.2859","1.20.1-recommended":"47.4.0","1.20.1-latest":"47.4.10","26.1-latest":"62.0.9"}}"#,
    );
    let list = list_versions(&http, &f.sources(), Flavor::Forge).unwrap();
    assert_eq!(
        list.iter().map(|e| e.mc_version.as_str()).collect::<Vec<_>>(),
        ["26.1", "1.20.1"]
    );
    let r = resolve(&http, &f.sources(), Flavor::Forge, "1.20.1").unwrap();
    assert_eq!(r.loader_version.as_deref(), Some("47.4.0"));
    assert!(r
        .artifact
        .url
        .ends_with("/net/minecraftforge/forge/1.20.1-47.4.0/forge-1.20.1-47.4.0-installer.jar"));
    assert_eq!(r.installer_args.as_deref(), Some(&["--installServer".to_string()][..]));
    let r = resolve(&http, &f.sources(), Flavor::Forge, "26.1").unwrap();
    assert!(r.notes.iter().any(|n| n.contains("versión recomendada")));
}

#[test]
fn spigot_needs_jdk_and_lists_versions() {
    let f = Fake::start();
    f.route(
        "/spigot/versions/",
        r#"<html><a href="1.20.4.json">1.20.4.json</a><a href="1.21.11.json">1.21.11.json</a><a href="latest.json">latest.json</a><a href="4321.json">x</a></html>"#,
    );
    let http = Http::new();
    let list = list_versions(&http, &f.sources(), Flavor::Spigot).unwrap();
    assert_eq!(
        list.iter().map(|e| e.mc_version.as_str()).collect::<Vec<_>>(),
        ["1.21.11", "1.20.4"]
    );
    let r = resolve(&http, &f.sources(), Flavor::Spigot, "1.21.11").unwrap();
    assert!(r.needs_jdk && r.installer_in_cache);
    assert!(matches!(
        resolve(&http, &f.sources(), Flavor::Spigot, "1.99"),
        Err(InstallError::NotAvailable(_))
    ));
    // Un "java" sin javac al lado: se rechaza antes de crear nada.
    let tools = tempfile::tempdir().unwrap();
    let java = tools.path().join("java");
    std::fs::write(&java, "").unwrap();
    let d = tempfile::tempdir().unwrap();
    let mut p = plan(r, d.path().join("s"), true);
    p.java = java;
    let err = install(&http, &p, &mut |_| {}, &AtomicBool::new(false)).unwrap_err();
    assert!(
        matches!(&err, InstallError::InvalidInput(m) if m.contains("JDK")),
        "{err:?}"
    );
    assert!(!d.path().join("s").exists());
}

/// Instalador simulado de Spigot: comprueba los argumentos y deja spigot-<v>.jar en --output-dir.
#[cfg(unix)]
#[test]
fn spigot_buildtools_runs_in_cache_and_outputs_to_server() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fake::start();
    f.route("/spigot/versions/", r#"<a href="1.21.11.json">"#);
    f.route("/spigot/BuildTools.jar", b"bt".to_vec());
    let http = Http::new();
    let r = resolve(&http, &f.sources(), Flavor::Spigot, "1.21.11").unwrap();
    let tools = tempfile::tempdir().unwrap();
    std::fs::write(tools.path().join("javac"), "").unwrap();
    let java = tools.path().join("java");
    std::fs::write(
        &java,
        "#!/bin/sh\n[ \"$3\" = \"--rev\" ] && [ \"$4\" = \"1.21.11\" ] || exit 9\necho compilando > BuildTools.log.txt\necho 'Success! Everything completed successfully.'\necho x > \"$8/spigot-1.21.11.jar\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&java, std::fs::Permissions::from_mode(0o755)).unwrap();
    let d = tempfile::tempdir().unwrap();
    let cache = d.path().join("cache");
    let mut p = plan(r, d.path().join("srv"), true);
    p.java = java;
    p.cache_dir = Some(cache.clone());
    let out = install(&http, &p, &mut |_| {}, &AtomicBool::new(false)).unwrap();
    assert_eq!(out.detected.kind, ServerKind::Spigot);
    assert_eq!(out.detected.mc_version.as_deref(), Some("1.21.11"));
    assert!(
        cache.join("BuildTools.log.txt").is_file(),
        "trabaja en la caché, no en el servidor"
    );
    assert!(!d.path().join("srv/BuildTools.jar").exists());
    assert!(!d.path().join("srv/BuildTools.log.txt").exists());
}

#[cfg(unix)]
#[test]
fn quilt_installer_args() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fake::start();
    f.route("/quilt/versions/game", r#"[{"version":"26.1.2","stable":true}]"#);
    f.route(
        "/quilt/versions/loader",
        r#"[{"version":"0.30.0-beta.2"},{"version":"0.29.2"}]"#,
    );
    f.route(
        "/quiltmaven/org/quiltmc/quilt-installer/maven-metadata.xml",
        "<metadata><versioning><release>0.12.1</release></versioning></metadata>",
    );
    f.route(
        "/quiltmaven/org/quiltmc/quilt-installer/0.12.1/quilt-installer-0.12.1.jar",
        b"qi".to_vec(),
    );
    let http = Http::new();
    let r = resolve(&http, &f.sources(), Flavor::Quilt, "26.1.2").unwrap();
    assert_eq!(r.loader_version.as_deref(), Some("0.29.2"));
    let tools = tempfile::tempdir().unwrap();
    let java = tools.path().join("java");
    std::fs::write(
        &java,
        "#!/bin/sh\n[ \"$3 $4 $5 $6 $7 $8\" = \"install server 26.1.2 0.29.2 --download-server --install-dir=.\" ] || exit 9\necho x > quilt-server-launch.jar\necho x > server.jar\n",
    )
    .unwrap();
    std::fs::set_permissions(&java, std::fs::Permissions::from_mode(0o755)).unwrap();
    let d = tempfile::tempdir().unwrap();
    let mut p = plan(r, d.path().join("q"), true);
    p.java = java;
    let out = install(&http, &p, &mut |_| {}, &AtomicBool::new(false)).unwrap();
    assert_eq!(out.detected.kind, ServerKind::Quilt);
}

#[test]
fn arclight_from_github_releases() {
    let f = Fake::start();
    let jar = b"arclight".repeat(200);
    let sha = hex(&sha2::Sha256::digest(&jar));
    let releases = serde_json::json!([
        {"tag_name": "GroupOfSoldiers/26.1.2-exp", "prerelease": true, "draft": false, "assets": [
            {"name": "arclight-fabric-26.1.2-2.0.0-abc.jar", "browser_download_url": "https://descarga.ejemplo.invalid/a.jar", "size": 1}
        ]},
        {"tag_name": "FeudalKings/1.0.1", "prerelease": false, "draft": false, "assets": [
            {"name": "arclight-fabric-1.21.1-1.0.1-8ec9529.jar", "browser_download_url": "https://descarga.ejemplo.invalid/f.jar", "digest": "sha256:00"},
            {"name": "arclight-neoforge-1.21.1-1.0.1-8ec9529.jar", "browser_download_url": format!("{}/dl/neo.jar", f.base), "digest": format!("sha256:{sha}"), "size": jar.len()},
            {"name": "arclight-forge-1.21.1-1.0.1-8ec9529.jar", "browser_download_url": "https://descarga.ejemplo.invalid/forge.jar"},
            {"name": "notas.txt", "browser_download_url": "https://descarga.ejemplo.invalid/n.txt"}
        ]},
        {"tag_name": "Borrador", "prerelease": false, "draft": true, "assets": [
            {"name": "arclight-neoforge-1.30-9.jar", "browser_download_url": "https://descarga.ejemplo.invalid/x.jar"}
        ]}
    ]);
    f.route("/arclight/releases", releases.to_string());
    f.route("/dl/neo.jar", jar);
    let http = Http::new();
    let list = list_versions(&http, &f.sources(), Flavor::Arclight).unwrap();
    assert_eq!(
        list.iter().map(|e| e.mc_version.as_str()).collect::<Vec<_>>(),
        ["26.1.2", "1.21.1"]
    );

    // Experimental: se avisa.
    let r = resolve(&http, &f.sources(), Flavor::Arclight, "26.1.2").unwrap();
    assert!(r.experimental && r.notes.iter().any(|n| n.contains("experimental")));
    assert_eq!(r.artifact.checksum, merc_install::Checksum::None);

    // 1.21.1: estable y NeoForge primero, con la huella de GitHub.
    let r = resolve(&http, &f.sources(), Flavor::Arclight, "1.21.1").unwrap();
    assert_eq!(r.artifact.file_name, "arclight-neoforge-1.21.1-1.0.1-8ec9529.jar");
    assert!(matches!(r.artifact.checksum, merc_install::Checksum::Sha256(_)));
    assert!(!r.experimental);
    assert!(matches!(
        resolve(&http, &f.sources(), Flavor::Arclight, "1.30"),
        Err(InstallError::NotAvailable(_))
    ));

    let d = tempfile::tempdir().unwrap();
    let out = install(
        &http,
        &plan(r, d.path().join("a"), true),
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(out.detected.kind, ServerKind::Arclight);
    assert_eq!(out.detected.mc_version.as_deref(), Some("1.21.1"));
}

#[test]
fn empty_response_falls_back_to_mirror() {
    let f = Fake::start();
    let jar = b"paper".repeat(500);
    f.route("/vacio", Vec::<u8>::new());
    f.route("/bueno.jar", jar.clone());
    let mut r = merc_install::catalog::parse_paper_builds(
        &serde_json::json!([{"id": 7, "channel": "STABLE", "downloads": {"server:default": {
            "name": "paper-1.21.11-7.jar", "url": format!("{}/vacio", f.base),
            "checksums": {"sha256": hex(&sha2::Sha256::digest(&jar))}}}}]),
        "1.21.11",
    )
    .unwrap();
    r.artifact.mirrors = vec![format!("{}/bueno.jar", f.base)];
    let d = tempfile::tempdir().unwrap();
    let (ev, mut sink) = collect();
    let out = install(
        &Http::new(),
        &plan(r, d.path().join("p"), true),
        &mut sink,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(out.detected.kind, ServerKind::Paper);
    let logs: Vec<String> = ev
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| match e {
            InstallEvent::Log(l) => Some(l.clone()),
            _ => None,
        })
        .collect();
    assert!(
        logs.iter()
            .any(|l| l.contains("archivo vacío") && l.contains("se prueba el siguiente")),
        "{logs:?}"
    );
    assert!(logs
        .iter()
        .any(|l| l.starts_with("Origen:") && l.ends_with("/bueno.jar")));
}

#[test]
fn pinned_loader_versions_for_modpacks() {
    let fake = Fake::start();
    let http = Http::new();
    let src = fake.sources();
    fake.route(
        "/fabric/v2/versions/installer",
        r#"[{"version":"1.1.0","stable":true}]"#,
    );
    // El loader fijado no se consulta: se usa tal cual.
    let r = merc_install::resolve_pinned(&http, &src, Flavor::Fabric, "1.21.1", Some("0.15.11")).unwrap();
    assert_eq!(r.loader_version.as_deref(), Some("0.15.11"));
    assert!(r
        .artifact
        .url
        .ends_with("/v2/versions/loader/1.21.1/0.15.11/1.1.0/server/jar"));

    fake.route(
        "/maven/net/neoforged/neoforge/maven-metadata.xml",
        "<metadata><versioning><versions><version>21.1.77</version><version>21.1.200</version></versions></versioning></metadata>",
    );
    let r = merc_install::resolve_pinned(&http, &src, Flavor::NeoForge, "1.21.1", Some("21.1.77")).unwrap();
    assert_eq!(r.loader_version.as_deref(), Some("21.1.77"));
    assert!(matches!(
        merc_install::resolve_pinned(&http, &src, Flavor::NeoForge, "1.21.1", Some("21.1.999")),
        Err(InstallError::NotAvailable(_))
    ));
    assert!(merc_install::resolve_pinned(&http, &src, Flavor::Fabric, "1.21.1", Some("../x")).is_err());
}
