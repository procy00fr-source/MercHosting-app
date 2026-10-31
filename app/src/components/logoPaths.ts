// Logo de MercHosting (diseño del autor), redibujado con geometría exacta sobre una rejilla
// isométrica: cubo de tres caras separadas por un hueco constante. Seis piezas para poder
// animarlas por separado. El rombo central ("core") se usa como indicador de estado del servidor.
// Generado con scripts/logo/generar-logo.py (no editar a mano).

export const LOGO_VIEWBOX = "-90.6 -104 181.2 208";
/** Sin transformación: las coordenadas ya están en el espacio del viewBox. */
export const LOGO_TRANSFORM = "translate(0,0)";

export const LOGO_PIECES = {
  top: "M-29.01 -20.50 L-83.35 -51.88 L0.00 -100.00 L83.35 -51.88 L29.01 -20.50 L8.23 -32.50 L41.79 -51.88 L0.00 -76.00 L-41.79 -51.88 L-8.23 -32.50 Z",
  outerLeft: "M-86.60 -46.25 L-86.60 50.00 L-3.25 98.12 L-3.25 74.12 L-65.82 38.00 L-65.82 -34.25 Z",
  outerRight: "M86.60 -46.25 L86.60 50.00 L3.25 98.12 L3.25 74.12 L65.82 38.00 L65.82 -34.25 Z",
  core: "M0.00 -3.75 L-22.52 -16.75 L0.00 -29.75 L22.52 -16.75 Z",
  innerRight: "M3.25 1.88 L59.32 -30.50 L59.32 -6.50 L24.03 13.87 L24.03 54.62 L3.25 66.62 Z",
  innerLeft: "M-3.25 1.88 L-59.32 -30.50 L-59.32 -6.50 L-24.03 13.87 L-24.03 54.62 L-3.25 66.62 Z",
} as const;

export type LogoPiece = keyof typeof LOGO_PIECES;
