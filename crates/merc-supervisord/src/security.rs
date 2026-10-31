//! Centro de seguridad de un servidor: reúne los datos reales y los evalúa.

use super::{api, Daemon};
use merc_admin::security::{self, CompanionAccess, SecurityReport};
use merc_core::Instance;
use merc_ipc::*;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Nombres de archivo de claves que nunca deberían estar dentro de un servidor.
const SECRET_FILES: [&str; 4] = [
    "playit.toml",
    "llave-dispositivo",
    "curseforge-api-key.txt",
    "anfitrion.json",
];

impl Daemon {
    fn security_report(&self, inst: &Instance) -> SecurityReport {
        let props: BTreeMap<String, String> = std::fs::read_to_string(inst.root.join("server.properties"))
            .map(|t| {
                merc_admin::props::PropsDoc::parse(&t)
                    .entries()
                    .into_iter()
                    .map(|e| (e.key, e.value))
                    .collect()
            })
            .unwrap_or_default();
        let public_addresses: Vec<String> = self
            .connection(inst)
            .tunnels
            .into_iter()
            .filter(|t| !t.is_disabled)
            .map(|t| t.display_address)
            .collect();
        let companions: Vec<CompanionAccess> = self
            .companions_on(inst.id)
            .into_iter()
            .map(|(name, perms)| CompanionAccess {
                name,
                high_risk: perms
                    .into_iter()
                    .filter(|p| p.high_risk())
                    .map(|p| super::companions::perm_label(p).to_string())
                    .collect(),
            })
            .collect();
        let now = super::playit::now_ms();
        let week_ago = now / 1000 - 7 * 86_400;
        let denied_recently = self
            .store
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .recent_audit(Some(inst.id), 2000)
            .unwrap_or_default()
            .iter()
            .filter(|r| r.action == "companero.denegado" && r.at_unix >= week_ago)
            .count() as u32;
        let last_backup_ms = merc_admin::backups::list(&self.backups_dir(inst.id))
            .iter()
            .map(|b| b.created_unix_ms)
            .max();
        let backup_scheduled = self
            .load_tasks(inst.id)
            .iter()
            .any(|t| t.enabled && matches!(t.action, merc_admin::schedule::TaskAction::Backup { .. }));
        let secrets_in_server: Vec<String> = SECRET_FILES
            .iter()
            .filter(|f| inst.root.join(f).exists() || inst.root.join("config").join(f).exists())
            .map(|f| f.to_string())
            .collect();
        security::evaluate(&security::Inputs {
            props: &props,
            public_addresses: &public_addresses,
            companions: &companions,
            denied_recently,
            last_backup_ms,
            backup_scheduled,
            secrets_in_server: &secrets_in_server,
            now_ms: now,
        })
    }

    pub(crate) fn handle_security(self: &Arc<Self>, body: RequestBody) -> Result<ResponseBody, ApiError> {
        match body {
            RequestBody::SecurityReport { id } => Ok(ResponseBody::SecurityReport(Box::new(
                self.security_report(&self.instance(id)?),
            ))),
            _ => Err(api(ErrorCode::BadRequest, "Petición no válida")),
        }
    }
}
