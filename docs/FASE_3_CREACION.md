# Fase 3 · Creación e instalación de servidores

Estado: **asistente real funcionando de punta a punta** (catálogo oficial → descarga verificada →
instalación → registro → encendido). Verificado en Linux con fuentes simuladas en `127.0.0.1`.
Falta comprobarlo en Windows contra las fuentes reales (ver «Cómo probarlo»).

## Qué hace

Botón **Crear servidor** (barra lateral y pantalla vacía). Seis pasos:

1. **Tipo**: Vanilla, Paper, Fabric o NeoForge.
2. **Versión**: lista oficial en vivo, con buscador, la última marcada y el Java que pide cada una.
   Busca Java en la PC y elige el adecuado (Java 25 para 26.x, 21 para 1.20.5–1.21.11…). Si falta,
   lo dice y permite volver a buscar.
3. **Ajustes**: nombre, carpeta (nueva, por defecto `%USERPROFILE%\MercHosting\Servidores\<nombre>`),
   memoria (límite según la RAM real de la PC), puerto libre sugerido, jugadores, modo, dificultad,
   mensaje, semilla, lista blanca y online-mode (con advertencia clara si se desactiva).
4. **EULA**: enlace oficial y casilla. MercHosting nunca la acepta solo; si no se marca, el servidor
   se crea igual pero no arrancará hasta aceptarla.
5. **Resumen**: compilación exacta que se descargará, tamaño, tipo de huella y avisos de la fuente.
6. **Creación**: pasos reales, barra de descarga con bytes reales, registro en vivo, botón cancelar.
   Al terminar: **Encender ahora** o **Ir al servidor**.

## Fuentes oficiales (comprobadas en octubre de 2026)

| Tipo | Lista de versiones | Descarga | Verificación |
|---|---|---|---|
| Vanilla | `piston-meta.mojang.com` (manifiesto v2) | `downloads.server` del JSON de la versión | SHA-1 y tamaño publicados por Mojang; Java según `javaVersion.majorVersion` |
| Paper | Fill v3 `fill.papermc.io/v3/projects/paper` | última compilación `STABLE` (si no hay, la más reciente marcada como experimental) | SHA-256 y tamaño publicados |
| Fabric | `meta.fabricmc.net/v2/versions/game` (solo estables) | lanzador de servidor con loader e instalador estables | Fabric no publica huella: HTTPS y aviso al usuario |
| NeoForge | `maven.neoforged.net` (`maven-metadata.xml`) | instalador oficial, ejecutado con `--installServer` y el Java elegido | `.sha256` (o `.sha1`) del propio maven |

- Versiones de Minecraft por año (26.1, 26.1.2…) y de NeoForge en 4 partes (`26.1.2.76` → MC 26.1.2)
  están soportadas en el núcleo.
- PaperMC pide un User-Agent que identifique la aplicación: se envía `MercHosting/<versión> (app de
  escritorio libre GPL-3.0 …)`. No se incluye URL de contacto porque el proyecto todavía no tiene
  repositorio ni web públicos; añadirla cuando existan.
- Paper y Fabric descargan el servidor de Mojang en su primer arranque; NeoForge lo hace durante la
  instalación. El asistente lo avisa.

## Seguridad y datos

- Solo HTTPS (HTTP únicamente hacia `127.0.0.1`, para las pruebas). Límite de 512 MB por archivo.
- Descarga a `.part`, comprobación de tamaño y huella, y solo entonces se renombra.
- La carpeta destino debe **no existir o estar vacía**; nunca se escribe en una carpeta con archivos
  ni en enlaces/junctions. Si algo falla o se cancela, se borra **solo** lo creado (la carpeta que
  se creó, o el contenido de la carpeta que estaba vacía).
- `server.properties` solo con las claves que permite el asistente, validadas (puerto 1024–65535,
  modos y dificultades conocidos, sin saltos de línea; tildes como `\uXXXX`). El resto lo completa
  Minecraft.
- Nombres de archivo que vienen de las APIs se validan; si son raros se usa uno propio.
- Java del usuario: se comprueba su versión real antes de instalar.
- Trabajos de creación en el supervisor (siguen aunque se cierre la ventana), máximo 2 a la vez,
  nunca dos en la misma carpeta; el supervisor no se puede cerrar mientras crea. Todo se audita.
- Las direcciones de las fuentes solo se pueden redirigir en compilaciones de desarrollo
  (`MERC_SOURCE_*`, para las pruebas).

## Protocolo (añadido, compatible con v1)

`catalog`, `resolve`, `creation_defaults`, `check_target`, `create_server` (devuelve un trabajo),
`job` (estado + registro incremental) y `cancel_job`; nuevo código de error `network`.
Ejemplos en `docs/contratos/ipc-v1-ejemplos.json`, comprobados desde Rust y TypeScript.

## Pruebas

- Rust: 96 pruebas (`cargo test --workspace`), clippy sin avisos. Nuevas:
  - `merc-install`: análisis de las respuestas reales de cada fuente, reglas de carpeta y deshacer,
    propiedades, y 7 flujos completos contra un servidor HTTP local (Vanilla, Paper, Fabric,
    NeoForge con instalador simulado que falla y que funciona, descarga alterada rechazada,
    cancelación, carpeta con archivos intacta).
  - `merc-supervisord/tests/create.rs`: supervisor real crea un Paper desde la fuente simulada,
    rechaza Java 21 para 26.1.2, registra el servidor y lo **enciende hasta ONLINE**; sin Internet
    responde `network` y no deja nada.
- Interfaz: 35 pruebas (7 nuevas del asistente).
- Capturas del asistente: `docs/capturas/fase3-*.png` (interfaz con supervisor simulado).
- `merc crear paper 1.21.11` probado en Linux contra fuente simulada: descarga, verifica SHA-256,
  registra y llega a ONLINE.

## Tipos añadidos después (Fase 3 · ampliación)

| Tipo | Fuente | Instalación | Verificación |
|---|---|---|---|
| Purpur | `api.purpurmc.org/v2` (versiones, última compilación y su detalle) | JAR directo | MD5 publicado por Purpur (solo detecta archivos dañados) |
| Spigot | Lista de `hub.spigotmc.org/versions/` | **BuildTools**, la herramienta oficial, compila en el PC (`--rev <versión> --compile spigot --output-dir <servidor>`); trabaja en `%LOCALAPPDATA%\MercHosting\cache\buildtools-<versión>` y solo deja `spigot-<versión>.jar` en el servidor | BuildTools se baja por HTTPS (Jenkins no publica huella) |
| Quilt | `meta.quiltmc.org/v3` y `maven.quiltmc.org` | Instalador oficial `install server <mc> <loader> --download-server` | `.sha256`/`.sha1` del maven |
| Forge (1.17+) | `promotions_slim.json` (recomendada; si no hay, la más reciente, avisando) y `maven.minecraftforge.net` | Instalador oficial `--installServer` | `.sha256`/`.sha1` del maven |
| Arclight | Publicaciones de `IzzelAliz/Arclight` en la API de GitHub (`arclight-<cargador>-<mc>-<versión>.jar`) | JAR directo; prefiere publicación estable y NeoForge > Forge > Fabric | SHA-256 que GitHub calcula de cada archivo publicado (`digest`) |

Notas:
- SpigotMC no permite redistribuir servidores ya compilados; por eso se compila en tu PC. Necesita
  un **JDK** (Java con `javac`): el asistente solo ofrece Java que lo tenga. En Windows, BuildTools
  descarga Git portátil si hace falta. Tarda entre 5 y 20 minutos.
- Forge anterior a 1.17 no se ofrece: usaba otro sistema de arranque que MercHosting aún no gestiona.
- **Mohist se retiró del asistente** (30/10/2026): su lista y sus compilaciones se leían bien, pero
  sus dos enlaces de descarga respondían HTTP 200 con 0 bytes, también con `curl`. Un servidor Mohist
  existente se sigue reconociendo al importarlo.
- **Arclight** (GPL-3.0): las publicaciones de 1.21.11 y 26.1.2 están marcadas como experimentales por
  su autor y solo para NeoForge y Fabric; el asistente lo avisa. La API de GitHub sin cuenta permite
  60 consultas por hora; el catálogo se guarda 10 minutos para no gastarlas.
- Los servidores híbridos (Arclight) pueden fallar con algunas combinaciones de mods y plugins; se avisa.

## Pendiente (siguientes incrementos)

- Elegir versión concreta del loader/compilación (ahora: la última estable).
- Icono del servidor, distancias de visión/simulación, `keepInventory` (es una regla del juego, no
  de `server.properties`), modpacks (Fase 5) y conectividad (Fase 7).

## Cómo probarlo en Windows

1. `scripts\verificar-windows.ps1` (todas las pruebas).
2. `scripts\probar-creacion.ps1`: consulta las 4 listas oficiales reales, crea un servidor de
   prueba en `%TEMP%`, lo enciende, te deja entrar y al final pregunta si borrar la carpeta.
3. `scripts\abrir-app.ps1` y usa **Crear servidor**.
