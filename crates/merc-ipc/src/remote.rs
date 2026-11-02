//! Compañeros: permisos por servidor y qué permiso exige cada petición que llega desde la PC
//! de un compañero. Todo lo que no aparece aquí explícitamente se **deniega**.

use crate::protocol::RequestBody;
use merc_core::InstanceId;
use serde::{Deserialize, Serialize};

/// Permisos separados que el propietario concede por servidor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Perm {
    /// Ver si está encendido, rendimiento, salud y tareas.
    Status,
    Start,
    Stop,
    /// Reiniciar (también con aviso) y cancelar un reinicio.
    Restart,
    ConsoleRead,
    /// Enviar comandos de Minecraft (y programar tareas, que pueden enviarlos). ALTO RIESGO.
    ConsoleCommand,
    /// Historial de actividad del servidor.
    Logs,
    /// Ver jugadores, expulsar, banear y perdonar.
    Players,
    Whitelist,
    /// Dar y quitar OP. ALTO RIESGO.
    Op,
    FilesList,
    /// Leer y descargar archivos.
    FilesRead,
    /// Editar, crear, renombrar, subir y restaurar archivos. ALTO RIESGO.
    FilesEdit,
    /// Mandar archivos a la papelera y vaciarla. ALTO RIESGO.
    FilesDelete,
    /// Buscar, instalar, activar y quitar mods y plugins (archivos JAR). ALTO RIESGO.
    Content,
    /// Ver y crear copias de seguridad.
    Backups,
    /// Restaurar copias. ALTO RIESGO.
    BackupsRestore,
    BackupsDelete,
    /// Mundos: duplicar, activar y mandar a la papelera.
    World,
    /// Ajustes del servidor, reglas del juego y mantenimiento.
    Settings,
    /// Conexión: direcciones, túneles y chat de voz.
    Network,
}

impl Perm {
    pub const ALL: [Perm; 21] = [
        Perm::Status,
        Perm::Start,
        Perm::Stop,
        Perm::Restart,
        Perm::ConsoleRead,
        Perm::ConsoleCommand,
        Perm::Logs,
        Perm::Players,
        Perm::Whitelist,
        Perm::Op,
        Perm::FilesList,
        Perm::FilesRead,
        Perm::FilesEdit,
        Perm::FilesDelete,
        Perm::Content,
        Perm::Backups,
        Perm::BackupsRestore,
        Perm::BackupsDelete,
        Perm::World,
        Perm::Settings,
        Perm::Network,
    ];

    /// Permisos de alto riesgo: nunca se agrupan con la lectura y piden confirmación reforzada.
    pub fn high_risk(self) -> bool {
        matches!(
            self,
            Perm::ConsoleCommand
                | Perm::Op
                | Perm::FilesEdit
                | Perm::FilesDelete
                | Perm::Content
                | Perm::BackupsRestore
        )
    }

    /// Dan acceso a archivos reales de la PC (muestran la advertencia obligatoria).
    pub fn touches_files(self) -> bool {
        matches!(
            self,
            Perm::FilesRead | Perm::FilesEdit | Perm::FilesDelete | Perm::Content | Perm::ConsoleCommand
        )
    }
}

/// Roles predefinidos (atajos: el propietario puede ajustar cada permiso).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Observador,
    Moderador,
    Operador,
    Administrador,
}

impl Role {
    pub fn perms(self) -> Vec<Perm> {
        use Perm::*;
        let mut p = vec![Status, ConsoleRead];
        if matches!(self, Role::Moderador | Role::Operador | Role::Administrador) {
            p.extend([Logs, Players, Whitelist]);
        }
        if matches!(self, Role::Operador | Role::Administrador) {
            p.extend([Start, Stop, Restart, Backups, World, Network]);
        }
        if self == Role::Administrador {
            p.extend([
                ConsoleCommand,
                Op,
                FilesList,
                FilesRead,
                FilesEdit,
                FilesDelete,
                Content,
                BackupsRestore,
                BackupsDelete,
                Settings,
            ]);
        }
        p.sort();
        p
    }
}

/// Lo que exige una petición remota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Need {
    /// Cualquier compañero con algún servidor.
    Any,
    /// La lista de servidores (se filtra a los concedidos).
    ListInstances,
    /// Un permiso concreto en un servidor concreto.
    On(InstanceId, Perm),
    /// Seguimiento de una tarea en curso que inició el mismo compañero.
    OwnJob(String),
    /// Algo de la PC entera (crear servidores, instalar Java, aceptar la EULA): solo un
    /// compañero con «control total» (el dueño administrando su servidor casero).
    Host,
    /// No se permite desde otra PC.
    Deny,
}

/// Permiso que exige cada petición. Lo que no está listado se deniega.
pub fn need(body: &RequestBody) -> Need {
    use crate::protocol::RequestBody as R;
    use merc_admin::players::PlayerAction as P;
    use Need::On;
    use Perm::*;
    match body {
        R::Ping => Need::Any,
        R::ListInstances => Need::ListInstances,
        R::Status { id } | R::Metrics { id, .. } | R::AdminState { id } | R::Health { id } | R::Tasks { id } => {
            On(*id, Status)
        }
        R::MeasureTps { id } => On(*id, Status),
        R::ContentInstalled { id, .. } => On(*id, Status),
        R::Start { id } => On(*id, Start),
        R::Stop { id, .. } => On(*id, Stop),
        R::Restart { id, .. } | R::RestartWithNotice { id, .. } | R::CancelRestart { id } => On(*id, Restart),
        R::Console { id, .. } => On(*id, ConsoleRead),
        R::SendCommand { id, .. } => On(*id, ConsoleCommand),
        R::SaveTask { id, .. } | R::DeleteTask { id, .. } | R::RunTaskNow { id, .. } => On(*id, ConsoleCommand),
        R::Activity { id, .. } => On(*id, Logs),
        R::Players { id } => On(*id, Players),
        R::PlayerAction { id, action, .. } => match action {
            P::WhitelistAdd | P::WhitelistRemove => On(*id, Whitelist),
            P::Op | P::Deop => On(*id, Op),
            P::Kick | P::Ban | P::Pardon => On(*id, Players),
        },
        R::ListFiles { id, .. } | R::SavedItems { id } => On(*id, FilesList),
        R::ReadFile { id, .. } => On(*id, FilesRead),
        R::WriteFile { id, .. }
        | R::MakeDir { id, .. }
        | R::RenameFile { id, .. }
        | R::MoveFile { id, .. }
        | R::CopyFile { id, .. }
        | R::ExtractZip { id, .. }
        | R::RestoreSaved { id, .. } => On(*id, FilesEdit),
        R::TrashFile { id, .. } | R::PurgeSaved { id, .. } => On(*id, FilesDelete),
        R::ContentPlatforms { id, .. }
        | R::ContentSearch { id, .. }
        | R::ContentProject { id, .. }
        | R::ContentPlan { id, .. }
        | R::ContentInstall { id, .. }
        | R::ContentToggle { id, .. }
        | R::ContentRemove { id, .. } => On(*id, Content),
        R::Backups { id } | R::CreateBackup { id, .. } | R::VerifyBackup { id, .. } => On(*id, Backups),
        R::RestoreBackup { id, .. } => On(*id, BackupsRestore),
        R::DeleteBackup { id, .. } | R::ProtectBackup { id, .. } => On(*id, BackupsDelete),
        R::Worlds { id }
        | R::DuplicateWorld { id, .. }
        | R::SetActiveWorld { id, .. }
        | R::TrashWorld { id, .. }
        | R::CancelWorldOp { id } => On(*id, World),
        R::Properties { id }
        | R::SetProperties { id, .. }
        | R::SetResourcePack { id, .. }
        | R::ResourcePackSearch { id, .. }
        | R::ResourcePackProject { id, .. }
        | R::GameRules { id }
        | R::SetGameRule { id, .. }
        | R::SetMaintenance { id, .. }
        | R::SetBackupSettings { id, .. } => On(*id, Settings),
        R::Connection { id }
        | R::TestAddress { id, .. }
        | R::TestBedrockAddress { id, .. }
        | R::SetVoiceConfig { id, .. } => On(*id, Network),
        R::Diagnose { id } => On(*id, Logs),
        R::ServerUpdateCheck { id } => On(*id, Status),
        R::ServerIcon { id } => On(*id, Status),
        R::Automation { id } => On(*id, Status),
        R::PlayerCommand { id, .. } | R::PlayerHistory { id } | R::SetPlayerNote { id, .. } => On(*id, Players),
        // Elegir un Java es elegir un programa de la PC del dueño: solo él.
        R::SetLaunch { java: Some(_), .. } => Need::Deny,
        // Los permisos de LuckPerms deciden quién es admin dentro del juego: solo el dueño.
        R::LuckPerms { .. } | R::LuckPermsPreview { .. } | R::LuckPermsApply { .. } => Need::Deny,
        R::FriendPack { id } => On(*id, Content),
        R::ModpackStatus { id }
        | R::ModpackUpdatePreview { id, .. }
        | R::ModpackUpdateApply { id, .. }
        | R::ModpackLink { id, .. }
        | R::ModpackUnlink { id } => On(*id, Content),
        R::ModSides { id } | R::QuarantineMods { id, .. } | R::RestoreMods { id, .. } => On(*id, Content),
        R::Addons { id } => On(*id, Status),
        R::BedrockPlayers { id } => On(*id, Players),
        R::BedrockAllowlist { id, .. } | R::BedrockAllowlistEnabled { id, .. } => On(*id, Whitelist),
        R::BedrockPermission { id, .. } => On(*id, Op),
        R::AddonSetActive { id, .. } | R::AddonRemove { id, .. } => On(*id, Content),
        // Lee un archivo de la PC del dueño: no se permite desde otra PC.
        R::AddonImport { .. } => Need::Deny,
        // Escribe un archivo en la PC del dueño.
        R::ExportFriendPack { .. } => Need::Deny,
        R::SetLaunch { id, .. } => On(*id, Settings),
        R::Job { job_id, .. } | R::CancelJob { job_id } => Need::OwnJob(job_id.clone()),
        // Servidor casero: crear servidores desde otra PC. El anfitrión elige la carpeta (dentro
        // de su carpeta de servidores) y el Java (entre los suyos): nunca los decide la otra PC.
        R::FindJava
        | R::Catalog { .. }
        | R::Resolve { .. }
        | R::LoaderVersions { .. }
        | R::CreationDefaults
        | R::ModpackSearch { .. }
        | R::ModpackProject { .. }
        | R::ModpackInspect { .. }
        | R::CreateServer { .. }
        | R::InstallJava { .. } => Need::Host,
        // La EULA la acepta la persona en su pantalla (queda en el historial quién y cuándo).
        R::AcceptEula { .. } => Need::Host,
        // Playit en el servidor casero, solo con control total: activar el agente oficial
        // (descarga verificada), vincularlo a la cuenta de playit.gg de la persona (el enlace se
        // abre en el navegador de su celular o su otra PC), verlo y apagarlo. Sirve para que sus
        // amigos JUEGUEN; la administración sigue yendo siempre por Compañeros, nunca por Playit.
        // Un compañero normal (permisos de un servidor) no puede tocarlo.
        R::PlayitStatus | R::PlayitEnable | R::PlayitDisable | R::PlayitLink | R::PlayitUnlink | R::PlayitLoginUrl => {
            Need::Host
        }
        // Rutas de esta PC, EULA, crear o borrar servidores, Playit, compañeros, apagar el
        // supervisor, descargas y subidas por ruta local…: solo el propietario.
        _ => Need::Deny,
    }
}

/// Algo que un compañero se lleva a su PC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RemoteItem {
    /// Un archivo del servidor (ruta relativa a su carpeta).
    File { id: InstanceId, path: String },
    /// Un mundo exportado a .zip (con el servidor detenido).
    World { id: InstanceId, name: String },
    /// Una copia de seguridad (.zip tal cual).
    Backup { id: InstanceId, backup: String },
    /// El registro de consola guardado.
    Console { id: InstanceId },
}

impl RemoteItem {
    pub fn server(&self) -> InstanceId {
        match self {
            RemoteItem::File { id, .. }
            | RemoteItem::World { id, .. }
            | RemoteItem::Backup { id, .. }
            | RemoteItem::Console { id } => *id,
        }
    }

    /// Permiso que exige llevárselo.
    pub fn perm(&self) -> Perm {
        match self {
            RemoteItem::File { .. } => Perm::FilesRead,
            RemoteItem::World { .. } => Perm::World,
            RemoteItem::Backup { .. } => Perm::Backups,
            RemoteItem::Console { .. } => Perm::ConsoleRead,
        }
    }
}

/// Lo que ve el compañero de cada servidor que se le concedió.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteServer {
    pub id: InstanceId,
    pub name: String,
    pub perms: Vec<Perm>,
}

/// Una invitación pendiente (el código solo se muestra al crearla).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InviteInfo {
    pub invite_id: String,
    /// `None` = invitación de control total de la PC.
    pub server: Option<InstanceId>,
    pub server_name: String,
    pub perms: Vec<Perm>,
    pub created_at_unix_ms: i64,
    pub expires_at_unix_ms: i64,
    /// Control total de la PC (servidor casero).
    #[serde(default)]
    pub host_admin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompanionInfo {
    /// Llave pública del dispositivo del compañero (hex).
    pub companion: String,
    pub name: String,
    /// Huella corta para comprobar de palabra que es la PC correcta.
    pub fingerprint: String,
    pub added_at_unix_ms: i64,
    pub last_seen_unix_ms: Option<i64>,
    pub connected: bool,
    pub servers: Vec<RemoteServer>,
    /// Tiene control total de esta PC (todos los servidores y crear nuevos).
    #[serde(default)]
    pub host_admin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompanionsInfo {
    /// Esta PC acepta conexiones de compañeros.
    pub enabled: bool,
    /// El canal de compañeros está en marcha.
    pub online: bool,
    /// Huella de esta PC.
    pub fingerprint: Option<String>,
    pub error: Option<String>,
    pub invites: Vec<InviteInfo>,
    pub companions: Vec<CompanionInfo>,
}

/// Invitación recién creada: el código solo viaja una vez a la UI del propietario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewInvite {
    pub code: String,
    pub invite: InviteInfo,
}

/// PC de un amigo a la que esta PC puede administrar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteHost {
    /// Llave pública del anfitrión (hex).
    pub host: String,
    /// Nombre que el anfitrión puso a su PC.
    pub name: String,
    pub fingerprint: String,
    pub servers: Vec<RemoteServer>,
    pub last_ok_unix_ms: Option<i64>,
    pub last_error: Option<String>,
    /// Esta PC tiene control total de esa PC (es tu servidor casero): puede crear servidores.
    #[serde(default)]
    pub host_admin: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_grow_and_admin_has_everything() {
        let o = Role::Observador.perms();
        assert!(o.iter().all(|p| !p.high_risk()));
        assert!(!o.contains(&Perm::Start));
        let op = Role::Operador.perms();
        assert!(Role::Moderador.perms().iter().all(|p| op.contains(p)));
        assert!(!op.iter().any(|p| p.high_risk()));
        let mut a = Role::Administrador.perms();
        a.sort();
        let mut all = Perm::ALL.to_vec();
        all.sort();
        assert_eq!(a, all);
    }

    #[test]
    fn denies_by_default() {
        let id = InstanceId::new();
        use crate::protocol::RequestBody as R;
        for r in [
            R::Shutdown,
            R::Unregister {
                id,
                delete_files: false,
            },
            R::Alerts,
            R::CreateHostInvite { minutes: 60 },
            R::CheckTarget { path: "C:/x".into() },
            R::Detect { path: "C:/x".into() },
            R::DownloadFile {
                id,
                path: "x".into(),
                dest: "C:/x".into(),
            },
            R::UploadFile {
                id,
                source: "C:/x".into(),
                dest_dir: "".into(),
            },
            R::ForceKill {
                id,
                confirm: String::new(),
            },
            R::ApplyServerUpdate {
                id,
                loader_version: "130".into(),
            },
            R::SetStartup { enabled: true },
            R::SetKeepAwake { enabled: true },
            R::AddonImport {
                id,
                source: "C:/x.mcpack".into(),
            },
            R::ClearPlayerHistory { id },
            R::LuckPerms { id },
            R::ExportFriendPack {
                id,
                dest: "C:/x.mrpack".into(),
            },
            R::LuckPermsApply {
                id,
                change: merc_admin::luckperms::LpChange::CreateGroup { group: "vip".into() },
            },
            R::SetAutomation {
                id,
                auto_restart: Some(true),
                autostart: None,
            },
            R::Supporter,
            R::ActivateSupporter {
                license_key: "x".into(),
            },
            R::ForgetSupporter,
        ] {
            assert_eq!(need(&r), Need::Deny, "{r:?}");
        }
        assert_eq!(
            need(&R::SendCommand {
                id,
                command: "say hi".into()
            }),
            Need::On(id, Perm::ConsoleCommand)
        );
        assert_eq!(need(&R::Start { id }), Need::On(id, Perm::Start));
    }

    #[test]
    fn host_level_needs_full_control() {
        let id = InstanceId::new();
        use crate::protocol::RequestBody as R;
        for r in [
            R::FindJava,
            R::CreationDefaults,
            R::AcceptEula { id, accepted: true },
            R::InstallJava { major: 21 },
            R::PlayitStatus,
            R::PlayitEnable,
            R::PlayitLink,
            R::PlayitLoginUrl,
            R::PlayitUnlink,
            R::PlayitDisable,
        ] {
            assert_eq!(need(&r), Need::Host, "{r:?}");
        }
        // Lo que lee o escribe rutas de la PC del anfitrión sigue siendo solo local.
        assert_eq!(need(&R::CheckTarget { path: "/x".into() }), Need::Deny);
        assert_eq!(need(&R::Unregister { id, delete_files: true }), Need::Deny);
    }
}
