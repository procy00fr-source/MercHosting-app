import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

/** Quita los comentarios para contar llaves. */
function code(file: string): string {
  return readFileSync(resolve(__dirname, file), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
}

describe("CSS bien cerrado", () => {
  // Un bloque @media sin cerrar mete todo lo que viene después dentro de esa condición: con las
  // animaciones de Windows apagadas, medio diseño de la app desaparecía.
  it.each(["styles.css", "light.css"])("%s tiene las llaves balanceadas", (file) => {
    let depth = 0;
    for (const ch of code(file)) {
      if (ch === "{") depth += 1;
      if (ch === "}") depth -= 1;
      expect(depth).toBeGreaterThanOrEqual(0);
    }
    expect(depth).toBe(0);
  });

  it("los bloques de «prefers-reduced-motion» solo traen animaciones y se cierran pronto", () => {
    const css = code("styles.css");
    const re = /@media \(prefers-reduced-motion: no-preference\) \{/g;
    for (let m = re.exec(css); m; m = re.exec(css)) {
      let depth = 0;
      let end = -1;
      for (let i = m.index + m[0].length - 1; i < css.length; i++) {
        if (css[i] === "{") depth += 1;
        if (css[i] === "}" && --depth === 0) {
          end = i;
          break;
        }
      }
      expect(end, `bloque sin cerrar en la posición ${m.index}`).toBeGreaterThan(0);
      expect(css.slice(m.index, end).split("\n").length, `bloque demasiado largo en la posición ${m.index}`).toBeLessThan(60);
    }
  });
});
