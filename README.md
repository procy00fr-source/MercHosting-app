# MercHosting

> «Tu primer servidor no debería necesitar una mensualidad».

**MercHosting** es una app gratuita y de código abierto para **Windows 10/11** que crea y administra servidores de Minecraft **en tu propia PC**: sin cuotas mensuales, sin cuentas y sin telemetría.

**Código:** este repositorio · **Descargar:** [última versión](https://github.com/procy00fr-source/MercHosting-releases/releases/latest) · Guía: [docs/INSTALACION.md](docs/INSTALACION.md)

## Qué hace

- **Crear servidores** Vanilla, Paper, Purpur, Spigot, Fabric, Quilt, Forge, NeoForge y Arclight, o desde un **modpack** de Modrinth o CurseForge. Todo se descarga de la fuente oficial y se comprueba su huella.
- **Mods, plugins, datapacks y paquetes de recursos** desde Modrinth, CurseForge y Hangar, con dependencias, actualizaciones, desactivación reversible y un **separador de mods cliente/servidor**.
- **Actualizar modpacks** sin pisar tus configs ni los mods que quitaste.
- **Consola** con niveles, favoritos y confirmación de comandos peligrosos; **archivos** con editor, papelera y copias; **jugadores**, **mundos**, **ajustes**, **MOTD e icono**, **LuckPerms**.
- **Copias de seguridad** comprobadas (también con el servidor encendido), **tareas programadas**, **salud** (CPU, memoria, TPS, disco) y **alertas**.
- **Jugar con amigos**: red local y por Internet con **Playit.gg** sin abrir puertos, **Simple Voice Chat** y prueba real de la conexión.
- **Compañeros**: un amigo administra tu servidor desde su PC con los permisos que elijas, por un canal cifrado P2P (iroh), sin cuentas.
- **Diagnóstico** de crashes y conflictos con su prueba, y arreglos reversibles.
- Español e inglés. Supporter Edition opcional (US$8, pago único) **solo cosmética**.

## Privacidad

Todo se queda en tu PC. MercHosting solo se conecta a Internet para lo que pides. Ver [docs/PRIVACIDAD.md](docs/PRIVACIDAD.md).

## Estructura

| Carpeta | Contenido |
|---|---|
| `crates/merc-core` | Dominio: estados, confinamiento de rutas, detección de servidores, arranque, Java |
| `crates/merc-store` | SQLite con migraciones: instancias y actividad |
| `crates/merc-supervisor` | Procesos Java: arranque, parada limpia, crash, registros y adopción de procesos |
| `crates/merc-install` | Fuentes oficiales, descarga verificada e instalación |
| `crates/merc-admin` | Ajustes, jugadores, archivos, copias, mundos, diagnóstico, LuckPerms |
| `crates/merc-content` | Modrinth, CurseForge, Hangar, modpacks, paquetes de recursos |
| `crates/merc-net` | Playit.gg y prueba de conexión |
| `crates/merc-remote` | Compañeros: canal cifrado entre PCs (iroh) |
| `crates/merc-ipc` | Protocolo versionado y canal local autenticado |
| `crates/merc-supervisord` | Supervisor persistente (sin ventana) |
| `app/` | Interfaz React + TypeScript y ventana Tauri 2 (`app/src-tauri`) |
| `docs/` | Documentación de cada parte, contratos del protocolo y especificación |
| `scripts/` | Verificación, publicación, licencias, traducción y logo |

## Compilar

Requisitos: Rust estable, Node.js LTS y, para las pruebas del supervisor, un JDK 17+ (`java`, `javac`, `jar` en el PATH).

```
cargo test --workspace
npm --prefix app ci
npm --prefix app test
```

En Windows, `scripts\abrir-app.ps1` compila y abre la app; `scripts\verificar-windows.ps1` comprueba las herramientas y pasa todas las pruebas. Publicar una versión: [docs/PUBLICAR_VERSION.md](docs/PUBLICAR_VERSION.md).

**CurseForge:** su API necesita una clave que no está en el repositorio. Sin ella, MercHosting funciona igual y muestra CurseForge como no disponible.

## Contribuir

Ideas y errores: la sección **Comentarios** de la app o un *issue*. Para código, lee [CONTRIBUTING.md](CONTRIBUTING.md). Problemas de seguridad: [SECURITY.md](SECURITY.md).

## Licencia

MercHosting es software libre bajo la **GNU General Public License v3.0 o posterior** ([LICENSE](LICENSE)). Componentes de terceros: [docs/LICENCIAS_DE_TERCEROS.md](docs/LICENCIAS_DE_TERCEROS.md) y [docs/CREDITOS.md](docs/CREDITOS.md).

Copyright (C) 2026 Amir (Amercson) · [YouTube @Amercson](https://www.youtube.com/@Amercson) · [TikTok @amercfr](https://www.tiktok.com/@amercfr)

Minecraft es una marca de Mojang Studios y Microsoft. MercHosting no está afiliado a ellos.
