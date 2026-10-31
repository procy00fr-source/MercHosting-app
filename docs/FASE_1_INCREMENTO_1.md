# Fase 1, incremento 1: núcleo, datos y supervisor

Fecha: 30-10-2026 · Estado: **completado con limitaciones documentadas**

## 0. Auditoría de partida (Fase 0)

La carpeta del proyecto estaba vacía: no había código, `.git`, dependencias ni web. Por eso no había nada que
conservar ni que migrar. Se adopta la arquitectura preferida de la directiva maestra (Rust + SQLite ahora;
Tauri 2 + React/TypeScript en los siguientes incrementos). El primer commit es el punto de recuperación.

## 1. Qué se construyó

| Módulo | Archivo | Qué hace |
|---|---|---|
| Estados | `merc-core/src/state.rs` | Los 10 estados de la directiva y la tabla de transiciones permitidas. ONLINE solo se alcanza desde un estado con proceso. |
| Rutas confinadas | `merc-core/src/paths.rs` | `SandboxRoot::resolve`: rechaza `..`, rutas absolutas/UNC/unidad, nombres reservados de Windows, flujos alternos `:`, symlinks/junctions y cualquier escape detectado con `canonicalize`. |
| Readiness | `merc-core/src/readiness.rs` | Reconoce `]: Done (…)! For help` (Vanilla/Paper/Fabric/NeoForge), puerto ocupado, EULA y fase de carga. Un mensaje de chat no puede simular readiness. |
| Detección | `merc-core/src/detect.rs` | Identifica NeoForge/Forge (leyendo `run.bat`/`run.sh`), Fabric, Paper, Purpur, Spigot y Vanilla sin modificar la carpeta. Cada conclusión trae evidencia y nivel de confianza (Verificado / Inferido / Desconocido). |
| Perfil de arranque | `merc-core/src/launch.rs` | Construye los argumentos de Java sin shell: `-jar` o `@win_args.txt` de NeoForge, la memoria de la app tiene prioridad y se fuerza UTF-8 en la salida. Bloquea argumentos que cambien qué se ejecuta (`-jar`, `@`, `-cp`…). |
| Java | `merc-core/src/java.rs` | Tabla de Java requerido por versión de Minecraft y lectura real de `java -version`, sin ventana y con límite de tiempo. |
| Propiedades / EULA | `merc-core/src/properties.rs` | Solo lectura de `server.properties` y `eula.txt`. **Nunca acepta la EULA por el usuario.** |
| Persistencia | `merc-store/src/lib.rs` | SQLite (WAL) con migraciones transaccionales, rechazo de esquemas más nuevos, instancias con id inmutable, carpeta única por instancia y auditoría. Quitar una instancia del registro no borra archivos. |
| Supervisor | `merc-supervisor/src/lib.rs` | Arranque validado (EULA, archivos, Java, puerto, carpeta), Java lanzado directamente sin CMD, lectura de stdout/stderr, readiness, parada limpia con `stop`, timeout sin matar, cierre forzado solo con confirmación explícita, detección de crash con código de salida, reinicio secuencial, comandos validados y búfer de consola con reconexión por número de secuencia. |
| Logs | `merc-supervisor/src/log.rs` | `console.log` rotativo (10 MB × 5) en la carpeta de datos de MercHosting, nunca dentro del servidor. |

## 2. Pruebas ejecutadas (resultados reales)

Entorno: Linux x86_64 (contenedor de desarrollo), Rust 1.97.0, OpenJDK 21.0.12.
Comando: `cargo test --workspace`. Resultado: **45 pasan, 0 fallan**, repetido 3 veces seguidas sin fallos
intermitentes. `cargo clippy --workspace --all-targets`: 0 avisos.

- `merc-core`: 28 pruebas unitarias (rutas, traversal, symlinks, nombres de Windows, detección, mapeo de versiones de NeoForge, tabla de Java, argumentos y readiness).
- `merc-store`: 4 pruebas (ida y vuelta, carpeta duplicada, quitar registro, reapertura y esquema más nuevo).
- `merc-supervisor`: 1 prueba unitaria (rotación) y 12 de integración con **un proceso Java real** (`tests/fixtures/FakeServer.java`):

| Caso | Resultado |
|---|---|
| Arranque → ONLINE solo tras "Done"; el puerto se abre de verdad; comando con tildes; stop limpio (código 0) | PASS |
| Doble arranque y 8 arranques simultáneos: solo uno gana | PASS |
| Crash durante la carga: CRASHED, código 1, stderr capturado; se puede reiniciar después | PASS |
| EULA ausente: ERROR sin crear proceso; `eula.txt` no se crea | PASS |
| Puerto ocupado por otro programa: error claro, no se mata nada | PASS |
| Dos instancias con la misma carpeta o el mismo puerto: rechazadas | PASS |
| Readiness no llega: UNKNOWN (no se mata); cierre forzado con confirmación → STOPPED | PASS |
| El servidor ignora `stop`: StopTimeout, sigue en STOPPING, no se mata | PASS |
| Comando con salto de línea (inyección de un segundo comando): rechazado | PASS |
| JAR inexistente / Java inexistente: errores claros | PASS |
| Reinicio secuencial con PID nuevo | PASS |

**Importante:** `FakeServer` imita el formato de log, el puerto y la respuesta a `stop` de Minecraft,
pero **no es Minecraft**. Desde este entorno no se puede descargar el servidor de Mojang ni NeoForge.
La prueba con un servidor real está pendiente (ver sección 4).

## 3. Inventario

**HECHO (probado)**
- Máquina de estados, confinamiento de rutas, detección de loader, perfiles de arranque, requisitos de Java.
- Persistencia SQLite con migraciones y auditoría.
- Ciclo de vida completo del proceso Java en el supervisor como biblioteca.

**PARCIAL**
- Específico de Windows: el arranque sin ventana (`CREATE_NO_WINDOW`) y la comparación de carpetas sin distinguir mayúsculas están escritos, pero **no se compilaron ni se ejecutaron en Windows**. Este entorno no tiene el target de Windows y no puede descargarlo.
- Detección de Vanilla: si el JAR se llama `server.jar`, la versión de Minecraft no se lee todavía (haría falta abrir el `version.json` del JAR). Se pide al usuario.

**PENDIENTE (próximos incrementos de la Fase 1)**
1. `merc-supervisor-bin`: el supervisor como proceso persistente e independiente de la ventana, con IPC por named pipe (ACL limitada al usuario, protocolo versionado y autenticado) y reconciliación al reiniciar el supervisor por identidad de proceso (PID + hora de creación), no solo por PID.
2. Job Objects en Windows, diseñados para que cerrar la UI no mate los servidores.
3. Shell de Tauri 2 + React con la primera pantalla real: importar una carpeta y ver detección, estado y consola.
4. Política anti-bucle de crashes (reintentos limitados con espera creciente).
5. Comprobación de RAM total y disco libre antes de arrancar.

**BLOQUEADO (necesita tu PC o tu decisión)**
- Prueba con tu servidor NeoForge 1.21.1 real y Java 21: hace falta ejecutar en Windows.

## 4. Cómo reproducir en Windows

1. Instala Rust (https://rustup.rs) y un JDK 21 (por ejemplo, Eclipse Temurin).
2. Ejecuta `scripts\verificar-windows.ps1` con clic derecho → «Ejecutar con PowerShell».
3. Se genera `verificacion-windows.log` con las versiones y el resultado de las 45 pruebas.

## 4b. Historial de verificación en Windows

- 30-10-2026, intento 1: Windows con Rust 1.99.1 y Temurin 21.0.13. **Las pruebas no llegaron a ejecutarse**
  porque el script tenía un error: en Windows PowerShell 5.1, `$ErrorActionPreference = "Stop"` junto con
  `2>&1` convierte la salida normal de cargo en stderr en un error fatal. Script corregido (usa `cmd /c`).
- 30-10-2026, intento 2: Windows 11 (NT 10.0.26200), PowerShell 5.1, Rust 1.99.1 y Temurin 21.0.13.
  **43 pruebas pasan y 0 fallan.** Son 43 y no 45 porque las 2 pruebas de symlinks solo existen en Unix;
  en el incremento 2 se añade una prueba equivalente con junctions de Windows.

## 5. Riesgos y límites conocidos

- **TOCTOU en rutas:** entre validar y usar una ruta, otro proceso local podría cambiar una carpeta por un enlace. Las escrituras futuras deberán revalidar justo antes de abrir.
- **Puerto:** la comprobación previa es orientativa (hay una ventana de carrera); si otro programa toma el puerto, el log del servidor lo detecta y se muestra el aviso.
- **Codificación:** se fuerza UTF-8 en stdout/stderr de Java. En Windows, rutas con caracteres fuera de la página de códigos del sistema (por ejemplo, chino en un Windows en español) pueden fallar en Java; hay que probarlo.
- **Supervisor en proceso:** mientras no exista `merc-supervisor-bin`, si se cerrara el proceso que contiene el supervisor, los pipes se perderían. Esto se resuelve en el siguiente incremento, no se oculta.
- **JAR de terceros:** MercHosting ejecuta código que el usuario elige; no hay protección absoluta frente a un JAR malicioso.
