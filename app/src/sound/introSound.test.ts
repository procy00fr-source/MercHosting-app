import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { CORE_AT, PIECE_LANDINGS, introSoundEnabled, setIntroSoundEnabled } from "./introSound";

const css = readFileSync(resolve(process.cwd(), "src/styles.css"), "utf8");

function delayOf(piece: string): number {
  const block = css.match(new RegExp(`\\.logo-assemble \\.logo-${piece} \\{([^}]*)\\}`))?.[1] ?? "";
  const m = block.match(/animation-delay:\s*([\d.]+)s/) ?? block.match(/piece-in [\d.]+s var\(--ease-spring\) ([\d.]+)s/);
  if (!m) throw new Error(`no se encontró el retraso de ${piece}`);
  return Number(m[1]);
}

describe("sonido de entrada sincronizado con la animación", () => {
  it("cada golpe suena cuando su pieza encaja (retraso + ~0,35 s)", () => {
    const pieces = ["outerLeft", "outerRight", "top", "innerLeft", "innerRight"];
    pieces.forEach((p, i) => {
      expect(Math.abs(PIECE_LANDINGS[i] - (delayOf(p) + 0.35))).toBeLessThanOrEqual(0.06);
    });
  });

  it("el acorde suena cuando se enciende el núcleo", () => {
    expect(Math.abs(CORE_AT - (delayOf("core") + 0.3))).toBeLessThanOrEqual(0.06);
  });

  it("la preferencia de sonido se guarda", () => {
    setIntroSoundEnabled(false);
    expect(introSoundEnabled()).toBe(false);
    setIntroSoundEnabled(true);
    expect(introSoundEnabled()).toBe(true);
  });
});
