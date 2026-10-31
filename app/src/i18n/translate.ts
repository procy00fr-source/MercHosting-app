// Traductor de textos de la interfaz: el texto en español es la clave. Busca primero el texto
// exacto y después patrones con huecos ({0}, {1}…) para los textos que llevan valores.
// Lo que no está en el diccionario se deja como está (nunca se inventa una traducción).

export interface Dictionary {
  exact: Record<string, string>;
  patterns: [string, string][];
}

interface Pattern {
  re: RegExp;
  /** Trozo fijo más largo: filtro rápido antes de probar la expresión. */
  probe: string;
  /** Cuánto texto fijo tiene: los más concretos se prueban antes. */
  weight: number;
  to: string;
}

const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const norm = (s: string) => s.replace(/\s+/g, " ").trim();

export class Translator {
  private exact = new Map<string, string>();
  private patterns: Pattern[] = [];

  constructor(dict: Dictionary) {
    for (const [k, v] of Object.entries(dict.exact)) this.exact.set(norm(k), v);
    for (const [from, to] of dict.patterns) {
      const parts = norm(from).split(/\{\d+\}/);
      const holes = [...norm(from).matchAll(/\{(\d+)\}/g)].map((m) => Number(m[1]));
      if (!holes.length) {
        this.exact.set(norm(from), to);
        continue;
      }
      let src = "^";
      parts.forEach((p, i) => {
        src += escape(p);
        if (i < holes.length) src += `(?<h${holes[i]}>[\\s\\S]*?)`;
      });
      src += "$";
      try {
        this.patterns.push({
          re: new RegExp(src),
          probe: parts.reduce((a, b) => (b.length > a.length ? b : a), ""),
          weight: parts.join("").length,
          to,
        });
      } catch {
        /* patrón no válido: se ignora */
      }
    }
    // Primero los más específicos (más texto fijo).
    this.patterns.sort((a, b) => b.weight - a.weight);
  }

  /** Traduce conservando los espacios de los extremos. Devuelve el mismo texto si no lo conoce. */
  tr(text: string): string {
    if (!text || !/[A-Za-zÁÉÍÓÚáéíóúÑñ]/.test(text)) return text;
    const lead = /^\s/.test(text) ? " " : "";
    const trail = /\s$/.test(text) && text.trim() ? " " : "";
    const core = norm(text);
    const out = this.core(core, 0);
    return out === null ? text : lead + out + trail;
  }

  private core(core: string, depth: number): string | null {
    const hit = this.exact.get(core);
    if (hit !== undefined) return hit;
    if (depth > 3) return null;
    for (const p of this.patterns) {
      if (p.probe && !core.includes(p.probe)) continue;
      const m = p.re.exec(core);
      if (!m) continue;
      return p.to.replace(/\{(\d+)\}/g, (_, n) => {
        const v = m.groups?.[`h${n}`] ?? "";
        const inner = v.trim();
        const done = inner ? this.core(inner, depth + 1) : null;
        return done === null ? v : v.slice(0, v.indexOf(inner)) + done + v.slice(v.indexOf(inner) + inner.length);
      });
    }
    // Listas «a · b · c»: cada parte por separado.
    if (core.includes(" · ")) {
      const parts = core.split(" · ");
      const out = parts.map((x) => this.core(x, depth + 1));
      if (out.some((x) => x !== null)) return out.map((x, i) => x ?? parts[i]).join(" · ");
    }
    return null;
  }
}

let active: Translator | null = null;
export function setActiveTranslator(t: Translator | null) {
  active = t;
}
/** Texto para fuera del DOM (notificaciones de Windows, título de la ventana). */
export function t(text: string): string {
  return active ? active.tr(text) : text;
}
