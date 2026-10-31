# Fase 1, incremento 2: supervisor persistente y canal local seguro

Fecha: 30-10-2026 · Estado: **completado en Linux; pendiente de verificación en Windows**

## 1. Qué se construyó

| Pieza | Dónde | Qué hace |
|---|---|---|
| Protocolo v1 | `merc-ipc/src/protocol.rs` | Mensajes tipados y versionados: saludo con token, petición/respuesta con número de secuencia y errores con código (`auth_failed`, `version_mismatch`, `conflict`, `no_console`…). Una línea JSON por mensaje, máximo 1 MiB. |
| Canal local | `merc-ipc/src/transport.rs` | Windows: named pipe que rechaza clientes remotos, con ACL limitada a SYSTEM y al propietario. Linux: socket Unix con permisos 0600. El nombre se deriva de la carpeta de datos, así que una prueba nunca choca con la instalación real. **No se abre ningún puerto de red.** |
| Autenticación | `merc-ipc/src/lib.rs` | Token aleatorio de 256 bits, nuevo en cada arranque del supervisor y guardado en la carpeta de datos privada del usuario. Se compara en tiempo constante. Los rechazos quedan en el registro. |
| Supervisor persistente | `merc-supervisord` | Proceso aparte sin ventana, uno por carpeta de datos (un segundo intento detecta al primero y sale). Registra instancias en SQLite, audita cada acción con el nombre del cliente y no se cierra si hay servidores en marcha. |
| Recuperación tras caída | `merc-supervisor/src/runtime.rs` | Cada proceso Java lanzado se anota en disco con su PID y su hora de inicio según el sistema. Si el supervisor muere y vuelve, **adopta** el servidor que sigue vivo: lo muestra como UNKNOWN y sin consola, impide arrancar otro encima del mismo mundo y detecta cuándo termina. Solo lo cierra a la fuerza con la confirmación exacta y si el PID sigue siendo el mismo proceso. |
| Búsqueda de Java | `merc-core/src/java.rs` | Busca en `JAVA_HOME`, el `PATH` y las carpetas habituales de Windows (Adoptium, Microsoft, Oracle, Zulu, Corretto, BellSoft, Semeru) y comprueba la versión real de cada uno. |
| Herramienta `merc` | `merc-supervisord/src/bin/merc.rs` | **Solo para desarrollo:** permite probar el supervisor con servidores reales antes de que exista la interfaz. Arranca el supervisor separado de la terminal (incluso fuera de su Job Object si Windows lo permite). |
| Prueba con servidor real | `scripts/probar-servidor-real.ps1` | Selector de carpeta, detección, importación, arranque, espera de EN LÍNEA real, comando, momento para que entres a jugar, parada con guardado y registro completo. |

## 2. Pruebas (resultados reales en Linux)

`cargo test --workspace`: **64 pasan, 0 fallan**, repetido 3 veces. `cargo clippy --workspace --all-targets`: 0 avisos.
Después de las pruebas no quedan procesos Java ni supervisores colgados (comprobado con `ps`).

Pruebas de extremo a extremo con el binario real del supervisor (`merc-supervisord/tests/daemon.rs`):

| Caso | Resultado |
|---|---|
| Token falso → `auth_failed`; protocolo futuro → `version_mismatch`; el supervisor sigue atendiendo | PASS |
| Segundo supervisor con la misma carpeta de datos: sale solo, sin error; sigue atendiendo el primero | PASS |
| **Se cierra el cliente (la "interfaz") con el servidor en marcha**: el servidor sigue EN LÍNEA; al reconectar se ven el estado y la consola acumulada | PASS |
| No se puede cerrar el supervisor ni quitar un servidor mientras está en marcha | PASS |
| **El supervisor muere de golpe**: el servidor sigue vivo con su puerto; el supervisor nuevo lo adopta (UNKNOWN, sin consola, mismo PID); rechaza arrancar otro, enviar comandos y `stop`; detecta su final; luego se arranca de nuevo con consola | PASS |
| Cierre forzado de un huérfano: exige el texto exacto de confirmación; libera el puerto | PASS |
| Importación: carpeta duplicada, carpeta sin servidor, ruta relativa; **la importación no crea ni cambia archivos** en la carpeta del servidor | PASS |

Pruebas nuevas en otros módulos: identidad de proceso por PID + hora de inicio (no se mata un PID reutilizado), escritura atómica de registros, framing con límites, forma JSON estable del protocolo, permisos del token, canal real con instancia única, búsqueda de Java y prefijo `\\?\` de Windows.

## 3. Pendiente de verificar en Windows

Este entorno no puede compilar para Windows. Los dos scripts de `scripts/` lo verifican en tu PC:

- ACL del named pipe (`D:P(A;;GA;;;SY)(A;;GA;;;OW)`): se espera que el mismo usuario se conecte y nadie más. Si la prueba `real_endpoint_listen_and_connect` falla con «acceso denegado», se cambiará a usar el SID del usuario.
- Prueba de junctions (`rejects_junction_escape`), que solo existe en Windows.
- Supervisor sin ventana (`windows_subsystem = "windows"`) y desacoplado de la terminal.
- Hora de inicio de procesos en Windows (`sysinfo`) para la adopción de huérfanos.

## 3b. Fallos encontrados en Windows y corregidos

- **30-10-2026, primera prueba con servidor real:** el script se quedaba colgado después de «Java encontrado».
  Causa: en Windows, `CreateProcess` hereda todos los handles heredables, y el supervisor recién lanzado
  heredaba el tubo de salida de `merc` hacia PowerShell, manteniéndolo abierto para siempre. Corrección:
  `merc` marca sus handles estándar como no heredables antes de lanzar el supervisor (`SetHandleInformation`).
  En Linux no ocurre (los descriptores se cierran al ejecutar), por eso las pruebas automáticas no lo vieron.
- **Elección de Java:** `merc importar` elegía el Java más nuevo (25). Ahora se detecta primero la versión
  de Minecraft y se elige la recomendada (21 para 1.21.x, 17 para 1.18–1.20.4, 8 para ≤ 1.16.5),
  prefiriendo instalaciones reales antes que los accesos `javapath` de Oracle. Con prueba unitaria.
- **Ejecutable bloqueado:** un supervisor de prueba que quedó abierto impide recompilar en Windows. Los
  scripts lo detectan y preguntan antes de cerrarlo.

## 4. Límites conocidos (no se ocultan)

- **Servidor huérfano sin consola:** tras una caída del supervisor no se puede recuperar el stdin/stdout de Java. El servidor sigue y el mundo está a salvo, pero para detenerlo con guardado hay que usar `/stop` dentro del juego, o el cierre forzado con riesgo. Una solución futura es RCON con contraseña aleatoria local, que se evaluará en la Fase 4.
- **Supervisor elevado:** si el supervisor se ejecutara como administrador, el propietario del pipe sería el grupo Administradores y la interfaz sin elevar no podría conectarse. MercHosting no necesita permisos de administrador.
- **Suplantación del nombre del pipe:** otro usuario de la misma PC podría crear antes un pipe con ese nombre. El supervisor real lo detectaría al no poder crearlo, pero la interfaz todavía no verifica la identidad del servidor del pipe. Se añadirá al conectar la interfaz (Windows permite obtener el PID del servidor del pipe).
- **Conexiones:** máximo 32 a la vez. Un proceso local malicioso del mismo usuario podría ocuparlas (riesgo local aceptado y documentado).
- **Consultas sin notificaciones:** la consola se lee preguntando cada poco tiempo, sin envío automático. Es suficiente para la interfaz inicial.

## 5. Siguiente incremento propuesto

Aplicación Tauri 2 + React con la primera pantalla real: lista de servidores, importar con selector de carpeta, estado real, iniciar y detener, y consola en vivo, todo hablando con este supervisor. Sin pantallas de módulos que todavía no existen.
