// Doble de pruebas: un supervisor en memoria para probar la interfaz sin Tauri.
// SOLO se usa en las pruebas; la aplicación real usa src/api/tauri.ts.

import type { FileDropEvent, MercApi } from "../api/client";
import type {
  AddonImportResult,
  AddonPack,
  AddonsInfo,
  BedrockPlayersInfo,
  AppUpdateCheck,
  BedrockInfo,
  FriendPackPlan,
  ModSidesInfo,
  ModpackStatusInfo,
  ModpackUpdatePlan,
  LpChange,
  LpSnapshot,
  LpTarget,
  ActivityEntry,
  AdminState,
  BackupInfo,
  IconFit,
  ServerIconInfo,
  ServerUpdateInfo,
  AutomationInfo,
  PlayerCommand,
  PlayerRecord,
  SupporterInfo,
  ScheduledTask,
  PlayitInfo,
  AddressTest,
  ConnLayer,
  Diagnosis,
  SecurityReport,
  CompanionsInfo,
  Perm,
  RemoteHost,
  VoiceInfo,
  Alert,
  AlertSettings,
  ContentHit,
  ContentKind,
  ContentList,
  ContentSource,
  ContentPlan,
  InstalledItem,
  Platform,
  RuleValue,
  SavedItem,
  WorldInfo,
  PlayerLists,
  PropEntry,
  CatalogEntry,
  ConsoleLine,
  CreateServerArgs,
  Flavor,
  JobSnapshot,
  DetectedServer,
  InstanceSummary,
  JavaChoice,
  RequestBody,
  ResponseBody,
  StatusSnapshot,
} from "../api/types";
import { FORCE_KILL_CONFIRMATION, MercError } from "../api/types";

function lpHolder(s: LpSnapshot, t: LpTarget) {
  const h = t.kind === "group" ? s.groups.find((g) => g.name === t.name) : s.users.find((u) => u.uuid === t.name);
  if (!h) throw new MercError("not_found", "no existe");
  return h;
}

function lpDescribe(s: LpSnapshot, c: LpChange): { command: string; summary: string } {
  const who = (t: LpTarget) => (t.kind === "group" ? `el grupo «${t.name}»` : (s.users.find((u) => u.uuid === t.name)?.username ?? t.name));
  const tc = (t: LpTarget) => `luckperms ${t.kind} ${t.name}`;
  switch (c.type) {
    case "create_group":
      if (s.groups.some((g) => g.name === c.group)) throw new MercError("conflict", `ya existe el grupo «${c.group}»`);
      return { command: `luckperms creategroup ${c.group}`, summary: `Crear el grupo «${c.group}» (sin permisos).` };
    case "delete_group":
      return { command: `luckperms deletegroup ${c.group}`, summary: `Borrar el grupo «${c.group}» con todos sus permisos.` };
    case "set_permission":
      lpHolder(s, c.target);
      return {
        command: `${tc(c.target)} permission set ${c.node} ${c.value}`,
        summary: c.value ? `Dar el permiso ${c.node} a ${who(c.target)}.` : `Negar el permiso ${c.node} a ${who(c.target)} (queda en false).`,
      };
    case "unset_permission":
      return { command: `${tc(c.target)} permission unset ${c.node}`, summary: `Quitar el permiso ${c.node} de ${who(c.target)} (vuelve a decidirlo la herencia).` };
    case "add_parent":
      return { command: `${tc(c.target)} parent add ${c.group}`, summary: c.target.kind === "group" ? `Hacer que ${who(c.target)} herede los permisos de «${c.group}».` : `Poner a ${who(c.target)} en el grupo «${c.group}».` };
    case "remove_parent":
      return { command: `${tc(c.target)} parent remove ${c.group}`, summary: c.target.kind === "group" ? `Hacer que ${who(c.target)} deje de heredar de «${c.group}».` : `Sacar a ${who(c.target)} del grupo «${c.group}».` };
  }
}

function lpApply(s: LpSnapshot, c: LpChange) {
  switch (c.type) {
    case "create_group":
      s.groups.push({ name: c.group, weight: null, prefix: null, parents: [], nodes: [] });
      break;
    case "delete_group":
      s.groups = s.groups.filter((g) => g.name !== c.group);
      break;
    case "set_permission": {
      const h = lpHolder(s, c.target);
      h.nodes = h.nodes.filter((n) => !(n.key === c.node && Object.keys(n.context).length === 0));
      h.nodes.push({ kind: "permission", key: c.node, value: c.value, expiry: null, context: {} });
      break;
    }
    case "unset_permission": {
      const h = lpHolder(s, c.target);
      h.nodes = h.nodes.filter((n) => !(n.key === c.node && JSON.stringify(n.context) === JSON.stringify(c.context)));
      break;
    }
    case "add_parent":
      lpHolder(s, c.target).parents.push(c.group);
      break;
    case "remove_parent": {
      const h = lpHolder(s, c.target);
      h.parents = h.parents.filter((p) => p !== c.group);
      break;
    }
  }
}

export const stoppedStatus = (): StatusSnapshot => ({
  state: "STOPPED",
  has_console: false,
  pid: null,
  port: null,
  started_at_unix_ms: null,
  online_at_unix_ms: null,
  last_exit: null,
  hint: null,
  log_file: null,
});

export function makeSummary(over: Partial<InstanceSummary["instance"]> = {}, status: Partial<StatusSnapshot> = {}): InstanceSummary {
  return {
    instance: {
      id: over.id ?? "11111111-1111-4111-8111-111111111111",
      name: over.name ?? "Survival",
      root: over.root ?? "C:\\Servidores\\survival",
      kind: over.kind ?? "neo_forge",
      mc_version: over.mc_version ?? "1.21.1",
      loader_version: over.loader_version ?? "21.1.259",
      launch: over.launch ?? {
        java: "C:\\Java21\\bin\\java.exe",
        min_memory_mb: 0,
        max_memory_mb: 4096,
        extra_jvm_args: [],
        target: { type: "args_file", args_file: "libraries/x/win_args.txt", user_jvm_args: "user_jvm_args.txt" },
      },
      imported: true,
      created_at_unix: 1,
    },
    status: { ...stoppedStatus(), ...status },
    eula_accepted: true,
    configured_port: 25565,
  };
}

export class FakeApi implements MercApi {
  servers: InstanceSummary[] = [];
  consoles = new Map<string, ConsoleLine[]>();
  calls: RequestBody[] = [];
  folder: string | null = "C:\\Servidores\\nuevo";
  detected: DetectedServer = {
    kind: "neo_forge",
    mc_version: "1.21.1",
    loader_version: "21.1.259",
    launch: { type: "args_file", args_file: "libraries/x/win_args.txt", user_jvm_args: "user_jvm_args.txt" },
    confidence: "verified",
    evidence: ["run.bat apunta a libraries/x/win_args.txt"],
    warnings: [],
  };
  java: JavaChoice = {
    candidates: [
      { path: "C:\\Java25\\bin\\java.exe", major: 25, description: "", error: null, jdk: true },
      { path: "C:\\Java21\\bin\\java.exe", major: 21, description: "", error: null, jdk: false },
    ],
    recommended: "C:\\Java21\\bin\\java.exe",
    recommended_major: 21,
  };
  javaInstallMajor: number | null = null;
  supporterState: SupporterInfo = {
    available: true,
    checkout_url: "https://example.gumroad.com/l/merchosting",
    price_usd: 8,
    active: false,
    email_hint: null,
    key_hint: null,
    purchased: null,
    checked_at_unix_ms: null,
    stale: false,
    revoked_reason: null,
    last_error: null,
  };
  /** Claves que el Gumroad falso acepta; el valor dice si se reembolsó. */
  gumroadKeys: Record<string, { refunded: boolean }> = { "ABCD1234-EFGH5678-IJKL9012-MNOP3456": { refunded: false } };
  serverUpdate: ServerUpdateInfo = {
    supported: true,
    reason: null,
    mc_version: "1.21.11",
    current: "98",
    latest: "130",
    newer: true,
    experimental: false,
    notes: [],
  };
  updating: string | null = null;
  cloneResult: InstanceSummary | null = null;
  automationState: AutomationInfo = {
    auto_restart: false,
    max_restarts: 3,
    recent_restarts: 0,
    gave_up: false,
    autostart: false,
    startup_available: true,
    startup_enabled: false,
    keep_awake: false,
    keep_awake_active: false,
  };
  autostartIds = new Set<string>();
  playerCommands: { name: string; command: PlayerCommand }[] = [];
  history: PlayerRecord[] = [];
  friendPackPlan: FriendPackPlan = {
    mc_version: "1.21.1",
    loader: ["neoforge", "21.1.259"],
    included: [],
    manual: [],
    server_only: [],
    blocked: "Ningún mod de este servidor se puede incluir en un pack (ninguno es de Modrinth para el juego).",
  };
  exportedPacks: string[] = [];
  /** LuckPerms de prueba: null = no instalado. */
  lp: LpSnapshot | null = null;
  lpJar = "LuckPerms-Bukkit-5.5.17.jar";
  lpBackups: string[] = [];
  lpApplied: LpChange[] = [];
  /** Si es false, el cambio no se refleja (para probar «sin confirmar»). */
  lpApplies = true;
  updateError: string | null = null;
  /** Imagen que devuelve el selector de archivos para el icono. */
  iconSource: string | null = "C:\\Users\\ana\\Pictures\\logo.jpg";
  icon: ServerIconInfo = { present: false, valid: false, width: null, height: null, png_base64: null };
  iconFits: IconFit[] = [];
  failNext: MercError | null = null;
  /** Falla la próxima llamada de esta operación (sin depender del orden de las consultas periódicas). */
  failOn: { op: RequestBody["op"]; error: MercError } | null = null;
  catalogs: Record<Flavor, CatalogEntry[]> = {
    vanilla: [
      { mc_version: "26.1.2", java_major: 25, latest: true },
      { mc_version: "1.21.11", java_major: 21, latest: false },
    ],
    paper: [
      { mc_version: "26.1.2", java_major: 25, latest: true },
      { mc_version: "1.21.11", java_major: 21, latest: false },
      { mc_version: "1.20.4", java_major: 17, latest: false },
    ],
    fabric: [{ mc_version: "1.21.11", java_major: 21, latest: true }],
    purpur: [{ mc_version: "1.21.11", java_major: 21, latest: true }],
    spigot: [
      { mc_version: "26.1.2", java_major: 25, latest: true },
      { mc_version: "1.21.11", java_major: 21, latest: false },
    ],
    quilt: [{ mc_version: "1.21.11", java_major: 21, latest: true }],
    forge: [{ mc_version: "1.20.1", java_major: 17, latest: true }],
    arclight: [
      { mc_version: "26.1.2", java_major: 25, latest: true },
      { mc_version: "1.21.1", java_major: 21, latest: false },
    ],
    neo_forge: [{ mc_version: "1.21.1", java_major: 21, latest: true }],
    bedrock: [{ mc_version: "1.21.124.2", java_major: 0, latest: true }],
  };
  catalogFails = 0;
  /** Carpetas que "ya tienen archivos". */
  busyFolders = new Set<string>();
  /** Si se define, el trabajo de creación termina con este error. */
  jobError: string | null = null;
  created: CreateServerArgs | null = null;
  private jobPolls = 0;
  private jobCancelled = false;
  lan: string | null = "192.168.1.20";
  players: string[] = [];
  openedUrls: string[] = [];

  // --- Administración ---
  props: PropEntry[] = [
    { key: "motd", value: "A Minecraft Server" },
    { key: "max-players", value: "20" },
    { key: "difficulty", value: "easy" },
    { key: "pvp", value: "true" },
    { key: "white-list", value: "false" },
    { key: "online-mode", value: "true" },
    { key: "view-distance", value: "10" },
    { key: "mod-especial", value: "x" },
  ];
  propsModified = 1000;
  lists: PlayerLists = {
    whitelist: [{ name: "Alex", uuid: "a", level: null, reason: null, expires: null }],
    ops: [{ name: "Steve", uuid: "s", level: 4, reason: null, expires: null }],
    banned: [{ name: "Griefer", uuid: "g", level: null, reason: "Romper casas", expires: "forever" }],
    banned_ips: [],
    warnings: [],
  };
  /** Árbol de archivos: ruta → contenido (null = carpeta). */
  files = new Map<string, string | null>([
    ["config", null],
    ["config/a.toml", "a = 1\n"],
    ["world", null],
    ["server.properties", "motd=Hola\n"],
  ]);
  worlds: WorldInfo[] = [
    { name: "world", size_bytes: 120 * 1024 ** 2, modified_unix_ms: 1_793_300_000_000, active: true, companions: ["world_nether", "world_the_end"] },
    { name: "creativo", size_bytes: 8 * 1024 ** 2, modified_unix_ms: 1_793_200_000_000, active: false, companions: [] },
  ];
  saved: SavedItem[] = [
    { id: "papelera/20261030-041500-123", kind: "trash", path: "mods/roto.jar", is_dir: false, size_bytes: 52_000, saved_unix_ms: 1_793_300_000_000 },
    { id: "copias/20261030-040000-001", kind: "copy", path: "server.properties", is_dir: false, size_bytes: 1200, saved_unix_ms: 1_793_290_000_000 },
  ];
  rules: RuleValue[] = [
    { key: "keep_inventory", name: "keepInventory", kind: { type: "bool" }, value: "false" },
    { key: "mob_griefing", name: "mobGriefing", kind: { type: "bool" }, value: "true" },
    { key: "random_tick_speed", name: "randomTickSpeed", kind: { type: "int", min: 0, max: 4096 }, value: "3" },
    { key: "spawn_phantoms", name: "doInsomnia", kind: { type: "bool" }, value: null },
  ];
  activity: ActivityEntry[] = [
    { at_unix: 1_793_300_100, actor: "local:merchosting-app", action: "server.start", detail: "" },
    { at_unix: 1_793_300_000, actor: "local:merchosting-app", action: "server.properties.set", detail: "max-players=12" },
  ];
  zipSource: string | null = "C:\\Users\\ana\\Downloads\\mapa.zip";
  uploadSource: string | null = "C:\\Users\\ana\\Downloads\\mod.jar";
  savePath: string | null = "C:\\Users\\ana\\Desktop\\copia.txt";

  /** Lo que devuelve el selector al elegir un addon de Bedrock. */
  addonSource: string | null = "C:\\Users\\ana\\Downloads\\dragones.mcaddon";
  /** Addons del servidor Bedrock simulado; `addonImports` = packs que trae cada archivo. */
  bedrockPlayersState: BedrockPlayersInfo = {
    allow_list_enabled: false,
    online_mode: true,
    default_permission: "member",
    legacy_whitelist: false,
    running: false,
    allowlist: [],
    permissions: [],
    known: [],
    online: [],
  };
  private bedrockPlayersData(id: string) {
    const s = this.find(id);
    if (s.instance.kind !== "bedrock") throw new MercError("bad_request", "Esta pantalla es para servidores Bedrock; en Java se usa la de siempre.");
    const st = this.bedrockPlayersState;
    st.running = s.status.state === "ONLINE";
    st.permissions = st.permissions.map((p) => ({ ...p, name: st.known.find((k) => k.xuid === p.xuid)?.name ?? null }));
    return { kind: "bedrock_players" as const, data: structuredClone(st) };
  }
  addonsState: AddonsInfo = { level_name: "Bedrock level", world_exists: true, packs: [], last_import: null };
  addonImports: Record<string, AddonPack[]> = {
    "C:\\Users\\ana\\Downloads\\dragones.mcaddon": [
      { uuid: "11111111-2222-3333-4444-555555555555", version: [1, 0, 0], name: "Dragones", description: "Dragones que vuelan", kind: "behavior", folder: "behavior_packs/Dragones_11111111", active: true, bundled: false, scripts: true },
      { uuid: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee", version: [1, 0, 0], name: "Dragones (texturas)", description: "", kind: "resource", folder: "resource_packs/Dragones_aaaaaaaa", active: true, bundled: false, scripts: false },
    ],
  };

  async pickFile(filter?: { name: string; extensions: string[] }) {
    if (filter?.extensions.includes("mcaddon")) return this.addonSource;
    if (filter?.extensions.includes("png")) return this.iconSource;
    return filter?.extensions.includes("zip") ? this.zipSource : this.uploadSource;
  }
  /** Iconos de ventana puestos (SVG) o null al volver al oficial. */
  windowIcons: (string | null)[] = [];
  appUpdate: AppUpdateCheck = { enabled: false, current: "0.1.0", available: false, version: null, notes: null, date: null };
  appUpdateInstalled = false;
  async appUpdateCheck() {
    return structuredClone(this.appUpdate);
  }
  async appUpdateInstall(onProgress: (d: number, t: number | null) => void) {
    if (!this.appUpdate.available) throw new MercError("not_found", "Ya tienes la versión más nueva.");
    const running = this.servers.filter((s) => this.running(s.instance.id)).map((s) => s.instance.name);
    if (running.length) throw new MercError("conflict", `Hay servidores en marcha: ${running.join(", ")}. Detenlos antes de cerrar el supervisor.`);
    onProgress(50, 100);
    onProgress(100, 100);
    this.appUpdateInstalled = true;
  }
  dropHandlers = new Set<(e: FileDropEvent) => void>();
  onFileDrop(handler: (e: FileDropEvent) => void) {
    this.dropHandlers.add(handler);
    return () => this.dropHandlers.delete(handler);
  }
  /** Simula arrastrar archivos del Explorador a la ventana. */
  dropFiles(e: FileDropEvent) {
    this.dropHandlers.forEach((h) => h(e));
  }
  async setWindowIcon(svg: string | null) {
    this.windowIcons.push(svg);
  }
  async pickSavePath() {
    return this.savePath;
  }

  private running(id: string) {
    const st = this.find(id).status.state;
    return ["STARTING", "LOADING", "ONLINE", "STOPPING", "UNKNOWN", "RECOVERING"].includes(st);
  }
  private requireStopped(id: string) {
    if (this.running(id)) throw new MercError("invalid_state", "Detén el servidor antes de modificar archivos.");
  }

  /** Mantiene el trabajo "en marcha" unas consultas más (para probar cancelar). */
  holdJob = false;

  private jobSnap(state: JobSnapshot["state"], step: JobSnapshot["step"], log: JobSnapshot["log"]): JobSnapshot {
    return {
      job_id: "job-1",
      name: this.created?.name ?? "",
      state,
      step,
      message: state === "running" ? "Descargando" : "Fin",
      download: null,
      log,
      next_after: log.length ? log[log.length - 1].seq : 0,
      instance_id: null,
      error: null,
    };
  }

  async info() {
    return { server_version: "0.1.0", supervisor_pid: 42, data_dir: "C:\\datos" };
  }
  async javaFor() {
    return this.java;
  }
  async pickFolder() {
    return this.folder;
  }
  async openUrl(url: string) {
    this.openedUrls.push(url);
  }
  openedBackupFolders: string[] = [];
  async openBackupsFolder(id: string) {
    this.openedBackupFolders.push(id);
  }
  playit: PlayitInfo = {
    supported: true,
    enabled: false,
    installed: false,
    agent_version: "1.2.0",
    running: false,
    agent: null,
    agent_error: null,
    claim: null,
  };
  portsInUse: { port: number; server: string }[] = [];
  /** Direcciones que responden como un servidor de Minecraft. */
  reachable = new Set<string>();
  private addressTests: AddressTest[] = [];
  /** Simula que la persona aprobó la vinculación y creó un túnel en playit.gg. */
  approveClaim(tunnelAddress = "dark-sea.joinmc.link", destination = "127.0.0.1:25565") {
    if (this.playit.claim) this.playit.claim = { ...this.playit.claim, state: "approved", done: true };
    this.playit.agent = {
      phase: "running",
      message: null,
      version: "1.2.0",
      account_status: "verified",
      tunnels: tunnelAddress ? [{ display_address: tunnelAddress, destination, is_disabled: false, disabled_reason: null }] : [],
      pending: [],
      notices: [],
    };
  }
  security: SecurityReport = { checked_at_unix_ms: 1_793_300_300_000, public_addresses: [], checks: [] };
  diagnosis: Diagnosis = { checked_at_unix_ms: 1_793_300_300_000, sources: ["mods/ (0 archivos)"], mods_scanned: 0, plugins_scanned: 0, findings: [] };
  /** Compañeros (lado anfitrión). */
  modpackState: ModpackStatusInfo = { linked: null, online: true, error: null, versions: [] };
  modpackPlan: ModpackUpdatePlan | null = null;
  modSidesState: ModSidesInfo = { items: [], quarantined: [], folder: "mods-cuarentena", catalog_ok: true };
  companionsState: CompanionsInfo = { enabled: false, online: false, fingerprint: "ABCD-EFGH-IJKL", error: null, invites: [], companions: [] };
  /** PC de un amigo simulada (lado compañero): otra FakeApi y los permisos concedidos. */
  friend: { fake: FakeApi; host: RemoteHost } | null = null;
  friendHosts: RemoteHost[] = [];
  private static readonly REMOTE_NEED: Record<string, Perm | "any" | "host"> = {
    ping: "any",
    list_instances: "any",
    status: "status",
    metrics: "status",
    admin_state: "status",
    health: "status",
    console: "console_read",
    send_command: "console_command",
    start: "start",
    stop: "stop",
    restart: "restart",
    restart_with_notice: "restart",
    players: "players",
    list_files: "files_list",
    read_file: "files_read",
    connection: "network",
    // Servidor casero (control total).
    find_java: "host",
    catalog: "host",
    resolve: "host",
    loader_versions: "host",
    creation_defaults: "host",
    modpack_search: "host",
    modpack_project: "host",
    modpack_inspect: "host",
    create_server: "host",
    install_java: "host",
    accept_eula: "host",
    // Playit del servidor casero (control total).
    playit_status: "host",
    playit_enable: "host",
    playit_disable: "host",
    playit_link: "host",
    playit_unlink: "host",
    playit_login_url: "host",
    job: "any",
  };
  /** Simple Voice Chat detectado (null = no instalado) y estado por capas. */
  voice: VoiceInfo | null = null;
  layers: ConnLayer[] = [];
  /** Destino real (tras SRV) que devuelve la prueba de una dirección; por defecto, el túnel. */
  testedAs: Record<string, string> = {};
  /** Geyser instalado (null = no) y direcciones de Bedrock que «responden». */
  bedrock: BedrockInfo | null = null;
  reachableBedrock = new Set<string>();
  voiceWrites: { voice_host?: string | null; port?: number | null }[] = [];
  private connectionInfo() {
    const port = Number(this.props.find((p) => p.key === "server-port")?.value ?? 25565);
    const tunnels = (this.playit.enabled ? (this.playit.agent?.tunnels ?? []) : []).filter((t) => t.destination.endsWith(`:${port}`));
    return {
      kind: "connection" as const,
      data: { port, lan: [`192.168.1.20:${port}`], tunnels, tests: structuredClone(this.addressTests), ports_in_use: structuredClone(this.portsInUse), voice: structuredClone(this.voice), layers: structuredClone(this.layers), bedrock: structuredClone(this.bedrock) },
    };
  }
  notifications: { title: string; body: string }[] = [];
  async notify(title: string, body: string) {
    this.notifications.push({ title, body });
    return true;
  }
  alertItems: Alert[] = [];
  alertSettings: AlertSettings = { notify: true, crash: true, disk: true, resources: true, tasks: true, disk_min_gb: 5, tunnel: true, backups: true, updates: true };
  private alertList() {
    return {
      kind: "alerts" as const,
      data: {
        items: structuredClone(this.alertItems),
        unread: this.alertItems.filter((a) => !a.read).length,
        settings: { ...this.alertSettings },
      },
    };
  }
  pcMemoryUsed = 9_000_000_000;
  taskItems: ScheduledTask[] = [];
  private taskSeq = 0;
  private taskList() {
    return { kind: "tasks" as const, data: { items: structuredClone(this.taskItems), now_unix_ms: 1_793_300_000_000, utc_offset_minutes: -300 } };
  }
  backupItems: BackupInfo[] = [];
  backupKeep = 10;
  /** Copias que «fallan» al verificarse. */
  damagedBackups = new Set<string>();
  private backupSeq = 0;
  private backupList(restored: string | null = null) {
    return {
      kind: "backups" as const,
      data: {
        items: structuredClone(this.backupItems),
        keep: this.backupKeep,
        free_bytes: 120_000_000_000,
        folder: "C:\\Users\\amir\\AppData\\Local\\MercHosting\\instancias\\x\\backups",
        restored,
      },
    };
  }
  private newBackup(scope: "world" | "full", reason: BackupInfo["reason"], hot: boolean, note: string | null): BackupInfo {
    this.backupSeq++;
    return {
      id: `20261030-1315${String(this.backupSeq).padStart(2, "0")}-000-${scope === "world" ? "mundo" : "completa"}`,
      created_unix_ms: 1_793_300_000_000 + this.backupSeq * 60_000,
      scope,
      reason,
      world: "world",
      mc_version: "1.21.1",
      size_bytes: 40_000_000,
      content_bytes: 60_000_000,
      files: 800,
      hot,
      consistent: true,
      verified: true,
      verified_unix_ms: 1_793_300_000_000 + this.backupSeq * 60_000,
      problems: [],
      protected: false,
      note,
    };
  }
  openedFolders: { id: string; rel: string }[] = [];
  async openServerFolder(id: string, rel: string) {
    this.openedFolders.push({ id, rel });
  }
  admin: AdminState = {
    maintenance: null,
    whitelist_on: false,
    enforce_whitelist: false,
    restart_in_seconds: null,
    world_op: null,
  };
  /** Carpetas mandadas a la Papelera de reciclaje al quitar servidores. */
  recycled: string[] = [];
  contentOffline = false;
  installedMods: InstalledItem[] = [
    {
      file_name: "jei-19.0.jar",
      enabled: true,
      size: 1_200_000,
      sha1: "a".repeat(40),
      project_id: "u6dRKJwZ",
      version_id: "jei19",
      title: "Just Enough Items",
      version_number: "19.0",
      icon_url: "https://cdn.modrinth.com/data/u6dRKJwZ/icon.png",
      server_side: "optional",
      update: { version_id: "jei20", version_number: "20.0" },
      source: "modrinth",
      other_ids: [],
    },
    {
      file_name: "viejo.jar.disabled",
      enabled: false,
      size: 50_000,
      sha1: "b".repeat(40),
      project_id: null,
      version_id: null,
      title: null,
      version_number: null,
      icon_url: null,
      server_side: null,
      update: null,
      source: null,
      other_ids: [],
    },
  ];
  /** Si la instalación tiene clave de CurseForge (los mods la ofrecen como segundo catálogo). */
  curseforge = false;
  contentHits: ContentHit[] = [
    {
      project_id: "P7dR8mSH",
      slug: "fabric-api",
      title: "Mapa Mundial",
      author: "lucia",
      description: "Un mapa del mundo en el navegador.",
      icon_url: null,
      downloads: 1_500_000,
      categories: ["utility"],
      client_side: "optional",
      server_side: "required",
      date_modified: "2026-10-01T00:00:00Z",
      license: "MIT",
    },
  ];
  contentPlanResult: ContentPlan = {
    items: [
      {
        project_id: "P7dR8mSH",
        title: "Mapa Mundial",
        icon_url: null,
        version_id: "map2",
        version_number: "2.0",
        file_name: "mapa-2.0.jar",
        url: "https://cdn.modrinth.com/x.jar",
        sha1: "c".repeat(40),
        sha256: null,
        size: 800_000,
        reason: "Lo elegiste tú",
        replaces: null,
      },
      {
        project_id: "LIB",
        title: "Librería Base",
        icon_url: null,
        version_id: "lib1",
        version_number: "1.0",
        file_name: "lib-1.0.jar",
        url: "https://cdn.modrinth.com/l.jar",
        sha1: "d".repeat(40),
        sha256: null,
        size: 200_000,
        reason: "Lo necesita «Mapa Mundial»",
        replaces: null,
      },
    ],
    already_installed: [],
    optional: [{ project_id: "OPT", title: "Extras del mapa", needed_by: "Mapa Mundial" }],
    warnings: [],
    blockers: [],
    download_bytes: 1_000_000,
  };

  private platformsFor(kind: string): Platform[] {
    const dp: Platform = { kind: "datapack", folder: "world/datapacks", loaders: ["datapack"], experimental: false, sources: ["modrinth"] };
    if (["paper", "purpur"].includes(kind))
      return [
        { kind: "plugin", folder: "plugins", loaders: ["paper", "spigot", "bukkit"], experimental: false, sources: ["modrinth", "hangar"] },
        dp,
      ];
    if (kind === "spigot")
      return [{ kind: "plugin", folder: "plugins", loaders: ["spigot", "bukkit"], experimental: false, sources: ["modrinth"] }, dp];
    const modSources: ContentSource[] = this.curseforge ? ["modrinth", "curseforge"] : ["modrinth"];
    if (["fabric", "quilt", "forge", "neo_forge"].includes(kind))
      return [
        { kind: "mod", folder: "mods", loaders: [kind === "neo_forge" ? "neoforge" : kind], experimental: false, sources: modSources },
        dp,
      ];
    if (kind === "arclight")
      return [
        { kind: "mod", folder: "mods", loaders: ["neoforge"], experimental: true, sources: modSources },
        { kind: "plugin", folder: "plugins", loaders: ["spigot", "bukkit"], experimental: true, sources: ["modrinth"] },
        dp,
      ];
    if (kind === "vanilla") return [dp];
    return [];
  }

  private contentList(id: string, kind: ContentKind): ContentList {
    const p = this.platformsFor(this.find(id).instance.kind).find((x) => x.kind === kind)!;
    const items = structuredClone(kind === "datapack" ? [] : this.installedMods).map((i) =>
      this.contentOffline ? { ...i, title: null, project_id: null, version_id: null, version_number: null, update: null } : i,
    );
    return { platform: p, items, online: !this.contentOffline, offline_reason: this.contentOffline ? "sin red" : null };
  }

  private find(id: string) {
    const s = this.servers.find((x) => x.instance.id === id);
    if (!s) throw new MercError("not_found", "No existe");
    return s;
  }

  async call(body: RequestBody): Promise<ResponseBody> {
    this.calls.push(body);
    if (this.failOn && this.failOn.op === body.op) {
      const e = this.failOn.error;
      this.failOn = null;
      throw e;
    }
    if (this.failNext) {
      const e = this.failNext;
      this.failNext = null;
      throw e;
    }
    switch (body.op) {
      case "list_instances":
        return { kind: "instances", data: structuredClone(this.servers) };
      case "detect":
        return { kind: "detected", data: this.detected };
      case "import_instance": {
        const s = makeSummary({ id: `id-${this.servers.length + 1}`, name: body.name, root: body.path });
        s.instance.launch.java = body.java;
        s.instance.launch.max_memory_mb = body.max_memory_mb;
        this.servers.push(s);
        return { kind: "instance", data: s.instance };
      }
      case "start": {
        const s = this.find(body.id);
        if (s.archived) throw new MercError("invalid_state", "Este servidor está archivado. Desarchívalo para encenderlo.");
        s.status = { ...s.status, state: "ONLINE", has_console: true, pid: 99, port: 25565, online_at_unix_ms: Date.now() };
        return { kind: "status", data: s.status };
      }
      case "stop": {
        const s = this.find(body.id);
        s.status = { ...stoppedStatus(), last_exit: { code: 0, forced: false, requested: true, at_unix_ms: 1 } };
        return { kind: "status", data: s.status };
      }
      case "force_kill": {
        if (body.confirm !== FORCE_KILL_CONFIRMATION) throw new MercError("bad_request", "confirmación");
        const s = this.find(body.id);
        s.status = { ...stoppedStatus(), last_exit: { code: null, forced: true, requested: true, at_unix_ms: 1 } };
        return { kind: "ok" };
      }
      case "accept_eula": {
        if (!body.accepted) throw new MercError("bad_request", "sin consentimiento");
        this.find(body.id).eula_accepted = true;
        return { kind: "ok" };
      }
      case "send_command": {
        const lines = this.consoles.get(body.id) ?? [];
        const seq = (lines[lines.length - 1]?.seq ?? 0) + 1;
        lines.push({ seq, at_unix_ms: 1, stream: "system", text: `> ${body.command}` });
        this.consoles.set(body.id, lines);
        return { kind: "ok" };
      }
      case "console": {
        const lines = (this.consoles.get(body.id) ?? []).filter((l) => l.seq > body.after);
        return { kind: "console", data: { lines, next_after: lines.length ? lines[lines.length - 1].seq : body.after } };
      }
      case "metrics": {
        const s = this.find(body.id);
        const on = ["STARTING", "LOADING", "ONLINE", "STOPPING", "UNKNOWN"].includes(s.status.state);
        return {
          kind: "metrics",
          data: {
            samples: on
              ? [
                  { at_unix_ms: 1, cpu_percent: 8, memory_bytes: 1.5 * 1024 ** 3 },
                  { at_unix_ms: 2, cpu_percent: 12.5, memory_bytes: 2 * 1024 ** 3 },
                ]
              : [],
            players: on && s.status.has_console ? this.players : [],
            max_players: 20,
          },
        };
      }
      case "system_info":
        return { kind: "system", data: { total_memory_bytes: 16 * 1024 ** 3, logical_cpus: 12, lan_ipv4: this.lan } };
      case "catalog":
        if (this.catalogFails > 0) {
          this.catalogFails--;
          throw new MercError("network", "sin conexión con la fuente oficial");
        }
        return { kind: "catalog", data: this.catalogs[body.flavor] };
      case "creation_defaults":
        return {
          kind: "creation_defaults",
          data: { servers_dir: "C:\\Users\\ana\\MercHosting\\Servidores", suggested_port: 25566, curseforge: this.curseforge },
        };
      case "check_target":
        if (this.busyFolders.has(body.path)) throw new MercError("conflict", `la carpeta no está vacía: ${body.path}`);
        return { kind: "ok" };
      case "modpack_search":
        return {
          kind: "content_search",
          data: {
            hits: [
              {
                project_id: "PACK1",
                slug: "aventura",
                title: "Aventura Épica",
                author: "maria",
                description: "Más de 200 mods de exploración.",
                icon_url: null,
                downloads: 2_000_000,
                categories: [],
                client_side: "required",
                server_side: "required",
                date_modified: "",
                license: "MIT",
              },
            ],
            total: 1,
            offset: body.offset,
          },
        };
      case "resource_pack_search":
        this.find(body.id);
        return {
          kind: "content_search",
          data: {
            hits: [
              {
                project_id: "RP1",
                slug: "texturas-suaves",
                title: "Texturas Suaves",
                author: "lu",
                description: "Texturas 32x fieles al original.",
                icon_url: null,
                downloads: 900_000,
                categories: ["32x"],
                client_side: "required",
                server_side: "unsupported",
                date_modified: "2026-10-01T00:00:00Z",
                license: "CC-BY-4.0",
              },
            ],
            total: 1,
            offset: 0,
          },
        };
      case "resource_pack_project": {
        this.find(body.id);
        const v = (id: string, num: string, mc: string[]) => ({
          id,
          project_id: "RP1",
          name: `Texturas Suaves ${num}`,
          version_number: num,
          version_type: "release",
          game_versions: mc,
          loaders: ["minecraft"],
          date_published: "2026-10-01T00:00:00Z",
          downloads: 5,
          changelog: null,
          dependencies: [],
          files: [{ url: `https://cdn.modrinth.com/data/RP1/versions/${id}/suaves-${num}.zip`, filename: `suaves-${num}.zip`, primary: true, size: 4_000_000, sha1: "a".repeat(40), sha512: null, sha256: null }],
          server_side: "unsupported" as const,
        });
        return {
          kind: "content_details",
          data: {
            project: {
              id: "RP1",
              slug: "texturas-suaves",
              title: "Texturas Suaves",
              description: "Texturas 32x fieles al original.",
              body: "",
              icon_url: null,
              client_side: "required",
              server_side: "unsupported",
              license: "CC-BY-4.0",
              downloads: 900_000,
              categories: [],
              source_url: null,
              issues_url: null,
              wiki_url: null,
              page_url: "https://modrinth.com/project/texturas-suaves",
              distribution_allowed: true,
            },
            versions: [v("rpv2", "2.0", ["1.21.1"]), v("rpv1", "1.0", ["1.20.1"])],
          },
        };
      }
      case "modpack_project":
        return {
          kind: "content_details",
          data: {
            project: {
              id: body.project,
              slug: "aventura",
              title: "Aventura Épica",
              description: "Más de 200 mods de exploración.",
              body: "",
              icon_url: null,
              client_side: "required",
              server_side: "required",
              license: "MIT",
              downloads: 2_000_000,
              categories: [],
              source_url: null,
              issues_url: null,
              wiki_url: null,
              page_url: "https://modrinth.com/modpack/aventura",
              distribution_allowed: true,
            },
            versions: [
              {
                id: "packv2",
                project_id: body.project,
                name: "Aventura 2.0",
                version_number: "2.0",
                version_type: "release",
                game_versions: ["1.21.1"],
                loaders: ["neoforge"],
                date_published: "2026-10-01T00:00:00Z",
                downloads: 5,
                changelog: null,
                dependencies: [],
                files: [],
                server_side: "unknown",
              },
            ],
          },
        };
      case "modpack_inspect":
        return {
          kind: "modpack_summary",
          data: {
            source: body.source,
            flavor: "neo_forge",
            name: "Aventura Épica",
            pack_version: "2.0",
            mc_version: "1.21.1",
            loader: "neoforge",
            loader_version: "21.1.77",
            java_major: 21,
            server_files: 210,
            client_only_files: 14,
            download_bytes: 480 * 1024 ** 2,
            warnings: ["14 archivos solo para el juego de cada jugador no se instalan en el servidor."],
          },
        };
      case "resolve":
        return {
          kind: "resolved",
          data: {
            flavor: body.flavor,
            mc_version: body.mc_version,
            loader_version: body.loader_version ?? (body.flavor === "paper" ? "42" : body.flavor === "fabric" ? "0.16.10" : null),
            java_major: 21,
            experimental: false,
            artifact: {
              url: "https://descarga.ejemplo.invalid/server.jar",
              mirrors: [],
              file_name: `paper-${body.mc_version}-42.jar`,
              checksum: { algorithm: "sha256", hex: "ab".repeat(32) },
              size: 50 * 1024 ** 2,
            },
            installer_args: null,
            installer_in_cache: false,
            needs_jdk: body.flavor === "spigot",
            notes: [],
          },
        };
      case "create_server": {
        const { op: _op, ...args } = body;
        this.created = args;
        this.jobPolls = 0;
        this.jobCancelled = false;
        return { kind: "job", data: this.jobSnap("running", "preparing", [{ seq: 1, text: "▸ Preparando la carpeta del servidor" }]) };
      }
      case "cancel_job":
        this.jobCancelled = true;
        return { kind: "ok" };
      case "install_java": {
        this.javaInstallMajor = body.major;
        return {
          kind: "job",
          data: { ...this.jobSnap("running", "downloading", []), job_id: `java-${body.major}`, name: `Java ${body.major}`, download: { file: "jre.zip", done: 10 * 1024 ** 2, total: 47 * 1024 ** 2 } },
        };
      }
      case "job": {
        if (body.job_id === "clone" && this.cloneResult) {
          const c = this.cloneResult;
          this.cloneResult = null;
          this.servers.push(c);
          return { kind: "job", data: { ...this.jobSnap("succeeded", "finishing", []), job_id: "clone", instance_id: c.instance.id, message: `«${c.instance.name}» listo` } };
        }
        if (body.job_id === "upd" && this.updating !== null) {
          const v = this.updating;
          this.updating = null;
          if (this.updateError)
            return { kind: "job", data: { ...this.jobSnap("failed", "downloading", []), job_id: "upd", error: this.updateError, message: "No se pudo actualizar; el servidor quedó como estaba" } };
          this.backupItems.unshift(this.newBackup("full", "before_update", false, `Antes de actualizar a ${v}`));
          this.serverUpdate = { ...this.serverUpdate, current: v, newer: false };
          return { kind: "job", data: { ...this.jobSnap("succeeded", "finishing", []), job_id: "upd", message: `Actualizado a ${v}. Copia previa: ${this.backupItems[0].id}` } };
        }
        if (body.job_id.startsWith("java-") && this.javaInstallMajor !== null) {
          const major = this.javaInstallMajor;
          this.javaInstallMajor = null;
          const path = `C:\\Users\\ana\\AppData\\Local\\MercHosting\\java\\jdk-${major}-jre\\bin\\java.exe`;
          this.java = { ...this.java, candidates: [...this.java.candidates, { path, major, description: "", error: null, jdk: false }] };
          return { kind: "job", data: { ...this.jobSnap("succeeded", "finishing", []), job_id: body.job_id, message: `Java ${major}.0.5+11 listo` } };
        }
        this.jobPolls++;
        if (this.jobCancelled) return { kind: "job", data: this.jobSnap("cancelled", "downloading", [{ seq: 9, text: "✔ cancelado" }]) };
        if (this.jobPolls === 1)
          return {
            kind: "job",
            data: {
              ...this.jobSnap("running", "downloading", [{ seq: 2, text: "▸ Descargando paper.jar" }]),
              download: { file: "paper.jar", done: 25 * 1024 ** 2, total: 50 * 1024 ** 2 },
            },
          };
        if (this.jobPolls < 4 && this.holdJob) return { kind: "job", data: this.jobSnap("running", "downloading", []) };
        if (this.jobError) return { kind: "job", data: { ...this.jobSnap("failed", "downloading", [{ seq: 3, text: `✖ ${this.jobError}` }]), error: this.jobError } };
        const s = makeSummary({ id: "id-creado", name: this.created!.name, root: this.created!.path, kind: "paper", mc_version: this.created!.mc_version });
        s.instance.imported = false;
        s.eula_accepted = this.created!.accept_eula;
        if (!this.servers.some((x) => x.instance.id === "id-creado")) this.servers.push(s);
        return { kind: "job", data: { ...this.jobSnap("succeeded", "finishing", [{ seq: 3, text: "✔ Servidor creado" }]), instance_id: "id-creado" } };
      }
      case "properties":
        return { kind: "properties", data: { exists: true, entries: structuredClone(this.props), modified_unix_ms: this.propsModified } };
      case "set_properties": {
        for (const [k, v] of Object.entries(body.changes))
          if (k.endsWith("distance") && !/^\d+$/.test(v)) throw new MercError("bad_request", `${k}: debe ser un número`);
        if (body.expected_modified !== this.propsModified) throw new MercError("conflict", "el archivo cambió desde que lo abriste");
        for (const [k, v] of Object.entries(body.changes)) {
          if (k === "max-players" && !/^\d+$/.test(v)) throw new MercError("bad_request", "max-players: debe ser un número");
          const e = this.props.find((p) => p.key === k);
          if (e) e.value = v;
          else this.props.push({ key: k, value: v });
        }
        this.propsModified++;
        return { kind: "properties", data: { exists: true, entries: structuredClone(this.props), modified_unix_ms: this.propsModified } };
      }
      case "set_resource_pack": {
        const url = body.url.trim();
        if (url && !url.startsWith("https://")) throw new MercError("bad_request", "La dirección del paquete debe empezar por https://");
        if (url.endsWith(".html")) throw new MercError("bad_request", "El archivo no es un .zip.");
        const put = (k: string, v: string) => {
          const e = this.props.find((p) => p.key === k);
          if (e) e.value = v;
          else this.props.push({ key: k, value: v });
        };
        put("resource-pack", url);
        put("resource-pack-sha1", url ? "0123456789abcdef0123456789abcdef01234567" : "");
        put("require-resource-pack", String(url !== "" && body.required));
        this.propsModified++;
        return { kind: "properties", data: { exists: true, entries: structuredClone(this.props), modified_unix_ms: this.propsModified } };
      }
      case "players": {
        const s = this.find(body.id);
        return {
          kind: "players",
          data: {
            lists: structuredClone(this.lists),
            online: s.status.state === "ONLINE" ? this.players : [],
            can_act: s.status.state === "ONLINE" && s.status.has_console,
          },
        };
      }
      case "player_action": {
        if (this.find(body.id).status.state !== "ONLINE") throw new MercError("invalid_state", "Enciende el servidor");
        const p = { name: body.name, uuid: "x", level: null, reason: body.reason, expires: null };
        if (body.action === "whitelist_add") this.lists.whitelist.push(p);
        if (body.action === "op") this.lists.ops.push({ ...p, level: 4 });
        if (body.action === "ban") this.lists.banned.push(p);
        if (body.action === "pardon") this.lists.banned = this.lists.banned.filter((b) => b.name !== body.name);
        if (body.action === "deop") this.lists.ops = this.lists.ops.filter((b) => b.name !== body.name);
        if (body.action === "whitelist_remove") this.lists.whitelist = this.lists.whitelist.filter((b) => b.name !== body.name);
        return { kind: "ok" };
      }
      case "list_files": {
        const prefix = body.path ? `${body.path}/` : "";
        const entries = [...this.files.entries()]
          .filter(([p]) => p.startsWith(prefix) && !p.slice(prefix.length).includes("/") && p !== body.path)
          .map(([p, c]) => ({
            name: p.slice(prefix.length),
            is_dir: c === null,
            size: c === null ? null : c.length,
            modified_unix_ms: 1_793_300_000_000,
            link: false,
          }))
          .sort((a, b) => Number(b.is_dir) - Number(a.is_dir) || a.name.localeCompare(b.name));
        return { kind: "files", data: { path: body.path, entries } };
      }
      case "read_file": {
        const c = this.files.get(body.path);
        if (c === undefined) throw new MercError("not_found", "no existe");
        if (c === null) throw new MercError("bad_request", "es una carpeta");
        return { kind: "text_file", data: { path: body.path, content: c, size: c.length, modified_unix_ms: 5 } };
      }
      case "write_file":
        this.requireStopped(body.id);
        this.files.set(body.path, body.content);
        return { kind: "text_file", data: { path: body.path, content: body.content, size: body.content.length, modified_unix_ms: 6 } };
      case "make_dir":
        this.requireStopped(body.id);
        this.files.set(body.path, null);
        return { kind: "file_done", data: { path: body.path } };
      case "rename_file": {
        this.requireStopped(body.id);
        const parent = body.path.includes("/") ? body.path.slice(0, body.path.lastIndexOf("/") + 1) : "";
        const to = parent + body.new_name;
        for (const [p, c] of [...this.files.entries()]) {
          if (p === body.path || p.startsWith(`${body.path}/`)) {
            this.files.delete(p);
            this.files.set(to + p.slice(body.path.length), c);
          }
        }
        return { kind: "file_done", data: { path: to } };
      }
      case "move_file":
      case "copy_file": {
        this.requireStopped(body.id);
        const name = body.path.split("/").pop()!;
        const to = body.dest_dir ? `${body.dest_dir}/${name}` : name;
        if (this.files.has(to)) throw new MercError("conflict", `ya existe «${name}» en esa carpeta`);
        if (to === body.path) throw new MercError("bad_request", "ya está en esa carpeta");
        if (to.startsWith(`${body.path}/`)) throw new MercError("bad_request", "no se puede meter una carpeta dentro de sí misma");
        for (const [p, c] of [...this.files.entries()]) {
          if (p === body.path || p.startsWith(`${body.path}/`)) {
            if (body.op === "move_file") this.files.delete(p);
            this.files.set(to + p.slice(body.path.length), c);
          }
        }
        return { kind: "file_done", data: { path: to } };
      }
      case "extract_zip": {
        this.requireStopped(body.id);
        const folder = body.path.replace(/\.zip$/i, "");
        if (this.files.has(folder)) throw new MercError("conflict", "ya existe una carpeta con ese nombre");
        this.files.set(folder, null);
        this.files.set(`${folder}/leeme.txt`, "hola");
        return { kind: "zip_extracted", data: { folder, files: 1, bytes: 4, skipped: ["../fuera.txt"] } };
      }
      case "trash_file":
        this.requireStopped(body.id);
        for (const p of [...this.files.keys()]) if (p === body.path || p.startsWith(`${body.path}/`)) this.files.delete(p);
        return { kind: "file_done", data: { path: `C:\\datos\\papelera\\${body.path}` } };
      case "upload_file": {
        this.requireStopped(body.id);
        const name = body.source.split(/[\\/]/).pop()!;
        const to = body.dest_dir ? `${body.dest_dir}/${name}` : name;
        if (this.files.has(to)) throw new MercError("conflict", `ya existe «${name}» en esa carpeta`);
        this.files.set(to, "jar");
        return { kind: "file_done", data: { path: to } };
      }
      case "download_file":
      case "export_console":
      case "download_backup":
        return { kind: "file_done", data: { path: body.dest } };
      case "remote_download":
      case "remote_upload": {
        const f = this.friend;
        if (!f || !this.friendHosts.some((h) => h.host === body.host)) throw new MercError("not_found", "No tienes esa PC guardada.");
        const sid = body.op === "remote_upload" ? body.id : body.item.id;
        const need: Perm =
          body.op === "remote_upload"
            ? "files_edit"
            : ({ file: "files_read", world: "world", backup: "backups", console: "console_read" } as const)[body.item.type];
        const server = f.host.servers.find((x) => x.id === sid);
        if (!server || !server.perms.includes(need)) throw new MercError("forbidden", "No tienes permiso para esto.");
        return { kind: "file_done", data: { path: body.op === "remote_upload" ? body.source : body.dest } };
      }
      case "worlds":
        return { kind: "worlds", data: { worlds: structuredClone(this.worlds), level_name: this.worlds.find((w) => w.active)?.name ?? "world" } };
      case "duplicate_world": {
        this.requireStopped(body.id);
        if (this.worlds.some((w) => w.name === body.new_name)) throw new MercError("conflict", `ya existe una carpeta «${body.new_name}»`);
        const src = this.worlds.find((w) => w.name === body.name)!;
        this.worlds.push({ ...src, name: body.new_name, active: false, companions: src.companions.map((c) => c.replace(src.name, body.new_name)) });
        return { kind: "worlds", data: { worlds: structuredClone(this.worlds), level_name: this.worlds.find((w) => w.active)!.name } };
      }
      case "export_world":
        this.requireStopped(body.id);
        return { kind: "file_done", data: { path: body.dest } };
      case "import_world":
        this.requireStopped(body.id);
        this.worlds.push({ name: body.new_name, size_bytes: 1000, modified_unix_ms: 1, active: false, companions: [] });
        return { kind: "worlds", data: { worlds: structuredClone(this.worlds), level_name: this.worlds.find((w) => w.active)!.name } };
      case "set_active_world":
        this.requireStopped(body.id);
        for (const w of this.worlds) w.active = w.name === body.name;
        return { kind: "worlds", data: { worlds: structuredClone(this.worlds), level_name: body.name } };
      case "trash_world": {
        this.requireStopped(body.id);
        if (this.worlds.find((w) => w.name === body.name)?.active) throw new MercError("conflict", "Es el mundo activo.");
        this.worlds = this.worlds.filter((w) => w.name !== body.name);
        return { kind: "worlds", data: { worlds: structuredClone(this.worlds), level_name: this.worlds.find((w) => w.active)!.name } };
      }
      case "saved_items":
        return { kind: "saved_items", data: structuredClone(this.saved) };
      case "restore_saved": {
        this.requireStopped(body.id);
        const it = this.saved.find((i) => i.id === body.item_id)!;
        if (it.kind === "trash") this.saved = this.saved.filter((i) => i.id !== body.item_id);
        return { kind: "file_done", data: { path: it.path } };
      }
      case "purge_saved":
        this.saved = this.saved.filter((i) => i.id !== body.item_id);
        return { kind: "ok" };
      case "game_rules":
        if (this.find(body.id).status.state !== "ONLINE") throw new MercError("invalid_state", "Enciende el servidor");
        return { kind: "game_rules", data: { rules: structuredClone(this.rules), new_names: false } };
      case "set_game_rule": {
        const r = this.rules.find((x) => x.key === body.key)!;
        r.value = body.value;
        return { kind: "ok" };
      }
      case "activity":
        return { kind: "activity", data: structuredClone(this.activity) };
      case "restart_with_notice":
        this.admin.restart_in_seconds = body.seconds;
        return { kind: "ok" };
      case "admin_state":
        return { kind: "admin_state", data: structuredClone(this.admin) };
      case "set_maintenance":
        if (body.on && this.find(body.id).status.state !== "ONLINE") throw new MercError("invalid_state", "Enciende el servidor");
        this.admin.maintenance = body.on ? { since_unix: 1, previous_whitelist: this.admin.whitelist_on } : null;
        return { kind: "admin_state", data: structuredClone(this.admin) };
      case "cancel_restart":
        this.admin.restart_in_seconds = null;
        return { kind: "admin_state", data: structuredClone(this.admin) };
      case "cancel_world_op":
        this.admin.world_op = null;
        return { kind: "admin_state", data: structuredClone(this.admin) };
      case "backups":
        return this.backupList();
      case "create_backup": {
        const st = this.find(body.id).status.state;
        if (st !== "STOPPED" && st !== "ONLINE") throw new MercError("invalid_state", "Espera a que el servidor termine de encender");
        this.backupItems.unshift(this.newBackup(body.scope, "manual", st === "ONLINE", body.note));
        const free = this.backupItems.filter((b) => !b.protected);
        const drop = new Set(free.slice(this.backupKeep).map((b) => b.id));
        this.backupItems = this.backupItems.filter((b) => !drop.has(b.id));
        return this.backupList();
      }
      case "verify_backup": {
        const b = this.backupItems.find((x) => x.id === body.backup);
        if (!b) throw new MercError("not_found", "No existe la copia");
        const bad = this.damagedBackups.has(b.id);
        b.verified = !bad;
        b.problems = bad ? ["«world/level.dat» no coincide con su huella"] : [];
        return this.backupList();
      }
      case "restore_backup": {
        if (this.find(body.id).status.state !== "STOPPED")
          throw new MercError("invalid_state", "Detén el servidor antes de restaurar una copia: podría estar usando esos archivos.");
        const b = this.backupItems.find((x) => x.id === body.backup);
        if (!b) throw new MercError("not_found", "No existe la copia");
        if (this.damagedBackups.has(b.id)) throw new MercError("conflict", "La copia está dañada y no se restauró");
        this.backupItems.unshift(this.newBackup(b.scope, "before_restore", false, `Antes de restaurar ${b.id}`));
        return this.backupList(b.id);
      }
      case "delete_backup": {
        const b = this.backupItems.find((x) => x.id === body.backup);
        if (!b) throw new MercError("not_found", "No existe la copia");
        if (b.protected) throw new MercError("conflict", "Esta copia está protegida: quita la protección antes de borrarla.");
        this.backupItems = this.backupItems.filter((x) => x.id !== b.id);
        return this.backupList();
      }
      case "protect_backup": {
        const b = this.backupItems.find((x) => x.id === body.backup);
        if (!b) throw new MercError("not_found", "No existe la copia");
        b.protected = body.protected;
        return this.backupList();
      }
      case "tasks":
        return this.taskList();
      case "playit_status":
        return { kind: "playit", data: structuredClone(this.playit) };
      case "playit_enable":
        this.playit = {
          ...this.playit,
          enabled: true,
          installed: true,
          running: true,
          agent: this.playit.agent ?? { phase: "waiting_for_secret", message: null, version: null, account_status: null, tunnels: [], pending: [], notices: [] },
        };
        return { kind: "playit", data: structuredClone(this.playit) };
      case "playit_disable":
        this.playit = { ...this.playit, enabled: false, running: false };
        return { kind: "playit", data: structuredClone(this.playit) };
      case "playit_link":
        if (!this.playit.enabled) throw new MercError("invalid_state", "Activa Playit antes de vincular tu cuenta.");
        this.playit.claim = { url: "https://playit.gg/claim/ab01ff1000", state: "waiting_for_visit", done: false, error: null };
        return { kind: "playit", data: structuredClone(this.playit) };
      case "playit_unlink":
        this.playit.claim = null;
        this.playit.agent = { phase: "waiting_for_secret", message: null, version: null, account_status: null, tunnels: [], pending: [], notices: [] };
        return { kind: "playit", data: structuredClone(this.playit) };
      case "playit_login_url":
        return { kind: "playit_login_url", data: "https://playit.gg/login/guest-account/abc" };
      case "connection":
        return this.connectionInfo();
      case "companions":
        return { kind: "companions", data: structuredClone(this.companionsState) };
      case "security_report":
        this.find(body.id);
        return { kind: "security_report", data: structuredClone(this.security) };
      case "loader_versions":
        return {
          kind: "loader_versions",
          data:
            body.flavor === "fabric"
              ? [
                  { version: "0.17.0-beta.1", stable: false, recommended: false },
                  { version: "0.16.10", stable: true, recommended: true },
                  { version: "0.16.9", stable: true, recommended: false },
                ]
              : [],
        };
      case "clone_server": {
        const s = this.find(body.id);
        if (this.running(body.id)) throw new MercError("invalid_state", "Detén el servidor antes de clonarlo (así el mundo queda guardado del todo).");
        const copy = makeSummary({ ...s.instance, id: "id-clon", name: body.name, root: `${s.instance.root} (copia)` });
        copy.configured_port = (s.configured_port ?? 25565) + 1;
        this.cloneResult = copy;
        return { kind: "job", data: { ...this.jobSnap("running", "preparing", []), job_id: "clone", message: "Copiando", download: { file: "copia", done: 10 * 1024 ** 2, total: 80 * 1024 ** 2 } } };
      }
      case "set_archived": {
        const s = this.find(body.id);
        if (body.archived && this.running(body.id)) throw new MercError("invalid_state", "Detén el servidor antes de archivarlo.");
        s.archived = body.archived;
        return { kind: "ok" };
      }
      case "player_command": {
        this.find(body.id);
        if (!this.running(body.id)) throw new MercError("invalid_state", "Enciende el servidor: estas acciones las hace Minecraft.");
        if (!this.players.includes(body.name)) throw new MercError("invalid_state", `${body.name} no está conectado ahora.`);
        this.playerCommands.push({ name: body.name, command: body.command });
        return { kind: "ok" };
      }
      case "player_history":
        return { kind: "player_history", data: structuredClone(this.history) };
      case "set_player_note": {
        const r = this.history.find((h) => h.name === body.name);
        if (r) r.note = body.note.trim() || null;
        return { kind: "player_history", data: structuredClone(this.history) };
      }
      case "clear_player_history":
        this.history = [];
        return { kind: "player_history", data: [] };
      case "modpack_status":
        this.find(body.id);
        return { kind: "modpack_status", data: structuredClone(this.modpackState) };
      case "modpack_link": {
        this.find(body.id);
        this.modpackState.linked = {
          source: body.source,
          project_id: "PACK1",
          title: "Aventura Épica",
          version_id: body.version_id,
          version_number: "2.0",
          mc_version: "1.21.1",
          loader: "neoforge",
          loader_version: "21.1.77",
          installed_at_unix: 1_793_300_000,
          files: 210,
        };
        return { kind: "modpack_status", data: structuredClone(this.modpackState) };
      }
      case "modpack_unlink":
        this.modpackState = { linked: null, online: true, error: null, versions: [] };
        return { kind: "modpack_status", data: structuredClone(this.modpackState) };
      case "modpack_update_preview":
        if (!this.modpackPlan) throw new MercError("not_found", "No hay plan");
        return { kind: "modpack_update_plan", data: structuredClone(this.modpackPlan) };
      case "modpack_update_apply": {
        this.requireStopped(body.id);
        if (this.modpackPlan?.loader_mismatch && !body.accept_loader_change)
          throw new MercError("conflict", "La versión nueva pide otra versión del loader. Confírmalo para seguir.");
        const st = this.modpackState;
        if (st.linked) {
          const v = st.versions.find((x) => x.version_id === body.version_id);
          st.linked = { ...st.linked, version_id: body.version_id, version_number: v?.version_number ?? "?" };
          st.versions = st.versions.map((x) => ({ ...x, current: x.version_id === body.version_id, newer: false }));
        }
        return {
          kind: "job",
          data: {
            job_id: "job-modpack",
            name: "Actualizar modpack",
            state: "succeeded",
            step: null,
            message: "Aventura Épica actualizado a 2.0: 3 archivos nuevos o cambiados, 1 quitados, 2 respetados por tus cambios. Copia previa: 20261031",
            download: null,
            log: [],
            next_after: 0,
            instance_id: body.id,
            error: null,
          },
        };
      }
      case "bedrock_players":
        return this.bedrockPlayersData(body.id);
      case "bedrock_allowlist": {
        const st = this.bedrockPlayersState;
        const c = body.change;
        const i = st.allowlist.findIndex((a) => a.name.toLowerCase() === c.name.trim().toLowerCase());
        if (c.type === "add") {
          if (!/^[\p{L}\p{N} _#.-]{1,32}$/u.test(c.name.trim())) throw new MercError("bad_request", "gamertag no válido");
          if (i >= 0) st.allowlist[i].ignores_player_limit = c.ignores_player_limit;
          else st.allowlist.push({ name: c.name.trim(), xuid: null, ignores_player_limit: c.ignores_player_limit });
        } else if (i < 0) throw new MercError("not_found", `no existe: ${c.name}`);
        else if (c.type === "remove") st.allowlist.splice(i, 1);
        else st.allowlist[i].ignores_player_limit = c.value;
        return this.bedrockPlayersData(body.id);
      }
      case "bedrock_allowlist_enabled":
        this.bedrockPlayersState.allow_list_enabled = body.enabled;
        return this.bedrockPlayersData(body.id);
      case "bedrock_permission": {
        const st = this.bedrockPlayersState;
        st.permissions = st.permissions.filter((p) => p.xuid !== body.xuid);
        if (body.level) st.permissions.push({ xuid: body.xuid, level: body.level, name: null });
        return this.bedrockPlayersData(body.id);
      }
      case "addons":
        if (this.find(body.id).instance.kind !== "bedrock") throw new MercError("bad_request", "Los addons son para servidores Bedrock.");
        return { kind: "addons", data: { ...structuredClone(this.addonsState), last_import: null } };
      case "addon_import": {
        this.requireStopped(body.id);
        const packs = this.addonImports[body.source];
        if (!packs) throw new MercError("bad_request", "no se encontró ningún behavior pack ni resource pack dentro");
        const r: AddonImportResult = { installed: [], unchanged: [], replaced: [], notes: [] };
        for (const p of packs) {
          const label = `${p.name} ${p.version.join(".")}`;
          const old = this.addonsState.packs.find((x) => x.uuid === p.uuid);
          if (old && old.version.join(".") === p.version.join(".")) r.unchanged.push(label);
          else {
            if (old) r.replaced.push(label);
            else r.installed.push(label);
            this.addonsState.packs = [...this.addonsState.packs.filter((x) => x.uuid !== p.uuid), structuredClone(p)];
          }
          if (p.scripts) r.notes.push(`«${p.name}» usa scripts: si no funciona, puede necesitar experimentos o APIs beta activados en el mundo.`);
        }
        return { kind: "addons", data: { ...structuredClone(this.addonsState), last_import: r } };
      }
      case "addon_set_active": {
        this.requireStopped(body.id);
        const p = this.addonsState.packs.find((x) => x.uuid === body.uuid);
        if (!p) throw new MercError("not_found", "pack");
        p.active = body.active;
        return { kind: "addons", data: { ...structuredClone(this.addonsState), last_import: null } };
      }
      case "addon_remove":
        this.requireStopped(body.id);
        this.addonsState.packs = this.addonsState.packs.filter((x) => x.uuid !== body.uuid);
        return { kind: "addons", data: { ...structuredClone(this.addonsState), last_import: null } };
      case "mod_sides":
        this.find(body.id);
        return { kind: "mod_sides", data: structuredClone(this.modSidesState) };
      case "quarantine_mods":
      case "restore_mods": {
        this.find(body.id);
        if (this.running(body.id)) throw new MercError("invalid_state", "Detén el servidor para mover mods.");
        const st = this.modSidesState;
        if (body.op === "quarantine_mods") {
          for (const f of body.files) {
            const it = st.items.find((i) => i.file === f);
            if (!it) throw new MercError("not_found", `No existe ${f} en mods.`);
            st.items = st.items.filter((i) => i.file !== f);
            st.quarantined.push({ file: f, moved_at_unix: 1_700_000_000, reason: "Solo cliente" });
          }
        } else {
          for (const f of body.files) {
            if (!st.quarantined.some((q) => q.file === f)) throw new MercError("not_found", `${f} no está en la cuarentena.`);
            st.quarantined = st.quarantined.filter((q) => q.file !== f);
            st.items.push({ file: f, name: f.replace(/\.jar$/, ""), enabled: true, side: "unknown", confidence: "baja", evidence: [], needed_by: [], suggested: false });
          }
        }
        return { kind: "mod_sides", data: structuredClone(st) };
      }
      case "friend_pack":
        this.find(body.id);
        return { kind: "friend_pack", data: structuredClone(this.friendPackPlan) };
      case "export_friend_pack":
        this.find(body.id);
        if (this.friendPackPlan.blocked) throw new MercError("bad_request", this.friendPackPlan.blocked);
        if (!body.dest.toLowerCase().endsWith(".mrpack")) throw new MercError("bad_request", "El pack se guarda como un archivo .mrpack.");
        this.exportedPacks.push(body.dest);
        return { kind: "file_done", data: { path: body.dest } };
      case "luck_perms": {
        this.find(body.id);
        const online = this.running(body.id);
        return {
          kind: "luck_perms",
          data: {
            install: this.lp ? { jar: this.lpJar, data_dir: "plugins/LuckPerms" } : null,
            online,
            snapshot: this.lp && online ? structuredClone(this.lp) : null,
            backups: [...this.lpBackups],
          },
        };
      }
      case "luck_perms_preview":
      case "luck_perms_apply": {
        this.find(body.id);
        if (!this.lp) throw new MercError("not_found", "LuckPerms no está instalado en este servidor.");
        if (!this.running(body.id)) throw new MercError("invalid_state", "Enciende el servidor: los permisos los lee LuckPerms.");
        const { command, summary } = lpDescribe(this.lp, body.change);
        if (body.op === "luck_perms_preview") return { kind: "luck_perms_preview", data: { command, summary } };
        const backup = `2026-10-30-12-00-0${this.lpBackups.length}.json.gz`;
        this.lpBackups.unshift(backup);
        this.lpApplied.push(body.change);
        if (this.lpApplies) lpApply(this.lp, body.change);
        return { kind: "luck_perms_applied", data: { verified: this.lpApplies, command, backup, snapshot: structuredClone(this.lp) } };
      }
      case "automation":
        this.find(body.id);
        return { kind: "automation", data: { ...this.automationState, autostart: this.autostartIds.has(body.id) } };
      case "set_automation":
        this.find(body.id);
        if (body.auto_restart != null) this.automationState = { ...this.automationState, auto_restart: body.auto_restart, gave_up: false };
        if (body.autostart != null) {
          if (body.autostart) this.autostartIds.add(body.id);
          else this.autostartIds.delete(body.id);
        }
        return { kind: "automation", data: { ...this.automationState, autostart: this.autostartIds.has(body.id) } };
      case "set_startup":
        this.automationState = { ...this.automationState, startup_enabled: body.enabled };
        return { kind: "ok" };
      case "set_keep_awake":
        this.automationState = { ...this.automationState, keep_awake: body.enabled, keep_awake_active: body.enabled && this.servers.some((x) => x.status.state === "ONLINE") };
        return { kind: "ok" };
      case "supporter":
        return { kind: "supporter", data: structuredClone(this.supporterState) };
      case "activate_supporter": {
        const key = body.license_key.trim();
        if (!this.supporterState.available) throw new MercError("invalid_state", "Supporter Edition todavía no está a la venta.");
        const k = this.gumroadKeys[key];
        if (!k) throw new MercError("bad_request", "Gumroad dice: That license does not exist for the provided product.");
        if (k.refunded) throw new MercError("bad_request", "La compra se reembolsó.");
        this.supporterState = { ...this.supporterState, active: true, email_hint: "a•••@gmail.com", key_hint: `…${key.slice(-4)}`, purchased: "2026-10-30", checked_at_unix_ms: Date.now(), stale: false, revoked_reason: null, last_error: null };
        return { kind: "supporter", data: structuredClone(this.supporterState) };
      }
      case "refresh_supporter": {
        const hint = this.supporterState.key_hint;
        const entry = Object.entries(this.gumroadKeys).find(([key]) => hint && key.endsWith(hint.slice(1)));
        if (entry?.[1].refunded) this.supporterState = { ...this.supporterState, active: false, revoked_reason: "La compra se reembolsó.", stale: false };
        else this.supporterState = { ...this.supporterState, stale: false, checked_at_unix_ms: Date.now() };
        return { kind: "supporter", data: structuredClone(this.supporterState) };
      }
      case "forget_supporter":
        this.supporterState = { ...this.supporterState, active: false, email_hint: null, key_hint: null, purchased: null, checked_at_unix_ms: null, revoked_reason: null };
        return { kind: "supporter", data: structuredClone(this.supporterState) };
      case "server_update_check":
        this.find(body.id);
        return { kind: "server_update", data: structuredClone(this.serverUpdate) };
      case "apply_server_update":
        if (this.running(body.id)) throw new MercError("invalid_state", "Detén el servidor antes de actualizar el servidor.");
        this.updating = body.loader_version;
        return { kind: "job", data: { ...this.jobSnap("running", "preparing", []), job_id: "upd", message: "Haciendo una copia completa antes de actualizar" } };
      case "server_icon":
        this.find(body.id);
        return { kind: "server_icon", data: structuredClone(this.icon) };
      case "preview_server_icon":
        this.iconFits.push(body.fit);
        if (body.source.endsWith(".txt")) throw new MercError("bad_request", "No se reconoce la imagen. Usa PNG, JPG, GIF, WebP, BMP, ICO o TIFF.");
        return { kind: "icon_preview", data: { png_base64: `PREVIEW-${body.fit}`, source_width: 1920, source_height: 1080, format: "JPG" } };
      case "set_server_icon":
        this.find(body.id);
        this.icon = { present: true, valid: true, width: 64, height: 64, png_base64: `SAVED-${body.fit}` };
        return { kind: "server_icon", data: structuredClone(this.icon) };
      case "remove_server_icon":
        this.icon = { present: false, valid: false, width: null, height: null, png_base64: null };
        return { kind: "server_icon", data: structuredClone(this.icon) };
      case "diagnose":
        this.find(body.id);
        return { kind: "diagnosis", data: structuredClone(this.diagnosis) };
      case "set_launch": {
        const s = this.find(body.id);
        if (this.running(body.id)) throw new MercError("invalid_state", "Detén el servidor antes de cambiar el Java o la memoria.");
        if (body.max_memory_mb != null) s.instance.launch.max_memory_mb = body.max_memory_mb;
        if (body.java != null) s.instance.launch.java = body.java;
        return { kind: "instance", data: structuredClone(s.instance) };
      }
      case "set_companions_enabled":
        this.companionsState.enabled = body.enabled;
        this.companionsState.online = body.enabled;
        return { kind: "companions", data: structuredClone(this.companionsState) };
      case "create_invite": {
        if (!this.companionsState.enabled) throw new MercError("invalid_state", "Activa «Permitir compañeros» antes de invitar.");
        const s = this.find(body.id);
        const invite = {
          invite_id: `inv${this.companionsState.invites.length + 1}`,
          server: body.id,
          server_name: s.instance.name,
          perms: [...body.perms].sort(),
          created_at_unix_ms: 1_793_300_300_000,
          expires_at_unix_ms: 1_793_300_300_000 + body.minutes * 60_000,
          host_admin: false,
        };
        this.companionsState.invites.push(invite);
        return { kind: "invite", data: { code: "MH1-PRUEBA2ABCDEF", invite } };
      }
      case "create_host_invite": {
        if (!this.companionsState.enabled) throw new MercError("invalid_state", "Activa «Permitir compañeros» antes de invitar.");
        const invite = {
          invite_id: `inv${this.companionsState.invites.length + 1}`,
          server: null,
          server_name: "",
          perms: [] as Perm[],
          created_at_unix_ms: 1_793_300_300_000,
          expires_at_unix_ms: 1_793_300_300_000 + body.minutes * 60_000,
          host_admin: true,
        };
        this.companionsState.invites.push(invite);
        return { kind: "invite", data: { code: "MH1-CASERO2ABCDEF", invite } };
      }
      case "cancel_invite":
        this.companionsState.invites = this.companionsState.invites.filter((i) => i.invite_id !== body.invite_id);
        return { kind: "companions", data: structuredClone(this.companionsState) };
      case "set_companion_perms": {
        const c = this.companionsState.companions.find((x) => x.companion === body.companion);
        if (!c) throw new MercError("not_found", "Ese compañero ya no está.");
        c.servers = c.servers.filter((x) => x.id !== body.id);
        if (body.perms.length > 0) c.servers.push({ id: body.id, name: this.find(body.id).instance.name, perms: [...body.perms].sort() });
        return { kind: "companions", data: structuredClone(this.companionsState) };
      }
      case "remove_companion":
        this.companionsState.companions = this.companionsState.companions.filter((c) => c.companion !== body.companion);
        return { kind: "companions", data: structuredClone(this.companionsState) };
      case "join_with_code": {
        if (!this.friend || body.code !== "MH1-AMIGO") throw new MercError("forbidden", "El código no es válido, ya se usó o caducó.");
        if (!this.friendHosts.some((h) => h.host === this.friend!.host.host)) this.friendHosts.push(structuredClone(this.friend.host));
        return { kind: "remote_hosts", data: structuredClone(this.friendHosts) };
      }
      case "remote_hosts":
      case "refresh_host":
        return { kind: "remote_hosts", data: structuredClone(this.friendHosts) };
      case "forget_host":
        this.friendHosts = this.friendHosts.filter((h) => h.host !== body.host);
        return { kind: "remote_hosts", data: structuredClone(this.friendHosts) };
      case "remote_call": {
        const f = this.friend;
        if (!f || !this.friendHosts.some((h) => h.host === body.host)) throw new MercError("not_found", "No tienes esa PC guardada.");
        const req = body.request;
        const need = FakeApi.REMOTE_NEED[req.op];
        const sid = "id" in req ? (req as { id: string }).id : null;
        const server = f.host.servers.find((x) => x.id === sid);
        const admin = f.host.host_admin;
        const ok =
          need === "any" ||
          (need === "host" && admin) ||
          (admin && need !== undefined && need !== "host") ||
          (need !== undefined && need !== "host" && !!server && server.perms.includes(need));
        if (!ok) throw new MercError("forbidden", "No tienes permiso para esto.");
        // Como el anfitrión real: carpeta y Java los pone su PC.
        const r = await f.fake.call(req.op === "create_server" ? { ...req, path: "/casero/Servidores/x", java: "/casero/java" } : req);
        if (r.kind === "instances" && !admin) {
          return { kind: "instances", data: r.data.filter((x) => f.host.servers.some((h) => h.id === x.instance.id)) };
        }
        return r;
      }
      case "set_voice_config": {
        if (!this.voice) throw new MercError("invalid_state", "Este servidor no tiene Simple Voice Chat.");
        this.voiceWrites.push({ voice_host: body.voice_host, port: body.port });
        const v = this.voice;
        if (body.port != null) {
          v.port = body.port;
          v.port_setting = body.port;
          v.suggested_port = null;
          v.problems = v.problems.filter((p) => p.code !== "shared_port" && p.code !== "port_conflict");
        }
        if (body.voice_host != null) {
          if (!v.tunnels.some((t) => t.display_address === body.voice_host))
            throw new MercError("bad_request", "Esa dirección no es la de un túnel de voz activo de este servidor.");
          v.voice_host = body.voice_host;
          v.problems = v.problems.filter((p) => p.code !== "host_missing" && p.code !== "host_mismatch");
        }
        return this.connectionInfo();
      }
      case "test_address": {
        const ok = this.reachable.has(body.address);
        this.addressTests = [
          {
            address: body.address,
            at_unix_ms: 1_793_300_300_000,
            result: ok ? { tested: this.testedAs[body.address] ?? "dark-sea.gl.at.ply.gg:41234", latency_ms: 48, version: "Paper 1.21.1", players_online: 2, players_max: 20, motd: "Hola" } : null,
            error: ok ? null : "sin conexión: no se pudo conectar con dark-sea.gl.at.ply.gg:41234",
          },
          ...this.addressTests.filter((t) => t.address !== body.address),
        ];
        return this.connectionInfo();
      }
      case "test_bedrock_address": {
        if (!this.bedrock) throw new MercError("invalid_state", "Geyser no está instalado.");
        const ok = this.reachableBedrock.has(body.address);
        this.bedrock.tests = [
          {
            address: body.address,
            at_unix_ms: 1_793_300_300_000,
            result: ok ? { tested: body.address, latency_ms: 52, version: "Bedrock 1.21.40", players_online: 0, players_max: 20, motd: "Hola" } : null,
            error: ok ? null : `${body.address} no respondió como un servidor Bedrock (UDP)`,
          },
          ...this.bedrock.tests.filter((t) => t.address !== body.address),
        ];
        return this.connectionInfo();
      }
      case "health": {
        const s = this.find(body.id);
        const tpsKinds = ["paper", "purpur", "spigot", "mohist", "arclight", "neo_forge", "forge"];
        return {
          kind: "health",
          data: {
            pc_cpu_percent: 23.5,
            pc_memory_used_bytes: this.pcMemoryUsed,
            pc_memory_total_bytes: 16 * 1024 ** 3,
            disks: [
              { what: "server", path: "C:\\MercHosting\\Survival", free_bytes: 120 * 1024 ** 3, total_bytes: 500 * 1024 ** 3 },
              { what: "backups", path: "C:\\Users\\amir\\AppData\\Local\\MercHosting", free_bytes: 120 * 1024 ** 3, total_bytes: 500 * 1024 ** 3 },
            ],
            max_memory_mb: s.instance.launch.max_memory_mb,
            tps_command: tpsKinds.includes(s.instance.kind) ? (s.instance.kind === "neo_forge" ? "neoforge tps" : s.instance.kind === "forge" ? "forge tps" : "tps") : null,
          },
        };
      }
      case "measure_tps": {
        if (this.find(body.id).status.state !== "ONLINE") throw new MercError("invalid_state", "Enciende el servidor para medir su TPS.");
        return {
          kind: "tps",
          data: { tps: [["1m", 20], ["5m", 19.9], ["15m", 17.2]], mspt: null, command: "tps", raw: "TPS from last 1m, 5m, 15m: 20.0, 19.9, 17.2" },
        };
      }
      case "alerts":
        return this.alertList();
      case "mark_alerts_read":
        for (const a of this.alertItems) if (body.ids.length === 0 || body.ids.includes(a.id)) a.read = true;
        return this.alertList();
      case "set_alert_settings":
        if (body.settings.disk_min_gb < 1) throw new MercError("bad_request", "El aviso de disco va de 1 a 1000 GB.");
        this.alertSettings = { ...body.settings };
        return this.alertList();
      case "save_task": {
        const t = body.task;
        if (t.action.type === "command" && /^\/?\s*(stop|save-off|save-on)\b/.test(t.action.command.trim()))
          throw new MercError("bad_request", "«stop» no se programa como comando: usa las tareas de apagar, reiniciar o copia.");
        if (t.schedule.type === "daily" && (t.schedule.times.length === 0 || t.schedule.times.some((x) => !/^\d{1,2}:\d{2}$/.test(x))))
          throw new MercError("bad_request", "Elige al menos una hora válida.");
        if (t.schedule.type === "every" && t.schedule.minutes < 5)
          throw new MercError("bad_request", "El intervalo tiene que estar entre 5 minutos y 7 días.");
        const next = t.enabled ? 1_793_350_800_000 : null;
        if (t.id) {
          const cur = this.taskItems.find((x) => x.id === t.id);
          if (!cur) throw new MercError("not_found", "Esa tarea ya no existe");
          Object.assign(cur, { enabled: t.enabled, action: t.action, schedule: t.schedule, next_run_unix_ms: next });
        } else {
          this.taskSeq++;
          this.taskItems.push({
            id: `tarea${this.taskSeq}`,
            enabled: t.enabled,
            action: t.action,
            schedule: t.schedule,
            created_unix_ms: 1_793_300_000_000,
            next_run_unix_ms: next,
            last_run: null,
            running: false,
          });
        }
        return this.taskList();
      }
      case "delete_task":
        if (!this.taskItems.some((x) => x.id === body.task_id)) throw new MercError("not_found", "Esa tarea ya no existe");
        this.taskItems = this.taskItems.filter((x) => x.id !== body.task_id);
        return this.taskList();
      case "run_task_now": {
        const t = this.taskItems.find((x) => x.id === body.task_id);
        if (!t) throw new MercError("not_found", "Esa tarea ya no existe");
        const on = this.find(body.id).status.state === "ONLINE";
        const needsOnline = ["command", "announce", "restart", "stop"].includes(t.action.type);
        t.last_run =
          needsOnline && !on
            ? { at_unix_ms: 1_793_300_060_000, outcome: "skipped", message: "El servidor no está en línea.", manual: true }
            : { at_unix_ms: 1_793_300_060_000, outcome: "ok", message: "Hecho.", manual: true };
        return this.taskList();
      }
      case "set_backup_settings":
        if (body.keep < 1 || body.keep > 500) throw new MercError("bad_request", "Elige entre 1 y 500 copias.");
        this.backupKeep = body.keep;
        return this.backupList();
      case "content_platforms":
        return { kind: "content_platforms", data: this.platformsFor(this.find(body.id).instance.kind) };
      case "content_search":
        return {
          kind: "content_search",
          data: { hits: structuredClone(this.contentHits), total: this.contentHits.length, offset: body.offset },
        };
      case "content_project":
        return {
          kind: "content_details",
          data: {
            project: {
              id: body.project,
              slug: "mapa",
              title: this.contentHits.find((h) => h.project_id === body.project)?.title ?? "Just Enough Items",
              description: "Un mapa del mundo en el navegador.",
              body: "# Mapa\nTexto largo",
              icon_url: null,
              client_side: "optional",
              server_side: "required",
              license: "MIT",
              downloads: 1_500_000,
              categories: [],
              source_url: null,
              issues_url: null,
              wiki_url: null,
              page_url:
                body.source === "curseforge" ? "https://www.curseforge.com/minecraft/mc-mods/mapa" : "https://modrinth.com/project/mapa",
              distribution_allowed: true,
            },
            versions: [
              {
                id: "map2",
                project_id: body.project,
                name: "2.0",
                version_number: "2.0",
                version_type: "release",
                game_versions: ["1.21.1"],
                loaders: ["neoforge"],
                date_published: "2026-10-01T00:00:00Z",
                downloads: 5,
                changelog: "Arreglos",
                dependencies: [],
                files: [],
                server_side: "unknown",
              },
              {
                id: "jei20",
                project_id: body.project,
                name: "20.0",
                version_number: "20.0",
                version_type: "beta",
                game_versions: ["1.21.1"],
                loaders: ["neoforge"],
                date_published: "2026-09-01T00:00:00Z",
                downloads: 5,
                changelog: null,
                dependencies: [],
                files: [],
                server_side: "unknown",
              },
            ],
          },
        };
      case "content_plan":
        return { kind: "content_plan", data: structuredClone(this.contentPlanResult) };
      case "content_install": {
        this.requireStopped(body.id);
        for (const it of this.contentPlanResult.items)
          this.installedMods.push({
            file_name: it.file_name,
            enabled: true,
            size: it.size,
            sha1: it.sha1,
            project_id: it.project_id,
            version_id: it.version_id,
            title: it.title,
            version_number: it.version_number,
            icon_url: null,
            server_side: "required",
            update: null,
            source: body.source,
            other_ids: [],
          });
        return { kind: "content_list", data: this.contentList(body.id, body.kind) };
      }
      case "install_floodgate": {
        this.requireStopped(body.id);
        if (this.bedrock) {
          this.bedrock.floodgate = "plugins/floodgate-spigot.jar";
          this.bedrock.floodgate_enabled = true;
        }
        return { kind: "content_list", data: this.contentList(body.id, "plugin") };
      }
      case "content_installed":
        return { kind: "content_list", data: this.contentList(body.id, body.kind) };
      case "content_toggle": {
        this.requireStopped(body.id);
        const it = this.installedMods.find((i) => i.file_name === body.file_name)!;
        it.enabled = body.enabled;
        it.file_name = body.enabled ? it.file_name.replace(/\.disabled$/, "") : `${it.file_name}.disabled`;
        return { kind: "content_list", data: this.contentList(body.id, body.kind) };
      }
      case "content_remove":
        this.requireStopped(body.id);
        this.installedMods = this.installedMods.filter((i) => i.file_name !== body.file_name);
        return { kind: "content_list", data: this.contentList(body.id, body.kind) };
      case "unregister": {
        const s = this.find(body.id);
        if (body.delete_files) this.recycled.push(s.instance.root);
        this.servers = this.servers.filter((x) => x.instance.id !== body.id);
        return { kind: "ok" };
      }
      default:
        return { kind: "ok" };
    }
  }
}
