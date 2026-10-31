//! Icono del servidor: convierte cualquier imagen de la PC a `server-icon.png` (64×64).

use super::{admin_err, api, Daemon};
use base64::Engine;
use merc_admin::icon::{self, IconFit, ICON_FILE, MAX_SOURCE_BYTES};
use merc_core::Instance;
use merc_ipc::*;
use std::path::Path;
use std::sync::Arc;

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn read_source(source: &str) -> Result<Vec<u8>, ApiError> {
    let p = Path::new(source);
    if !p.is_absolute() {
        return Err(api(ErrorCode::BadRequest, "Elige una imagen de tu PC"));
    }
    let meta = std::fs::metadata(p).map_err(|_| api(ErrorCode::NotFound, "No se encontró la imagen"))?;
    if !meta.is_file() {
        return Err(api(ErrorCode::BadRequest, "Eso no es un archivo de imagen"));
    }
    if meta.len() > MAX_SOURCE_BYTES {
        return Err(api(
            ErrorCode::BadRequest,
            "La imagen pesa más de 25 MB. Usa una más liviana.",
        ));
    }
    std::fs::read(p).map_err(|e| api(ErrorCode::Internal, format!("No se pudo leer la imagen: {e}")))
}

impl Daemon {
    fn icon_info(&self, inst: &Instance) -> ServerIconInfo {
        let path = inst.root.join(ICON_FILE);
        let Ok(bytes) = std::fs::read(&path) else {
            return ServerIconInfo {
                present: false,
                valid: false,
                width: None,
                height: None,
                png_base64: None,
            };
        };
        let dims = icon::inspect(&bytes).ok();
        ServerIconInfo {
            present: true,
            valid: dims.is_some_and(|d| d.2),
            width: dims.map(|d| d.0),
            height: dims.map(|d| d.1),
            // Un icono válido pesa pocos KB; uno enorme no se manda entero.
            png_base64: (bytes.len() <= 512 * 1024 && dims.is_some()).then(|| b64(&bytes)),
        }
    }

    fn trash_icon(&self, inst: &Instance) -> Result<bool, ApiError> {
        if !inst.root.join(ICON_FILE).is_file() {
            return Ok(false);
        }
        self.explorer(inst)?.trash(ICON_FILE).map_err(admin_err)?;
        Ok(true)
    }

    pub(crate) fn handle_icon(self: &Arc<Self>, client: &str, body: RequestBody) -> Result<ResponseBody, ApiError> {
        match body {
            RequestBody::ServerIcon { id } => Ok(ResponseBody::ServerIcon(self.icon_info(&self.instance(id)?))),
            RequestBody::PreviewServerIcon { source, fit } => {
                let c = icon::convert(&read_source(&source)?, fit).map_err(admin_err)?;
                Ok(ResponseBody::IconPreview(IconPreview {
                    png_base64: b64(&c.png),
                    source_width: c.source_width,
                    source_height: c.source_height,
                    format: c.format.to_string(),
                }))
            }
            RequestBody::SetServerIcon { id, source, fit } => {
                let inst = self.instance(id)?;
                let c = icon::convert(&read_source(&source)?, fit).map_err(admin_err)?;
                // Se escribe aparte y luego se cambia: nunca queda un icono a medias.
                let tmp = inst.root.join(".server-icon.png.merc-tmp");
                std::fs::write(&tmp, &c.png)
                    .map_err(|e| api(ErrorCode::Internal, format!("No se pudo guardar el icono: {e}")))?;
                let old = match self.trash_icon(&inst) {
                    Ok(o) => o,
                    Err(e) => {
                        let _ = std::fs::remove_file(&tmp);
                        return Err(e);
                    }
                };
                std::fs::rename(&tmp, inst.root.join(ICON_FILE))
                    .map_err(|e| api(ErrorCode::Internal, format!("No se pudo guardar el icono: {e}")))?;
                let name = Path::new(&source)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let fit_label = if fit == IconFit::Crop { "recortado" } else { "completo" };
                self.audit(
                    client,
                    Some(id),
                    "icon.set",
                    &format!(
                        "{name} ({} {}×{}, {fit_label}){}",
                        c.format,
                        c.source_width,
                        c.source_height,
                        if old { "; el anterior está en la papelera" } else { "" }
                    ),
                );
                Ok(ResponseBody::ServerIcon(self.icon_info(&inst)))
            }
            RequestBody::RemoveServerIcon { id } => {
                let inst = self.instance(id)?;
                if self.trash_icon(&inst)? {
                    self.audit(client, Some(id), "icon.remove", "server-icon.png a la papelera");
                }
                Ok(ResponseBody::ServerIcon(self.icon_info(&inst)))
            }
            _ => Err(api(ErrorCode::BadRequest, "Petición no válida")),
        }
    }
}
