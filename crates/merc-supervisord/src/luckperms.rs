//! LuckPerms: leer grupos y usuarios con `luckperms export` y aplicar cambios validados.
//!
//! - Leer exige el servidor encendido: quien sabe abrir la base de datos de LuckPerms (H2,
//!   MySQL…) es LuckPerms. MercHosting le pide un export a un archivo con nombre único dentro
//!   de su carpeta de datos, lo lee cuando está completo y lo borra.
//! - Antes de cada cambio se guarda ese export como copia en
//!   `instancias/<id>/luckperms-copias/` (sirve para `luckperms import`). Se guardan las 10
//!   últimas.
//! - Después del cambio se hace otro export para comprobar que LuckPerms lo aplicó. Si no se ve,
//!   se dice: no se da por hecho.

use super::{admin_err, api, sup_err, Daemon};
use merc_admin::luckperms::{self as lp, LpInstall, LpSnapshot};
use merc_core::{Instance, InstanceId, RuntimeState};
use merc_ipc::*;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Una operación de LuckPerms a la vez (cada una son varios comandos seguidos).
static LOCK: Mutex<()> = Mutex::new(());
const KEEP_BACKUPS: usize = 10;

fn export_timeout() -> Duration {
    if cfg!(debug_assertions) {
        if let Some(ms) = std::env::var("MERC_LP_TIMEOUT_MS").ok().and_then(|v| v.parse().ok()) {
            return Duration::from_millis(ms);
        }
    }
    Duration::from_secs(30)
}

impl Daemon {
    fn lp_backup_dir(&self, id: InstanceId) -> PathBuf {
        self.store_dir(id).join("luckperms-copias")
    }

    fn lp_backups(&self, id: InstanceId) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(self.lp_backup_dir(id))
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| e.file_name().into_string().ok())
                    .filter(|n| n.ends_with(".json.gz"))
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v.reverse();
        v
    }

    fn lp_install(&self, inst: &Instance) -> Result<LpInstall, ApiError> {
        lp::detect(&inst.root, inst.kind).ok_or_else(|| {
            api(
                ErrorCode::NotFound,
                "LuckPerms no está instalado en este servidor (no hay un .jar en plugins ni en mods).",
            )
        })
    }

    /// Pide un export a LuckPerms y lo lee. Devuelve los bytes (para la copia) y los datos.
    fn lp_export(&self, inst: &Instance, install: &LpInstall) -> Result<(Vec<u8>, LpSnapshot), ApiError> {
        if self.sup.status(inst.id).state != RuntimeState::Online {
            return Err(api(
                ErrorCode::InvalidState,
                "Enciende el servidor: los permisos los lee LuckPerms.",
            ));
        }
        let dir = lp::data_path(&inst.root, install);
        let name = format!(
            "merchosting-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        );
        let file = dir.join(format!("{name}.json.gz"));
        self.sup
            .send_command(inst.id, &format!("{} export {name}", lp::COMMAND))
            .map_err(sup_err)?;
        let deadline = Instant::now() + export_timeout();
        let mut last_err = None;
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(200));
            let Ok(bytes) = std::fs::read(&file) else { continue };
            if bytes.is_empty() {
                continue;
            }
            match lp::parse_export(&bytes) {
                Ok(snap) => {
                    let _ = std::fs::remove_file(&file);
                    return Ok((bytes, snap));
                }
                // Puede estar a medio escribir: se reintenta hasta el límite.
                Err(e) => last_err = Some(e.to_string()),
            }
        }
        let _ = std::fs::remove_file(&file);
        Err(api(
            ErrorCode::InvalidState,
            match last_err {
                Some(e) => format!("No se pudo leer lo que exportó LuckPerms: {e}"),
                None => format!(
                    "LuckPerms no respondió a «{} export» a tiempo. Mira la consola por si dice por qué.",
                    lp::COMMAND
                ),
            },
        ))
    }

    fn lp_save_backup(&self, id: InstanceId, bytes: &[u8]) -> Result<String, ApiError> {
        let dir = self.lp_backup_dir(id);
        std::fs::create_dir_all(&dir).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        let mut name = format!("{}.json.gz", chrono_name());
        // Muy improbable con milisegundos; si pasa, se espera al siguiente.
        while dir.join(&name).exists() {
            std::thread::sleep(Duration::from_millis(2));
            name = format!("{}.json.gz", chrono_name());
        }
        std::fs::write(dir.join(&name), bytes).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        for old in self.lp_backups(id).into_iter().skip(KEEP_BACKUPS) {
            let _ = std::fs::remove_file(dir.join(old));
        }
        Ok(name)
    }

    pub(crate) fn handle_luckperms(
        self: &Arc<Self>,
        client: &str,
        body: RequestBody,
    ) -> Result<ResponseBody, ApiError> {
        match body {
            RequestBody::LuckPerms { id } => {
                let inst = self.instance(id)?;
                let online = self.sup.status(id).state == RuntimeState::Online;
                let install = lp::detect(&inst.root, inst.kind);
                let snapshot = match (&install, online) {
                    (Some(i), true) => {
                        let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
                        Some(self.lp_export(&inst, i)?.1)
                    }
                    _ => None,
                };
                Ok(ResponseBody::LuckPerms(Box::new(LuckPermsInfo {
                    install,
                    online,
                    snapshot,
                    backups: self.lp_backups(id),
                })))
            }
            RequestBody::LuckPermsPreview { id, change } => {
                let inst = self.instance(id)?;
                let install = self.lp_install(&inst)?;
                let command = lp::change_command(&change).map_err(admin_err)?;
                let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
                let (_, snap) = self.lp_export(&inst, &install)?;
                lp::precheck(&snap, &change).map_err(admin_err)?;
                Ok(ResponseBody::LuckPermsPreview(LuckPermsPreview {
                    command,
                    summary: lp::describe(&snap, &change),
                }))
            }
            RequestBody::LuckPermsApply { id, change } => {
                let inst = self.instance(id)?;
                let install = self.lp_install(&inst)?;
                let command = lp::change_command(&change).map_err(admin_err)?;
                let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
                let (bytes, before) = self.lp_export(&inst, &install)?;
                lp::precheck(&before, &change).map_err(admin_err)?;
                let summary = lp::describe(&before, &change);
                let backup = self.lp_save_backup(id, &bytes)?;
                self.sup.send_command(id, &command).map_err(sup_err)?;
                // LuckPerms guarda el cambio de forma asíncrona: se mira un par de veces.
                let mut verified = false;
                let mut snapshot = before;
                for _ in 0..3 {
                    std::thread::sleep(Duration::from_millis(300));
                    let (_, s) = self.lp_export(&inst, &install)?;
                    verified = lp::change_applied(&s, &change);
                    snapshot = s;
                    if verified {
                        break;
                    }
                }
                self.audit(
                    client,
                    Some(id),
                    "luckperms.change",
                    &format!(
                        "{summary} ({command}){}",
                        if verified { "" } else { " — sin confirmar" }
                    ),
                );
                Ok(ResponseBody::LuckPermsApplied(Box::new(LuckPermsApplied {
                    verified,
                    command,
                    backup,
                    snapshot,
                })))
            }
            _ => Err(api(ErrorCode::BadRequest, "Petición no válida")),
        }
    }
}

/// `2026-10-30-12-00-00-123` en hora local, con milisegundos: ordena bien aunque haya
/// varios cambios en el mismo segundo.
fn chrono_name() -> String {
    chrono::Local::now().format("%Y-%m-%d-%H-%M-%S-%3f").to_string()
}
