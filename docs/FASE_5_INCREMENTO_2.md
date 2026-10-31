# Fase 5 · incremento 2: crear un servidor desde un modpack de Modrinth

En «Crear servidor» hay una opción nueva, **Modpack de Modrinth** (al final de la lista de tipos).

## Cómo funciona
1. **Buscar y elegir** un modpack de Modrinth y una de sus versiones.
2. MercHosting **descarga el `.mrpack`** y lo verifica con su SHA-1. Lo guarda en
   `%LOCALAPPDATA%\MercHosting\cache\modpacks` y lee su índice para saber:
   - la versión de Minecraft y el **loader exacto** que pide (NeoForge, Forge, Fabric o Quilt);
   - cuántos archivos son para el servidor y cuántos solo para el juego de cada jugador;
   - cuánto hay que descargar.
3. El asistente sigue igual que siempre: Java, nombre, carpeta, memoria (6 GB por defecto),
   puerto, EULA y resumen. Se instala **esa versión exacta del loader**, no la última.
4. Al crear, en este orden:
   1. Se descargan y verifican **todos los archivos del modpack** en una carpeta temporal, antes de
      tocar la carpeta del servidor.
   2. Se instala el servidor con el instalador oficial.
   3. Se copian los archivos del modpack y se extraen `overrides/` y `server-overrides/`
      (estos últimos tienen prioridad).
   4. Si algo falla, la carpeta queda vacía como estaba; si cancelas, también.

## Reglas de seguridad
- Solo se descargan archivos marcados para servidor. Lo «solo cliente» se salta y se avisa.
- Solo desde los dominios que permite el formato: `cdn.modrinth.com`, `github.com`,
  `raw.githubusercontent.com` y `gitlab.com`, siempre por HTTPS. Cada archivo se verifica con su SHA-1.
- Se rechazan rutas peligrosas en el índice (`..`, rutas absolutas, unidades) y los modpacks que
  piden dos loaders.
- Nunca se copia el `eula.txt` del modpack (la EULA la aceptas tú) ni su `server.properties` (se
  conservan los ajustes del asistente). Se avisa en el registro.
- Límites: 2000 archivos y 8 GB en `overrides`.

## Protocolo (compatible con v1)
`modpack_search`, `modpack_project` y `modpack_inspect`. `resolve` acepta `loader_version` y
`create_server` acepta `modpack_version`, los dos opcionales.

## Pruebas
- Rust: 147. Nuevas:
  - versión de loader fijada (Fabric y NeoForge, inexistente, ruta peligrosa);
  - lectura del índice (archivos de servidor, solo cliente, dominios, rutas, dos loaders);
  - `overrides` sin EULA ni `server.properties`;
  - de punta a punta con el supervisor real: inspeccionar el modpack, resolver el loader exacto,
    crear el servidor Fabric con el modpack, comprobar archivos, `overrides`, que no se copió lo
    solo cliente ni la EULA, y que no quedan temporales.
- Interfaz: 68 (1 nueva). Captura: `docs/capturas/fase5b-modpack.png`.
- **No probado contra Modrinth real** desde aquí: la primera prueba real es en tu PC.

## Pendiente (fase 5)
Modpacks de CurseForge (necesitan clave y usan *server packs*), Hangar para plugins de Paper,
datapacks y resource packs.
