//! Contrato del protocolo v1: ejemplos JSON generados con los tipos reales de Rust.
//!
//! El archivo `docs/contratos/ipc-v1-ejemplos.json` lo leen también las pruebas de la interfaz
//! (TypeScript). Si un cambio en Rust altera la forma de los mensajes, esta prueba falla y obliga
//! a revisar la interfaz. Para regenerarlo a propósito: `MERC_UPDATE_CONTRACT=1 cargo test -p merc-ipc`.

use merc_admin::files::{FileEntry, SavedItem, SavedKind, TextFile, ZipExtracted};
use merc_admin::gamerules::{RuleKind, RuleValue};
use merc_admin::players::{ListedPlayer, PlayerAction, PlayerLists};
use merc_admin::props::PropEntry;
use merc_admin::worlds::WorldInfo;
use merc_core::detect::{Confidence, DetectedServer, ServerKind};
use merc_core::launch::{LaunchProfile, LaunchTarget};
use merc_core::{Instance, InstanceId, RuntimeState};
use merc_install::{Artifact, CatalogEntry, Checksum, Flavor, Resolved, Step};
use merc_ipc::*;
use merc_supervisor::{ConsoleLine, ConsoleStream, ExitInfo, MetricSample, StatusSnapshot};
use serde_json::{json, Value};
use std::path::Path;

fn id() -> InstanceId {
    InstanceId(uuid::Uuid::parse_str("2f6d1c3a-8b4e-4c1d-9a7e-5b3c2d1e0f9a").unwrap())
}

fn instance() -> Instance {
    Instance {
        id: id(),
        name: "Mi servidor".into(),
        root: r"C:\Users\ejemplo\Servidores\neoforge".into(),
        kind: ServerKind::NeoForge,
        mc_version: Some("1.21.1".into()),
        loader_version: Some("21.1.259".into()),
        launch: LaunchProfile {
            java: r"C:\Program Files\Eclipse Adoptium\jdk-21\bin\java.exe".into(),
            min_memory_mb: 0,
            max_memory_mb: 4096,
            extra_jvm_args: vec![],
            target: LaunchTarget::ArgsFile {
                args_file: "libraries/net/neoforged/neoforge/21.1.259/win_args.txt".into(),
                user_jvm_args: Some("user_jvm_args.txt".into()),
            },
        },
        imported: true,
        created_at_unix: 1_793_300_000,
    }
}

fn status_online() -> StatusSnapshot {
    StatusSnapshot {
        state: RuntimeState::Online,
        has_console: true,
        pid: Some(3556),
        port: Some(25565),
        started_at_unix_ms: Some(1_793_300_100_000),
        online_at_unix_ms: Some(1_793_300_115_000),
        last_exit: None,
        hint: None,
        log_file: Some(r"C:\Users\ejemplo\AppData\Local\MercHosting\logs\x\console.log".into()),
    }
}

fn status_crashed() -> StatusSnapshot {
    StatusSnapshot {
        state: RuntimeState::Crashed,
        has_console: false,
        pid: None,
        port: None,
        started_at_unix_ms: Some(1),
        online_at_unix_ms: None,
        last_exit: Some(ExitInfo {
            code: Some(1),
            forced: false,
            requested: false,
            at_unix_ms: 2,
        }),
        hint: Some("El servidor terminó inesperadamente (código 1).".into()),
        log_file: None,
    }
}

fn examples() -> Value {
    let reqs = vec![
        RequestBody::Ping,
        RequestBody::ListInstances,
        RequestBody::Detect {
            path: r"C:\Servidores\a".into(),
        },
        RequestBody::FindJava,
        RequestBody::ImportInstance {
            name: "Mi servidor".into(),
            path: r"C:\Servidores\a".into(),
            java: r"C:\Java\bin\java.exe".into(),
            min_memory_mb: 0,
            max_memory_mb: 4096,
        },
        RequestBody::Start { id: id() },
        RequestBody::Stop {
            id: id(),
            timeout_secs: 120,
        },
        RequestBody::Restart {
            id: id(),
            stop_timeout_secs: 120,
        },
        RequestBody::ForceKill {
            id: id(),
            confirm: FORCE_KILL_CONFIRMATION.into(),
        },
        RequestBody::SendCommand {
            id: id(),
            command: "say hola".into(),
        },
        RequestBody::Console {
            id: id(),
            after: 10,
            limit: 500,
        },
        RequestBody::AcceptEula {
            id: id(),
            accepted: true,
        },
        RequestBody::Unregister {
            id: id(),
            delete_files: true,
        },
        RequestBody::Status { id: id() },
        RequestBody::Metrics { id: id() },
        RequestBody::SystemInfo,
        RequestBody::Catalog { flavor: Flavor::Paper },
        RequestBody::Catalog { flavor: Flavor::Spigot },
        RequestBody::Catalog {
            flavor: Flavor::Bedrock,
        },
        RequestBody::Catalog {
            flavor: Flavor::Arclight,
        },
        RequestBody::Resolve {
            flavor: Flavor::NeoForge,
            mc_version: "26.1.2".into(),
            loader_version: Some("26.1.2.76".into()),
        },
        RequestBody::CreationDefaults,
        RequestBody::CheckTarget {
            path: r"C:\Users\ejemplo\MercHosting\Servidores\Mi servidor".into(),
        },
        RequestBody::CreateServer {
            name: "Mi servidor".into(),
            flavor: Flavor::Paper,
            mc_version: "26.1.2".into(),
            path: r"C:\Users\ejemplo\MercHosting\Servidores\Mi servidor".into(),
            java: r"C:\Program Files\Eclipse Adoptium\jdk-25\bin\java.exe".into(),
            min_memory_mb: 0,
            max_memory_mb: 4096,
            properties: [
                ("server-port".to_string(), "25565".to_string()),
                ("gamemode".to_string(), "survival".to_string()),
            ]
            .into_iter()
            .collect(),
            accept_eula: true,
            modpack_version: Some("packv1".into()),
            modpack_source: ContentSource::Modrinth,
            loader_version: None,
            game_rules: Default::default(),
        },
        RequestBody::Job {
            job_id: "6b1f0c1e-2a3b-4c5d-8e9f-0a1b2c3d4e5f".into(),
            after: 12,
        },
        RequestBody::CancelJob {
            job_id: "6b1f0c1e-2a3b-4c5d-8e9f-0a1b2c3d4e5f".into(),
        },
        RequestBody::Properties { id: id() },
        RequestBody::SetProperties {
            id: id(),
            changes: [("max-players".to_string(), "10".to_string())].into_iter().collect(),
            expected_modified: Some(1_793_300_000_000),
        },
        RequestBody::SetResourcePack {
            id: id(),
            url: "https://ejemplo.dev/texturas.zip".into(),
            required: false,
        },
        RequestBody::Players { id: id() },
        RequestBody::PlayerAction {
            id: id(),
            action: PlayerAction::Ban,
            name: "Griefer".into(),
            reason: Some("Romper casas".into()),
        },
        RequestBody::ListFiles {
            id: id(),
            path: "config".into(),
        },
        RequestBody::ReadFile {
            id: id(),
            path: "config/neoforge-server.toml".into(),
        },
        RequestBody::WriteFile {
            id: id(),
            path: "config/neoforge-server.toml".into(),
            content: "a = 1\n".into(),
            expected_modified: Some(1_793_300_000_000),
        },
        RequestBody::MakeDir {
            id: id(),
            path: "mods/extra".into(),
        },
        RequestBody::RenameFile {
            id: id(),
            path: "mods/viejo.jar".into(),
            new_name: "viejo.jar.desactivado".into(),
        },
        RequestBody::MoveFile {
            id: id(),
            path: "mods/viejo.jar".into(),
            dest_dir: "mods-viejos".into(),
        },
        RequestBody::CopyFile {
            id: id(),
            path: "config/create.toml".into(),
            dest_dir: "copias-config".into(),
        },
        RequestBody::ExtractZip {
            id: id(),
            path: "descargas/mapa.zip".into(),
        },
        RequestBody::TrashFile {
            id: id(),
            path: "mods/roto.jar".into(),
        },
        RequestBody::UploadFile {
            id: id(),
            source: r"C:\Users\ejemplo\Downloads\mod.jar".into(),
            dest_dir: "mods".into(),
        },
        RequestBody::DownloadFile {
            id: id(),
            path: "logs/latest.log".into(),
            dest: r"C:\Users\ejemplo\Desktop\latest.log".into(),
        },
        RequestBody::ExportConsole {
            id: id(),
            dest: r"C:\Users\ejemplo\Desktop\consola.log".into(),
        },
        RequestBody::Worlds { id: id() },
        RequestBody::DuplicateWorld {
            id: id(),
            name: "world".into(),
            new_name: "world-copia".into(),
        },
        RequestBody::ExportWorld {
            id: id(),
            name: "world".into(),
            dest: r"C:\Users\ejemplo\Desktop\world.zip".into(),
        },
        RequestBody::ImportWorld {
            id: id(),
            source: r"C:\Users\ejemplo\Downloads\mapa.zip".into(),
            new_name: "mapa".into(),
        },
        RequestBody::SetActiveWorld {
            id: id(),
            name: "mapa".into(),
        },
        RequestBody::TrashWorld {
            id: id(),
            name: "world-copia".into(),
        },
        RequestBody::SavedItems { id: id() },
        RequestBody::RestoreSaved {
            id: id(),
            item_id: "papelera/20261030-041500-123".into(),
        },
        RequestBody::PurgeSaved {
            id: id(),
            item_id: "copias/20261030-041500-123".into(),
        },
        RequestBody::GameRules { id: id() },
        RequestBody::SetGameRule {
            id: id(),
            key: "keep_inventory".into(),
            value: "true".into(),
        },
        RequestBody::Activity { id: id(), limit: 100 },
        RequestBody::RestartWithNotice { id: id(), seconds: 60 },
        RequestBody::AdminState { id: id() },
        RequestBody::SetMaintenance { id: id(), on: true },
        RequestBody::CancelRestart { id: id() },
        RequestBody::CancelWorldOp { id: id() },
        RequestBody::Backups { id: id() },
        RequestBody::CreateBackup {
            id: id(),
            scope: BackupScope::World,
            note: Some("Antes del evento".into()),
        },
        RequestBody::VerifyBackup {
            id: id(),
            backup: "20261030-131500-123-mundo".into(),
        },
        RequestBody::RestoreBackup {
            id: id(),
            backup: "20261030-131500-123-mundo".into(),
        },
        RequestBody::DeleteBackup {
            id: id(),
            backup: "20261030-131500-123-mundo".into(),
        },
        RequestBody::ProtectBackup {
            id: id(),
            backup: "20261030-131500-123-mundo".into(),
            protected: true,
        },
        RequestBody::SetBackupSettings { id: id(), keep: 10 },
        RequestBody::Tasks { id: id() },
        RequestBody::PlayitStatus,
        RequestBody::PlayitEnable,
        RequestBody::PlayitDisable,
        RequestBody::PlayitLink,
        RequestBody::PlayitUnlink,
        RequestBody::PlayitLoginUrl,
        RequestBody::Connection { id: id() },
        RequestBody::TestAddress {
            id: id(),
            address: "dark-sea.joinmc.link".into(),
        },
        RequestBody::TestBedrockAddress {
            id: id(),
            address: "147.185.221.20:19132".into(),
        },
        RequestBody::SetVoiceConfig {
            id: id(),
            voice_host: Some("147.185.221.181:25732".into()),
            port: Some(24455),
        },
        RequestBody::InstallJava { major: 21 },
        RequestBody::Diagnose { id: id() },
        RequestBody::ServerUpdateCheck { id: id() },
        RequestBody::ModpackStatus { id: id() },
        RequestBody::ModpackUpdatePreview {
            id: id(),
            version_id: "packv2".into(),
        },
        RequestBody::ModpackUpdateApply {
            id: id(),
            version_id: "packv2".into(),
            accept_loader_change: false,
        },
        RequestBody::ModpackLink {
            id: id(),
            source: ContentSource::Modrinth,
            version_id: "packv1".into(),
        },
        RequestBody::ModpackUnlink { id: id() },
        RequestBody::LoaderVersions {
            flavor: Flavor::Fabric,
            mc_version: "1.21.1".into(),
        },
        RequestBody::CloneServer {
            id: id(),
            name: "Survival (copia)".into(),
        },
        RequestBody::SetArchived {
            id: id(),
            archived: true,
        },
        RequestBody::PlayerCommand {
            id: id(),
            name: "Alex".into(),
            command: merc_admin::players::PlayerCommand::TeleportCoords { x: 10, y: 64, z: -3 },
        },
        RequestBody::PlayerCommand {
            id: id(),
            name: "Alex".into(),
            command: merc_admin::players::PlayerCommand::GameMode {
                mode: merc_admin::players::GameMode::Creative,
            },
        },
        RequestBody::PlayerHistory { id: id() },
        RequestBody::SetPlayerNote {
            id: id(),
            name: "Alex".into(),
            note: "Construyó el castillo".into(),
        },
        RequestBody::ClearPlayerHistory { id: id() },
        RequestBody::LuckPerms { id: id() },
        RequestBody::FriendPack { id: id() },
        RequestBody::Addons { id: id() },
        RequestBody::BedrockPlayers { id: id() },
        RequestBody::BedrockAllowlist {
            id: id(),
            change: merc_admin::bedrock_players::AllowChange::Add {
                name: "Steve Craft".into(),
                ignores_player_limit: false,
            },
        },
        RequestBody::BedrockAllowlist {
            id: id(),
            change: merc_admin::bedrock_players::AllowChange::Remove { name: "Alex".into() },
        },
        RequestBody::BedrockAllowlist {
            id: id(),
            change: merc_admin::bedrock_players::AllowChange::SetIgnoresLimit {
                name: "Alex".into(),
                value: true,
            },
        },
        RequestBody::BedrockAllowlistEnabled {
            id: id(),
            enabled: true,
        },
        RequestBody::BedrockPermission {
            id: id(),
            xuid: "2535428000000000".into(),
            level: Some(merc_admin::bedrock_players::Level::Operator),
        },
        RequestBody::AddonImport {
            id: id(),
            source: "C:/Users/amir/Downloads/dragones.mcaddon".into(),
        },
        RequestBody::AddonSetActive {
            id: id(),
            uuid: "11111111-2222-3333-4444-555555555555".into(),
            active: false,
        },
        RequestBody::AddonRemove {
            id: id(),
            uuid: "11111111-2222-3333-4444-555555555555".into(),
        },
        RequestBody::ModSides { id: id() },
        RequestBody::QuarantineMods {
            id: id(),
            files: vec!["zoomify-2.14.jar".into()],
        },
        RequestBody::RestoreMods {
            id: id(),
            files: vec!["zoomify-2.14.jar".into()],
        },
        RequestBody::ExportFriendPack {
            id: id(),
            dest: "C:/Users/Ana/Desktop/Survival.mrpack".into(),
        },
        RequestBody::LuckPermsPreview {
            id: id(),
            change: merc_admin::luckperms::LpChange::AddParent {
                target: merc_admin::luckperms::LpTarget {
                    kind: merc_admin::luckperms::LpTargetKind::User,
                    name: "069a79f4-44e9-4726-a5be-fca90e38aaf5".into(),
                },
                group: "vip".into(),
            },
        },
        RequestBody::LuckPermsApply {
            id: id(),
            change: merc_admin::luckperms::LpChange::SetPermission {
                target: merc_admin::luckperms::LpTarget {
                    kind: merc_admin::luckperms::LpTargetKind::Group,
                    name: "vip".into(),
                },
                node: "essentials.fly".into(),
                value: true,
            },
        },
        RequestBody::Automation { id: id() },
        RequestBody::SetAutomation {
            id: id(),
            auto_restart: Some(true),
            autostart: None,
        },
        RequestBody::SetStartup { enabled: true },
        RequestBody::SetKeepAwake { enabled: true },
        RequestBody::Supporter,
        RequestBody::ActivateSupporter {
            license_key: "ABCD1234-EFGH5678-IJKL9012-MNOP3456".into(),
        },
        RequestBody::RefreshSupporter,
        RequestBody::ForgetSupporter,
        RequestBody::ServerIcon { id: id() },
        RequestBody::PreviewServerIcon {
            source: r"C:\Users\Alex\Pictures\logo.jpg".into(),
            fit: merc_admin::icon::IconFit::Crop,
        },
        RequestBody::SetServerIcon {
            id: id(),
            source: r"C:\Users\Alex\Pictures\logo.jpg".into(),
            fit: merc_admin::icon::IconFit::Contain,
        },
        RequestBody::RemoveServerIcon { id: id() },
        RequestBody::ApplyServerUpdate {
            id: id(),
            loader_version: "130".into(),
        },
        RequestBody::SecurityReport { id: id() },
        RequestBody::SetLaunch {
            id: id(),
            java: Some(r"C:\Program Files\Eclipse Adoptium\jdk-21\bin\java.exe".into()),
            min_memory_mb: None,
            max_memory_mb: Some(6144),
        },
        RequestBody::Companions,
        RequestBody::SetCompanionsEnabled { enabled: true },
        RequestBody::CreateInvite {
            id: id(),
            perms: vec![merc_ipc::remote::Perm::Status, merc_ipc::remote::Perm::ConsoleRead],
            minutes: 60,
        },
        RequestBody::CreateHostInvite { minutes: 60 },
        RequestBody::CancelInvite {
            invite_id: "a1b2c3d4e5f6".into(),
        },
        RequestBody::SetCompanionPerms {
            companion: "ab".repeat(32),
            id: id(),
            perms: vec![merc_ipc::remote::Perm::Start],
        },
        RequestBody::RemoveCompanion {
            companion: "ab".repeat(32),
        },
        RequestBody::JoinWithCode {
            code: "MH1-ABCDEF".into(),
            name: "Shandé".into(),
        },
        RequestBody::RemoteHosts,
        RequestBody::ForgetHost { host: "cd".repeat(32) },
        RequestBody::RefreshHost { host: "cd".repeat(32) },
        RequestBody::RemoteDownload {
            host: "cd".repeat(32),
            item: merc_ipc::remote::RemoteItem::World {
                id: id(),
                name: "world".into(),
            },
            dest: r"C:\Users\ejemplo\Downloads\world.zip".into(),
        },
        RequestBody::RemoteUpload {
            host: "cd".repeat(32),
            id: id(),
            source: r"C:\Users\ejemplo\Downloads\mod.jar".into(),
            dest_dir: "mods".into(),
        },
        RequestBody::DownloadBackup {
            id: id(),
            backup_id: "20261030-153000".into(),
            dest: r"C:\Users\ejemplo\Downloads\copia.zip".into(),
        },
        RequestBody::RemoteCall {
            host: "cd".repeat(32),
            request: Box::new(RequestBody::Status { id: id() }),
        },
        RequestBody::Health { id: id() },
        RequestBody::MeasureTps { id: id() },
        RequestBody::Alerts,
        RequestBody::MarkAlertsRead {
            ids: vec!["0a1b2c3d4e5f".into()],
        },
        RequestBody::SetAlertSettings {
            settings: AlertSettings::default(),
        },
        RequestBody::SaveTask {
            id: id(),
            task: TaskDraft {
                id: None,
                enabled: true,
                action: TaskAction::Restart {
                    notice_seconds: 300,
                    if_players: PlayersPolicy::Notify,
                },
                schedule: Schedule::Daily {
                    times: vec!["04:00".into()],
                    days: vec![1, 3, 5],
                },
            },
        },
        RequestBody::DeleteTask {
            id: id(),
            task_id: "a1b2c3d4e5f6".into(),
        },
        RequestBody::RunTaskNow {
            id: id(),
            task_id: "a1b2c3d4e5f6".into(),
        },
        RequestBody::ContentPlatforms { id: id() },
        RequestBody::ModpackSearch {
            source: ContentSource::Curseforge,
            query: "aventura".into(),
            sort: SearchSort::Downloads,
            offset: 0,
        },
        RequestBody::ResourcePackSearch {
            id: id(),
            query: "faithful".into(),
            sort: SearchSort::Downloads,
            offset: 0,
        },
        RequestBody::ResourcePackProject {
            id: id(),
            project: "faithful-32x".into(),
        },
        RequestBody::ModpackProject {
            source: ContentSource::Modrinth,
            project: "1KVo5zza".into(),
        },
        RequestBody::ModpackInspect {
            source: ContentSource::Modrinth,
            version_id: "packv1".into(),
        },
        RequestBody::ContentSearch {
            id: id(),
            kind: ContentKind::Mod,
            source: ContentSource::Modrinth,
            query: "mapa".into(),
            sort: SearchSort::Downloads,
            offset: 20,
        },
        RequestBody::ContentProject {
            id: id(),
            kind: ContentKind::Mod,
            source: ContentSource::Modrinth,
            project: "AANobbMI".into(),
        },
        RequestBody::ContentPlan {
            id: id(),
            kind: ContentKind::Mod,
            source: ContentSource::Modrinth,
            version_id: "u6dRKJwZ".into(),
        },
        RequestBody::ContentInstall {
            id: id(),
            kind: ContentKind::Plugin,
            source: ContentSource::Hangar,
            version_id: "1234".into(),
        },
        RequestBody::ContentInstalled {
            id: id(),
            kind: ContentKind::Plugin,
        },
        RequestBody::ContentToggle {
            id: id(),
            kind: ContentKind::Mod,
            file_name: "sodium.jar".into(),
            enabled: false,
        },
        RequestBody::ContentRemove {
            id: id(),
            kind: ContentKind::Datapack,
            file_name: "terreno.zip.disabled".into(),
        },
        RequestBody::Shutdown,
    ];
    let responses = vec![
        ResponseBody::Pong {
            server_version: "0.1.0".into(),
            supervisor_pid: 1234,
            protocol_id: "0123456789abcdef".into(),
        },
        ResponseBody::Ok,
        ResponseBody::Instances(vec![
            InstanceSummary {
                instance: instance(),
                status: status_online(),
                eula_accepted: true,
                configured_port: Some(25565),
                archived: false,
            },
            InstanceSummary {
                instance: Instance {
                    kind: ServerKind::Paper,
                    launch: LaunchProfile {
                        target: LaunchTarget::Jar {
                            jar: "paper-1.21.1-130.jar".into(),
                        },
                        ..instance().launch
                    },
                    ..instance()
                },
                status: status_crashed(),
                eula_accepted: false,
                configured_port: None,
                archived: false,
            },
            InstanceSummary {
                instance: Instance {
                    kind: ServerKind::Bedrock,
                    mc_version: Some("1.21.124.2".into()),
                    loader_version: None,
                    launch: LaunchProfile {
                        java: "".into(),
                        target: LaunchTarget::Native {
                            exe: "bedrock_server.exe".into(),
                        },
                        ..instance().launch
                    },
                    ..instance()
                },
                status: status_crashed(),
                eula_accepted: true,
                configured_port: Some(19132),
                archived: false,
            },
        ]),
        ResponseBody::Instance(instance()),
        ResponseBody::Status(status_online()),
        ResponseBody::Detected(DetectedServer {
            kind: ServerKind::NeoForge,
            mc_version: Some("1.21.1".into()),
            loader_version: Some("21.1.259".into()),
            launch: Some(instance().launch.target),
            confidence: Confidence::Verified,
            evidence: vec!["run.bat apunta a libraries/net/neoforged/neoforge/21.1.259/win_args.txt".into()],
            warnings: vec![],
        }),
        ResponseBody::JavaCandidates(vec![
            JavaCandidate {
                path: r"C:\Java21\bin\java.exe".into(),
                major: Some(21),
                description: "openjdk version \"21.0.13\"".into(),
                error: None,
                jdk: true,
            },
            JavaCandidate {
                path: r"C:\Roto\java.exe".into(),
                major: None,
                description: String::new(),
                error: Some("Java no respondió a tiempo".into()),
                jdk: false,
            },
        ]),
        ResponseBody::Console {
            lines: vec![
                ConsoleLine {
                    seq: 11,
                    at_unix_ms: 1,
                    stream: ConsoleStream::Stdout,
                    text: "[Server thread/INFO]: Done (3.440s)! For help, type \"help\"".into(),
                },
                ConsoleLine {
                    seq: 12,
                    at_unix_ms: 2,
                    stream: ConsoleStream::Stderr,
                    text: "WARN algo".into(),
                },
                ConsoleLine {
                    seq: 13,
                    at_unix_ms: 3,
                    stream: ConsoleStream::System,
                    text: "> say hola".into(),
                },
            ],
            next_after: 13,
        },
        ResponseBody::Metrics {
            samples: vec![
                MetricSample {
                    at_unix_ms: 1_000,
                    cpu_percent: 12.5,
                    memory_bytes: 2_147_483_648,
                },
                MetricSample {
                    at_unix_ms: 3_000,
                    cpu_percent: 9.25,
                    memory_bytes: 2_200_000_000,
                },
            ],
            players: vec!["Alex".into(), "Steve".into()],
            max_players: Some(20),
        },
        ResponseBody::System {
            total_memory_bytes: Some(17_179_869_184),
            logical_cpus: 12,
            lan_ipv4: Some("192.168.1.20".into()),
        },
        ResponseBody::Catalog(vec![
            CatalogEntry {
                mc_version: "26.1.2".into(),
                java_major: 25,
                latest: true,
            },
            CatalogEntry {
                mc_version: "1.21.11".into(),
                java_major: 21,
                latest: false,
            },
        ]),
        ResponseBody::Resolved(Resolved {
            flavor: Flavor::Paper,
            mc_version: "26.1.2".into(),
            loader_version: Some("14".into()),
            java_major: 25,
            experimental: false,
            artifact: Artifact {
                url: "https://descarga.ejemplo.invalid/paper-26.1.2-14.jar".into(),
                mirrors: vec![],
                file_name: "paper-26.1.2-14.jar".into(),
                checksum: Checksum::Sha256("0".repeat(64)),
                size: Some(52_428_800),
            },
            installer_args: None,
            installer_in_cache: false,
            needs_jdk: false,
            notes: vec![],
        }),
        ResponseBody::CreationDefaults {
            servers_dir: r"C:\Users\ejemplo\MercHosting\Servidores".into(),
            suggested_port: 25566,
            curseforge: false,
        },
        ResponseBody::Job(JobSnapshot {
            job_id: "6b1f0c1e-2a3b-4c5d-8e9f-0a1b2c3d4e5f".into(),
            name: "Mi servidor".into(),
            state: JobState::Running,
            step: Some(Step::Downloading),
            message: "Descargando paper-26.1.2-14.jar".into(),
            download: Some(JobDownload {
                file: "paper-26.1.2-14.jar".into(),
                done: 10_485_760,
                total: Some(52_428_800),
            }),
            log: vec![JobLogLine {
                seq: 13,
                text: "▸ Descargando paper-26.1.2-14.jar".into(),
            }],
            next_after: 13,
            instance_id: None,
            error: None,
        }),
        ResponseBody::Properties {
            exists: true,
            entries: vec![
                PropEntry {
                    key: "motd".into(),
                    value: "Servidor de Ñandú".into(),
                },
                PropEntry {
                    key: "max-players".into(),
                    value: "10".into(),
                },
            ],
            modified_unix_ms: Some(1_793_300_000_000),
        },
        ResponseBody::Players {
            lists: PlayerLists {
                whitelist: vec![ListedPlayer {
                    name: "Alex".into(),
                    uuid: Some("ec561538-f3fd-461d-aff5-086b22154bce".into()),
                    ..Default::default()
                }],
                ops: vec![ListedPlayer {
                    name: "Steve".into(),
                    uuid: Some("8667ba71-b85a-4004-af54-457a9734eed7".into()),
                    level: Some(4),
                    ..Default::default()
                }],
                banned: vec![ListedPlayer {
                    name: "Griefer".into(),
                    uuid: None,
                    reason: Some("Romper casas".into()),
                    expires: Some("forever".into()),
                    ..Default::default()
                }],
                banned_ips: vec![],
                warnings: vec![],
            },
            online: vec!["Alex".into()],
            can_act: true,
        },
        ResponseBody::Files {
            path: "".into(),
            entries: vec![
                FileEntry {
                    name: "world".into(),
                    is_dir: true,
                    size: None,
                    modified_unix_ms: Some(1_793_300_000_000),
                    link: false,
                },
                FileEntry {
                    name: "server.properties".into(),
                    is_dir: false,
                    size: Some(1234),
                    modified_unix_ms: Some(1_793_300_000_000),
                    link: false,
                },
            ],
        },
        ResponseBody::TextFile(TextFile {
            path: "server.properties".into(),
            content: "motd=Hola\n".into(),
            size: 10,
            modified_unix_ms: Some(1_793_300_000_000),
        }),
        ResponseBody::FileDone {
            path: "mods/viejo.jar.desactivado".into(),
        },
        ResponseBody::ZipExtracted(ZipExtracted {
            folder: "descargas/mapa".into(),
            files: 412,
            bytes: 52_428_800,
            skipped: vec!["../fuera.txt".into()],
        }),
        ResponseBody::Worlds {
            worlds: vec![WorldInfo {
                name: "world".into(),
                size_bytes: 104_857_600,
                modified_unix_ms: Some(1_793_300_000_000),
                active: true,
                companions: vec!["world_nether".into(), "world_the_end".into()],
            }],
            level_name: "world".into(),
        },
        ResponseBody::SavedItems(vec![SavedItem {
            id: "papelera/20261030-041500-123".into(),
            kind: SavedKind::Trash,
            path: "mods/roto.jar".into(),
            is_dir: false,
            size_bytes: 52_000,
            saved_unix_ms: Some(1_793_300_000_000),
        }]),
        ResponseBody::GameRules {
            rules: vec![
                RuleValue {
                    key: "keep_inventory".into(),
                    name: "keep_inventory".into(),
                    kind: RuleKind::Bool,
                    value: Some("false".into()),
                },
                RuleValue {
                    key: "random_tick_speed".into(),
                    name: "random_tick_speed".into(),
                    kind: RuleKind::Int { min: 0, max: 4096 },
                    value: None,
                },
            ],
            new_names: true,
        },
        ResponseBody::Activity(vec![ActivityEntry {
            at_unix: 1_793_300_000,
            actor: "local:merchosting-app".into(),
            action: "world.gamerule".into(),
            detail: "gamerule keep_inventory true".into(),
        }]),
        ResponseBody::Playit(Box::new(PlayitInfo {
            supported: true,
            enabled: true,
            installed: true,
            agent_version: "1.2.0".into(),
            running: true,
            agent: Some(AgentStatus {
                phase: AgentPhase::Running,
                message: None,
                version: Some("1.2.0".into()),
                account_status: Some("verified".into()),
                tunnels: vec![Tunnel {
                    display_address: "dark-sea.joinmc.link".into(),
                    destination: "127.0.0.1:25565".into(),
                    is_disabled: false,
                    disabled_reason: None,
                }],
                pending: vec![],
                notices: vec![Notice {
                    priority: "High".into(),
                    message: "Verifica tu correo".into(),
                    resolve_link: Some("https://playit.gg/account".into()),
                }],
            }),
            agent_error: None,
            claim: Some(ClaimInfo {
                url: "https://playit.gg/claim/ab01ff1000".into(),
                state: ClaimState::WaitingForApproval,
                done: false,
                error: None,
            }),
        })),
        ResponseBody::PlayitLoginUrl("https://playit.gg/login/guest-account/abc".into()),
        ResponseBody::Connection(Box::new(ConnectionInfo {
            port: Some(25565),
            lan: vec!["192.168.1.20:25565".into()],
            tunnels: vec![Tunnel {
                display_address: "dark-sea.joinmc.link".into(),
                destination: "127.0.0.1:25565".into(),
                is_disabled: false,
                disabled_reason: None,
            }],
            tests: vec![AddressTest {
                address: "dark-sea.joinmc.link".into(),
                at_unix_ms: 1_793_300_300_000,
                result: Some(PingResult {
                    tested: "dark-sea.gl.at.ply.gg:41234".into(),
                    latency_ms: 48,
                    version: "Paper 1.21.1".into(),
                    players_online: 2,
                    players_max: 20,
                    motd: "Survival de Amir".into(),
                }),
                error: None,
            }],
            ports_in_use: vec![PortUse {
                port: 25566,
                server: "Creativo".into(),
            }],
            voice: Some(VoiceInfo {
                installed: true,
                enabled: true,
                jar: Some("mods/voicechat-fabric-1.21.1-2.5.32.jar".into()),
                config: Some("config/voicechat/voicechat-server.properties".into()),
                port_setting: Some(24454),
                port: Some(24454),
                voice_host: String::new(),
                bind_address: String::new(),
                tunnels: vec![Tunnel {
                    display_address: "147.185.221.181:25732".into(),
                    destination: "127.0.0.1:24454".into(),
                    is_disabled: false,
                    disabled_reason: None,
                }],
                suggested_port: None,
                listening: Some(24454),
                restart_needed: false,
                problems: vec![VoiceProblem {
                    code: "host_missing".into(),
                    level: "error".into(),
                    message: "«voice_host» está vacío".into(),
                }],
            }),
            layers: vec![ConnLayer {
                key: "voice".into(),
                state: LayerState::Degradado,
                detail: "Escuchando en el puerto UDP 24454.".into(),
                checked_at_unix_ms: 1_793_300_300_000,
            }],
            bedrock: Some(BedrockInfo {
                geyser: "plugins/Geyser-Spigot.jar".into(),
                geyser_enabled: true,
                floodgate: Some("plugins/floodgate-spigot.jar".into()),
                floodgate_enabled: true,
                config: Some("plugins/Geyser-Spigot/config.yml".into()),
                port: 19132,
                port_from_config: true,
                lan: vec!["192.168.1.20:19132".into()],
                tunnels: vec![Tunnel {
                    display_address: "147.185.221.20:19132".into(),
                    destination: "127.0.0.1:19132".into(),
                    is_disabled: false,
                    disabled_reason: None,
                }],
                tests: vec![AddressTest {
                    address: "147.185.221.20:19132".into(),
                    at_unix_ms: 1_793_300_300_000,
                    result: Some(PingResult {
                        tested: "147.185.221.20:19132".into(),
                        latency_ms: 52,
                        version: "Bedrock 1.21.40".into(),
                        players_online: 1,
                        players_max: 20,
                        motd: "Survival de Amir".into(),
                    }),
                    error: None,
                }],
            }),
        })),
        ResponseBody::SecurityReport(Box::new(merc_admin::security::SecurityReport {
            checked_at_unix_ms: 1_793_300_300_000,
            public_addresses: vec!["dark-sea.joinmc.link".into()],
            checks: vec![
                merc_admin::security::SecurityCheck {
                    code: "whitelist".into(),
                    level: merc_admin::security::Level::Aviso,
                    title: "Cualquiera con la dirección puede entrar".into(),
                    detail: "Activa la lista blanca.".into(),
                    action: Some(merc_admin::security::SecurityAction::SetProperty {
                        key: "white-list".into(),
                        value: "true".into(),
                    }),
                },
                merc_admin::security::SecurityCheck {
                    code: "companion".into(),
                    level: merc_admin::security::Level::Info,
                    title: "Shandé puede administrar este servidor".into(),
                    detail: "Sin permisos de alto riesgo.".into(),
                    action: Some(merc_admin::security::SecurityAction::OpenCompanions),
                },
                merc_admin::security::SecurityCheck {
                    code: "backups".into(),
                    level: merc_admin::security::Level::Bien,
                    title: "Hay una copia de hoy".into(),
                    detail: "".into(),
                    action: Some(merc_admin::security::SecurityAction::OpenTab { tab: "backups".into() }),
                },
            ],
        })),
        ResponseBody::ServerIcon(merc_ipc::ServerIconInfo {
            present: true,
            valid: true,
            width: Some(64),
            height: Some(64),
            png_base64: Some("iVBORw0KGgo=".into()),
        }),
        ResponseBody::PlayerHistory(vec![merc_admin::players::PlayerRecord {
            name: "Alex".into(),
            first_seen_unix: 1_793_300_000,
            last_seen_unix: 1_793_303_600,
            sessions: 3,
            total_secs: 5400,
            online_since_unix: Some(1_793_303_600),
            note: Some("Construyó el castillo".into()),
            xuid: None,
        }]),
        ResponseBody::LuckPerms(Box::new(merc_ipc::LuckPermsInfo {
            install: Some(merc_admin::luckperms::LpInstall {
                jar: "LuckPerms-Bukkit-5.5.17.jar".into(),
                data_dir: "plugins/LuckPerms".into(),
            }),
            online: true,
            snapshot: Some(merc_admin::luckperms::LpSnapshot {
                generated_at: Some("2026-10-30 12:00:00 UTC".into()),
                groups: vec![merc_admin::luckperms::LpGroup {
                    name: "vip".into(),
                    weight: Some(5),
                    prefix: Some("&6[VIP] ".into()),
                    parents: vec!["default".into()],
                    nodes: vec![merc_admin::luckperms::LpNode {
                        kind: "permission".into(),
                        key: "essentials.fly".into(),
                        value: true,
                        expiry: None,
                        context: Default::default(),
                    }],
                }],
                users: vec![merc_admin::luckperms::LpUser {
                    uuid: "069a79f4-44e9-4726-a5be-fca90e38aaf5".into(),
                    username: Some("Notch".into()),
                    primary_group: "vip".into(),
                    parents: vec!["vip".into()],
                    nodes: vec![],
                }],
                tracks: vec![merc_admin::luckperms::LpTrack {
                    name: "rangos".into(),
                    groups: vec!["default".into(), "vip".into()],
                }],
            }),
            backups: vec!["2026-10-30-12-00-00.json.gz".into()],
        })),
        ResponseBody::BedrockPlayers(Box::new(BedrockPlayersInfo {
            allow_list_enabled: true,
            online_mode: true,
            default_permission: "member".into(),
            legacy_whitelist: false,
            running: true,
            allowlist: vec![merc_admin::bedrock_players::AllowEntry {
                name: "Steve Craft".into(),
                xuid: Some("2535428000000000".into()),
                ignores_player_limit: false,
            }],
            permissions: vec![BedrockPermRow {
                xuid: "2535428000000000".into(),
                level: merc_admin::bedrock_players::Level::Operator,
                name: Some("Steve Craft".into()),
            }],
            known: vec![BedrockKnownPlayer {
                name: "Steve Craft".into(),
                xuid: "2535428000000000".into(),
                online: true,
                last_seen_unix: 1_793_303_600,
            }],
            online: vec!["Steve Craft".into()],
        })),
        ResponseBody::Addons(Box::new(AddonsInfo {
            level_name: "Bedrock level".into(),
            world_exists: true,
            packs: vec![merc_admin::addons::AddonPack {
                uuid: "11111111-2222-3333-4444-555555555555".into(),
                version: [1, 1, 0],
                name: "Dragones".into(),
                description: "Dragones que vuelan".into(),
                kind: merc_admin::addons::PackKind::Behavior,
                folder: "behavior_packs/Dragones_11111111".into(),
                active: true,
                bundled: false,
                scripts: true,
            }],
            last_import: Some(merc_admin::addons::ImportResult {
                installed: vec!["Dragones 1.1.0".into()],
                unchanged: vec![],
                replaced: vec![],
                notes: vec!["«Dragones» usa scripts.".into()],
            }),
        })),
        ResponseBody::ModSides(Box::new(merc_admin::modsides::ModSidesInfo {
            items: vec![merc_admin::modsides::ModSideReport {
                file: "zoomify-2.14.jar".into(),
                name: "Zoomify".into(),
                enabled: true,
                side: merc_admin::modsides::ModSide::ClientOnly,
                confidence: merc_admin::modsides::SideConfidence::Alta,
                evidence: vec!["Modrinth: el autor indica que no funciona en servidores.".into()],
                needed_by: vec![],
                suggested: true,
            }],
            quarantined: vec![merc_admin::modsides::QuarantinedMod {
                file: "xaeros-minimap.jar".into(),
                moved_at_unix: 1_793_300_000,
                reason: "Solo cliente".into(),
            }],
            folder: "mods-cuarentena".into(),
            catalog_ok: true,
        })),
        ResponseBody::FriendPack(Box::new(merc_content::friendpack::FriendPackPlan {
            mc_version: Some("1.21.1".into()),
            loader: Some(("neoforge".into(), "21.1.259".into())),
            included: vec![merc_content::friendpack::PackMod {
                title: "Sodium".into(),
                file_name: "sodium-0.6.jar".into(),
                optional: false,
                page_url: Some("https://modrinth.com/mod/sodium".into()),
                size: 1234,
            }],
            manual: vec![merc_content::friendpack::ManualMod {
                title: "JEI".into(),
                file_name: "jei.jar".into(),
                page_url: Some("https://www.curseforge.com/minecraft/mc-mods/jei".into()),
                reason: "Es de CurseForge: el formato .mrpack no admite sus descargas. Bájalo de su página.".into(),
            }],
            server_only: vec!["Lithium".into()],
            blocked: None,
        })),
        ResponseBody::LuckPermsPreview(merc_ipc::LuckPermsPreview {
            command: "luckperms group vip permission set essentials.fly true".into(),
            summary: "Dar el permiso essentials.fly al grupo «vip».".into(),
        }),
        ResponseBody::LuckPermsApplied(Box::new(merc_ipc::LuckPermsApplied {
            verified: true,
            command: "luckperms group vip permission set essentials.fly true".into(),
            backup: "2026-10-30-12-00-00.json.gz".into(),
            snapshot: merc_admin::luckperms::LpSnapshot {
                generated_at: Some("2026-10-30 12:00:00 UTC".into()),
                groups: vec![merc_admin::luckperms::LpGroup {
                    name: "vip".into(),
                    weight: Some(5),
                    prefix: Some("&6[VIP] ".into()),
                    parents: vec!["default".into()],
                    nodes: vec![merc_admin::luckperms::LpNode {
                        kind: "permission".into(),
                        key: "essentials.fly".into(),
                        value: true,
                        expiry: None,
                        context: Default::default(),
                    }],
                }],
                users: vec![merc_admin::luckperms::LpUser {
                    uuid: "069a79f4-44e9-4726-a5be-fca90e38aaf5".into(),
                    username: Some("Notch".into()),
                    primary_group: "vip".into(),
                    parents: vec!["vip".into()],
                    nodes: vec![],
                }],
                tracks: vec![merc_admin::luckperms::LpTrack {
                    name: "rangos".into(),
                    groups: vec!["default".into(), "vip".into()],
                }],
            },
        })),
        ResponseBody::Automation(merc_ipc::AutomationInfo {
            auto_restart: true,
            max_restarts: 3,
            recent_restarts: 1,
            gave_up: false,
            autostart: false,
            startup_available: true,
            startup_enabled: false,
            keep_awake: true,
            keep_awake_active: true,
        }),
        ResponseBody::LoaderVersions(vec![merc_install::LoaderOption {
            version: "0.16.10".into(),
            stable: true,
            recommended: true,
        }]),
        ResponseBody::Supporter(merc_ipc::SupporterInfo {
            available: true,
            checkout_url: Some("https://example.gumroad.com/l/merchosting".into()),
            price_usd: 8,
            active: true,
            email_hint: Some("a•••@gmail.com".into()),
            key_hint: Some("…3456".into()),
            purchased: Some("2026-10-30".into()),
            checked_at_unix_ms: Some(1_793_300_000_000),
            stale: false,
            revoked_reason: None,
            last_error: None,
        }),
        ResponseBody::IconPreview(merc_ipc::IconPreview {
            png_base64: "iVBORw0KGgo=".into(),
            source_width: 1920,
            source_height: 1080,
            format: "JPG".into(),
        }),
        ResponseBody::ServerUpdate(Box::new(merc_ipc::ServerUpdateInfo {
            supported: true,
            reason: None,
            mc_version: Some("1.21.4".into()),
            current: Some("98".into()),
            latest: Some("130".into()),
            newer: true,
            experimental: false,
            notes: vec![],
        })),
        ResponseBody::ModpackStatus(Box::new(merc_ipc::ModpackStatusInfo {
            linked: Some(merc_content::packupdate::LinkedPack {
                source: ContentSource::Modrinth,
                project_id: "PACK".into(),
                title: "Aventura".into(),
                version_id: "packv1".into(),
                version_number: "1.0".into(),
                mc_version: "1.21.1".into(),
                loader: Some("neoforge".into()),
                loader_version: Some("21.1.77".into()),
                installed_at_unix: 1_793_300_000,
                files: 212,
            }),
            online: true,
            error: None,
            versions: vec![merc_ipc::ModpackVersionChoice {
                version_id: "packv2".into(),
                version_number: "2.0".into(),
                name: "Aventura 2.0".into(),
                date_published: "2026-10-01T00:00:00Z".into(),
                game_versions: vec!["1.21.1".into()],
                compatible: true,
                current: false,
                newer: true,
            }],
        })),
        ResponseBody::ModpackUpdatePlan(Box::new(merc_ipc::ModpackUpdatePlan {
            from_version: "1.0".into(),
            to_version: "2.0".into(),
            to_version_id: "packv2".into(),
            mc_version: "1.21.1".into(),
            loader: Some("neoforge".into()),
            loader_from: Some("21.1.77".into()),
            loader_to: Some("21.1.80".into()),
            loader_mismatch: true,
            changes: vec![merc_content::packupdate::PlannedChange {
                path: "config/create-common.toml".into(),
                kind: merc_content::packupdate::ChangeKind::KeepModified,
                from_override: true,
            }],
            download_bytes: 52_428_800,
            notes: vec![],
        })),
        ResponseBody::Diagnosis(Box::new(merc_admin::diagnose::Diagnosis {
            checked_at_unix_ms: 1_793_300_300_000,
            sources: vec!["mods/ (42 archivos)".into(), "logs/latest.log (30/10 16:00)".into()],
            mods_scanned: 42,
            plugins_scanned: 0,
            findings: vec![merc_admin::diagnose::Finding {
                code: "duplicate".into(),
                title: "El mod «jei» está 2 veces".into(),
                confidence: merc_admin::diagnose::Confidence::Alta,
                explanation: "Deja solo uno.".into(),
                evidence: vec![
                    "mods/jei-1.jar (versión 19.1)".into(),
                    "mods/jei-2.jar (versión 19.2)".into(),
                ],
                source: "mods/".into(),
                actions: vec![
                    merc_admin::diagnose::FixAction::DisableContent {
                        kind: "mod".into(),
                        file: "jei-1.jar".into(),
                    },
                    merc_admin::diagnose::FixAction::SetPort { port: 25566 },
                    merc_admin::diagnose::FixAction::SetMemory { max_mb: 6144 },
                    merc_admin::diagnose::FixAction::ChooseJava { min_major: 21 },
                    merc_admin::diagnose::FixAction::SearchContent {
                        kind: "mod".into(),
                        query: "geckolib".into(),
                    },
                    merc_admin::diagnose::FixAction::OpenTab { tab: "backups".into() },
                ],
            }],
        })),
        ResponseBody::Companions(Box::new(merc_ipc::remote::CompanionsInfo {
            enabled: true,
            online: true,
            fingerprint: Some("ABCD-EFGH-IJKL".into()),
            error: None,
            invites: vec![merc_ipc::remote::InviteInfo {
                invite_id: "a1b2c3d4e5f6".into(),
                server: Some(id()),
                server_name: "Survival".into(),
                perms: vec![merc_ipc::remote::Perm::Status],
                created_at_unix_ms: 1_793_300_300_000,
                expires_at_unix_ms: 1_793_303_900_000,
                host_admin: false,
            }],
            companions: vec![merc_ipc::remote::CompanionInfo {
                companion: "ab".repeat(32),
                name: "Shandé".into(),
                fingerprint: "MNOP-QRST-UVWX".into(),
                added_at_unix_ms: 1_793_300_300_000,
                last_seen_unix_ms: Some(1_793_300_400_000),
                connected: true,
                servers: vec![merc_ipc::remote::RemoteServer {
                    id: id(),
                    name: "Survival".into(),
                    perms: vec![merc_ipc::remote::Perm::Status, merc_ipc::remote::Perm::Start],
                }],
                host_admin: false,
            }],
        })),
        ResponseBody::Invite(Box::new(merc_ipc::remote::NewInvite {
            code: "MH1-ABCDEF".into(),
            invite: merc_ipc::remote::InviteInfo {
                invite_id: "a1b2c3d4e5f6".into(),
                server: Some(id()),
                server_name: "Survival".into(),
                perms: vec![merc_ipc::remote::Perm::Status],
                created_at_unix_ms: 1_793_300_300_000,
                expires_at_unix_ms: 1_793_303_900_000,
                host_admin: false,
            },
        })),
        ResponseBody::RemoteHosts(vec![merc_ipc::remote::RemoteHost {
            host: "cd".repeat(32),
            name: "PC-DE-AMIR".into(),
            fingerprint: "ABCD-EFGH-IJKL".into(),
            servers: vec![merc_ipc::remote::RemoteServer {
                id: id(),
                name: "Survival".into(),
                perms: vec![merc_ipc::remote::Perm::Status],
            }],
            last_ok_unix_ms: Some(1_793_300_400_000),
            last_error: None,
            host_admin: true,
        }]),
        ResponseBody::Health(HealthReport {
            pc_cpu_percent: Some(23.5),
            pc_memory_used_bytes: Some(9_663_676_416),
            pc_memory_total_bytes: Some(17_179_869_184),
            disks: vec![DiskSpace {
                what: "server".into(),
                path: r"C:\MercHosting\Servidores\Survival".into(),
                free_bytes: 120_000_000_000,
                total_bytes: 500_000_000_000,
            }],
            max_memory_mb: 4096,
            tps_command: Some("tps".into()),
        }),
        ResponseBody::Tps(TpsReading {
            tps: vec![("1m".into(), 20.0), ("5m".into(), 19.9), ("15m".into(), 19.95)],
            mspt: None,
            command: "tps".into(),
            raw: "TPS from last 1m, 5m, 15m: 20.0, 19.9, 19.95".into(),
        }),
        ResponseBody::Alerts(AlertList {
            items: vec![Alert {
                id: "0a1b2c3d4e5f".into(),
                at_unix_ms: 1_793_300_200_000,
                level: AlertLevel::Critical,
                kind: AlertKind::Crash,
                instance_id: Some("11111111-1111-4111-8111-111111111111".into()),
                server_name: Some("Survival".into()),
                title: "«Survival» se cerró inesperadamente".into(),
                message: "Código de salida 1.".into(),
                read: false,
            }],
            unread: 1,
            settings: AlertSettings::default(),
        }),
        ResponseBody::Tasks(TaskList {
            items: vec![
                ScheduledTask {
                    id: "a1b2c3d4e5f6".into(),
                    enabled: true,
                    action: TaskAction::Backup {
                        scope: BackupScope::World,
                    },
                    schedule: Schedule::Daily {
                        times: vec!["04:00".into()],
                        days: vec![],
                    },
                    created_unix_ms: 1_793_300_000_000,
                    next_run_unix_ms: Some(1_793_350_800_000),
                    last_run: Some(TaskRun {
                        at_unix_ms: 1_793_264_400_000,
                        outcome: RunOutcome::Ok,
                        message: "Copia hecha y comprobada (812 archivos).".into(),
                        manual: false,
                    }),
                    running: false,
                },
                ScheduledTask {
                    id: "f6e5d4c3b2a1".into(),
                    enabled: false,
                    action: TaskAction::Announce {
                        message: "¡Evento a las 20:00!".into(),
                    },
                    schedule: Schedule::Every { minutes: 90 },
                    created_unix_ms: 1_793_300_000_000,
                    next_run_unix_ms: None,
                    last_run: None,
                    running: false,
                },
            ],
            now_unix_ms: 1_793_300_100_000,
            utc_offset_minutes: -300,
        }),
        ResponseBody::Backups(BackupList {
            items: vec![BackupInfo {
                id: "20261030-131500-123-mundo".into(),
                created_unix_ms: 1_793_300_100_000,
                scope: BackupScope::World,
                reason: BackupReason::Manual,
                world: Some("world".into()),
                mc_version: Some("1.21.1".into()),
                size_bytes: 41_943_040,
                content_bytes: 62_914_560,
                files: 812,
                hot: true,
                consistent: true,
                verified: Some(true),
                verified_unix_ms: Some(1_793_300_160_000),
                problems: vec![],
                protected: false,
                note: Some("Antes del evento".into()),
            }],
            keep: 10,
            free_bytes: Some(120_000_000_000),
            folder: r"C:\Users\amir\AppData\Local\MercHosting\instancias\x\backups".into(),
            restored: None,
        }),
        ResponseBody::AdminState(AdminState {
            maintenance: Some(Maintenance {
                since_unix: 1_793_300_000,
                previous_whitelist: false,
            }),
            whitelist_on: true,
            enforce_whitelist: false,
            restart_in_seconds: Some(45),
            world_op: Some(WorldOp {
                kind: "export".into(),
                world: "world".into(),
                done_bytes: 52_428_800,
                total_bytes: 125_829_120,
            }),
        }),
        ResponseBody::ModpackSummary(ModpackSummary {
            source: ContentSource::Modrinth,
            flavor: Flavor::NeoForge,
            name: "Aventura".into(),
            pack_version: "1.0".into(),
            mc_version: "1.21.1".into(),
            loader: Some("neoforge".into()),
            loader_version: Some("21.1.77".into()),
            java_major: 21,
            server_files: 180,
            client_only_files: 12,
            download_bytes: 450_000_000,
            warnings: vec!["Aviso".into()],
        }),
        ResponseBody::ContentPlatforms(vec![Platform {
            kind: ContentKind::Mod,
            folder: "mods".into(),
            loaders: vec!["neoforge".into()],
            experimental: false,
            sources: vec![ContentSource::Modrinth, ContentSource::Curseforge],
        }]),
        ResponseBody::ContentSearch(SearchPage {
            hits: vec![ContentHit {
                project_id: "AANobbMI".into(),
                slug: "ejemplo".into(),
                title: "Ejemplo".into(),
                author: "autora".into(),
                description: "Un mod de ejemplo".into(),
                icon_url: Some("https://cdn.modrinth.com/data/AANobbMI/icon.png".into()),
                downloads: 1000,
                categories: vec!["optimization".into()],
                client_side: Side::Optional,
                server_side: Side::Required,
                date_modified: "2026-10-01T00:00:00Z".into(),
                license: "MIT".into(),
            }],
            total: 1,
            offset: 0,
        }),
        ResponseBody::ContentDetails {
            project: Box::new(ContentProject {
                id: "AANobbMI".into(),
                slug: "ejemplo".into(),
                title: "Ejemplo".into(),
                description: "Un mod de ejemplo".into(),
                body: "# Ejemplo".into(),
                icon_url: None,
                client_side: Side::Optional,
                server_side: Side::Required,
                license: "MIT".into(),
                downloads: 1000,
                categories: vec!["optimization".into()],
                source_url: Some("https://github.com/ejemplo/ejemplo".into()),
                issues_url: None,
                wiki_url: None,
                page_url: "https://modrinth.com/project/ejemplo".into(),
                distribution_allowed: true,
            }),
            versions: vec![ContentVersion {
                id: "u6dRKJwZ".into(),
                project_id: "AANobbMI".into(),
                name: "Ejemplo 1.0".into(),
                version_number: "1.0.0".into(),
                version_type: "release".into(),
                game_versions: vec!["1.21.1".into()],
                loaders: vec!["neoforge".into()],
                date_published: "2026-10-01T00:00:00Z".into(),
                downloads: 10,
                changelog: Some("Cambios".into()),
                dependencies: vec![Dependency {
                    version_id: None,
                    project_id: Some("P7dR8mSH".into()),
                    file_name: None,
                    dependency_type: DependencyType::Required,
                }],
                files: vec![VersionFile {
                    url: "https://cdn.modrinth.com/data/AANobbMI/versions/u6dRKJwZ/ejemplo-1.0.0.jar".into(),
                    filename: "ejemplo-1.0.0.jar".into(),
                    primary: true,
                    size: 123_456,
                    sha1: Some("0123456789abcdef0123456789abcdef01234567".into()),
                    sha512: None,
                    sha256: None,
                }],
                server_side: Side::Unknown,
            }],
        },
        ResponseBody::ContentPlan(ContentPlan {
            items: vec![PlanItem {
                project_id: "AANobbMI".into(),
                title: "Ejemplo".into(),
                icon_url: None,
                version_id: "u6dRKJwZ".into(),
                version_number: "1.0.0".into(),
                file_name: "ejemplo-1.0.0.jar".into(),
                url: "https://cdn.modrinth.com/data/AANobbMI/versions/u6dRKJwZ/ejemplo-1.0.0.jar".into(),
                sha1: "0123456789abcdef0123456789abcdef01234567".into(),
                sha256: None,
                size: 123_456,
                reason: "Lo elegiste tú".into(),
                replaces: Some("ejemplo-0.9.jar".into()),
            }],
            already_installed: vec!["Librería".into()],
            optional: vec![OptionalDep {
                project_id: "P7dR8mSH".into(),
                title: "Extra".into(),
                needed_by: "Ejemplo".into(),
            }],
            warnings: vec!["Aviso".into()],
            blockers: vec![],
            download_bytes: 123_456,
        }),
        ResponseBody::ContentList(ContentList {
            platform: Platform {
                kind: ContentKind::Plugin,
                folder: "plugins".into(),
                loaders: vec!["paper".into(), "spigot".into(), "bukkit".into()],
                experimental: false,
                sources: vec![ContentSource::Modrinth],
            },
            items: vec![InstalledItem {
                file_name: "ejemplo-1.0.0.jar".into(),
                enabled: true,
                size: 123_456,
                sha1: "0123456789abcdef0123456789abcdef01234567".into(),
                project_id: Some("AANobbMI".into()),
                version_id: Some("u6dRKJwZ".into()),
                title: Some("Ejemplo".into()),
                version_number: Some("1.0.0".into()),
                icon_url: None,
                server_side: Some(Side::Required),
                client_side: None,
                page_url: None,
                update: Some(UpdateInfo {
                    version_id: "zzzzzzzz".into(),
                    version_number: "1.1.0".into(),
                }),
                source: Some(ContentSource::Modrinth),
                other_ids: vec!["238222".into()],
            }],
            online: true,
            offline_reason: None,
        }),
    ];
    let errors = vec![
        Response {
            seq: 9,
            result: ResponseResult::Err(ApiError::new(ErrorCode::Conflict, "Ya está en marcha")),
        },
        Response {
            seq: 10,
            result: ResponseResult::Err(ApiError::new(ErrorCode::NoConsole, "Sin consola")),
        },
    ];
    json!({
        "protocol": PROTOCOL_VERSION,
        "force_kill_confirmation": FORCE_KILL_CONFIRMATION,
        "requests": reqs,
        "responses": responses,
        "error_responses": errors,
        "all_states": [
            RuntimeState::Stopped, RuntimeState::Validating, RuntimeState::Starting, RuntimeState::Loading,
            RuntimeState::Online, RuntimeState::Stopping, RuntimeState::Crashed, RuntimeState::Error,
            RuntimeState::Recovering, RuntimeState::Unknown
        ],
        "all_error_codes": [
            ErrorCode::AuthFailed, ErrorCode::VersionMismatch, ErrorCode::BadRequest, ErrorCode::NotFound,
            ErrorCode::Conflict, ErrorCode::InvalidState, ErrorCode::NoConsole, ErrorCode::Network,
            ErrorCode::Forbidden, ErrorCode::Internal
        ],
    })
}

#[test]
fn contract_file_matches_rust_types() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/contratos/ipc-v1-ejemplos.json");
    let generated = serde_json::to_string_pretty(&examples()).unwrap() + "\n";
    if std::env::var_os("MERC_UPDATE_CONTRACT").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &generated).unwrap();
    }
    let on_disk = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("falta {}; genera con MERC_UPDATE_CONTRACT=1", path.display()))
        .replace("\r\n", "\n");
    assert_eq!(
        on_disk, generated,
        "el contrato cambió: revisa la interfaz y regenera con MERC_UPDATE_CONTRACT=1"
    );

    // Y todo lo que está en el archivo se puede volver a leer con los tipos de Rust.
    let v: Value = serde_json::from_str(&on_disk).unwrap();
    for r in v["requests"].as_array().unwrap() {
        serde_json::from_value::<RequestBody>(r.clone()).unwrap();
    }
    for r in v["responses"].as_array().unwrap() {
        serde_json::from_value::<ResponseBody>(r.clone()).unwrap();
    }
}
