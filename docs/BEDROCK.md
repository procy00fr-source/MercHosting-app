# Servidores Bedrock y addons

MercHosting puede crear e importar servidores de **Minecraft Bedrock** (celular, consolas y Windows) usando el
**Bedrock Dedicated Server (BDS)** oficial de Mojang. Es un programa nativo: no usa Java ni una memoria fija.
Solo entran jugadores de Bedrock. Para que jugadores de Bedrock entren a un servidor **Java**, se usa Geyser
(pestaña Conexión de un servidor Java), que es otra cosa.

## De dónde se descarga

- La lista de enlaces sale de la API que usa la propia página oficial de minecraft.net:
  `https://net-secondary.web.minecraft-services.net/api/v1.0/download/links`
  (`result.links[]` con `downloadType` = `serverBedrockWindows` o `serverBedrockLinux` y `downloadUrl`).
- Solo se aceptan descargas desde `www.minecraft.net`, `minecraft.net` o `minecraft.azureedge.net` por HTTPS.
- Mojang **solo ofrece la última versión**. Si se elige otra, la creación se detiene y lo dice.

## Qué no se puede verificar

- Mojang **no publica la huella** del `.zip`. MercHosting lo descarga por HTTPS, calcula su SHA-256 y lo guarda en
  `merchosting-bedrock-sha256.txt`, en la carpeta del servidor (con el nombre del archivo y la URL), para poder
  compararlo más adelante. Eso no prueba que el archivo sea el original. Solo deja constancia de cuál se instaló.
- La versión instalada se anota en `merchosting-bedrock-version.txt`, porque BDS no la guarda en ningún archivo.

## EULA y privacidad

- Al descargar el servidor, Mojang pide aceptar la EULA y la Política de privacidad de Minecraft. MercHosting **nunca
  las acepta por ti**: el asistente muestra una casilla que tú marcas.
- BDS no usa `eula.txt`. MercHosting escribe ese archivo, con un comentario que lo explica, solo para registrar tu
  aceptación y no encender el servidor sin ella. Usa las mismas comprobaciones que en los servidores Java.

## Cómo se instala

1. El `.zip` se extrae en una carpeta **nueva o vacía**, con rutas seguras, sin enlaces y con límites de tamaño y de
   número de archivos. Si algo falla, se borra solo lo que se creó.
2. El `server.properties` que trae Mojang se conserva con sus comentarios. Solo se cambian las claves que elegiste
   (`server-name`, `server-port`, `max-players`, `gamemode`, `difficulty`, `online-mode`, `allow-list` y `level-seed`).
   En Bedrock los valores se escriben en UTF-8 tal cual (Bedrock no entiende los `\uXXXX` de Java).

## Funcionamiento

- Arranque: `bedrock_server.exe` en la carpeta del servidor (en Linux, `bedrock_server` con `LD_LIBRARY_PATH=.`).
- Listo cuando la consola dice `Server started.`. Jugadores con `Player connected: …, xuid: …` /
  `Player disconnected: …`. Puerto ocupado con `Port [N] may be in use by another process`. Solo cuentan las líneas
  que empiezan con la marca de tiempo de BDS, para que el chat no pueda falsificarlas.
- Detener: `stop` por la consola, igual que en Java.
- Puerto: UDP (19132 por defecto). Antes de encender se comprueba que esté libre por UDP.
- Conexión: la prueba de dirección es el ping de Bedrock por UDP (RakNet), como hace el juego. En Playit hay que crear
  un túnel de tipo **Minecraft Bedrock**. Bedrock no usa registros SRV, así que «Usar mi dominio» no aparece.
- Pestañas que no se muestran porque son de Java o darían datos equivocados: Diagnóstico, Seguridad,
  Actualizaciones, Jugadores, Invitar jugadores, MOTD e icono, Mundos y LuckPerms. En Ajustes no aparece la tarjeta de
  Java y memoria, y las opciones son las de Bedrock.

## Addons

Pestaña **Addons** (solo en servidores Bedrock):

- Instala `.mcpack` (un pack) y `.mcaddon` (varios packs, en carpetas o como `.mcpack` dentro), con el botón o
  arrastrando los archivos. El servidor tiene que estar detenido.
- Cada pack se identifica por su `manifest.json` (`header.uuid`, `header.version` y `modules[].type`: `data`/`script` =
  comportamiento, `resources` = recursos). Se copia a `behavior_packs/<nombre>_<uuid>/` o
  `resource_packs/<nombre>_<uuid>/` y se activa en `worlds/<level-name>/world_behavior_packs.json` o
  `world_resource_packs.json`.
- La misma versión ya instalada no se toca. Si llega una versión nueva, la anterior va a la **papelera de
  MercHosting** (se puede recuperar desde Archivos) y se instala la nueva.
- Quitar un pack lo desactiva y manda su carpeta a la papelera. No se borra nada.
- Los packs que trae el propio servidor (carpetas `vanilla*`, `chemistry*`, `experimental*`, `editor*`…) se listan
  aparte, solo para consultar. Se reconocen por el nombre de la carpeta.
- Los packs con scripts pueden necesitar activar experimentos o APIs beta en el mundo. MercHosting lo avisa, pero no
  lo activa.

### Buscador de addons

CurseForge tiene una sección de Minecraft Bedrock, pero todavía no comprobamos si su API la ofrece ni con qué
condiciones, y MercHosting aún no tiene clave de CurseForge. Por eso el buscador figura como **«Próximamente»**, sin
botones. Cuando haya clave, el identificador del juego se debe obtener en el momento con `/v1/games` (slug
`minecraft-bedrock`), nunca escrito a mano en el código.

## Limitaciones

- BDS para Linux solo funciona oficialmente en Ubuntu 22.04 o más nuevo. La app de MercHosting es para Windows.
- No hay actualización automática de BDS. Para cambiar de versión hay que crear el servidor de nuevo y copiar el mundo.
- La lista de permitidos (`allowlist.json`) y los permisos (`permissions.json`, por XUID) se editan desde Archivos.
  Todavía no tienen una pantalla propia.
