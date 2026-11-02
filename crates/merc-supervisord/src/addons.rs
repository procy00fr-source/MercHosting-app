//! Addons de servidores Bedrock (ver `merc_admin::addons`).

use super::{admin_err, api, Daemon};
use merc_admin::addons as ad;
use merc_core::detect::ServerKind;
use merc_core::{Instance, InstanceId};
use merc_ipc::*;
use std::sync::Arc;

impl Daemon {
    pub(crate) fn bedrock_instance(&self, id: InstanceId) -> Result<Instance, ApiError> {
        let inst = self.instance(id)?;
        if inst.kind != ServerKind::Bedrock {
            return Err(api(
                ErrorCode::BadRequest,
                "Los addons son para servidores Bedrock. En Java se usan mods, plugins o datapacks.",
            ));
        }
        Ok(inst)
    }

    fn addons_info(&self, inst: &Instance, last_import: Option<ad::ImportResult>) -> AddonsInfo {
        let level_name = ad::level_name(&inst.root);
        AddonsInfo {
            world_exists: inst.root.join("worlds").join(&level_name).is_dir(),
            level_name,
            packs: ad::list(&inst.root),
            last_import,
        }
    }

    pub(crate) fn handle_addons(self: &Arc<Self>, client: &str, body: RequestBody) -> Result<ResponseBody, ApiError> {
        use RequestBody as R;
        match body {
            R::Addons { id } => {
                let inst = self.bedrock_instance(id)?;
                Ok(ResponseBody::Addons(Box::new(self.addons_info(&inst, None))))
            }
            R::AddonImport { id, source } => {
                let inst = self.bedrock_instance(id)?;
                self.require_stopped(id, "instalar addons")?;
                if !source.is_absolute() || !source.is_file() {
                    return Err(api(
                        ErrorCode::BadRequest,
                        "Elige un archivo .mcpack o .mcaddon de tu PC",
                    ));
                }
                let explorer = self.explorer(&inst)?;
                let r =
                    ad::import(&inst.root, &source, &mut |rel| explorer.trash(rel).map(|_| ())).map_err(admin_err)?;
                let file = source
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.audit(
                    client,
                    Some(id),
                    "addons.import",
                    &format!(
                        "{file}: {} instalados, {} reemplazados, {} sin cambios",
                        r.installed.len(),
                        r.replaced.len(),
                        r.unchanged.len()
                    ),
                );
                Ok(ResponseBody::Addons(Box::new(self.addons_info(&inst, Some(r)))))
            }
            R::AddonSetActive { id, uuid, active } => {
                let inst = self.bedrock_instance(id)?;
                self.require_stopped(id, "activar o desactivar addons")?;
                ad::set_active(&inst.root, &uuid, active).map_err(admin_err)?;
                self.audit(
                    client,
                    Some(id),
                    "addons.toggle",
                    &format!("{uuid}: {}", if active { "activado" } else { "desactivado" }),
                );
                Ok(ResponseBody::Addons(Box::new(self.addons_info(&inst, None))))
            }
            R::AddonRemove { id, uuid } => {
                let inst = self.bedrock_instance(id)?;
                self.require_stopped(id, "quitar addons")?;
                let pack = ad::list(&inst.root)
                    .into_iter()
                    .find(|p| p.uuid.eq_ignore_ascii_case(&uuid))
                    .ok_or_else(|| api(ErrorCode::NotFound, "Ese addon ya no está instalado"))?;
                if pack.active {
                    ad::set_active(&inst.root, &pack.uuid, false).map_err(admin_err)?;
                }
                self.explorer(&inst)?.trash(&pack.folder).map_err(admin_err)?;
                self.audit(
                    client,
                    Some(id),
                    "addons.remove",
                    &format!("{} → papelera ({})", pack.name, pack.folder),
                );
                Ok(ResponseBody::Addons(Box::new(self.addons_info(&inst, None))))
            }
            _ => Err(api(ErrorCode::BadRequest, "Petición no válida")),
        }
    }
}
