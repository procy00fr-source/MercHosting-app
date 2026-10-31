// Permisos de compañeros: nombres, grupos y riesgos (iguales a los del supervisor, que es quien
// decide de verdad cada petición).

import type { Perm } from "./api/types";

export interface PermInfo {
  perm: Perm;
  label: string;
  group: string;
  /** Alto riesgo: confirmación reforzada. */
  high?: boolean;
  /** Da acceso a archivos reales de la PC (muestra la advertencia obligatoria). */
  files?: boolean;
}

export const PERMS: PermInfo[] = [
  { perm: "status", label: "Ver el estado, el rendimiento y la salud", group: "Servidor" },
  { perm: "start", label: "Iniciar", group: "Servidor" },
  { perm: "stop", label: "Detener", group: "Servidor" },
  { perm: "restart", label: "Reiniciar (también con aviso)", group: "Servidor" },
  { perm: "console_read", label: "Leer la consola", group: "Consola" },
  { perm: "console_command", label: "Enviar comandos de Minecraft y programar tareas", group: "Consola", high: true, files: true },
  { perm: "logs", label: "Ver el historial de actividad", group: "Consola" },
  { perm: "players", label: "Jugadores: expulsar, banear y perdonar", group: "Jugadores" },
  { perm: "whitelist", label: "Lista blanca", group: "Jugadores" },
  { perm: "op", label: "Dar y quitar OP", group: "Jugadores", high: true },
  { perm: "files_list", label: "Ver la lista de archivos", group: "Archivos" },
  { perm: "files_read", label: "Leer y descargar archivos", group: "Archivos", files: true },
  { perm: "files_edit", label: "Editar, crear, renombrar, subir y restaurar archivos", group: "Archivos", high: true, files: true },
  { perm: "files_delete", label: "Mandar archivos a la papelera", group: "Archivos", high: true, files: true },
  { perm: "content", label: "Instalar, activar y quitar mods y plugins (archivos JAR)", group: "Contenido", high: true, files: true },
  { perm: "backups", label: "Ver y crear copias de seguridad", group: "Copias" },
  { perm: "backups_restore", label: "Restaurar copias", group: "Copias", high: true },
  { perm: "backups_delete", label: "Borrar y proteger copias", group: "Copias" },
  { perm: "world", label: "Mundos: duplicar, activar y mandar a la papelera", group: "Mundo y ajustes" },
  { perm: "settings", label: "Ajustes del servidor, reglas y mantenimiento", group: "Mundo y ajustes" },
  { perm: "network", label: "Conexión y chat de voz", group: "Mundo y ajustes" },
];

export const PERM_LABEL: Record<Perm, string> = Object.fromEntries(PERMS.map((p) => [p.perm, p.label])) as Record<Perm, string>;

export type RoleName = "observador" | "moderador" | "operador" | "administrador" | "personalizado";

export const ROLE_LABEL: Record<RoleName, string> = {
  observador: "Observador",
  moderador: "Moderador",
  operador: "Operador",
  administrador: "Administrador",
  personalizado: "Personalizado",
};

const OBS: Perm[] = ["status", "console_read"];
const MOD: Perm[] = [...OBS, "logs", "players", "whitelist"];
const OPE: Perm[] = [...MOD, "start", "stop", "restart", "backups", "world", "network"];

export const ROLE_PERMS: Record<Exclude<RoleName, "personalizado">, Perm[]> = {
  observador: OBS,
  moderador: MOD,
  operador: OPE,
  administrador: PERMS.map((p) => p.perm),
};

export const ROLE_HELP: Record<RoleName, string> = {
  observador: "Ve si está encendido y lee la consola.",
  moderador: "Además, cuida a los jugadores y la lista blanca.",
  operador: "Además, inicia, detiene, reinicia, hace copias y gestiona mundos y conexión.",
  administrador: "Todo, incluidos comandos, archivos y mods. Solo para alguien de máxima confianza.",
  personalizado: "Elige cada permiso.",
};

/** Rol que corresponde exactamente a estos permisos (o «personalizado»). */
export function roleOf(perms: Perm[]): RoleName {
  const key = [...perms].sort().join(",");
  for (const r of ["observador", "moderador", "operador", "administrador"] as const) {
    if ([...ROLE_PERMS[r]].sort().join(",") === key) return r;
  }
  return "personalizado";
}

/** Texto literal obligatorio al dar acceso a archivos. */
export const FILES_WARNING =
  "Estás a punto de darle a esta persona acceso a archivos reales almacenados dentro de tu computadora. Aunque MercHosting limitará su acceso a los archivos del servidor autorizado, esta persona podría ver, descargar, modificar o eliminar información según los permisos que concedas. Los mods y plugins JAR pueden ejecutar código en tu PC. Concede estos permisos únicamente a personas de confianza.";
