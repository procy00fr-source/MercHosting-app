// Comandos de consola que conviene confirmar antes de enviar, y favoritos por servidor.

export interface Danger {
  /** Qué hace, en palabras simples. */
  why: string;
  /** Alternativa más segura dentro de MercHosting, si la hay. */
  instead?: string;
}

const ALL = /@[ae]\b/;

/** Devuelve por qué un comando es peligroso, o null si no lo es. Acepta con o sin «/». */
export function dangerOf(raw: string): Danger | null {
  const cmd = raw.trim().replace(/^\/+/, "");
  const [head = "", ...rest] = cmd.split(/\s+/);
  const name = head.toLowerCase().replace(/^minecraft:/, "");
  const args = rest.join(" ");
  switch (name) {
    case "stop":
      return { why: "Apaga el servidor ahora para todos los jugadores.", instead: "El botón Detener avisa a los jugadores y guarda antes de apagar." };
    case "op":
      return { why: `Da a ${rest[0] ?? "ese jugador"} poder total: puede usar cualquier comando, banear gente o borrar cosas.` };
    case "deop":
      return { why: `Quita los permisos de operador a ${rest[0] ?? "ese jugador"}.` };
    case "ban":
    case "ban-ip":
      return { why: `Prohíbe la entrada a ${rest[0] ?? "ese jugador"}${name === "ban-ip" ? " y a cualquiera que use su conexión" : ""}.` };
    case "whitelist":
      if (/^off\b/i.test(args)) return { why: "Desactiva la lista blanca: cualquiera que tenga la dirección podrá entrar." };
      return null;
    case "kill":
      if (!args || ALL.test(args)) return { why: !args ? "Mata a quien envía el comando (en la consola no tiene efecto útil)." : "Mata a todas las entidades o jugadores que coincidan: pueden perderse mascotas, aldeanos y objetos." };
      return null;
    case "clear":
      if (ALL.test(args) || !args) return { why: "Vacía el inventario de los jugadores indicados. No se puede deshacer." };
      return null;
    case "kick":
      if (ALL.test(args)) return { why: "Expulsa a todos los jugadores conectados." };
      return null;
    case "save-off":
      return { why: "El servidor deja de guardar el mundo. Si se cierra así, se pierde lo que pase desde ahora." };
    case "reload":
      return { why: "Recarga datapacks (y en servidores de plugins, los plugins): puede romper plugins o dejar el servidor en mal estado.", instead: "Es más seguro reiniciar el servidor." };
    case "fill":
    case "clone":
      return { why: "Cambia muchos bloques de golpe. Si te equivocas de coordenadas, no hay deshacer.", instead: "Haz una copia del mundo antes." };
    case "forceload":
      if (/^add\b/i.test(args)) return { why: "Mantiene trozos del mundo cargados siempre: usa más memoria y CPU." };
      return null;
    default:
      return null;
  }
}

const KEY = "merchosting.consoleFavs";

function readAll(): Record<string, string[]> {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "{}") as Record<string, string[]>;
  } catch {
    return {};
  }
}

export function loadFavorites(id: string): string[] {
  const v = readAll()[id];
  return Array.isArray(v) ? v.filter((x) => typeof x === "string") : [];
}

export function saveFavorites(id: string, favs: string[]) {
  try {
    const all = readAll();
    if (favs.length) all[id] = favs.slice(0, 30);
    else delete all[id];
    localStorage.setItem(KEY, JSON.stringify(all));
  } catch {
    /* sin almacenamiento: solo esta sesión */
  }
}
