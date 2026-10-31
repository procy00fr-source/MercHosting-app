// Celebración breve (Supporter, desactivable): bloquecitos que caen cuando un servidor termina de
// encender. Dura menos de 3 s, no bloquea clics y no aparece si Windows pide reducir animaciones.

import { useEffect, useMemo } from "react";

const COLORS = ["var(--copper)", "var(--copper-2)", "var(--champagne)", "var(--ok)", "var(--c-players)"];

export function Celebration({ text, onDone }: { text: string; onDone: () => void }) {
  useEffect(() => {
    const t = setTimeout(onDone, 2800);
    return () => clearTimeout(t);
  }, [onDone]);
  const bits = useMemo(
    () =>
      Array.from({ length: 36 }, (_, i) => ({
        left: (i * 37) % 100,
        delay: ((i * 53) % 60) / 100,
        size: 6 + ((i * 7) % 7),
        color: COLORS[i % COLORS.length],
        spin: (i % 2 ? 1 : -1) * (180 + ((i * 29) % 360)),
      })),
    [],
  );
  return (
    <div className="celebration" aria-hidden="true" data-testid="celebration">
      {bits.map((b, i) => (
        <span
          key={i}
          style={
            {
              left: `${b.left}%`,
              width: b.size,
              height: b.size,
              background: b.color,
              animationDelay: `${b.delay}s`,
              "--spin": `${b.spin}deg`,
            } as React.CSSProperties
          }
        />
      ))}
      <p className="celebration-text">{text}</p>
    </div>
  );
}
