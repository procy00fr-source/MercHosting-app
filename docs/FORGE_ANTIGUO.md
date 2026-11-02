# Forge antiguo (1.12.2 a 1.16.5)

MercHosting crea servidores Forge de **1.12.2** y de **1.13 a 1.16.5**, además del Forge moderno (1.17 en adelante).

## Cómo se instala

- Se descarga el instalador oficial `forge-<mc>-<build>-installer.jar` del repositorio Maven de Forge (`maven.minecraftforge.net`), el mismo que usa Forge moderno. La huella se comprueba con el `.sha256` o `.sha1` que publique el repositorio; si no publica ninguna, se descarga por HTTPS y se avisa.
- Se ejecuta con `--installServer` dentro de la carpeta nueva, **con Java 8**. El instalador descarga el servidor de Mojang y las librerías (necesita Internet).
- Al terminar, MercHosting **busca** el JAR que dejó el instalador (`forge-<mc>-<build>.jar` o `forge-<mc>-<build>-universal.jar`; no se supone el nombre) y comprueba que es ejecutable (`Main-Class` en su manifiesto). Si no lo encuentra o no es ejecutable, la creación falla con ese mensaje y la carpeta se limpia.
- El servidor se arranca con `java -jar forge-….jar nogui`.

## Java 8

- Forge 1.16.5 o anterior solo funciona con Java 8: el asistente solo ofrece Java 8 y, si no lo hay, ofrece descargar Eclipse Temurin 8.
- Al crear, el supervisor rechaza otro Java con un mensaje claro.
- Si después se cambia a un Java más nuevo, el Diagnóstico lo avisa (`java_new_legacy_forge`) con el botón para elegir Java 8.

## Versiones que se ofrecen

- **1.12.2**: solo compilaciones de la rama final **14.23.5.x**, que es la que se usa hoy.
- **1.13 a 1.16.5**: las que publica Forge.
- **No se ofrecen versiones anteriores a 1.12.2** (1.7.10, 1.8.9, 1.10.2, 1.11.2…): sus instaladores dependen de direcciones de descarga antiguas que ya no responden de forma fiable.

## Importar

Una carpeta con `forge-<mc>-<build>[-universal].jar` se reconoce como Forge antiguo y se arranca con ese JAR. Esto pasa siempre que no haya un archivo de argumentos de Forge moderno (`win_args.txt` o `unix_args.txt`), porque en ese caso manda el Forge moderno.

## Mods

- Los mods de esa época traen `mcmod.info` en vez de `mods.toml`. MercHosting lo lee para saber el ID, el nombre y la versión.
- `mcmod.info` no dice si un mod es solo de cliente ni sus dependencias reales. Por eso el separador de mods se apoya en los datos de Modrinth para clasificarlo y no inventa un lado.
- El buscador de mods filtra por la versión del servidor (1.12.2) y por Forge.

## Qué no se verificó desde el entorno de desarrollo

No se pudo descargar desde ahí el instalador real ni su `.sha1` (red bloqueada). Las pruebas usan un instalador simulado que deja el mismo tipo de archivos.

Antes de publicar conviene crear un servidor 1.12.2 de verdad en Windows y comprobar tres cosas:

1. que el instalador termina y deja `forge-1.12.2-14.23.5.xxxx.jar`;
2. que el servidor llega a «Done»;
3. que un mod de la época, por ejemplo Scape and Run: Parasites, carga.
