import { useEffect, useRef } from "react";

/**
 * "Polvo de bloques": pequeños rombos isométricos que suben lentamente detrás de la interfaz.
 * Se detiene si la ventana está oculta y no se dibuja si el sistema pide reducir movimiento.
 */
export function Particles({ density = 46 }: { density?: number }) {
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const reduce = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
    const ctx = canvas.getContext("2d");
    if (!ctx || reduce) return;

    let w = 0;
    let h = 0;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    type P = { x: number; y: number; s: number; v: number; a: number; drift: number; hue: number; phase: number };
    let ps: P[] = [];
    const mouse = { x: 0.5, y: 0.5 };

    const spawn = (anyY: boolean): P => ({
      x: Math.random() * w,
      y: anyY ? Math.random() * h : h + 10,
      s: 1.5 + Math.random() * 3.5,
      v: 0.08 + Math.random() * 0.28,
      a: 0.08 + Math.random() * 0.32,
      drift: (Math.random() - 0.5) * 0.15,
      hue: Math.random(),
      phase: Math.random() * Math.PI * 2,
    });

    const resize = () => {
      w = canvas.clientWidth;
      h = canvas.clientHeight;
      canvas.width = Math.floor(w * dpr);
      canvas.height = Math.floor(h * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      const n = Math.round((density * (w * h)) / (1280 * 800));
      ps = Array.from({ length: n }, () => spawn(true));
    };
    resize();
    window.addEventListener("resize", resize);
    const onMove = (e: MouseEvent) => {
      mouse.x = e.clientX / Math.max(1, w);
      mouse.y = e.clientY / Math.max(1, h);
    };
    window.addEventListener("mousemove", onMove);

    let raf = 0;
    let t = 0;
    const frame = () => {
      raf = requestAnimationFrame(frame);
      if (document.hidden) return;
      t += 1;
      ctx.clearRect(0, 0, w, h);
      const px = (mouse.x - 0.5) * 14;
      const py = (mouse.y - 0.5) * 10;
      for (const p of ps) {
        p.y -= p.v;
        p.x += p.drift + Math.sin((t + p.phase * 60) / 140) * 0.06;
        if (p.y < -12) Object.assign(p, spawn(false));
        const depth = p.s / 5;
        const x = p.x + px * depth;
        const y = p.y + py * depth;
        const twinkle = 0.65 + 0.35 * Math.sin(t / 50 + p.phase);
        ctx.globalAlpha = p.a * twinkle;
        ctx.fillStyle = p.hue > 0.72 ? "#9b7bff" : p.hue > 0.35 ? "#dbe5ff" : "#5b8cff";
        // Rombo isométrico (la cara superior de un bloque)
        ctx.beginPath();
        ctx.moveTo(x, y - p.s * 0.58);
        ctx.lineTo(x + p.s, y);
        ctx.lineTo(x, y + p.s * 0.58);
        ctx.lineTo(x - p.s, y);
        ctx.closePath();
        ctx.fill();
      }
      ctx.globalAlpha = 1;
    };
    raf = requestAnimationFrame(frame);
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", resize);
      window.removeEventListener("mousemove", onMove);
    };
  }, [density]);

  return <canvas ref={ref} className="particles" aria-hidden="true" />;
}
