//! Supporter Edition contra un Gumroad falso: activar, revisar, reembolso, sin red y nueva PC.

use merc_ipc::*;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// clave → (código HTTP, JSON). Guarda cada cuerpo recibido.
#[derive(Default)]
struct Gumroad {
    keys: HashMap<String, (u16, String)>,
    seen: Vec<String>,
}

fn fake_gumroad(g: Arc<Mutex<Gumroad>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!(
        "http://127.0.0.1:{}/v2/licenses/verify",
        listener.local_addr().unwrap().port()
    );
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let g = g.clone();
            std::thread::spawn(move || {
                let mut r = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                let _ = r.read_line(&mut line);
                let mut len = 0usize;
                loop {
                    let mut h = String::new();
                    if r.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                        break;
                    }
                    if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0u8; len];
                let _ = r.read_exact(&mut body);
                let body = String::from_utf8_lossy(&body).to_string();
                let key = body
                    .split('&')
                    .find_map(|kv| kv.strip_prefix("license_key="))
                    .unwrap_or("")
                    .to_string();
                let (status, json) = {
                    let mut g = g.lock().unwrap();
                    g.seen.push(body.clone());
                    g.keys.get(&key).cloned().unwrap_or((
                        404,
                        r#"{"success":false,"message":"That license does not exist for the provided product."}"#.into(),
                    ))
                };
                let mut s = stream;
                let _ = write!(
                    s,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
                    json.len()
                );
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

fn spawn(data: &Path, api: &str, product: &str) -> (Daemon, Client) {
    let d = Daemon(
        Command::new(env!("CARGO_BIN_EXE_merc-supervisord"))
            .arg("--data-dir")
            .arg(data)
            .env("MERC_GUMROAD_API", api)
            .env("MERC_GUMROAD_PRODUCT", product)
            .env("MERC_SUPPORTER_CHECKOUT", "https://example.gumroad.com/l/merchosting")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match Client::connect(data, "pruebas") {
            Ok(c) => return (d, c),
            Err(e) if Instant::now() > deadline => panic!("no se pudo conectar: {e}"),
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

fn info(c: &mut Client, body: RequestBody) -> Result<SupporterInfo, ClientError> {
    match c.call(body)? {
        ResponseBody::Supporter(i) => Ok(i),
        other => panic!("{other:?}"),
    }
}

const KEY: &str = "ABCD1234-EFGH5678-IJKL9012-MNOP3456";

fn purchase(refunded: bool) -> (u16, String) {
    (
        200,
        format!(
            r#"{{"success":true,"uses":1,"purchase":{{"product_id":"PROD1","email":"amir@gmail.com","sale_timestamp":"2026-10-30T20:00:00Z","refunded":{refunded},"chargebacked":false,"disputed":false}}}}"#
        ),
    )
}

#[test]
fn activate_refresh_refund_offline_and_new_pc() {
    let tmp = tempfile::tempdir().unwrap();
    let g = Arc::new(Mutex::new(Gumroad::default()));
    g.lock().unwrap().keys.insert(KEY.into(), purchase(false));
    let api = fake_gumroad(g.clone());
    let data = tmp.path().join("pc1");

    // Sin producto configurado: no se puede activar.
    {
        let (_d, mut c) = spawn(&data, &api, "");
        let i = info(&mut c, RequestBody::Supporter).unwrap();
        assert!(!i.available && !i.active);
        assert!(matches!(
            info(&mut c, RequestBody::ActivateSupporter { license_key: KEY.into() }),
            Err(ClientError::Api(e)) if e.code == ErrorCode::InvalidState
        ));
    }

    let (d, mut c) = spawn(&data, &api, "PROD1");
    let i = info(&mut c, RequestBody::Supporter).unwrap();
    assert!(i.available && !i.active);
    assert_eq!(i.price_usd, 8);
    assert_eq!(
        i.checkout_url.as_deref(),
        Some("https://example.gumroad.com/l/merchosting")
    );

    // Clave inventada: Gumroad la rechaza y no se guarda nada.
    let e = info(
        &mut c,
        RequestBody::ActivateSupporter {
            license_key: "FAKE0000-FAKE0000".into(),
        },
    )
    .unwrap_err();
    assert!(
        matches!(&e, ClientError::Api(e) if e.code == ErrorCode::BadRequest && e.message.contains("does not exist")),
        "{e:?}"
    );
    assert!(!info(&mut c, RequestBody::Supporter).unwrap().active);

    // Clave real (con espacios al copiarla).
    let i = info(
        &mut c,
        RequestBody::ActivateSupporter {
            license_key: format!("  {KEY}\n"),
        },
    )
    .unwrap();
    assert!(i.active, "{i:?}");
    assert_eq!(i.email_hint.as_deref(), Some("a•••@gmail.com"));
    assert_eq!(i.key_hint.as_deref(), Some("…3456"));
    assert_eq!(i.purchased.as_deref(), Some("2026-10-30"));
    // La activación cuenta un uso; repetirla (doble clic) no rompe nada.
    assert!(g
        .lock()
        .unwrap()
        .seen
        .last()
        .unwrap()
        .contains("increment_uses_count=true"));
    assert!(
        info(
            &mut c,
            RequestBody::ActivateSupporter {
                license_key: KEY.into()
            }
        )
        .unwrap()
        .active
    );
    assert!(g.lock().unwrap().seen.last().unwrap().contains("product_id=PROD1"));

    // Revisión sin red: se conserva lo que había.
    drop(d);
    let (_d, mut c) = spawn(&data, "http://127.0.0.1:9/v2/licenses/verify", "PROD1");
    assert!(
        info(&mut c, RequestBody::Supporter).unwrap().active,
        "sigue activo tras reiniciar"
    );
    let i = info(&mut c, RequestBody::RefreshSupporter).unwrap();
    assert!(i.active && i.last_error.is_some(), "{i:?}");
    drop(_d);

    // Reembolso: la siguiente revisión lo desactiva (sin contar un uso nuevo).
    let (_d, mut c) = spawn(&data, &api, "PROD1");
    g.lock().unwrap().keys.insert(KEY.into(), purchase(true));
    let i = info(&mut c, RequestBody::RefreshSupporter).unwrap();
    assert!(!i.active);
    assert_eq!(i.revoked_reason.as_deref(), Some("La compra se reembolsó."));
    assert!(g
        .lock()
        .unwrap()
        .seen
        .last()
        .unwrap()
        .contains("increment_uses_count=false"));

    // Otra PC: misma clave, mismo resultado; y olvidarla la quita de esa PC.
    g.lock().unwrap().keys.insert(KEY.into(), purchase(false));
    let (_d2, mut c2) = spawn(&tmp.path().join("pc2"), &api, "PROD1");
    assert!(
        info(
            &mut c2,
            RequestBody::ActivateSupporter {
                license_key: KEY.into()
            }
        )
        .unwrap()
        .active
    );
    let i = info(&mut c2, RequestBody::ForgetSupporter).unwrap();
    assert!(!i.active && i.key_hint.is_none());
    assert!(!tmp.path().join("pc2").join("supporter.json").exists());
}
