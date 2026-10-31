import { useEffect, useId, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";

interface DialogProps {
  title: string;
  lead?: ReactNode;
  onClose: () => void;
  children?: ReactNode;
  footer: ReactNode;
  /** Diálogo ancho (asistentes con varios pasos). */
  wide?: boolean;
}

/** Diálogo modal accesible: foco inicial, Escape para cerrar y foco atrapado dentro. */
export function Dialog({ title, lead, onClose, children, footer, wide = false }: DialogProps) {
  const titleId = useId();
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const first = ref.current?.querySelector<HTMLElement>("button, input, select, [tabindex]");
    first?.focus();
    return () => previous?.focus();
  }, []);

  function onKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onClose();
      return;
    }
    if (e.key !== "Tab" || !ref.current) return;
    const items = Array.from(
      ref.current.querySelectorAll<HTMLElement>("button:not(:disabled), input:not(:disabled), select:not(:disabled)"),
    );
    if (items.length === 0) return;
    const first = items[0];
    const last = items[items.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }

  // Se dibuja en <body>: así no lo desplaza ni lo recorta el panel con scroll que lo abrió.
  return createPortal(
    <div className="dialog-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={wide ? "dialog dialog-wide" : "dialog"} role="dialog" aria-modal="true" aria-labelledby={titleId} ref={ref} onKeyDown={onKeyDown}>
        <h2 id={titleId}>{title}</h2>
        {lead && <div className="dialog-lead">{lead}</div>}
        {children}
        <div className="dialog-foot">{footer}</div>
      </div>
    </div>,
    document.body,
  );
}
