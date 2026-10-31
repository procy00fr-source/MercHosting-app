import {
  AlertTriangle,
  KeyRound,
  ArrowUpCircle,
  Image as ImageIcon,
  Check,
  Clock,
  Copy,
  Cpu,
  Download,
  FolderOpen,
  FolderTree,
  Globe,
  History,
  Archive,
  ArchiveRestore,
  CopyPlus,
  CalendarClock,
  HeartPulse,
  Globe2,
  Lock,
  House,
  LayoutDashboard,
  MemoryStick,
  Megaphone,
  Play,
  Plug,
  Puzzle,
  Layers,
  RotateCw,
  SlidersHorizontal,
  Square,
  Stethoscope,
  ShieldCheck,
  Send,
  Terminal,
  Trash2,
  Users,
  Wifi,
  Zap,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { MercApi } from "../api/client";
import { Supervisor, errorMessage } from "../api/client";
import type { AdminState, Alert, InstanceSummary, Metrics, Perm, Platform, SystemInfo } from "../api/types";
import {
  canStart,
  formatBytes,
  formatDuration,
  formatMemory,
  formatPercent,
  hasProcess,
  KIND_LABEL,
  STATE_LABEL,
  STATE_TONE,
} from "../format";
import { AreaChart, Meter } from "./Charts";
import { Console, useConsoleLines } from "./Console";
import { ActivityPanel } from "./ActivityPanel";
import { BackupsPanel } from "./BackupsPanel";
import { TasksPanel } from "./TasksPanel";
import { HealthPanel } from "./HealthPanel";
import { ConnectionPanel } from "./ConnectionPanel";
import { DiagnosisPanel } from "./DiagnosisPanel";
import { AutomationCard } from "./AutomationCard";
import { CloneDialog } from "./CloneDialog";
import { MotdIconPanel } from "./MotdIconPanel";
import { UpdatesPanel } from "./UpdatesPanel";
import { ADVANCED_TABS, useUiLevel } from "../uiLevel";
import { LuckPermsPanel } from "./LuckPermsPanel";
import { LaunchCard } from "./LaunchCard";
import { SecurityPanel } from "./SecurityPanel";
import { InvitePanel } from "./InvitePanel";
import { ContentPanel } from "./ContentPanel";
import { Dialog } from "./Dialog";
import { FilesPanel } from "./FilesPanel";
import { PlayersPanel } from "./PlayersPanel";
import { PropertiesPanel } from "./PropertiesPanel";
import { WorldsPanel } from "./WorldsPanel";
import { Logo } from "./Logo";
import { Sidebar, type NavGroup, type SidebarCommon } from "./Sidebar";
import { VoxelScene } from "./VoxelScene";

const EULA_URL = "https://aka.ms/MinecraftEULA";
const METRICS_POLL_MS = 2000;

interface Props {
  api: MercApi;
  summary: InstanceSummary;
  system: SystemInfo | null;
  /** Alertas de todos los servidores (la pestaña Salud muestra las de este). */
  alerts?: Alert[];
  onChanged: () => void;
  onRemoved: () => void;
  onError: (message: string) => void;
  sidebar: SidebarCommon;
  onServers: () => void;
  /** Abre la página de compañeros (desde el centro de seguridad). */
  onOpenCompanions?: () => void;
  /** Abre otro servidor (la copia recién clonada). */
  onOpenServer?: (id: string) => void;
  /** Servidor de un amigo: solo lo que permiten sus permisos (su PC decide de verdad). */
  remote?: { perms: Perm[]; hostName: string };
}

function useNow(intervalMs: number) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), intervalMs);
    return () => clearInterval(t);
  }, [intervalMs]);
  return now;
}

function CopyChip({ icon, label, value }: { icon: React.ReactNode; label: string; value: string }) {
  const [copied, setCopied] = useState(false);
  async function copy() {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      setTimeout(() => setCopied(false), 1400);
    } catch {
      // Sin acceso al portapapeles: el texto sigue visible para copiarlo a mano.
    }
  }
  return (
    <button type="button" className="addr" onClick={copy} aria-label={`Copiar ${label}: ${value}`}>
      <span className="addr-icon">{icon}</span>
      <span className="addr-text">
        <span className="addr-label">{label}</span>
        <span className="addr-value">{value}</span>
      </span>
      <span className="addr-copy" aria-hidden="true">
        {copied ? <Check size={15} /> : <Copy size={15} />}
      </span>
      <span className="sr-only" aria-live="polite">
        {copied ? "Copiado" : ""}
      </span>
    </button>
  );
}

export function ServerView({ api, summary, system, alerts = [], onChanged, onRemoved, onError, sidebar, onServers, remote, onOpenCompanions, onOpenServer }: Props) {
  const can = (p: Perm) => !remote || remote.perms.includes(p);
  const sup = useMemo(() => new Supervisor(api), [api]);
  const { instance: inst, status } = summary;
  const [tab, setTab] = useState<
    "overview" | "console" | "diagnose" | "updates" | "appearance" | "security" | "invite" | "players" | "worlds" | "settings" | "files" | "activity" | "mod" | "plugin" | "datapack" | "backups" | "tasks" | "health" | "connection" | "luckperms"
  >("overview");
  const [contentQuery, setContentQuery] = useState("");
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  // Los tutoriales piden abrir una pestaña con el evento «merc:tab».
  useEffect(() => {
    const on = (e: Event) => {
      const want = (e as CustomEvent<string>).detail;
      const t = want === "contenido" ? (platforms[0]?.kind ?? "overview") : want;
      setContentQuery("");
      setTab(t as typeof tab);
    };
    window.addEventListener("merc:tab", on);
    return () => window.removeEventListener("merc:tab", on);
  }, [platforms]);
  const [pending, setPending] = useState<string | null>(null);
  const [eulaChecked, setEulaChecked] = useState(false);
  const [confirmKill, setConfirmKill] = useState(false);
  const [killChecked, setKillChecked] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [cloning, setCloning] = useState(false);
  const [confirmRestart, setConfirmRestart] = useState(false);
  const [noticeSecs, setNoticeSecs] = useState(60);
  const [metrics, setMetrics] = useState<Metrics | null>(null);
  const [deleteFiles, setDeleteFiles] = useState(false);
  const [admin, setAdmin] = useState<AdminState | null>(null);
  /** Momento estimado del reinicio avisado (para la cuenta atrás entre consultas). */
  const [restartAt, setRestartAt] = useState<number | null>(null);
  const now = useNow(1000);
  const lines = useConsoleLines(inst.id, sup, can("console_read"));

  async function exportConsole() {
    const dest = await api.pickSavePath(`consola-${inst.name.replace(/[<>:"/\\|?*]/g, "-")}.log`);
    if (!dest) return;
    try {
      await sup.exportConsole(inst.id, dest);
    } catch (e) {
      onError(errorMessage(e));
    }
  }

  const state = status.state;
  const tone = STATE_TONE[state];
  const running = hasProcess(state);
  const startable = canStart(state);
  const orphan = running && !status.has_console;

  const canContent = can("content");
  useEffect(() => {
    let alive = true;
    if (!canContent) return;
    sup.contentPlatforms(inst.id).then(
      (p) => alive && setPlatforms(p),
      () => undefined,
    );
    return () => {
      alive = false;
    };
  }, [sup, inst.id, inst.kind, canContent]);

  useEffect(() => {
    let alive = true;
    async function tick() {
      try {
        const [m, a] = await Promise.all([sup.metrics(inst.id), sup.adminState(inst.id)]);
        if (!alive) return;
        setMetrics(m);
        applyAdmin(a);
      } catch {
        // se reintenta en el siguiente ciclo
      }
    }
    tick();
    const t = setInterval(tick, METRICS_POLL_MS);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [inst.id, sup]);

  function applyAdmin(a: AdminState) {
    setAdmin(a);
    setRestartAt(a.restart_in_seconds !== null ? Date.now() + a.restart_in_seconds * 1000 : null);
  }

  async function run(label: string, action: () => Promise<unknown>) {
    setPending(label);
    try {
      await action();
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      setPending(null);
      onChanged();
    }
  }

  const uptime =
    state === "ONLINE" && status.online_at_unix_ms
      ? formatDuration(now - status.online_at_unix_ms)
      : (state === "STARTING" || state === "LOADING") && status.started_at_unix_ms
        ? formatDuration(now - status.started_at_unix_ms)
        : null;

  const port = status.port ?? summary.configured_port ?? 25565;
  const portSuffix = port === 25565 ? "" : `:${port}`;
  const samples = running ? (metrics?.samples ?? []) : [];
  const last = samples[samples.length - 1];
  const players = running ? (metrics?.players ?? []) : [];
  const maxMem = inst.launch.max_memory_mb * 1024 * 1024;

  const restartLeft = restartAt !== null ? Math.max(0, Math.ceil((restartAt - now) / 1000)) : null;
  const canAct = state === "ONLINE" && status.has_console;

  type Tab = typeof tab;
  const SECTION_LABEL: Record<Tab, string> = {
    overview: "Resumen",
    console: "Consola",
    diagnose: "Diagnóstico",
    updates: "Actualizaciones",
    appearance: "MOTD e icono",
    security: "Seguridad",
    invite: "Invitar jugadores",
    players: "Jugadores",
    worlds: "Mundos",
    settings: "Ajustes",
    files: "Archivos",
    activity: "Actividad",
    mod: "Mods",
    plugin: "Plugins",
    datapack: "Datapacks",
    backups: "Copias de seguridad",
    tasks: "Tareas programadas",
    health: "Salud y rendimiento",
    connection: "Conexión",
    luckperms: "Permisos (LuckPerms)",
  };
  const TAB_PERM: Partial<Record<string, Perm[]>> = {
    console: ["console_read"],
    diagnose: ["logs"],
    appearance: ["settings"],
    connection: ["network"],
    files: ["files_list"],
    backups: ["backups"],
    settings: ["settings"],
    activity: ["logs"],
    players: ["players", "whitelist", "op"],
    worlds: ["world"],
  };
  // La seguridad (compañeros, claves, exposición) es solo del dueño.
  const level = useUiLevel();
  const allowed = (tabId: string) =>
    !(level === "principiante" && ADVANCED_TABS.has(tabId)) &&
    (!remote || (tabId !== "security" && tabId !== "luckperms" && (!TAB_PERM[tabId] || TAB_PERM[tabId]!.some((p) => can(p)))));
  // Al pasar a principiante, una pestaña avanzada abierta vuelve al resumen.
  useEffect(() => {
    if (level === "principiante" && ADVANCED_TABS.has(tab)) setTab("overview");
  }, [level, tab]);
  const allGroups: NavGroup[] = [
    {
      title: "Principal",
      items: [
        { id: "overview", label: "Resumen", icon: <LayoutDashboard size={18} aria-hidden="true" /> },
        { id: "console", label: "Consola", icon: <Terminal size={18} aria-hidden="true" /> },
        { id: "diagnose", label: "Diagnóstico", icon: <Stethoscope size={18} aria-hidden="true" /> },
        { id: "security", label: "Seguridad", icon: <ShieldCheck size={18} aria-hidden="true" /> },
        { id: "connection", label: "Conexión", icon: <Globe2 size={18} aria-hidden="true" /> },
        { id: "files", label: "Archivos", icon: <FolderTree size={18} aria-hidden="true" /> },
        { id: "health", label: "Salud y rendimiento", icon: <HeartPulse size={18} aria-hidden="true" /> },
        { id: "backups", label: "Copias de seguridad", icon: <Archive size={18} aria-hidden="true" /> },
        { id: "updates", label: "Actualizaciones", icon: <ArrowUpCircle size={18} aria-hidden="true" /> },
        { id: "tasks", label: "Tareas programadas", icon: <CalendarClock size={18} aria-hidden="true" /> },
        { id: "settings", label: "Ajustes", icon: <SlidersHorizontal size={18} aria-hidden="true" /> },
        { id: "activity", label: "Actividad", icon: <History size={18} aria-hidden="true" /> },
      ],
    },
    {
      title: "Juego",
      items: [
        { id: "players", label: "Jugadores", icon: <Users size={18} aria-hidden="true" />, dot: !!admin?.maintenance },
        { id: "invite", label: "Invitar jugadores", icon: <Send size={18} aria-hidden="true" /> },
        { id: "appearance", label: "MOTD e icono", icon: <ImageIcon size={18} aria-hidden="true" /> },
        { id: "worlds", label: "Mundos", icon: <Globe size={18} aria-hidden="true" /> },
        { id: "luckperms", label: "Permisos (LuckPerms)", icon: <KeyRound size={18} aria-hidden="true" /> },
        ...platforms.map((p) => ({
          id: p.kind,
          label: p.kind === "mod" ? "Mods" : p.kind === "plugin" ? "Plugins" : "Datapacks",
          icon:
            p.kind === "mod" ? (
              <Puzzle size={18} aria-hidden="true" />
            ) : p.kind === "plugin" ? (
              <Plug size={18} aria-hidden="true" />
            ) : (
              <Layers size={18} aria-hidden="true" />
            ),
        })),
      ],
    },
  ];
  const groups = allGroups.map((g) => ({ ...g, items: g.items.filter((it) => allowed(it.id)) })).filter((g) => g.items.length > 0);

  return (
    <>
      <Sidebar
        {...sidebar}
        current={{ name: inst.name, sub: `${KIND_LABEL[inst.kind]} ${inst.mc_version ?? ""}`, tone }}
        serversActive={false}
        onServers={onServers}
        groups={groups}
        active={tab}
        onSelect={(id) => {
          setContentQuery("");
          setTab(id as Tab);
        }}
      />
    <main className="main" data-tone={tone}>
      <header className="page-head server-head">
        <div>
          <p className="page-eyebrow">
            {remote ? `PC de ${remote.hostName} · ` : ""}
            {SECTION_LABEL[tab]}
          </p>
          <h1 className="page-title" translate="no">{inst.name}</h1>
          <div className="hero-meta">
            <span className="status-pill" data-tone={tone} data-tour="estado">
              <Logo size={16} tone={tone} />
              {STATE_LABEL[state]}
            </span>
            <span className="chip chip-kind">
              {KIND_LABEL[inst.kind]} {inst.mc_version ?? ""}
            </span>
            {inst.loader_version && inst.kind !== "vanilla" && <span className="chip">Loader {inst.loader_version}</span>}
            {pending && <span className="chip chip-busy">{pending}…</span>}
            {summary.archived && <span className="chip">Archivado</span>}
          </div>
        </div>
        <dl className="head-stats" aria-label="Estado en vivo">
          <div>
            <dt>
              <Clock size={15} aria-hidden="true" /> Encendido
            </dt>
            <dd>{uptime ?? "—"}</dd>
          </div>
          <div>
            <dt>
              <Cpu size={15} aria-hidden="true" /> CPU
            </dt>
            <dd>{running && last ? formatPercent(last.cpu_percent) : "—"}</dd>
          </div>
          <div>
            <dt>
              <MemoryStick size={15} aria-hidden="true" /> Memoria
            </dt>
            <dd>
              {running && last ? formatBytes(last.memory_bytes) : "—"}
              <span className="head-of"> / {formatMemory(inst.launch.max_memory_mb)}</span>
            </dd>
          </div>
          <div>
            <dt>
              <Users size={15} aria-hidden="true" /> Jugadores
            </dt>
            <dd>
              {running && status.has_console ? players.length : "—"}
              {metrics?.max_players ? <span className="head-of"> / {metrics.max_players}</span> : null}
            </dd>
          </div>
        </dl>
      </header>

      <div className="notices">
        {remote && !summary.eula_accepted && !running && (
          <div className="notice notice-warn">
            <AlertTriangle size={18} aria-hidden="true" className="notice-icon" />
            <p>El dueño de este servidor todavía no aceptó la EULA de Minecraft en su PC. Hasta entonces no se puede iniciar.</p>
          </div>
        )}
        {!remote && !summary.eula_accepted && !running && (
          <div className="notice notice-warn">
            <AlertTriangle size={18} aria-hidden="true" className="notice-icon" />
            <div>
              <p>
                Para iniciar un servidor de Minecraft tienes que aceptar la EULA de Minecraft (el acuerdo de licencia de
                Mojang). MercHosting no la acepta por ti.{" "}
                <button type="button" className="link" onClick={() => api.openUrl(EULA_URL)}>
                  Leer la EULA
                </button>
              </p>
              <div className="notice-actions">
                <label className="check">
                  <input type="checkbox" checked={eulaChecked} onChange={(e) => setEulaChecked(e.target.checked)} />
                  <span>He leído la EULA de Minecraft y la acepto</span>
                </label>
                <button
                  type="button"
                  className="btn"
                  disabled={!eulaChecked || !!pending}
                  onClick={() => run("Guardando la aceptación", () => sup.acceptEula(inst.id))}
                >
                  Aceptar EULA
                </button>
              </div>
            </div>
          </div>
        )}
        {restartLeft !== null && running && (
          <div className="notice notice-warn" role="status">
            <RotateCw size={18} aria-hidden="true" className="notice-icon" />
            <div>
              <p>
                {restartLeft > 0
                  ? `El servidor se reiniciará en ${restartLeft >= 60 ? `${Math.floor(restartLeft / 60)} min ${restartLeft % 60} s` : `${restartLeft} s`}. Los jugadores ya recibieron el aviso por el chat.`
                  : "Reiniciando…"}
              </p>
              {restartLeft > 0 && (
                <div className="notice-actions">
                  <button
                    type="button"
                    className="btn btn-sm"
                    disabled={!!pending}
                    onClick={() => run("Cancelando el reinicio", async () => applyAdmin(await sup.cancelRestart(inst.id)))}
                  >
                    Cancelar reinicio
                  </button>
                </div>
              )}
            </div>
          </div>
        )}
        {admin?.maintenance && (
          <div className="notice notice-info" role="status">
            <Lock size={18} aria-hidden="true" className="notice-icon" />
            <div>
              <p>
                Modo mantenimiento: solo pueden entrar los jugadores de la lista blanca (y normalmente los operadores). Los que ya
                estaban conectados siguen jugando.
              </p>
              <div className="notice-actions">
                <button
                  type="button"
                  className="btn btn-sm"
                  disabled={!!pending || (running && !canAct)}
                  onClick={() => run("Terminando el mantenimiento", async () => applyAdmin(await sup.setMaintenance(inst.id, false)))}
                >
                  Terminar mantenimiento
                </button>
              </div>
            </div>
          </div>
        )}
        {orphan && (
          <div className="notice notice-warn">
            <AlertTriangle size={18} aria-hidden="true" className="notice-icon" />
            <p>
              Este servidor siguió encendido mientras MercHosting estaba cerrado. Funciona con normalidad, pero su consola no se
              puede recuperar. Para apagarlo guardando el mundo, entra al juego como operador y escribe /stop.
            </p>
          </div>
        )}
        {status.hint && !orphan && (
          <div className={`notice ${state === "CRASHED" || state === "ERROR" ? "notice-bad" : "notice-warn"}`} role="status">
            <AlertTriangle size={18} aria-hidden="true" className="notice-icon" />
            <div>
              <p>{status.hint}</p>
              {(state === "CRASHED" || state === "ERROR") && can("logs") && tab !== "diagnose" && (
                <div className="notice-actions">
                  <button type="button" className="btn btn-sm" onClick={() => setTab("diagnose")}>
                    <Stethoscope size={14} aria-hidden="true" /> Ver diagnóstico
                  </button>
                </div>
              )}
            </div>
          </div>
        )}
        {state === "CRASHED" && !status.hint && can("logs") && tab !== "diagnose" && (
          <div className="notice notice-bad" role="status">
            <AlertTriangle size={18} aria-hidden="true" className="notice-icon" />
            <div>
              <p>El servidor se cerró solo. El diagnóstico revisa el error y te dice qué lo pudo causar.</p>
              <div className="notice-actions">
                <button type="button" className="btn btn-sm" onClick={() => setTab("diagnose")}>
                  <Stethoscope size={14} aria-hidden="true" /> Ver diagnóstico
                </button>
              </div>
            </div>
          </div>
        )}
      </div>

      {tab === "overview" ? (
        <div className="overview" role="tabpanel" aria-label="Resumen">
          <div className="ov-top">
            {can("console_read") && (
            <article className="card ov-console">
              <header className="card-head">
                <h2>
                  <Terminal size={16} aria-hidden="true" /> Consola
                  {status.has_console && running && <span className="badge">Conectada</span>}
                </h2>
                <button type="button" className="btn btn-quiet btn-sm" onClick={() => setTab("console")}>
                  Ver completa
                </button>
              </header>
              <Console
                id={inst.id}
                sup={sup}
                lines={lines}
                canSend={running && status.has_console && state !== "STOPPING" && can("console_command")}
                disabledReason={orphan ? "Sin consola: el servidor viene de una sesión anterior" : "Inicia el servidor para enviar comandos"}
              />
            </article>
            )}
            <div className="ov-side">
              <section className="card power-card" aria-label="Encendido" data-tour="encendido">
                {startable && can("start") && (
                  <button
                    type="button"
                    className="btn btn-lg btn-go"
                    disabled={!!pending || !summary.eula_accepted || !!summary.archived}
                    title={
                      summary.archived
                        ? "Está archivado: desarchívalo para encenderlo"
                        : summary.eula_accepted
                          ? undefined
                          : "Primero acepta la EULA de Minecraft"
                    }
                    onClick={() => run("Iniciando", () => sup.start(inst.id))}
                  >
                    <Play size={18} aria-hidden="true" /> Iniciar
                  </button>
                )}
                {summary.archived && <p className="field-help">Archivado: no se puede encender. Desarchívalo en Detalles para usarlo.</p>}
                {state === "ONLINE" && status.has_console && can("restart") && (
                  <button type="button" className="btn btn-lg btn-warn" disabled={!!pending} onClick={() => setConfirmRestart(true)}>
                    <RotateCw size={17} aria-hidden="true" /> Reiniciar
                  </button>
                )}
                {running && status.has_console && can("stop") && (
                  <button
                    type="button"
                    className="btn btn-lg btn-stop"
                    disabled={!!pending || state === "STOPPING"}
                    onClick={() => run("Deteniendo y guardando", () => sup.stop(inst.id))}
                  >
                    <Square size={16} aria-hidden="true" /> Detener
                  </button>
                )}
                {running && !remote && (
                  <button type="button" className="btn btn-lg btn-danger-ghost" disabled={!!pending} onClick={() => setConfirmKill(true)}>
                    <Zap size={15} aria-hidden="true" /> Forzar cierre
                  </button>
                )}
                {!running && state === "VALIDATING" && <p className="field-help">Comprobando…</p>}
              </section>
              <div className="tile-grid">
                <CopyChip icon={<House size={16} />} label="Desde esta PC" value={`localhost${portSuffix}`} />
                {system?.lan_ipv4 && (
                  <CopyChip icon={<Wifi size={16} />} label="Desde tu red (LAN)" value={`${system.lan_ipv4}:${port}`} />
                )}
              </div>
              <article className="card banner-card">
                <VoxelScene seed={inst.id} tone={tone} />
                <div className="banner-text">
                  <h2>Juega con tus amigos</h2>
                  <p>
                    En tu casa entran con la dirección de tu red. Para que entren desde Internet, activa Playit en Conexión: no
                    hace falta abrir puertos del router.
                  </p>
                  {can("network") && (
                    <button type="button" className="btn btn-sm" onClick={() => setTab("connection")}>
                      Ir a Conexión
                    </button>
                  )}
                </div>
              </article>
            </div>
          </div>
          <div className="stat-grid">
            <article className="card stat">
              <header className="stat-head">
                <span className="stat-icon stat-cpu">
                  <Cpu size={17} aria-hidden="true" />
                </span>
                <h2>Procesador</h2>
              </header>
              {running && last ? (
                <>
                  <p className="stat-value">{formatPercent(last.cpu_percent)}</p>
                  <p className="stat-sub">de toda tu PC{system ? ` (${system.logical_cpus} núcleos)` : ""}</p>
                  <AreaChart values={samples.map((s) => s.cpu_percent)} max={Math.max(10, ...samples.map((s) => s.cpu_percent)) * 1.2} color="var(--c-cpu)" label="Uso de procesador en los últimos minutos" />
                </>
              ) : (
                <p className="stat-empty">{running ? "Midiendo…" : "Inicia el servidor para ver su consumo."}</p>
              )}
            </article>

            <article className="card stat">
              <header className="stat-head">
                <span className="stat-icon stat-ram">
                  <MemoryStick size={17} aria-hidden="true" />
                </span>
                <h2>Memoria</h2>
              </header>
              {running && last ? (
                <>
                  <p className="stat-value">{formatBytes(last.memory_bytes)}</p>
                  <p className="stat-sub">
                    en uso, con {formatMemory(inst.launch.max_memory_mb)} asignados
                    {system?.total_memory_bytes ? ` de ${formatBytes(system.total_memory_bytes)} de tu PC` : ""}
                  </p>
                  <Meter value={last.memory_bytes} max={maxMem} color="var(--c-ram)" />
                  <AreaChart values={samples.map((s) => s.memory_bytes)} max={Math.max(maxMem, ...samples.map((s) => s.memory_bytes))} color="var(--c-ram)" label="Memoria en los últimos minutos" />
                </>
              ) : (
                <p className="stat-empty">{running ? "Midiendo…" : "Inicia el servidor para ver su consumo."}</p>
              )}
            </article>

            <article className="card stat">
              <header className="stat-head">
                <span className="stat-icon stat-players">
                  <Users size={17} aria-hidden="true" />
                </span>
                <h2>Jugadores</h2>
              </header>
              {running && status.has_console ? (
                <>
                  <p className="stat-value">
                    {players.length}
                    {metrics?.max_players ? <span className="stat-of"> / {metrics.max_players}</span> : null}
                  </p>
                  <p className="stat-sub">conectados ahora, según la consola</p>
                  <ul className="players">
                    {players.map((p) => (
                      <li key={p} className="player">
                        <span className="player-avatar" aria-hidden="true">
                          {p.slice(0, 1).toUpperCase()}
                        </span>
                        {p}
                      </li>
                    ))}
                  </ul>
                </>
              ) : (
                <p className="stat-empty">
                  {orphan ? "Sin consola conectada no se pueden contar los jugadores." : "Inicia el servidor para ver quién está conectado."}
                </p>
              )}
            </article>
          </div>

          <div className="overview-row">
            <article className="card details">
              <header className="card-head">
                <h2>Detalles</h2>
              </header>
              <dl>
                <div>
                  <dt>
                    <FolderOpen size={14} aria-hidden="true" /> Carpeta
                  </dt>
                  <dd className="path">{inst.root}</dd>
                </div>
                <div>
                  <dt>Java</dt>
                  <dd className="path">{inst.launch.java}</dd>
                </div>
                <div>
                  <dt>Puerto</dt>
                  <dd>
                    {port} {status.port ? "(en uso)" : summary.configured_port ? "(server.properties)" : "(por defecto)"}
                  </dd>
                </div>
                {status.pid !== null && (
                  <div>
                    <dt>Proceso</dt>
                    <dd>PID {status.pid}</dd>
                  </div>
                )}
                {status.last_exit && !running && (
                  <div>
                    <dt>Última salida</dt>
                    <dd>
                      Código {status.last_exit.code ?? "desconocido"}
                      {status.last_exit.forced ? ", cierre forzado" : status.last_exit.requested ? ", detenido por ti" : ""}
                    </dd>
                  </div>
                )}
              </dl>
              {!running && state !== "VALIDATING" && !remote && (
                <div className="details-actions">
                  <button type="button" className="btn btn-quiet" disabled={!!pending} onClick={() => setCloning(true)}>
                    <CopyPlus size={15} aria-hidden="true" /> Clonar
                  </button>
                  <button
                    type="button"
                    className="btn btn-quiet"
                    disabled={!!pending}
                    onClick={() =>
                      run(summary.archived ? "Desarchivando" : "Archivando", async () => {
                        await sup.setArchived(inst.id, !summary.archived);
                        onChanged();
                      })
                    }
                  >
                    {summary.archived ? <ArchiveRestore size={15} aria-hidden="true" /> : <Archive size={15} aria-hidden="true" />}{" "}
                    {summary.archived ? "Desarchivar" : "Archivar"}
                  </button>
                  <button type="button" className="btn btn-quiet" disabled={!!pending} onClick={() => setConfirmRemove(true)}>
                    <Trash2 size={15} aria-hidden="true" /> Quitar de la lista
                  </button>
                </div>
              )}
            </article>
          </div>
        </div>
      ) : tab === "players" ? (
        <>
          {admin && !admin.maintenance && (
            <section className="card maint-card" aria-label="Modo mantenimiento">
              <Lock size={20} aria-hidden="true" />
              <div>
                <h3>Modo mantenimiento</h3>
                <p className="field-help">
                  {admin.whitelist_on
                    ? "Ya tienes la lista blanca activada: solo entra quien esté en ella. El mantenimiento la mantiene así y avisa por el chat."
                    : "Mientras haces cambios, nadie nuevo puede entrar: se activa la lista blanca de Minecraft y al terminar se desactiva."}
                  {admin.enforce_whitelist && !admin.whitelist_on
                    ? " Ojo: tienes «enforce-whitelist» activado, así que se expulsará a los conectados que no estén en la lista."
                    : ""}
                  {!canAct ? " Enciende el servidor para activarlo." : ""}
                </p>
              </div>
              <button
                type="button"
                className="btn"
                disabled={!canAct || !!pending}
                onClick={() => run("Activando el mantenimiento", async () => applyAdmin(await sup.setMaintenance(inst.id, true)))}
              >
                Activar mantenimiento
              </button>
            </section>
          )}
          <PlayersPanel sup={sup} id={inst.id} />
        </>
      ) : tab === "luckperms" && !remote ? (
        <LuckPermsPanel
          sup={sup}
          id={inst.id}
          online={state === "ONLINE" && status.has_console}
          onSearchPlugin={(() => {
            const p = platforms.find((x) => x.kind === "plugin") ?? platforms.find((x) => x.kind === "mod");
            return p
              ? () => {
                  setContentQuery("LuckPerms");
                  setTab(p.kind as Tab);
                }
              : undefined;
          })()}
        />
      ) : tab === "worlds" ? (
        <WorldsPanel api={api} sup={sup} id={inst.id} running={running} online={state === "ONLINE" && status.has_console} remote={!!remote} />
      ) : (tab === "mod" || tab === "plugin" || tab === "datapack") && platforms.some((p) => p.kind === tab) ? (
        <ContentPanel
          key={`${tab}-${contentQuery}`}
          initialQuery={contentQuery || undefined}
          api={api}
          sup={sup}
          id={inst.id}
          platform={platforms.find((p) => p.kind === tab)!}
          running={running}
          mcVersion={inst.mc_version}
        />
      ) : tab === "diagnose" ? (
        <DiagnosisPanel
          api={api}
          sup={sup}
          id={inst.id}
          running={running}
          mcVersion={inst.mc_version}
          can={can}
          remote={!!remote}
          onOpenTab={(t, q) => {
            setContentQuery(q ?? "");
            setTab(t as Tab);
          }}
        />
      ) : tab === "updates" ? (
        <UpdatesPanel
          sup={sup}
          id={inst.id}
          mcVersion={inst.mc_version}
          running={running}
          platforms={platforms}
          kind={inst.kind}
          can={can}
          remote={!!remote}
          onOpenTab={(t) => setTab(t as Tab)}
          onChanged={onChanged}
        />
      ) : tab === "appearance" ? (
        <MotdIconPanel
          api={api}
          sup={sup}
          id={inst.id}
          name={inst.name}
          running={running}
          online={players.length}
          remote={!!remote}
          canEdit={can("settings")}
        />
      ) : tab === "security" && !remote ? (
        <SecurityPanel
          sup={sup}
          id={inst.id}
          running={running}
          onOpenTab={(t) => setTab(t as Tab)}
          onOpenCompanions={onOpenCompanions}
        />
      ) : tab === "invite" ? (
        <InvitePanel api={api} sup={sup} inst={inst} platforms={platforms} can={can} remote={!!remote} />
      ) : tab === "activity" ? (
        <ActivityPanel sup={sup} id={inst.id} />
      ) : tab === "settings" ? (
        <>
          <LaunchCard
            api={api}
            sup={sup}
            id={inst.id}
            launch={inst.launch}
            mcVersion={inst.mc_version}
            running={running}
            remote={!!remote}
            onChanged={onChanged}
          />
          {!remote && <AutomationCard sup={sup} id={inst.id} />}
          <PropertiesPanel sup={sup} id={inst.id} running={running} mcVersion={inst.mc_version} openUrl={(u) => void api.openUrl(u)} />
        </>
      ) : tab === "files" ? (
        <FilesPanel api={api} sup={sup} id={inst.id} running={running} remote={!!remote} can={can} />
      ) : tab === "connection" ? (
        <ConnectionPanel api={api} sup={sup} id={inst.id} online={state === "ONLINE"} stopped={!running} remote={!!remote} />
      ) : tab === "health" ? (
        <HealthPanel sup={sup} id={inst.id} state={state} alerts={alerts} />
      ) : tab === "tasks" ? (
        <TasksPanel sup={sup} id={inst.id} />
      ) : tab === "backups" ? (
        <BackupsPanel api={api} sup={sup} id={inst.id} state={state} hasConsole={status.has_console} remote={!!remote} />
      ) : (
        <div className="console-wrap" role="tabpanel" aria-label="Consola">
          <div className="console-tools">
            <button type="button" className="btn btn-sm btn-quiet" onClick={exportConsole}>
              <Download size={14} aria-hidden="true" /> Exportar registro
            </button>
          </div>
          <Console
            id={inst.id}
            sup={sup}
            lines={lines}
            canSend={running && status.has_console && state !== "STOPPING"}
            disabledReason={orphan ? "Sin consola: el servidor viene de una sesión anterior" : "Inicia el servidor para enviar comandos"}
          />
        </div>
      )}

      {confirmRestart && (
        <Dialog
          title="Reiniciar el servidor"
          lead={
            (metrics?.players.length ?? 0) > 0
              ? `Hay ${metrics!.players.length} jugador${metrics!.players.length === 1 ? "" : "es"} conectado${metrics!.players.length === 1 ? "" : "s"}. Puedes avisarles por el chat antes de reiniciar.`
              : "El mundo se guarda antes de reiniciar."
          }
          onClose={() => setConfirmRestart(false)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setConfirmRestart(false)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn"
                onClick={() => {
                  setConfirmRestart(false);
                  run("Avisando a los jugadores", () => sup.restartWithNotice(inst.id, noticeSecs));
                }}
              >
                <Megaphone size={15} aria-hidden="true" /> Avisar y reiniciar
              </button>
              <button
                type="button"
                className="btn btn-go"
                onClick={() => {
                  setConfirmRestart(false);
                  run("Reiniciando", () => sup.restart(inst.id));
                }}
              >
                Reiniciar ahora
              </button>
            </>
          }
        >
          <div className="field">
            <label htmlFor="notice-secs">Tiempo de aviso</label>
            <select id="notice-secs" value={noticeSecs} onChange={(e) => setNoticeSecs(Number(e.target.value))}>
              <option value={30}>30 segundos</option>
              <option value={60}>1 minuto</option>
              <option value={120}>2 minutos</option>
              <option value={300}>5 minutos</option>
            </select>
            <p className="field-help">Se avisa por el chat al empezar y en los últimos segundos. MercHosting lo hace aunque cierres la ventana.</p>
          </div>
        </Dialog>
      )}

      {confirmKill && (
        <Dialog
          title="Forzar el cierre del servidor"
          lead="El proceso se cerrará de inmediato, sin guardar. Úsalo solo si el servidor no responde a Detener."
          onClose={() => setConfirmKill(false)}
          footer={
            <>
              <button type="button" className="btn btn-quiet" onClick={() => setConfirmKill(false)}>
                Cancelar
              </button>
              <button
                type="button"
                className="btn btn-stop"
                disabled={!killChecked}
                onClick={() => {
                  setConfirmKill(false);
                  setKillChecked(false);
                  run("Forzando el cierre", () => sup.forceKill(inst.id));
                }}
              >
                Forzar cierre
              </button>
            </>
          }
        >
          <label className="check">
            <input type="checkbox" checked={killChecked} onChange={(e) => setKillChecked(e.target.checked)} />
            <span>Entiendo que se puede perder lo que no se haya guardado del mundo</span>
          </label>
        </Dialog>
      )}

      {cloning && (
        <CloneDialog
          sup={sup}
          inst={inst}
          onClose={() => setCloning(false)}
          onCloned={(newId) => {
            setCloning(false);
            onChanged();
            onOpenServer?.(newId);
          }}
        />
      )}

      {confirmRemove && (
        <Dialog
          title={`Quitar «${inst.name}» de la lista`}
          lead={
            deleteFiles
              ? "Se quitará de MercHosting y su carpeta irá a la Papelera de reciclaje de Windows, con el mundo, los mods y todo lo demás. Desde la Papelera la puedes recuperar mientras no la vacíes."
              : "MercHosting dejará de mostrar este servidor. Sus archivos, el mundo y los mods se quedan en la carpeta y no se borran."
          }
          onClose={() => {
            setConfirmRemove(false);
            setDeleteFiles(false);
          }}
          footer={
            <>
              <button
                type="button"
                className="btn btn-quiet"
                onClick={() => {
                  setConfirmRemove(false);
                  setDeleteFiles(false);
                }}
              >
                Cancelar
              </button>
              <button
                type="button"
                className={deleteFiles ? "btn btn-stop" : "btn"}
                onClick={() => {
                  const del = deleteFiles;
                  setConfirmRemove(false);
                  setDeleteFiles(false);
                  run(del ? "Mandando la carpeta a la Papelera" : "Quitando", async () => {
                    await sup.unregister(inst.id, del);
                    onRemoved();
                  });
                }}
              >
                {deleteFiles ? "Quitar y mandar a la Papelera" : "Quitar de la lista"}
              </button>
            </>
          }
        >
          <p className="path">{inst.root}</p>
          <label className="check">
            <input type="checkbox" checked={deleteFiles} onChange={(e) => setDeleteFiles(e.target.checked)} />
            <span>También borrar la carpeta del servidor (va a la Papelera de reciclaje)</span>
          </label>
          {deleteFiles && (
            <p className="field-help warn-text">
              Si quieres conservar el mundo, expórtalo antes desde la pestaña Mundos. Si la carpeta es más grande de lo que cabe en
              la Papelera, Windows te preguntará si quieres borrarla definitivamente; si dices que no, no se borra ni se quita nada.
            </p>
          )}
        </Dialog>
      )}
    </main>
    </>
  );
}
