import { useEffect, useState } from "react";
import { playIntroSound } from "../sound/introSound";
import { Logo } from "./Logo";

/**
 * Entrada de la app: el cubo se arma pieza a pieza, aparece la marca y la pantalla se abre.
 * Dura lo que tarde en conectar con el supervisor, con un mínimo para que se pueda leer el eslogan
 * con calma. Ya conectada, un clic (o una tecla) la salta.
 */
export function Splash({ ready, error }: { ready: boolean; error: string | null }) {
  const reduce = typeof window !== "undefined" && window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
  const minMs = reduce ? 0 : 4500;
  const [minDone, setMinDone] = useState(minMs === 0);
  const [leaving, setLeaving] = useState(false);
  const [gone, setGone] = useState(false);

  useEffect(() => {
    if (!reduce) playIntroSound();
  }, [reduce]);

  useEffect(() => {
    if (minMs === 0) return;
    const t = setTimeout(() => setMinDone(true), minMs);
    return () => clearTimeout(t);
  }, [minMs]);

  useEffect(() => {
    if (ready && minDone && !leaving) {
      setLeaving(true);
      const t = setTimeout(() => setGone(true), reduce ? 0 : 650);
      return () => clearTimeout(t);
    }
  }, [ready, minDone, leaving, reduce]);

  useEffect(() => {
    if (!ready || minDone) return;
    const skip = () => setMinDone(true);
    window.addEventListener("keydown", skip);
    return () => window.removeEventListener("keydown", skip);
  }, [ready, minDone]);

  if (gone) return null;
  return (
    <div
      className={`splash ${leaving ? "splash-leave" : ""}`}
      aria-hidden={leaving}
      onClick={() => {
        if (ready) setMinDone(true);
      }}
    >
      <div className="splash-inner">
        <Logo size={150} assemble tone={error ? "bad" : "brand"} />
        <h1 className="splash-word">
          {"MercHosting".split("").map((ch, i) => (
            <span key={i} style={{ animationDelay: `${0.85 + i * 0.045}s` }}>
              {ch}
            </span>
          ))}
        </h1>
        <p className="splash-tag">Tu primer servidor no debería necesitar una mensualidad</p>
        <p className="splash-status" role="status">
          {error ?? (ready ? "Listo" : "Conectando con el supervisor…")}
        </p>
      </div>
    </div>
  );
}
