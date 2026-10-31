// Capa del tutorial: oscurece todo menos el control real que se explica y muestra una tarjeta
// con el texto. Esc sale, Enter avanza. Si el paso pide hacer clic en el control, se puede
// hacer clic de verdad (el resto de la pantalla queda bloqueada mientras tanto).

import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { Tour, TourNav } from "../tours";

interface Props {
  tour: Tour;
  onNav: (nav: TourNav) => void;
  onClose: (finished: boolean) => void;
}

type Rect = { top: number; left: number; width: number; height: number };
const PAD = 6;
const WAIT_MS = 1800;

export function TourOverlay({ tour, onNav, onClose }: Props) {
  const [i, setI] = useState(0);
  const [rect, setRect] = useState<Rect | null>(null);
  const [missing, setMissing] = useState(false);
  const card = useRef<HTMLDivElement>(null);
  const step = tour.steps[i];
  const last = i === tour.steps.length - 1;

  const next = useCallback(() => (last ? onClose(true) : setI((n) => n + 1)), [last, onClose]);

  // Navegar (si el paso lo pide) y esperar a que aparezca el control.
  useEffect(() => {
    setRect(null);
    setMissing(false);
    if (step.nav) onNav(step.nav);
    if (!step.target) return;
    const started = Date.now();
    let el: Element | null = null;
    let attached: Element | null = null;
    const onClick = () => {
      if (step.advanceOnClick) setTimeout(next, 0);
    };
    const measure = () => {
      el = document.querySelector(step.target!);
      if (el) {
        // El clic se escucha en cuanto aparece el control (si cambia, en el nuevo).
        if (step.advanceOnClick && attached !== el) {
          attached?.removeEventListener("click", onClick);
          el.addEventListener("click", onClick, { once: true });
          attached = el;
        }
        el.scrollIntoView?.({ block: "nearest" });
        const r = el.getBoundingClientRect();
        setRect({ top: r.top - PAD, left: r.left - PAD, width: r.width + PAD * 2, height: r.height + PAD * 2 });
        setMissing(false);
      } else if (Date.now() - started > WAIT_MS) {
        setRect(null);
        setMissing(true);
      }
    };
    measure();
    const t = setInterval(measure, 250);
    return () => {
      clearInterval(t);
      attached?.removeEventListener("click", onClick);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [i]);

  useLayoutEffect(() => {
    card.current?.focus();
  }, [i]);

  function onKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") onClose(false);
    else if (e.key === "Enter" && !(step.advanceOnClick && rect)) next();
  }

  // Tarjeta debajo del control si cabe; si no, encima. Sin control: centrada.
  const style: React.CSSProperties = {};
  if (rect) {
    const below = rect.top + rect.height + 12;
    if (below + 220 < window.innerHeight) style.top = below;
    else style.top = Math.max(12, rect.top - 232);
    style.left = Math.min(Math.max(12, rect.left), window.innerWidth - 372);
  }

  const blockers = rect
    ? [
        { top: 0, left: 0, width: "100%", height: Math.max(0, rect.top) },
        { top: rect.top + rect.height, left: 0, width: "100%", bottom: 0 },
        { top: rect.top, left: 0, width: Math.max(0, rect.left), height: rect.height },
        { top: rect.top, left: rect.left + rect.width, right: 0, height: rect.height },
      ]
    : [{ top: 0, left: 0, right: 0, bottom: 0 }];

  return (
    <div className="tour" onKeyDown={onKeyDown}>
      {blockers.map((b, k) => (
        <div key={k} className="tour-shade" style={b as React.CSSProperties} />
      ))}
      {rect && <div className="tour-ring" style={rect} aria-hidden="true" />}
      <div
        ref={card}
        className={`tour-card ${rect ? "" : "tour-card-center"}`}
        style={style}
        role="dialog"
        aria-modal="true"
        aria-label={`Tutorial: ${tour.name}`}
        tabIndex={-1}
      >
        <p className="tour-count">
          {tour.name} · {i + 1} de {tour.steps.length}
        </p>
        <h2>{step.title}</h2>
        <p>{step.body}</p>
        {missing && <p className="field-help">(Ese control no está a la vista ahora mismo; puede depender del estado del servidor.)</p>}
        {step.advanceOnClick && rect && <p className="field-help">Haz clic en el control resaltado para seguir.</p>}
        <div className="tour-actions">
          <button type="button" className="btn btn-sm btn-quiet" onClick={() => onClose(false)}>
            Saltar tutorial
          </button>
          <span className="dialog-spacer" />
          {i > 0 && (
            <button type="button" className="btn btn-sm btn-quiet" onClick={() => setI(i - 1)}>
              Atrás
            </button>
          )}
          {!(step.advanceOnClick && rect) && (
            <button type="button" className="btn btn-sm btn-go" onClick={next}>
              {last ? "Terminar" : "Siguiente"}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
