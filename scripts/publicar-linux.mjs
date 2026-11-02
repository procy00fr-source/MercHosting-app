// Compila MercHosting para Linux (.deb y AppImage) y lo añade a una release YA PUBLICADA desde
// Windows con scripts/publicar-version.mjs. Lo ejecuta GitHub Actions (.github/workflows/linux.yml)
// en Ubuntu; no hace falta tener Linux en casa.
//
// Uso (en el workflow): node scripts/publicar-linux.mjs 1.0.0 [--sin-subir]
//
// Necesita (secretos del repositorio, ver docs/LINUX.md):
//   TAURI_SIGNING_PRIVATE_KEY y TAURI_SIGNING_PRIVATE_KEY_PASSWORD: la MISMA clave con la que
//     se firman las actualizaciones de Windows (nunca se guarda en el repositorio).
//   GH_TOKEN: token con permiso para subir archivos a las releases del repositorio de releases.
//
// Qué hace:
//  1. Pone la versión en tauri.conf.json, package.json y Cargo.toml (solo en la copia del
//     workflow: no se sube nada al repositorio del código).
//  2. Compila el supervisor y la app con el actualizador y genera el .deb y el AppImage firmados.
//  3. Empaqueta solo la terminal (merc + merc-supervisord) en un .tar.gz para servidores caseros
//     sin escritorio (no necesitan las librerías gráficas del .deb).
//  4. Descarga el latest.json de esa release, le añade «linux-x86_64» (el AppImage) y lo vuelve
//     a subir junto con el .deb, el AppImage, el .tar.gz y SHA256SUMS-linux.txt.

import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  STABLE_LINUX,
  bumpCargoVersion,
  bumpJsonVersion,
  checkRelease,
  downloadUrl,
  mergeLinuxPlatform,
  parseVersion,
  releaseTauriConfigLinux,
  sha256Line,
} from "./publicar-version.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function fail(msg) {
  console.error(`\n✗ ${msg}\n`);
  process.exit(1);
}

function run(cmd, args, opts = {}) {
  console.log(`\n> ${cmd} ${args.join(" ")}`);
  const r = spawnSync(cmd, args, { stdio: "inherit", shell: false, ...opts });
  if (r.error) fail(`No se pudo ejecutar ${cmd}: ${r.error.message}`);
  if (r.status !== 0) fail(`Falló: ${cmd} ${args.join(" ")}`);
}

function main() {
  const args = process.argv.slice(2);
  const version = args.find((a) => !a.startsWith("--"));
  const upload = !args.includes("--sin-subir");
  if (process.platform !== "linux") fail("Este script compila la versión de Linux y se ejecuta en Linux (GitHub Actions).");
  if (!parseVersion(version)) fail("Indica la versión, por ejemplo: node scripts/publicar-linux.mjs 1.0.0");

  const release = JSON.parse(readFileSync(join(ROOT, "release.json"), "utf8"));
  const problems = checkRelease(release);
  if (problems.length) fail(problems.join("\n  "));
  if (!process.env.TAURI_SIGNING_PRIVATE_KEY) fail("Falta el secreto TAURI_SIGNING_PRIVATE_KEY (mira docs/LINUX.md).");
  if (upload && !process.env.GH_TOKEN) fail("Falta el secreto GH_TOKEN para subir a las releases (mira docs/LINUX.md).");

  const confPath = join(ROOT, "app/src-tauri/tauri.conf.json");
  writeFileSync(confPath, bumpJsonVersion(readFileSync(confPath, "utf8"), version));
  const pkgPath = join(ROOT, "app/package.json");
  writeFileSync(pkgPath, bumpJsonVersion(readFileSync(pkgPath, "utf8"), version));
  const cargoPath = join(ROOT, "Cargo.toml");
  writeFileSync(cargoPath, bumpCargoVersion(readFileSync(cargoPath, "utf8"), version));

  const env = { ...process.env, TAURI_SIGNING_PRIVATE_KEY_PASSWORD: process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ?? "" };
  run("cargo", ["build", "--release", "-p", "merc-supervisord"], { cwd: ROOT, env });
  const binDir = join(ROOT, "app/src-tauri/binaries");
  mkdirSync(binDir, { recursive: true });
  copyFileSync(join(ROOT, "target/release/merc-supervisord"), join(binDir, "merc-supervisord-x86_64-unknown-linux-gnu"));
  copyFileSync(join(ROOT, "target/release/merc"), join(binDir, "merc-x86_64-unknown-linux-gnu"));

  const extra = join(ROOT, "target/tauri.release.linux.conf.json");
  writeFileSync(extra, JSON.stringify(releaseTauriConfigLinux(release), null, 2));
  run("npx", ["tauri", "build", "--features", "updater", "--bundles", "deb,appimage", "--config", extra], {
    cwd: join(ROOT, "app"),
    env,
  });

  const bundle = join(ROOT, "target/release/bundle");
  const find = (dir, ext) => {
    const d = join(bundle, dir);
    const f = existsSync(d) ? readdirSync(d).find((x) => x.endsWith(ext) && x.includes(version)) : undefined;
    if (!f) fail(`No se encontró el ${ext} de ${version} en ${d}.`);
    return join(d, f);
  };
  const appimage = find("appimage", ".AppImage");
  const deb = find("deb", ".deb");
  if (!existsSync(`${appimage}.sig`)) fail("El AppImage no tiene firma (.sig): revisa TAURI_SIGNING_PRIVATE_KEY.");

  const out = join(ROOT, "target/release-linux", version);
  mkdirSync(out, { recursive: true });
  const name = (p) => p.split("/").pop();
  for (const [src, dst] of [
    [appimage, name(appimage)],
    [deb, name(deb)],
    [appimage, STABLE_LINUX.appimage],
    [deb, STABLE_LINUX.deb],
  ])
    copyFileSync(src, join(out, dst));
  // Servidor casero sin escritorio: solo los dos programas de terminal y unas líneas de ayuda.
  const stage = join(ROOT, "target/release-linux/servidor-casero");
  mkdirSync(stage, { recursive: true });
  copyFileSync(join(ROOT, "target/release/merc"), join(stage, "merc"));
  copyFileSync(join(ROOT, "target/release/merc-supervisord"), join(stage, "merc-supervisord"));
  writeFileSync(
    join(stage, "LEEME.txt"),
    `MercHosting ${version} — servidor casero sin pantalla (Linux x86_64)\n\n` +
      "1. Deja estos dos archivos juntos en una carpeta tuya, por ejemplo ~/merchosting\n" +
      "2. ./merc servicio instalar\n" +
      "3. ./merc compañeros invitar --control-total\n" +
      "4. Pega el código en MercHosting de tu PC: Compañeros › Conectar a mi servidor casero\n\n" +
      "Guía completa: docs/SERVIDOR_CASERO.md en https://github.com/procy00fr-source/MercHosting-app\n",
  );
  run("tar", ["-czf", join(out, STABLE_LINUX.headless), "-C", stage, "merc", "merc-supervisord", "LEEME.txt"]);
  writeFileSync(
    join(out, "SHA256SUMS-linux.txt"),
    [name(appimage), name(deb), STABLE_LINUX.appimage, STABLE_LINUX.deb, STABLE_LINUX.headless]
      .map((f) => sha256Line(f, readFileSync(join(out, f))))
      .join("\n") + "\n",
  );
  console.log(`\n✓ Listo en ${out}`);
  if (!upload) {
    console.log("No se subió nada (--sin-subir).");
    return;
  }

  const tag = `v${version}`;
  run("gh", ["release", "download", tag, "--repo", release.releases_repo, "--pattern", "latest.json", "--dir", out, "--clobber"]);
  const latest = mergeLinuxPlatform(JSON.parse(readFileSync(join(out, "latest.json"), "utf8")), {
    version,
    signature: readFileSync(`${appimage}.sig`, "utf8"),
    url: downloadUrl(release.releases_repo, version, name(appimage)),
  });
  writeFileSync(join(out, "latest.json"), JSON.stringify(latest, null, 2));
  run("gh", [
    "release", "upload", tag,
    ...[name(appimage), name(deb), STABLE_LINUX.appimage, STABLE_LINUX.deb, STABLE_LINUX.headless, "SHA256SUMS-linux.txt", "latest.json"].map(
      (f) => join(out, f),
    ),
    "--repo", release.releases_repo,
    "--clobber",
  ]);
  console.log(`\n✓ Linux añadido a https://github.com/${release.releases_repo}/releases/tag/${tag}`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
