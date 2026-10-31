// Modo principiante / avanzado. Es la misma app y la misma lógica: el modo principiante solo
// oculta secciones avanzadas de la barra (se dice dónde activarlas).

import { createContext, useContext } from "react";

export type UiLevel = "principiante" | "avanzado";

const KEY = "merchosting.level";
const WELCOME_KEY = "merchosting.welcomeSeen";

/** Pestañas de cada servidor que solo se ven en modo avanzado. */
export const ADVANCED_TABS = new Set(["files", "tasks", "health", "activity", "security", "worlds", "datapack", "luckperms"]);

export function loadLevel(): UiLevel {
  try {
    return localStorage.getItem(KEY) === "principiante" ? "principiante" : "avanzado";
  } catch {
    return "avanzado";
  }
}

export function saveLevel(l: UiLevel) {
  try {
    localStorage.setItem(KEY, l);
  } catch {
    /* sin almacenamiento: solo esta sesión */
  }
}

export function welcomeSeen(): boolean {
  try {
    return localStorage.getItem(WELCOME_KEY) === "1";
  } catch {
    return true;
  }
}

export function markWelcomeSeen() {
  try {
    localStorage.setItem(WELCOME_KEY, "1");
  } catch {
    /* nada */
  }
}

export const UiLevelContext = createContext<UiLevel>("avanzado");
export const useUiLevel = () => useContext(UiLevelContext);
