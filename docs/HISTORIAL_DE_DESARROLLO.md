# MercHosting

> «Tu primer servidor no debería necesitar una mensualidad».

MercHosting es una aplicación de escritorio para Windows que permite crear y administrar servidores de
Minecraft que se ejecutan **en tu propia computadora**. Es gratuita: todas las funciones esenciales son gratis.
La Supporter Edition opcional (US$8, pago único) solo añadirá extras cosméticos.

## Estado actual

**Fase 10 (incrementos 1 y 2): Free + Supporter Edition.** Modo claro para todos (o igual que Windows), iconos alternativos de la ventana y créditos solo con consentimiento. Página «Apoyar MercHosting» con comparador honesto, compra en la página oficial de Gumroad (US$8, pago único) y activación o restauración con la clave de licencia (reembolsos y contracargos detectados, sin quitar nada por estar sin red); y «Apariencia» con temas exclusivos, fondos, insignia «Colaborador» y celebraciones desactivables, todo cosmético. **Fase 9 (incrementos 1 a 4): diagnóstico, seguridad, invitaciones, Java, plantillas, actualizaciones y MOTD.** Analizador de crashes y detector de conflictos (mods para otro loader, duplicados, dependencias que faltan, Java incompatible, memoria, puerto ocupado…), cada hallazgo con su prueba y nivel de confianza y arreglos reversibles a un clic; Java y memoria editables en Ajustes; centro de seguridad (cuentas oficiales, exposición, lista blanca, RCON, compañeros, copias y claves); e invitación para jugadores con la dirección probada, la versión, el loader y los mods que necesitan en su juego con sus páginas oficiales; instalador de Java (Eclipse Temurin desde la API oficial de Adoptium, con SHA-256, sin instalar nada en Windows); plantillas de supervivencia, creativo, mods y minijuegos con valores reales; centro de actualizaciones (compilación de Paper y Purpur o loader de Fabric, y todos los mods y plugins de una vez) con copia completa previa y vuelta atrás restaurándola; y pestaña «MOTD e icono» con editor de colores y formato con vista previa, y cualquier foto convertida al icono de 64×64. **Fase 8 (incrementos 1 y 2): compañeros sin cuentas.** Un amigo con MercHosting en su PC administra un servidor tuyo con un código de invitación de un solo uso y los permisos que elijas (roles y 21 permisos separados, advertencia obligatoria para archivos, comandos y mods); canal cifrado de punta a punta entre las dos PCs con iroh, sin servidor propio ni pagos; tu PC decide cada acción, lo anota en la Actividad y puede revocar al momento; y archivos, mundos, copias y registros viajan entre las dos PCs comprobados con SHA-256. **Fase 7 (incrementos 1 y 2): conexión, Playit.gg y chat de voz.** Dirección de la red local y amigos por Internet con el agente oficial de Playit (descarga verificada, vinculación oficial con la cuenta propia, túneles detectados y dirección pública probada de verdad antes de copiarla); Simple Voice Chat detectado con su puerto UDP real, guía del túnel de voz, `voice_host` escrito con copia previa y diagnóstico; y estado de la conexión por capas. **Fase 6 (completa): copias, tareas programadas, salud y alertas.** Copias del mundo o completas, comprobadas con SHA-256, también con el servidor encendido (pausando el guardado), y restauración con copia previa automática; tareas programadas de copias, reinicios y apagados con aviso, encender, comandos y mensajes; salud medida (PC, discos, servidor y TPS cuando el servidor lo informa), modos de rendimiento con vista previa y deshacer, y alertas con notificaciones de Windows. **Fase 5 (completa): contenido.** Pestañas Mods, Plugins y Datapacks con búsqueda en Modrinth, CurseForge (este con clave de API) y Hangar (plugins de Paper y Purpur), dependencias, instalación verificada, actualizaciones y desactivación reversible; crear servidores desde modpacks con el loader exacto; y paquete de recursos del servidor comprobado y con su SHA-1. CurseForge está construido y aparece como «Próximamente» hasta tener la clave oficial. **Fase 4 (completa): administración local.** Además del asistente de creación (Vanilla, Paper, Purpur, Spigot, Fabric, Quilt, Forge, NeoForge y Arclight, con descarga verificada), cada servidor tiene pestañas de Jugadores, Mundos (duplicar, exportar, importar y reglas del juego), Ajustes (`server.properties`), Archivos (con papelera y copias restaurables) y Actividad, además de modo mantenimiento, reinicio con aviso (cancelable) y borrar un servidor mandando su carpeta a la Papelera de reciclaje. Todavía no hay instalador de MercHosting.
- [`docs/FASE_1_INCREMENTO_1.md`](FASE_1_INCREMENTO_1.md): núcleo, datos y ciclo de vida.
- [`docs/FASE_1_INCREMENTO_2.md`](FASE_1_INCREMENTO_2.md): supervisor aparte, canal local seguro y recuperación tras caídas.
- [`docs/FASE_1_INCREMENTO_3.md`](FASE_1_INCREMENTO_3.md): ventana de escritorio (Tauri 2 + React).
- [`docs/FASE_1_INCREMENTO_4.md`](FASE_1_INCREMENTO_4.md): rediseño, logo, métricas reales y arreglo del parpadeo de la consola.
- [`docs/FASE_3_CREACION.md`](FASE_3_CREACION.md): asistente de creación, fuentes oficiales y descargas verificadas.
- [`docs/FASE_4_INCREMENTO_1.md`](FASE_4_INCREMENTO_1.md): jugadores, editor de ajustes y explorador de archivos.
- [`docs/FASE_4_INCREMENTO_2.md`](FASE_4_INCREMENTO_2.md): mundos, reglas del juego, papelera, actividad y reinicio con aviso.
- [`docs/FASE_4_INCREMENTO_3.md`](FASE_4_INCREMENTO_3.md): mantenimiento, cancelar reinicio, progreso de mundos, más reglas, ver carpeta y borrar servidor.
- [`docs/FASE_5_INCREMENTO_1.md`](FASE_5_INCREMENTO_1.md): mods y plugins desde Modrinth y CurseForge.
- [`docs/FASE_5_INCREMENTO_2.md`](FASE_5_INCREMENTO_2.md): crear un servidor desde un modpack de Modrinth.
- [`docs/FASE_5_INCREMENTO_3.md`](FASE_5_INCREMENTO_3.md): CurseForge completo, «Próximamente» hasta tener la clave.
- [`docs/FASE_5_INCREMENTO_4.md`](FASE_5_INCREMENTO_4.md): plugins de Hangar, datapacks y paquete de recursos.
- [`docs/FASE_6_INCREMENTO_1.md`](FASE_6_INCREMENTO_1.md): copias de seguridad comprobadas y restauración.
- [`docs/FASE_6_INCREMENTO_2.md`](FASE_6_INCREMENTO_2.md): tareas programadas.
- [`docs/FASE_6_INCREMENTO_3.md`](FASE_6_INCREMENTO_3.md): salud, alertas y modos de rendimiento.
- [`docs/FASE_7_INCREMENTO_1.md`](FASE_7_INCREMENTO_1.md): conexión y Playit.gg para Minecraft.
- [`docs/FASE_7_INCREMENTO_2.md`](FASE_7_INCREMENTO_2.md): Simple Voice Chat y estado por capas.
- [`docs/FASE_8_INCREMENTO_1.md`](FASE_8_INCREMENTO_1.md): compañeros entre PCs, sin cuentas.
- [`docs/FASE_8_INCREMENTO_2.md`](FASE_8_INCREMENTO_2.md): archivos entre PCs de compañeros.
- [`docs/FASE_9_INCREMENTO_1.md`](FASE_9_INCREMENTO_1.md): diagnóstico, y Java y memoria.
- [`docs/FASE_9_INCREMENTO_2.md`](FASE_9_INCREMENTO_2.md): centro de seguridad e invitación de jugadores.
- [`docs/FASE_9_INCREMENTO_3.md`](FASE_9_INCREMENTO_3.md): instalador de Java y plantillas.
- [`docs/FASE_9_INCREMENTO_4.md`](FASE_9_INCREMENTO_4.md): centro de actualizaciones, y MOTD e icono.
- [`docs/FASE_10_INCREMENTO_1.md`](FASE_10_INCREMENTO_1.md): Supporter Edition (Gumroad) y apariencia.
- [`docs/FASE_10_INCREMENTO_2.md`](FASE_10_INCREMENTO_2.md): modo claro, iconos alternativos y créditos.
- [`docs/REDISENO_NAVEGACION.md`](REDISENO_NAVEGACION.md): rediseño con la página «Servidores» y la barra de secciones.

## Estructura

| Carpeta | Contenido |
|---|---|
| `crates/merc-core` | Dominio: estados reales, confinamiento de rutas, detección de servidores, perfiles de arranque, requisitos de Java |
| `crates/merc-store` | SQLite con migraciones versionadas: instancias y auditoría |
| `crates/merc-supervisor` | Supervisor de procesos Java: arranque, readiness, parada limpia, crash, logs rotativos y adopción de procesos huérfanos |
| `crates/merc-install` | Catálogo de las fuentes oficiales, descarga verificada (SHA-1/SHA-256) e instalación en carpeta nueva |
| `crates/merc-admin` | Administración: `server.properties` editable, listas de jugadores y explorador confinado con copias y papelera |
| `crates/merc-remote` | Canal cifrado entre PCs para compañeros (iroh), códigos de invitación y mensajes |
| `crates/merc-net` | Conectividad: agente oficial de Playit (vinculación y estado) y prueba real de conexión a Minecraft |
| `crates/merc-content` | Mods, plugins y datapacks: catálogos de Modrinth, CurseForge y Hangar, paquete de recursos, plan con dependencias, instalación verificada y desactivación reversible |
| `crates/merc-ipc` | Protocolo versionado y canal local autenticado (named pipe en Windows) |
| `crates/merc-supervisord` | Proceso supervisor persistente sin ventana y herramienta de desarrollo `merc` |
| `app/` | Interfaz React + TypeScript y ventana Tauri 2 (`app/src-tauri`) |
| `docs/contratos` | Ejemplos JSON del protocolo, comprobados desde Rust y desde TypeScript |
| `docs/especificacion` | Paquete de directivas PRIME Fase 2 (fuente de verdad del producto) |
| `scripts` | Ayudas de verificación para desarrolladores |


## Desarrollo

Requisitos: Rust estable (1.82+), Node.js LTS y, para las pruebas del supervisor, un JDK 17+ con `java`, `javac` y `jar` en el PATH.

```
cargo test --workspace
npm --prefix app ci && npm --prefix app test
```

En Windows:
- `scripts\abrir-app.ps1`: compila y abre la aplicación.
- `scripts\verificar-windows.ps1`: comprueba las herramientas y ejecuta todas las pruebas.
- `scripts\probar-servidor-real.ps1`: prueba guiada con una copia de un servidor real.

## Licencia

MercHosting es software libre: puedes redistribuirlo y/o modificarlo bajo los términos de la
**Licencia Pública General de GNU, versión 3** o (a tu elección) cualquier versión posterior,
publicada por la Free Software Foundation. Se distribuye sin ninguna garantía.
Texto completo en [`LICENSE`](../LICENSE).

Copyright (C) 2026 Amir (Amercson)
