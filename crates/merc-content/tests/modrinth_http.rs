//! Cliente de Modrinth contra un servidor HTTP local que responde como la API v2 documentada.

use merc_content::curseforge::{enrich_curseforge, fingerprint, CurseForge};
use merc_content::local::{apply_plan, enrich, list_local, InstallProgress};
use merc_content::modrinth::{Modrinth, SearchSort, Side, Source};
use merc_content::plan::build_plan;
use merc_content::{ContentError, ContentKind, Platform};
use merc_install::Http;
use serde_json::json;
use sha1::Digest as _;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

type Routes = Arc<Mutex<HashMap<String, Vec<u8>>>>;

struct Fake {
    base: String,
    routes: Routes,
    /// Peticiones recibidas: "MÉTODO ruta-completa" y el cuerpo.
    seen: Arc<Mutex<Vec<(String, String)>>>,
}

impl Fake {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let routes: Routes = Default::default();
        let seen: Arc<Mutex<Vec<(String, String)>>> = Default::default();
        let (r, sn) = (routes.clone(), seen.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (r, sn) = (r.clone(), sn.clone());
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_err() {
                        return;
                    }
                    let mut parts = line.split_whitespace();
                    let method = parts.next().unwrap_or("GET").to_string();
                    let full = parts.next().unwrap_or("/").to_string();
                    let mut len = 0usize;
                    let mut key = String::new();
                    loop {
                        let mut h = String::new();
                        if reader.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                            break;
                        }
                        if let Some(v) = h.to_lowercase().strip_prefix("content-length:") {
                            len = v.trim().parse().unwrap_or(0);
                        }
                        if let Some(v) = h.strip_prefix("x-api-key:") {
                            key = format!(" [clave {}]", v.trim());
                        }
                    }
                    let mut body = vec![0u8; len];
                    let _ = reader.read_exact(&mut body);
                    sn.lock()
                        .unwrap()
                        .push((format!("{method} {full}{key}"), String::from_utf8_lossy(&body).into()));
                    let path = full.split('?').next().unwrap().to_string();
                    let key = format!("{method} {path}");
                    let resp = r.lock().unwrap().get(&key).cloned();
                    let mut s = stream;
                    match resp {
                        Some(b) => {
                            let _ = write!(
                                s,
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                b.len()
                            );
                            let _ = s.write_all(&b);
                        }
                        None => {
                            let _ = write!(s, "HTTP/1.1 404 X\r\nContent-Length: 2\r\nConnection: close\r\n\r\nno");
                        }
                    }
                });
            }
        });
        Self { base, routes, seen }
    }
    fn route(&self, key: &str, body: impl Into<Vec<u8>>) {
        self.routes.lock().unwrap().insert(key.to_string(), body.into());
    }
}

fn sha1hex(b: &[u8]) -> String {
    sha1::Sha1::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

fn version(fake: &Fake, id: &str, project: &str, jar: &[u8], deps: serde_json::Value) -> serde_json::Value {
    json!({
        "id": id, "project_id": project, "name": id, "version_number": format!("{id}-v"),
        "version_type": "release", "game_versions": ["1.21.1"], "loaders": ["fabric"],
        "date_published": "2026-10-01T00:00:00Z", "downloads": 3, "changelog": "cambios",
        "dependencies": deps,
        "files": [{ "hashes": { "sha1": sha1hex(jar), "sha512": "x" }, "url": format!("{}/cdn/{id}.jar", fake.base),
                    "filename": format!("{id}.jar"), "primary": true, "size": jar.len(), "file_type": null }]
    })
}

fn project(id: &str, side: &str) -> serde_json::Value {
    json!({ "id": id, "slug": id.to_lowercase(), "title": format!("Proyecto {id}"), "description": "d",
            "body": "# Hola", "client_side": "optional", "server_side": side, "downloads": 9,
            "categories": ["utility"], "license": { "id": "MIT", "name": "MIT License" }, "icon_url": null })
}

fn fabric() -> Platform {
    Platform {
        kind: ContentKind::Mod,
        folder: "mods".into(),
        loaders: vec!["fabric".into()],
        experimental: false,
        sources: vec![],
    }
}

#[test]
fn search_plan_install_and_detect_installed() {
    let fake = Fake::start();
    let http = Http::new();
    let m = Modrinth::new(&http, format!("{}/v2", fake.base));

    fake.route(
        "GET /v2/search",
        json!({ "hits": [{ "project_id": "AAA", "slug": "aaa", "title": "Mod A", "author": "ana",
            "description": "desc", "icon_url": "https://cdn.modrinth.com/i.png", "downloads": 42,
            "display_categories": ["fabric"], "client_side": "optional", "server_side": "required",
            "date_modified": "2026-10-01", "license": "MIT" }], "offset": 0, "limit": 20, "total_hits": 1 })
        .to_string(),
    );
    let page = m
        .search(&fabric(), Some("1.21.1"), "mapa á", SearchSort::Downloads, 0)
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.hits[0].server_side, Side::Required);
    let req = fake.seen.lock().unwrap()[0].0.clone();
    assert!(req.contains("query=mapa%20%C3%A1"), "{req}");
    assert!(req.contains("index=downloads"));
    assert!(req.contains("%22categories%3Afabric%22"));

    let jar_a = b"jar de A".to_vec();
    let jar_b = b"jar de B (dependencia)".to_vec();
    let va = version(
        &fake,
        "va1",
        "AAA",
        &jar_a,
        json!([{ "project_id": "BBB", "dependency_type": "required" }]),
    );
    let vb = version(&fake, "vb1", "BBB", &jar_b, json!([]));
    fake.route("GET /v2/version/va1", va.to_string());
    fake.route("GET /v2/project/AAA", project("AAA", "required").to_string());
    fake.route("GET /v2/project/BBB", project("BBB", "optional").to_string());
    fake.route("GET /v2/project/BBB/version", json!([vb]).to_string());
    fake.route("GET /cdn/va1.jar", jar_a.clone());
    fake.route("GET /cdn/vb1.jar", jar_b.clone());

    let server = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(server.path().join("mods")).unwrap();
    // Un archivo con el mismo nombre ya existente va a la papelera, no se pisa.
    std::fs::write(server.path().join("mods/vb1.jar"), "viejo").unwrap();

    let plan = build_plan(&m, &fabric(), Some("1.21.1"), "va1", &[]).unwrap();
    assert!(plan.blockers.is_empty(), "{:?}", plan.blockers);
    assert_eq!(plan.items.len(), 2);
    let staging = server
        .path()
        .parent()
        .unwrap()
        .join(format!("staging-{}", std::process::id()));
    let mut trashed = vec![];
    let placed = apply_plan(
        &http,
        server.path(),
        "mods",
        ".jar",
        &plan,
        &staging,
        &mut |rel| {
            trashed.push(rel.to_string());
            std::fs::remove_file(server.path().join(rel)).map_err(|e| e.to_string())
        },
        &InstallProgress::default(),
    )
    .unwrap();
    assert_eq!(placed, ["va1.jar", "vb1.jar"]);
    assert_eq!(trashed, ["mods/vb1.jar"]);
    assert_eq!(std::fs::read(server.path().join("mods/vb1.jar")).unwrap(), jar_b);
    assert!(!staging.exists());

    // Detectar lo instalado por huella y avisar de actualizaciones.
    let mut items = list_local(server.path(), "mods", ".jar").unwrap();
    let va2 = version(&fake, "va2", "AAA", b"A nueva", json!([]));
    fake.route(
        "POST /v2/version_files",
        json!({ sha1hex(&jar_a): va, sha1hex(&jar_b): vb }).to_string(),
    );
    fake.route(
        "POST /v2/version_files/update",
        json!({ sha1hex(&jar_a): va2, sha1hex(&jar_b): vb }).to_string(),
    );
    fake.route(
        "GET /v2/projects",
        json!([project("AAA", "required"), project("BBB", "optional")]).to_string(),
    );
    enrich(&m, &fabric(), Some("1.21.1"), &mut items).unwrap();
    let a = items.iter().find(|i| i.file_name == "va1.jar").unwrap();
    assert_eq!(a.title.as_deref(), Some("Proyecto AAA"));
    assert_eq!(a.update.as_ref().unwrap().version_id, "va2");
    assert!(items
        .iter()
        .find(|i| i.file_name == "vb1.jar")
        .unwrap()
        .update
        .is_none());
    let posts: Vec<(String, String)> = fake
        .seen
        .lock()
        .unwrap()
        .iter()
        .filter(|(r, _)| r.starts_with("POST"))
        .cloned()
        .collect();
    assert!(posts[0].1.contains("\"algorithm\":\"sha1\""));
    assert!(posts[1].1.contains("\"game_versions\":[\"1.21.1\"]"));
}

#[test]
fn corrupted_download_changes_nothing() {
    let fake = Fake::start();
    let http = Http::new();
    let m = Modrinth::new(&http, format!("{}/v2", fake.base));
    let v = version(&fake, "vx", "XXX", b"lo que se publica", json!([]));
    fake.route("GET /v2/version/vx", v.to_string());
    fake.route("GET /v2/project/XXX", project("XXX", "required").to_string());
    fake.route("GET /cdn/vx.jar", b"otra cosa".to_vec());
    let server = tempfile::tempdir().unwrap();
    let plan = build_plan(&m, &fabric(), Some("1.21.1"), "vx", &[]).unwrap();
    let staging = server.path().join("..").join(format!("st-x-{}", std::process::id()));
    let r = apply_plan(
        &http,
        server.path(),
        "mods",
        ".jar",
        &plan,
        &staging,
        &mut |_| Ok(()),
        &InstallProgress::default(),
    );
    assert!(matches!(r, Err(ContentError::Integrity(_))), "{r:?}");
    assert!(!server.path().join("mods").exists(), "no se tocó el servidor");
    assert!(!staging.exists());
}

fn neoforge() -> Platform {
    Platform {
        kind: ContentKind::Mod,
        folder: "mods".into(),
        loaders: vec!["neoforge".into()],
        experimental: false,
        sources: vec![],
    }
}

fn cf_file(fake: &Fake, id: u64, mod_id: u64, jar: &[u8], url: bool, deps: serde_json::Value) -> serde_json::Value {
    json!({
        "id": id, "modId": mod_id, "displayName": format!("archivo-{id}"), "fileName": format!("archivo-{id}.jar"),
        "releaseType": 1, "fileDate": format!("2026-10-0{}T00:00:00Z", id % 9 + 1), "fileLength": jar.len(),
        "downloadCount": 1,
        "downloadUrl": if url { json!(format!("{}/files/{id}.jar", fake.base)) } else { json!(null) },
        "gameVersions": ["1.21.1", "NeoForge", "Server"],
        "hashes": [{ "value": sha1hex(jar), "algo": 1 }],
        "dependencies": deps, "fileFingerprint": fingerprint(jar)
    })
}

#[test]
fn curseforge_search_install_and_fingerprints() {
    let fake = Fake::start();
    let http = Http::new();
    let cf = CurseForge::new(&http, fake.base.clone(), "clave-de-prueba");
    let modj = |id: u64, allow: serde_json::Value| {
        json!({ "id": id, "name": format!("Mod {id}"), "slug": format!("mod-{id}"), "summary": "s",
                "downloadCount": 10, "allowModDistribution": allow, "authors": [{ "name": "autora" }],
                "links": { "websiteUrl": format!("https://www.curseforge.com/minecraft/mc-mods/mod-{id}") } })
    };
    fake.route(
        "GET /v1/mods/search",
        json!({ "data": [modj(10, json!(true))], "pagination": { "totalCount": 1 } }).to_string(),
    );
    let page = cf
        .search(&neoforge(), Some("1.21.1"), "mapa", SearchSort::Relevance, 0)
        .unwrap();
    assert_eq!(page.hits[0].author, "autora");
    let req = fake.seen.lock().unwrap()[0].0.clone();
    assert!(
        req.contains("gameId=432") && req.contains("classId=6") && req.contains("modLoaderType=6"),
        "{req}"
    );
    assert!(
        req.ends_with("[clave clave-de-prueba]"),
        "la clave va en la cabecera: {req}"
    );
    assert!(
        !req.split(' ').nth(1).unwrap().contains("clave"),
        "la clave no va en la URL"
    );

    let jar = b"mod 10".to_vec();
    let dep = b"dependencia 20".to_vec();
    let f10 = cf_file(&fake, 101, 10, &jar, true, json!([{ "modId": 20, "relationType": 3 }]));
    let f20 = cf_file(&fake, 201, 20, &dep, true, json!([]));
    fake.route("POST /v1/mods/files", json!({ "data": [f10] }).to_string());
    fake.route("GET /v1/mods/10", json!({ "data": modj(10, json!(true)) }).to_string());
    fake.route("GET /v1/mods/20", json!({ "data": modj(20, json!(null)) }).to_string());
    fake.route("GET /v1/mods/20/files", json!({ "data": [f20] }).to_string());
    fake.route("GET /files/101.jar", jar.clone());
    fake.route("GET /files/201.jar", dep.clone());
    let plan = build_plan(&cf, &neoforge(), Some("1.21.1"), "101", &[]).unwrap();
    assert!(plan.blockers.is_empty(), "{:?}", plan.blockers);
    assert_eq!(plan.items.len(), 2);
    let server = tempfile::tempdir().unwrap();
    let staging = server
        .path()
        .parent()
        .unwrap()
        .join(format!("st-cf-{}", std::process::id()));
    apply_plan(
        &http,
        server.path(),
        "mods",
        ".jar",
        &plan,
        &staging,
        &mut |_| Ok(()),
        &InstallProgress::default(),
    )
    .unwrap();

    // Reconocer lo instalado por huella y avisar de una versión más nueva.
    let f11 = cf_file(&fake, 107, 10, b"mod 10 nuevo", true, json!([]));
    fake.route(
        "POST /v1/fingerprints/432",
        json!({ "data": { "exactMatches": [
            { "id": 10, "file": f10, "latestFiles": [f10, f11] },
            { "id": 20, "file": f20, "latestFiles": [f20] }
        ] } })
        .to_string(),
    );
    fake.route(
        "POST /v1/mods",
        json!({ "data": [modj(10, json!(true)), modj(20, json!(true))] }).to_string(),
    );
    let mut items = list_local(server.path(), "mods", ".jar").unwrap();
    enrich_curseforge(&cf, server.path(), &neoforge(), Some("1.21.1"), &mut items).unwrap();
    let a = items.iter().find(|i| i.file_name == "archivo-101.jar").unwrap();
    assert_eq!(a.title.as_deref(), Some("Mod 10"));
    assert_eq!(a.source, Some(merc_content::ContentSource::Curseforge));
    assert_eq!(a.update.as_ref().unwrap().version_id, "107");

    // Si el autor no permite descargas desde otras apps: bloqueado, con explicación.
    let f30 = cf_file(&fake, 301, 30, b"x", false, json!([]));
    fake.route("POST /v1/mods/files", json!({ "data": [f30] }).to_string());
    fake.route("GET /v1/mods/30", json!({ "data": modj(30, json!(false)) }).to_string());
    let plan = build_plan(&cf, &neoforge(), Some("1.21.1"), "301", &[]).unwrap();
    assert!(
        plan.blockers[0].contains("no permite descargarlo"),
        "{:?}",
        plan.blockers
    );
}

fn paper() -> Platform {
    Platform {
        kind: ContentKind::Plugin,
        folder: "plugins".into(),
        loaders: vec!["paper".into(), "spigot".into(), "bukkit".into()],
        experimental: false,
        sources: vec![],
    }
}

fn sha256hex(b: &[u8]) -> String {
    use sha2::Digest as _;
    sha2::Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn hangar_search_install_with_sha256_and_recognize() {
    use merc_content::hangar::{enrich_hangar, Hangar, HangarCache};
    let fake = Fake::start();
    let http = Http::new();
    let h = Hangar::new(&http, fake.base.clone());
    let proj = |id: u64, name: &str| {
        json!({ "id": id, "name": name, "namespace": { "owner": "Autora", "slug": name },
                "stats": { "downloads": 5 }, "category": "chat", "lastUpdated": "2026-10-01T00:00:00Z",
                "avatarUrl": "https://hangarcdn.papermc.io/avatars/project/1.webp", "description": "d",
                "settings": { "license": { "name": "MIT" }, "links": [] } })
    };
    let ver = |id: u64, project: u64, name: &str, jar: &[u8], date: &str, deps: serde_json::Value| {
        json!({ "id": id, "projectId": project, "name": name, "createdAt": date, "description": "c",
                "stats": { "totalDownloads": 1 }, "channel": { "name": "Release" },
                "downloads": { "PAPER": { "fileInfo": { "name": format!("{name}.jar"), "sizeBytes": jar.len(),
                    "sha256Hash": sha256hex(jar) }, "externalUrl": null,
                    "downloadUrl": format!("{}/cdn/{id}.jar", fake.base) } },
                "pluginDependencies": { "PAPER": deps }, "platformDependencies": { "PAPER": ["1.21.1"] } })
    };
    fake.route(
        "GET /projects",
        json!({ "pagination": { "count": 1, "limit": 20, "offset": 0 }, "result": [proj(1, "Chat")] }).to_string(),
    );
    let page = h
        .search(&paper(), Some("1.21.1"), "chat", SearchSort::Relevance, 0)
        .unwrap();
    assert_eq!(
        (page.hits[0].project_id.as_str(), page.hits[0].author.as_str()),
        ("1", "Autora")
    );
    let req = fake.seen.lock().unwrap()[0].0.clone();
    assert!(
        req.contains("platform=PAPER") && req.contains("version=1.21.1") && req.contains("q=chat"),
        "{req}"
    );

    let jar = b"plugin chat".to_vec();
    let lib = b"libreria".to_vec();
    let v11 = ver(
        11,
        1,
        "Chat-1.0",
        &jar,
        "2026-09-01T00:00:00Z",
        json!([{ "name": "Lib", "projectId": 2, "required": true, "externalUrl": null, "platform": "PAPER" },
               { "name": "Vault", "projectId": null, "required": true, "externalUrl": "https://github.com/MilkBowl/Vault", "platform": "PAPER" }]),
    );
    let v21 = ver(21, 2, "Lib-2.0", &lib, "2026-09-01T00:00:00Z", json!([]));
    fake.route("GET /versions/11", v11.to_string());
    fake.route("GET /projects/1", proj(1, "Chat").to_string());
    fake.route("GET /projects/2", proj(2, "Lib").to_string());
    fake.route(
        "GET /projects/2/versions",
        json!({ "pagination": {}, "result": [v21] }).to_string(),
    );
    fake.route("GET /cdn/11.jar", jar.clone());
    fake.route("GET /cdn/21.jar", lib.clone());
    let plan = build_plan(&h, &paper(), Some("1.21.1"), "11", &[]).unwrap();
    assert!(plan.blockers.is_empty(), "{:?}", plan.blockers);
    assert_eq!(plan.items.len(), 2);
    assert!(plan.items[0].sha1.is_empty() && plan.items[0].sha256.is_some());
    assert!(plan.warnings.iter().any(|w| w.contains("Vault")), "{:?}", plan.warnings);
    let server = tempfile::tempdir().unwrap();
    let staging = server
        .path()
        .parent()
        .unwrap()
        .join(format!("st-h-{}", std::process::id()));
    apply_plan(
        &http,
        server.path(),
        "plugins",
        ".jar",
        &plan,
        &staging,
        &mut |_| Ok(()),
        &InstallProgress::default(),
    )
    .unwrap();
    assert_eq!(std::fs::read(server.path().join("plugins/Chat-1.0.jar")).unwrap(), jar);

    // Un archivo alterado no pasa la verificación SHA-256.
    fake.route("GET /cdn/11.jar", b"otro contenido".to_vec());
    let s2 = server
        .path()
        .parent()
        .unwrap()
        .join(format!("st-h2-{}", std::process::id()));
    let other = tempfile::tempdir().unwrap();
    let r = apply_plan(
        &http,
        other.path(),
        "plugins",
        ".jar",
        &plan,
        &s2,
        &mut |_| Ok(()),
        &InstallProgress::default(),
    );
    assert!(matches!(r, Err(ContentError::Integrity(_))), "{r:?}");

    // Reconocer lo instalado por SHA-256 y ver la actualización.
    let v12 = ver(12, 1, "Chat-1.1", b"nuevo", "2026-10-01T00:00:00Z", json!([]));
    fake.route(
        &format!("GET /versions/hash/{}", sha256hex(&jar)),
        proj(1, "Chat").to_string(),
    );
    fake.route(
        "GET /projects/1/versions",
        json!({ "pagination": {}, "result": [v11, v12] }).to_string(),
    );
    let mut items = list_local(server.path(), "plugins", ".jar").unwrap();
    let mut cache = HangarCache::default();
    enrich_hangar(&h, &mut cache, server.path(), &paper(), Some("1.21.1"), &mut items).unwrap();
    let chat = items.iter().find(|i| i.file_name == "Chat-1.0.jar").unwrap();
    assert_eq!(chat.source, Some(merc_content::ContentSource::Hangar));
    assert_eq!(chat.version_id.as_deref(), Some("11"));
    assert_eq!(chat.update.as_ref().unwrap().version_id, "12");
    // Lib no está en el servidor falso por hash: se queda sin reconocer, sin error.
    assert!(items
        .iter()
        .find(|i| i.file_name == "Lib-2.0.jar")
        .unwrap()
        .source
        .is_none());
    // La segunda vez sale de la caché: ninguna petición nueva.
    let before = fake.seen.lock().unwrap().len();
    let mut items = list_local(server.path(), "plugins", ".jar").unwrap();
    enrich_hangar(&h, &mut cache, server.path(), &paper(), Some("1.21.1"), &mut items).unwrap();
    assert_eq!(fake.seen.lock().unwrap().len(), before);
}

#[test]
fn modrinth_datapack_installs_as_zip_in_world() {
    let fake = Fake::start();
    let http = Http::new();
    let m = Modrinth::new(&http, fake.base.clone());
    let zip = b"PK datapack".to_vec();
    let mut v = version(&fake, "dp1", "DP", &zip, json!([]));
    v["loaders"] = json!(["datapack"]);
    v["files"][0]["filename"] = json!("terreno.zip");
    v["files"][0]["url"] = json!(format!("{}/cdn/dp1.zip", fake.base));
    fake.route("GET /version/dp1", v.to_string());
    fake.route("GET /project/DP", project("DP", "required").to_string());
    fake.route("GET /cdn/dp1.zip", zip.clone());
    let dp = Platform {
        kind: ContentKind::Datapack,
        folder: "world/datapacks".into(),
        loaders: vec!["datapack".into()],
        experimental: false,
        sources: vec![],
    };
    let plan = build_plan(&m, &dp, Some("1.21.1"), "dp1", &[]).unwrap();
    assert!(plan.blockers.is_empty(), "{:?}", plan.blockers);
    let server = tempfile::tempdir().unwrap();
    let staging = server
        .path()
        .parent()
        .unwrap()
        .join(format!("st-dp-{}", std::process::id()));
    apply_plan(
        &http,
        server.path(),
        &dp.folder,
        ".zip",
        &plan,
        &staging,
        &mut |_| Ok(()),
        &InstallProgress::default(),
    )
    .unwrap();
    assert!(server.path().join("world/datapacks/terreno.zip").is_file());
    let l = list_local(server.path(), &dp.folder, ".zip").unwrap();
    assert_eq!(l.len(), 1);
    // Un .jar no es un datapack.
    let mut j = v.clone();
    j["files"][0]["filename"] = json!("x.jar");
    fake.route("GET /version/dp1", j.to_string());
    let plan = build_plan(&m, &dp, Some("1.21.1"), "dp1", &[]).unwrap();
    assert!(plan.blockers.iter().any(|b| b.contains(".zip")), "{:?}", plan.blockers);
}

#[test]
fn hangar_external_download_is_blocked_with_explanation() {
    use merc_content::hangar::Hangar;
    let fake = Fake::start();
    let http = Http::new();
    let h = Hangar::new(&http, fake.base.clone());
    fake.route(
        "GET /versions/5",
        json!({ "id": 5, "projectId": 3, "name": "Ext-1.0", "createdAt": "2026-10-01T00:00:00Z",
                "channel": { "name": "Release" },
                "downloads": { "PAPER": { "fileInfo": null, "externalUrl": "https://autor.dev/ext.jar", "downloadUrl": null } },
                "pluginDependencies": {}, "platformDependencies": { "PAPER": ["1.21.1"] } })
        .to_string(),
    );
    fake.route(
        "GET /projects/3",
        json!({ "id": 3, "name": "Ext", "namespace": { "owner": "A", "slug": "Ext" } }).to_string(),
    );
    let plan = build_plan(&h, &paper(), Some("1.21.1"), "5", &[]).unwrap();
    assert!(plan.blockers[0].contains("otra web"), "{:?}", plan.blockers);
    assert!(plan.items.is_empty());
}
