//! Navegador de paquetes de recursos (Modrinth). Solo busca y muestra: el paquete se pone con
//! `SetResourcePack`, que lo descarga, comprueba y calcula su SHA-1.

use super::{api, content_err, Daemon};
use merc_ipc::*;
use std::sync::Arc;

impl Daemon {
    pub(crate) fn handle_rp_browse(self: &Arc<Self>, body: RequestBody) -> Result<ResponseBody, ApiError> {
        use merc_content::modrinth::{Modrinth, Source};
        let m = Modrinth::new(&self.http, &self.modrinth_api);
        match body {
            RequestBody::ResourcePackSearch {
                id,
                query,
                sort,
                offset,
            } => {
                let inst = self.instance(id)?;
                Ok(ResponseBody::ContentSearch(
                    m.search_resourcepacks(&query, inst.mc_version.as_deref(), sort, offset.min(10_000))
                        .map_err(content_err)?,
                ))
            }
            RequestBody::ResourcePackProject { id, project } => {
                let inst = self.instance(id)?;
                let mut proj = m.project(&project).map_err(content_err)?;
                let mut versions = m.all_versions(&proj.id).map_err(content_err)?;
                if proj.body.chars().count() > 20_000 {
                    proj.body = proj.body.chars().take(20_000).collect::<String>() + "…";
                }
                // Primero las que sirven para el Minecraft del servidor; se mantiene el orden de Modrinth.
                if let Some(mc) = inst.mc_version.as_deref() {
                    versions.sort_by_key(|v| !v.game_versions.iter().any(|g| g == mc));
                }
                versions.truncate(40);
                for v in &mut versions {
                    if let Some(c) = &mut v.changelog {
                        if c.chars().count() > 2000 {
                            *c = c.chars().take(2000).collect::<String>() + "…";
                        }
                    }
                }
                Ok(ResponseBody::ContentDetails {
                    project: Box::new(proj),
                    versions,
                })
            }
            _ => Err(api(ErrorCode::Internal, "petición inesperada")),
        }
    }
}
