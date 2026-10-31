// Markdown mínimo para los textos que incluye la app (novedades, privacidad): títulos, párrafos,
// listas, tablas, **negrita** y `código`. Todo se pinta como elementos de React, nunca como HTML.

import type { ReactNode } from "react";

function inline(text: string, key: string): ReactNode[] {
  const out: ReactNode[] = [];
  const re = /(\*\*[^*]+\*\*|`[^`]+`)/g;
  let last = 0;
  let i = 0;
  for (const m of text.matchAll(re)) {
    if (m.index! > last) out.push(text.slice(last, m.index));
    const t = m[0];
    out.push(t.startsWith("**") ? <strong key={`${key}-${i++}`}>{t.slice(2, -2)}</strong> : <code key={`${key}-${i++}`}>{t.slice(1, -1)}</code>);
    last = m.index! + t.length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

export function Markdown({ text, skipTitle = false }: { text: string; skipTitle?: boolean }) {
  const lines = text.replace(/\r/g, "").split("\n");
  const blocks: ReactNode[] = [];
  let i = 0;
  let k = 0;
  while (i < lines.length) {
    const l = lines[i];
    if (!l.trim()) {
      i++;
      continue;
    }
    const h = /^(#{1,4})\s+(.*)$/.exec(l);
    if (h) {
      if (!(skipTitle && h[1].length === 1)) {
        const level = Math.min(h[1].length + 1, 4);
        const Tag = `h${level}` as "h2" | "h3" | "h4";
        blocks.push(<Tag key={k++}>{inline(h[2], `h${k}`)}</Tag>);
      }
      i++;
      continue;
    }
    if (l.startsWith("```")) {
      const code: string[] = [];
      i++;
      while (i < lines.length && !lines[i].startsWith("```")) code.push(lines[i++]);
      i++;
      blocks.push(
        <pre key={k++} className="md-code">
          {code.join("\n")}
        </pre>,
      );
      continue;
    }
    if (l.startsWith("|")) {
      const rows: string[][] = [];
      while (i < lines.length && lines[i].startsWith("|")) {
        const cells = lines[i].slice(1, lines[i].trimEnd().endsWith("|") ? -1 : undefined).split("|").map((c) => c.trim());
        if (!cells.every((c) => /^-+$/.test(c))) rows.push(cells);
        i++;
      }
      const [head, ...body] = rows;
      blocks.push(
        <div key={k++} className="md-table-wrap">
          <table className="md-table">
            <thead>
              <tr>{head.map((c, j) => <th key={j}>{inline(c, `th${k}${j}`)}</th>)}</tr>
            </thead>
            <tbody>
              {body.map((r, ri) => (
                <tr key={ri}>{r.map((c, j) => <td key={j}>{inline(c, `td${k}${ri}${j}`)}</td>)}</tr>
              ))}
            </tbody>
          </table>
        </div>,
      );
      continue;
    }
    if (/^\s*(-|\d+\.)\s/.test(l)) {
      const items: string[] = [];
      const ordered = /^\s*\d+\./.test(l);
      while (i < lines.length && /^\s*(-|\d+\.)\s/.test(lines[i])) {
        let item = lines[i].replace(/^\s*(-|\d+\.)\s+/, "");
        i++;
        while (i < lines.length && /^\s{2,}\S/.test(lines[i]) && !/^\s*(-|\d+\.)\s/.test(lines[i])) item += " " + lines[i++].trim();
        items.push(item);
      }
      const List = ordered ? "ol" : "ul";
      blocks.push(<List key={k++}>{items.map((it, j) => <li key={j}>{inline(it, `li${k}${j}`)}</li>)}</List>);
      continue;
    }
    const para: string[] = [];
    while (i < lines.length && lines[i].trim() && !/^(#|\||```|\s*(-|\d+\.)\s)/.test(lines[i])) para.push(lines[i++].trim());
    blocks.push(<p key={k++}>{inline(para.join(" "), `p${k}`)}</p>);
  }
  return <div className="md">{blocks}</div>;
}

/** Secciones `## versión` del CHANGELOG con sus puntos. */
export function changelogVersions(text: string): { version: string; items: string[] }[] {
  const out: { version: string; items: string[] }[] = [];
  for (const line of text.replace(/\r/g, "").split("\n")) {
    const h = /^##\s+(.+)$/.exec(line);
    if (h) out.push({ version: h[1].trim(), items: [] });
    else if (out.length && /^\s*-\s/.test(line)) out[out.length - 1].items.push(line.replace(/^\s*-\s+/, ""));
  }
  return out;
}
