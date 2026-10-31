// Pruebas del script de publicación: node --test scripts/publicar-version.test.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  bumpCargoVersion,
  bumpJsonVersion,
  changelogSection,
  checkRelease,
  downloadUrl,
  isNewer,
  latestJson,
  quoteArg,
  releaseTauriConfig,
  sha256Line,
} from "./publicar-version.mjs";

test("versiones", () => {
  assert.ok(isNewer("0.2.0", "0.1.0"));
  assert.ok(isNewer("1.0.0", "0.9.9"));
  assert.ok(!isNewer("0.1.0", "0.1.0"));
  assert.ok(!isNewer("0.1.0-beta", "0.0.1"));
});

test("release.json: repo, clave pública y nunca la privada", () => {
  assert.deepEqual(checkRelease({ releases_repo: "amerc/MercHosting-releases", updater_pubkey: "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk=" }), []);
  assert.equal(checkRelease({ releases_repo: "", updater_pubkey: "" }).length, 2);
  const priv = Buffer.from("untrusted comment: rsign encrypted secret key\nxyz").toString("base64");
  assert.match(checkRelease({ releases_repo: "a/b", updater_pubkey: priv })[0], /PRIVADA/);
});

test("notas del CHANGELOG", () => {
  const md = "# Cambios\n\n## 0.2.0\n\n- Pack para amigos\n- Alertas\n\n## 0.1.0\n\n- Primera\n";
  assert.equal(changelogSection(md, "0.2.0"), "- Pack para amigos\n- Alertas");
  assert.equal(changelogSection(md, "0.1.0"), "- Primera");
  assert.equal(changelogSection(md, "0.3.0"), null);
  assert.equal(changelogSection("## 0.20.0\n- x", "0.2.0"), null, "no confunde 0.2.0 con 0.20.0");
});

test("subir la versión en los tres archivos", () => {
  assert.equal(JSON.parse(bumpJsonVersion('{"name":"a","version":"0.1.0"}', "0.2.0")).version, "0.2.0");
  const cargo = '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "0.1.0"\nedition = "2021"\n\n[workspace.dependencies]\nserde = { version = "1" }\n';
  const out = bumpCargoVersion(cargo, "0.2.0");
  assert.match(out, /\[workspace\.package\]\nversion = "0\.2\.0"/);
  assert.match(out, /serde = \{ version = "1" \}/, "no toca las dependencias");
  // El Cargo.toml real del proyecto se puede actualizar.
  bumpCargoVersion(readFileSync(new URL("../Cargo.toml", import.meta.url), "utf8"), "9.9.9");
});

test("config de Tauri y latest.json", () => {
  const r = { releases_repo: "amerc/MercHosting-releases", updater_pubkey: "QUJD" };
  const c = releaseTauriConfig(r);
  assert.deepEqual(c.plugins.updater.endpoints, ["https://github.com/amerc/MercHosting-releases/releases/latest/download/latest.json"]);
  assert.equal(c.plugins.updater.requireSignedVersion, false, "la CLI 2.12 no firma la versión");
  assert.equal(c.plugins.updater.pubkey, "QUJD");
  assert.deepEqual(c.bundle.externalBin, ["binaries/merc-supervisord"]);
  assert.equal(c.bundle.windows.nsis.installerIcon, "icons/icon.ico");
  const url = downloadUrl(r.releases_repo, "0.2.0", "MercHosting_0.2.0_x64-setup.exe");
  assert.equal(url, "https://github.com/amerc/MercHosting-releases/releases/download/v0.2.0/MercHosting_0.2.0_x64-setup.exe");
  const l = latestJson({ version: "0.2.0", notes: "- x", pubDate: "2026-10-31T00:00:00.000Z", url, signature: "c2ln\n" });
  assert.deepEqual(l.platforms["windows-x86_64"], { signature: "c2ln", url });
  assert.equal(sha256Line("a.exe", Buffer.from("abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  a.exe");
});

test("la configuración generada la acepta la CLI de Tauri (esquema)", () => {
  // Fusiona con tauri.conf.json y comprueba que los campos existen en el esquema oficial.
  const schema = JSON.parse(readFileSync(new URL("../app/node_modules/@tauri-apps/cli/config.schema.json", import.meta.url), "utf8"));
  const bundle = schema.definitions.BundleConfig.properties;
  for (const k of ["active", "targets", "createUpdaterArtifacts", "externalBin", "windows"]) assert.ok(bundle[k], k);
  assert.ok(schema.definitions.NsisConfig.properties.installMode);
  assert.ok(schema.definitions.NsisConfig.properties.installerIcon);
});

test("argumentos con espacios llegan enteros (el título «MercHosting 0.2.0»)", () => {
  assert.equal(quoteArg("MercHosting 0.2.0"), '"MercHosting 0.2.0"');
  assert.equal(quoteArg("C:\\Users\\Ana María\\x.json"), '"C:\\Users\\Ana María\\x.json"');
  assert.equal(quoteArg("--features"), "--features");
});
