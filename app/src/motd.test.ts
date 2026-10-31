import { describe, expect, it } from "vitest";
import { applyToSelection, centerLine, fromEditor, layout, LINE_WIDTH, parse, styleAt, textWidth, toEditor } from "./motd";

describe("MOTD", () => {
  it("pasa de «&» a «§» solo con códigos válidos", () => {
    expect(fromEditor("&6&lHola & adiós &z")).toBe("§6§lHola & adiós &z");
    expect(toEditor("§6Hola")).toBe("&6Hola");
    expect(fromEditor("&L")).toBe("§l");
  });

  it("un color quita los formatos y &r lo quita todo, como en el juego", () => {
    const s = parse("§lA§cB§oC§rD");
    expect(s.map((x) => [x.text, x.color, x.bold, x.italic])).toEqual([
      ["A", null, true, false],
      ["B", "c", false, false],
      ["C", "c", false, true],
      ["D", null, false, false],
    ]);
  });

  it("aplica un color a lo seleccionado y vuelve al estilo de antes", () => {
    const r = applyToSelection("&lHola mundo", 2, 6, "c");
    expect(r.text).toBe("&l&c&lHola&r&l mundo");
    expect(styleAt(r.text, r.text.length)).toMatchObject({ color: null, bold: true });
    expect(applyToSelection("abc", 1, 1, "l")).toEqual({ text: "a&lbc", cursor: 3 });
  });

  it("corta como la lista de servidores: lo largo baja de línea y solo se ven dos", () => {
    const long = "palabra ".repeat(12).trim();
    const v = layout(`${long}\nsegunda`);
    expect(v.lines.length).toBe(2);
    expect(v.hidden).toBe(true);
    for (const l of v.lines) expect(textWidth(l)).toBeLessThanOrEqual(LINE_WIDTH);
    expect(layout("Hola\nMundo")).toMatchObject({ hidden: false });
  });

  it("la negrita es más ancha y centrar añade espacios", () => {
    expect(textWidth(parse("§lAAA"))).toBe(21);
    expect(textWidth(parse("AAA"))).toBe(18);
    const c = centerLine("&6Hola");
    expect(c.startsWith(" ".repeat(20))).toBe(true);
    expect(centerLine(c)).toBe(c);
  });
});
