//! Mensajes entre la interfaz (o la herramienta `merc`) y el supervisor.
//!
//! Formato: una línea JSON por mensaje (UTF-8, `\n` al final), máximo [`MAX_MESSAGE_BYTES`].
//! Secuencia: el cliente envía [`Hello`]; el servidor responde [`Welcome`] o un error y cierra.
//! Después: [`Request`] → [`Response`], una a la vez por conexión.
//!
//! Regla de compatibilidad: añadir variantes o campos opcionales no cambia la versión;
//! cambiar el significado de algo existente sí incrementa [`PROTOCOL_VERSION`].

pub use merc_admin::backups::{BackupInfo, BackupReason, BackupScope};
use merc_admin::files::{FileEntry, SavedItem, TextFile, ZipExtracted};
use merc_admin::gamerules::RuleValue;
pub use merc_admin::health::{Alert, AlertKind, AlertLevel, AlertSettings, TpsReading};
use merc_admin::players::{PlayerAction, PlayerLists};
use merc_admin::props::PropEntry;
pub use merc_admin::schedule::{PlayersPolicy, RunOutcome, Schedule, ScheduledTask, TaskAction, TaskRun};
use merc_admin::worlds::WorldInfo;
pub use merc_content::local::{InstalledItem, UpdateInfo};
pub use merc_content::modrinth::{
    ContentHit, ContentProject, ContentVersion, Dependency, DependencyType, SearchPage, SearchSort, Side, VersionFile,
};
pub use merc_content::mrpack::ModpackSummary;
pub use merc_content::plan::{ContentPlan, OptionalDep, PlanItem};
pub use merc_content::{ContentKind, ContentSource, Platform};
use merc_core::detect::DetectedServer;
use merc_core::{Instance, InstanceId};
use merc_install::{CatalogEntry, Flavor, Resolved, Step};
pub use merc_net::playit::{AgentPhase, AgentStatus, ClaimState, Notice, Tunnel};
pub use merc_net::slp::PingResult;
use merc_supervisor::{ConsoleLine, MetricSample, StatusSnapshot};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
/// Texto exacto que el cliente debe enviar para autorizar un cierre forzado.
pub const FORCE_KILL_CONFIRMATION: &str = "ENTIENDO_QUE_PUEDO_PERDER_DATOS";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol: u32,
    pub token: String,
    /// Nombre del cliente para la auditoría ("merc-cli 0.1.0", "merchosting-app 0.2.0"…).
    pub client: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Welcome {
    pub protocol: u32,
    pub server_version: String,
    pub supervisor_pid: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HelloReply {
    Welcome(Welcome),
    Rejected { error: ApiError },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// Número de petición elegido por el cliente; se repite en la respuesta.
    pub seq: u64,
    #[serde(flatten)]
    pub body: RequestBody,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum RequestBody {
    Ping,
    /// Analiza una carpeta sin modificarla.
    Detect {
        path: PathBuf,
    },
    /// Busca instalaciones de Java y comprueba su versión real.
    FindJava,
    /// Registra una carpeta existente sin moverla ni modificarla.
    ImportInstance {
        name: String,
        path: PathBuf,
        java: PathBuf,
        min_memory_mb: u32,
        max_memory_mb: u32,
    },
    ListInstances,
    Status {
        id: InstanceId,
    },
    Start {
        id: InstanceId,
    },
    Stop {
        id: InstanceId,
        timeout_secs: u32,
    },
    Restart {
        id: InstanceId,
        stop_timeout_secs: u32,
    },
    ForceKill {
        id: InstanceId,
        confirm: String,
    },
    SendCommand {
        id: InstanceId,
        command: String,
    },
    Console {
        id: InstanceId,
        after: u64,
        limit: u32,
    },
    /// Escribe `eula=true` en el `eula.txt` del servidor. Solo debe enviarse después de que el
    /// usuario haya leído la EULA y marcado expresamente que la acepta (`accepted: true`).
    AcceptEula {
        id: InstanceId,
        accepted: bool,
    },
    /// Quita la instancia de MercHosting. Solo si `delete_files` (casilla marcada por el usuario)
    /// manda además la carpeta del servidor a la Papelera de reciclaje; si eso falla, no se quita.
    Unregister {
        id: InstanceId,
        #[serde(default)]
        delete_files: bool,
    },
    /// Muestras reales de CPU/memoria y jugadores conectados de un servidor.
    Metrics {
        id: InstanceId,
    },
    /// Datos de esta PC: memoria total, núcleos lógicos e IP de red local.
    SystemInfo,
    /// Versiones publicadas por la fuente oficial del tipo de servidor.
    Catalog {
        flavor: Flavor,
    },
    /// Resuelve una versión concreta (compilación, Java necesario, avisos) sin descargar nada.
    Resolve {
        flavor: Flavor,
        mc_version: String,
        /// Versión exacta del loader (la que pide un modpack).
        #[serde(default)]
        loader_version: Option<String>,
    },
    /// Carpeta por defecto para servidores nuevos y un puerto libre sugerido.
    CreationDefaults,
    /// Comprueba que se puede crear un servidor en esa carpeta (nueva o vacía).
    CheckTarget {
        path: PathBuf,
    },
    /// Crea un servidor nuevo. Devuelve un trabajo cuyo progreso se consulta con `job`.
    /// `accept_eula` solo puede ser `true` si el usuario marcó expresamente que acepta la EULA.
    CreateServer {
        name: String,
        flavor: Flavor,
        mc_version: String,
        path: PathBuf,
        java: PathBuf,
        min_memory_mb: u32,
        max_memory_mb: u32,
        #[serde(default)]
        properties: BTreeMap<String, String>,
        accept_eula: bool,
        /// Versión de un modpack de Modrinth que se instala encima (fija Minecraft y el loader).
        #[serde(default)]
        modpack_version: Option<String>,
        /// Catálogo del modpack (Modrinth por defecto).
        #[serde(default)]
        modpack_source: ContentSource,
        /// Versión concreta del loader (Fabric, Quilt, Forge, NeoForge). Sin ella, la recomendada.
        #[serde(default)]
        loader_version: Option<String>,
        /// Reglas del juego (`keep_inventory`…) que se aplican al encenderlo por primera vez.
        #[serde(default)]
        game_rules: BTreeMap<String, String>,
    },
    /// Versiones del loader que se pueden elegir para un tipo y una versión de Minecraft.
    LoaderVersions {
        flavor: Flavor,
        mc_version: String,
    },
    /// Copia un servidor detenido a una carpeta nueva y lo registra con otro puerto.
    CloneServer {
        id: InstanceId,
        name: String,
    },
    /// Reinicio automático tras caídas y encendido al iniciar Windows de un servidor.
    Automation {
        id: InstanceId,
    },
    SetAutomation {
        id: InstanceId,
        #[serde(default)]
        auto_restart: Option<bool>,
        #[serde(default)]
        autostart: Option<bool>,
    },
    /// Registra (o quita) el supervisor en el inicio de sesión de Windows del usuario.
    SetStartup {
        enabled: bool,
    },
    /// Archiva o desarchiva un servidor: archivado no se puede encender y se lista aparte.
    SetArchived {
        id: InstanceId,
        archived: bool,
    },
    /// Estado de un trabajo y las líneas de registro posteriores a `after`.
    Job {
        job_id: String,
        #[serde(default)]
        after: u64,
    },
    /// Pide cancelar un trabajo. Lo que ya se creó se deshace.
    CancelJob {
        job_id: String,
    },
    /// `server.properties` del servidor, en el orden del archivo y con los valores desescapados.
    Properties {
        id: InstanceId,
    },
    /// Cambia opciones de `server.properties` (con copia previa). `expected_modified` evita pisar
    /// cambios hechos por otro programa desde que se leyó.
    SetProperties {
        id: InstanceId,
        changes: BTreeMap<String, String>,
        expected_modified: Option<i64>,
    },
    /// Paquete de recursos del servidor: el supervisor lo descarga para comprobar que es un
    /// paquete válido, calcula su SHA-1 y escribe `resource-pack`, `resource-pack-sha1` y
    /// `require-resource-pack` (con copia previa). Una dirección vacía lo quita.
    SetResourcePack {
        id: InstanceId,
        url: String,
        #[serde(default)]
        required: bool,
    },
    /// Lista blanca, operadores, baneos y jugadores conectados.
    Players {
        id: InstanceId,
    },
    /// Acción sobre un jugador mediante el comando oficial de Minecraft (servidor encendido).
    PlayerAction {
        id: InstanceId,
        action: PlayerAction,
        name: String,
        #[serde(default)]
        reason: Option<String>,
    },
    /// Mensaje privado, modo de juego o teletransporte para un jugador conectado.
    PlayerCommand {
        id: InstanceId,
        name: String,
        command: merc_admin::players::PlayerCommand,
    },
    /// Jugadores que MercHosting vio conectarse (última conexión, sesiones, tiempo) y notas.
    PlayerHistory {
        id: InstanceId,
    },
    /// Nota privada sobre un jugador (vacía = quitarla).
    SetPlayerNote {
        id: InstanceId,
        name: String,
        note: String,
    },
    /// Borra el historial de conexiones y las notas de este servidor.
    ClearPlayerHistory {
        id: InstanceId,
    },
    /// LuckPerms: si está instalado y, con el servidor encendido, sus grupos y usuarios
    /// (leídos con `luckperms export`).
    LuckPerms {
        id: InstanceId,
    },
    /// Qué comando mandaría un cambio de LuckPerms y qué hace, sin mandarlo.
    LuckPermsPreview {
        id: InstanceId,
        change: merc_admin::luckperms::LpChange,
    },
    /// Aplica un cambio de LuckPerms: copia antes, comando validado y comprobación después.
    LuckPermsApply {
        id: InstanceId,
        change: merc_admin::luckperms::LpChange,
    },
    /// Separador de mods: de qué lado es cada mod de `mods/` (con evidencia) y la cuarentena.
    ModSides {
        id: InstanceId,
    },
    /// Mueve mods a `mods-cuarentena/` (reversible, nunca se borran). Servidor detenido.
    QuarantineMods {
        id: InstanceId,
        files: Vec<String>,
    },
    /// Devuelve mods de la cuarentena a `mods/`. Servidor detenido.
    RestoreMods {
        id: InstanceId,
        files: Vec<String>,
    },
    /// Qué llevaría el pack para amigos (mods de cliente con enlace oficial) y qué no.
    FriendPack {
        id: InstanceId,
    },
    /// Guarda el pack para amigos (`.mrpack` solo con enlaces) en la ruta elegida.
    ExportFriendPack {
        id: InstanceId,
        dest: PathBuf,
    },
    /// Contenido de una carpeta del servidor (`path` relativa; vacía = raíz).
    ListFiles {
        id: InstanceId,
        path: String,
    },
    ReadFile {
        id: InstanceId,
        path: String,
    },
    /// Guarda texto (con copia previa). Requiere el servidor detenido.
    WriteFile {
        id: InstanceId,
        path: String,
        content: String,
        expected_modified: Option<i64>,
    },
    MakeDir {
        id: InstanceId,
        path: String,
    },
    RenameFile {
        id: InstanceId,
        path: String,
        new_name: String,
    },
    /// Mueve un archivo o carpeta a otra carpeta del servidor (nunca sobrescribe).
    MoveFile {
        id: InstanceId,
        path: String,
        dest_dir: String,
    },
    /// Copia un archivo o carpeta a otra carpeta del servidor (nunca sobrescribe).
    CopyFile {
        id: InstanceId,
        path: String,
        dest_dir: String,
    },
    /// Extrae un .zip del servidor en una carpeta nueva junto a él.
    ExtractZip {
        id: InstanceId,
        path: String,
    },
    /// Mueve a la papelera de MercHosting (no borra). Requiere el servidor detenido.
    TrashFile {
        id: InstanceId,
        path: String,
    },
    /// Copia un archivo elegido por el usuario dentro de una carpeta del servidor.
    UploadFile {
        id: InstanceId,
        source: PathBuf,
        dest_dir: String,
    },
    /// Copia una copia de seguridad (.zip) a la ruta que eligió el usuario.
    DownloadBackup {
        id: InstanceId,
        backup_id: String,
        dest: PathBuf,
    },
    /// Copia un archivo del servidor a la ruta que eligió el usuario.
    DownloadFile {
        id: InstanceId,
        path: String,
        dest: PathBuf,
    },
    /// Copia el registro de consola guardado a la ruta que eligió el usuario.
    ExportConsole {
        id: InstanceId,
        dest: PathBuf,
    },
    /// Mundos de la carpeta del servidor y cuál está activo.
    Worlds {
        id: InstanceId,
    },
    DuplicateWorld {
        id: InstanceId,
        name: String,
        new_name: String,
    },
    /// Exporta un mundo a un .zip en la ruta que eligió el usuario.
    ExportWorld {
        id: InstanceId,
        name: String,
        dest: PathBuf,
    },
    /// Importa un mundo desde un .zip elegido por el usuario, como carpeta nueva.
    ImportWorld {
        id: InstanceId,
        source: PathBuf,
        new_name: String,
    },
    /// Cambia `level-name` para que el servidor cargue otro mundo.
    SetActiveWorld {
        id: InstanceId,
        name: String,
    },
    /// Mueve un mundo (y sus dimensiones) a la papelera. No puede ser el activo.
    TrashWorld {
        id: InstanceId,
        name: String,
    },
    /// Papelera y copias previas de este servidor.
    SavedItems {
        id: InstanceId,
    },
    RestoreSaved {
        id: InstanceId,
        item_id: String,
    },
    /// Borra definitivamente algo de la papelera o de las copias (confirmado por el usuario).
    PurgeSaved {
        id: InstanceId,
        item_id: String,
    },
    /// Reglas del juego, consultadas al servidor encendido.
    GameRules {
        id: InstanceId,
    },
    SetGameRule {
        id: InstanceId,
        key: String,
        value: String,
    },
    /// Actividad registrada de este servidor (auditoría), la más reciente primero.
    Activity {
        id: InstanceId,
        #[serde(default)]
        limit: u32,
    },
    /// Avisa a los jugadores por el chat y reinicia pasado `seconds`.
    RestartWithNotice {
        id: InstanceId,
        seconds: u32,
    },
    /// Estado de administración: mantenimiento, reinicio avisado pendiente y operación de mundos.
    AdminState {
        id: InstanceId,
    },
    /// Modo mantenimiento: activa la lista blanca (comando oficial) y al terminar la deja como estaba.
    SetMaintenance {
        id: InstanceId,
        on: bool,
    },
    /// Cancela un reinicio con aviso que aún no ocurrió.
    CancelRestart {
        id: InstanceId,
    },
    /// Cancela la operación de mundos en curso (duplicar, exportar, importar) y deshace lo hecho.
    CancelWorldOp {
        id: InstanceId,
    },
    /// Copias de seguridad del servidor, su ajuste de retención y el espacio libre.
    Backups {
        id: InstanceId,
    },
    /// Hace una copia comprobada. Con el servidor encendido pausa el guardado del juego
    /// (`save-off`, `save-all flush`) y lo reanuda al terminar (`save-on`).
    CreateBackup {
        id: InstanceId,
        scope: BackupScope,
        #[serde(default)]
        note: Option<String>,
    },
    /// Vuelve a leer la copia entera y compara cada archivo con su huella.
    VerifyBackup {
        id: InstanceId,
        backup: String,
    },
    /// Restaura una copia (servidor detenido). Antes hace una copia «Antes de restaurar».
    RestoreBackup {
        id: InstanceId,
        backup: String,
    },
    DeleteBackup {
        id: InstanceId,
        backup: String,
    },
    /// Las copias protegidas no se borran al limpiar las viejas.
    ProtectBackup {
        id: InstanceId,
        backup: String,
        protected: bool,
    },
    /// Cuántas copias sin proteger se conservan (las más viejas se borran al hacer una nueva).
    SetBackupSettings {
        id: InstanceId,
        keep: u32,
    },
    /// Salud: CPU y memoria de la PC, espacio en los discos del servidor y de las copias.
    Health {
        id: InstanceId,
    },
    /// Pide al servidor su TPS con su propio comando (Paper, Spigot, Purpur, Forge, NeoForge).
    MeasureTps {
        id: InstanceId,
    },
    /// Alertas de todos los servidores y sus ajustes.
    Alerts,
    /// Marca alertas como leídas (lista vacía = todas).
    MarkAlertsRead {
        #[serde(default)]
        ids: Vec<String>,
    },
    SetAlertSettings {
        settings: AlertSettings,
    },
    /// Estado de Playit.gg en esta PC: agente, vinculación y túneles.
    PlayitStatus,
    /// Activa Playit: descarga (si hace falta) el agente oficial verificado y lo pone en marcha.
    PlayitEnable,
    /// Apaga el agente (la vinculación se conserva).
    PlayitDisable,
    /// Empieza la vinculación con la cuenta de playit.gg de la persona (enlace oficial).
    PlayitLink,
    /// Olvida la vinculación de esta PC con la cuenta de Playit.
    PlayitUnlink,
    /// Enlace para entrar en la cuenta de playit.gg vinculada (lo da el agente).
    PlayitLoginUrl,
    /// Conexión de un servidor: direcciones de la red local y túneles de Playit hacia su puerto.
    Connection {
        id: InstanceId,
    },
    /// Prueba real de una dirección (ping de la lista de servidores de Minecraft).
    TestAddress {
        id: InstanceId,
        address: String,
    },
    /// Cambia `voice_host` (a la dirección de un túnel de voz detectado, o vacío) y/o el
    /// puerto de Simple Voice Chat. Guarda copia del archivo anterior.
    SetVoiceConfig {
        id: InstanceId,
        #[serde(default)]
        voice_host: Option<String>,
        #[serde(default)]
        port: Option<u16>,
    },
    /// Descarga Java (Eclipse Temurin, JRE de Windows x64) a la carpeta de MercHosting y lo
    /// verifica. Devuelve un trabajo con progreso.
    InstallJava {
        major: u32,
    },
    /// Estado de Supporter Edition en esta PC (lo guardado; no consulta a Gumroad).
    Supporter,
    /// Comprueba una clave de licencia de Gumroad y, si es válida, activa Supporter en esta PC.
    ActivateSupporter {
        license_key: String,
    },
    /// Vuelve a comprobar con Gumroad la clave guardada (reembolsos, claves desactivadas).
    RefreshSupporter,
    /// Quita la clave de esta PC (se puede volver a poner cuando quieras).
    ForgetSupporter,
    /// Icono actual (`server-icon.png`) del servidor.
    ServerIcon {
        id: InstanceId,
    },
    /// Convierte una imagen de la PC a 64×64 sin guardarla, para verla antes.
    PreviewServerIcon {
        source: String,
        #[serde(default)]
        fit: merc_admin::icon::IconFit,
    },
    /// Convierte una imagen a 64×64 y la guarda como `server-icon.png`. El icono anterior va a
    /// la papelera de MercHosting.
    SetServerIcon {
        id: InstanceId,
        source: String,
        #[serde(default)]
        fit: merc_admin::icon::IconFit,
    },
    /// Quita el icono (a la papelera de MercHosting).
    RemoveServerIcon {
        id: InstanceId,
    },
    /// ¿Hay una compilación o versión del loader más nueva para la misma versión de Minecraft?
    ServerUpdateCheck {
        id: InstanceId,
    },
    /// Actualiza el software del servidor (misma versión de Minecraft) con copia completa
    /// previa. Devuelve un trabajo con progreso.
    ApplyServerUpdate {
        id: InstanceId,
        loader_version: String,
    },
    /// Diagnóstico: problemas conocidos en mods, plugins y registros, con evidencias.
    Diagnose {
        id: InstanceId,
    },
    /// Cambia el Java y/o la memoria del servidor (con el servidor detenido).
    SetLaunch {
        id: InstanceId,
        #[serde(default)]
        java: Option<PathBuf>,
        #[serde(default)]
        min_memory_mb: Option<u32>,
        #[serde(default)]
        max_memory_mb: Option<u32>,
    },
    /// Centro de seguridad del servidor (ajustes, exposición, compañeros, copias y claves).
    SecurityReport {
        id: InstanceId,
    },
    /// Compañeros de esta PC: invitaciones, quién tiene acceso y con qué permisos.
    Companions,
    /// Permite (o deja de permitir) que compañeros administren servidores de esta PC.
    SetCompanionsEnabled {
        enabled: bool,
    },
    /// Crea un código de invitación de un solo uso para un servidor.
    CreateInvite {
        id: InstanceId,
        perms: Vec<crate::remote::Perm>,
        /// Validez en minutos (5 a 1440).
        minutes: u32,
    },
    CancelInvite {
        invite_id: String,
    },
    /// Cambia los permisos de un compañero en un servidor (vacío = sin acceso a ese servidor).
    SetCompanionPerms {
        companion: String,
        id: InstanceId,
        perms: Vec<crate::remote::Perm>,
    },
    /// Quita a un compañero por completo; su conexión se corta al momento.
    RemoveCompanion {
        companion: String,
    },
    /// Usa el código que dio un amigo para administrar sus servidores desde esta PC.
    JoinWithCode {
        code: String,
        /// Cómo te verá tu amigo.
        name: String,
    },
    /// PCs de amigos que esta PC puede administrar.
    RemoteHosts,
    ForgetHost {
        host: String,
    },
    /// Vuelve a pedir al amigo qué servidores y permisos tienes.
    RefreshHost {
        host: String,
    },
    /// Trae a esta PC algo de un servidor de un amigo (se comprueba su SHA-256 al terminar).
    RemoteDownload {
        host: String,
        item: crate::remote::RemoteItem,
        dest: PathBuf,
    },
    /// Sube un archivo de esta PC a un servidor de un amigo.
    RemoteUpload {
        host: String,
        id: InstanceId,
        source: PathBuf,
        dest_dir: String,
    },
    /// Petición a un servidor de un amigo (su PC decide si está permitida).
    RemoteCall {
        host: String,
        request: Box<RequestBody>,
    },
    /// Tareas programadas del servidor (con la hora de esta PC).
    Tasks {
        id: InstanceId,
    },
    /// Crea (sin `task.id`) o cambia una tarea programada.
    SaveTask {
        id: InstanceId,
        task: TaskDraft,
    },
    DeleteTask {
        id: InstanceId,
        task_id: String,
    },
    /// Ejecuta una tarea ya, sin esperar a su hora. Responde enseguida; el resultado queda en
    /// `last_run`.
    RunTaskNow {
        id: InstanceId,
        task_id: String,
    },
    /// Busca modpacks en Modrinth.
    ModpackSearch {
        #[serde(default)]
        source: ContentSource,
        query: String,
        #[serde(default)]
        sort: SearchSort,
        #[serde(default)]
        offset: u64,
    },
    /// Busca paquetes de recursos en Modrinth para la versión de Minecraft del servidor.
    ResourcePackSearch {
        id: InstanceId,
        query: String,
        #[serde(default)]
        sort: SearchSort,
        #[serde(default)]
        offset: u64,
    },
    /// Ficha de un paquete de recursos de Modrinth con sus versiones (las del Minecraft del servidor primero).
    ResourcePackProject {
        id: InstanceId,
        project: String,
    },
    /// Modpack instalado en el servidor (si se sabe) y sus versiones publicadas.
    ModpackStatus {
        id: InstanceId,
    },
    /// Qué cambiaría al pasar a otra versión del modpack (no toca nada).
    ModpackUpdatePreview {
        id: InstanceId,
        version_id: String,
    },
    /// Actualiza el modpack con copia completa previa y sin pisar lo que cambió la persona.
    /// Devuelve un trabajo con progreso.
    ModpackUpdateApply {
        id: InstanceId,
        version_id: String,
        /// La versión nueva pide otra versión del loader y la persona lo aceptó.
        #[serde(default)]
        accept_loader_change: bool,
    },
    /// Indica qué modpack y qué versión tiene un servidor creado antes de que se guardara.
    ModpackLink {
        id: InstanceId,
        #[serde(default)]
        source: ContentSource,
        version_id: String,
    },
    /// Olvida qué modpack tiene el servidor (no toca archivos).
    ModpackUnlink {
        id: InstanceId,
    },
    /// Ficha de un modpack con todas sus versiones.
    ModpackProject {
        #[serde(default)]
        source: ContentSource,
        project: String,
    },
    /// Descarga (verificado) el `.mrpack` de una versión y resume qué servidor hace falta.
    ModpackInspect {
        #[serde(default)]
        source: ContentSource,
        version_id: String,
    },
    /// Qué contenido admite el servidor (mods, plugins o ambos).
    ContentPlatforms {
        id: InstanceId,
    },
    /// Busca mods o plugins compatibles en Modrinth.
    ContentSearch {
        id: InstanceId,
        kind: ContentKind,
        /// Catálogo: Modrinth (por defecto) o CurseForge.
        #[serde(default)]
        source: ContentSource,
        query: String,
        #[serde(default)]
        sort: SearchSort,
        #[serde(default)]
        offset: u64,
    },
    /// Ficha de un proyecto y sus versiones compatibles con este servidor.
    ContentProject {
        id: InstanceId,
        kind: ContentKind,
        /// Catálogo: Modrinth (por defecto) o CurseForge.
        #[serde(default)]
        source: ContentSource,
        project: String,
    },
    /// Plan de instalación (dependencias, reemplazos, avisos) sin tocar nada.
    ContentPlan {
        id: InstanceId,
        kind: ContentKind,
        /// Catálogo: Modrinth (por defecto) o CurseForge.
        #[serde(default)]
        source: ContentSource,
        version_id: String,
    },
    /// Instala una versión con sus dependencias (servidor detenido). El plan se recalcula aquí.
    ContentInstall {
        id: InstanceId,
        kind: ContentKind,
        /// Catálogo: Modrinth (por defecto) o CurseForge.
        #[serde(default)]
        source: ContentSource,
        version_id: String,
    },
    /// Lo instalado en `mods/` o `plugins/`, con datos de Modrinth si hay conexión.
    ContentInstalled {
        id: InstanceId,
        kind: ContentKind,
    },
    /// Activa o desactiva un archivo (renombrándolo; reversible).
    ContentToggle {
        id: InstanceId,
        kind: ContentKind,
        file_name: String,
        enabled: bool,
    },
    /// Manda un mod o plugin a la papelera de MercHosting.
    ContentRemove {
        id: InstanceId,
        kind: ContentKind,
        file_name: String,
    },
    /// Cierra el supervisor. Se rechaza si hay servidores en marcha.
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentList {
    pub platform: Platform,
    pub items: Vec<InstalledItem>,
    /// Se pudo consultar Modrinth (nombres, iconos, actualizaciones).
    pub online: bool,
    pub offline_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Maintenance {
    pub since_unix: i64,
    /// Si la lista blanca ya estaba activada antes (al terminar se deja así).
    pub previous_whitelist: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldOp {
    /// `duplicate`, `export`, `import`, `install`, `backup`, `verify` o `restore`.
    pub kind: String,
    pub world: String,
    pub done_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimInfo {
    /// Enlace oficial que hay que abrir (`https://playit.gg/claim/<código>`).
    pub url: String,
    pub state: ClaimState,
    /// Si se terminó: la vinculación quedó hecha.
    pub done: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayitInfo {
    /// En esta PC se puede usar (el agente oficial es para Windows).
    pub supported: bool,
    pub enabled: bool,
    /// El agente verificado ya está descargado.
    pub installed: bool,
    pub agent_version: String,
    /// El agente responde por su canal local.
    pub running: bool,
    pub agent: Option<AgentStatus>,
    /// Por qué no responde o no arranca, si se sabe.
    pub agent_error: Option<String>,
    pub claim: Option<ClaimInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressTest {
    pub address: String,
    pub at_unix_ms: i64,
    pub result: Option<PingResult>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionInfo {
    /// `server-port` de server.properties.
    pub port: Option<u16>,
    /// Direcciones para jugar desde la misma red (`192.168.1.20:25565`).
    pub lan: Vec<String>,
    /// Túneles de Playit que llevan a este servidor.
    pub tunnels: Vec<Tunnel>,
    /// Última prueba de cada dirección (en esta sesión del supervisor).
    pub tests: Vec<AddressTest>,
    /// Puertos que usan los OTROS servidores registrados (para no ofrecer uno ocupado).
    #[serde(default)]
    pub ports_in_use: Vec<PortUse>,
    /// Simple Voice Chat, si está instalado (o si quedó un túnel suyo).
    #[serde(default)]
    pub voice: Option<VoiceInfo>,
    /// Estado por capas, con la hora de cada comprobación.
    #[serde(default)]
    pub layers: Vec<ConnLayer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerState {
    Operativo,
    Degradado,
    Caido,
    Desconocido,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnLayer {
    /// `server`, `agent`, `tunnel`, `external` o `voice`.
    pub key: String,
    pub state: LayerState,
    pub detail: String,
    pub checked_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceProblem {
    pub code: String,
    /// `error`, `warning` o `info`.
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceInfo {
    /// El mod o plugin está en mods/ o plugins/.
    pub installed: bool,
    pub enabled: bool,
    pub jar: Option<String>,
    /// Ruta relativa de `voicechat-server.properties` (si ya existe).
    pub config: Option<String>,
    /// Valor de `port` en el archivo (`-1` = el de Minecraft).
    pub port_setting: Option<i64>,
    /// Puerto UDP real de la voz.
    pub port: Option<u16>,
    pub voice_host: String,
    pub bind_address: String,
    /// Túneles de Playit hacia el puerto de la voz.
    pub tunnels: Vec<Tunnel>,
    /// Puerto libre que se propone si el actual choca con otro.
    #[serde(default)]
    pub suggested_port: Option<u16>,
    /// Puerto en el que la consola dice que arrancó la voz en esta sesión.
    pub listening: Option<u16>,
    /// La configuración cambió después de arrancar el servidor.
    pub restart_needed: bool,
    pub problems: Vec<VoiceProblem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LuckPermsInfo {
    /// `None` si no hay un .jar de LuckPerms en `plugins/` ni en `mods/`.
    pub install: Option<merc_admin::luckperms::LpInstall>,
    pub online: bool,
    /// Solo con el servidor encendido: LuckPerms es quien sabe leer su base de datos.
    pub snapshot: Option<merc_admin::luckperms::LpSnapshot>,
    /// Copias de los permisos guardadas antes de cambios (para `luckperms import`).
    pub backups: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LuckPermsPreview {
    pub command: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LuckPermsApplied {
    /// `true` si un export posterior muestra el cambio hecho.
    pub verified: bool,
    pub command: String,
    /// Nombre de la copia guardada antes del cambio.
    pub backup: String,
    pub snapshot: merc_admin::luckperms::LpSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationInfo {
    /// Reiniciar solo si se cae.
    pub auto_restart: bool,
    pub max_restarts: u32,
    /// Reinicios automáticos en los últimos 10 minutos.
    pub recent_restarts: u32,
    /// Se pasó del tope y dejó de reintentar (hasta que alguien lo encienda).
    pub gave_up: bool,
    /// Encender este servidor cuando Windows inicia sesión (si el arranque con Windows está activo).
    pub autostart: bool,
    /// El arranque con Windows existe en este sistema.
    pub startup_available: bool,
    /// MercHosting está en el inicio de sesión de Windows.
    pub startup_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupporterInfo {
    /// La tienda está configurada (hay producto de Gumroad). Si no, comprar y activar no están
    /// disponibles todavía.
    pub available: bool,
    /// Página de compra oficial (Gumroad), si ya existe.
    pub checkout_url: Option<String>,
    pub price_usd: u32,
    /// Supporter activo en esta PC.
    pub active: bool,
    /// Correo de la compra, oculto en parte (`a•••@gmail.com`).
    pub email_hint: Option<String>,
    /// Últimos caracteres de la clave guardada, para reconocerla.
    pub key_hint: Option<String>,
    /// Fecha de la compra según Gumroad.
    pub purchased: Option<String>,
    pub checked_at_unix_ms: Option<i64>,
    /// Hace más de 7 días que no se comprueba: conviene volver a mirar (si hay red).
    pub stale: bool,
    /// Por qué se desactivó (reembolso, contracargo, clave desactivada…).
    pub revoked_reason: Option<String>,
    /// La última comprobación no pudo hacerse (sin red, Gumroad caído). No cambia el estado.
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerIconInfo {
    pub present: bool,
    /// PNG de 64×64: lo único que Minecraft muestra.
    pub valid: bool,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// El archivo en base64 (como mucho unos KB), para mostrarlo.
    pub png_base64: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconPreview {
    pub png_base64: String,
    pub source_width: u32,
    pub source_height: u32,
    pub format: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModpackVersionChoice {
    pub version_id: String,
    pub version_number: String,
    pub name: String,
    pub date_published: String,
    pub game_versions: Vec<String>,
    /// Misma versión de Minecraft y mismo loader que el servidor.
    pub compatible: bool,
    pub current: bool,
    pub newer: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModpackStatusInfo {
    pub linked: Option<merc_content::packupdate::LinkedPack>,
    /// Se pudo consultar el catálogo.
    pub online: bool,
    pub error: Option<String>,
    /// Versiones publicadas (la más nueva primero).
    pub versions: Vec<ModpackVersionChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModpackUpdatePlan {
    pub from_version: String,
    pub to_version: String,
    pub to_version_id: String,
    pub mc_version: String,
    pub loader: Option<String>,
    pub loader_from: Option<String>,
    pub loader_to: Option<String>,
    /// La versión nueva pide otra versión del loader que la instalada.
    pub loader_mismatch: bool,
    /// Solo lo que cambia o se conserva por decisión de la persona.
    pub changes: Vec<merc_content::packupdate::PlannedChange>,
    pub download_bytes: u64,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerUpdateInfo {
    /// MercHosting puede actualizar este tipo de servidor en su carpeta.
    pub supported: bool,
    /// Por qué no (si no se puede).
    pub reason: Option<String>,
    pub mc_version: Option<String>,
    /// Compilación o versión del loader instalada.
    pub current: Option<String>,
    /// La más nueva para la misma versión de Minecraft.
    pub latest: Option<String>,
    pub newer: bool,
    pub experimental: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortUse {
    pub port: u16,
    pub server: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskSpace {
    /// `server` (carpeta del servidor) o `backups` (copias de seguridad).
    pub what: String,
    pub path: String,
    pub free_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthReport {
    /// CPU de toda la PC (0–100), medida por el supervisor; `None` hasta la primera medición.
    pub pc_cpu_percent: Option<f32>,
    pub pc_memory_used_bytes: Option<u64>,
    pub pc_memory_total_bytes: Option<u64>,
    pub disks: Vec<DiskSpace>,
    /// Memoria máxima asignada al servidor (`-Xmx`).
    pub max_memory_mb: u32,
    /// Comando con el que el servidor informa su TPS (`None`: este tipo de servidor no lo informa).
    pub tps_command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertList {
    pub items: Vec<Alert>,
    pub unread: u32,
    pub settings: AlertSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDraft {
    #[serde(default)]
    pub id: Option<String>,
    pub enabled: bool,
    pub action: TaskAction,
    pub schedule: Schedule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskList {
    pub items: Vec<ScheduledTask>,
    pub now_unix_ms: i64,
    /// Diferencia de la hora de esta PC con UTC, en minutos (Lima: -300).
    pub utc_offset_minutes: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupList {
    pub items: Vec<BackupInfo>,
    /// Copias sin proteger que se conservan.
    pub keep: u32,
    /// Espacio libre en el disco de las copias (`None` si no se pudo medir).
    pub free_bytes: Option<u64>,
    /// Carpeta donde se guardan.
    pub folder: String,
    /// Copia de la que salió la última restauración, si la hubo en esta respuesta.
    #[serde(default)]
    pub restored: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminState {
    pub maintenance: Option<Maintenance>,
    /// `white-list` de server.properties.
    pub whitelist_on: bool,
    /// `enforce-whitelist`: al activar la lista blanca se expulsa a los conectados que no estén.
    pub enforce_whitelist: bool,
    /// Segundos que faltan para un reinicio avisado, si hay uno pendiente.
    pub restart_in_seconds: Option<u32>,
    pub world_op: Option<WorldOp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityEntry {
    pub at_unix: i64,
    /// Quién: `local:merchosting-app`, `local:merc-cli`, `supervisor`…
    pub actor: String,
    pub action: String,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobDownload {
    pub file: String,
    pub done: u64,
    pub total: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobLogLine {
    pub seq: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobSnapshot {
    pub job_id: String,
    pub name: String,
    pub state: JobState,
    pub step: Option<Step>,
    /// Frase del paso actual ("Descargando paper-26.1.2-14.jar").
    pub message: String,
    pub download: Option<JobDownload>,
    pub log: Vec<JobLogLine>,
    pub next_after: u64,
    /// Servidor registrado al terminar bien.
    pub instance_id: Option<InstanceId>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaCandidate {
    pub path: PathBuf,
    pub major: Option<u32>,
    pub description: String,
    pub error: Option<String>,
    /// Es un JDK (tiene `javac`): necesario para compilar Spigot con BuildTools.
    #[serde(default)]
    pub jdk: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceSummary {
    pub instance: Instance,
    pub status: StatusSnapshot,
    /// Lectura actual de `eula.txt` (`false` si falta o dice `eula=false`).
    #[serde(default)]
    pub eula_accepted: bool,
    /// `server-port` de `server.properties`, si existe.
    #[serde(default)]
    pub configured_port: Option<u16>,
    /// Archivado: no se puede encender y se muestra aparte.
    #[serde(default)]
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum ResponseBody {
    Pong {
        server_version: String,
        supervisor_pid: u32,
    },
    Detected(DetectedServer),
    JavaCandidates(Vec<JavaCandidate>),
    Instance(Instance),
    Instances(Vec<InstanceSummary>),
    Status(StatusSnapshot),
    Console {
        lines: Vec<ConsoleLine>,
        next_after: u64,
    },
    Metrics {
        samples: Vec<MetricSample>,
        /// Jugadores conectados según la consola del servidor (vacío si no hay consola conectada).
        players: Vec<String>,
        /// `max-players` de server.properties, si está definido.
        max_players: Option<u32>,
    },
    System {
        total_memory_bytes: Option<u64>,
        logical_cpus: u32,
        /// IPv4 de esta PC en la red local (para que entren desde la misma red), si se detecta.
        lan_ipv4: Option<String>,
    },
    Catalog(Vec<CatalogEntry>),
    Resolved(Resolved),
    CreationDefaults {
        servers_dir: PathBuf,
        suggested_port: u16,
        /// Hay clave de API de CurseForge en esta instalación.
        #[serde(default)]
        curseforge: bool,
    },
    Job(JobSnapshot),
    Properties {
        exists: bool,
        entries: Vec<PropEntry>,
        modified_unix_ms: Option<i64>,
    },
    Players {
        lists: PlayerLists,
        /// Conectados según la consola (vacío si no hay consola conectada).
        online: Vec<String>,
        /// Si se pueden usar las acciones (servidor encendido con consola).
        can_act: bool,
    },
    Files {
        path: String,
        entries: Vec<FileEntry>,
    },
    TextFile(TextFile),
    ZipExtracted(ZipExtracted),
    /// Resultado de una operación de archivos: ruta resultante o dónde quedó en la papelera.
    FileDone {
        path: String,
    },
    Worlds {
        worlds: Vec<WorldInfo>,
        level_name: String,
    },
    SavedItems(Vec<SavedItem>),
    GameRules {
        rules: Vec<RuleValue>,
        /// Nombres modernos (1.21.11+) o antiguos.
        new_names: bool,
    },
    Activity(Vec<ActivityEntry>),
    AdminState(AdminState),
    ContentPlatforms(Vec<Platform>),
    Backups(BackupList),
    Tasks(TaskList),
    Health(HealthReport),
    Tps(TpsReading),
    Alerts(AlertList),
    Playit(Box<PlayitInfo>),
    PlayitLoginUrl(String),
    Connection(Box<ConnectionInfo>),
    Companions(Box<crate::remote::CompanionsInfo>),
    Diagnosis(Box<merc_admin::diagnose::Diagnosis>),
    ServerUpdate(Box<ServerUpdateInfo>),
    ModpackStatus(Box<ModpackStatusInfo>),
    ModpackUpdatePlan(Box<ModpackUpdatePlan>),
    ServerIcon(ServerIconInfo),
    LoaderVersions(Vec<merc_install::LoaderOption>),
    Automation(AutomationInfo),
    PlayerHistory(Vec<merc_admin::players::PlayerRecord>),
    LuckPerms(Box<LuckPermsInfo>),
    FriendPack(Box<merc_content::friendpack::FriendPackPlan>),
    ModSides(Box<merc_admin::modsides::ModSidesInfo>),
    LuckPermsPreview(LuckPermsPreview),
    LuckPermsApplied(Box<LuckPermsApplied>),
    Supporter(SupporterInfo),
    IconPreview(IconPreview),
    SecurityReport(Box<merc_admin::security::SecurityReport>),
    Invite(Box<crate::remote::NewInvite>),
    RemoteHosts(Vec<crate::remote::RemoteHost>),
    ContentSearch(SearchPage),
    ContentDetails {
        project: Box<ContentProject>,
        versions: Vec<ContentVersion>,
    },
    ContentPlan(ContentPlan),
    ContentList(ContentList),
    ModpackSummary(ModpackSummary),
    Ok,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    AuthFailed,
    VersionMismatch,
    BadRequest,
    NotFound,
    Conflict,
    InvalidState,
    NoConsole,
    /// No se pudo hablar con una fuente oficial (sin Internet, servicio caído…).
    Network,
    /// Un compañero pidió algo que su permiso no cubre (o que solo puede hacer el dueño).
    Forbidden,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{message}")]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
}

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub seq: u64,
    #[serde(flatten)]
    pub result: ResponseResult,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseResult {
    Ok(ResponseBody),
    Err(ApiError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_json_shape_is_stable() {
        let r = Request {
            seq: 7,
            body: RequestBody::Stop {
                id: InstanceId(uuid::Uuid::nil()),
                timeout_secs: 60,
            },
        };
        let j = serde_json::to_string(&r).unwrap();
        assert_eq!(
            j,
            r#"{"seq":7,"op":"stop","id":"00000000-0000-0000-0000-000000000000","timeout_secs":60}"#
        );
        assert_eq!(serde_json::from_str::<Request>(&j).unwrap(), r);
        let ping: Request = serde_json::from_str(r#"{"seq":1,"op":"ping"}"#).unwrap();
        assert_eq!(ping.body, RequestBody::Ping);
    }

    #[test]
    fn response_roundtrip() {
        let ok = Response {
            seq: 3,
            result: ResponseResult::Ok(ResponseBody::Ok),
        };
        let j = serde_json::to_string(&ok).unwrap();
        assert_eq!(serde_json::from_str::<Response>(&j).unwrap(), ok);
        let err = Response {
            seq: 4,
            result: ResponseResult::Err(ApiError::new(ErrorCode::Conflict, "x")),
        };
        let j = serde_json::to_string(&err).unwrap();
        assert_eq!(serde_json::from_str::<Response>(&j).unwrap(), err);
    }

    #[test]
    fn hello_reply_variants() {
        let w = HelloReply::Welcome(Welcome {
            protocol: 1,
            server_version: "0.1.0".into(),
            supervisor_pid: 9,
        });
        let j = serde_json::to_string(&w).unwrap();
        assert_eq!(serde_json::from_str::<HelloReply>(&j).unwrap(), w);
        let r = HelloReply::Rejected {
            error: ApiError::new(ErrorCode::AuthFailed, "no"),
        };
        let j = serde_json::to_string(&r).unwrap();
        assert_eq!(serde_json::from_str::<HelloReply>(&j).unwrap(), r);
    }
}
