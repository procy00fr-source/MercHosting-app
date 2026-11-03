// Asistente para crear un servidor nuevo desde la fuente oficial de cada proyecto.
// Todo lo que muestra viene del supervisor (catálogo real, Java real, progreso real).

import {
  Smartphone,
  Anvil,
  Box,
  Check,
  ChevronLeft,
  ChevronRight,
  CircleCheck,
  Combine,
  Droplet,
  Hammer,
  Layers,
  Sparkles,
  CircleX,
  ExternalLink,
  Feather,
  FolderOpen,
  LoaderCircle,
  Package,
  Power,
  Puzzle,
  Search,
  ShieldCheck,
  TriangleAlert,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { MercApi } from "../api/client";
import { Supervisor, errorMessage } from "../api/client";
import type {
  CatalogEntry,
  ContentHit,
  ContentSource,
  ContentVersion,
  Flavor,
  InstallStep,
  InstanceId,
  JavaChoice,
  JobSnapshot,
  LoaderOption,
  ModpackSummary,
  Resolved,
  SystemInfo,
} from "../api/types";
import { MercError } from "../api/types";
import { formatBytes, formatMemory, joinPath, safeFolderName } from "../format";
import { Dialog } from "./Dialog";
import { JavaInstall } from "./JavaInstall";
import { isLinux } from "../platform";

export const EULA_URL = "https://aka.ms/MinecraftEULA";
const JOB_POLL_MS = 500;

const FLAVOR_GROUPS: { title: string; items: { id: Flavor; title: string; text: string; icon: ReactNode }[] }[] = [
  {
    title: "Sin mods ni plugins",
    items: [
      {
        id: "vanilla",
        title: "Vanilla",
        text: "El servidor oficial de Mojang, sin cambios.",
        icon: <Box size={20} aria-hidden="true" />,
      },
    ],
  },
  {
    title: "Plugins (Bukkit / Spigot)",
    items: [
      {
        id: "paper",
        title: "Paper",
        text: "El más usado para plugins. Rápido y estable.",
        icon: <Feather size={20} aria-hidden="true" />,
      },
      {
        id: "purpur",
        title: "Purpur",
        text: "Paper con muchas opciones extra de juego.",
        icon: <Sparkles size={20} aria-hidden="true" />,
      },
      {
        id: "spigot",
        title: "Spigot",
        text: "El clásico. Se compila en tu PC con BuildTools (tarda unos minutos).",
        icon: <Droplet size={20} aria-hidden="true" />,
      },
    ],
  },
  {
    title: "Mods",
    items: [
      {
        id: "fabric",
        title: "Fabric",
        text: "Ligero. Muy usado para mods de rendimiento.",
        icon: <Puzzle size={20} aria-hidden="true" />,
      },
      {
        id: "quilt",
        title: "Quilt",
        text: "Basado en Fabric; acepta la mayoría de sus mods.",
        icon: <Layers size={20} aria-hidden="true" />,
      },
      {
        id: "forge",
        title: "Forge",
        text: "El cargador de mods clásico (1.12.2 a 1.16.5 y desde 1.17).",
        icon: <Hammer size={20} aria-hidden="true" />,
      },
      {
        id: "neo_forge",
        title: "NeoForge",
        text: "Sucesor de Forge, para modpacks modernos.",
        icon: <Anvil size={20} aria-hidden="true" />,
      },
    ],
  },
  {
    title: "Mods + plugins",
    items: [
      {
        id: "arclight",
        title: "Arclight",
        text: "Mods de NeoForge (o Fabric) junto con plugins. Algunas combinaciones pueden chocar.",
        icon: <Combine size={20} aria-hidden="true" />,
      },
    ],
  },
  {
    title: "Bedrock (celular, consola, Windows)",
    items: [
      {
        id: "bedrock",
        title: "Bedrock",
        text: "El servidor oficial de Mojang para Minecraft Bedrock. Solo entran jugadores de Bedrock; usa addons en vez de mods.",
        icon: <Smartphone size={20} aria-hidden="true" />,
      },
    ],
  },
];

const FLAVOR_TITLE: Record<Flavor, string> = {
  vanilla: "Vanilla",
  paper: "Paper",
  purpur: "Purpur",
  spigot: "Spigot",
  fabric: "Fabric",
  quilt: "Quilt",
  forge: "Forge",
  neo_forge: "NeoForge",
  arclight: "Arclight",
  bedrock: "Bedrock",
};

/** Tipos que se instalan ejecutando un instalador oficial (muestran ese paso en el progreso). */
const USES_INSTALLER: Flavor[] = ["neo_forge", "forge", "quilt", "spigot", "bedrock"];
/** Puerto UDP por defecto de Bedrock Dedicated Server. */
const BEDROCK_PORT = "19132";

type Page = "type" | "version" | "settings" | "eula" | "summary" | "progress";
/** Tipos con versión de loader elegible. */
const LOADER_FLAVORS: Flavor[] = ["fabric", "quilt", "forge", "neo_forge"];

const PAGES: { id: Page; label: string }[] = [
  { id: "type", label: "Tipo" },
  { id: "version", label: "Versión" },
  { id: "settings", label: "Ajustes" },
  { id: "eula", label: "EULA" },
  { id: "summary", label: "Resumen" },
  { id: "progress", label: "Creación" },
];

const STEP_LABEL: Record<InstallStep, string> = {
  preparing: "Preparar la carpeta",
  downloading: "Descargar y verificar",
  installing: "Instalador oficial",
  configuring: "Guardar la configuración",
  finishing: "Comprobar la instalación",
};

/** Plantillas: valores reales de server.properties (todos se muestran y se pueden cambiar). */
type TemplateId = "survival" | "creativo" | "modded" | "minijuegos";
const TEMPLATES: {
  id: TemplateId;
  label: string;
  help: string;
  gamemode: string;
  difficulty: string;
  maxPlayers: string;
  whitelist: boolean;
  extra: Record<string, string>;
  /** Memoria recomendada (MB), si la PC la tiene. */
  ram?: number;
}[] = [
  {
    id: "survival",
    label: "Supervivencia con amigos",
    help: "Normal, con PvP y lista blanca para que solo entren tus amigos.",
    gamemode: "survival",
    difficulty: "normal",
    maxPlayers: "10",
    whitelist: true,
    extra: { pvp: "true", "spawn-protection": "16", "view-distance": "10", "simulation-distance": "10" },
  },
  {
    id: "creativo",
    label: "Creativo",
    help: "Para construir: modo creativo, sin monstruos ni PvP, se puede volar y construir en el spawn.",
    gamemode: "creative",
    difficulty: "peaceful",
    maxPlayers: "10",
    whitelist: true,
    extra: { pvp: "false", "allow-flight": "true", "spawn-protection": "0", "view-distance": "12", "simulation-distance": "8" },
  },
  {
    id: "modded",
    label: "Con mods",
    help: "Distancias más cortas para que la PC aguante los mods, y vuelo permitido (muchos mods tienen jetpacks o alas y, si no, echan al jugador).",
    gamemode: "survival",
    difficulty: "normal",
    maxPlayers: "8",
    whitelist: true,
    extra: { pvp: "true", "allow-flight": "true", "spawn-protection": "0", "view-distance": "8", "simulation-distance": "6" },
    ram: 6144,
  },
  {
    id: "minijuegos",
    label: "Minijuegos",
    help: "Modo aventura (no se rompen bloques sin permiso), sin monstruos, con PvP y sin protección de spawn para tus mapas.",
    gamemode: "adventure",
    difficulty: "peaceful",
    maxPlayers: "20",
    whitelist: false,
    extra: { pvp: "true", "allow-flight": "true", "spawn-protection": "0", "view-distance": "8", "simulation-distance": "6" },
  },
];

const EXTRA_LABEL: Record<string, (v: string) => string> = {
  pvp: (v) => `PvP ${v === "true" ? "activado" : "desactivado"}`,
  "allow-flight": (v) => (v === "true" ? "se permite volar" : "no se permite volar"),
  "spawn-protection": (v) => (v === "0" ? "sin protección de spawn" : `protección de spawn de ${v} bloques`),
  "view-distance": (v) => `distancia de visión ${v}`,
  "simulation-distance": (v) => `distancia de simulación ${v}`,
};

const GAMEMODES = [
  ["survival", "Supervivencia"],
  ["creative", "Creativo"],
  ["adventure", "Aventura"],
] as const;
const DIFFICULTIES = [
  ["peaceful", "Pacífica"],
  ["easy", "Fácil"],
  ["normal", "Normal"],
  ["hard", "Difícil"],
] as const;

interface Props {
  api: MercApi;
  system: SystemInfo | null;
  /** Puertos que ya usan otros servidores registrados. */
  usedPorts: number[];
  onClose: () => void;
  onCreated: (id: InstanceId) => void;
  /** Se crea en tu servidor casero (otra PC): la carpeta y el Java los elige esa PC. */
  remote?: { hostName: string };
}

/** Java aceptable para una versión: Java 8 exacto para las antiguas; si no, igual o superior. */
/** Forge antiguo (1.12.2 y 1.13 – 1.16.5): instalador distinto y solo Java 8. */
export function isLegacyForge(mc: string): boolean {
  const p = mc.split(".");
  if (p[0] !== "1") return false;
  const minor = Number(p[1]);
  return (minor === 12 && p[2] === "2") || (minor >= 13 && minor <= 16);
}

function javaFits(major: number | null, required: number): boolean {
  if (major === null) return false;
  return required <= 8 ? major === 8 : major >= required;
}

function ramLimits(system: SystemInfo | null): { max: number } {
  const totalMb = system?.total_memory_bytes ? Math.floor(system.total_memory_bytes / 1024 ** 2) : null;
  // Se deja al menos 2 GB para el sistema.
  const max = totalMb ? Math.max(1024, Math.min(32768, Math.floor((totalMb - 2048) / 512) * 512)) : 16384;
  return { max };
}

export function CreateWizard({ api, system, usedPorts, onClose, onCreated, remote }: Props) {
  const sup = useMemo(() => new Supervisor(api), [api]);
  const [page, setPage] = useState<Page>("type");
  const [flavor, setFlavor] = useState<Flavor | null>(null);
  /** «server»: un tipo de servidor elegido a mano; «modpack»: lo fija un modpack de Modrinth. */
  const [mode, setMode] = useState<"server" | "modpack">("server");

  // Modpack
  const [packSource, setPackSource] = useState<ContentSource>("modrinth");
  const [packQuery, setPackQuery] = useState("");
  const [packHits, setPackHits] = useState<ContentHit[] | null>(null);
  const [packError, setPackError] = useState<string | null>(null);
  const [pack, setPack] = useState<ContentHit | null>(null);
  const [packVersions, setPackVersions] = useState<ContentVersion[] | null>(null);
  const [packVersionId, setPackVersionId] = useState("");
  const [packSummary, setPackSummary] = useState<ModpackSummary | null>(null);
  const [packInspecting, setPackInspecting] = useState(false);

  // Versión
  const [catalog, setCatalog] = useState<CatalogEntry[] | null>(null);
  const [catalogError, setCatalogError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [version, setVersion] = useState<CatalogEntry | null>(null);
  const [java, setJava] = useState<JavaChoice | null>(null);
  const [javaPath, setJavaPath] = useState("");
  const [javaLoading, setJavaLoading] = useState(false);

  // Ajustes
  const [defaults, setDefaults] = useState<{ servers_dir: string; suggested_port: number; curseforge: boolean } | null>(null);
  const [name, setName] = useState("");
  const [parentDir, setParentDir] = useState<string | null>(null);
  const [targetError, setTargetError] = useState<string | null>(null);
  const [targetChecking, setTargetChecking] = useState(false);
  const { max: ramMax } = ramLimits(system);
  const [ram, setRam] = useState(Math.min(4096, ramMax));
  const [port, setPort] = useState("");
  const [maxPlayers, setMaxPlayers] = useState("10");
  const [gamemode, setGamemode] = useState("survival");
  const [difficulty, setDifficulty] = useState("normal");
  const [seed, setSeed] = useState("");
  const [motd, setMotd] = useState("");
  const [onlineMode, setOnlineMode] = useState(true);
  const [whitelist, setWhitelist] = useState(false);
  const [keepInventory, setKeepInventory] = useState(false);
  /** Versiones del loader para elegir (Fabric, Quilt, Forge, NeoForge); "" = la recomendada. */
  const [loaderOpts, setLoaderOpts] = useState<LoaderOption[] | null>(null);
  const [loaderChoice, setLoaderChoice] = useState("");
  const [template, setTemplate] = useState<TemplateId | null>(null);
  const [extraProps, setExtraProps] = useState<Record<string, string>>({});

  // EULA y resumen
  const [eula, setEula] = useState(false);
  const [resolved, setResolved] = useState<Resolved | null>(null);
  const [resolveError, setResolveError] = useState<string | null>(null);
  const [resolveTry, setResolveTry] = useState(0);

  // Progreso
  const [job, setJob] = useState<JobSnapshot | null>(null);
  const [log, setLog] = useState<{ seq: number; text: string }[]>([]);
  const [createError, setCreateError] = useState<string | null>(null);
  const [cancelling, setCancelling] = useState(false);
  const [starting, setStarting] = useState(false);
  const logRef = useRef<HTMLDivElement>(null);

  // En el servidor casero la carpeta la pone esa PC (dentro de su carpeta de servidores).
  const path = remote
    ? defaults
      ? `${remote.hostName}/${safeFolderName(name)}`
      : null
    : defaults && parentDir
      ? joinPath(parentDir, safeFolderName(name))
      : null;
  const running = job?.state === "running";

  // --- Carga de datos ---

  async function loadCatalog(f: Flavor) {
    setCatalog(null);
    setCatalogError(null);
    try {
      setCatalog(await sup.catalog(f));
    } catch (e) {
      setCatalogError(
        e instanceof MercError && e.code === "network"
          ? `No se pudo consultar la lista oficial de versiones. Revisa tu conexión a Internet. (${e.message})`
          : errorMessage(e),
      );
    }
  }

  async function loadJava(entry: CatalogEntry) {
    setJavaLoading(true);
    try {
      const choice = await api.javaFor(entry.mc_version);
      setJava(choice);
      const fits = choice.candidates.filter((c) => javaFits(c.major, entry.java_major) && (flavor !== "spigot" || c.jdk));
      const rec = fits.find((c) => c.path === choice.recommended) ?? fits[0];
      setJavaPath(rec?.path ?? "");
    } catch (e) {
      setJava(null);
      setJavaPath("");
      setCatalogError(errorMessage(e));
    } finally {
      setJavaLoading(false);
    }
  }

  useEffect(() => {
    sup.creationDefaults().then(
      (d) => {
        setDefaults(d);
        setParentDir((cur) => cur ?? d.servers_dir);
        setPort((cur) => cur || String(d.suggested_port));
      },
      (e) => setCreateError(errorMessage(e)),
    );
  }, [sup]);

  // Comprobación de la carpeta destino (con una pequeña espera mientras se escribe).
  useEffect(() => {
    if (page !== "settings" || !path || remote) return;
    let alive = true;
    setTargetChecking(true);
    const t = setTimeout(() => {
      sup.checkTarget(path).then(
        () => alive && setTargetError(null),
        (e) => alive && setTargetError(errorMessage(e)),
      ).finally(() => alive && setTargetChecking(false));
    }, 300);
    return () => {
      alive = false;
      clearTimeout(t);
    };
  }, [page, path, sup, remote]);

  // Versiones del loader para la versión de Minecraft elegida.
  useEffect(() => {
    setLoaderChoice("");
    setLoaderOpts(null);
    if (mode !== "server" || !flavor || !version || !LOADER_FLAVORS.includes(flavor)) return;
    let alive = true;
    sup.loaderVersions(flavor, version.mc_version).then(
      (l) => alive && setLoaderOpts(l),
      () => alive && setLoaderOpts([]),
    );
    return () => {
      alive = false;
    };
  }, [mode, flavor, version, sup]);

  // Resolver la versión exacta al llegar al resumen.
  useEffect(() => {
    if (page !== "summary" || !flavor || !version) return;
    let alive = true;
    setResolved(null);
    setResolveError(null);
    sup.resolve(flavor, version.mc_version, mode === "modpack" ? (packSummary?.loader_version ?? null) : loaderChoice || null).then(
      (r) => alive && setResolved(r),
      (e) => alive && setResolveError(errorMessage(e)),
    );
    return () => {
      alive = false;
    };
  }, [page, flavor, version, sup, resolveTry, mode, packSummary, loaderChoice]);

  // Seguimiento del trabajo de creación.
  useEffect(() => {
    if (!job || job.state !== "running") return;
    let alive = true;
    const t = setTimeout(async () => {
      try {
        const next = await sup.job(job.job_id, job.next_after);
        if (!alive) return;
        if (next.log.length) setLog((l) => [...l, ...next.log].slice(-400));
        setJob(next);
      } catch (e) {
        if (alive) setCreateError(errorMessage(e));
      }
    }, JOB_POLL_MS);
    return () => {
      alive = false;
      clearTimeout(t);
    };
  }, [job, sup]);

  useEffect(() => {
    const el = logRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [log]);

  // --- Acciones ---

  function chooseFlavor(f: Flavor) {
    setMode("server");
    setFlavor(f);
    setVersion(null);
    setJava(null);
    setJavaPath("");
    setQuery("");
    setRam(Math.min(f === "neo_forge" ? 6144 : 4096, ramMax));
    // Bedrock escucha por UDP en su propio puerto; el resto, el puerto de Java sugerido.
    if (f === "bedrock") {
      setPort(BEDROCK_PORT);
      setTemplate(null);
      setExtraProps({});
      setKeepInventory(false);
    } else if (port === BEDROCK_PORT && defaults) setPort(String(defaults.suggested_port));
    loadCatalog(f);
  }

  async function searchPacks(q: string, source: ContentSource = packSource) {
    setPackError(null);
    setPackHits(null);
    try {
      setPackHits((await sup.modpackSearch(source, q, q.trim() ? "relevance" : "downloads")).hits);
    } catch (e) {
      setPackError(errorMessage(e));
    }
  }

  function chooseModpackMode(source: ContentSource) {
    const changed = source !== packSource;
    setPackSource(source);
    if (changed) {
      setPack(null);
      setPackVersions(null);
      setPackVersionId("");
      setPackSummary(null);
    }
    setMode("modpack");
    setFlavor(null);
    setVersion(null);
    setJava(null);
    setJavaPath("");
    setRam(Math.min(6144, ramMax));
    if (packHits === null || changed) searchPacks("", source);
  }

  async function choosePack(h: ContentHit) {
    setPack(h);
    setPackVersions(null);
    setPackVersionId("");
    setPackSummary(null);
    setPackError(null);
    try {
      const d = await sup.modpackProject(packSource, h.project_id);
      setPackVersions(d.versions);
      const first = d.versions.find((v) => v.version_type === "release") ?? d.versions[0];
      if (first) inspectPack(first.id, h.title);
    } catch (e) {
      setPackError(errorMessage(e));
    }
  }

  async function inspectPack(versionId: string, title: string) {
    setPackVersionId(versionId);
    setPackSummary(null);
    setFlavor(null);
    setVersion(null);
    setPackInspecting(true);
    setPackError(null);
    try {
      const sum = await sup.modpackInspect(packSource, versionId);
      setPackSummary(sum);
      setFlavor(sum.flavor);
      const entry: CatalogEntry = { mc_version: sum.mc_version, java_major: sum.java_major, latest: false };
      setVersion(entry);
      setName((cur) => (!cur || cur === title || /^[A-Za-z]+ \d/.test(cur) ? title : cur));
      loadJava(entry);
    } catch (e) {
      setPackError(errorMessage(e));
    } finally {
      setPackInspecting(false);
    }
  }

  function chooseVersion(entry: CatalogEntry) {
    const autoName = flavor && version ? `${FLAVOR_TITLE[flavor]} ${version.mc_version}` : "";
    setVersion(entry);
    if (flavor && (!name || name === autoName)) setName(`${FLAVOR_TITLE[flavor]} ${entry.mc_version}`);
    // Bedrock Dedicated Server es un programa nativo: no usa Java.
    if (flavor === "bedrock") {
      setJava(null);
      setJavaPath("");
      return;
    }
    loadJava(entry);
  }

  async function pickParent() {
    const picked = await api.pickFolder();
    if (picked) setParentDir(picked);
  }

  function properties(): Record<string, string> {
    if (flavor === "bedrock") {
      const b: Record<string, string> = {
        "server-port": port,
        "max-players": maxPlayers,
        gamemode,
        difficulty,
        "online-mode": String(onlineMode),
        "allow-list": String(whitelist),
        "server-name": (motd.trim() || name.trim()).slice(0, 64),
      };
      if (seed.trim()) b["level-seed"] = seed.trim();
      return b;
    }
    const p: Record<string, string> = {
      "server-port": port,
      "max-players": maxPlayers,
      gamemode,
      difficulty,
      "online-mode": String(onlineMode),
      "white-list": String(whitelist),
    };
    Object.assign(p, extraProps);
    if (whitelist) p["enforce-whitelist"] = "true";
    if (seed.trim()) p["level-seed"] = seed.trim();
    if (motd.trim()) p.motd = motd.trim();
    return p;
  }

  async function create() {
    if (!flavor || !version || !path) return;
    setCreateError(null);
    setLog([]);
    setPage("progress");
    try {
      const j = await sup.createServer({
        name: name.trim(),
        flavor,
        mc_version: version.mc_version,
        path,
        java: isBedrock ? "" : javaPath,
        min_memory_mb: 0,
        max_memory_mb: isBedrock ? 0 : ram,
        properties: properties(),
        accept_eula: eula,
        modpack_version: mode === "modpack" ? packVersionId : null,
        modpack_source: packSource,
        loader_version: mode === "server" ? loaderChoice || null : null,
        game_rules: keepInventory && !isBedrock ? { keep_inventory: "true" } : {},
      });
      setLog(j.log);
      setJob(j);
    } catch (e) {
      setCreateError(errorMessage(e));
    }
  }

  async function cancel() {
    if (!job) return;
    setCancelling(true);
    try {
      await sup.cancelJob(job.job_id);
    } catch (e) {
      setCreateError(errorMessage(e));
    }
  }

  async function startNow(id: InstanceId) {
    setStarting(true);
    try {
      await sup.start(id);
      onCreated(id);
    } catch (e) {
      setCreateError(errorMessage(e));
      setStarting(false);
    }
  }

  function close() {
    if (running) return; // El trabajo sigue en el supervisor: hay que esperar o cancelar.
    if (job?.state === "succeeded" && job.instance_id) onCreated(job.instance_id);
    else onClose();
  }

  // --- Validación por página ---

  const portNum = Number(port);
  const portValid = Number.isInteger(portNum) && portNum >= 1024 && portNum <= 65535;
  const portTaken = portValid && usedPorts.includes(portNum);
  const playersNum = Number(maxPlayers);
  const playersValid = Number.isInteger(playersNum) && playersNum >= 1 && playersNum <= 1000;
  const needsJdk = flavor === "spigot";
  const isBedrock = flavor === "bedrock";
  const usableJava =
    java?.candidates.filter((c) => version && javaFits(c.major, version.java_major) && (!needsJdk || c.jdk)) ?? [];

  const canNext: Record<Page, boolean> = {
    type: !!flavor || mode === "modpack",
    version: !!version && (isBedrock || (!!javaPath && !javaLoading)) && (mode === "server" || !!packSummary),
    settings:
      name.trim().length > 0 && !!path && !targetError && !targetChecking && portValid && playersValid && (isBedrock || ram >= 1024),
    eula: true,
    summary: !!resolved,
    progress: false,
  };
  const pageIndex = PAGES.findIndex((p) => p.id === page);
  const next = () => setPage(PAGES[pageIndex + 1].id);
  const back = () => setPage(PAGES[pageIndex - 1].id);

  const filtered = (catalog ?? []).filter((e) => e.mc_version.includes(query.trim()));

  // --- Pie del diálogo ---

  let footer: ReactNode;
  if (page === "progress") {
    footer = running ? (
      <button type="button" className="btn btn-danger-ghost" onClick={cancel} disabled={cancelling}>
        {cancelling ? "Cancelando…" : "Cancelar creación"}
      </button>
    ) : job?.state === "succeeded" && job.instance_id ? (
      eula ? (
        <>
          <button type="button" className="btn" onClick={() => onCreated(job.instance_id!)}>
            Ir al servidor
          </button>
          <button type="button" className="btn btn-go btn-lg" disabled={starting} onClick={() => startNow(job.instance_id!)}>
            <Power size={17} aria-hidden="true" /> {starting ? "Encendiendo…" : "Encender ahora"}
          </button>
        </>
      ) : (
        <button type="button" className="btn btn-go btn-lg" onClick={() => onCreated(job.instance_id!)}>
          Ir al servidor <ChevronRight size={17} aria-hidden="true" />
        </button>
      )
    ) : (
      <>
        <button type="button" className="btn btn-quiet" onClick={onClose}>
          Cerrar
        </button>
        <button
          type="button"
          className="btn"
          onClick={() => {
            setJob(null);
            setCancelling(false);
            setPage("summary");
          }}
        >
          <ChevronLeft size={16} aria-hidden="true" /> Volver al resumen
        </button>
      </>
    );
  } else {
    footer = (
      <>
        <button type="button" className="btn btn-quiet" onClick={onClose}>
          Cancelar
        </button>
        {pageIndex > 0 && (
          <button type="button" className="btn" onClick={back}>
            <ChevronLeft size={16} aria-hidden="true" /> Atrás
          </button>
        )}
        {page === "summary" ? (
          <button type="button" className="btn btn-go" disabled={!canNext.summary} onClick={create}>
            Crear servidor
          </button>
        ) : (
          <button type="button" className="btn btn-go" disabled={!canNext[page]} onClick={next}>
            Siguiente <ChevronRight size={16} aria-hidden="true" />
          </button>
        )}
      </>
    );
  }

  return (
    <Dialog
      title="Crear un servidor"
      wide
      lead="MercHosting descarga el servidor desde su fuente oficial, comprueba que el archivo no esté dañado y lo deja listo en tu PC."
      onClose={close}
      footer={footer}
    >
      <ol className="wiz-steps" aria-label="Pasos" data-tour="wizard-pasos">
        {PAGES.map((p, i) => (
          <li key={p.id} data-state={i < pageIndex ? "done" : i === pageIndex ? "current" : "todo"} aria-current={i === pageIndex ? "step" : undefined}>
            <span className="wiz-dot" aria-hidden="true">
              {i < pageIndex ? <Check size={12} /> : i + 1}
            </span>
            {p.label}
          </li>
        ))}
      </ol>

      {page === "type" && (
        <div className="flavor-groups" data-tour="wizard-tipo">
          {FLAVOR_GROUPS.map((g) => (
            <section key={g.title} className="flavor-group">
              <h3>{g.title}</h3>
              <div className="flavor-grid" role="group" aria-label={g.title}>
                {g.items.map((f) => (
                  <button
                    key={f.id}
                    type="button"
                    className="flavor-card"
                    aria-pressed={mode === "server" && flavor === f.id}
                    onClick={() => chooseFlavor(f.id)}
                  >
                    <span className="flavor-icon">{f.icon}</span>
                    <span className="flavor-title">{f.title}</span>
                    <span className="flavor-text">{f.text}</span>
                  </button>
                ))}
              </div>
            </section>
          ))}
          <section className="flavor-group">
            <h3>Modpack</h3>
            <div className="flavor-grid" role="group" aria-label="Modpack">
              <button
                type="button"
                className="flavor-card"
                aria-pressed={mode === "modpack" && packSource === "modrinth"}
                onClick={() => chooseModpackMode("modrinth")}
              >
                <span className="flavor-icon">
                  <Package size={20} aria-hidden="true" />
                </span>
                <span className="flavor-title">Modpack de Modrinth</span>
                <span className="flavor-text">Elige un modpack y MercHosting instala el loader exacto y sus mods de servidor.</span>
              </button>
              <button
                type="button"
                className="flavor-card"
                aria-pressed={mode === "modpack" && packSource === "curseforge"}
                disabled={!defaults?.curseforge}
                onClick={() => chooseModpackMode("curseforge")}
              >
                <span className="flavor-icon">
                  <Package size={20} aria-hidden="true" />
                </span>
                <span className="flavor-title">
                  Modpack de CurseForge {!defaults?.curseforge && <span className="badge badge-muted">Próximamente</span>}
                </span>
                <span className="flavor-text">
                  {defaults?.curseforge
                    ? "Igual que con Modrinth. Si un autor no permite descargar su mod desde otras apps, te dirá cuál bajar a mano."
                    : "Estará disponible cuando MercHosting tenga su clave oficial de CurseForge."}
                </span>
              </button>
            </div>
          </section>
        </div>
      )}

      {page === "version" && mode === "modpack" && (
        <div className="wiz-page">
          <form
            className="version-search"
            onSubmit={(e) => {
              e.preventDefault();
              searchPacks(packQuery);
            }}
          >
            <Search size={15} aria-hidden="true" />
            <input
              type="text"
              aria-label="Buscar modpack"
              placeholder={`Buscar modpacks en ${packSource === "curseforge" ? "CurseForge" : "Modrinth"}…`}
              value={packQuery}
              maxLength={100}
              onChange={(e) => setPackQuery(e.target.value)}
            />
          </form>
          {packError && !pack && (
            <div className="wiz-alert" role="alert">
              <TriangleAlert size={18} aria-hidden="true" />
              <div>
                <p>{packError}</p>
              </div>
            </div>
          )}
          {packHits === null && !packError ? (
            <p className="wiz-loading" role="status">
              <LoaderCircle size={16} className="spin" aria-hidden="true" /> Consultando{" "}
              {packSource === "curseforge" ? "CurseForge" : "Modrinth"}…
            </p>
          ) : (
            <div className="version-list pack-list" role="group" aria-label="Modpacks">
              {(packHits ?? []).map((h) => (
                <button
                  key={h.project_id}
                  type="button"
                  className="version-item pack-item"
                  aria-pressed={pack?.project_id === h.project_id}
                  onClick={() => choosePack(h)}
                >
                  <span className="version-num">{h.title}</span>
                  <span className="version-java">{h.description}</span>
                </button>
              ))}
              {packHits?.length === 0 && <p className="field-help">Ningún modpack coincide con «{packQuery}».</p>}
            </div>
          )}

          {pack && (
            <div className="detect-box" aria-live="polite">
              <h3>{pack.title}</h3>
              {packVersions === null && packError ? null : packVersions === null ? (
                <p className="field-help" role="status">
                  Cargando versiones…
                </p>
              ) : packVersions.length === 0 ? (
                <p className="form-error" role="alert">
                  Este modpack no tiene versiones publicadas.
                </p>
              ) : (
                <div className="field">
                  <label htmlFor="wiz-pack-version">Versión del modpack</label>
                  <select
                    id="wiz-pack-version"
                    value={packVersionId}
                    disabled={packInspecting}
                    onChange={(e) => inspectPack(e.target.value, pack.title)}
                  >
                    {packVersions.map((v) => (
                      <option key={v.id} value={v.id}>
                        {v.version_number} · Minecraft {v.game_versions.join(", ")} · {v.loaders.join(", ")}
                        {v.version_type !== "release" ? " (prueba)" : ""}
                      </option>
                    ))}
                  </select>
                </div>
              )}
              {packInspecting && (
                <p className="field-help" role="status">
                  Descargando y revisando el modpack…
                </p>
              )}
              {packError && !packInspecting && (
                <div className="wiz-alert" role="alert">
                  <TriangleAlert size={18} aria-hidden="true" />
                  <div>
                    <p>No se pudo revisar este modpack: {packError}</p>
                    <button
                      type="button"
                      className="btn btn-sm"
                      onClick={() => (packVersionId ? inspectPack(packVersionId, pack.title) : choosePack(pack))}
                    >
                      Volver a intentar
                    </button>
                  </div>
                </div>
              )}
              {packSummary && (
                <>
                  <p className="field-help">
                    Se creará un servidor <strong>{FLAVOR_TITLE[packSummary.flavor]}</strong> para Minecraft {packSummary.mc_version}
                    {packSummary.loader_version ? ` (loader ${packSummary.loader_version}, el que pide el modpack)` : ""} con{" "}
                    {packSummary.server_files} archivos ({formatBytes(packSummary.download_bytes)}).
                  </p>
                  <ul className="wiz-notes">
                    {packSummary.warnings.map((w) => (
                      <li key={w}>{w}</li>
                    ))}
                  </ul>
                  {javaLoading ? (
                    <p className="field-help" role="status">
                      {remote ? `Buscando Java en ${remote.hostName}…` : "Buscando Java en tu PC…"}
                    </p>
                  ) : usableJava.length > 0 ? (
                    <div className="field">
                      <label htmlFor="wiz-pack-java">Java que usará el servidor</label>
                      <select id="wiz-pack-java" value={javaPath} onChange={(e) => setJavaPath(e.target.value)}>
                        {usableJava.map((c) => (
                          <option key={c.path} value={c.path}>
                            Java {c.major}
                            {c.path === java?.recommended ? " (recomendado)" : ""}
                            {remote ? ` · en ${remote.hostName}` : ` en ${c.path}`}
                          </option>
                        ))}
                      </select>
                    </div>
                  ) : (
                    version && (
                      <>
                        <p className="form-error" role="alert">
                          Este modpack necesita Java {version.java_major} y no lo encontré {remote ? `en ${remote.hostName}` : "en tu PC"}.
                          MercHosting puede descargarlo por ti:
                        </p>
                        <JavaInstall sup={sup} minMajor={version.java_major} onInstalled={() => loadJava(version)} />
                      </>
                    )
                  )}
                </>
              )}
            </div>
          )}
        </div>
      )}

      {page === "version" && mode === "server" && flavor && (
        <div className="wiz-page">
          {catalogError ? (
            <div className="wiz-alert" role="alert">
              <TriangleAlert size={18} aria-hidden="true" />
              <div>
                <p>{catalogError}</p>
                <button type="button" className="btn btn-sm" onClick={() => loadCatalog(flavor)}>
                  Reintentar
                </button>
              </div>
            </div>
          ) : catalog === null ? (
            <p className="wiz-loading" role="status">
              <LoaderCircle size={16} className="spin" aria-hidden="true" /> Consultando las versiones oficiales de{" "}
              {FLAVOR_TITLE[flavor]}…
            </p>
          ) : (
            <>
              <div className="version-search">
                <Search size={15} aria-hidden="true" />
                <input
                  type="text"
                  aria-label="Buscar versión"
                  placeholder="Buscar versión (por ejemplo 1.21)"
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                />
              </div>
              <div className="version-list" role="group" aria-label="Versiones de Minecraft">
                {filtered.map((e) => (
                  <button
                    key={e.mc_version}
                    type="button"
                    className="version-item"
                    aria-pressed={version?.mc_version === e.mc_version}
                    onClick={() => chooseVersion(e)}
                  >
                    <span className="version-num">{e.mc_version}</span>
                    <span className="version-java">{isBedrock ? "Sin Java" : `Java ${e.java_major}`}</span>
                    {e.latest && <span className="badge">Última</span>}
                  </button>
                ))}
                {filtered.length === 0 && <p className="field-help">Ninguna versión coincide con «{query}».</p>}
              </div>
            </>
          )}

          {version && isBedrock && (
            <div className="detect-box" aria-live="polite">
              <h3>Bedrock Dedicated Server {version.mc_version}</h3>
              <p className="field-help">
                {isLinux()
                  ? "Es un programa de Mojang que en Linux solo funciona oficialmente en Ubuntu 22.04 o más nuevo: no necesita Java. Mojang solo ofrece la versión más reciente, y los jugadores necesitan una versión de Bedrock compatible con ella."
                  : "Es un programa de Mojang para Windows 10 o más nuevo: no necesita Java. Mojang solo ofrece la versión más reciente, y los jugadores necesitan una versión de Bedrock compatible con ella."}
              </p>
            </div>
          )}

          {version && flavor === "forge" && isLegacyForge(version.mc_version) && (
            <div className="wiz-alert" role="note" aria-label="Forge antiguo">
              <TriangleAlert size={18} aria-hidden="true" />
              <p>
                Forge {version.mc_version} es una versión antigua: funciona solo con <strong>Java 8</strong> (MercHosting puede
                descargarlo) y con mods hechos para {version.mc_version}. El buscador de mods filtra por esta versión.
              </p>
            </div>
          )}

          {version && !isBedrock && (
            <div className="detect-box" aria-live="polite">
              <h3>Java para Minecraft {version.mc_version}</h3>
              {javaLoading ? (
                <p className="field-help" role="status">
                  {remote ? `Buscando Java en ${remote.hostName}…` : "Buscando Java en tu PC…"}
                </p>
              ) : usableJava.length > 0 ? (
                <div className="field">
                  <label htmlFor="wiz-java">Java que usará el servidor</label>
                  <select id="wiz-java" value={javaPath} onChange={(e) => setJavaPath(e.target.value)}>
                    {usableJava.map((c) => (
                      <option key={c.path} value={c.path}>
                        Java {c.major}
                        {c.path === java?.recommended ? " (recomendado)" : ""}
                            {remote ? ` · en ${remote.hostName}` : ` en ${c.path}`}
                      </option>
                    ))}
                  </select>
                </div>
              ) : (
                <div className="wiz-alert" role="alert">
                  <TriangleAlert size={18} aria-hidden="true" />
                  <div>
                    <p>
                      {needsJdk
                        ? `Spigot se compila en tu PC y necesita un JDK ${version.java_major} (Java con compilador). No encontré uno: instala, por ejemplo, Eclipse Temurin JDK ${version.java_major} y pulsa «Buscar de nuevo».`
                        : `Esta versión necesita Java ${version.java_major} y no lo encontré ${remote ? `en ${remote.hostName}` : "en tu PC"}. MercHosting puede descargarlo por ti:`}
                    </p>
                    {!needsJdk && <JavaInstall sup={sup} minMajor={version.java_major} onInstalled={() => loadJava(version)} />}
                    <button type="button" className="btn btn-sm" onClick={() => loadJava(version)}>
                      Buscar de nuevo
                    </button>
                  </div>
                </div>
              )}
            </div>
          )}

          {version && LOADER_FLAVORS.includes(flavor) && (
            <div className="detect-box">
              <h3>Versión de {FLAVOR_TITLE[flavor]}</h3>
              {loaderOpts === null ? (
                <p className="field-help" role="status">
                  Consultando versiones…
                </p>
              ) : loaderOpts.length === 0 ? (
                <p className="field-help">Se usará la versión recomendada de la fuente oficial.</p>
              ) : (
                <div className="field">
                  <label htmlFor="wiz-loader">Versión del loader</label>
                  <select id="wiz-loader" value={loaderChoice} onChange={(e) => setLoaderChoice(e.target.value)}>
                    <option value="">Recomendada ({loaderOpts.find((o) => o.recommended)?.version ?? "la más estable"})</option>
                    {loaderOpts.map((o) => (
                      <option key={o.version} value={o.version}>
                        {o.version}
                        {o.stable ? "" : " (beta)"}
                      </option>
                    ))}
                  </select>
                  <span className="field-help">Elige otra solo si un mod o modpack te pide una versión concreta.</span>
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {page === "settings" && (
        <div className="wiz-page settings-grid">
          {!isBedrock && (
          <fieldset className="field span-2 template-row">
            <legend className="field-title">Plantilla (opcional)</legend>
            <div className="template-options">
              {TEMPLATES.map((t) => (
                <label key={t.id} className="role-option" data-on={template === t.id}>
                  <input
                    type="radio"
                    name="plantilla"
                    checked={template === t.id}
                    onChange={() => {
                      setTemplate(t.id);
                      setGamemode(t.gamemode);
                      setDifficulty(t.difficulty);
                      setMaxPlayers(t.maxPlayers);
                      setWhitelist(t.whitelist);
                      setExtraProps(t.extra);
                      if (t.ram) setRam((cur) => Math.max(cur, Math.min(t.ram!, ramMax)));
                    }}
                  />
                  <span>
                    <strong>{t.label}</strong>
                    <span className="field-help">{t.help}</span>
                  </span>
                </label>
              ))}
            </div>
            {template && (
              <p className="field-help" aria-live="polite">
                Además fija: {Object.entries(extraProps).map(([k, v]) => (EXTRA_LABEL[k] ? EXTRA_LABEL[k](v) : `${k}=${v}`)).join(", ")}. Lo
                demás lo ves y lo puedes cambiar abajo; todo se puede cambiar después en Ajustes.{" "}
                <button
                  type="button"
                  className="link"
                  onClick={() => {
                    setTemplate(null);
                    setExtraProps({});
                  }}
                >
                  Quitar plantilla
                </button>
              </p>
            )}
          </fieldset>
          )}

          <div className="field span-2">
            <label htmlFor="wiz-name">Nombre del servidor</label>
            <input id="wiz-name" type="text" value={name} maxLength={64} onChange={(e) => setName(e.target.value)} />
          </div>

          <div className="field span-2" role="group" aria-labelledby="wiz-folder-title">
            <span id="wiz-folder-title" className="field-title">
              Carpeta
            </span>
            {remote ? (
              <p className="field-help">
                La elige <strong translate="no">{remote.hostName}</strong>: una carpeta nueva dentro de su carpeta de servidores, con el
                nombre del servidor. Otra PC nunca decide dónde se escribe.
              </p>
            ) : (
            <div className="folder-pick">
              <button type="button" className="btn btn-sm" onClick={pickParent}>
                <FolderOpen size={15} aria-hidden="true" /> Cambiar ubicación
              </button>
              <span className="folder-path">{path ?? "…"}</span>
            </div>
            )}
            {remote ? null : targetError ? (
              <p className="form-error" role="alert">
                {targetError}
              </p>
            ) : (
              <p className="field-help">Se crea una carpeta nueva. MercHosting nunca escribe en una carpeta que ya tenga archivos.</p>
            )}
          </div>

          {!isBedrock && (
          <div className="field span-2">
            <label htmlFor="wiz-ram">Memoria máxima</label>
            <div className="ram-row">
              <input
                id="wiz-ram"
                type="range"
                min={1024}
                max={ramMax}
                step={512}
                value={ram}
                onChange={(e) => setRam(Number(e.target.value))}
              />
              <span className="ram-value">{formatMemory(ram)}</span>
            </div>
            <p className="field-help">
              {system?.total_memory_bytes
                ? `Tu PC tiene ${formatBytes(system.total_memory_bytes)}. Deja memoria para el sistema y, si juegas aquí, para Minecraft.`
                : "Deja memoria para el sistema y, si juegas en esta PC, para Minecraft."}
              {flavor === "neo_forge" || flavor === "fabric" ? " Los mods suelen necesitar más memoria." : ""}
            </p>
          </div>
          )}

          <div className="field">
            <label htmlFor="wiz-port">Puerto</label>
            <input id="wiz-port" type="text" inputMode="numeric" value={port} onChange={(e) => setPort(e.target.value.replace(/\D/g, ""))} />
            {!portValid ? (
              <p className="form-error">Usa un número entre 1024 y 65535.</p>
            ) : portTaken ? (
              <p className="field-help warn-text">Otro de tus servidores ya usa este puerto; no podrán estar encendidos a la vez.</p>
            ) : (
              <p className="field-help">{isBedrock ? "El habitual de Bedrock es 19132 (UDP)." : "El habitual de Minecraft es 25565."}</p>
            )}
          </div>

          <div className="field">
            <label htmlFor="wiz-players">Jugadores máximos</label>
            <input
              id="wiz-players"
              type="text"
              inputMode="numeric"
              value={maxPlayers}
              onChange={(e) => setMaxPlayers(e.target.value.replace(/\D/g, ""))}
            />
            {!playersValid && <p className="form-error">Entre 1 y 1000.</p>}
          </div>

          <div className="field">
            <label htmlFor="wiz-mode">Modo de juego</label>
            <select id="wiz-mode" value={gamemode} onChange={(e) => setGamemode(e.target.value)}>
              {GAMEMODES.map(([v, l]) => (
                <option key={v} value={v}>
                  {l}
                </option>
              ))}
            </select>
          </div>

          <div className="field">
            <label htmlFor="wiz-diff">Dificultad</label>
            <select id="wiz-diff" value={difficulty} onChange={(e) => setDifficulty(e.target.value)}>
              {DIFFICULTIES.map(([v, l]) => (
                <option key={v} value={v}>
                  {l}
                </option>
              ))}
            </select>
          </div>

          <div className="field">
            <label htmlFor="wiz-motd">{isBedrock ? "Nombre en la lista de servidores (opcional)" : "Mensaje en la lista (opcional)"}</label>
            <input
              id="wiz-motd"
              type="text"
              maxLength={isBedrock ? 64 : 120}
              placeholder={isBedrock ? "Vacío = el nombre del servidor" : undefined}
              value={motd}
              onChange={(e) => setMotd(e.target.value)}
            />
          </div>

          <div className="field">
            <label htmlFor="wiz-seed">Semilla del mundo (opcional)</label>
            <input
              id="wiz-seed"
              type="text"
              maxLength={64}
              placeholder="Vacío = mundo aleatorio"
              value={seed}
              onChange={(e) => setSeed(e.target.value)}
            />
          </div>

          {!isBedrock && (
          <label className="switch-row span-2">
            <input type="checkbox" checked={keepInventory} onChange={(e) => setKeepInventory(e.target.checked)} />
            <span>
              <strong>Conservar el inventario al morir (keepInventory)</strong>
              <span className="field-help">Es una regla del juego: se aplica sola la primera vez que el servidor quede en línea.</span>
            </span>
          </label>
          )}

          <label className="switch-row span-2">
            <input type="checkbox" checked={whitelist} onChange={(e) => setWhitelist(e.target.checked)} />
            <span>
              <strong>{isBedrock ? "Lista de permitidos (allow-list)" : "Lista blanca"}</strong>
              <span className="field-help">Solo podrán entrar los jugadores que agregues.</span>
            </span>
          </label>

          <label className="switch-row span-2">
            <input type="checkbox" checked={onlineMode} onChange={(e) => setOnlineMode(e.target.checked)} />
            <span>
              <strong>
                {isBedrock ? "Comprobar cuentas de Xbox/Microsoft (online-mode)" : "Comprobar cuentas oficiales de Minecraft (online-mode)"}
              </strong>
              <span className="field-help">
                {isBedrock
                  ? "Recomendado. Cada jugador entra con su cuenta de Microsoft verificada."
                  : "Recomendado. Verifica con Mojang que cada jugador es quien dice ser."}
              </span>
            </span>
          </label>
          {!onlineMode && (
            <div className="wiz-alert span-2" role="alert">
              <TriangleAlert size={18} aria-hidden="true" />
              <p>
                Sin esta comprobación cualquiera puede entrar usando cualquier nombre, incluido el tuyo o el de un
                administrador. Actívala salvo que sepas exactamente por qué la necesitas.
              </p>
            </div>
          )}
        </div>
      )}

      {page === "eula" && (
        <div className="wiz-page">
          <div className="detect-box">
            <h3>EULA de Minecraft</h3>
            <p className="field-help">
              Para encender cualquier servidor de Minecraft hay que aceptar el Acuerdo de Licencia de Usuario Final (EULA)
              de Mojang. MercHosting no lo acepta por ti: solo lo marca si tú lo decides aquí.
            </p>
            {isBedrock && (
              <p className="field-help">
                Al descargar el servidor de Bedrock, Mojang pide aceptar también su Política de privacidad (enlazada desde la EULA).
                Bedrock no usa el archivo eula.txt: MercHosting guarda ahí tu aceptación solo para no encenderlo sin ella.
              </p>
            )}
            <button type="button" className="btn btn-sm" onClick={() => api.openUrl(EULA_URL)}>
              <ExternalLink size={14} aria-hidden="true" /> Leer la EULA (aka.ms/MinecraftEULA)
            </button>
          </div>
          <label className="switch-row">
            <input type="checkbox" checked={eula} onChange={(e) => setEula(e.target.checked)} />
            <span>
              <strong>
                {isBedrock
                  ? "He leído y acepto la EULA y la Política de privacidad de Minecraft"
                  : "He leído y acepto la EULA de Minecraft"}
              </strong>
              <span className="field-help">
                Si no la aceptas ahora, el servidor se crea igual pero no arrancará hasta que la aceptes.
              </span>
            </span>
          </label>
        </div>
      )}

      {page === "summary" && flavor && version && (
        <div className="wiz-page">
          {resolveError ? (
            <div className="wiz-alert" role="alert">
              <TriangleAlert size={18} aria-hidden="true" />
              <div>
                <p>{resolveError}</p>
                <button type="button" className="btn btn-sm" onClick={() => setResolveTry((n) => n + 1)}>
                  Reintentar
                </button>
              </div>
            </div>
          ) : !resolved ? (
            <p className="wiz-loading" role="status">
              <LoaderCircle size={16} className="spin" aria-hidden="true" /> Preparando la descarga oficial…
            </p>
          ) : (
            <>
              <dl className="summary-grid">
                <dt>Servidor</dt>
                <dd>
                  {FLAVOR_TITLE[flavor]} {resolved.mc_version}
                  {resolved.loader_version
                    ? ` · ${["paper", "purpur"].includes(flavor) ? "compilación" : "versión"} ${resolved.loader_version}`
                    : ""}
                </dd>
                {mode === "modpack" && packSummary && (
                  <>
                    <dt>Modpack</dt>
                    <dd>
                      {packSummary.name} {packSummary.pack_version} · {packSummary.server_files} archivos verificados por SHA-1 (
                      {formatBytes(packSummary.download_bytes)})
                    </dd>
                  </>
                )}
                <dt>Nombre</dt>
                <dd>{name.trim()}</dd>
                <dt>Carpeta</dt>
                <dd className={remote ? undefined : "mono"}>{remote ? `La elige ${remote.hostName}` : path}</dd>
                {isBedrock ? (
                  <>
                    <dt>Java</dt>
                    <dd>No hace falta (programa nativo de Mojang)</dd>
                  </>
                ) : (
                  <>
                    <dt>Java</dt>
                    <dd className={remote ? undefined : "mono"}>
                      {remote ? `Java ${usableJava.find((c) => c.path === javaPath)?.major ?? version?.java_major ?? ""} de ${remote.hostName}` : javaPath}
                    </dd>
                    <dt>Memoria</dt>
                    <dd>{formatMemory(ram)}</dd>
                  </>
                )}
                <dt>Puerto</dt>
                <dd>{port}</dd>
                <dt>{resolved.installer_args ? "Instalador oficial" : "Archivo"}</dt>
                <dd>
                  {resolved.artifact.file_name}
                  {resolved.artifact.size ? ` · ${formatBytes(resolved.artifact.size)}` : ""}
                </dd>
                <dt>Verificación</dt>
                <dd>
                  {resolved.artifact.checksum.algorithm === "none" ? (
                    isBedrock ? "Por HTTPS desde minecraft.net (Mojang no publica huella; se anota su SHA-256)" : "Por HTTPS (la fuente no publica huella)"
                  ) : (
                    <>
                      <ShieldCheck size={14} aria-hidden="true" className="inline-icon" /> Huella{" "}
                      {resolved.artifact.checksum.algorithm === "sha1" ? "SHA-1" : "SHA-256"} publicada por la fuente
                    </>
                  )}
                </dd>
                <dt>EULA</dt>
                <dd>{eula ? "Aceptada por ti" : "Pendiente (no arrancará hasta aceptarla)"}</dd>
              </dl>
              {(resolved.experimental || resolved.notes.length > 0) && (
                <ul className="wiz-notes">
                  {resolved.notes.map((n) => (
                    <li key={n}>{n}</li>
                  ))}
                </ul>
              )}
            </>
          )}
        </div>
      )}

      {page === "progress" && (
        <div className="wiz-page">
          {job ? (
            <>
              <ol className="job-steps">
                {(["preparing", "downloading", ...(flavor && USES_INSTALLER.includes(flavor) ? ["installing"] : []), "configuring", "finishing"] as InstallStep[]).map(
                  (s, i, all) => {
                    const cur = job.step ? all.indexOf(job.step) : -1;
                    const state =
                      job.state === "succeeded" || i < cur ? "done" : i === cur ? (job.state === "running" ? "active" : "stopped") : "todo";
                    return (
                      <li key={s} data-state={state}>
                        <span className="job-dot" aria-hidden="true">
                          {state === "done" ? <Check size={12} /> : state === "active" ? <LoaderCircle size={12} className="spin" /> : null}
                        </span>
                        {STEP_LABEL[s]}
                      </li>
                    );
                  },
                )}
              </ol>

              {job.download && (
                <div className="dl">
                  <div className="dl-head">
                    <span className="mono">{job.download.file}</span>
                    <span>
                      {formatBytes(job.download.done)}
                      {job.download.total ? ` de ${formatBytes(job.download.total)}` : ""}
                    </span>
                  </div>
                  <div
                    className="dl-bar"
                    role="progressbar"
                    aria-label="Descarga"
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-valuenow={job.download.total ? Math.round((job.download.done / job.download.total) * 100) : undefined}
                  >
                    <span
                      style={{ width: job.download.total ? `${(job.download.done / job.download.total) * 100}%` : "35%" }}
                      data-indeterminate={!job.download.total}
                    />
                  </div>
                </div>
              )}

              <p className="job-message" role="status" data-state={job.state}>
                {job.state === "succeeded" ? (
                  <>
                    <CircleCheck size={18} aria-hidden="true" /> ¡Listo! «{job.name}» está creado.
                    {eula ? " Ya puedes encenderlo." : " Acepta la EULA en su página para poder encenderlo."}
                  </>
                ) : job.state === "failed" ? (
                  <>
                    <CircleX size={18} aria-hidden="true" /> No se pudo crear el servidor. No quedó nada a medias en tu PC.
                  </>
                ) : job.state === "cancelled" ? (
                  <>
                    <CircleX size={18} aria-hidden="true" /> Creación cancelada. Se borró lo que se había descargado.
                  </>
                ) : (
                  <>{job.message}…</>
                )}
              </p>
              {job.error && (
                <p className="form-error" role="alert">
                  {job.error}
                </p>
              )}

              <div className="job-log" ref={logRef} aria-label="Registro de la creación" tabIndex={0}>
                {log.map((l) => (
                  <div key={l.seq}>{l.text}</div>
                ))}
              </div>
              {running && <p className="field-help">Puedes cancelar en cualquier momento; se deshace lo que se haya creado.</p>}
            </>
          ) : createError ? null : (
            <p className="wiz-loading" role="status">
              <LoaderCircle size={16} className="spin" aria-hidden="true" /> Empezando…
            </p>
          )}
        </div>
      )}

      {createError && (
        <p className="form-error" role="alert">
          {createError}
        </p>
      )}
    </Dialog>
  );
}
