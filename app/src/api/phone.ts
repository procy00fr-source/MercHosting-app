// Servidores dentro de ESTE celular (fase 2, solo Android arm64). Todo pasa por el comando
// `phone_call` de app/src-tauri/src/phone.rs, que llama a merc_phone::api (probado en Linux).

import { invoke } from "@tauri-apps/api/core";
import { isPermissionGranted, requestPermission } from "@tauri-apps/plugin-notification";
import type { BackupInfo, CatalogEntry, Flavor } from "./types";
import { MercError } from "./types";

/** Tipos que se pueden crear en el celular (no necesitan ejecutar un instalador de Java). */
export const PHONE_FLAVORS = ["paper", "vanilla", "purpur", "fabric"] as const;
export type PhoneFlavor = (typeof PHONE_FLAVORS)[number];

/** Avisos que hay que aceptar uno por uno (mismos identificadores que merc_phone::warnings). */
export const PHONE_WARNINGS = ["bateria", "calor", "cierre", "cargador_wifi", "datos", "rendimiento", "desgaste"] as const;
export type PhoneWarning = (typeof PHONE_WARNINGS)[number];

export interface HeapLimits {
  suggested_mb: number;
  max_mb: number;
}

export interface PhoneInfo {
  arch: string;
  supported: boolean;
  reason: string | null;
  total_ram_mb: number;
  heap: HeapLimits | null;
  jre_installed: number[];
  playit_available: boolean;
  warnings: string[];
}

export interface LocalServer {
  id: string;
  name: string;
  flavor: Flavor;
  mc_version: string;
  java_major: number;
  heap_mb: number;
  jar: string;
  created_unix_ms: number;
  playit: boolean;
}

export type LocalState = "stopped" | "starting" | "online" | "stopping";

export interface LocalStatus {
  state: LocalState;
  pid: number | null;
  eula_accepted: boolean;
  port: number | null;
  problem: string | null;
}

export interface CreateProgress {
  running: boolean;
  step: string;
  done: number;
  total: number | null;
  error: string | null;
}

export type ClaimState = "waiting_for_visit" | "waiting_for_approval" | "approved" | "rejected" | "expired";

export interface PhonePlayit {
  available: boolean;
  running: boolean;
  linked: boolean;
  agent: { phase: string; message: string | null; tunnels: { display_address: string; destination: string }[] } | null;
  claim: { url: string; state: ClaimState; done: boolean; error: string | null } | null;
  address: string | null;
  error: string | null;
}

export interface CreateRequest {
  name: string;
  flavor: PhoneFlavor;
  mc_version: string;
  heap_mb: number;
  accept_eula: boolean;
  warnings_accepted: string[];
}

export type PhoneOp =
  | { op: "info" }
  | { op: "versions"; flavor: PhoneFlavor }
  | { op: "create"; req: CreateRequest }
  | { op: "progress" }
  | { op: "cancel" }
  | { op: "servers" }
  | { op: "status"; id: string }
  | { op: "console"; id: string }
  | { op: "start"; id: string }
  | { op: "stop"; id: string }
  | { op: "force_stop"; id: string }
  | { op: "command"; id: string; text: string }
  | { op: "accept_eula"; id: string }
  | { op: "set_playit"; id: string; on: boolean }
  | { op: "set_heap"; id: string; heap_mb: number }
  | { op: "backup"; id: string }
  | { op: "backups"; id: string }
  | { op: "remove"; id: string }
  | { op: "playit"; id: string | null }
  | { op: "playit_link" }
  | { op: "playit_unlink" };

export interface PhoneApi {
  info(): Promise<PhoneInfo>;
  versions(flavor: PhoneFlavor): Promise<CatalogEntry[]>;
  create(req: CreateRequest): Promise<LocalServer>;
  progress(): Promise<CreateProgress>;
  cancel(): Promise<void>;
  servers(): Promise<LocalServer[]>;
  status(id: string): Promise<LocalStatus>;
  console(id: string): Promise<string[]>;
  start(id: string): Promise<void>;
  stop(id: string): Promise<void>;
  forceStop(id: string): Promise<void>;
  command(id: string, text: string): Promise<void>;
  acceptEula(id: string): Promise<void>;
  setPlayit(id: string, on: boolean): Promise<LocalServer>;
  setHeap(id: string, heapMb: number): Promise<LocalServer>;
  backup(id: string): Promise<BackupInfo>;
  backups(id: string): Promise<BackupInfo[]>;
  remove(id: string): Promise<void>;
  playit(id: string | null): Promise<PhonePlayit>;
  playitLink(): Promise<PhonePlayit["claim"]>;
  playitUnlink(): Promise<void>;
  /** Pide permiso de notificaciones (la del servidor encendido, con su botón Detener). */
  askNotifications(): Promise<boolean>;
}

function toMercError(e: unknown): MercError {
  if (typeof e === "object" && e !== null && "code" in e && "message" in e) {
    const { code, message } = e as { code: string; message: string };
    return new MercError(code as MercError["code"], message);
  }
  return new MercError("internal", String(e));
}

/** Construye la API sobre una función que manda una petición (la real usa `invoke`). */
export function phoneApiFrom(send: (op: PhoneOp) => Promise<unknown>, askNotifications: () => Promise<boolean>): PhoneApi {
  const c = <T>(op: PhoneOp) => send(op) as Promise<T>;
  return {
    info: () => c({ op: "info" }),
    versions: (flavor) => c({ op: "versions", flavor }),
    create: (req) => c({ op: "create", req }),
    progress: () => c({ op: "progress" }),
    cancel: () => c({ op: "cancel" }),
    servers: () => c({ op: "servers" }),
    status: (id) => c({ op: "status", id }),
    console: (id) => c({ op: "console", id }),
    start: (id) => c({ op: "start", id }),
    stop: (id) => c({ op: "stop", id }),
    forceStop: (id) => c({ op: "force_stop", id }),
    command: (id, text) => c({ op: "command", id, text }),
    acceptEula: (id) => c({ op: "accept_eula", id }),
    setPlayit: (id, on) => c({ op: "set_playit", id, on }),
    setHeap: (id, heap_mb) => c({ op: "set_heap", id, heap_mb }),
    backup: (id) => c({ op: "backup", id }),
    backups: (id) => c({ op: "backups", id }),
    remove: (id) => c({ op: "remove", id }),
    playit: (id) => c({ op: "playit", id }),
    playitLink: () => c({ op: "playit_link" }),
    playitUnlink: () => c({ op: "playit_unlink" }),
    askNotifications,
  };
}

export const phoneApi: PhoneApi = phoneApiFrom(
  async (op) => {
    try {
      return await invoke("phone_call", { op });
    } catch (e) {
      throw toMercError(e);
    }
  },
  async () => {
    try {
      return (await isPermissionGranted()) || (await requestPermission()) === "granted";
    } catch {
      return false;
    }
  },
);
