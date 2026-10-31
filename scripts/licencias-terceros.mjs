// Genera la lista de componentes de terceros que van DENTRO de MercHosting (dependencias
// normales de Rust para Windows y de npm en producción), con su licencia:
//   docs/LICENCIAS_DE_TERCEROS.md   (para el repositorio)
//   app/src/about/terceros.json      (la muestra «Acerca de»)
// Avisa si aparece una licencia que no está en la lista de compatibles con GPL-3.0.
// Uso (desde la raíz): node scripts/licencias-terceros.mjs

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

// Licencias (y combinaciones) aceptadas para distribuir junto a código GPL-3.0-or-later.
const COMPATIBLE = new Set([
  "MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib", "0BSD", "Unlicense", "CC0-1.0", "MIT-0",
  "Unicode-3.0", "Unicode-DFS-2016", "MPL-2.0", "BSL-1.0", "CDLA-Permissive-2.0", "OFL-1.1", "Apache-2.0 WITH LLVM-exception",
  "BlueOak-1.0.0", "Python-2.0", "CC-BY-4.0",
]);

/** ¿Alguna opción de la expresión SPDX es compatible? (AND: todas las partes; OR: alguna). */
export function compatible(expr) {
  if (!expr) return false;
  const e = expr.replace(/\//g, " OR ").replace(/[()]/g, " ").trim();
  return e.split(/\s+OR\s+/).some((alt) => alt.split(/\s+AND\s+/).every((p) => COMPATIBLE.has(p.trim())));
}

function rust() {
  const out = execFileSync("cargo", ["metadata", "--format-version", "1", "--filter-platform", "x86_64-pc-windows-msvc"], {
    cwd: root,
    maxBuffer: 256 * 1024 * 1024,
  });
  const m = JSON.parse(out.toString());
  const ws = new Set(m.workspace_members);
  const nodes = new Map(m.resolve.nodes.map((n) => [n.id, n]));
  const pkgs = new Map(m.packages.map((p) => [p.id, p]));
  const seen = new Set();
  const stack = [...ws];
  while (stack.length) {
    const id = stack.pop();
    const n = nodes.get(id);
    if (!n) continue;
    for (const d of n.deps) {
      // Solo dependencias normales (las de compilación y pruebas no van en el programa).
      if (!d.dep_kinds.some((k) => k.kind === null)) continue;
      if (seen.has(d.pkg)) continue;
      seen.add(d.pkg);
      stack.push(d.pkg);
    }
  }
  return [...seen]
    .filter((id) => !ws.has(id))
    .map((id) => pkgs.get(id))
    .map((p) => ({ name: p.name, version: p.version, license: p.license ?? (p.license_file ? `ver ${p.license_file}` : ""), url: p.repository ?? p.homepage ?? `https://crates.io/crates/${p.name}`, kind: "rust" }));
}

function npm() {
  const lock = JSON.parse(readFileSync(join(root, "app", "package-lock.json"), "utf8"));
  const list = [];
  for (const [path, info] of Object.entries(lock.packages ?? {})) {
    if (!path.startsWith("node_modules/") || info.dev || info.devOptional) continue;
    const name = path.slice(path.lastIndexOf("node_modules/") + "node_modules/".length);
    let license = info.license ?? "";
    let url = `https://www.npmjs.com/package/${name}`;
    try {
      const pj = JSON.parse(readFileSync(join(root, "app", path, "package.json"), "utf8"));
      license ||= typeof pj.license === "string" ? pj.license : pj.license?.type ?? "";
      const repo = typeof pj.repository === "string" ? pj.repository : pj.repository?.url;
      if (repo) url = repo.replace(/^git\+/, "").replace(/\.git$/, "").replace(/^git:\/\//, "https://");
    } catch {
      /* sin node_modules: se usa lo del lockfile */
    }
    list.push({ name, version: info.version, license, url, kind: "npm" });
  }
  return list;
}

const items = [...rust(), ...npm()].sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version));
const bad = items.filter((i) => !compatible(i.license));

const md = [
  "# Licencias de terceros",
  "",
  "Componentes que van dentro de MercHosting (dependencias de Rust para Windows y de la interfaz), generado con `node scripts/licencias-terceros.mjs`. MercHosting se distribuye bajo **GPL-3.0-or-later** (ver `LICENSE`); todas estas licencias permiten distribuirlos junto a él.",
  "",
  "Lo que MercHosting **descarga a petición** desde su fuente oficial y no incluye (Java de Eclipse Temurin, el agente de Playit, los programas de servidor de Minecraft, mods y plugins) tiene su propia licencia: ver `docs/CREDITOS.md`.",
  "",
  `Total: ${items.length} componentes.`,
  "",
  "| Componente | Versión | Licencia | Origen |",
  "|---|---|---|---|",
  ...items.map((i) => `| ${i.name} | ${i.version} | ${i.license || "—"} | ${i.url} |`),
  "",
].join("\n");
writeFileSync(join(root, "docs", "LICENCIAS_DE_TERCEROS.md"), md);
mkdirSync(join(root, "app", "src", "about"), { recursive: true });
writeFileSync(join(root, "app", "src", "about", "terceros.json"), JSON.stringify(items.map(({ name, version, license, url }) => ({ name, version, license, url }))));
console.log(`${items.length} componentes (${items.filter((i) => i.kind === "rust").length} de Rust, ${items.filter((i) => i.kind === "npm").length} de npm)`);
if (bad.length) {
  console.log("⚠ Revisar a mano (licencia no reconocida como compatible):");
  for (const b of bad) console.log(`  ${b.name} ${b.version}: ${b.license || "(sin licencia declarada)"}`);
  process.exitCode = 1;
}
