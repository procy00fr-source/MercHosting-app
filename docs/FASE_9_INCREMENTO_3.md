# Fase 9 · incremento 3: Instalador de Java y plantillas

## Instalador de Java (Eclipse Temurin)
Botón **«Descargar Java N (Eclipse Temurin)»**. Aparece en tres sitios:
- en el asistente de creación (servidor y modpack) cuando no hay un Java compatible;
- en Ajustes › Java y memoria › Cambiar Java;
- en el Diagnóstico, en «Elegir Java».

Qué se comprobó antes (documentación oficial de Adoptium, «Using the Adoptium API»):
- La API oficial `api.adoptium.net/v3` da, para cada versión, el paquete con su enlace de descarga y su **SHA-256**. Los binarios se publican en los releases de GitHub de Adoptium.
- Existen JRE para Windows x64 en .zip (además del instalador .msi).

Cómo funciona:
1. Se pide a `GET /v3/assets/latest/{versión}/hotspot?architecture=x64&image_type=jre&os=windows&vendor=eclipse`. Se elige el JRE de Windows x64 en .zip, que tiene que venir por HTTPS y con su SHA-256.
2. Se usa la **versión LTS** que cumple lo pedido: 8, 11, 17, 21 o 25. Por ejemplo, Minecraft 1.17 pide Java 16 y se descarga 17.
3. Se descarga como un trabajo con progreso (paso, MB descargados y cancelación) y se **comprueba el SHA-256** publicado.
4. Se descomprime en una carpeta temporal **sin dejar salir ningún archivo** (protección contra zip slip). Se **arranca el Java** para comprobar su versión real y, si coincide, queda en `%LOCALAPPDATA%\MercHosting\java\jdk-…-jre\`.
5. **No se instala nada en Windows** (ni .msi, ni registro, ni PATH). Los Java de MercHosting aparecen en la lista junto a los que ya tenía la PC, con su versión real.
6. Queda anotado en la Actividad.

Para compañeros: instalar Java es cosa del dueño (se deniega por defecto).

Spigot necesita un JDK (se compila en la PC). El instalador baja JRE, así que para Spigot se mantiene el aviso de instalar un JDK.

## Plantillas (asistente de creación)
Cuatro plantillas con **valores reales** de `server.properties`. Todos se ven y se pueden cambiar en el asistente o después en Ajustes:

| Plantilla | Modo / dificultad | Jugadores | Además |
|---|---|---|---|
| Supervivencia con amigos | supervivencia / normal | 10, lista blanca | PvP, protección de spawn 16, visión 10, simulación 10 |
| Creativo | creativo / pacífica | 10, lista blanca | sin PvP, vuelo permitido, sin protección de spawn, visión 12, simulación 8 |
| Con mods | supervivencia / normal | 8, lista blanca | PvP, vuelo permitido (jetpacks y alas de mods), sin protección de spawn, visión 8, simulación 6; sube la memoria a 6 GB si la PC la tiene |
| Minijuegos | aventura / pacífica | 20 | PvP, vuelo permitido, sin protección de spawn, visión 8, simulación 6 |

Debajo se muestra en palabras qué más fija la plantilla, y hay un enlace para quitarla. Las claves que fija son solo las que el asistente ya valida (`pvp`, `allow-flight`, `spawn-protection`, `view-distance`, `simulation-distance`, además de las de siempre).

## Pruebas
- `merc-install`:
  - se elige el .zip del JRE de Windows x64 (no el JDK) con su SHA-256;
  - se extrae de forma segura y un zip con `../` no sale de su carpeta.
- Supervisor:
  - instalación completa con una API de Adoptium falsa: descarga, huella, extracción y prueba de la versión real;
  - no quedan restos de la descarga;
  - el nuevo Java aparece en «Buscar Java»;
  - una versión no válida se rechaza.
- UI:
  - la versión LTS adecuada;
  - sin Java compatible, se descarga, aparece en la lista y se elige;
  - las plantillas fijan sus valores, se pueden cambiar a mano y llegan al crear.

Captura: `docs/capturas/fase9c-plantillas.png`.
