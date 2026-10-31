import { useMemo } from "react";
import type { StateTone } from "../format";

/**
 * "Isla" de bloques isométricos generada a partir del id del servidor: cada servidor tiene la
 * suya y siempre es la misma. Arte original de MercHosting (no usa recursos de Minecraft).
 */

function seeded(seed: string) {
  let h = 2166136261;
  for (let i = 0; i < seed.length; i++) {
    h ^= seed.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return () => {
    h ^= h << 13;
    h ^= h >>> 17;
    h ^= h << 5;
    return ((h >>> 0) % 10000) / 10000;
  };
}

const PALETTE: Record<StateTone, { top: string; left: string; right: string; lit: string }> = {
  ok: { top: "#3d6dff", left: "#22388f", right: "#172663", lit: "#8fe6ff" },
  busy: { top: "#8a7448", left: "#4c3f2a", right: "#33291c", lit: "#ffd27a" },
  bad: { top: "#8f4a5a", left: "#4d2733", right: "#341a23", lit: "#ff8a8a" },
  idle: { top: "#33437a", left: "#1d2850", right: "#141c3a", lit: "#8c9ccc" },
  unsure: { top: "#5e5a88", left: "#36334f", right: "#262438", lit: "#d4ccff" },
};

const TW = 34; // medio ancho del rombo
const TH = 19; // medio alto del rombo
const BH = 24; // altura de un bloque

export function VoxelScene({ seed, tone }: { seed: string; tone: StateTone }) {
  const blocks = useMemo(() => {
    const rnd = seeded(seed);
    const N = 7;
    const out: { x: number; y: number; z: number; lit: boolean; float: boolean; key: string }[] = [];
    for (let gy = 0; gy < N; gy++) {
      for (let gx = 0; gx < N; gx++) {
        const dx = gx - (N - 1) / 2;
        const dy = gy - (N - 1) / 2;
        const d = Math.sqrt(dx * dx + dy * dy);
        if (d > 3.3 + rnd() * 0.6) continue; // forma de isla
        const height = Math.max(1, Math.round(3.2 - d * 0.7 + rnd() * 1.6));
        for (let z = 0; z < height; z++) {
          out.push({ x: gx, y: gy, z, lit: z === height - 1 && rnd() > 0.82, float: false, key: `${gx}-${gy}-${z}` });
        }
      }
    }
    // Algunos bloques flotando sobre la isla
    for (let i = 0; i < 4; i++) {
      out.push({ x: 1 + Math.floor(rnd() * 5), y: 1 + Math.floor(rnd() * 5), z: 5 + Math.floor(rnd() * 3), lit: rnd() > 0.5, float: true, key: `f${i}` });
    }
    // Orden de pintado: de atrás hacia delante y de abajo arriba
    return out.sort((a, b) => a.x + a.y - (b.x + b.y) || a.z - b.z);
  }, [seed]);

  const c = PALETTE[tone];
  return (
    <svg className="voxel-scene" viewBox="-260 -230 520 420" aria-hidden="true" data-tone={tone}>
      <defs>
        <radialGradient id="vx-glow" cx="0.5" cy="0.62" r="0.55">
          <stop offset="0" stopColor={c.lit} stopOpacity="0.22" />
          <stop offset="1" stopColor={c.lit} stopOpacity="0" />
        </radialGradient>
      </defs>
      <ellipse cx="0" cy="60" rx="250" ry="150" fill="url(#vx-glow)" />
      {blocks.map((b, i) => {
        const cx = (b.x - b.y) * TW;
        const cy = (b.x + b.y) * TH - b.z * BH - 40;
        const top = `${cx},${cy - TH} ${cx + TW},${cy} ${cx},${cy + TH} ${cx - TW},${cy}`;
        const left = `${cx - TW},${cy} ${cx},${cy + TH} ${cx},${cy + TH + BH} ${cx - TW},${cy + BH}`;
        const right = `${cx + TW},${cy} ${cx},${cy + TH} ${cx},${cy + TH + BH} ${cx + TW},${cy + BH}`;
        return (
          <g key={b.key} className={b.float ? "vx-float" : undefined} style={b.float ? { animationDelay: `${(i % 4) * -1.3}s` } : undefined}>
            <polygon points={left} fill={c.left} />
            <polygon points={right} fill={c.right} />
            <polygon points={top} fill={b.lit ? c.lit : c.top} opacity={b.lit ? 0.9 : 1} />
            <polyline points={`${cx - TW},${cy} ${cx},${cy - TH} ${cx + TW},${cy}`} fill="none" stroke="#ffffff" strokeOpacity="0.08" />
          </g>
        );
      })}
    </svg>
  );
}
