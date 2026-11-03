// node --test scripts/android-firma.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { patchGradle, keystoreProperties, patchBuildSrcSettings } from "./android-firma.mjs";

// Forma de la plantilla de «tauri android init» (Tauri 2).
const TEMPLATE = `import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

android {
    compileSdk = 34
    namespace = "app.merchosting.movil"
    defaultConfig {
        applicationId = "app.merchosting.movil"
    }
    buildTypes {
        getByName("debug") {
            isDebuggable = true
        }
        getByName("release") {
            isMinifyEnabled = true
        }
    }
}
`;

test("añade la firma de release a la plantilla de Tauri", () => {
  const out = patchGradle(TEMPLATE);
  assert.match(out, /^import java\.io\.FileInputStream/m);
  assert.equal(out.match(/import java\.util\.Properties/g).length, 1);
  assert.ok(out.indexOf("signingConfigs {") < out.indexOf("buildTypes {"));
  assert.match(out, /getByName\("release"\) \{\n\s+signingConfig = signingConfigs\.getByName\("release"\)\n\s+isMinifyEnabled/);
  // El tipo debug no se toca.
  assert.match(out, /getByName\("debug"\) \{\n\s+isDebuggable/);
});

test("es idempotente", () => {
  const once = patchGradle(TEMPLATE);
  assert.equal(patchGradle(once), once);
});

test("falla si la plantilla cambió", () => {
  assert.throws(() => patchGradle("android {\n}\n"), /plantilla de Tauri cambió/);
  assert.throws(() => patchGradle("android {\n    buildTypes {\n    }\n}\n"), /getByName/);
});

test("keystore.properties: valores obligatorios, sin saltos de línea y con «\\» escapada", () => {
  assert.equal(
    keystoreProperties({ alias: "upload", password: "a\\b", storeFile: "/tmp/k.jks" }),
    "password=a\\\\b\nkeyAlias=upload\nstoreFile=/tmp/k.jks\n",
  );
  assert.throws(() => keystoreProperties({ alias: "upload", password: "", storeFile: "/k" }), /Falta password/);
  assert.throws(() => keystoreProperties({ alias: "upload\nx=1", password: "p", storeFile: "/k" }), /saltos de línea/);
});

test("buildSrc: Maven Central y Google de respaldo, al principio e idempotente", () => {
  const nuevo = patchBuildSrcSettings(null);
  assert.match(nuevo, /pluginManagement \{\n    repositories \{\n        gradlePluginPortal\(\)\n        mavenCentral\(\)\n        google\(\)/);
  const conTexto = patchBuildSrcSettings('rootProject.name = "buildSrc"\n');
  assert.ok(conTexto.indexOf("pluginManagement") < conTexto.indexOf("rootProject.name"), "pluginManagement va primero");
  assert.equal(patchBuildSrcSettings(conTexto), conTexto);
  assert.throws(() => patchBuildSrcSettings("pluginManagement {\n  repositories { gradlePluginPortal() }\n}\n"), /plantilla de Tauri cambió/);
});
