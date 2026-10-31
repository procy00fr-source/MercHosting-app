//! Pack para amigos: `.mrpack` de cliente con enlaces oficiales de Modrinth (ver
//! `merc_content::friendpack`). Solo se escriben enlaces y huellas: ningún archivo de mod.

use super::{api, content_err, Daemon};
use merc_content::friendpack::{self as fp, FriendPackPlan, PackEntry};
use merc_content::modrinth::Source;
use merc_content::{ContentKind, ContentSource};
use merc_core::Instance;
use merc_ipc::*;
use std::sync::Arc;

impl Daemon {
    fn friend_pack(&self, inst: &Instance) -> Result<(FriendPackPlan, Vec<PackEntry>), ApiError> {
        let Some(platform) = self.platforms_of(inst).into_iter().find(|p| p.kind == ContentKind::Mod) else {
            let (mut plan, _) = fp::plan(inst.kind, inst.mc_version.as_deref(), None, &[], &Default::default());
            plan.loader = None;
            plan.blocked = Some(match &inst.mc_version {
                Some(v) => format!("Este servidor no usa mods: a tus amigos les basta con Minecraft {v}."),
                None => "Este servidor no usa mods: a tus amigos les basta con Minecraft.".into(),
            });
            return Ok((plan, vec![]));
        };
        let list = self.content_list(inst, platform)?;
        // Solo importa Modrinth (CurseForge o Hangar sin conexión no cambian qué va en el pack).
        if list.offline_reason.as_deref().is_some_and(|r| r.contains("Modrinth:")) {
            return Err(api(
                ErrorCode::Network,
                format!(
                    "Hace falta conexión con Modrinth para armar el pack. {}",
                    list.offline_reason.unwrap_or_default()
                ),
            ));
        }
        let hashes: Vec<String> = list
            .items
            .iter()
            .filter(|i| i.enabled && i.source == Some(ContentSource::Modrinth))
            .map(|i| i.sha1.clone())
            .collect();
        let source = merc_content::modrinth::Modrinth::new(&self.http, &self.modrinth_api);
        let versions = source.versions_by_sha1(&hashes).map_err(content_err)?;
        Ok(fp::plan(
            inst.kind,
            inst.mc_version.as_deref(),
            inst.loader_version.as_deref(),
            &list.items,
            &versions,
        ))
    }

    pub(crate) fn handle_friend_pack(
        self: &Arc<Self>,
        client: &str,
        body: RequestBody,
    ) -> Result<ResponseBody, ApiError> {
        match body {
            RequestBody::FriendPack { id } => {
                let inst = self.instance(id)?;
                Ok(ResponseBody::FriendPack(Box::new(self.friend_pack(&inst)?.0)))
            }
            RequestBody::ExportFriendPack { id, dest } => {
                let inst = self.instance(id)?;
                let ok_ext = dest
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("mrpack"));
                if !dest.is_absolute() || !ok_ext {
                    return Err(api(ErrorCode::BadRequest, "El pack se guarda como un archivo .mrpack."));
                }
                let (plan, entries) = self.friend_pack(&inst)?;
                let version = chrono::Local::now().format("%Y-%m-%d").to_string();
                let index = fp::index_json(&format!("{} (para jugar)", inst.name), &version, &plan, &entries)
                    .map_err(content_err)?;
                fp::write(&dest, &index).map_err(content_err)?;
                self.audit(
                    client,
                    Some(id),
                    "friendpack.export",
                    &format!("{} mods con enlace, {} a mano", plan.included.len(), plan.manual.len()),
                );
                Ok(ResponseBody::FileDone {
                    path: dest.display().to_string(),
                })
            }
            _ => Err(api(ErrorCode::BadRequest, "Petición no válida")),
        }
    }
}
