import type { RuntimeState, ServerKind } from "./api/types";
import { uiLang } from "./i18n/lang";

export const STATE_LABEL: Record<RuntimeState, string> = {
  STOPPED: "Detenido",
  VALIDATING: "Validando",
  STARTING: "Iniciando",
  LOADING: "Cargando",
  ONLINE: "En línea",
  STOPPING: "Deteniendo",
  CRASHED: "Se cerró inesperadamente",
  ERROR: "Error",
  RECOVERING: "Recuperando",
  UNKNOWN: "Sin confirmar",
};

export type StateTone = "ok" | "busy" | "bad" | "idle" | "unsure";

export const STATE_TONE: Record<RuntimeState, StateTone> = {
  STOPPED: "idle",
  VALIDATING: "busy",
  STARTING: "busy",
  LOADING: "busy",
  ONLINE: "ok",
  STOPPING: "busy",
  CRASHED: "bad",
  ERROR: "bad",
  RECOVERING: "unsure",
  UNKNOWN: "unsure",
};

export const KIND_LABEL: Record<ServerKind, string> = {
  vanilla: "Vanilla",
  paper: "Paper",
  purpur: "Purpur",
  spigot: "Spigot",
  fabric: "Fabric",
  forge: "Forge",
  neo_forge: "NeoForge",
  quilt: "Quilt",
  mohist: "Mohist",
  arclight: "Arclight",
  unknown: "Desconocido",
};

/** Mismos estados que RuntimeState::can_start en Rust. */
export function canStart(s: RuntimeState): boolean {
  return s === "STOPPED" || s === "CRASHED" || s === "ERROR";
}

/** Mismos estados que RuntimeState::has_process en Rust. */
export function hasProcess(s: RuntimeState): boolean {
  return ["STARTING", "LOADING", "ONLINE", "STOPPING", "UNKNOWN", "RECOVERING"].includes(s);
}

export function formatMemory(mb: number): string {
  return mb >= 1024 ? `${(mb / 1024).toFixed(mb % 1024 === 0 ? 0 : 1)} GB` : `${mb} MB`;
}

export function formatDuration(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  if (h > 0) return `${h} h ${m} min`;
  if (m > 0) return `${m} min ${sec} s`;
  return `${sec} s`;
}

export function folderName(path: string): string {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  return parts[parts.length - 1] ?? "Servidor";
}

export function formatBytes(bytes: number): string {
  const gb = bytes / 1024 ** 3;
  if (gb >= 1) return `${uiLang() === "en" ? gb.toFixed(gb >= 10 ? 0 : 1) : gb.toFixed(gb >= 10 ? 0 : 1).replace(".", ",")} GB`;
  return `${Math.round(bytes / 1024 ** 2)} MB`;
}

export function formatPercent(p: number): string {
  return `${p < 10 ? (uiLang() === "en" ? p.toFixed(1) : p.toFixed(1).replace(".", ",")) : Math.round(p)} %`;
}

const WINDOWS_RESERVED = /^(con|prn|aux|nul|com[1-9]|lpt[1-9])$/i;

/** Nombre de carpeta válido en Windows a partir del nombre que escribe el usuario. */
export function safeFolderName(name: string): string {
  // eslint-disable-next-line no-control-regex
  let n = name.replace(/[<>:"/\\|?*\u0000-\u001f]/g, "-").trim().replace(/[. ]+$/, "");
  if (WINDOWS_RESERVED.test(n)) n = `${n}-servidor`;
  return n.slice(0, 64) || "Servidor";
}

/** Une una carpeta y un nombre usando el separador que ya trae la carpeta. */
export function joinPath(base: string, name: string): string {
  const sep = base.includes("\\") ? "\\" : "/";
  return base.replace(/[\\/]+$/, "") + sep + name;
}


/** Tamaño de archivo legible (B, KB, MB, GB). */
export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${Math.round(bytes / 1024)} KB`;
  const dec = (n: number) => (uiLang() === "en" ? n.toFixed(1) : n.toFixed(1).replace(".", ","));
  if (bytes < 1024 ** 3) return `${dec(bytes / 1024 ** 2)} MB`;
  return `${dec(bytes / 1024 ** 3)} GB`;
}
