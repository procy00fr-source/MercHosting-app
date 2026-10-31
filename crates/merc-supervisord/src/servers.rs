//! Servidores: clonar (copia completa con otro puerto), archivar, versiones del loader y reglas del
//! juego elegidas al crear (se aplican la primera vez que el servidor queda en línea).

use super::{admin_err, api, install_err, now_unix, store_err, Daemon};
use merc_core::{Instance, InstanceId};
use merc_install::Step;
use merc_ipc::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

impl Daemon {
    fn archived_file(&self, id: InstanceId) -> PathBuf {
        self.store_dir(id).join("archivado")
    }

    pub(crate) fn is_archived(&self, id: InstanceId) -> bool {
        self.archived_file(id).is_file()
    }

    pub(crate) fn require_not_archived(&self, id: InstanceId) -> Result<(), ApiError> {
        if self.is_archived(id) {
            return Err(api(
                ErrorCode::InvalidState,
                "Este servidor está archivado. Desarchívalo para encenderlo.",
            ));
        }
        Ok(())
    }

    fn pending_rules_file(&self, id: InstanceId) -> PathBuf {
        self.store_dir(id).join("reglas-pendientes.json")
    }

    pub(crate) fn save_pending_rules(&self, id: InstanceId, rules: &BTreeMap<String, String>) {
        let f = self.pending_rules_file(id);
        if let Some(p) = f.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        if let Ok(t) = serde_json::to_string_pretty(rules) {
            let _ = std::fs::write(f, t);
        }
    }

    /// Aplica (una sola vez) las reglas elegidas al crear el servidor. Se llama cuando está en línea.
    pub(crate) fn apply_pending_rules(&self, inst: &Instance) {
        let f = self.pending_rules_file(inst.id);
        let Ok(text) = std::fs::read_to_string(&f) else { return };
        let _ = std::fs::remove_file(&f);
        let Ok(rules) = serde_json::from_str::<BTreeMap<String, String>>(&text) else {
            return;
        };
        for (k, v) in rules {
            match merc_admin::gamerules::set_command(&k, &v, inst.mc_version.as_deref()) {
                Ok(cmd) => match self.sup.send_command(inst.id, &cmd) {
                    Ok(()) => self.audit(
                        "supervisor",
                        Some(inst.id),
                        "world.gamerule",
                        &format!("{cmd} (elegida al crear)"),
                    ),
                    Err(e) => self.audit(
                        "supervisor",
                        Some(inst.id),
                        "world.gamerule.failed",
                        &format!("{cmd}: {e}"),
                    ),
                },
                Err(e) => self.audit(
                    "supervisor",
                    Some(inst.id),
                    "world.gamerule.failed",
                    &format!("{k}={v}: {e}"),
                ),
            }
        }
    }

    pub(crate) fn handle_servers(self: &Arc<Self>, client: &str, body: RequestBody) -> Result<ResponseBody, ApiError> {
        match body {
            RequestBody::LoaderVersions { flavor, mc_version } => Ok(ResponseBody::LoaderVersions(
                merc_install::loader_versions(&self.http, &self.sources, flavor, &mc_version).map_err(install_err)?,
            )),
            RequestBody::SetArchived { id, archived } => {
                self.instance(id)?;
                if archived {
                    if self.sup.status(id).state.has_process() {
                        return Err(api(ErrorCode::InvalidState, "Detén el servidor antes de archivarlo."));
                    }
                    std::fs::create_dir_all(self.store_dir(id)).map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
                    std::fs::write(self.archived_file(id), now_unix().to_string())
                        .map_err(|e| api(ErrorCode::Internal, e.to_string()))?;
                    self.audit(client, Some(id), "server.archive", "");
                } else {
                    let _ = std::fs::remove_file(self.archived_file(id));
                    self.audit(client, Some(id), "server.unarchive", "");
                }
                Ok(ResponseBody::Ok)
            }
            RequestBody::CloneServer { id, name } => {
                let inst = self.instance(id)?;
                let name =
                    merc_core::instance::validate_instance_name(&name).map_err(|e| api(ErrorCode::BadRequest, e))?;
                self.require_stopped(id, "clonarlo (así el mundo queda guardado del todo)")?;
                let dest = self.clone_target(&inst, &name);
                let job = self
                    .jobs
                    .create(format!("Clonar {}", inst.name), dest.clone())
                    .map_err(|e| api(ErrorCode::Conflict, e))?;
                let d = self.clone();
                let j = job.clone();
                let client = client.to_string();
                std::thread::spawn(move || match d.run_clone(&client, j.clone(), &inst, &name, &dest) {
                    Ok(new) => j.finish_with(JobState::Succeeded, None, Some(new.id), format!("«{}» listo", new.name)),
                    Err(e) => j.finish_with(
                        JobState::Failed,
                        Some(e),
                        None,
                        "No se clonó: no quedó nada a medias".into(),
                    ),
                });
                Ok(ResponseBody::Job(job.snapshot(0)))
            }
            _ => Err(api(ErrorCode::BadRequest, "Petición no válida")),
        }
    }

    /// Carpeta nueva junto a la original (o en la carpeta de servidores), sin pisar nada.
    fn clone_target(&self, inst: &Instance, name: &str) -> PathBuf {
        let parent = inst
            .root
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(super::default_servers_dir);
        let safe: String = name
            .chars()
            .map(|c| {
                if r#"<>:"/\|?*"#.contains(c) || c.is_control() {
                    '-'
                } else {
                    c
                }
            })
            .collect::<String>()
            .trim_end_matches(['.', ' '])
            .to_string();
        let base = if safe.is_empty() { "servidor".to_string() } else { safe };
        let mut dest = parent.join(&base);
        let mut n = 2;
        while dest.exists() {
            dest = parent.join(format!("{base} ({n})"));
            n += 1;
        }
        dest
    }

    fn run_clone(
        &self,
        client: &str,
        job: Arc<crate::jobs::Job>,
        inst: &Instance,
        name: &str,
        dest: &PathBuf,
    ) -> Result<Instance, String> {
        job.set_step(Step::Preparing, &format!("Copiando a {}", dest.display()));
        let progress = Arc::new(merc_admin::worlds::Progress::default());
        let (p2, j2) = (progress.clone(), job.clone());
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let d2 = done.clone();
        let reporter = std::thread::spawn(move || {
            while !d2.load(Ordering::Relaxed) {
                if j2.cancel.load(Ordering::Relaxed) {
                    p2.cancel.store(true, Ordering::Relaxed);
                }
                let total = p2.total.load(Ordering::Relaxed);
                j2.event(merc_install::InstallEvent::Download {
                    file: "copia".into(),
                    done: p2.done.load(Ordering::Relaxed),
                    total: (total > 0).then_some(total),
                });
                std::thread::sleep(Duration::from_millis(300));
            }
        });
        let copied = merc_admin::worlds::clone_server(&inst.root, dest, &progress);
        done.store(true, Ordering::Relaxed);
        let _ = reporter.join();
        copied.map_err(|e| admin_err(e).message)?;

        // Otro puerto para que puedan estar encendidos a la vez.
        job.set_step(Step::Configuring, "Eligiendo puertos libres");
        let mut used: Vec<u16> = self.others_ports(inst).iter().filter_map(|o| o.minecraft).collect();
        used.extend(Self::server_port(inst));
        let port = (25565u16..25700)
            .find(|p| !used.contains(p) && std::net::TcpListener::bind(("0.0.0.0", *p)).is_ok())
            .ok_or("No hay un puerto libre entre 25565 y 25700")?;
        let props = dest.join("server.properties");
        if let Ok(text) = std::fs::read_to_string(&props) {
            let mut ch = BTreeMap::new();
            ch.insert("server-port".to_string(), port.to_string());
            let out = merc_admin::props::apply_changes(&text, &ch).map_err(|e| admin_err(e).message)?;
            std::fs::write(&props, out).map_err(|e| e.to_string())?;
        }
        job.log(format!("Puerto de Minecraft: {port}"));
        // Simple Voice Chat con puerto propio: también otro.
        if let Some((cfg, text)) = merc_admin::voicechat::read_config(dest) {
            if cfg.port.is_some_and(|p| p > 0) {
                let mut voice_used: Vec<u16> = self.others_ports(inst).iter().filter_map(|o| o.voice).collect();
                voice_used.extend(Self::voice_port_of(inst, Self::server_port(inst)));
                if let Some(v) = (24454u16..24600).find(|p| !voice_used.contains(p) && *p != port) {
                    std::fs::write(dest.join(&cfg.rel), merc_admin::voicechat::apply(&text, None, Some(v)))
                        .map_err(|e| e.to_string())?;
                    job.log(format!("Puerto de la voz: {v}"));
                }
            }
        }

        let new = Instance {
            id: InstanceId::new(),
            name: name.to_string(),
            root: dest.clone(),
            kind: inst.kind,
            mc_version: inst.mc_version.clone(),
            loader_version: inst.loader_version.clone(),
            launch: inst.launch.clone(),
            imported: false,
            created_at_unix: now_unix(),
        };
        if let Err(e) = self
            .store
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert_instance(&new)
        {
            let _ = std::fs::remove_dir_all(dest);
            return Err(store_err(e).message);
        }
        self.audit(
            client,
            Some(inst.id),
            "server.clone",
            &format!("-> {} ({})", new.name, dest.display()),
        );
        self.audit(client, Some(new.id), "server.cloned_from", &inst.name);
        Ok(new)
    }
}
