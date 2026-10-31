# Créditos y licencias de terceros

MercHosting se distribuye bajo GPL-3.0-or-later (ver `LICENSE`). Usa estos componentes de terceros:

| Componente | Uso | Licencia |
|---|---|---|
| Tauri 2 y sus plugins (dialog, opener, notification) | Ventana de escritorio y notificaciones de Windows | MIT / Apache-2.0 |
| React | Interfaz | MIT |
| Vite, Vitest, Testing Library, TypeScript | Desarrollo y pruebas | MIT / Apache-2.0 |
| Unbounded (Fontsource) | Tipografía de marca y títulos | SIL Open Font License 1.1 |
| Manrope (Fontsource) | Tipografía de la interfaz | SIL Open Font License 1.1 |
| Lucide | Iconos | ISC |
| JetBrains Mono (Fontsource) | Tipografía de la consola | SIL Open Font License 1.1 |
| rusqlite (SQLite incluido) | Base de datos local | MIT; SQLite es de dominio público |
| interprocess | Canal local (named pipes) | MIT / Apache-2.0 |
| sysinfo | Identidad de procesos, memoria y CPU de la PC, espacio en disco | MIT |
| chrono | Hora local de la PC para las tareas programadas | MIT / Apache-2.0 |
| ureq, rustls, rustls-platform-verifier | Descargas HTTPS desde las fuentes oficiales | MIT / Apache-2.0 (rustls también ISC) |
| sha1, sha2, md-5 (RustCrypto) | Verificación de huellas de descarga y de las copias de seguridad | MIT / Apache-2.0 |
| zip | Exportar e importar mundos (.zip) | MIT |
| Modrinth, CurseForge y Hangar de PaperMC (APIs) | Catálogo de mods, plugins y datapacks; los iconos y textos son de cada proyecto y se muestran tal cual | Términos de cada servicio |
| Agente de Playit.gg (`playitd`, se descarga de su publicación oficial; no se incluye en MercHosting) | Conexión por Internet sin abrir puertos | BSD-2-Clause (Developed Methods LLC) |
| iroh (n0, Inc.) y su relé público gratuito | Canal cifrado entre PCs para compañeros (conexión directa o por relé) | MIT OR Apache-2.0 |
| Tokio | Ejecución asíncrona del canal de compañeros | MIT |
| Eclipse Temurin (Adoptium; se descarga de su publicación oficial a petición, no se incluye en MercHosting) | Java para los servidores | GPLv2 con Classpath Exception |
| Simple Voice Chat (henkelmax; no se incluye ni se descarga: MercHosting solo lee y ajusta su archivo de configuración en el servidor de la persona) | Diagnóstico y túnel del chat de voz | Mod de terceros, con su propia licencia |
| image (y png, zune-jpeg, gif, image-webp, tiff, moxcms, pxfm) | Convertir cualquier foto al icono del servidor de 64×64 | MIT / Apache-2.0 (tiff y color_quant: MIT; moxcms y pxfm: BSD-3-Clause o Apache-2.0) |
| Gumroad (API pública de licencias; la compra se hace en su página oficial) | Comprobar la clave de Supporter Edition | Términos de Gumroad |
| trash | Mandar a la Papelera de reciclaje la carpeta de un servidor que quitas | MIT |
| serde, serde_json, thiserror, uuid, regex, windows-sys, widestring | Utilidades | MIT / Apache-2.0 |

El logo de MercHosting es obra del autor del proyecto (`app/branding/logo-original.png`); su versión vectorial está en `assets/logo/` (generada con `scripts/logo/generar-logo.py`).

La lista completa de componentes incluidos, con versión y licencia, está en `docs/LICENCIAS_DE_TERCEROS.md` (se genera con `node scripts/licencias-terceros.mjs`) y se ve en la app en **Acerca de**.
Minecraft es una marca de Mojang/Microsoft. MercHosting no está afiliado a ellas.
