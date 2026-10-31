//! Copias de seguridad en el supervisor: dónde se guardan, coordinación con el juego encendido
//! (pausar y reanudar el guardado) y restauración con copia previa.

use super::{admin_err, api, Daemon};
use merc_admin::backups::{self, BackupInfo, BackupReason, BackupScope, NewBackup};
use merc_core::{Instance, InstanceId, RuntimeState};
use merc_ipc::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

/// Copias sin proteger que se conservan si no se cambió el ajuste.
pub const DEFAULT_KEEP: u32 = 10;
const MAX_KEEP: u32 = 500;
/// Tiempo máximo esperando a que el juego confirme que guardó («Saved the game»).
const SAVE_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Settings {
    keep: u32,
}

impl Daemon {
    pub(crate) fn backups_dir(&self, id: InstanceId) -> PathBuf {
        self.store_dir(id).join("backups")
    }

    fn backup_settings_file(&self, id: InstanceId) -> PathBuf {
        self.store_dir(id).join("backups-ajustes.json")
    }

    fn backup_keep(&self, id: InstanceId) -> u32 {
        std::fs::read_to_string(self.backup_settings_file(id))
            .ok()
            .and_then(|t| serde_json::from_str::<Settings>(&t).ok())
            .map(|s| s.keep.clamp(1, MAX_KEEP))
            .unwrap_or(DEFAULT_KEEP)
    }

    fn backup_list(&self, id: InstanceId, restored: Option<String>) -> BackupList {
        let dir = self.backups_dir(id);
        BackupList {
            items: backups::list(&dir),
            keep: self.backup_keep(id),
            free_bytes: backups::free_space(&dir),
            folder: merc_core::paths::strip_verbatim(&dir).display().to_string(),
            restored,
        }
    }

    /// Espera una línea de consola posterior a `after` que contenga alguno de los textos.
    fn wait_console(&self, id: InstanceId, after: u64, needles: &[&str], timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if self
                .sup
                .console_since(id, after)
                .iter()
                .any(|l| needles.iter().any(|n| l.text.contains(n)))
            {
                return true;
            }
            if self.sup.status(id).state != RuntimeState::Online {
                return false;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        false
    }

    /// Hace una copia. Con el servidor encendido pausa el guardado automático, pide guardar todo
    /// y espera la confirmación; al terminar (bien o mal) lo reanuda.
    /// Hace una copia. Si falla una automática (programada, antes de restaurar o de actualizar),
    /// además de devolver el error se avisa con una alerta: nadie está mirando la pantalla.
    pub(crate) fn make_backup(
        &self,
        client: &str,
        inst: &Instance,
        scope: BackupScope,
        reason: BackupReason,
        note: Option<String>,
    ) -> Result<BackupInfo, ApiError> {
        let r = self.make_backup_inner(client, inst, scope, reason, note);
        if let (Err(e), false) = (&r, reason == BackupReason::Manual) {
            let what = match reason {
                BackupReason::Scheduled => "la copia programada",
                BackupReason::BeforeUpdate => "la copia previa a actualizar (no se actualizó)",
                BackupReason::BeforeRestore => "la copia previa a restaurar (no se restauró)",
                BackupReason::Manual => "la copia",
            };
            self.push_alert(
                merc_admin::health::AlertKind::BackupFailed,
                merc_admin::health::AlertLevel::Warning,
                Some((inst.id, inst.name.clone())),
                format!("Falló {what} de «{}»", inst.name),
                e.message.clone(),
            );
        }
        r
    }

    fn make_backup_inner(
        &self,
        client: &str,
        inst: &Instance,
        scope: BackupScope,
        reason: BackupReason,
        note: Option<String>,
    ) -> Result<BackupInfo, ApiError> {
        let id = inst.id;
        let st = self.sup.status(id);
        let hot = match st.state {
            s if !s.has_process() => false,
            RuntimeState::Online if st.has_console => true,
            RuntimeState::Online => {
                return Err(api(
                    ErrorCode::InvalidState,
                    "MercHosting perdió la consola de este servidor: no puede pausar el guardado. Detén el servidor y haz la copia en frío.",
                ))
            }
            _ => {
                return Err(api(
                    ErrorCode::InvalidState,
                    "Espera a que el servidor termine de encender o de apagarse para hacer la copia.",
                ))
            }
        };
        let level = merc_content::level_folder(&inst.root);
        let note = note
            .map(|n| n.trim().chars().take(200).collect::<String>())
            .filter(|n| !n.is_empty() && !n.chars().any(char::is_control));
        let mut consistent = !hot;
        if hot {
            let after = self.sup.console_since(id, 0).last().map(|l| l.seq).unwrap_or(0);
            let paused = self.sup.send_command(id, "save-off").is_ok();
            let flushed = paused && self.sup.send_command(id, "save-all flush").is_ok();
            consistent = flushed && self.wait_console(id, after, &["Saved the game"], SAVE_TIMEOUT);
        }
        let new = NewBackup {
            scope,
            reason,
            level_name: level.clone(),
            mc_version: inst.mc_version.clone(),
            hot,
            consistent,
            note,
        };
        let dir = self.backups_dir(id);
        let result = self.world_op(inst, "backup", &level, |p| backups::create(&inst.root, &dir, &new, p));
        if hot {
            // Siempre se reanuda el guardado, aunque la copia haya fallado.
            let _ = self.sup.send_command(id, "save-on");
        }
        let info = result?;
        // La copia previa a una restauración no limpia: podría borrar justo la que se restaura.
        let removed = if reason == BackupReason::BeforeRestore {
            vec![]
        } else {
            backups::apply_retention(&dir, self.backup_keep(id), &info.id)
        };
        self.audit(
            client,
            Some(id),
            "backup.create",
            &format!(
                "{} ({}, {} archivos, {} bytes{}{})",
                info.id,
                match scope {
                    BackupScope::World => "mundo",
                    BackupScope::Full => "completa",
                },
                info.files,
                info.size_bytes,
                if hot {
                    if consistent {
                        ", en caliente con guardado pausado"
                    } else {
                        ", en caliente SIN confirmar el guardado"
                    }
                } else {
                    ""
                },
                if removed.is_empty() {
                    String::new()
                } else {
                    format!("; se borraron por antigüedad: {}", removed.join(", "))
                }
            ),
        );
        Ok(info)
    }

    pub(crate) fn handle_backups(&self, client: &str, body: RequestBody) -> Result<ResponseBody, ApiError> {
        use RequestBody as R;
        Ok(match body {
            R::Backups { id } => {
                self.instance(id)?;
                ResponseBody::Backups(self.backup_list(id, None))
            }
            R::CreateBackup { id, scope, note } => {
                let inst = self.instance(id)?;
                self.make_backup(client, &inst, scope, BackupReason::Manual, note)?;
                ResponseBody::Backups(self.backup_list(id, None))
            }
            R::VerifyBackup { id, backup } => {
                let inst = self.instance(id)?;
                let dir = self.backups_dir(id);
                let info = self.world_op(&inst, "verify", &backup, |p| backups::verify(&dir, &backup, p))?;
                self.audit(
                    client,
                    Some(id),
                    "backup.verify",
                    &format!(
                        "{} {}",
                        info.id,
                        if info.verified == Some(true) {
                            "correcta".to_string()
                        } else {
                            format!("DAÑADA: {}", info.problems.join("; "))
                        }
                    ),
                );
                ResponseBody::Backups(self.backup_list(id, None))
            }
            R::RestoreBackup { id, backup } => {
                let inst = self.instance(id)?;
                self.require_stopped(id, "restaurar una copia")?;
                let dir = self.backups_dir(id);
                let target = backups::get(&dir, &backup).map_err(admin_err)?;
                // Copia de seguridad de lo actual, del mismo alcance, antes de tocar nada.
                let safety = match self.make_backup(
                    client,
                    &inst,
                    target.scope,
                    BackupReason::BeforeRestore,
                    Some(format!("Antes de restaurar {}", target.id)),
                ) {
                    Ok(i) => Some(i.id),
                    // El mundo actual no existe (por ejemplo, se borró): no hay nada que guardar.
                    Err(e) if e.code == ErrorCode::NotFound => None,
                    Err(e) => {
                        return Err(api(
                            e.code,
                            format!(
                                "No se restauró porque no se pudo hacer antes la copia de lo actual: {}",
                                e.message
                            ),
                        ))
                    }
                };
                self.require_stopped(id, "restaurar una copia")?;
                let tops = self.world_op(&inst, "restore", &backup, |p| {
                    p.done.store(0, Ordering::Relaxed);
                    backups::restore(&inst.root, &dir, &backup, p)
                })?;
                self.audit(
                    client,
                    Some(id),
                    "backup.restore",
                    &format!(
                        "{} -> {}{}",
                        backup,
                        tops.join(", "),
                        safety.map(|s| format!(" (copia previa {s})")).unwrap_or_default()
                    ),
                );
                ResponseBody::Backups(self.backup_list(id, Some(backup)))
            }
            R::DownloadBackup { id, backup_id, dest } => {
                self.instance(id)?;
                if !dest.is_absolute() {
                    return Err(api(ErrorCode::BadRequest, "Destino no válido"));
                }
                let dir = self.backups_dir(id);
                backups::get(&dir, &backup_id).map_err(admin_err)?;
                std::fs::copy(backups::zip_path(&dir, &backup_id), &dest)
                    .map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
                self.audit(client, Some(id), "backup.download", &backup_id);
                ResponseBody::FileDone {
                    path: dest.display().to_string(),
                }
            }
            R::DeleteBackup { id, backup } => {
                self.instance(id)?;
                if self.world_op_running(id) {
                    return Err(api(ErrorCode::Conflict, "Espera a que termine la operación en marcha."));
                }
                backups::delete(&self.backups_dir(id), &backup).map_err(admin_err)?;
                self.audit(client, Some(id), "backup.delete", &backup);
                ResponseBody::Backups(self.backup_list(id, None))
            }
            R::ProtectBackup { id, backup, protected } => {
                self.instance(id)?;
                backups::set_protected(&self.backups_dir(id), &backup, protected).map_err(admin_err)?;
                self.audit(
                    client,
                    Some(id),
                    if protected {
                        "backup.protect"
                    } else {
                        "backup.unprotect"
                    },
                    &backup,
                );
                ResponseBody::Backups(self.backup_list(id, None))
            }
            R::SetBackupSettings { id, keep } => {
                self.instance(id)?;
                if !(1..=MAX_KEEP).contains(&keep) {
                    return Err(api(
                        ErrorCode::BadRequest,
                        format!("Elige entre 1 y {MAX_KEEP} copias."),
                    ));
                }
                let f = self.backup_settings_file(id);
                if let Some(parent) = f.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
                }
                std::fs::write(&f, serde_json::to_string(&Settings { keep }).unwrap_or_default())
                    .map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
                self.audit(client, Some(id), "backup.settings", &format!("conservar {keep}"));
                ResponseBody::Backups(self.backup_list(id, None))
            }
            _ => return Err(api(ErrorCode::BadRequest, "Petición no válida")),
        })
    }
}
