// Prepara la firma del APK de Android en el proyecto que genera «tauri android init»
// (app/src-tauri/gen/android, que no se guarda en el repositorio).
//
// Sigue la guía oficial de Tauri (https://v2.tauri.app/distribute/sign/android/):
//  1. Añade a app/build.gradle.kts el bloque signingConfigs «release» que lee keystore.properties
//     y lo usa en el tipo de compilación «release».
//  2. Escribe keystore.properties con el alias, la contraseña y la ruta del almacén de claves.
//
// Las contraseñas llegan SOLO por variables de entorno (secretos de GitHub), nunca por argumentos,
// y keystore.properties queda dentro de gen/ (ignorado por git).
//
// Uso (lo hace .github/workflows/android.yml):
//   ANDROID_KEY_ALIAS=upload ANDROID_KEY_PASSWORD=… ANDROID_KEYSTORE_PATH=/ruta/keystore.jks \
//     node scripts/android-firma.mjs app/src-tauri/gen/android
//
// Si la plantilla de Tauri cambia y no encuentra dónde añadir la firma, FALLA con un mensaje claro
// (mejor que publicar un APK sin firmar o firmado con la clave de depuración).
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const MARK = "// MercHosting: firma del APK (scripts/android-firma.mjs)";

const SIGNING = `    ${MARK}
    signingConfigs {
        create("release") {
            val keystorePropertiesFile = rootProject.file("keystore.properties")
            val keystoreProperties = Properties()
            if (keystorePropertiesFile.exists()) {
                keystoreProperties.load(FileInputStream(keystorePropertiesFile))
            }

            keyAlias = keystoreProperties["keyAlias"] as String
            keyPassword = keystoreProperties["password"] as String
            storeFile = file(keystoreProperties["storeFile"] as String)
            storePassword = keystoreProperties["password"] as String
        }
    }
`;

/** Devuelve build.gradle.kts con la firma añadida. Lanza un error si la plantilla no encaja. */
export function patchGradle(text) {
  if (text.includes(MARK)) return text; // ya preparado
  let out = text;
  for (const imp of ["import java.io.FileInputStream", "import java.util.Properties"]) {
    if (!out.includes(imp)) out = `${imp}\n${out}`;
  }
  const buildTypes = /^(\s*)buildTypes\s*\{/m;
  if (!/^\s*android\s*\{/m.test(out) || !buildTypes.test(out)) {
    throw new Error("No encontré «android { … buildTypes {» en build.gradle.kts: la plantilla de Tauri cambió.");
  }
  out = out.replace(buildTypes, (m) => `${SIGNING}${m}`);
  const release = /getByName\("release"\)\s*\{\n/;
  if (!release.test(out)) {
    throw new Error("No encontré «getByName(\"release\") {» en build.gradle.kts: la plantilla de Tauri cambió.");
  }
  out = out.replace(release, (m) => `${m}            signingConfig = signingConfigs.getByName("release")\n`);
  return out;
}

const REPOS_MARK = "// MercHosting: repositorios de respaldo (scripts/android-firma.mjs)";

/**
 * settings.gradle.kts de buildSrc con Maven Central y Google además del portal de plugins.
 * Sin esto, los plugins de buildSrc (kotlin-dsl → Kotlin) se buscan SOLO en plugins.gradle.org,
 * y si el portal no tiene una versión recién publicada la compilación falla («Could not find
 * org.jetbrains.kotlin:kotlin-stdlib:…»). `pluginManagement` tiene que ir al principio.
 */
export function patchBuildSrcSettings(text) {
  const current = text ?? "";
  if (current.includes(REPOS_MARK)) return current;
  if (/^\s*pluginManagement\s*\{/m.test(current)) {
    throw new Error("buildSrc/settings.gradle.kts ya trae pluginManagement: la plantilla de Tauri cambió, revísalo a mano.");
  }
  const block = `${REPOS_MARK}
pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
        google()
    }
}
`;
  return current ? `${block}\n${current}` : block;
}

/** Texto de keystore.properties. Rechaza valores con saltos de línea (inyectarían otras claves). */
export function keystoreProperties({ alias, password, storeFile }) {
  for (const [name, v] of Object.entries({ alias, password, storeFile })) {
    if (!v) throw new Error(`Falta ${name} para firmar el APK (mira docs/MOVIL.md).`);
    if (/[\r\n]/.test(v)) throw new Error(`${name} no puede tener saltos de línea.`);
  }
  // En .properties «\» es carácter de escape: se duplica para que la ruta y la contraseña se lean tal cual.
  const esc = (v) => v.replace(/\\/g, "\\\\");
  return `password=${esc(password)}\nkeyAlias=${esc(alias)}\nstoreFile=${esc(storeFile)}\n`;
}

function main() {
  const dir = process.argv[2];
  if (!dir) throw new Error("Uso: node scripts/android-firma.mjs app/src-tauri/gen/android");
  const gradle = join(dir, "app", "build.gradle.kts");
  if (!existsSync(gradle)) throw new Error(`No existe ${gradle}: ejecuta antes «npx tauri android init».`);
  writeFileSync(gradle, patchGradle(readFileSync(gradle, "utf8")));
  const buildSrc = join(dir, "buildSrc");
  if (existsSync(buildSrc)) {
    const settings = join(buildSrc, "settings.gradle.kts");
    writeFileSync(settings, patchBuildSrcSettings(existsSync(settings) ? readFileSync(settings, "utf8") : null));
    console.log("✓ Repositorios de respaldo para buildSrc (Maven Central y Google).");
  }
  writeFileSync(
    join(dir, "keystore.properties"),
    keystoreProperties({
      alias: process.env.ANDROID_KEY_ALIAS,
      password: process.env.ANDROID_KEY_PASSWORD,
      storeFile: process.env.ANDROID_KEYSTORE_PATH,
    }),
    { mode: 0o600 },
  );
  console.log("✓ Firma del APK preparada.");
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    main();
  } catch (e) {
    console.error(`✗ ${e.message}`);
    process.exit(1);
  }
}
