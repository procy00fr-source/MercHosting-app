// Aplica el traductor a lo que React pone en la página, sin tocar los componentes: textos y
// atributos visibles (placeholder, title, aria-label, alt). Guarda el original de cada nodo y,
// cuando React cambia un texto, vuelve a traducir desde el nuevo original.
// No se traduce lo que es de la persona o del servidor: consola, editor, campos de texto,
// rutas (`.mono`) y lo marcado con translate="no".

import type { Translator } from "./translate";

const ATTRS = ["placeholder", "title", "aria-label", "alt"];
const SKIP = 'textarea, input, script, style, code, pre, .mono, .console-lines, .code-hl, [translate="no"], [contenteditable="true"]';

export function startDomTranslator(root: HTMLElement, tr: Translator): () => void {
  const original = new WeakMap<Node, string>();
  const written = new WeakMap<Node, string>();
  const attrOriginal = new WeakMap<Element, Map<string, string>>();
  const attrWritten = new WeakMap<Element, Map<string, string>>();

  const skipped = (n: Node) => {
    const el = n.nodeType === Node.ELEMENT_NODE ? (n as Element) : n.parentElement;
    return !el || !!el.closest(SKIP);
  };

  const text = (n: Text) => {
    if (skipped(n)) return;
    const now = n.nodeValue ?? "";
    if (written.get(n) !== now) original.set(n, now);
    const src = original.get(n) ?? now;
    const out = tr.tr(src);
    if (out !== now) {
      written.set(n, out);
      n.nodeValue = out;
    } else written.set(n, now);
  };

  /** Elemento solo con textos (p. ej. «Hay », «3», « mods»): se traduce la frase entera. */
  const block = (el: Element): boolean => {
    const kids = Array.from(el.childNodes);
    if (kids.length < 2 || !kids.every((k) => k.nodeType === Node.TEXT_NODE) || skipped(el)) return false;
    const texts = kids as Text[];
    for (const k of texts) {
      const now = k.nodeValue ?? "";
      if (written.get(k) !== now) original.set(k, now);
    }
    const whole = texts.map((k) => original.get(k) ?? "").join("");
    const out = tr.tr(whole);
    if (out === whole) {
      texts.forEach(text);
      return true;
    }
    texts.forEach((k, i) => {
      const v = i === 0 ? out : "";
      written.set(k, v);
      if (k.nodeValue !== v) k.nodeValue = v;
    });
    return true;
  };

  const attrs = (el: Element) => {
    if (el.closest('[translate="no"]')) return;
    for (const a of ATTRS) {
      const now = el.getAttribute(a);
      if (now === null) continue;
      const orig = attrOriginal.get(el) ?? new Map<string, string>();
      const mine = attrWritten.get(el) ?? new Map<string, string>();
      if (mine.get(a) !== now) orig.set(a, now);
      const out = tr.tr(orig.get(a) ?? now);
      mine.set(a, out);
      attrOriginal.set(el, orig);
      attrWritten.set(el, mine);
      if (out !== now) el.setAttribute(a, out);
    }
  };

  const walk = (n: Node) => {
    if (n.nodeType === Node.TEXT_NODE) {
      const p = n.parentElement;
      if (p && block(p)) return;
      text(n as Text);
      return;
    }
    if (n.nodeType !== Node.ELEMENT_NODE) return;
    const el = n as Element;
    attrs(el);
    if (block(el)) return;
    el.childNodes.forEach(walk);
  };

  walk(root);
  const obs = new MutationObserver((list) => {
    for (const m of list) {
      if (m.type === "characterData") {
        const p = m.target.parentElement;
        if (p && block(p)) continue;
        text(m.target as Text);
      } else if (m.type === "attributes") attrs(m.target as Element);
      else {
        if (m.target.nodeType === Node.ELEMENT_NODE && block(m.target as Element)) continue;
        m.addedNodes.forEach(walk);
      }
    }
  });
  obs.observe(root, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ATTRS });
  return () => obs.disconnect();
}
