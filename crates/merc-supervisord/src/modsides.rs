//! Separador de mods cliente/servidor (ver `merc_admin::modsides`).

use super::{admin_err, api, now_unix, Daemon};
use merc_admin::modsides::{self as ms, CatalogSides, ModSidesInfo};
use merc_content::{ContentKind, ContentSource};
use merc_core::Instance;
use merc_ipc::*;
use std::sync::Arc;

fn side_str(s: merc_content::modrinth::Side) -> String {
    serde_json::to_value(s)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".into())
}

impl Daemon {
    fn mod_sides(&self, inst: &Instance) -> Result<ModSidesInfo, ApiError> {
        let Some(platform) = self.platforms_of(inst).into_iter().find(|p| p.kind == ContentKind::Mod) else {
            return Err(api(ErrorCode::BadRequest, "Este servidor no usa mods."));
        };
        // Catálogo (Modrinth y, si hay clave, CurseForge): sin conexión se sigue con los jars.
        let list = self.content_list(inst, platform)?;
        let catalog_ok = list.online;
        let metas = merc_admin::modmeta::scan(&inst.root, "mods");
        let entries = list
            .items
            .into_iter()
            .map(|it| {
                let meta = metas.iter().find(|m| m.file == it.file_name).cloned();
                let cat = CatalogSides {
                    source: it.source.map(|s| {
                        match s {
                            ContentSource::Modrinth => "Modrinth",
                            ContentSource::Curseforge => "CurseForge",
                            ContentSource::Hangar => "Hangar",
                        }
                        .to_string()
                    }),
                    title: it.title.clone(),
                    client: it.client_side.map(side_str),
                    server: it.server_side.map(side_str),
                };
                (it.file_name, it.enabled, meta, cat)
            })
            .collect();
        Ok(ModSidesInfo {
            items: ms::report_with_deps(entries),
            quarantined: ms::quarantined(&inst.root),
            folder: ms::QUARANTINE_DIR.into(),
            catalog_ok,
        })
    }

    pub(crate) fn handle_mod_sides(
        self: &Arc<Self>,
        client: &str,
        body: RequestBody,
    ) -> Result<ResponseBody, ApiError> {
        match body {
            RequestBody::ModSides { id } => {
                let inst = self.instance(id)?;
                Ok(ResponseBody::ModSides(Box::new(self.mod_sides(&inst)?)))
            }
            RequestBody::QuarantineMods { id, files } => {
                let inst = self.instance(id)?;
                self.require_stopped(id, "mover mods a la cuarentena")?;
                // El motivo que queda anotado es la clasificación de ese momento.
                let info = self.mod_sides(&inst)?;
                let with_reason: Vec<(String, String)> = files
                    .iter()
                    .map(|f| {
                        let reason = info
                            .items
                            .iter()
                            .find(|i| &i.file == f)
                            .map(|i| match i.side {
                                ms::ModSide::ClientOnly => "Solo cliente",
                                ms::ModSide::ServerOnly => "Solo servidor",
                                ms::ModSide::Both => "Cliente y servidor",
                                ms::ModSide::Unknown => "Sin datos",
                            })
                            .unwrap_or("Sin datos");
                        (f.clone(), reason.to_string())
                    })
                    .collect();
                ms::move_to_quarantine(&inst.root, &with_reason, now_unix()).map_err(admin_err)?;
                self.audit(
                    client,
                    Some(id),
                    "mods.quarantine",
                    &format!("{} → {}/: {}", files.len(), ms::QUARANTINE_DIR, files.join(", ")),
                );
                Ok(ResponseBody::ModSides(Box::new(self.mod_sides(&inst)?)))
            }
            RequestBody::RestoreMods { id, files } => {
                let inst = self.instance(id)?;
                self.require_stopped(id, "devolver mods de la cuarentena")?;
                ms::restore(&inst.root, &files).map_err(admin_err)?;
                self.audit(client, Some(id), "mods.restore", &files.join(", "));
                Ok(ResponseBody::ModSides(Box::new(self.mod_sides(&inst)?)))
            }
            _ => Err(api(ErrorCode::BadRequest, "Petición no válida")),
        }
    }
}
