// MOTD de Minecraft: códigos de formato «§», cómo los dibuja la lista de servidores y
// herramientas del editor. En el editor se escriben con «&» (más fácil de teclear) y se guardan
// con «§», que es lo que lee el servidor.

export interface McColor {
  code: string;
  name: string;
  hex: string;
}

/** Los 16 colores oficiales de Minecraft. */
export const COLORS: McColor[] = [
  { code: "0", name: "Negro", hex: "#000000" },
  { code: "1", name: "Azul oscuro", hex: "#0000AA" },
  { code: "2", name: "Verde oscuro", hex: "#00AA00" },
  { code: "3", name: "Turquesa oscuro", hex: "#00AAAA" },
  { code: "4", name: "Rojo oscuro", hex: "#AA0000" },
  { code: "5", name: "Morado", hex: "#AA00AA" },
  { code: "6", name: "Dorado", hex: "#FFAA00" },
  { code: "7", name: "Gris", hex: "#AAAAAA" },
  { code: "8", name: "Gris oscuro", hex: "#555555" },
  { code: "9", name: "Azul", hex: "#5555FF" },
  { code: "a", name: "Verde", hex: "#55FF55" },
  { code: "b", name: "Celeste", hex: "#55FFFF" },
  { code: "c", name: "Rojo", hex: "#FF5555" },
  { code: "d", name: "Rosa", hex: "#FF55FF" },
  { code: "e", name: "Amarillo", hex: "#FFFF55" },
  { code: "f", name: "Blanco", hex: "#FFFFFF" },
];

export type FormatKey = "bold" | "italic" | "underline" | "strike" | "obfuscated";

export const FORMATS: { code: string; key: FormatKey; name: string }[] = [
  { code: "l", key: "bold", name: "Negrita" },
  { code: "o", key: "italic", name: "Cursiva" },
  { code: "n", key: "underline", name: "Subrayado" },
  { code: "m", key: "strike", name: "Tachado" },
  { code: "k", key: "obfuscated", name: "Ofuscado (letras que cambian)" },
];

/** Símbolos que el juego dibuja bien en la lista de servidores. */
export const SYMBOLS = ["★", "✦", "❤", "⚔", "➤", "»", "«", "┃", "✔", "✖", "☀", "☾", "♫", "⛏"];

/** Color por defecto del MOTD en la lista de servidores (gris del juego). */
export const DEFAULT_HEX = "#808080";
/** Ancho del texto en la lista de servidores, en píxeles del juego. Lo que no cabe baja de línea. */
export const LINE_WIDTH = 271;

export interface Style {
  color: string | null;
  bold: boolean;
  italic: boolean;
  underline: boolean;
  strike: boolean;
  obfuscated: boolean;
}

export interface Segment extends Style {
  text: string;
}

const PLAIN: Style = { color: null, bold: false, italic: false, underline: false, strike: false, obfuscated: false };
const CODE = /[0-9a-fk-or]/i;

/** «§» → «&» para mostrarlo en el editor. */
export function toEditor(motd: string): string {
  return motd.replace(/§/g, "&");
}

/** «&» seguido de un código válido → «§» para guardarlo. Un «&» suelto queda como está. */
export function fromEditor(text: string): string {
  return text.replace(/&([0-9a-fk-or])/gi, (_, c: string) => `§${c.toLowerCase()}`);
}

function applyCode(st: Style, c: string): Style {
  c = c.toLowerCase();
  if (/[0-9a-f]/.test(c)) return { ...PLAIN, color: c }; // un color quita los formatos, como en el juego
  if (c === "r") return { ...PLAIN };
  const f = FORMATS.find((x) => x.code === c);
  return f ? { ...st, [f.key]: true } : st;
}

/** Trocea una línea (con «§» o «&») en partes con su estilo. */
export function parse(line: string): Segment[] {
  const out: Segment[] = [];
  let st: Style = { ...PLAIN };
  let buf = "";
  const chars = [...line];
  for (let i = 0; i < chars.length; i++) {
    const ch = chars[i];
    if ((ch === "§" || ch === "&") && i + 1 < chars.length && CODE.test(chars[i + 1])) {
      if (buf) out.push({ ...st, text: buf });
      buf = "";
      st = applyCode(st, chars[i + 1]);
      i++;
      continue;
    }
    buf += ch;
  }
  if (buf) out.push({ ...st, text: buf });
  return out;
}

/** Estilo activo en una posición del texto del editor. */
export function styleAt(text: string, pos: number): Style {
  let st: Style = { ...PLAIN };
  const re = /&([0-9a-fk-or])/gi;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) && m.index + 2 <= pos) st = applyCode(st, m[1]);
  return st;
}

function formatCodes(st: Style): string {
  return FORMATS.filter((f) => st[f.key])
    .map((f) => `&${f.code}`)
    .join("");
}

/** Códigos que reproducen un estilo. */
export function codesFor(st: Style): string {
  return (st.color ? `&${st.color}` : "") + formatCodes(st);
}

/**
 * Aplica un código donde está el cursor. Con texto seleccionado lo aplica solo a la selección
 * y después vuelve al estilo que había.
 */
export function applyToSelection(text: string, start: number, end: number, code: string): { text: string; cursor: number } {
  const c = code.toLowerCase();
  const before = styleAt(text, start);
  // Un color quita la negrita y demás: se repiten para no perderlas.
  const insert = /[0-9a-f]/.test(c) ? `&${c}${formatCodes(before)}` : `&${c}`;
  if (start === end) {
    const t = text.slice(0, start) + insert + text.slice(start);
    return { text: t, cursor: start + insert.length };
  }
  const restore = `&r${codesFor(styleAt(text, end))}`;
  const t = text.slice(0, start) + insert + text.slice(start, end) + restore + text.slice(end);
  return { text: t, cursor: end + insert.length + restore.length };
}

// Ancho de cada letra en la fuente del juego (con su espacio). El resto mide 6.
const NARROW: Record<string, number> = {
  i: 2, "!": 2, ".": 2, ",": 2, ":": 2, ";": 2, "|": 2, "'": 2,
  l: 3, "`": 3,
  I: 4, t: 4, "[": 4, "]": 4, " ": 4,
  f: 5, k: 5, "<": 5, ">": 5, "(": 5, ")": 5, "{": 5, "}": 5, "*": 5, '"': 5,
  "@": 7, "~": 7,
};

export function charWidth(ch: string, bold = false): number {
  const base = NARROW[ch] ?? (ch.charCodeAt(0) > 0x7f && !/\p{L}/u.test(ch) ? 9 : 6);
  return base + (bold ? 1 : 0);
}

export function textWidth(segs: Segment[]): number {
  return segs.reduce((w, s) => w + [...s.text].reduce((a, ch) => a + charWidth(ch, s.bold), 0), 0);
}

/**
 * Cómo lo dibuja la lista de servidores: corta en líneas de {@link LINE_WIDTH} píxeles (por
 * espacios cuando puede) y muestra solo las dos primeras.
 */
export function layout(motd: string): { lines: Segment[][]; hidden: boolean } {
  const all: Segment[][] = [];
  for (const raw of motd.split("\n")) {
    const words: { seg: Style; text: string }[] = [];
    for (const s of parse(raw)) for (const part of s.text.split(/(?<= )/)) if (part) words.push({ seg: s, text: part });
    let line: Segment[] = [];
    let w = 0;
    const push = (st: Style, text: string) => {
      const last = line[line.length - 1];
      if (last && sameStyle(last, st)) last.text += text;
      else line.push({ ...st, text });
    };
    for (const { seg, text } of words) {
      const ww = [...text].reduce((a, ch) => a + charWidth(ch, seg.bold), 0);
      if (w + ww > LINE_WIDTH && w > 0) {
        all.push(line);
        line = [];
        w = 0;
        if (text === " ") continue;
      }
      if (ww > LINE_WIDTH) {
        // Una palabra más larga que la línea se parte por letras.
        for (const ch of text) {
          const cw = charWidth(ch, seg.bold);
          if (w + cw > LINE_WIDTH) {
            all.push(line);
            line = [];
            w = 0;
          }
          push(seg, ch);
          w += cw;
        }
        continue;
      }
      push(seg, text);
      w += ww;
    }
    all.push(line);
  }
  return { lines: all.slice(0, 2), hidden: all.length > 2 };
}

function sameStyle(a: Style, b: Style) {
  return a.color === b.color && a.bold === b.bold && a.italic === b.italic && a.underline === b.underline && a.strike === b.strike && a.obfuscated === b.obfuscated;
}

/** Centra una línea del editor con espacios al principio (aproximado: el juego no centra). */
export function centerLine(text: string): string {
  const trimmed = text.replace(/^((?:&[0-9a-fk-or])*) +/i, "$1");
  const w = textWidth(parse(trimmed));
  const spaces = Math.max(0, Math.floor((LINE_WIDTH - w) / 2 / charWidth(" ")));
  return " ".repeat(spaces) + trimmed;
}

export function hexFor(st: Style): string {
  return COLORS.find((c) => c.code === st.color)?.hex ?? DEFAULT_HEX;
}
