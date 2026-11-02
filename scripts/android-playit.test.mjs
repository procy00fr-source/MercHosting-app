// node --test scripts/android-playit.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { isElfAarch64, patchGradle } from "./android-playit.mjs";

function elf(machine, cls = 2) {
  const b = Buffer.alloc(64);
  b.set([0x7f, 0x45, 0x4c, 0x46, cls, 1], 0);
  b[18] = machine & 0xff;
  b[19] = machine >> 8;
  return b;
}

test("solo acepta ELF de 64 bits para aarch64", () => {
  assert.equal(isElfAarch64(elf(183)), true);
  assert.equal(isElfAarch64(elf(62)), false, "x86_64");
  assert.equal(isElfAarch64(elf(40, 1)), false, "arm de 32 bits");
  assert.equal(isElfAarch64(Buffer.from("#!/bin/sh\n")), false);
});

test("añade el empaquetado clásico una sola vez", () => {
  const t = 'plugins {\n    id("com.android.application")\n}\n\nandroid {\n    compileSdk = 36\n}\n';
  const once = patchGradle(t);
  assert.match(once, /useLegacyPackaging = true/);
  assert.ok(once.startsWith(t.trimEnd()));
  assert.equal(patchGradle(once), once);
  assert.throws(() => patchGradle("plugins {}\n"), /plantilla/);
});
