# Cambios de MercHosting

Cada versión nueva se describe aquí antes de publicarla (`scripts/publicar-version.ps1` toma de aquí las notas que ve la gente al actualizar).

## 1.0.0

- MercHosting llega a Linux: app de escritorio en .deb y AppImage (Ubuntu y derivadas).
- Servidor casero sin pantalla: deja MercHosting en una PC vieja o un servidor Linux con `merc servicio instalar` y manéjalo desde tu PC con Compañeros (código de control total, sin abrir puertos).
- Servidores Bedrock oficiales de Mojang, con su pestaña Addons (.mcpack y .mcaddon) y su pestaña Jugadores (lista blanca y permisos).
- Jugadores de Bedrock en servidores Java: instala Geyser y Floodgate desde Conexión y prueba la dirección de Bedrock de verdad.
- Forge antiguo: crea servidores 1.12.2 y 1.13–1.16.5 con Java 8.
- Modo 24/7: un botón activa el reinicio si se cae, el arranque con el sistema y que la PC no se suspenda sola mientras haya un servidor encendido.
- Usar tu propio dominio: la app arma el registro SRV que va al túnel de Playit y lo prueba.
- La dirección para tus amigos por Internet ahora se ve y se copia desde el Resumen.
- Buscador de addons de CurseForge: llegará cuando MercHosting tenga su clave oficial.
- Modpacks de Modrinth que anotan mal el tamaño de un archivo ya se instalan si su huella coincide.
- App para celular (Android, beta): maneja los servidores de tu PC desde el celular con un código de Compañeros, o crea un servidor dentro del propio celular (Java o Java + Bedrock con Geyser), con avisos de batería y calor, Playit y mods y plugins de Modrinth. CurseForge, próximamente.

## 0.2.4

- Nueva sección Acerca de: versión, licencia GPL-3.0, novedades, política de privacidad y componentes de terceros.
- Logo redibujado limpio (icono de la app, instalador y la interfaz).
- MercHosting en inglés: elígelo en Apariencia → Idioma · Language.
- Actualizar modpacks (Actualizaciones → Modpack): ves qué cambia antes, se hace una copia completa y se respetan tus configs editados y los mods que quitaste.
- Paquetes de recursos: búscalos en Modrinth desde Ajustes y ponlos en el servidor con un clic (se comprueban y se guarda su huella).
- Archivos: el editor tiene colores, puedes mover y copiar a otra carpeta y extraer .zip de forma segura.
- Consola: filtra por Info, Avisos y Errores, muestra la hora, guarda comandos favoritos y te pide confirmar los comandos peligrosos (op, ban, stop…).
- Arrastra tus .jar desde el Explorador y suéltalos en Mods o Plugins (o cualquier archivo en Archivos).
- Nuevo separador de mods cliente/servidor (Mods → Separar cliente/servidor), apagado hasta que lo actives: revisa todos los mods, explica de qué lado es cada uno y aparta los que solo sirven en el juego a una cuarentena que puedes deshacer.
- Forge y NeoForge: el Diagnóstico explica mejor cuando un mod de solo cliente puede impedir que el servidor arranque.

## 0.2.3

- Forge: ahora puedes elegir cualquier versión publicada para tu versión de Minecraft, no solo la recomendada.

## 0.2.2

- Nueva sección Comentarios: manda ideas o errores al desarrollador.
