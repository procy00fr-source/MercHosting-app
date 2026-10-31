import { useId } from "react";

/** Gráfica de área para series de 0..max. Sin datos inventados: con menos de 2 puntos no dibuja. */
export function AreaChart({ values, max, color, label }: { values: number[]; max: number; color: string; label: string }) {
  const gid = useId().replace(/:/g, "");
  const W = 300;
  const H = 72;
  if (values.length < 2 || max <= 0) {
    return (
      <div className="chart-empty" role="img" aria-label={`${label}: reuniendo datos`}>
        Reuniendo datos…
      </div>
    );
  }
  const step = W / (values.length - 1);
  const pts = values.map((v, i) => [i * step, H - (Math.min(v, max) / max) * (H - 6) - 3] as const);
  const line = pts.map(([x, y], i) => `${i ? "L" : "M"}${x.toFixed(1)},${y.toFixed(1)}`).join(" ");
  const area = `${line} L${W},${H} L0,${H} Z`;
  const [lx, ly] = pts[pts.length - 1];
  return (
    <svg className="chart" viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" role="img" aria-label={label}>
      <defs>
        <linearGradient id={`g-${gid}`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor={color} stopOpacity="0.38" />
          <stop offset="1" stopColor={color} stopOpacity="0" />
        </linearGradient>
      </defs>
      {[0.25, 0.5, 0.75].map((f) => (
        <line key={f} x1="0" x2={W} y1={H * f} y2={H * f} className="chart-grid" />
      ))}
      <path d={area} fill={`url(#g-${gid})`} />
      <path d={line} fill="none" stroke={color} strokeWidth="2" vectorEffect="non-scaling-stroke" strokeLinejoin="round" />
      <circle cx={lx} cy={ly} r="3.2" fill={color} className="chart-dot" />
    </svg>
  );
}

/** Barra de proporción con relleno animado. */
export function Meter({ value, max, color }: { value: number; max: number; color: string }) {
  const pct = max > 0 ? Math.min(100, (value / max) * 100) : 0;
  return (
    <div className="meter" role="presentation">
      <div className="meter-fill" style={{ width: `${pct}%`, background: color }} />
    </div>
  );
}
