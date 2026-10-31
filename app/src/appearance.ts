import { ICONS, type IconVariant } from "./appIcon";

// Apariencia: tema, fondo, insignia y celebraciones. Son solo cosméticos: no cambian ninguna
// función. Los extras de Supporter se apagan solos si Supporter no está activo en esta PC.

export type ThemeId = "noche" | "esmeralda" | "nether" | "amatista" | "glaciar";
export type BackgroundId = "particulas" | "sencillo" | "aurora" | "bloques";
/** Claro u oscuro (para todos). «sistema» sigue la opción de Windows. */
export type ModeId = "oscuro" | "claro" | "sistema";

export const MODES: { id: ModeId; name: string }[] = [
  { id: "oscuro", name: "Oscuro" },
  { id: "claro", name: "Claro" },
  { id: "sistema", name: "Igual que Windows" },
];

export interface ThemeDef {
  id: ThemeId;
  name: string;
  description: string;
  supporter: boolean;
  /** Colores para la muestra de la tarjeta. */
  swatch: [string, string, string];
}

export const THEMES: ThemeDef[] = [
  { id: "noche", name: "Noche", description: "El tema de MercHosting.", supporter: false, swatch: ["#080d20", "#4f7dff", "#b9ccff"] },
  { id: "esmeralda", name: "Esmeralda", description: "Verde de aldea y brillo de esmeralda.", supporter: true, swatch: ["#04140f", "#21c47a", "#a8f0c8"] },
  { id: "nether", name: "Nether", description: "Rojo de netherrack y lava.", supporter: true, swatch: ["#170707", "#ff6a3d", "#ffc4a0"] },
  { id: "amatista", name: "Amatista", description: "Morado de geoda.", supporter: true, swatch: ["#100822", "#a66bff", "#dcc6ff"] },
  { id: "glaciar", name: "Glaciar", description: "Hielo compacto y cielo frío.", supporter: true, swatch: ["#04121a", "#38c6e8", "#bdf1ff"] },
];

export interface BackgroundDef {
  id: BackgroundId;
  name: string;
  description: string;
  supporter: boolean;
}

export const BACKGROUNDS: BackgroundDef[] = [
  { id: "particulas", name: "Polvo de bloques", description: "Partículas que suben despacio.", supporter: false },
  { id: "sencillo", name: "Sencillo", description: "Sin animación: lo más ligero.", supporter: false },
  { id: "aurora", name: "Aurora", description: "Luces suaves que cambian muy lento.", supporter: true },
  { id: "bloques", name: "Bloques", description: "Cuadrícula de bloques en el fondo.", supporter: true },
];

export interface AppearancePrefs {
  mode: ModeId;
  theme: ThemeId;
  background: BackgroundId;
  /** Animación al encender un servidor (Supporter). */
  celebrations: boolean;
  /** Insignia «Colaborador» en la barra (Supporter). */
  badge: boolean;
  /** Icono de la ventana (las variantes son de Supporter). */
  icon: IconVariant;
}

export const DEFAULT_PREFS: AppearancePrefs = { mode: "oscuro", theme: "noche", background: "particulas", celebrations: true, badge: true, icon: "oficial" };
const KEY = "merchosting.appearance";

export function loadPrefs(): AppearancePrefs {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? "{}") as Partial<AppearancePrefs>;
    return {
      mode: MODES.some((m) => m.id === raw.mode) ? raw.mode! : DEFAULT_PREFS.mode,
      theme: THEMES.some((t) => t.id === raw.theme) ? raw.theme! : DEFAULT_PREFS.theme,
      background: BACKGROUNDS.some((b) => b.id === raw.background) ? raw.background! : DEFAULT_PREFS.background,
      celebrations: raw.celebrations ?? DEFAULT_PREFS.celebrations,
      badge: raw.badge ?? DEFAULT_PREFS.badge,
      icon: ICONS.some((i) => i.id === raw.icon) ? raw.icon! : DEFAULT_PREFS.icon,
    };
  } catch {
    return { ...DEFAULT_PREFS };
  }
}

export function savePrefs(p: AppearancePrefs) {
  try {
    localStorage.setItem(KEY, JSON.stringify(p));
  } catch {
    /* sin almacenamiento: solo dura esta sesión */
  }
}

/** Lo que de verdad se usa: sin Supporter, los extras vuelven a lo gratuito. */
export function effective(p: AppearancePrefs, supporter: boolean): AppearancePrefs {
  if (supporter) return p;
  return {
    mode: p.mode,
    theme: THEMES.find((t) => t.id === p.theme)?.supporter ? "noche" : p.theme,
    background: BACKGROUNDS.find((b) => b.id === p.background)?.supporter ? "particulas" : p.background,
    celebrations: false,
    badge: false,
    icon: "oficial",
  };
}

export function applyTheme(theme: ThemeId) {
  if (theme === "noche") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = theme;
}

/** ¿Se ve en claro? (con «sistema», según Windows) */
export function isLight(mode: ModeId): boolean {
  if (mode === "claro") return true;
  if (mode === "oscuro") return false;
  return !!window.matchMedia?.("(prefers-color-scheme: light)").matches;
}

export function applyMode(light: boolean) {
  if (light) document.documentElement.dataset.mode = "light";
  else delete document.documentElement.dataset.mode;
}

export function reducedMotion(): boolean {
  return !!window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
}
