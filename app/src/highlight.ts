// Colores para el editor de Archivos. Ligero y sin dependencias: reconoce lo justo de
// server.properties / .cfg / .ini / .toml, JSON (.json, .mcmeta) y YAML (.yml, .yaml).
// Solo da clases a trozos de texto; el texto en sí no cambia nunca.

export type Tok = { t: string; c?: "k" | "s" | "n" | "b" | "c" | "h" | "p" };
export type Lang = "props" | "toml" | "json" | "yaml" | null;

/** Más allá de esto no se colorea (para que el editor siga fluido). */
export const MAX_HIGHLIGHT_CHARS = 300_000;

export function langOf(path: string): Lang {
  const p = path.toLowerCase();
  if (/\.(properties|cfg|conf|ini)$/.test(p)) return "props";
  if (/\.toml$/.test(p)) return "toml";
  if (/\.(json|mcmeta|json5)$/.test(p)) return "json";
  if (/\.ya?ml$/.test(p)) return "yaml";
  return null;
}

const VALUE = /("(?:[^"\\]|\\.)*"?|'[^']*'?)|(\b(?:true|false|null|yes|no|on|off)\b)|(-?\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b)/gi;

/** Trocea un valor en cadenas, números y booleanos. */
function value(text: string, out: Tok[]) {
  let last = 0;
  for (const m of text.matchAll(VALUE)) {
    const i = m.index ?? 0;
    if (i > last) out.push({ t: text.slice(last, i) });
    out.push({ t: m[0], c: m[1] ? "s" : m[2] ? "b" : "n" });
    last = i + m[0].length;
  }
  if (last < text.length) out.push({ t: text.slice(last) });
}

function line(lang: Exclude<Lang, null>, l: string, out: Tok[]) {
  if (lang === "json") {
    const re = /("(?:[^"\\]|\\.)*"?)(\s*:)?|(\b(?:true|false|null)\b)|(-?\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b)|([{}[\],])/g;
    let last = 0;
    for (const m of l.matchAll(re)) {
      const i = m.index ?? 0;
      if (i > last) out.push({ t: l.slice(last, i) });
      if (m[1]) {
        out.push({ t: m[1], c: m[2] ? "k" : "s" });
        if (m[2]) out.push({ t: m[2], c: "p" });
      } else out.push({ t: m[0], c: m[3] ? "b" : m[4] ? "n" : "p" });
      last = i + m[0].length;
    }
    if (last < l.length) out.push({ t: l.slice(last) });
    return;
  }
  const trimmed = l.trimStart();
  const lead = l.slice(0, l.length - trimmed.length);
  if (lead) out.push({ t: lead });
  if (!trimmed) return;
  if (/^[#;!]/.test(trimmed) || (lang !== "props" && trimmed.startsWith("#"))) {
    out.push({ t: trimmed, c: "c" });
    return;
  }
  if ((lang === "toml" || lang === "props") && /^\[.*\]\s*$/.test(trimmed)) {
    out.push({ t: trimmed, c: "h" });
    return;
  }
  const sep = lang === "yaml" ? /^(-\s+)?([^:#"']+?|"[^"]*"|'[^']*')(\s*:)(\s|$)/ : /^([^=:#]+?)(\s*[=:])/;
  const m = sep.exec(trimmed);
  if (m) {
    let rest = trimmed;
    if (lang === "yaml") {
      if (m[1]) out.push({ t: m[1], c: "p" });
      out.push({ t: m[2], c: "k" }, { t: m[3], c: "p" });
      rest = trimmed.slice((m[1]?.length ?? 0) + m[2].length + m[3].length);
    } else {
      out.push({ t: m[1], c: "k" }, { t: m[2], c: "p" });
      rest = trimmed.slice(m[0].length);
    }
    // Comentario al final (TOML/YAML): « # algo».
    const cm = lang === "props" ? null : /\s#.*$/.exec(rest.replace(/"(?:[^"\\]|\\.)*"|'[^']*'/g, (s) => " ".repeat(s.length)));
    if (cm) {
      if (lang === "props") value(rest, out);
      else {
        value(rest.slice(0, cm.index), out);
        out.push({ t: rest.slice(cm.index), c: "c" });
      }
    } else if (lang === "props") out.push({ t: rest, c: rest.trim() && /^(true|false)$/i.test(rest.trim()) ? "b" : /^-?\d+(\.\d+)?$/.test(rest.trim()) ? "n" : "s" });
    else value(rest, out);
    return;
  }
  if (lang === "yaml" && trimmed.startsWith("- ")) {
    out.push({ t: "- ", c: "p" });
    value(trimmed.slice(2), out);
    return;
  }
  value(trimmed, out);
}

/** Tokens de todo el texto; `\n` se conserva como token sin clase. Unir todos los `t` da el texto original. */
export function highlight(text: string, lang: Lang): Tok[] {
  if (!lang || text.length > MAX_HIGHLIGHT_CHARS) return [{ t: text }];
  const out: Tok[] = [];
  const lines = text.split("\n");
  lines.forEach((l, i) => {
    line(lang, l, out);
    if (i < lines.length - 1) out.push({ t: "\n" });
  });
  return out;
}
