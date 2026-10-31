//! Actualizar el modpack de un servidor (ver `merc_content::packupdate`).

use super::{admin_err, api, content_err, now_unix, Daemon, LoadedPack};
use merc_admin::backups::{BackupReason, BackupScope};
use merc_content::packupdate::{self as pu, ChangeKind, OwnedFile, PackManifest};
use merc_content::ContentSource;
use merc_core::detect::ServerKind;
use merc_core::{Instance, InstanceId};
use merc_install::Step;
use merc_ipc::*;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Loader que corresponde a cada tipo de servidor (como lo escribe el modpack).
fn loader_of(kind: ServerKind) -> Option<&'static str> {
    match kind {
        ServerKind::Forge => Some("forge"),
        ServerKind::NeoForge => Some("neoforge"),
        ServerKind::Fabric => Some("fabric"),
        ServerKind::Quilt => Some("quilt"),
        _ => None,
    }
}

fn label(v: &str) -> String {
    let clean: String = v
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .take(40)
        .collect();
    format!("modpack-anterior-{}", clean.trim_matches(['.', '-']))
}

impl Daemon {
    fn pack_versions(
        &self,
        source: ContentSource,
        project: &str,
    ) -> Result<Vec<merc_content::modrinth::ContentVersion>, ApiError> {
        match source {
            ContentSource::Modrinth => merc_content::modrinth::Modrinth::new(&self.http, &self.modrinth_api)
                .all_versions(project)
                .map_err(content_err),
            ContentSource::Curseforge => self.require_curseforge()?.all_files(project).map_err(content_err),
            ContentSource::Hangar => Err(api(ErrorCode::BadRequest, "Hangar no tiene modpacks")),
        }
    }

    /// Comprueba que la versión del modpack es para este servidor (Minecraft y loader).
    fn check_pack_fits(inst: &Instance, pack: &LoadedPack) -> Result<(), ApiError> {
        let mc = inst.mc_version.as_deref().unwrap_or("");
        if pack.index.mc_version != mc {
            return Err(api(
                ErrorCode::BadRequest,
                format!(
                    "Esa versión del modpack es para Minecraft {} y el servidor es {}. Cambiar de versión de Minecraft puede romper el mundo: crea un servidor nuevo para ella.",
                    pack.index.mc_version,
                    if mc.is_empty() { "desconocido" } else { mc }
                ),
            ));
        }
        if pack.index.loader.as_deref() != loader_of(inst.kind) {
            return Err(api(
                ErrorCode::BadRequest,
                format!(
                    "Esa versión del modpack usa {} y el servidor es otro tipo.",
                    pack.index.loader.as_deref().unwrap_or("Minecraft sin loader")
                ),
            ));
        }
        Ok(())
    }

    fn manifest_for(
        &self,
        source: ContentSource,
        version_id: &str,
        pack: &LoadedPack,
    ) -> Result<PackManifest, ApiError> {
        let prefixes: Vec<&str> = pack.overrides.iter().map(String::as_str).collect();
        Ok(PackManifest {
            source,
            project_id: pack.project_id.clone(),
            title: pack.index.name.clone(),
            version_id: version_id.to_string(),
            version_number: pack.version_number.clone(),
            mc_version: pack.index.mc_version.clone(),
            loader: pack.index.loader.clone(),
            loader_version: pack.index.loader_version.clone(),
            installed_at_unix: now_unix(),
            files: pu::desired(&pack.index, &pack.path, &prefixes).map_err(content_err)?,
        })
    }

    /// Al crear un servidor desde un modpack: se guarda qué puso (para poder actualizarlo).
    pub(crate) fn record_modpack(
        &self,
        id: InstanceId,
        source: ContentSource,
        version_id: &str,
        pack: &LoadedPack,
    ) -> Result<(), String> {
        let m = self.manifest_for(source, version_id, pack).map_err(|e| e.message)?;
        pu::save(&self.store_dir(id), &m).map_err(|e| e.to_string())
    }

    fn modpack_status(&self, inst: &Instance) -> ModpackStatusInfo {
        let Some(m) = pu::load(&self.store_dir(inst.id)) else {
            return ModpackStatusInfo {
                linked: None,
                online: true,
                error: None,
                versions: vec![],
            };
        };
        let (versions, error) = match self.pack_versions(m.source, &m.project_id) {
            Ok(v) => (v, None),
            Err(e) => (vec![], Some(e.message)),
        };
        let current_date = versions
            .iter()
            .find(|v| v.id == m.version_id)
            .map(|v| v.date_published.clone());
        let loader = m.loader.clone().unwrap_or_default();
        let versions = versions
            .into_iter()
            .take(60)
            .map(|v| ModpackVersionChoice {
                compatible: v.game_versions.contains(&m.mc_version)
                    && (v.loaders.is_empty()
                        || loader.is_empty()
                        || v.loaders.iter().any(|l| l.eq_ignore_ascii_case(&loader))),
                current: v.id == m.version_id,
                newer: current_date.as_ref().is_some_and(|d| v.date_published > *d),
                version_id: v.id,
                version_number: v.version_number,
                name: v.name,
                date_published: v.date_published,
                game_versions: v.game_versions,
            })
            .collect();
        ModpackStatusInfo {
            linked: Some(m.summary()),
            online: error.is_none(),
            error,
            versions,
        }
    }

    fn update_plan(
        &self,
        inst: &Instance,
        version_id: &str,
    ) -> Result<(ModpackUpdatePlan, PackManifest, PackManifest, LoadedPack), ApiError> {
        let old = pu::load(&self.store_dir(inst.id)).ok_or_else(|| {
            api(
                ErrorCode::NotFound,
                "MercHosting no sabe qué modpack tiene este servidor. Vincúlalo primero.",
            )
        })?;
        if version_id == old.version_id {
            return Err(api(ErrorCode::BadRequest, "Ya tienes esa versión del modpack."));
        }
        let pack = self.load_modpack(old.source, version_id, &AtomicBool::new(false))?;
        Self::check_pack_fits(inst, &pack)?;
        if pack.project_id != old.project_id {
            return Err(api(ErrorCode::BadRequest, "Esa versión es de otro modpack."));
        }
        let new = self.manifest_for(old.source, version_id, &pack)?;
        let changes = pu::plan(&inst.root, &old.files, &new.files);
        let download_bytes = pack
            .index
            .files
            .iter()
            .filter(|f| {
                changes
                    .iter()
                    .any(|c| c.path == f.path && matches!(c.kind, ChangeKind::Add | ChangeKind::Replace))
            })
            .map(|f| f.size)
            .sum();
        let current_loader = inst.loader_version.clone().or(old.loader_version.clone());
        let loader_mismatch = new.loader_version.is_some() && new.loader_version != current_loader;
        let mut notes = pack.warnings.clone();
        if loader_mismatch {
            notes.push(format!(
                "La versión nueva está hecha para {} {} y el servidor tiene {}. MercHosting no cambia el loader en un servidor existente: si algún mod pide la versión nueva, el servidor no arrancará y tendrás que restaurar la copia previa.",
                new.loader.as_deref().unwrap_or("el loader"),
                new.loader_version.as_deref().unwrap_or("?"),
                current_loader.as_deref().unwrap_or("otra versión")
            ));
        }
        Ok((
            ModpackUpdatePlan {
                from_version: old.version_number.clone(),
                to_version: new.version_number.clone(),
                to_version_id: version_id.to_string(),
                mc_version: new.mc_version.clone(),
                loader: new.loader.clone(),
                loader_from: current_loader,
                loader_to: new.loader_version.clone(),
                loader_mismatch,
                changes,
                download_bytes,
                notes,
            },
            old,
            new,
            pack,
        ))
    }

    pub(crate) fn handle_modpacks(self: &Arc<Self>, client: &str, body: RequestBody) -> Result<ResponseBody, ApiError> {
        match body {
            RequestBody::ModpackStatus { id } => {
                let inst = self.instance(id)?;
                Ok(ResponseBody::ModpackStatus(Box::new(self.modpack_status(&inst))))
            }
            RequestBody::ModpackUpdatePreview { id, version_id } => {
                let inst = self.instance(id)?;
                let (plan, ..) = self.update_plan(&inst, &version_id)?;
                Ok(ResponseBody::ModpackUpdatePlan(Box::new(plan)))
            }
            RequestBody::ModpackLink { id, source, version_id } => {
                let inst = self.instance(id)?;
                let pack = self.load_modpack(source, &version_id, &AtomicBool::new(false))?;
                Self::check_pack_fits(&inst, &pack)?;
                let m = self.manifest_for(source, &version_id, &pack)?;
                pu::save(&self.store_dir(id), &m).map_err(content_err)?;
                self.audit(
                    client,
                    Some(id),
                    "modpack.link",
                    &format!("{} {}", m.title, m.version_number),
                );
                Ok(ResponseBody::ModpackStatus(Box::new(self.modpack_status(&inst))))
            }
            RequestBody::ModpackUnlink { id } => {
                let inst = self.instance(id)?;
                let _ = std::fs::remove_file(self.store_dir(id).join(pu::MANIFEST_FILE));
                self.audit(client, Some(id), "modpack.unlink", "");
                Ok(ResponseBody::ModpackStatus(Box::new(self.modpack_status(&inst))))
            }
            RequestBody::ModpackUpdateApply {
                id,
                version_id,
                accept_loader_change,
            } => {
                let inst = self.instance(id)?;
                self.require_stopped(id, "actualizar el modpack")?;
                let (plan, old, new, pack) = self.update_plan(&inst, &version_id)?;
                if plan.loader_mismatch && !accept_loader_change {
                    return Err(api(
                        ErrorCode::Conflict,
                        "La versión nueva pide otra versión del loader. Confírmalo para seguir.",
                    ));
                }
                let job = self
                    .jobs
                    .create(format!("Actualizar modpack de {}", inst.name), inst.root.clone())
                    .map_err(|e| api(ErrorCode::Conflict, e))?;
                let d = self.clone();
                let j = job.clone();
                let client = client.to_string();
                std::thread::spawn(move || {
                    let r = d.apply_modpack_update(&client, &j, &inst, &plan, &old, &new, &pack);
                    match r {
                        Ok(msg) => j.finish_with(JobState::Succeeded, None, Some(inst.id), msg),
                        Err(e) => j.finish_with(
                            JobState::Failed,
                            Some(e),
                            Some(inst.id),
                            "No se completó la actualización".into(),
                        ),
                    }
                });
                Ok(ResponseBody::Job(job.snapshot(0)))
            }
            _ => Err(api(ErrorCode::BadRequest, "Petición no válida")),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_modpack_update(
        &self,
        client: &str,
        job: &crate::jobs::Job,
        inst: &Instance,
        plan: &ModpackUpdatePlan,
        old: &PackManifest,
        new: &PackManifest,
        pack: &LoadedPack,
    ) -> Result<String, String> {
        let take = |k: &[ChangeKind]| -> Vec<&merc_content::packupdate::PlannedChange> {
            plan.changes.iter().filter(|c| k.contains(&c.kind)).collect()
        };
        let put = take(&[ChangeKind::Add, ChangeKind::Replace]);
        let out = take(&[ChangeKind::Replace, ChangeKind::Remove]);
        // 1. Copia completa y verificada antes de tocar nada.
        job.set_step(Step::Preparing, "Haciendo una copia completa antes de actualizar");
        let backup = self
            .make_backup(
                client,
                inst,
                BackupScope::Full,
                BackupReason::BeforeUpdate,
                Some(format!("Antes de actualizar el modpack a {}", plan.to_version)),
            )
            .map_err(|e| {
                format!(
                    "No se pudo hacer la copia previa, así que no se cambió nada: {}",
                    e.message
                )
            })?;
        job.log(format!("Copia previa: {} (verificada).", backup.id));
        // 2. Descargar y preparar fuera del servidor.
        let staging = self
            .data_dir
            .join("cache")
            .join("modpack-staging")
            .join(uuid::Uuid::new_v4().to_string());
        let cleanup = || {
            let _ = std::fs::remove_dir_all(&staging);
        };
        let files = staging.join("archivos");
        let extras = staging.join("extras");
        let empty = staging.join("vacio");
        for d in [&files, &extras, &empty] {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let mut wanted = pack.index.clone();
        wanted
            .files
            .retain(|f| put.iter().any(|c| c.path == f.path && !c.from_override));
        job.set_step(
            Step::Downloading,
            &format!("Descargando {} archivos", wanted.files.len()),
        );
        if let Err(e) = merc_content::mrpack::stage_files(
            &self.http,
            &wanted,
            &files,
            &mut |file, done, total| {
                job.event(merc_install::InstallEvent::Download {
                    file: file.to_string(),
                    done,
                    total: Some(total),
                })
            },
            &job.cancel,
        ) {
            cleanup();
            return Err(e.to_string());
        }
        let prefixes: Vec<&str> = pack.overrides.iter().map(String::as_str).collect();
        if put.iter().any(|c| c.from_override) {
            if let Err(e) = merc_content::mrpack::apply_with(&pack.path, &empty, &extras, &prefixes) {
                cleanup();
                return Err(e.to_string());
            }
        }
        // Todo lo que se va a poner, comprobado contra su huella.
        let mut sources: BTreeMap<&str, std::path::PathBuf> = BTreeMap::new();
        for c in &put {
            let src = if c.from_override {
                extras.join(&c.path)
            } else {
                files.join(&c.path)
            };
            let expect: Option<&OwnedFile> = new.files.get(&c.path);
            let ok = expect
                .is_some_and(|f| merc_content::local::sha1_file(&src).is_ok_and(|h| h.eq_ignore_ascii_case(&f.sha1)));
            if !ok {
                cleanup();
                return Err(format!(
                    "{} no coincide con lo que publica el modpack; no se cambió nada.",
                    c.path
                ));
            }
            sources.insert(c.path.as_str(), src);
        }
        if job.cancel.load(std::sync::atomic::Ordering::SeqCst) {
            cleanup();
            return Err("Cancelado: no se cambió nada.".into());
        }
        // 3. Lo viejo a la papelera (todo junto) y lo nuevo a su sitio.
        job.set_step(Step::Configuring, "Cambiando los archivos del modpack");
        let ex = self.explorer(inst).map_err(|e| e.message)?;
        let paths: Vec<String> = out.iter().map(|c| c.path.clone()).collect();
        if !paths.is_empty() {
            ex.trash_group(&label(&old.version_number), &paths)
                .map_err(admin_err)
                .map_err(|e| {
                    cleanup();
                    format!(
                        "No se pudo apartar la versión anterior ({}); no se cambió nada.",
                        e.message
                    )
                })?;
            job.log(format!(
                "{} archivos de la versión {} a la papelera de MercHosting",
                paths.len(),
                old.version_number
            ));
        }
        for (rel, src) in &sources {
            let dst = inst.root.join(rel);
            let r = (|| {
                if let Some(p) = dst.parent() {
                    std::fs::create_dir_all(p)?;
                }
                if std::fs::rename(src, &dst).is_err() {
                    std::fs::copy(src, &dst)?;
                }
                Ok::<(), std::io::Error>(())
            })();
            if let Err(e) = r {
                cleanup();
                return Err(format!(
                    "Falló al copiar {rel}: {e}. Restaura la copia previa {} en Copias de seguridad.",
                    backup.id
                ));
            }
        }
        cleanup();
        // 4. Registro nuevo.
        let mut m = new.clone();
        m.installed_at_unix = now_unix();
        pu::save(&self.store_dir(inst.id), &m).map_err(|e| e.to_string())?;
        let kept = plan
            .changes
            .iter()
            .filter(|c| {
                matches!(
                    c.kind,
                    ChangeKind::KeepModified | ChangeKind::KeepUserFile | ChangeKind::SkipRemoved
                )
            })
            .count();
        self.audit(
            client,
            Some(inst.id),
            "modpack.update",
            &format!(
                "{} {} -> {} (copia previa {})",
                old.title, old.version_number, new.version_number, backup.id
            ),
        );
        Ok(format!(
            "{} actualizado a {}: {} archivos nuevos o cambiados, {} quitados, {} respetados por tus cambios. Copia previa: {}",
            old.title,
            new.version_number,
            put.len(),
            take(&[ChangeKind::Remove]).len(),
            kept,
            backup.id
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trash_label_is_a_safe_folder_name() {
        assert_eq!(label("1.2.0"), "modpack-anterior-1.2.0");
        assert_eq!(label("v2 (beta)/x"), "modpack-anterior-v2--beta--x");
    }
}
