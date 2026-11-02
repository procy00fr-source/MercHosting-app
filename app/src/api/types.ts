// Tipos del protocolo v1 del supervisor (crates/merc-ipc/src/protocol.rs).
// La forma exacta está fijada por docs/contratos/ipc-v1-ejemplos.json, generado desde Rust
// y comprobado en src/api/contract.test.ts. Si cambias algo aquí, cambia también Rust.

export type InstanceId = string;

export const RUNTIME_STATES = [
  "STOPPED",
  "VALIDATING",
  "STARTING",
  "LOADING",
  "ONLINE",
  "STOPPING",
  "CRASHED",
  "ERROR",
  "RECOVERING",
  "UNKNOWN",
] as const;
export type RuntimeState = (typeof RUNTIME_STATES)[number];

export type ServerKind =
  | "vanilla"
  | "paper"
  | "purpur"
  | "spigot"
  | "fabric"
  | "forge"
  | "neo_forge"
  | "quilt"
  | "mohist"
  | "arclight"
  | "bedrock"
  | "unknown";
export type Confidence = "verified" | "inferred" | "unknown";
export type ConsoleStream = "stdout" | "stderr" | "system";

export const ERROR_CODES = [
  "auth_failed",
  "version_mismatch",
  "bad_request",
  "not_found",
  "conflict",
  "invalid_state",
  "no_console",
  "network",
  "forbidden",
  "internal",
] as const;
export type ErrorCode = (typeof ERROR_CODES)[number];

export type LaunchTarget =
  | { type: "jar"; jar: string }
  | { type: "args_file"; args_file: string; user_jvm_args: string | null }
  /** Ejecutable nativo sin Java (Bedrock Dedicated Server). */
  | { type: "native"; exe: string };

export interface LaunchProfile {
  java: string;
  min_memory_mb: number;
  max_memory_mb: number;
  extra_jvm_args: string[];
  target: LaunchTarget;
}

export interface Instance {
  id: InstanceId;
  name: string;
  root: string;
  kind: ServerKind;
  mc_version: string | null;
  loader_version: string | null;
  launch: LaunchProfile;
  imported: boolean;
  created_at_unix: number;
}

export interface ExitInfo {
  code: number | null;
  forced: boolean;
  requested: boolean;
  at_unix_ms: number;
}

export interface StatusSnapshot {
  state: RuntimeState;
  has_console: boolean;
  pid: number | null;
  port: number | null;
  started_at_unix_ms: number | null;
  online_at_unix_ms: number | null;
  last_exit: ExitInfo | null;
  hint: string | null;
  log_file: string | null;
}

export interface InstanceSummary {
  instance: Instance;
  status: StatusSnapshot;
  eula_accepted: boolean;
  configured_port: number | null;
  /** Archivado: no se puede encender y se lista aparte. */
  archived?: boolean;
}

export interface DetectedServer {
  kind: ServerKind;
  mc_version: string | null;
  loader_version: string | null;
  launch: LaunchTarget | null;
  confidence: Confidence;
  evidence: string[];
  warnings: string[];
}

export interface JavaCandidate {
  path: string;
  major: number | null;
  description: string;
  error: string | null;
  /** Es un JDK (tiene javac); Spigot lo necesita para compilarse. */
  jdk: boolean;
}

export interface ConsoleLine {
  seq: number;
  at_unix_ms: number;
  stream: ConsoleStream;
  text: string;
}

export interface MetricSample {
  at_unix_ms: number;
  /** % de CPU de toda la PC (0–100). */
  cpu_percent: number;
  memory_bytes: number;
}

export interface Metrics {
  samples: MetricSample[];
  players: string[];
  max_players: number | null;
}

export interface SystemInfo {
  total_memory_bytes: number | null;
  logical_cpus: number;
  lan_ipv4: string | null;
}

/** Tipos de servidor que MercHosting sabe crear desde su fuente oficial. */
export type Flavor = "vanilla" | "paper" | "purpur" | "spigot" | "fabric" | "quilt" | "forge" | "neo_forge" | "arclight" | "bedrock";

export interface CatalogEntry {
  mc_version: string;
  java_major: number;
  latest: boolean;
}

export type Checksum =
  | { algorithm: "md5"; hex: string }
  | { algorithm: "sha1"; hex: string } | { algorithm: "sha256"; hex: string } | { algorithm: "none" };

export interface Resolved {
  flavor: Flavor;
  mc_version: string;
  loader_version: string | null;
  java_major: number;
  experimental: boolean;
  artifact: { url: string; mirrors: string[]; file_name: string; checksum: Checksum; size: number | null };
  installer_args: string[] | null;
  installer_in_cache: boolean;
  needs_jdk: boolean;
  notes: string[];
}

export type InstallStep = "preparing" | "downloading" | "installing" | "configuring" | "finishing";
export type JobState = "running" | "succeeded" | "failed" | "cancelled";

export interface JobSnapshot {
  job_id: string;
  name: string;
  state: JobState;
  step: InstallStep | null;
  message: string;
  download: { file: string; done: number; total: number | null } | null;
  log: { seq: number; text: string }[];
  next_after: number;
  instance_id: InstanceId | null;
  error: string | null;
}

export interface CreateServerArgs {
  name: string;
  flavor: Flavor;
  mc_version: string;
  path: string;
  java: string;
  min_memory_mb: number;
  max_memory_mb: number;
  properties: Record<string, string>;
  accept_eula: boolean;
  /** Versión (Modrinth) o archivo (CurseForge) de un modpack que se instala encima. */
  modpack_version: string | null;
  modpack_source: ContentSource;
  /** Versión concreta del loader; null = la recomendada. */
  loader_version?: string | null;
  /** Reglas del juego que se aplican la primera vez que se enciende. */
  game_rules?: Record<string, string>;
}

export type GameMode = "survival" | "creative" | "adventure" | "spectator";

export type PlayerCommand =
  | { type: "message"; text: string }
  | { type: "game_mode"; mode: GameMode }
  | { type: "teleport_to"; target: string }
  | { type: "teleport_coords"; x: number; y: number; z: number };

export interface PlayerRecord {
  name: string;
  first_seen_unix: number;
  last_seen_unix: number;
  sessions: number;
  total_secs: number;
  online_since_unix: number | null;
  note: string | null;
  /** Bedrock: XUID que dio el servidor al entrar (Java: null). */
  xuid: string | null;
}

/** Actualización de la propia app (GitHub Releases, firmada). */
export interface AppUpdateCheck {
  /** `false` en compilaciones de desarrollo. */
  enabled: boolean;
  current: string;
  available: boolean;
  version: string | null;
  notes: string | null;
  date: string | null;
  /** Se actualiza sola (Windows o AppImage). Con el .deb de Linux se descarga a mano. */
  self_install?: boolean;
}

export type ModSide = "client_only" | "server_only" | "both" | "unknown";
export interface ModSideReport {
  file: string;
  name: string;
  enabled: boolean;
  side: ModSide;
  confidence: "alta" | "media" | "baja";
  evidence: string[];
  needed_by: string[];
  suggested: boolean;
}
export interface QuarantinedMod {
  file: string;
  moved_at_unix: number;
  reason: string;
}
export type AddonKind = "behavior" | "resource";

export interface AddonPack {
  uuid: string;
  version: [number, number, number];
  name: string;
  description: string;
  kind: AddonKind;
  /** Carpeta relativa a la raíz del servidor. */
  folder: string;
  /** Está en la lista de packs del mundo. */
  active: boolean;
  /** Parece venir con el propio servidor de Mojang (vanilla, chemistry…). */
  bundled: boolean;
  scripts: boolean;
}

export interface AddonImportResult {
  installed: string[];
  unchanged: string[];
  replaced: string[];
  notes: string[];
}

export type BedrockLevel = "operator" | "member" | "visitor";

export interface BedrockAllowEntry {
  name: string;
  xuid: string | null;
  ignores_player_limit: boolean;
}

export type BedrockAllowChange =
  | { type: "add"; name: string; ignores_player_limit: boolean }
  | { type: "remove"; name: string }
  | { type: "set_ignores_limit"; name: string; value: boolean };

export interface BedrockPlayersInfo {
  allow_list_enabled: boolean;
  online_mode: boolean;
  default_permission: string;
  legacy_whitelist: boolean;
  running: boolean;
  allowlist: BedrockAllowEntry[];
  permissions: { xuid: string; level: BedrockLevel; name: string | null }[];
  known: { name: string; xuid: string; online: boolean; last_seen_unix: number }[];
  online: string[];
}

export interface AddonsInfo {
  level_name: string;
  world_exists: boolean;
  packs: AddonPack[];
  last_import: AddonImportResult | null;
}

export interface ModSidesInfo {
  items: ModSideReport[];
  quarantined: QuarantinedMod[];
  folder: string;
  catalog_ok: boolean;
}

export interface PackMod {
  title: string;
  file_name: string;
  optional: boolean;
  page_url: string | null;
  size: number;
}
export interface ManualMod {
  title: string;
  file_name: string;
  page_url: string | null;
  reason: string;
}
export interface FriendPackPlan {
  mc_version: string | null;
  loader: [string, string] | null;
  included: PackMod[];
  manual: ManualMod[];
  server_only: string[];
  blocked: string | null;
}

export interface LpInstall {
  jar: string;
  data_dir: string;
}
export interface LpNode {
  kind: string;
  key: string;
  value: boolean;
  expiry: number | null;
  context: Record<string, string[]>;
}
export interface LpGroup {
  name: string;
  weight: number | null;
  prefix: string | null;
  parents: string[];
  nodes: LpNode[];
}
export interface LpUser {
  uuid: string;
  username: string | null;
  primary_group: string;
  parents: string[];
  nodes: LpNode[];
}
export interface LpTrack {
  name: string;
  groups: string[];
}
export interface LpSnapshot {
  generated_at: string | null;
  groups: LpGroup[];
  users: LpUser[];
  tracks: LpTrack[];
}
export interface LuckPermsInfo {
  install: LpInstall | null;
  online: boolean;
  snapshot: LpSnapshot | null;
  backups: string[];
}
export type LpTarget = { kind: "group" | "user"; name: string };
export type LpChange =
  | { type: "create_group"; group: string }
  | { type: "delete_group"; group: string }
  | { type: "set_permission"; target: LpTarget; node: string; value: boolean }
  | { type: "unset_permission"; target: LpTarget; node: string; context: Record<string, string[]> }
  | { type: "add_parent"; target: LpTarget; group: string }
  | { type: "remove_parent"; target: LpTarget; group: string };
export interface LuckPermsPreview {
  command: string;
  summary: string;
}
export interface LuckPermsApplied {
  verified: boolean;
  command: string;
  backup: string;
  snapshot: LpSnapshot;
}

export interface AutomationInfo {
  auto_restart: boolean;
  max_restarts: number;
  recent_restarts: number;
  gave_up: boolean;
  autostart: boolean;
  startup_available: boolean;
  startup_enabled: boolean;
  /** No suspender la PC mientras haya un servidor encendido (ajuste de toda la app). */
  keep_awake: boolean;
  /** Windows confirmó la petición y está vigente ahora. */
  keep_awake_active: boolean;
}

export interface LoaderOption {
  version: string;
  stable: boolean;
  recommended: boolean;
}

export interface ModpackSummary {
  source: ContentSource;
  flavor: Flavor;
  name: string;
  pack_version: string;
  mc_version: string;
  loader: string | null;
  loader_version: string | null;
  java_major: number;
  server_files: number;
  client_only_files: number;
  download_bytes: number;
  warnings: string[];
}

export interface PropEntry {
  key: string;
  value: string;
}

export interface ListedPlayer {
  name: string;
  uuid: string | null;
  level: number | null;
  reason: string | null;
  expires: string | null;
}

export interface PlayerLists {
  whitelist: ListedPlayer[];
  ops: ListedPlayer[];
  banned: ListedPlayer[];
  banned_ips: string[];
  warnings: string[];
}

export type PlayerAction = "whitelist_add" | "whitelist_remove" | "op" | "deop" | "kick" | "ban" | "pardon";

export interface FileEntry {
  name: string;
  is_dir: boolean;
  size: number | null;
  modified_unix_ms: number | null;
  link: boolean;
}

export interface TextFile {
  path: string;
  content: string;
  size: number;
  modified_unix_ms: number | null;
}

export interface LinkedPack {
  source: ContentSource;
  project_id: string;
  title: string;
  version_id: string;
  version_number: string;
  mc_version: string;
  loader: string | null;
  loader_version: string | null;
  installed_at_unix: number;
  files: number;
}

export interface ModpackVersionChoice {
  version_id: string;
  version_number: string;
  name: string;
  date_published: string;
  game_versions: string[];
  compatible: boolean;
  current: boolean;
  newer: boolean;
}

export interface ModpackStatusInfo {
  linked: LinkedPack | null;
  online: boolean;
  error: string | null;
  versions: ModpackVersionChoice[];
}

export type PackChangeKind = "add" | "replace" | "remove" | "keep_modified" | "keep_user_file" | "skip_removed";

export interface PlannedChange {
  path: string;
  kind: PackChangeKind;
  from_override: boolean;
}

export interface ModpackUpdatePlan {
  from_version: string;
  to_version: string;
  to_version_id: string;
  mc_version: string;
  loader: string | null;
  loader_from: string | null;
  loader_to: string | null;
  loader_mismatch: boolean;
  changes: PlannedChange[];
  download_bytes: number;
  notes: string[];
}

export interface ZipExtracted {
  folder: string;
  files: number;
  bytes: number;
  skipped: string[];
}

export interface WorldInfo {
  name: string;
  size_bytes: number;
  modified_unix_ms: number | null;
  active: boolean;
  companions: string[];
}

export interface SavedItem {
  id: string;
  kind: "trash" | "copy";
  path: string;
  is_dir: boolean;
  size_bytes: number;
  saved_unix_ms: number | null;
}

export type RuleKind = { type: "bool" } | { type: "int"; min: number; max: number };

export interface RuleValue {
  key: string;
  name: string;
  kind: RuleKind;
  value: string | null;
}

export interface Maintenance {
  since_unix: number;
  previous_whitelist: boolean;
}

export type BackupScope = "world" | "full";
export type BackupReason = "manual" | "before_restore" | "scheduled" | "before_update";

export interface BackupInfo {
  id: string;
  created_unix_ms: number;
  scope: BackupScope;
  reason: BackupReason;
  world: string | null;
  mc_version: string | null;
  size_bytes: number;
  content_bytes: number;
  files: number;
  hot: boolean;
  consistent: boolean;
  verified: boolean | null;
  verified_unix_ms: number | null;
  problems: string[];
  protected: boolean;
  note: string | null;
}

export type AgentPhase = "waiting_for_secret" | "invalid_secret" | "over_limit" | "starting" | "running" | "stopping" | "error";
export type ClaimState = "waiting_for_visit" | "waiting_for_approval" | "approved" | "rejected" | "expired";

export type SecurityLevel = "peligro" | "aviso" | "info" | "bien";

export type SecurityAction =
  | { type: "set_property"; key: string; value: string }
  | { type: "open_tab"; tab: string }
  | { type: "open_companions" };

export interface SecurityCheck {
  code: string;
  level: SecurityLevel;
  title: string;
  detail: string;
  action: SecurityAction | null;
}

export interface SecurityReport {
  checked_at_unix_ms: number;
  public_addresses: string[];
  checks: SecurityCheck[];
}

export type DiagConfidence = "alta" | "media" | "baja";

export type FixAction =
  | { type: "disable_content"; kind: "mod" | "plugin"; file: string }
  | { type: "set_port"; port: number }
  | { type: "set_memory"; max_mb: number }
  | { type: "choose_java"; min_major: number }
  | { type: "search_content"; kind: "mod" | "plugin"; query: string }
  | { type: "open_tab"; tab: string };

export interface Finding {
  code: string;
  title: string;
  confidence: DiagConfidence;
  explanation: string;
  evidence: string[];
  source: string;
  actions: FixAction[];
}

export interface SupporterInfo {
  /** Ya existe el producto en Gumroad (si no, comprar y activar están en «Próximamente»). */
  available: boolean;
  checkout_url: string | null;
  price_usd: number;
  active: boolean;
  email_hint: string | null;
  key_hint: string | null;
  purchased: string | null;
  checked_at_unix_ms: number | null;
  stale: boolean;
  revoked_reason: string | null;
  last_error: string | null;
}

export interface ServerUpdateInfo {
  supported: boolean;
  reason: string | null;
  mc_version: string | null;
  /** Compilación (Paper, Purpur) o versión del loader (Fabric) instalada. */
  current: string | null;
  latest: string | null;
  newer: boolean;
  experimental: boolean;
  notes: string[];
}

/** Cómo pasar una imagen no cuadrada a 64×64. */
export type IconFit = "crop" | "contain";

export interface ServerIconInfo {
  present: boolean;
  /** PNG de 64×64: lo único que Minecraft muestra. */
  valid: boolean;
  width: number | null;
  height: number | null;
  png_base64: string | null;
}

export interface IconPreview {
  png_base64: string;
  source_width: number;
  source_height: number;
  format: string;
}

export interface Diagnosis {
  checked_at_unix_ms: number;
  sources: string[];
  mods_scanned: number;
  plugins_scanned: number;
  findings: Finding[];
}

/** Permisos que el dueño concede por servidor a un compañero. */
export type Perm =
  | "status"
  | "start"
  | "stop"
  | "restart"
  | "console_read"
  | "console_command"
  | "logs"
  | "players"
  | "whitelist"
  | "op"
  | "files_list"
  | "files_read"
  | "files_edit"
  | "files_delete"
  | "content"
  | "backups"
  | "backups_restore"
  | "backups_delete"
  | "world"
  | "settings"
  | "network";

/** Algo que un compañero se lleva a su PC. */
export type RemoteItem =
  | { type: "file"; id: InstanceId; path: string }
  | { type: "world"; id: InstanceId; name: string }
  | { type: "backup"; id: InstanceId; backup: string }
  | { type: "console"; id: InstanceId };

export interface RemoteServer {
  id: InstanceId;
  name: string;
  perms: Perm[];
}

export interface InviteInfo {
  invite_id: string;
  /** null = invitación de control total de la PC (servidor casero). */
  server: InstanceId | null;
  server_name: string;
  perms: Perm[];
  created_at_unix_ms: number;
  expires_at_unix_ms: number;
  host_admin: boolean;
}

export interface CompanionInfo {
  companion: string;
  name: string;
  fingerprint: string;
  added_at_unix_ms: number;
  last_seen_unix_ms: number | null;
  connected: boolean;
  servers: RemoteServer[];
  /** Control total de esta PC (todos los servidores y crear nuevos). */
  host_admin: boolean;
}

export interface CompanionsInfo {
  enabled: boolean;
  online: boolean;
  fingerprint: string | null;
  error: string | null;
  invites: InviteInfo[];
  companions: CompanionInfo[];
}

export interface NewInvite {
  code: string;
  invite: InviteInfo;
}

export interface RemoteHost {
  host: string;
  name: string;
  fingerprint: string;
  servers: RemoteServer[];
  last_ok_unix_ms: number | null;
  last_error: string | null;
  /** Es tu servidor casero: tienes control total y puedes crear servidores allí. */
  host_admin: boolean;
}

export interface Tunnel {
  display_address: string;
  destination: string;
  is_disabled: boolean;
  disabled_reason: string | null;
}

export interface AgentStatus {
  phase: AgentPhase;
  message: string | null;
  version: string | null;
  account_status: string | null;
  tunnels: Tunnel[];
  pending: string[];
  notices: { priority: string; message: string; resolve_link: string | null }[];
}

export interface PlayitInfo {
  supported: boolean;
  enabled: boolean;
  installed: boolean;
  agent_version: string;
  running: boolean;
  agent: AgentStatus | null;
  agent_error: string | null;
  claim: { url: string; state: ClaimState; done: boolean; error: string | null } | null;
}

export interface PingResult {
  tested: string;
  latency_ms: number;
  version: string;
  players_online: number;
  players_max: number;
  motd: string;
}

export interface AddressTest {
  address: string;
  at_unix_ms: number;
  result: PingResult | null;
  error: string | null;
}

export interface ConnectionInfo {
  port: number | null;
  lan: string[];
  tunnels: Tunnel[];
  tests: AddressTest[];
  ports_in_use: { port: number; server: string }[];
  voice: VoiceInfo | null;
  layers: ConnLayer[];
  /** Geyser (jugadores de Bedrock), si está instalado. */
  bedrock: BedrockInfo | null;
}

export interface BedrockInfo {
  geyser: string;
  geyser_enabled: boolean;
  floodgate: string | null;
  floodgate_enabled: boolean;
  config: string | null;
  port: number;
  port_from_config: boolean;
  lan: string[];
  tunnels: Tunnel[];
  tests: AddressTest[];
}

export type LayerState = "operativo" | "degradado" | "caido" | "desconocido";

export interface ConnLayer {
  key: "server" | "agent" | "tunnel" | "external" | "voice" | string;
  state: LayerState;
  detail: string;
  checked_at_unix_ms: number;
}

export interface VoiceProblem {
  code: string;
  level: "error" | "warning" | "info";
  message: string;
}

export interface VoiceInfo {
  installed: boolean;
  enabled: boolean;
  jar: string | null;
  config: string | null;
  port_setting: number | null;
  port: number | null;
  voice_host: string;
  bind_address: string;
  tunnels: Tunnel[];
  suggested_port: number | null;
  listening: number | null;
  restart_needed: boolean;
  problems: VoiceProblem[];
}

export interface DiskSpace {
  what: "server" | "backups";
  path: string;
  free_bytes: number;
  total_bytes: number;
}

export interface HealthReport {
  pc_cpu_percent: number | null;
  pc_memory_used_bytes: number | null;
  pc_memory_total_bytes: number | null;
  disks: DiskSpace[];
  max_memory_mb: number;
  tps_command: string | null;
}

export interface TpsReading {
  tps: [string, number][];
  mspt: number | null;
  command: string;
  raw: string;
}

export type AlertKind =
  | "crash"
  | "low_disk"
  | "high_memory"
  | "task_failed"
  | "companion"
  | "tunnel_down"
  | "backup_failed"
  | "update_available";

export interface Alert {
  id: string;
  at_unix_ms: number;
  level: "info" | "warning" | "critical";
  kind: AlertKind;
  instance_id: string | null;
  server_name: string | null;
  title: string;
  message: string;
  read: boolean;
}

export interface AlertSettings {
  notify: boolean;
  crash: boolean;
  disk: boolean;
  resources: boolean;
  tasks: boolean;
  disk_min_gb: number;
  tunnel: boolean;
  backups: boolean;
  updates: boolean;
}

export interface AlertList {
  items: Alert[];
  unread: number;
  settings: AlertSettings;
}

export type Schedule = { type: "daily"; times: string[]; days: number[] } | { type: "every"; minutes: number };
export type PlayersPolicy = "skip" | "notify";
export type TaskAction =
  | { type: "backup"; scope: BackupScope }
  | { type: "restart"; notice_seconds: number; if_players: PlayersPolicy }
  | { type: "stop"; notice_seconds: number; if_players: PlayersPolicy }
  | { type: "start" }
  | { type: "command"; command: string }
  | { type: "announce"; message: string };

export interface TaskRun {
  at_unix_ms: number;
  outcome: "ok" | "skipped" | "failed";
  message: string;
  manual: boolean;
}

export interface ScheduledTask {
  id: string;
  enabled: boolean;
  action: TaskAction;
  schedule: Schedule;
  created_unix_ms: number;
  next_run_unix_ms: number | null;
  last_run: TaskRun | null;
  running: boolean;
}

export interface TaskDraft {
  id: string | null;
  enabled: boolean;
  action: TaskAction;
  schedule: Schedule;
}

export interface TaskList {
  items: ScheduledTask[];
  now_unix_ms: number;
  utc_offset_minutes: number;
}

export interface BackupList {
  items: BackupInfo[];
  keep: number;
  free_bytes: number | null;
  folder: string;
  restored: string | null;
}

export interface WorldOp {
  /** `install`: instalación de mods o plugins. */
  kind: "duplicate" | "export" | "import" | "install" | "backup" | "verify" | "restore";
  world: string;
  done_bytes: number;
  total_bytes: number;
}

export interface AdminState {
  maintenance: Maintenance | null;
  whitelist_on: boolean;
  enforce_whitelist: boolean;
  restart_in_seconds: number | null;
  world_op: WorldOp | null;
}

export type ContentKind = "mod" | "plugin" | "datapack";
export type ContentSource = "modrinth" | "curseforge" | "hangar";
export type Side = "required" | "optional" | "unsupported" | "unknown";
export type SearchSort = "relevance" | "downloads" | "updated" | "newest";

export interface Platform {
  kind: ContentKind;
  folder: string;
  loaders: string[];
  experimental: boolean;
  sources: ContentSource[];
}

export interface ContentHit {
  project_id: string;
  slug: string;
  title: string;
  author: string;
  description: string;
  icon_url: string | null;
  downloads: number;
  categories: string[];
  client_side: Side;
  server_side: Side;
  date_modified: string;
  license: string;
}

export interface SearchPage {
  hits: ContentHit[];
  total: number;
  offset: number;
}

export interface ContentProject {
  id: string;
  slug: string;
  title: string;
  description: string;
  body: string;
  icon_url: string | null;
  client_side: Side;
  server_side: Side;
  license: string;
  downloads: number;
  categories: string[];
  source_url: string | null;
  issues_url: string | null;
  wiki_url: string | null;
  page_url: string;
  distribution_allowed: boolean;
}

export interface ContentVersion {
  id: string;
  project_id: string;
  name: string;
  version_number: string;
  version_type: string;
  game_versions: string[];
  loaders: string[];
  date_published: string;
  downloads: number;
  changelog: string | null;
  dependencies: {
    version_id: string | null;
    project_id: string | null;
    file_name: string | null;
    dependency_type: "required" | "optional" | "incompatible" | "embedded";
  }[];
  files: { url: string; filename: string; primary: boolean; size: number; sha1: string | null; sha512: string | null; sha256: string | null }[];
  server_side: Side;
}

export interface PlanItem {
  project_id: string;
  title: string;
  icon_url: string | null;
  version_id: string;
  version_number: string;
  file_name: string;
  url: string;
  sha1: string;
  sha256: string | null;
  size: number;
  reason: string;
  replaces: string | null;
}

export interface ContentPlan {
  items: PlanItem[];
  already_installed: string[];
  optional: { project_id: string; title: string; needed_by: string }[];
  warnings: string[];
  blockers: string[];
  download_bytes: number;
}

export interface InstalledItem {
  file_name: string;
  enabled: boolean;
  size: number;
  sha1: string;
  project_id: string | null;
  version_id: string | null;
  title: string | null;
  version_number: string | null;
  icon_url: string | null;
  server_side: Side | null;
  /** Si los jugadores lo necesitan en su juego (dato de Modrinth). */
  client_side?: Side | null;
  /** Página oficial del proyecto. */
  page_url?: string | null;
  update: { version_id: string; version_number: string } | null;
  source: ContentSource | null;
  other_ids: string[];
}

export interface ContentList {
  platform: Platform;
  items: InstalledItem[];
  online: boolean;
  offline_reason: string | null;
}

export interface ActivityEntry {
  at_unix: number;
  actor: string;
  action: string;
  detail: string;
}

export type RequestBody =
  | { op: "ping" }
  | { op: "detect"; path: string }
  | { op: "find_java" }
  | { op: "import_instance"; name: string; path: string; java: string; min_memory_mb: number; max_memory_mb: number }
  | { op: "list_instances" }
  | { op: "status"; id: InstanceId }
  | { op: "start"; id: InstanceId }
  | { op: "stop"; id: InstanceId; timeout_secs: number }
  | { op: "restart"; id: InstanceId; stop_timeout_secs: number }
  | { op: "force_kill"; id: InstanceId; confirm: string }
  | { op: "send_command"; id: InstanceId; command: string }
  | { op: "console"; id: InstanceId; after: number; limit: number }
  | { op: "accept_eula"; id: InstanceId; accepted: boolean }
  | { op: "unregister"; id: InstanceId; delete_files: boolean }
  | { op: "metrics"; id: InstanceId }
  | { op: "system_info" }
  | { op: "catalog"; flavor: Flavor }
  | { op: "resolve"; flavor: Flavor; mc_version: string; loader_version: string | null }
  | { op: "modpack_search"; source: ContentSource; query: string; sort: SearchSort; offset: number }
  | { op: "modpack_project"; source: ContentSource; project: string }
  | { op: "modpack_inspect"; source: ContentSource; version_id: string }
  | { op: "creation_defaults" }
  | { op: "check_target"; path: string }
  | ({ op: "create_server" } & CreateServerArgs)
  | { op: "job"; job_id: string; after: number }
  | { op: "cancel_job"; job_id: string }
  | { op: "properties"; id: InstanceId }
  | { op: "set_properties"; id: InstanceId; changes: Record<string, string>; expected_modified: number | null }
  | { op: "set_resource_pack"; id: InstanceId; url: string; required: boolean }
  | { op: "players"; id: InstanceId }
  | { op: "player_action"; id: InstanceId; action: PlayerAction; name: string; reason: string | null }
  | { op: "list_files"; id: InstanceId; path: string }
  | { op: "read_file"; id: InstanceId; path: string }
  | { op: "write_file"; id: InstanceId; path: string; content: string; expected_modified: number | null }
  | { op: "make_dir"; id: InstanceId; path: string }
  | { op: "rename_file"; id: InstanceId; path: string; new_name: string }
  | { op: "resource_pack_search"; id: InstanceId; query: string; sort: SearchSort; offset: number }
  | { op: "resource_pack_project"; id: InstanceId; project: string }
  | { op: "modpack_status"; id: InstanceId }
  | { op: "modpack_update_preview"; id: InstanceId; version_id: string }
  | { op: "modpack_update_apply"; id: InstanceId; version_id: string; accept_loader_change: boolean }
  | { op: "modpack_link"; id: InstanceId; source: ContentSource; version_id: string }
  | { op: "modpack_unlink"; id: InstanceId }
  | { op: "move_file"; id: InstanceId; path: string; dest_dir: string }
  | { op: "copy_file"; id: InstanceId; path: string; dest_dir: string }
  | { op: "extract_zip"; id: InstanceId; path: string }
  | { op: "trash_file"; id: InstanceId; path: string }
  | { op: "upload_file"; id: InstanceId; source: string; dest_dir: string }
  | { op: "download_file"; id: InstanceId; path: string; dest: string }
  | { op: "export_console"; id: InstanceId; dest: string }
  | { op: "worlds"; id: InstanceId }
  | { op: "duplicate_world"; id: InstanceId; name: string; new_name: string }
  | { op: "export_world"; id: InstanceId; name: string; dest: string }
  | { op: "import_world"; id: InstanceId; source: string; new_name: string }
  | { op: "set_active_world"; id: InstanceId; name: string }
  | { op: "trash_world"; id: InstanceId; name: string }
  | { op: "saved_items"; id: InstanceId }
  | { op: "restore_saved"; id: InstanceId; item_id: string }
  | { op: "purge_saved"; id: InstanceId; item_id: string }
  | { op: "game_rules"; id: InstanceId }
  | { op: "set_game_rule"; id: InstanceId; key: string; value: string }
  | { op: "activity"; id: InstanceId; limit: number }
  | { op: "restart_with_notice"; id: InstanceId; seconds: number }
  | { op: "admin_state"; id: InstanceId }
  | { op: "set_maintenance"; id: InstanceId; on: boolean }
  | { op: "cancel_restart"; id: InstanceId }
  | { op: "cancel_world_op"; id: InstanceId }
  | { op: "backups"; id: InstanceId }
  | { op: "create_backup"; id: InstanceId; scope: BackupScope; note: string | null }
  | { op: "verify_backup"; id: InstanceId; backup: string }
  | { op: "restore_backup"; id: InstanceId; backup: string }
  | { op: "delete_backup"; id: InstanceId; backup: string }
  | { op: "protect_backup"; id: InstanceId; backup: string; protected: boolean }
  | { op: "set_backup_settings"; id: InstanceId; keep: number }
  | { op: "tasks"; id: InstanceId }
  | { op: "health"; id: InstanceId }
  | { op: "playit_status" }
  | { op: "playit_enable" }
  | { op: "playit_disable" }
  | { op: "playit_link" }
  | { op: "playit_unlink" }
  | { op: "playit_login_url" }
  | { op: "connection"; id: InstanceId }
  | { op: "test_address"; id: InstanceId; address: string }
  | { op: "test_bedrock_address"; id: InstanceId; address: string }
  | { op: "set_voice_config"; id: InstanceId; voice_host?: string | null; port?: number | null }
  | { op: "install_java"; major: number }
  | { op: "diagnose"; id: InstanceId }
  | { op: "server_update_check"; id: InstanceId }
  | { op: "apply_server_update"; id: InstanceId; loader_version: string }
  | { op: "loader_versions"; flavor: Flavor; mc_version: string }
  | { op: "clone_server"; id: InstanceId; name: string }
  | { op: "set_archived"; id: InstanceId; archived: boolean }
  | { op: "player_command"; id: InstanceId; name: string; command: PlayerCommand }
  | { op: "player_history"; id: InstanceId }
  | { op: "set_player_note"; id: InstanceId; name: string; note: string }
  | { op: "clear_player_history"; id: InstanceId }
  | { op: "addons"; id: InstanceId }
  | { op: "bedrock_players"; id: InstanceId }
  | { op: "bedrock_allowlist"; id: InstanceId; change: BedrockAllowChange }
  | { op: "bedrock_allowlist_enabled"; id: InstanceId; enabled: boolean }
  | { op: "bedrock_permission"; id: InstanceId; xuid: string; level: BedrockLevel | null }
  | { op: "addon_import"; id: InstanceId; source: string }
  | { op: "addon_set_active"; id: InstanceId; uuid: string; active: boolean }
  | { op: "addon_remove"; id: InstanceId; uuid: string }
  | { op: "mod_sides"; id: InstanceId }
  | { op: "quarantine_mods"; id: InstanceId; files: string[] }
  | { op: "restore_mods"; id: InstanceId; files: string[] }
  | { op: "friend_pack"; id: InstanceId }
  | { op: "export_friend_pack"; id: InstanceId; dest: string }
  | { op: "luck_perms"; id: InstanceId }
  | { op: "luck_perms_preview"; id: InstanceId; change: LpChange }
  | { op: "luck_perms_apply"; id: InstanceId; change: LpChange }
  | { op: "automation"; id: InstanceId }
  | { op: "set_automation"; id: InstanceId; auto_restart?: boolean | null; autostart?: boolean | null }
  | { op: "set_startup"; enabled: boolean }
  | { op: "set_keep_awake"; enabled: boolean }
  | { op: "supporter" }
  | { op: "activate_supporter"; license_key: string }
  | { op: "refresh_supporter" }
  | { op: "forget_supporter" }
  | { op: "server_icon"; id: InstanceId }
  | { op: "preview_server_icon"; source: string; fit: IconFit }
  | { op: "set_server_icon"; id: InstanceId; source: string; fit: IconFit }
  | { op: "remove_server_icon"; id: InstanceId }
  | { op: "security_report"; id: InstanceId }
  | { op: "set_launch"; id: InstanceId; java?: string | null; min_memory_mb?: number | null; max_memory_mb?: number | null }
  | { op: "companions" }
  | { op: "set_companions_enabled"; enabled: boolean }
  | { op: "create_invite"; id: InstanceId; perms: Perm[]; minutes: number }
  | { op: "create_host_invite"; minutes: number }
  | { op: "cancel_invite"; invite_id: string }
  | { op: "set_companion_perms"; companion: string; id: InstanceId; perms: Perm[] }
  | { op: "remove_companion"; companion: string }
  | { op: "join_with_code"; code: string; name: string }
  | { op: "remote_hosts" }
  | { op: "forget_host"; host: string }
  | { op: "refresh_host"; host: string }
  | { op: "remote_call"; host: string; request: RequestBody }
  | { op: "remote_download"; host: string; item: RemoteItem; dest: string }
  | { op: "remote_upload"; host: string; id: InstanceId; source: string; dest_dir: string }
  | { op: "download_backup"; id: InstanceId; backup_id: string; dest: string }
  | { op: "measure_tps"; id: InstanceId }
  | { op: "alerts" }
  | { op: "mark_alerts_read"; ids: string[] }
  | { op: "set_alert_settings"; settings: AlertSettings }
  | { op: "save_task"; id: InstanceId; task: TaskDraft }
  | { op: "delete_task"; id: InstanceId; task_id: string }
  | { op: "run_task_now"; id: InstanceId; task_id: string }
  | { op: "content_platforms"; id: InstanceId }
  | { op: "content_search"; id: InstanceId; kind: ContentKind; source: ContentSource; query: string; sort: SearchSort; offset: number }
  | { op: "content_project"; id: InstanceId; kind: ContentKind; source: ContentSource; project: string }
  | { op: "content_plan"; id: InstanceId; kind: ContentKind; source: ContentSource; version_id: string }
  | { op: "content_install"; id: InstanceId; kind: ContentKind; source: ContentSource; version_id: string }
  | { op: "content_installed"; id: InstanceId; kind: ContentKind }
  | { op: "content_toggle"; id: InstanceId; kind: ContentKind; file_name: string; enabled: boolean }
  | { op: "content_remove"; id: InstanceId; kind: ContentKind; file_name: string }
  | { op: "shutdown" };

export type ResponseBody =
  | { kind: "pong"; data: { server_version: string; supervisor_pid: number } }
  | { kind: "detected"; data: DetectedServer }
  | { kind: "java_candidates"; data: JavaCandidate[] }
  | { kind: "instance"; data: Instance }
  | { kind: "instances"; data: InstanceSummary[] }
  | { kind: "status"; data: StatusSnapshot }
  | { kind: "console"; data: { lines: ConsoleLine[]; next_after: number } }
  | { kind: "metrics"; data: Metrics }
  | { kind: "system"; data: SystemInfo }
  | { kind: "catalog"; data: CatalogEntry[] }
  | { kind: "resolved"; data: Resolved }
  | { kind: "creation_defaults"; data: { servers_dir: string; suggested_port: number; curseforge: boolean } }
  | { kind: "job"; data: JobSnapshot }
  | { kind: "properties"; data: { exists: boolean; entries: PropEntry[]; modified_unix_ms: number | null } }
  | { kind: "players"; data: { lists: PlayerLists; online: string[]; can_act: boolean } }
  | { kind: "files"; data: { path: string; entries: FileEntry[] } }
  | { kind: "text_file"; data: TextFile }
  | { kind: "file_done"; data: { path: string } }
  | { kind: "worlds"; data: { worlds: WorldInfo[]; level_name: string } }
  | { kind: "saved_items"; data: SavedItem[] }
  | { kind: "game_rules"; data: { rules: RuleValue[]; new_names: boolean } }
  | { kind: "activity"; data: ActivityEntry[] }
  | { kind: "admin_state"; data: AdminState }
  | { kind: "content_platforms"; data: Platform[] }
  | { kind: "backups"; data: BackupList }
  | { kind: "tasks"; data: TaskList }
  | { kind: "health"; data: HealthReport }
  | { kind: "playit"; data: PlayitInfo }
  | { kind: "playit_login_url"; data: string }
  | { kind: "connection"; data: ConnectionInfo }
  | { kind: "companions"; data: CompanionsInfo }
  | { kind: "diagnosis"; data: Diagnosis }
  | { kind: "server_update"; data: ServerUpdateInfo }
  | { kind: "server_icon"; data: ServerIconInfo }
  | { kind: "supporter"; data: SupporterInfo }
  | { kind: "loader_versions"; data: LoaderOption[] }
  | { kind: "automation"; data: AutomationInfo }
  | { kind: "player_history"; data: PlayerRecord[] }
  | { kind: "mod_sides"; data: ModSidesInfo }
  | { kind: "addons"; data: AddonsInfo }
  | { kind: "bedrock_players"; data: BedrockPlayersInfo }
  | { kind: "zip_extracted"; data: ZipExtracted }
  | { kind: "modpack_status"; data: ModpackStatusInfo }
  | { kind: "modpack_update_plan"; data: ModpackUpdatePlan }
  | { kind: "friend_pack"; data: FriendPackPlan }
  | { kind: "luck_perms"; data: LuckPermsInfo }
  | { kind: "luck_perms_preview"; data: LuckPermsPreview }
  | { kind: "luck_perms_applied"; data: LuckPermsApplied }
  | { kind: "icon_preview"; data: IconPreview }
  | { kind: "security_report"; data: SecurityReport }
  | { kind: "invite"; data: NewInvite }
  | { kind: "remote_hosts"; data: RemoteHost[] }
  | { kind: "tps"; data: TpsReading }
  | { kind: "alerts"; data: AlertList }
  | { kind: "content_search"; data: SearchPage }
  | { kind: "content_details"; data: { project: ContentProject; versions: ContentVersion[] } }
  | { kind: "content_plan"; data: ContentPlan }
  | { kind: "content_list"; data: ContentList }
  | { kind: "modpack_summary"; data: ModpackSummary }
  | { kind: "ok" };

export const FORCE_KILL_CONFIRMATION = "ENTIENDO_QUE_PUEDO_PERDER_DATOS";
export const PROTOCOL_VERSION = 1;

/** Información que añade la capa de escritorio (no viene del protocolo). */
export interface SupervisorInfo {
  server_version: string;
  supervisor_pid: number;
  data_dir: string;
}

export interface JavaChoice {
  candidates: JavaCandidate[];
  /** Ruta recomendada por merc_core::java::pick_java, o null si no hay ninguna compatible. */
  recommended: string | null;
  recommended_major: number;
}

/** Error devuelto por la capa de escritorio o por el supervisor. */
export class MercError extends Error {
  readonly code: ErrorCode | "unavailable";
  constructor(code: ErrorCode | "unavailable", message: string) {
    super(message);
    this.code = code;
    this.name = "MercError";
  }
}
