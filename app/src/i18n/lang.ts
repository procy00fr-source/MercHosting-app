// Idioma de la interfaz. Español por defecto; inglés si la persona lo elige o si Windows no
// está en español y no eligió nada.

export type Lang = "es" | "en";
const KEY = "merchosting.lang";

export function savedLang(): Lang | null {
  try {
    const v = localStorage.getItem(KEY);
    return v === "es" || v === "en" ? v : null;
  } catch {
    return null;
  }
}

export function systemLang(): Lang {
  const l = (typeof navigator !== "undefined" && navigator.language) || "es";
  return l.toLowerCase().startsWith("es") ? "es" : "en";
}

let current: Lang | null = null;

export function uiLang(): Lang {
  if (!current) current = savedLang() ?? systemLang();
  return current;
}

/** Para fechas y números (`toLocaleString`, `Intl.NumberFormat`). */
export function uiLocale(): string {
  return uiLang() === "en" ? "en-US" : "es";
}

/** Guarda la elección. El cambio se ve al recargar la interfaz. */
export function setLang(l: Lang) {
  try {
    localStorage.setItem(KEY, l);
  } catch {
    /* sin almacenamiento: solo esta sesión */
  }
  current = l;
}
