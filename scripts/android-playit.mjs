// Mete el agente de Playit (compilado para Android en el workflow desde el código oficial
// v1.2.0) dentro del APK, y prepara el proyecto para que Android lo deje ejecutar.
//
// - En Android una app NO puede ejecutar archivos de su carpeta de datos (W^X, Android 10+).
//   Solo puede ejecutar lo que viene en la carpeta de bibliotecas nativas del APK, así que el
//   agente va como `libplayit.so` en jniLibs/arm64-v8a del plugin tauri-plugin-mercphone.
// - Para que esa carpeta exista de verdad en el celular, el APK tiene que extraer las
//   bibliotecas al instalarse: `packaging { jniLibs { useLegacyPackaging = true } }`.
//
// Uso (lo hace .github/workflows/android.yml):
//   node scripts/android-playit.mjs <playitd compilado> app/src-tauri/gen/android
//
// Comprueba que el archivo es un ejecutable ELF de 64 bits para ARM (aarch64) antes de copiarlo.
import { readFileSync, writeFileSync, existsSync, mkdirSync, copyFileSync, chmodSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const MARK = "// MercHosting: bibliotecas nativas extraídas (scripts/android-playit.mjs)";
const PACKAGING = `
${MARK}
android {
    packaging {
        jniLibs {
            useLegacyPackaging = true
        }
    }
}
`;

/** ¿Es un ELF de 64 bits, little endian, para aarch64 (e_machine 183)? */
export function isElfAarch64(buf) {
  if (buf.length < 64) return false;
  const magic = buf[0] === 0x7f && buf[1] === 0x45 && buf[2] === 0x4c && buf[3] === 0x46;
  const is64 = buf[4] === 2;
  const little = buf[5] === 1;
  const machine = buf[18] | (buf[19] << 8);
  return magic && is64 && little && machine === 183;
}

/** build.gradle.kts de la app con el empaquetado clásico de bibliotecas (idempotente). */
export function patchGradle(text) {
  if (text.includes(MARK)) return text;
  if (!/^\s*android\s*\{/m.test(text)) {
    throw new Error("No encontré «android {» en build.gradle.kts: la plantilla de Tauri cambió.");
  }
  return `${text.replace(/\s*$/, "\n")}${PACKAGING}`;
}

export const PLUGIN_JNI = join("crates", "tauri-plugin-mercphone", "android", "src", "main", "jniLibs", "arm64-v8a");

function main() {
  const [agent, dir] = process.argv.slice(2);
  if (!agent || !dir) throw new Error("Uso: node scripts/android-playit.mjs <playitd> app/src-tauri/gen/android");
  const buf = readFileSync(agent);
  if (!isElfAarch64(buf) || buf.length < 1_000_000) {
    throw new Error(`${agent} no es un ejecutable de Android arm64 (aarch64).`);
  }
  const gradle = join(dir, "app", "build.gradle.kts");
  if (!existsSync(gradle)) throw new Error(`No existe ${gradle}: ejecuta antes «npx tauri android init».`);
  writeFileSync(gradle, patchGradle(readFileSync(gradle, "utf8")));
  const root = join(dirname(fileURLToPath(import.meta.url)), "..");
  const dest = join(root, PLUGIN_JNI);
  mkdirSync(dest, { recursive: true });
  copyFileSync(agent, join(dest, "libplayit.so"));
  chmodSync(join(dest, "libplayit.so"), 0o755);
  console.log(`✓ Agente de Playit dentro del APK (${buf.length} bytes) y bibliotecas extraídas al instalar.`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    main();
  } catch (e) {
    console.error(`✗ ${e.message}`);
    process.exit(1);
  }
}
