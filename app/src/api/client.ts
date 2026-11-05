// Interfaz que usa la UI para hablar con el escritorio. La implementación real está en tauri.ts;
// las pruebas usan una implementación en memoria (src/test/fakeApi.ts).

import type {
  AppUpdateCheck,
  ActivityEntry,
  AdminState,
  BackupList,
  BackupScope,
  TaskDraft,
  TaskList,
  AlertList,
  AlertSettings,
  HealthReport,
  TpsReading,
  PlayitInfo,
  ConnectionInfo,
  Diagnosis,
  IconFit,
  IconPreview,
  ServerIconInfo,
  ServerUpdateInfo,
  SupporterInfo,
  LoaderOption,
  AutomationInfo,
  PlayerCommand,
  PlayerRecord,
  LuckPermsInfo,
  FriendPackPlan,
  AddonsInfo,
  BedrockAllowChange,
  BedrockLevel,
  BedrockPlayersInfo,
  ModSidesInfo,
  ModpackStatusInfo,
  ModpackUpdatePlan,
  ZipExtracted,
  LuckPermsPreview,
  LuckPermsApplied,
  LpChange,
  SecurityReport,
  CompanionsInfo,
  NewInvite,
  Perm,
  RemoteHost,
  ContentKind,
  ContentList,
  ContentSource,
  ModpackSummary,
  ContentPlan,
  Platform,
  SearchPage,
  SearchSort,
  RuleValue,
  SavedItem,
  WorldInfo,
  FileEntry,
  PlayerAction,
  PlayerLists,
  PropEntry,
  TextFile,
  CatalogEntry,
  ConsoleLine,
  CreateServerArgs,
  Flavor,
  JobSnapshot,
  Resolved,
  DetectedServer,
  Instance,
  InstanceId,
  InstanceSummary,
  JavaChoice,
  Metrics,
  RequestBody,
  ResponseBody,
  StatusSnapshot,
  SupervisorInfo,
  SystemInfo,
} from "./types";
import { FORCE_KILL_CONFIRMATION, MercError } from "./types";

export interface MercApi {
  call(body: RequestBody): Promise<ResponseBody>;
  info(): Promise<SupervisorInfo>;
  javaFor(mcVersion: string | null): Promise<JavaChoice>;
  pickFolder(): Promise<string | null>;
  /** Elegir un archivo de la PC (opcionalmente solo de ciertas extensiones). */
  pickFile(filter?: { name: string; extensions: string[] }): Promise<string | null>;
  /** Elegir dónde guardar una copia (diálogo "Guardar como"). */
  pickSavePath(defaultName: string): Promise<string | null>;
  openUrl(url: string): Promise<void>;
  /** Abre en el Explorador la carpeta del servidor (o una subcarpeta, relativa a ella). */
  openServerFolder(id: InstanceId, rel: string): Promise<void>;
  /** Abre en el Explorador la carpeta de copias de seguridad del servidor. */
  openBackupsFolder(id: InstanceId): Promise<void>;
  /** Notificación de Windows (si la persona lo permite). Devuelve si se mostró. */
  notify(title: string, body: string): Promise<boolean>;
  /** Icono de la ventana y de la barra de tareas (SVG); null vuelve al oficial. */
  setWindowIcon(svg: string | null): Promise<void>;
  /** ¿Hay una versión nueva de MercHosting? */
  appUpdateCheck(): Promise<AppUpdateCheck>;
  /** Descarga, comprueba la firma e instala (la app se cierra para instalar). */
  appUpdateInstall(onProgress: (downloaded: number, total: number | null) => void): Promise<void>;
  /** Archivos de la PC arrastrados sobre la ventana. Devuelve la función para dejar de escuchar. */
  onFileDrop(handler: (e: FileDropEvent) => void): () => void;
  /** Se pueden traer archivos a este dispositivo y subirlos desde él (no en el celular). Sin valor = sí. */
  canTransfer?: boolean;
}

/** Arrastrar archivos desde el Explorador: «enter» al entrar, «drop» al soltar, «leave» al salir o cancelar. */
export type FileDropEvent = { type: "enter"; paths: string[] } | { type: "drop"; paths: string[] } | { type: "leave" };

function expect<K extends ResponseBody["kind"]>(r: ResponseBody, kind: K): Extract<ResponseBody, { kind: K }> {
  if (r.kind !== kind) {
    throw new MercError("internal", `Respuesta inesperada del supervisor (${r.kind}, se esperaba ${kind})`);
  }
  return r as Extract<ResponseBody, { kind: K }>;
}

/** Envoltorio tipado sobre MercApi.call. */
export class Supervisor {
  constructor(private api: MercApi) {}

  async list(): Promise<InstanceSummary[]> {
    return expect(await this.api.call({ op: "list_instances" }), "instances").data;
  }
  async detect(path: string): Promise<DetectedServer> {
    return expect(await this.api.call({ op: "detect", path }), "detected").data;
  }
  async importInstance(args: { name: string; path: string; java: string; maxMemoryMb: number }): Promise<Instance> {
    const r = await this.api.call({
      op: "import_instance",
      name: args.name,
      path: args.path,
      java: args.java,
      min_memory_mb: 0,
      max_memory_mb: args.maxMemoryMb,
    });
    return expect(r, "instance").data;
  }
  async start(id: InstanceId): Promise<StatusSnapshot> {
    return expect(await this.api.call({ op: "start", id }), "status").data;
  }
  async stop(id: InstanceId, timeoutSecs = 180): Promise<StatusSnapshot> {
    return expect(await this.api.call({ op: "stop", id, timeout_secs: timeoutSecs }), "status").data;
  }
  async restart(id: InstanceId): Promise<StatusSnapshot> {
    return expect(await this.api.call({ op: "restart", id, stop_timeout_secs: 180 }), "status").data;
  }
  async forceKill(id: InstanceId): Promise<void> {
    expect(await this.api.call({ op: "force_kill", id, confirm: FORCE_KILL_CONFIRMATION }), "ok");
  }
  async sendCommand(id: InstanceId, command: string): Promise<void> {
    expect(await this.api.call({ op: "send_command", id, command }), "ok");
  }
  async console(id: InstanceId, after: number): Promise<{ lines: ConsoleLine[]; next_after: number }> {
    return expect(await this.api.call({ op: "console", id, after, limit: 2000 }), "console").data;
  }
  async acceptEula(id: InstanceId): Promise<void> {
    expect(await this.api.call({ op: "accept_eula", id, accepted: true }), "ok");
  }
  async metrics(id: InstanceId): Promise<Metrics> {
    return expect(await this.api.call({ op: "metrics", id }), "metrics").data;
  }
  async systemInfo(): Promise<SystemInfo> {
    return expect(await this.api.call({ op: "system_info" }), "system").data;
  }
  async catalog(flavor: Flavor): Promise<CatalogEntry[]> {
    return expect(await this.api.call({ op: "catalog", flavor }), "catalog").data;
  }
  async resolve(flavor: Flavor, mcVersion: string, loaderVersion: string | null = null): Promise<Resolved> {
    return expect(
      await this.api.call({ op: "resolve", flavor, mc_version: mcVersion, loader_version: loaderVersion }),
      "resolved",
    ).data;
  }
  async modpackSearch(source: ContentSource, query: string, sort: SearchSort, offset = 0): Promise<SearchPage> {
    return expect(await this.api.call({ op: "modpack_search", source, query, sort, offset }), "content_search").data;
  }
  async resourcePackSearch(id: InstanceId, query: string, sort: SearchSort, offset = 0): Promise<SearchPage> {
    return expect(await this.api.call({ op: "resource_pack_search", id, query, sort, offset }), "content_search").data;
  }
  async resourcePackProject(id: InstanceId, project: string) {
    return expect(await this.api.call({ op: "resource_pack_project", id, project }), "content_details").data;
  }
  async modpackProject(source: ContentSource, project: string) {
    return expect(await this.api.call({ op: "modpack_project", source, project }), "content_details").data;
  }
  async modpackInspect(source: ContentSource, versionId: string): Promise<ModpackSummary> {
    return expect(await this.api.call({ op: "modpack_inspect", source, version_id: versionId }), "modpack_summary").data;
  }
  async creationDefaults(): Promise<{ servers_dir: string; suggested_port: number; curseforge: boolean }> {
    return expect(await this.api.call({ op: "creation_defaults" }), "creation_defaults").data;
  }
  async checkTarget(path: string): Promise<void> {
    expect(await this.api.call({ op: "check_target", path }), "ok");
  }
  async createServer(args: CreateServerArgs): Promise<JobSnapshot> {
    return expect(await this.api.call({ op: "create_server", ...args }), "job").data;
  }
  async job(jobId: string, after: number): Promise<JobSnapshot> {
    return expect(await this.api.call({ op: "job", job_id: jobId, after }), "job").data;
  }
  async cancelJob(jobId: string): Promise<void> {
    expect(await this.api.call({ op: "cancel_job", job_id: jobId }), "ok");
  }
  async properties(id: InstanceId): Promise<{ exists: boolean; entries: PropEntry[]; modified_unix_ms: number | null }> {
    return expect(await this.api.call({ op: "properties", id }), "properties").data;
  }
  /** El supervisor descarga el paquete, comprueba que es válido y escribe su dirección y SHA-1. */
  async setResourcePack(id: InstanceId, url: string, required: boolean) {
    return expect(await this.api.call({ op: "set_resource_pack", id, url, required }), "properties").data;
  }
  async setProperties(id: InstanceId, changes: Record<string, string>, expectedModified: number | null) {
    return expect(
      await this.api.call({ op: "set_properties", id, changes, expected_modified: expectedModified }),
      "properties",
    ).data;
  }
  async players(id: InstanceId): Promise<{ lists: PlayerLists; online: string[]; can_act: boolean }> {
    return expect(await this.api.call({ op: "players", id }), "players").data;
  }
  async playerAction(id: InstanceId, action: PlayerAction, name: string, reason: string | null = null): Promise<void> {
    expect(await this.api.call({ op: "player_action", id, action, name, reason }), "ok");
  }
  async listFiles(id: InstanceId, path: string): Promise<FileEntry[]> {
    return expect(await this.api.call({ op: "list_files", id, path }), "files").data.entries;
  }
  async readFile(id: InstanceId, path: string): Promise<TextFile> {
    return expect(await this.api.call({ op: "read_file", id, path }), "text_file").data;
  }
  async writeFile(id: InstanceId, path: string, content: string, expectedModified: number | null): Promise<TextFile> {
    return expect(
      await this.api.call({ op: "write_file", id, path, content, expected_modified: expectedModified }),
      "text_file",
    ).data;
  }
  async makeDir(id: InstanceId, path: string): Promise<string> {
    return expect(await this.api.call({ op: "make_dir", id, path }), "file_done").data.path;
  }
  async renameFile(id: InstanceId, path: string, newName: string): Promise<string> {
    return expect(await this.api.call({ op: "rename_file", id, path, new_name: newName }), "file_done").data.path;
  }
  async moveFile(id: InstanceId, path: string, destDir: string): Promise<string> {
    return expect(await this.api.call({ op: "move_file", id, path, dest_dir: destDir }), "file_done").data.path;
  }
  async copyFile(id: InstanceId, path: string, destDir: string): Promise<string> {
    return expect(await this.api.call({ op: "copy_file", id, path, dest_dir: destDir }), "file_done").data.path;
  }
  async extractZip(id: InstanceId, path: string): Promise<ZipExtracted> {
    return expect(await this.api.call({ op: "extract_zip", id, path }), "zip_extracted").data;
  }
  async trashFile(id: InstanceId, path: string): Promise<string> {
    return expect(await this.api.call({ op: "trash_file", id, path }), "file_done").data.path;
  }
  async uploadFile(id: InstanceId, source: string, destDir: string): Promise<string> {
    return expect(await this.api.call({ op: "upload_file", id, source, dest_dir: destDir }), "file_done").data.path;
  }
  async downloadFile(id: InstanceId, path: string, dest: string): Promise<string> {
    return expect(await this.api.call({ op: "download_file", id, path, dest }), "file_done").data.path;
  }
  async exportConsole(id: InstanceId, dest: string): Promise<string> {
    return expect(await this.api.call({ op: "export_console", id, dest }), "file_done").data.path;
  }
  async worlds(id: InstanceId): Promise<{ worlds: WorldInfo[]; level_name: string }> {
    return expect(await this.api.call({ op: "worlds", id }), "worlds").data;
  }
  async duplicateWorld(id: InstanceId, name: string, newName: string) {
    return expect(await this.api.call({ op: "duplicate_world", id, name, new_name: newName }), "worlds").data;
  }
  async exportWorld(id: InstanceId, name: string, dest: string): Promise<string> {
    return expect(await this.api.call({ op: "export_world", id, name, dest }), "file_done").data.path;
  }
  async importWorld(id: InstanceId, source: string, newName: string) {
    return expect(await this.api.call({ op: "import_world", id, source, new_name: newName }), "worlds").data;
  }
  async setActiveWorld(id: InstanceId, name: string) {
    return expect(await this.api.call({ op: "set_active_world", id, name }), "worlds").data;
  }
  async trashWorld(id: InstanceId, name: string) {
    return expect(await this.api.call({ op: "trash_world", id, name }), "worlds").data;
  }
  async savedItems(id: InstanceId): Promise<SavedItem[]> {
    return expect(await this.api.call({ op: "saved_items", id }), "saved_items").data;
  }
  async restoreSaved(id: InstanceId, itemId: string): Promise<string> {
    return expect(await this.api.call({ op: "restore_saved", id, item_id: itemId }), "file_done").data.path;
  }
  async purgeSaved(id: InstanceId, itemId: string): Promise<void> {
    expect(await this.api.call({ op: "purge_saved", id, item_id: itemId }), "ok");
  }
  async gameRules(id: InstanceId): Promise<{ rules: RuleValue[]; new_names: boolean }> {
    return expect(await this.api.call({ op: "game_rules", id }), "game_rules").data;
  }
  async setGameRule(id: InstanceId, key: string, value: string): Promise<void> {
    expect(await this.api.call({ op: "set_game_rule", id, key, value }), "ok");
  }
  async activity(id: InstanceId, limit = 200): Promise<ActivityEntry[]> {
    return expect(await this.api.call({ op: "activity", id, limit }), "activity").data;
  }
  async restartWithNotice(id: InstanceId, seconds: number): Promise<void> {
    expect(await this.api.call({ op: "restart_with_notice", id, seconds }), "ok");
  }
  /** Quita el servidor de la lista; con `deleteFiles` manda antes su carpeta a la Papelera de reciclaje. */
  async unregister(id: InstanceId, deleteFiles = false): Promise<void> {
    expect(await this.api.call({ op: "unregister", id, delete_files: deleteFiles }), "ok");
  }
  async adminState(id: InstanceId): Promise<AdminState> {
    return expect(await this.api.call({ op: "admin_state", id }), "admin_state").data;
  }
  async setMaintenance(id: InstanceId, on: boolean): Promise<AdminState> {
    return expect(await this.api.call({ op: "set_maintenance", id, on }), "admin_state").data;
  }
  async cancelRestart(id: InstanceId): Promise<AdminState> {
    return expect(await this.api.call({ op: "cancel_restart", id }), "admin_state").data;
  }
  async contentPlatforms(id: InstanceId): Promise<Platform[]> {
    return expect(await this.api.call({ op: "content_platforms", id }), "content_platforms").data;
  }
  async contentSearch(
    id: InstanceId,
    kind: ContentKind,
    source: ContentSource,
    query: string,
    sort: SearchSort,
    offset = 0,
  ): Promise<SearchPage> {
    return expect(await this.api.call({ op: "content_search", id, kind, source, query, sort, offset }), "content_search").data;
  }
  async contentProject(id: InstanceId, kind: ContentKind, source: ContentSource, project: string) {
    return expect(await this.api.call({ op: "content_project", id, kind, source, project }), "content_details").data;
  }
  async contentPlan(id: InstanceId, kind: ContentKind, source: ContentSource, versionId: string): Promise<ContentPlan> {
    return expect(await this.api.call({ op: "content_plan", id, kind, source, version_id: versionId }), "content_plan").data;
  }
  async contentInstall(id: InstanceId, kind: ContentKind, source: ContentSource, versionId: string): Promise<ContentList> {
    return expect(await this.api.call({ op: "content_install", id, kind, source, version_id: versionId }), "content_list").data;
  }
  /** Floodgate como plugin (Paper/Purpur/Spigot) desde la API oficial de GeyserMC, con su SHA-256. */
  async installFloodgate(id: InstanceId): Promise<ContentList> {
    return expect(await this.api.call({ op: "install_floodgate", id }), "content_list").data;
  }
  async contentInstalled(id: InstanceId, kind: ContentKind): Promise<ContentList> {
    return expect(await this.api.call({ op: "content_installed", id, kind }), "content_list").data;
  }
  async contentToggle(id: InstanceId, kind: ContentKind, fileName: string, enabled: boolean): Promise<ContentList> {
    return expect(await this.api.call({ op: "content_toggle", id, kind, file_name: fileName, enabled }), "content_list").data;
  }
  async contentRemove(id: InstanceId, kind: ContentKind, fileName: string): Promise<ContentList> {
    return expect(await this.api.call({ op: "content_remove", id, kind, file_name: fileName }), "content_list").data;
  }
  async cancelWorldOp(id: InstanceId): Promise<AdminState> {
    return expect(await this.api.call({ op: "cancel_world_op", id }), "admin_state").data;
  }
  async backups(id: InstanceId): Promise<BackupList> {
    return expect(await this.api.call({ op: "backups", id }), "backups").data;
  }
  async createBackup(id: InstanceId, scope: BackupScope, note: string | null): Promise<BackupList> {
    return expect(await this.api.call({ op: "create_backup", id, scope, note }), "backups").data;
  }
  async verifyBackup(id: InstanceId, backup: string): Promise<BackupList> {
    return expect(await this.api.call({ op: "verify_backup", id, backup }), "backups").data;
  }
  async restoreBackup(id: InstanceId, backup: string): Promise<BackupList> {
    return expect(await this.api.call({ op: "restore_backup", id, backup }), "backups").data;
  }
  async deleteBackup(id: InstanceId, backup: string): Promise<BackupList> {
    return expect(await this.api.call({ op: "delete_backup", id, backup }), "backups").data;
  }
  async protectBackup(id: InstanceId, backup: string, value: boolean): Promise<BackupList> {
    return expect(await this.api.call({ op: "protect_backup", id, backup, protected: value }), "backups").data;
  }
  async playitStatus(): Promise<PlayitInfo> {
    return expect(await this.api.call({ op: "playit_status" }), "playit").data;
  }
  async playitEnable(): Promise<PlayitInfo> {
    return expect(await this.api.call({ op: "playit_enable" }), "playit").data;
  }
  async playitDisable(): Promise<PlayitInfo> {
    return expect(await this.api.call({ op: "playit_disable" }), "playit").data;
  }
  async playitLink(): Promise<PlayitInfo> {
    return expect(await this.api.call({ op: "playit_link" }), "playit").data;
  }
  async playitUnlink(): Promise<PlayitInfo> {
    return expect(await this.api.call({ op: "playit_unlink" }), "playit").data;
  }
  async playitLoginUrl(): Promise<string> {
    return expect(await this.api.call({ op: "playit_login_url" }), "playit_login_url").data;
  }
  async connection(id: InstanceId): Promise<ConnectionInfo> {
    return expect(await this.api.call({ op: "connection", id }), "connection").data;
  }
  async testAddress(id: InstanceId, address: string): Promise<ConnectionInfo> {
    return expect(await this.api.call({ op: "test_address", id, address }), "connection").data;
  }
  async testBedrockAddress(id: InstanceId, address: string): Promise<ConnectionInfo> {
    return expect(await this.api.call({ op: "test_bedrock_address", id, address }), "connection").data;
  }
  async installJava(major: number): Promise<JobSnapshot> {
    return expect(await this.api.call({ op: "install_java", major }), "job").data;
  }
  async securityReport(id: InstanceId): Promise<SecurityReport> {
    return expect(await this.api.call({ op: "security_report", id }), "security_report").data;
  }
  async serverUpdateCheck(id: InstanceId): Promise<ServerUpdateInfo> {
    return expect(await this.api.call({ op: "server_update_check", id }), "server_update").data;
  }
  /** Trabajo: copia completa, descarga verificada y cambio de archivo. */
  async modpackStatus(id: InstanceId): Promise<ModpackStatusInfo> {
    return expect(await this.api.call({ op: "modpack_status", id }), "modpack_status").data;
  }
  async modpackUpdatePreview(id: InstanceId, versionId: string): Promise<ModpackUpdatePlan> {
    return expect(await this.api.call({ op: "modpack_update_preview", id, version_id: versionId }), "modpack_update_plan").data;
  }
  async modpackUpdateApply(id: InstanceId, versionId: string, acceptLoaderChange: boolean): Promise<JobSnapshot> {
    return expect(
      await this.api.call({ op: "modpack_update_apply", id, version_id: versionId, accept_loader_change: acceptLoaderChange }),
      "job",
    ).data;
  }
  async modpackLink(id: InstanceId, source: ContentSource, versionId: string): Promise<ModpackStatusInfo> {
    return expect(await this.api.call({ op: "modpack_link", id, source, version_id: versionId }), "modpack_status").data;
  }
  async modpackUnlink(id: InstanceId): Promise<ModpackStatusInfo> {
    return expect(await this.api.call({ op: "modpack_unlink", id }), "modpack_status").data;
  }
  async applyServerUpdate(id: InstanceId, loaderVersion: string): Promise<JobSnapshot> {
    return expect(await this.api.call({ op: "apply_server_update", id, loader_version: loaderVersion }), "job").data;
  }
  async loaderVersions(flavor: Flavor, mcVersion: string): Promise<LoaderOption[]> {
    return expect(await this.api.call({ op: "loader_versions", flavor, mc_version: mcVersion }), "loader_versions").data;
  }
  /** Trabajo: copia el servidor (detenido) a una carpeta nueva con otro puerto. */
  async cloneServer(id: InstanceId, name: string): Promise<JobSnapshot> {
    return expect(await this.api.call({ op: "clone_server", id, name }), "job").data;
  }
  async setArchived(id: InstanceId, archived: boolean): Promise<void> {
    expect(await this.api.call({ op: "set_archived", id, archived }), "ok");
  }
  async playerCommand(id: InstanceId, name: string, command: PlayerCommand): Promise<void> {
    expect(await this.api.call({ op: "player_command", id, name, command }), "ok");
  }
  async playerHistory(id: InstanceId): Promise<PlayerRecord[]> {
    return expect(await this.api.call({ op: "player_history", id }), "player_history").data;
  }
  async setPlayerNote(id: InstanceId, name: string, note: string): Promise<PlayerRecord[]> {
    return expect(await this.api.call({ op: "set_player_note", id, name, note }), "player_history").data;
  }
  async bedrockPlayers(id: InstanceId): Promise<BedrockPlayersInfo> {
    return expect(await this.api.call({ op: "bedrock_players", id }), "bedrock_players").data;
  }
  async bedrockAllowlist(id: InstanceId, change: BedrockAllowChange): Promise<BedrockPlayersInfo> {
    return expect(await this.api.call({ op: "bedrock_allowlist", id, change }), "bedrock_players").data;
  }
  async bedrockAllowlistEnabled(id: InstanceId, enabled: boolean): Promise<BedrockPlayersInfo> {
    return expect(await this.api.call({ op: "bedrock_allowlist_enabled", id, enabled }), "bedrock_players").data;
  }
  async bedrockPermission(id: InstanceId, xuid: string, level: BedrockLevel | null): Promise<BedrockPlayersInfo> {
    return expect(await this.api.call({ op: "bedrock_permission", id, xuid, level }), "bedrock_players").data;
  }
  async addons(id: InstanceId): Promise<AddonsInfo> {
    return expect(await this.api.call({ op: "addons", id }), "addons").data;
  }
  async addonImport(id: InstanceId, source: string): Promise<AddonsInfo> {
    return expect(await this.api.call({ op: "addon_import", id, source }), "addons").data;
  }
  async addonSearch(id: InstanceId, query: string, sort: SearchSort = "relevance", offset = 0): Promise<SearchPage> {
    return expect(await this.api.call({ op: "addon_search", id, query, sort, offset }), "content_search").data;
  }
  async addonInstallCurseforge(id: InstanceId, projectId: string, fileId: string | null = null): Promise<AddonsInfo> {
    return expect(await this.api.call({ op: "addon_install_curseforge", id, project_id: projectId, file_id: fileId }), "addons").data;
  }
  async addonSetActive(id: InstanceId, uuid: string, active: boolean): Promise<AddonsInfo> {
    return expect(await this.api.call({ op: "addon_set_active", id, uuid, active }), "addons").data;
  }
  async addonRemove(id: InstanceId, uuid: string): Promise<AddonsInfo> {
    return expect(await this.api.call({ op: "addon_remove", id, uuid }), "addons").data;
  }
  async modSides(id: InstanceId): Promise<ModSidesInfo> {
    return expect(await this.api.call({ op: "mod_sides", id }), "mod_sides").data;
  }
  async quarantineMods(id: InstanceId, files: string[]): Promise<ModSidesInfo> {
    return expect(await this.api.call({ op: "quarantine_mods", id, files }), "mod_sides").data;
  }
  async restoreMods(id: InstanceId, files: string[]): Promise<ModSidesInfo> {
    return expect(await this.api.call({ op: "restore_mods", id, files }), "mod_sides").data;
  }
  async friendPack(id: InstanceId): Promise<FriendPackPlan> {
    return expect(await this.api.call({ op: "friend_pack", id }), "friend_pack").data;
  }
  async exportFriendPack(id: InstanceId, dest: string): Promise<string> {
    return expect(await this.api.call({ op: "export_friend_pack", id, dest }), "file_done").data.path;
  }
  async luckPerms(id: InstanceId): Promise<LuckPermsInfo> {
    return expect(await this.api.call({ op: "luck_perms", id }), "luck_perms").data;
  }
  async luckPermsPreview(id: InstanceId, change: LpChange): Promise<LuckPermsPreview> {
    return expect(await this.api.call({ op: "luck_perms_preview", id, change }), "luck_perms_preview").data;
  }
  async luckPermsApply(id: InstanceId, change: LpChange): Promise<LuckPermsApplied> {
    return expect(await this.api.call({ op: "luck_perms_apply", id, change }), "luck_perms_applied").data;
  }
  async clearPlayerHistory(id: InstanceId): Promise<PlayerRecord[]> {
    return expect(await this.api.call({ op: "clear_player_history", id }), "player_history").data;
  }
  async automation(id: InstanceId): Promise<AutomationInfo> {
    return expect(await this.api.call({ op: "automation", id }), "automation").data;
  }
  async setAutomation(id: InstanceId, change: { auto_restart?: boolean; autostart?: boolean }): Promise<AutomationInfo> {
    return expect(
      await this.api.call({ op: "set_automation", id, auto_restart: change.auto_restart ?? null, autostart: change.autostart ?? null }),
      "automation",
    ).data;
  }
  async setStartup(enabled: boolean): Promise<void> {
    expect(await this.api.call({ op: "set_startup", enabled }), "ok");
  }
  async setKeepAwake(enabled: boolean): Promise<void> {
    expect(await this.api.call({ op: "set_keep_awake", enabled }), "ok");
  }
  async supporter(): Promise<SupporterInfo> {
    return expect(await this.api.call({ op: "supporter" }), "supporter").data;
  }
  async activateSupporter(licenseKey: string): Promise<SupporterInfo> {
    return expect(await this.api.call({ op: "activate_supporter", license_key: licenseKey }), "supporter").data;
  }
  async refreshSupporter(): Promise<SupporterInfo> {
    return expect(await this.api.call({ op: "refresh_supporter" }), "supporter").data;
  }
  async forgetSupporter(): Promise<SupporterInfo> {
    return expect(await this.api.call({ op: "forget_supporter" }), "supporter").data;
  }
  async serverIcon(id: InstanceId): Promise<ServerIconInfo> {
    return expect(await this.api.call({ op: "server_icon", id }), "server_icon").data;
  }
  async previewServerIcon(source: string, fit: IconFit): Promise<IconPreview> {
    return expect(await this.api.call({ op: "preview_server_icon", source, fit }), "icon_preview").data;
  }
  async setServerIcon(id: InstanceId, source: string, fit: IconFit): Promise<ServerIconInfo> {
    return expect(await this.api.call({ op: "set_server_icon", id, source, fit }), "server_icon").data;
  }
  async removeServerIcon(id: InstanceId): Promise<ServerIconInfo> {
    return expect(await this.api.call({ op: "remove_server_icon", id }), "server_icon").data;
  }
  async diagnose(id: InstanceId): Promise<Diagnosis> {
    return expect(await this.api.call({ op: "diagnose", id }), "diagnosis").data;
  }
  async setLaunch(id: InstanceId, change: { java?: string; min_memory_mb?: number; max_memory_mb?: number }): Promise<Instance> {
    return expect(await this.api.call({ op: "set_launch", id, ...change }), "instance").data;
  }
  async downloadBackup(id: InstanceId, backupId: string, dest: string): Promise<string> {
    return expect(await this.api.call({ op: "download_backup", id, backup_id: backupId, dest }), "file_done").data.path;
  }
  async companions(): Promise<CompanionsInfo> {
    return expect(await this.api.call({ op: "companions" }), "companions").data;
  }
  async setCompanionsEnabled(enabled: boolean): Promise<CompanionsInfo> {
    return expect(await this.api.call({ op: "set_companions_enabled", enabled }), "companions").data;
  }
  async createInvite(id: InstanceId, perms: Perm[], minutes: number): Promise<NewInvite> {
    return expect(await this.api.call({ op: "create_invite", id, perms, minutes }), "invite").data;
  }
  async createHostInvite(minutes: number): Promise<NewInvite> {
    return expect(await this.api.call({ op: "create_host_invite", minutes }), "invite").data;
  }
  async cancelInvite(invite_id: string): Promise<CompanionsInfo> {
    return expect(await this.api.call({ op: "cancel_invite", invite_id }), "companions").data;
  }
  async setCompanionPerms(companion: string, id: InstanceId, perms: Perm[]): Promise<CompanionsInfo> {
    return expect(await this.api.call({ op: "set_companion_perms", companion, id, perms }), "companions").data;
  }
  async removeCompanion(companion: string): Promise<CompanionsInfo> {
    return expect(await this.api.call({ op: "remove_companion", companion }), "companions").data;
  }
  async joinWithCode(code: string, name: string): Promise<RemoteHost[]> {
    return expect(await this.api.call({ op: "join_with_code", code, name }), "remote_hosts").data;
  }
  async remoteHosts(): Promise<RemoteHost[]> {
    return expect(await this.api.call({ op: "remote_hosts" }), "remote_hosts").data;
  }
  async forgetHost(host: string): Promise<RemoteHost[]> {
    return expect(await this.api.call({ op: "forget_host", host }), "remote_hosts").data;
  }
  async refreshHost(host: string): Promise<RemoteHost[]> {
    return expect(await this.api.call({ op: "refresh_host", host }), "remote_hosts").data;
  }
  /** Escribe `voice_host` (dirección de un túnel de voz detectado) y/o el puerto de Simple Voice Chat. */
  async setVoiceConfig(id: InstanceId, change: { voice_host?: string; port?: number }): Promise<ConnectionInfo> {
    return expect(await this.api.call({ op: "set_voice_config", id, ...change }), "connection").data;
  }
  async health(id: InstanceId): Promise<HealthReport> {
    return expect(await this.api.call({ op: "health", id }), "health").data;
  }
  async measureTps(id: InstanceId): Promise<TpsReading> {
    return expect(await this.api.call({ op: "measure_tps", id }), "tps").data;
  }
  async alerts(): Promise<AlertList> {
    return expect(await this.api.call({ op: "alerts" }), "alerts").data;
  }
  async markAlertsRead(ids: string[]): Promise<AlertList> {
    return expect(await this.api.call({ op: "mark_alerts_read", ids }), "alerts").data;
  }
  async setAlertSettings(settings: AlertSettings): Promise<AlertList> {
    return expect(await this.api.call({ op: "set_alert_settings", settings }), "alerts").data;
  }
  async tasks(id: InstanceId): Promise<TaskList> {
    return expect(await this.api.call({ op: "tasks", id }), "tasks").data;
  }
  async saveTask(id: InstanceId, task: TaskDraft): Promise<TaskList> {
    return expect(await this.api.call({ op: "save_task", id, task }), "tasks").data;
  }
  async deleteTask(id: InstanceId, taskId: string): Promise<TaskList> {
    return expect(await this.api.call({ op: "delete_task", id, task_id: taskId }), "tasks").data;
  }
  async runTaskNow(id: InstanceId, taskId: string): Promise<TaskList> {
    return expect(await this.api.call({ op: "run_task_now", id, task_id: taskId }), "tasks").data;
  }
  async setBackupSettings(id: InstanceId, keep: number): Promise<BackupList> {
    return expect(await this.api.call({ op: "set_backup_settings", id, keep }), "backups").data;
  }
}

export function errorMessage(e: unknown): string {
  if (e instanceof MercError) return e.message;
  if (e instanceof Error) return e.message;
  if (typeof e === "object" && e && "message" in e) return String((e as { message: unknown }).message);
  return String(e);
}

/** MercApi que manda cada petición a la PC de un amigo (su PC decide si está permitida).
 * Los diálogos para elegir archivos son de ESTA PC: lo que se descarga llega aquí y lo que se
 * sube sale de aquí, por el canal de compañeros. */
export function remoteApi(local: MercApi, host: string): MercApi {
  const ownerOnly = async (): Promise<never> => {
    throw new MercError("forbidden", "Esto solo lo puede hacer el dueño del servidor en su PC.");
  };
  return {
    call: (request) => {
      switch (request.op) {
        case "download_file":
          return local.call({ op: "remote_download", host, item: { type: "file", id: request.id, path: request.path }, dest: request.dest });
        case "export_world":
          return local.call({ op: "remote_download", host, item: { type: "world", id: request.id, name: request.name }, dest: request.dest });
        case "download_backup":
          return local.call({
            op: "remote_download",
            host,
            item: { type: "backup", id: request.id, backup: request.backup_id },
            dest: request.dest,
          });
        case "export_console":
          return local.call({ op: "remote_download", host, item: { type: "console", id: request.id }, dest: request.dest });
        case "upload_file":
          return local.call({ op: "remote_upload", host, id: request.id, source: request.source, dest_dir: request.dest_dir });
        default:
          return local.call({ op: "remote_call", host, request });
      }
    },
    info: () => local.info(),
    onFileDrop: (h) => local.onFileDrop(h),
    canTransfer: local.canTransfer,
    // Servidor casero: los Java de esa PC (sus rutas no viajan; esa PC elige al crear).
    javaFor: async (): Promise<JavaChoice> => {
      const r = await local.call({ op: "remote_call", host, request: { op: "find_java" } });
      if (r.kind !== "java_candidates") throw new MercError("internal", "Respuesta inesperada de la otra PC.");
      const candidates = r.data.map((c, i) => ({ ...c, path: `java-remoto-${i + 1}` }));
      return { candidates, recommended: null, recommended_major: 0 };
    },
    pickFolder: () => local.pickFolder(),
    pickFile: (filter) => local.pickFile(filter),
    pickSavePath: (name) => local.pickSavePath(name),
    openUrl: (url) => local.openUrl(url),
    openServerFolder: ownerOnly,
    openBackupsFolder: ownerOnly,
    notify: (title, body) => local.notify(title, body),
    setWindowIcon: (svg) => local.setWindowIcon(svg),
    appUpdateCheck: () => local.appUpdateCheck(),
    appUpdateInstall: (p) => local.appUpdateInstall(p),
  };
}
