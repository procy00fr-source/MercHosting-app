import { execFileSync } from "node:child_process";
import { resolve } from "node:path";
import { expect, it } from "vitest";

it("light.css (modo claro) está generado a partir del styles.css actual", () => {
  const out = execFileSync("node", [resolve(__dirname, "../../scripts/generar-modo-claro.mjs"), "--check"], { encoding: "utf8" });
  expect(out).toContain("al día");
});
