# Actualizar el modpack de un servidor

Dónde está: **Actualizaciones → Modpack** (servidores con mods).

## Cómo sabe MercHosting qué modpack hay

- Los servidores **creados desde un modpack con la 0.2.4 o posterior** guardan, en la carpeta de datos de MercHosting (fuera del servidor, `instancias/<id>/modpack.json`), qué modpack y versión se instaló y la huella SHA-1 de cada archivo que puso: las descargas (mods) y los `overrides` (configs, scripts…).
- Los servidores anteriores o importados se pueden **vincular**: eliges el modpack (Modrinth o CurseForge) y la versión que tienes. Solo se acepta si es para el mismo Minecraft y el mismo loader que el servidor. Vincular no cambia ningún archivo.
- «Olvidar este modpack» borra ese registro; no toca el servidor.

## Qué versiones se ofrecen

Solo las que son para **la misma versión de Minecraft y el mismo loader**. Cambiar de versión de Minecraft en un servidor existente puede romper el mundo: para esas, se recomienda crear un servidor nuevo desde el modpack.

Si la versión nueva pide **otra versión del loader** (p. ej. NeoForge 21.1.80 en vez de 21.1.77), MercHosting lo dice y pide confirmarlo: el loader **no** se cambia en un servidor existente (su instalador reescribe librerías y scripts). Si algún mod necesita el loader nuevo, el servidor no arrancará y habrá que restaurar la copia previa.

## Qué cambia (se ve antes de tocar nada)

Cada archivo se compara en tres puntos: lo que puso la versión instalada, lo que hay ahora en el disco y lo que trae la versión nueva.

| En el servidor | La versión nueva | Qué pasa |
|---|---|---|
| Igual que lo puso el modpack | Lo trae distinto | **Se actualiza** |
| Igual que lo puso el modpack | Ya no lo trae | **Se quita** (a la papelera de MercHosting) |
| Lo cambiaste tú (p. ej. un config) | Lo trae o no | **Se conserva tu versión** |
| Lo quitaste o desactivaste tú (también si está en `mods-cuarentena`) | Lo trae | **No se vuelve a poner** (ni la versión nueva del mismo proyecto de Modrinth) |
| Hay un archivo tuyo en esa ruta | Lo trae | **Se conserva el tuyo** |
| No existe | Lo trae | **Nuevo** |

`server.properties` y `eula.txt` nunca se tocan. Los mods que añadiste tú en otras rutas no se tocan.

## Cómo se aplica

1. Requiere el **servidor apagado**.
2. **Copia completa y verificada** del servidor (en Copias de seguridad, «Antes de actualizar el modpack a …»). Si no se puede hacer, no se cambia nada.
3. Se descarga lo nuevo **fuera del servidor**, desde los enlaces oficiales del modpack, y se comprueba la huella de cada archivo. Si algo no coincide, no se cambia nada.
4. Lo que se quita o se reemplaza va **junto** a la papelera de MercHosting, en una carpeta `modpack-anterior-<versión>`. Restaurarla desde «Papelera y copias» la pone en el servidor con los archivos viejos dentro.
5. Se copian los archivos nuevos y se guarda el registro de la versión nueva.
6. Queda en la Actividad (`modpack.update`).

Para volver atrás del todo: restaura la copia previa en Copias de seguridad.

## Límites

- El loader no se actualiza (ver arriba).
- Los mods de CurseForge cuyo autor no permite descargas desde otras apps no se pueden bajar: se avisa como al crear.
- No se ha probado todavía con modpacks reales grandes en Windows.
