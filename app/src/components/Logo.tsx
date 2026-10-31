import { useId } from "react";
import type { StateTone } from "../format";
import { LOGO_PIECES, LOGO_TRANSFORM, LOGO_VIEWBOX, type LogoPiece } from "./logoPaths";

interface Props {
  size?: number;
  /** Color del núcleo según el estado del servidor. Sin tono: núcleo azul de marca. */
  tone?: StateTone | "brand";
  /** Anima el ensamblado de las piezas (intro). */
  assemble?: boolean;
  className?: string;
  title?: string;
}

const ORDER: LogoPiece[] = ["outerLeft", "outerRight", "top", "innerLeft", "innerRight", "core"];

/** Logo de MercHosting. El rombo central es un indicador de estado vivo. */
export function Logo({ size = 40, tone = "brand", assemble = false, className = "", title }: Props) {
  const uid = useId().replace(/:/g, "");
  return (
    <svg
      className={`logo ${assemble ? "logo-assemble" : ""} ${className}`}
      data-tone={tone}
      width={size}
      height={size}
      viewBox={LOGO_VIEWBOX}
      role={title ? "img" : undefined}
      aria-label={title}
      aria-hidden={title ? undefined : true}
    >
      <defs>
        <linearGradient id={`iv-${uid}`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#ffffff" />
          <stop offset="1" stopColor="#c8d2f0" />
        </linearGradient>
      </defs>
      <g transform={LOGO_TRANSFORM}>
        {ORDER.map((k) => (
          <path
            key={k}
            className={`logo-piece logo-${k}`}
            d={LOGO_PIECES[k]}
            // El núcleo toma su color de CSS (.logo-core): WebKit no admite var() en atributos SVG.
            fill={k === "core" ? undefined : `url(#iv-${uid})`}
          />
        ))}
      </g>
    </svg>
  );
}
