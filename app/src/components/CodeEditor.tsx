// Editor de texto con colores: un <textarea> normal (accesible, deshacer del sistema, pegar)
// con el texto transparente encima de una copia coloreada que se desplaza a la vez.

import { useMemo, useRef } from "react";
import { highlight, langOf } from "../highlight";

interface Props {
  path: string;
  value: string;
  onChange: (v: string) => void;
  readOnly?: boolean;
  label: string;
}

export function CodeEditor({ path, value, onChange, readOnly, label }: Props) {
  const lang = langOf(path);
  const toks = useMemo(() => highlight(value, lang), [value, lang]);
  const pre = useRef<HTMLPreElement>(null);
  const sync = (e: React.UIEvent<HTMLTextAreaElement>) => {
    if (pre.current) {
      pre.current.scrollTop = e.currentTarget.scrollTop;
      pre.current.scrollLeft = e.currentTarget.scrollLeft;
    }
  };
  if (!lang) {
    return <textarea aria-label={label} value={value} spellCheck={false} readOnly={readOnly} onChange={(e) => onChange(e.target.value)} />;
  }
  return (
    <div className="code-editor" data-lang={lang}>
      <pre ref={pre} className="code-hl" aria-hidden="true">
        {toks.map((t, i) => (t.c ? <span key={i} className={`hl-${t.c}`}>{t.t}</span> : t.t))}
        {"\n"}
      </pre>
      <textarea
        aria-label={label}
        value={value}
        spellCheck={false}
        readOnly={readOnly}
        onChange={(e) => onChange(e.target.value)}
        onScroll={sync}
        wrap="off"
      />
    </div>
  );
}
