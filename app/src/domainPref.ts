// Dominio propio que la persona eligió para cada servidor (solo se guarda en esta PC).

const KEY = "merchosting.customDomain";

function read(): Record<string, string> {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "{}") as Record<string, string>;
  } catch {
    return {};
  }
}

export function customDomain(id: string): string | null {
  const v = read()[id];
  return typeof v === "string" && v !== "" ? v : null;
}

export function setCustomDomain(id: string, domain: string | null) {
  try {
    const all = read();
    if (domain) all[id] = domain;
    else delete all[id];
    localStorage.setItem(KEY, JSON.stringify(all));
  } catch {
    /* sin almacenamiento: solo esta sesión */
  }
}
