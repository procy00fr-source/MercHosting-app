// Publica una versión nueva de MercHosting en GitHub Releases (repositorio público de releases).
//
// Uso (en la PC del desarrollador, Windows):
//   node scripts/publicar-version.mjs 0.2.0
//   node scripts/publicar-version.mjs 0.2.0 --sin-subir   (compila y prepara todo, no publica)
//
// Qué hace:
//  1. Lee release.json (repositorio de releases y clave PÚBLICA del actualizador).
//  2. Toma las notas de CHANGELOG.md («## 0.2.0»).
//  3. Pone la versión en tauri.conf.json, package.json y Cargo.toml.
//  4. Compila el supervisor y lo incluye en el instalador.
//  5. Compila el instalador NSIS con el actualizador activado, firmado con la clave PRIVADA
//     (%USERPROFILE%\.tauri\merchosting.key o TAURI_SIGNING_PRIVATE_KEY). La clave privada
//     nunca se copia al proyecto.
//  6. Genera latest.json (lo que lee el actualizador) y SHA256SUMS.txt.
//  7. Crea la release v0.2.0 en GitHub con `gh` (tu sesión de GitHub CLI, sin tokens en archivos).

import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, readSync, readdirSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

// ---------- Funciones puras (probadas en publicar-version.test.mjs) ----------

/** Nombre fijo del instalador en cada release (para el botón de descarga de la web). */
export const STABLE_NAME = "MercHosting-setup.exe";

const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

export function parseVersion(v) {
  const m = SEMVER.exec(v ?? "");
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null;
}

export function isNewer(next, current) {
  const a = parseVersion(next);
  const b = parseVersion(current);
  if (!a || !b) return false;
  for (let i = 0; i < 3; i++) if (a[i] !== b[i]) return a[i] > b[i];
  return false;
}

/** Valida release.json. Devuelve la lista de problemas (vacía si está bien). */
export function checkRelease(r) {
  const problems = [];
  if (!r || typeof r !== "object") return ["release.json no es un objeto JSON."];
  if (!/^[A-Za-z0-9-]{1,39}\/[A-Za-z0-9._-]{1,100}$/.test(r.releases_repo ?? ""))
    problems.push("«releases_repo» debe ser «usuario/repositorio» (el repositorio PÚBLICO de releases).");
  const key = (r.updater_pubkey ?? "").trim();
  if (!key) problems.push("«updater_pubkey» está vacío: pega el contenido de merchosting.key.pub.");
  else if (!/^[A-Za-z0-9+/=]+$/.test(key)) problems.push("«updater_pubkey» no parece la clave pública (una sola línea en base64).");
  else if (/PRIVATE|SECRET/i.test(Buffer.from(key, "base64").toString("utf8")))
    problems.push("¡Eso es la clave PRIVADA! En release.json va solo la pública (merchosting.key.pub).");
  return problems;
}

/** Notas de una versión en CHANGELOG.md: lo que hay bajo «## <versión>» hasta el siguiente «## ». */
export function changelogSection(md, version) {
  const lines = md.split(/\r?\n/);
  const start = lines.findIndex((l) => new RegExp(`^##\\s+\\[?v?${version.replace(/\./g, "\\.")}\\]?(\\s|$)`).test(l));
  if (start < 0) return null;
  const out = [];
  for (const l of lines.slice(start + 1)) {
    if (/^##\s/.test(l)) break;
    out.push(l);
  }
  const text = out.join("\n").trim();
  return text || null;
}

/**
 * ¿Trae la compilación la clave de CurseForge? Nunca devuelve la clave: solo si está y si parece
 * válida (sin espacios y de largo razonable, como la revisa el supervisor).
 */
export function curseforgeKeyStatus(value) {
  const k = (value ?? "").trim();
  if (!k) return "falta";
  if (k.length >= 200 || /\s/.test(k)) return "rara";
  return "ok";
}

export function bumpJsonVersion(text, version) {
  const j = JSON.parse(text);
  j.version = version;
  return JSON.stringify(j, null, 2) + "\n";
}

export function bumpCargoVersion(text, version) {
  const re = /(\[workspace\.package\][^[]*?\nversion\s*=\s*")[^"]*(")/;
  if (!re.test(text)) throw new Error("No se encontró version en [workspace.package] de Cargo.toml");
  return text.replace(re, `$1${version}$2`);
}

/** Configuración extra para `tauri build --config`: instalador NSIS y actualizador. */
export function releaseTauriConfig(r) {
  return {
    bundle: {
      active: true,
      targets: ["nsis"],
      createUpdaterArtifacts: true,
      externalBin: ["binaries/merc-supervisord"],
      // El instalador (setup.exe) con el mismo cubo que la app.
      windows: { nsis: { installMode: "currentUser", installerIcon: "icons/icon.ico" } },
    },
    plugins: {
      updater: {
        pubkey: r.updater_pubkey.trim(),
        endpoints: [`https://github.com/${r.releases_repo}/releases/latest/download/latest.json`],
        // La CLI de Tauri 2.12 todavía no escribe la versión en la firma: si se exigiera, el
        // actualizador rechazaría todas las versiones. La firma sí se comprueba siempre.
        requireSignedVersion: false,
        windows: { installMode: "passive" },
      },
    },
  };
}

/** Lo mismo para Linux (lo usa scripts/publicar-linux.mjs en GitHub Actions): .deb y AppImage.
 * Solo el AppImage se actualiza solo (es lo que reemplaza el actualizador de Tauri en Linux). */
export function releaseTauriConfigLinux(r) {
  return {
    bundle: {
      active: true,
      targets: ["deb", "appimage"],
      createUpdaterArtifacts: true,
      // `merc` (terminal) va también: sirve para el servidor casero sin pantalla.
      externalBin: ["binaries/merc-supervisord", "binaries/merc"],
      publisher: "Amercson",
      category: "Game",
      shortDescription: "Tu servidor de Minecraft en tu PC, sin mensualidades",
      license: "GPL-3.0-or-later",
      linux: { deb: { section: "games" } },
    },
    plugins: {
      updater: {
        pubkey: r.updater_pubkey.trim(),
        endpoints: [`https://github.com/${r.releases_repo}/releases/latest/download/latest.json`],
        requireSignedVersion: false,
      },
    },
  };
}

/** Nombres fijos de Linux en cada release (para enlazarlos desde la web). */
export const STABLE_LINUX = {
  appimage: "MercHosting-linux-x86_64.AppImage",
  deb: "MercHosting-linux-amd64.deb",
  /** Solo terminal (merc + merc-supervisord) para un servidor casero sin escritorio. */
  headless: "MercHosting-servidor-casero-linux-x86_64.tar.gz",
};

/** Añade Linux al latest.json que ya publicó Windows, sin tocar las demás plataformas. */
export function mergeLinuxPlatform(latest, { version, signature, url }) {
  if (!latest || typeof latest !== "object" || !latest.platforms) throw new Error("latest.json no tiene «platforms»");
  if (latest.version !== version)
    throw new Error(`latest.json es de la versión ${latest.version}, no de ${version}: publica primero la de Windows`);
  return { ...latest, platforms: { ...latest.platforms, "linux-x86_64": { signature: signature.trim(), url } } };
}

export function downloadUrl(repo, version, file) {
  return `https://github.com/${repo}/releases/download/v${version}/${encodeURIComponent(file)}`;
}

export function latestJson({ version, notes, pubDate, url, signature }) {
  return {
    version,
    notes,
    pub_date: pubDate,
    platforms: { "windows-x86_64": { signature: signature.trim(), url } },
  };
}

export function sha256Line(name, bytes) {
  return `${createHash("sha256").update(bytes).digest("hex")}  ${name}`;
}

// ---------- Ejecución ----------

function fail(msg) {
  console.error(`\n✗ ${msg}\n`);
  process.exit(1);
}

/** Argumento para la línea de comandos de Windows (comillas si tiene espacios). */
export function quoteArg(a) {
  return /[\s"&|<>^]/.test(a) ? `"${a.replace(/"/g, '\\"')}"` : a;
}

function run(cmd, args, opts = {}) {
  const shown = args.map(quoteArg).join(" ");
  console.log(`\n> ${cmd} ${shown}`);
  // `cargo` y `gh` son .exe: sin shell, cada argumento llega tal cual (con espacios incluidos).
  // `npx` en Windows es un .cmd y necesita shell: ahí se ponen comillas a mano.
  const viaShell = process.platform === "win32" && cmd === "npx";
  const r = viaShell
    ? spawnSync(`${cmd} ${shown}`, { stdio: "inherit", shell: true, ...opts })
    : spawnSync(cmd, args, { stdio: "inherit", shell: false, ...opts });
  if (r.error) fail(`No se pudo ejecutar ${cmd}: ${r.error.message}`);
  if (r.status !== 0) fail(`Falló: ${cmd} ${shown}`);
}

function ask(question) {
  process.stdout.write(question);
  const buf = Buffer.alloc(1024);
  let n = 0;
  try {
    n = readSync(0, buf, 0, buf.length);
  } catch {
    return "";
  }
  return buf.toString("utf8", 0, n).trim();
}

async function main() {
  const args = process.argv.slice(2);
  const version = args.find((a) => !a.startsWith("--"));
  const upload = !args.includes("--sin-subir");
  if (process.platform !== "win32") fail("El instalador de Windows se compila en Windows.");

  const releasePath = join(ROOT, "release.json");
  if (!existsSync(releasePath)) fail("Falta release.json en la raíz del proyecto (mira docs/PUBLICAR_VERSION.md).");
  const release = JSON.parse(readFileSync(releasePath, "utf8"));
  const problems = checkRelease(release);
  if (problems.length) fail(problems.join("\n  "));

  const confPath = join(ROOT, "app/src-tauri/tauri.conf.json");
  const pkgPath = join(ROOT, "app/package.json");
  const cargoPath = join(ROOT, "Cargo.toml");
  const current = JSON.parse(readFileSync(confPath, "utf8")).version;
  if (!parseVersion(version)) fail(`Indica la versión nueva, por ejemplo: node scripts/publicar-version.mjs 0.2.0 (ahora: ${current}).`);
  if (!isNewer(version, current) && version !== current)
    fail(`La versión ${version} no es más nueva que la actual (${current}).`);

  const changelog = join(ROOT, "CHANGELOG.md");
  const notes = existsSync(changelog) ? changelogSection(readFileSync(changelog, "utf8"), version) : null;
  if (!notes) fail(`Escribe primero las novedades en CHANGELOG.md bajo «## ${version}».`);

  if (upload) {
    const auth = spawnSync("gh", ["auth", "status"], { encoding: "utf8" });
    if (auth.status !== 0) fail("No hay sesión de GitHub CLI. Instala «gh» y ejecuta: gh auth login");
    const info = spawnSync("gh", ["api", `repos/${release.releases_repo}`, "--jq", ".private"], { encoding: "utf8" });
    if (info.status !== 0) fail(`No se encontró el repositorio ${release.releases_repo} en GitHub (o tu sesión de gh no lo ve).`);
    if (info.stdout.trim() === "true")
      fail(`${release.releases_repo} es privado: las apps no podrían descargar las actualizaciones. Hazlo público en Settings → General → Danger Zone.`);
    const commits = spawnSync("gh", ["api", `repos/${release.releases_repo}/commits?per_page=1`], { encoding: "utf8" });
    if (commits.status !== 0)
      fail(`${release.releases_repo} está vacío y GitHub no deja crear releases así. Crea un README desde la web (botón «Add a README» o «creating a new file») y vuelve a intentarlo.`);
  }

  // Clave privada: de la variable de entorno o del archivo de la carpeta de usuario.
  const env = { ...process.env };
  if (!env.TAURI_SIGNING_PRIVATE_KEY) {
    const keyFile = join(homedir(), ".tauri", "merchosting.key");
    if (!existsSync(keyFile)) fail(`No se encontró la clave privada en ${keyFile} (mira docs/PUBLICAR_VERSION.md).`);
    env.TAURI_SIGNING_PRIVATE_KEY = readFileSync(keyFile, "utf8");
  }
  if (env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD === undefined) {
    env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ask("Contraseña de la clave de firma (se verá al escribirla; Enter si no tiene): ");
  }

  // Clave de CurseForge (variable MERC_CURSEFORGE_API_KEY): queda dentro del supervisor al
  // compilar. Nunca se escribe en el repositorio ni se muestra.
  const cf = curseforgeKeyStatus(env.MERC_CURSEFORGE_API_KEY);
  if (cf === "ok") {
    console.log("CurseForge: clave encontrada, esta versión trae CurseForge.");
  } else {
    console.log(
      cf === "rara"
        ? "CurseForge: la clave de MERC_CURSEFORGE_API_KEY tiene espacios o es demasiado larga; la app la ignoraría."
        : "CurseForge: no se encontró MERC_CURSEFORGE_API_KEY (si la guardaste con setx, cierra y abre de nuevo la ventana).",
    );
    if (!/^[sS]/.test(ask("¿Seguir sin CurseForge? (S/N): "))) fail("Publicación cancelada.");
    delete env.MERC_CURSEFORGE_API_KEY;
  }

  console.log(`\nPublicando MercHosting ${version} (antes ${current}) en ${release.releases_repo}`);
  writeFileSync(confPath, bumpJsonVersion(readFileSync(confPath, "utf8"), version));
  writeFileSync(pkgPath, bumpJsonVersion(readFileSync(pkgPath, "utf8"), version));
  writeFileSync(cargoPath, bumpCargoVersion(readFileSync(cargoPath, "utf8"), version));

  // Carpeta de compilación propia: no choca con la app de desarrollo (abrir-app.ps1), que puede
  // estar abierta y bloquear sus .exe en target/release.
  const target = join(ROOT, "target", "publicar");
  env.CARGO_TARGET_DIR = target;
  run("cargo", ["build", "--release", "-p", "merc-supervisord"], { cwd: ROOT, env });
  const binDir = join(ROOT, "app/src-tauri/binaries");
  mkdirSync(binDir, { recursive: true });
  copyFileSync(join(target, "release", "merc-supervisord.exe"), join(binDir, "merc-supervisord-x86_64-pc-windows-msvc.exe"));

  const extra = join(ROOT, "target/tauri.release.conf.json");
  writeFileSync(extra, JSON.stringify(releaseTauriConfig(release), null, 2));
  run("npx", ["tauri", "build", "--features", "updater", "--bundles", "nsis", "--config", extra], {
    cwd: join(ROOT, "app"),
    env,
  });

  let nsis = join(target, "release", "bundle", "nsis");
  if (!existsSync(nsis)) nsis = join(ROOT, "target", "release", "bundle", "nsis");
  const setup = readdirSync(nsis).find((f) => f.endsWith(".exe") && f.includes(version));
  if (!setup || !existsSync(join(nsis, `${setup}.sig`))) fail(`No se encontró el instalador firmado de ${version} en ${nsis}.`);
  const out = join(ROOT, "target/release-publicar", version);
  mkdirSync(out, { recursive: true });
  copyFileSync(join(nsis, setup), join(out, setup));
  // Copia con nombre fijo: la web enlaza siempre a
  // https://github.com/<repo>/releases/latest/download/MercHosting-setup.exe
  copyFileSync(join(nsis, setup), join(out, STABLE_NAME));
  const latest = latestJson({
    version,
    notes,
    pubDate: new Date().toISOString(),
    url: downloadUrl(release.releases_repo, version, setup),
    signature: readFileSync(join(nsis, `${setup}.sig`), "utf8"),
  });
  writeFileSync(join(out, "latest.json"), JSON.stringify(latest, null, 2));
  const setupBytes = readFileSync(join(out, setup));
  writeFileSync(join(out, "SHA256SUMS.txt"), [sha256Line(setup, setupBytes), sha256Line(STABLE_NAME, setupBytes)].join("\n") + "\n");
  writeFileSync(join(out, "notas.md"), notes + "\n");
  console.log(`\n✓ Listo en ${out}: ${setup}, latest.json, SHA256SUMS.txt`);

  if (!upload) {
    console.log("No se subió nada (--sin-subir).");
    return;
  }
  run("gh", [
    "release", "create", `v${version}`,
    join(out, setup), join(out, STABLE_NAME), join(out, "latest.json"), join(out, "SHA256SUMS.txt"),
    "--repo", release.releases_repo,
    "--title", `MercHosting ${version}`,
    "--notes-file", join(out, "notas.md"),
  ]);
  console.log(`\n✓ Publicada: https://github.com/${release.releases_repo}/releases/tag/v${version}`);
  console.log("Las apps instaladas la verán al buscar actualizaciones.");
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((e) => fail(e.message ?? String(e)));
}
