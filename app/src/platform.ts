// Sistema en el que corre la app. Lo dice el motor web: WebView2 en Windows («Windows NT»),
// WebKitGTK en Linux («X11; Linux x86_64» o «Wayland») y el WebView de Android («Android»).
// Las pruebas pueden fijarlo con setPlatformForTests.

export type Platform = "windows" | "linux" | "android";

function detect(): Platform {
  const ua = typeof navigator === "undefined" ? "" : navigator.userAgent;
  if (/Android/.test(ua)) return "android";
  return /\bLinux\b/.test(ua) ? "linux" : "windows";
}

let current: Platform = detect();

export function platform(): Platform {
  return current;
}

export function isLinux(): boolean {
  return current === "linux";
}

/** App del celular: sin supervisor propio; administra tus PCs con Compañeros. */
export function isMobile(): boolean {
  return current === "android";
}

/** Solo para pruebas. */
export function setPlatformForTests(p: Platform) {
  current = p;
}

/** Guía oficial de Playit para Linux (para equipos donde MercHosting no trae un agente verificado, p. ej. ARM). */
export const PLAYIT_LINUX_GUIDE = "https://playit.gg/support/run-on-linux";
