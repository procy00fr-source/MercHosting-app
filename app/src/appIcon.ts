// Iconos alternativos de la aplicación (Supporter): el mismo logo de MercHosting con otros
// colores. Cambian el icono de la ventana y de la barra de tareas mientras la app está abierta;
// el acceso directo y el instalador mantienen el icono oficial.

import { LOGO_PIECES, LOGO_TRANSFORM, LOGO_VIEWBOX } from "./components/logoPaths";

export type IconVariant = "oficial" | "esmeralda" | "nether" | "oro" | "noche";

export interface IconDef {
  id: IconVariant;
  name: string;
  supporter: boolean;
  /** Degradado de las caras (arriba → abajo) y color del núcleo. */
  body: [string, string];
  core: string;
}

export const ICONS: IconDef[] = [
  { id: "oficial", name: "Oficial", supporter: false, body: ["#ffffff", "#c8d2f0"], core: "#6c9bff" },
  { id: "esmeralda", name: "Esmeralda", supporter: true, body: ["#f2fff8", "#9fe6c0"], core: "#17b26a" },
  { id: "nether", name: "Nether", supporter: true, body: ["#fff1ea", "#ffad8a"], core: "#e8431c" },
  { id: "oro", name: "Oro", supporter: true, body: ["#fffbe8", "#f2cf63"], core: "#b37a00" },
  { id: "noche", name: "Noche", supporter: true, body: ["#3a4a86", "#141c3d"], core: "#8fb0ff" },
];

/** SVG completo del logo con los colores de la variante. */
export function iconSvg(id: IconVariant): string {
  const d = ICONS.find((i) => i.id === id) ?? ICONS[0];
  const paths = (Object.keys(LOGO_PIECES) as (keyof typeof LOGO_PIECES)[])
    .map((k) => `<path d="${LOGO_PIECES[k]}" fill="${k === "core" ? d.core : "url(#g)"}"/>`)
    .join("");
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${LOGO_VIEWBOX}" width="256" height="256">` +
    `<defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${d.body[0]}"/>` +
    `<stop offset="1" stop-color="${d.body[1]}"/></linearGradient></defs>` +
    `<g transform="${LOGO_TRANSFORM}">${paths}</g></svg>`
  );
}

export function iconDataUrl(id: IconVariant): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(iconSvg(id))}`;
}
