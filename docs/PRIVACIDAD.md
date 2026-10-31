# Política de privacidad de MercHosting

Última actualización: 31 de octubre de 2026.

MercHosting es una app de escritorio para Windows que crea y administra servidores de Minecraft **en tu propia PC**. No tiene cuentas, no tiene un servidor central de MercHosting y **no envía estadísticas de uso ni telemetría**.

## Qué se guarda y dónde

Todo se queda en tu PC:

- **Datos de la app** en `%LOCALAPPDATA%\MercHosting`: la lista de tus servidores, ajustes, el historial de Actividad, los registros de la consola, las copias de seguridad, la papelera de MercHosting, el registro de modpacks y, si eres Supporter, tu clave de licencia.
- **Servidores nuevos**, por defecto en `%USERPROFILE%\MercHosting\Servidores` (o la carpeta que elijas).
- Preferencias de la interfaz (idioma, tema, favoritos de la consola) en el almacenamiento local de la ventana.

Puedes borrarlo todo desinstalando la app y borrando esas carpetas. MercHosting nunca borra un servidor sin que lo pidas: al quitarlo, la carpeta va a la Papelera de reciclaje.

## Con quién se conecta y por qué

MercHosting solo se conecta a Internet para hacer lo que pides. En cada caso, el servicio recibe tu dirección IP y la petición, como cualquier descarga:

| Servicio | Cuándo | Qué se envía |
|---|---|---|
| Mojang, PaperMC, Purpur, Fabric, Quilt, Forge, NeoForge, SpigotMC (BuildTools), GitHub (Arclight) | Al crear o actualizar un servidor | La versión que pides |
| Eclipse Adoptium (Temurin) | Si instalas Java desde la app | La versión de Java |
| Modrinth, CurseForge, Hangar | Al buscar, instalar o actualizar mods, plugins, datapacks, modpacks o paquetes de recursos, y al revisar actualizaciones de lo instalado | Lo que buscas y las huellas (SHA-1) de los archivos instalados para reconocerlos |
| Playit.gg | Solo si activas la conexión por Internet | Lo necesario para crear tu túnel; la cuenta y el agente son de Playit y tienen su propia política |
| GitHub (repositorio de publicaciones de MercHosting) | Al buscar actualizaciones de la app | Nada además de la petición |
| Gumroad | Solo si activas Supporter Edition | Tu clave de licencia, para comprobarla |
| Relé de iroh (n0) | Solo si usas Compañeros y no hay conexión directa | Tráfico cifrado de extremo a extremo entre tus PCs; el relé no puede leerlo |
| Google Forms | Solo si envías un comentario | Se abre el formulario en tu navegador con lo que escribiste y la versión de la app; tú decides si lo envías |
| La dirección que pongas como paquete de recursos | Al comprobarlo | Una descarga del archivo para calcular su huella |

**Compañeros:** la conexión entre PCs es directa y cifrada (iroh). Un compañero solo puede hacer lo que le permitiste en cada servidor; nunca ejecuta programas en tu PC.

**Tus jugadores:** se conectan a tu servidor de Minecraft, no a MercHosting. El historial de jugadores (nombre, primera y última conexión, tiempo jugado y tus notas privadas) se guarda solo en tu PC, a partir de los registros de tu propio servidor.

## Lo que no hacemos

- No vendemos ni compartimos datos.
- No hay publicidad ni rastreadores.
- No se suben tus mundos, archivos ni registros a ningún sitio, salvo que tú los exportes o los compartas.

## Contacto

Usa la sección **Comentarios** de la app o las redes del autor (YouTube @Amercson, TikTok @amercfr).

Si cambiamos esta política, la fecha de arriba cambiará y lo diremos en las notas de la versión.
