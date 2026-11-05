//! Addons de servidores Bedrock (ver `merc_admin::addons`).

use super::{admin_err, api, content_err, install_err, Daemon};
use merc_admin::addons as ad;
use merc_content::curseforge::{pick_addon_file, BedrockCatalog, CurseForge};
use merc_core::detect::ServerKind;
use merc_core::{Instance, InstanceId};
use merc_ipc::*;
use std::sync::Arc;

/// Addon de CurseForge ya descargado y verificado (en la caché de la app).
struct CfAddonFile {
    path: std::path::PathBuf,
    name: String,
}

impl Daemon {
    /// Juego y clase de los addons de Bedrock en CurseForge (se piden una vez por sesión).
    fn bedrock_catalog(&self, cf: &CurseForge) -> Result<BedrockCatalog, ApiError> {
        if let Some(c) = *self.cf_bedrock.lock().unwrap_or_else(|e| e.into_inner()) {
            return Ok(c);
        }
        let c = cf.bedrock_catalog().map_err(content_err)?;
        *self.cf_bedrock.lock().unwrap_or_else(|e| e.into_inner()) = Some(c);
        Ok(c)
    }

    /// Descarga (o toma de la caché) el archivo de un addon, comprobando su SHA-1.
    fn cf_addon_file(&self, cf: &CurseForge, project: &str, file_id: Option<&str>) -> Result<CfAddonFile, ApiError> {
        use merc_content::modrinth::Source;
        let v = match file_id {
            Some(f) => {
                let v = cf.version(f).map_err(content_err)?;
                if v.project_id != project {
                    return Err(api(ErrorCode::BadRequest, "Ese archivo no es de este addon"));
                }
                if !merc_content::curseforge::is_installable_addon(&v) {
                    return Err(api(
                        ErrorCode::Conflict,
                        "Ese archivo no se puede instalar desde aquí (no es un .mcaddon/.mcpack o el autor no permite descargarlo desde otras apps).",
                    ));
                }
                v
            }
            None => {
                let files = cf.all_files(project).map_err(content_err)?;
                if files.is_empty() {
                    return Err(api(
                        ErrorCode::NotFound,
                        "Este addon todavía no tiene archivos publicados",
                    ));
                }
                pick_addon_file(&files).cloned().ok_or_else(|| {
                    api(
                        ErrorCode::Conflict,
                        "El autor de este addon no permite descargarlo desde otras aplicaciones (o no publica un .mcaddon/.mcpack). Descárgalo desde su página de CurseForge y arrástralo a esta pestaña.",
                    )
                })?
            }
        };
        let f = v
            .files
            .first()
            .ok_or_else(|| api(ErrorCode::NotFound, "Ese archivo de CurseForge no existe"))?;
        if f.size > ad::MAX_ADDON_BYTES {
            return Err(api(ErrorCode::BadRequest, "El addon es demasiado grande"));
        }
        let sha1 = f
            .sha1
            .clone()
            .filter(|h| h.len() == 40 && h.chars().all(|c| c.is_ascii_hexdigit()))
            .ok_or_else(|| api(ErrorCode::Internal, "CurseForge no publica la huella de este addon"))?;
        let lower = f.filename.to_ascii_lowercase();
        let ext = merc_content::curseforge::ADDON_EXTENSIONS
            .iter()
            .find(|e| lower.ends_with(*e))
            .copied()
            .unwrap_or(".mcaddon");
        let dir = self.data_dir.join("cache").join("addons");
        std::fs::create_dir_all(&dir).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
        let path = dir.join(format!("{sha1}{ext}"));
        let ok = path.is_file()
            && merc_content::local::sha1_file(&path)
                .map(|h| h.eq_ignore_ascii_case(&sha1))
                .unwrap_or(false);
        if !ok {
            let _ = std::fs::remove_file(&path);
            let cancel = std::sync::atomic::AtomicBool::new(false);
            self.http
                .download(
                    &f.url,
                    &path,
                    &merc_install::Checksum::Sha1(sha1),
                    Some(f.size).filter(|s| *s > 0),
                    &mut |_, _| {},
                    &cancel,
                )
                .map_err(install_err)?;
        }
        Ok(CfAddonFile {
            path,
            name: f.filename.clone(),
        })
    }

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
            curseforge: self.curseforge().is_some(),
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
            R::AddonSearch {
                id,
                query,
                sort,
                offset,
            } => {
                self.bedrock_instance(id)?;
                let cf = self.require_curseforge()?;
                let cat = self.bedrock_catalog(&cf)?;
                let page = cf
                    .search_bedrock_addons(cat, &query, sort, offset.min(10_000))
                    .map_err(content_err)?;
                Ok(ResponseBody::ContentSearch(page))
            }
            R::AddonInstallCurseforge {
                id,
                project_id,
                file_id,
            } => {
                let inst = self.bedrock_instance(id)?;
                self.require_stopped(id, "instalar addons")?;
                let cf = self.require_curseforge()?;
                let cat = self.bedrock_catalog(&cf)?;
                if !cf.is_bedrock_addon(cat, &project_id).map_err(content_err)? {
                    return Err(api(ErrorCode::BadRequest, "Eso no es un addon de Minecraft Bedrock"));
                }
                let file = self.cf_addon_file(&cf, &project_id, file_id.as_deref())?;
                let explorer = self.explorer(&inst)?;
                let r = ad::import(&inst.root, &file.path, &mut |rel| explorer.trash(rel).map(|_| ()))
                    .map_err(admin_err)?;
                self.audit(
                    client,
                    Some(id),
                    "addons.curseforge",
                    &format!(
                        "CurseForge {project_id} ({}): {} instalados, {} reemplazados, {} sin cambios",
                        file.name,
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
