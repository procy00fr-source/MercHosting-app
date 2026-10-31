// Genera app/src/light.css (modo claro) a partir de app/src/styles.css.
//
// El estilo de MercHosting se diseñó en oscuro y tiene colores escritos a mano. En vez de
// mantener dos hojas a mano, este script recorre cada regla y, para cada color de fondo, borde,
// texto o sombra, escribe su equivalente claro bajo :root[data-mode="light"].
// Ejecutar después de cambiar styles.css:   node scripts/generar-modo-claro.mjs

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const SRC = join(here, "..", "app", "src", "styles.css");
const OUT = join(here, "..", "app", "src", "light.css");
const SCOPE = ':root[data-mode="light"]';

/** Zonas que se quedan oscuras a propósito (la consola y la vista previa de Minecraft). */
const KEEP_DARK = /\.console|\.mc-|\.celebration|\.logo\b|\.splash|\.theme-swatch|\.motd-swatch/;
/** Texto sobre fondos de color (botones, insignias): se queda como está. */
const KEEP_TEXT = /\.btn-(go|stop|import|danger)|badge|-count|\.toast-action|\.seg\[aria-checked/;
const PROPS = /^(color|background(-color|-image)?|border(-(top|right|bottom|left))?(-color)?|outline(-color)?|box-shadow|text-shadow|fill|stroke|caret-color|text-decoration-color)$/;

// ---------- colores ----------
const COLOR_RE = /rgba?\([^)]*\)|#[0-9a-fA-F]{3,8}\b/g;

function parse(c) {
  if (c.startsWith("#")) {
    let h = c.slice(1);
    if (h.length === 3 || h.length === 4) h = [...h].map((x) => x + x).join("");
    const n = (i) => parseInt(h.slice(i, i + 2), 16);
    return { r: n(0), g: n(2), b: n(4), a: h.length === 8 ? n(6) / 255 : 1 };
  }
  const p = c.slice(c.indexOf("(") + 1, -1).split(/[\s,/]+/).filter(Boolean).map(Number);
  return { r: p[0], g: p[1], b: p[2], a: p.length > 3 ? p[3] : 1 };
}

const fmt = ({ r, g, b, a }) => {
  const R = (v) => Math.round(Math.max(0, Math.min(255, v)));
  const A = Math.round(Math.max(0, Math.min(1, a)) * 1000) / 1000;
  return A >= 1 ? `rgb(${R(r)}, ${R(g)}, ${R(b)})` : `rgba(${R(r)}, ${R(g)}, ${R(b)}, ${A})`;
};

function lum({ r, g, b }) {
  return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
}
function sat({ r, g, b }) {
  const mx = Math.max(r, g, b);
  const mn = Math.min(r, g, b);
  return mx === 0 ? 0 : (mx - mn) / mx;
}
const darken = (c, k) => ({ r: c.r * (1 - k), g: c.g * (1 - k), b: c.b * (1 - k), a: c.a });

/** Color equivalente en modo claro según para qué se usa. */
function mapColor(c, prop) {
  const L = lum(c);
  const S = sat(c);
  if (prop === "color" || prop === "caret-color" || prop === "fill" || prop === "stroke" || prop === "text-decoration-color") {
    if (L > 0.55 && S < 0.45) return { r: 22, g: 28, b: 52, a: c.a }; // texto claro → tinta oscura
    if (L > 0.45) return darken(c, 0.42); // acentos brillantes → más oscuros para leerse sobre blanco
    return c;
  }
  if (prop === "box-shadow" || prop === "text-shadow") {
    if (L < 0.2) return { r: 30, g: 40, b: 80, a: c.a * 0.3 };
    return { ...c, a: c.a * 0.6 };
  }
  // Fondos y bordes.
  if (L < 0.14 && S < 0.85) {
    // Superficies oscuras (paneles, campos) → blancas.
    return c.a >= 0.99 ? { r: 246, g: 248, b: 253, a: 1 } : { r: 255, g: 255, b: 255, a: Math.min(0.96, c.a + 0.3) };
  }
  if (c.r > 240 && c.g > 240 && c.b > 240 && c.a < 0.3) return { r: 20, g: 30, b: 70, a: Math.min(0.2, c.a * 1.6) };
  if (L > 0.55 && S < 0.5 && c.a < 0.7) return { r: 30, g: 50, b: 130, a: Math.min(0.45, c.a * 1.25) }; // velos azulados
  return c;
}

function mapValue(value, prop) {
  return value.replace(COLOR_RE, (m) => fmt(mapColor(parse(m), prop)));
}

// ---------- CSS ----------
function stripComments(s) {
  return s.replace(/\/\*[\s\S]*?\*\//g, "");
}

/** Recorre bloques: devuelve [{ selector, body, at }] (at = cabecera @media que lo envuelve). */
function blocks(css, at = null, out = []) {
  let i = 0;
  while (i < css.length) {
    const open = css.indexOf("{", i);
    if (open < 0) break;
    const head = css.slice(i, open).trim();
    let depth = 1;
    let j = open + 1;
    while (j < css.length && depth > 0) {
      if (css[j] === "{") depth++;
      else if (css[j] === "}") depth--;
      j++;
    }
    const body = css.slice(open + 1, j - 1);
    if (head.startsWith("@media") || head.startsWith("@supports")) blocks(body, head, out);
    else if (!head.startsWith("@")) out.push({ selector: head, body, at });
    i = j;
  }
  return out;
}

function decls(body) {
  const out = [];
  let buf = "";
  let paren = 0;
  for (const ch of body) {
    if (ch === "(") paren++;
    if (ch === ")") paren--;
    if (ch === ";" && paren === 0) {
      out.push(buf);
      buf = "";
    } else buf += ch;
  }
  if (buf.trim()) out.push(buf);
  return out
    .map((d) => {
      const k = d.indexOf(":");
      return k < 0 ? null : { prop: d.slice(0, k).trim(), value: d.slice(k + 1).trim() };
    })
    .filter(Boolean);
}

function scoped(selector) {
  return selector
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s && !s.startsWith(":root"))
    .map((s) => (s === "html" || s === "body" ? `${SCOPE} ${s}` : `${SCOPE} ${s}`))
    .join(",\n");
}

const css = stripComments(readFileSync(SRC, "utf8"));
const rules = [];
for (const { selector, body, at } of blocks(css)) {
  if (KEEP_DARK.test(selector)) continue;
  const sel = scoped(selector);
  if (!sel) continue;
  const out = [];
  for (const { prop, value } of decls(body)) {
    if (!PROPS.test(prop) || !COLOR_RE.test(value)) continue;
    COLOR_RE.lastIndex = 0;
    if (prop === "color" && KEEP_TEXT.test(selector)) continue;
    const mapped = mapValue(value, prop.startsWith("background") ? "background" : prop.startsWith("border") || prop.startsWith("outline") ? "border" : prop);
    if (mapped !== value) out.push(`  ${prop}: ${mapped};`);
  }
  if (out.length) rules.push({ at, text: `${sel} {\n${out.join("\n")}\n}` });
}

const VARS = `${SCOPE} {
  color-scheme: light;
  --night-950: #eef1f8;
  --night-900: #f5f7fc;
  --night-850: #ffffff;
  --night-800: #e8ecf6;
  --glass: rgba(255, 255, 255, 0.72);
  --glass-strong: rgba(255, 255, 255, 0.94);
  --line: rgba(20, 35, 90, 0.12);
  --line-strong: rgba(20, 35, 90, 0.22);
  --ivory: #141a30;
  --ivory-2: #39425f;
  --muted: #5d6886;
  --copper-2: color-mix(in srgb, var(--copper) 78%, #000);
  --champagne: color-mix(in srgb, var(--copper) 62%, #000);
  --ok: #0a8fb5;
  --busy: #b7770f;
  --bad: #c9432e;
  --idle: #7a84a3;
  --unsure: #6c6390;
}
${SCOPE} .backdrop {
  --glow-1: color-mix(in srgb, var(--copper) 16%, transparent);
  --glow-2: color-mix(in srgb, var(--copper) 10%, transparent);
  --glow-3: color-mix(in srgb, var(--copper) 6%, transparent);
}
${SCOPE} .sidebar {
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.9), rgba(246, 248, 253, 0.9));
  box-shadow: 0 20px 60px rgba(30, 40, 80, 0.12);
}
/* La consola se queda oscura, como una terminal: recupera los colores del modo oscuro. */
${SCOPE} .console {
  color-scheme: dark;
  --ivory: #eef2ff;
  --ivory-2: #c3cbe6;
  --muted: #8a95b8;
  --line: rgba(150, 175, 255, 0.12);
  --line-strong: rgba(150, 175, 255, 0.22);
  --glass: rgba(20, 30, 64, 0.55);
  --glass-strong: rgba(16, 24, 52, 0.85);
  --copper-2: #8fb0ff;
  --champagne: #b9ccff;
  --ok: #3fd2ff;
  --busy: #f1b85b;
  --bad: #f0715d;
  --idle: #5f6b93;
  background: #0c132b;
  color: var(--ivory);
}`;

let text = `/* Generado por scripts/generar-modo-claro.mjs a partir de styles.css. No editar a mano. */\n\n${VARS}\n`;
const byAt = new Map();
for (const r of rules) {
  if (!byAt.has(r.at)) byAt.set(r.at, []);
  byAt.get(r.at).push(r.text);
}
for (const [at, list] of byAt) {
  text += "\n" + (at ? `${at} {\n${list.join("\n\n")}\n}\n` : list.join("\n\n") + "\n");
}
// Los ajustes de la barra van al final para ganar a las reglas generadas.
text += `\n${SCOPE} .sidebar {\n  background: linear-gradient(180deg, rgba(255, 255, 255, 0.9), rgba(246, 248, 253, 0.9));\n}\n`;
if (process.argv.includes("--check")) {
  // Para las pruebas: falla si light.css no corresponde a styles.css.
  if (readFileSync(OUT, "utf8") !== text) {
    console.error("light.css está desactualizado: ejecuta node scripts/generar-modo-claro.mjs");
    process.exit(1);
  }
  console.log("light.css al día");
} else {
  writeFileSync(OUT, text);
  console.log(`light.css: ${rules.length} reglas`);
}
