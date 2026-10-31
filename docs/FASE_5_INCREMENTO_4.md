# Fase 5 · incremento 4: Hangar, datapacks y paquete de recursos

Con esto la Fase 5 (contenido) queda completa.

## Plugins de Hangar (Paper y Purpur)
- **Plugins → Buscar**: selector `Modrinth | Hangar`. Hangar es el catálogo oficial de plugins de
  PaperMC. Solo aparece en Paper y Purpur (Purpur carga los plugins de Paper); Spigot e híbridos
  siguen solo con Modrinth.
- API pública v1 (`https://hangar.papermc.io/api/v1`), sin cuenta ni clave. Se consultó el código
  del propio Hangar para los parámetros: búsqueda `q`, `platform=PAPER`, `version` y `sort`
  (`-downloads`, `-updated`, `-newest`); versiones por `platform` y `platformVersion`.
- Cada descarga se verifica con el **SHA-256** que publica Hangar (Modrinth y CurseForge usan
  SHA-1). Si no coincide, no se toca el servidor.
- Si el autor publica el archivo en **otra web** (`externalUrl`), no se descarga desde aquí: se
  avisa para bajarlo desde su página y subirlo en Archivos › plugins.
- Dependencias: las que están en Hangar se resuelven e instalan igual que en Modrinth; las que
  están fuera (por ejemplo Vault en GitHub) se avisan por su nombre.
- **Lo instalado** se reconoce por SHA-256 (`/versions/hash/…`) solo para lo que Modrinth no
  reconoció, con nombre, icono, versión y aviso de actualización.
- **Límite de peticiones**: Hangar permite unas 20 cada 5 segundos. Por eso se consultan como
  mucho 8 archivos nuevos por listado y las respuestas se guardan 10 minutos en memoria. Con
  muchos plugins sin reconocer, los demás se completan al volver a abrir la pestaña.

## Datapacks (todos los servidores, también Vanilla)
- Nueva pestaña **Datapacks**: los `.zip` de `<mundo activo>/datapacks` (el `level-name` de
  `server.properties`; si el nombre no es seguro se usa `world`).
- Búsqueda en Modrinth (loader `datapack`), instalación verificada, desactivar (renombrar a
  `.zip.disabled`, reversible) y quitar (va a la papelera de MercHosting).
- Cada mundo tiene sus datapacks: si cambias de mundo en Mundos, ves los del otro. Los datapacks
  que son carpetas (no `.zip`) se gestionan en Archivos.
- Si el mundo todavía no existe, la carpeta se crea y Minecraft los usa al generar el mundo.

## Paquete de recursos del servidor
- **Ajustes → Paquete de recursos**: pegas el enlace de descarga directa de un `.zip` (HTTPS) y
  marcas si es obligatorio.
- El supervisor lo **descarga una vez** para comprobar que es un paquete de recursos de verdad
  (`.zip` con `pack.mcmeta` en la raíz, máximo 250 MB) y calcula su **SHA-1**. Luego escribe
  `resource-pack`, `resource-pack-sha1` y `require-resource-pack` con copia previa del archivo.
- «Quitar paquete» deja las tres opciones vacías.
- Esas tres opciones ya no aparecen en «Avanzado», para no editarlas a medias.
- El archivo tiene que seguir en esa dirección: MercHosting no lo aloja.

## Protocolo (compatible con v1)
- `ContentKind` admite `datapack`; `ContentSource` admite `hangar`.
- Los archivos de versión y los elementos del plan tienen `sha256` (opcional).
- Nueva petición `set_resource_pack { id, url, required }` → responde `properties`.
- Los modpacks rechazan `hangar` (no tiene modpacks).

## Pruebas
- Rust: 156. Nuevas:
  - lectura de proyecto y versión de Hangar (enlaces, dependencias externas, archivo externo);
  - de punta a punta con un Hangar falso: buscar, plan con dependencia, instalar verificando
    SHA-256, rechazar un archivo alterado, bloquear (con explicación) uno publicado en otra web, reconocer lo instalado y su actualización, y no repetir
    consultas gracias a la caché;
  - datapack de Modrinth instalado como `.zip` en el mundo (y un `.jar` rechazado);
  - plataformas: Vanilla con datapacks, carpeta según `level-name`, Hangar solo en Paper/Purpur;
  - paquete de recursos en el supervisor: una página web se rechaza sin cambiar nada; un `.zip`
    válido guarda dirección y SHA-1; quitarlo vacía las opciones.
- Interfaz: 75. Nuevas: pestaña Datapacks en Vanilla, búsqueda en Hangar en Paper, tarjeta del
  paquete de recursos (HTTPS obligatorio, guardar, quitar).
- No se probó contra el Hangar real desde este entorno (sin acceso a la red externa): conviene
  buscar e instalar un plugin de Hangar en Windows.
