// Interruptor del separador de mods cliente/servidor, por servidor. Apagado por defecto:
// mientras está apagado no se revisa ningún mod ni se muestran sugerencias.

const KEY = "merchosting.modSides";

function read(): Record<string, boolean> {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "{}") as Record<string, boolean>;
  } catch {
    return {};
  }
}

export function modSidesEnabled(id: string): boolean {
  return read()[id] === true;
}

export function setModSidesEnabled(id: string, on: boolean) {
  try {
    const all = read();
    if (on) all[id] = true;
    else delete all[id];
    localStorage.setItem(KEY, JSON.stringify(all));
  } catch {
    /* sin almacenamiento: solo esta sesión */
  }
}
