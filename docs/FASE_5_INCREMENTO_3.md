# Fase 5 · incremento 3: CurseForge completo («Próximamente» hasta tener la clave)

Todo lo de CurseForge está construido. Mientras esta instalación no tenga la **clave de API
oficial**, se muestra como **«Próximamente»**: visible, desactivado y con su explicación. Así nadie
piensa que ya funciona y se ve lo que viene.

## Dónde aparece
- **Mods → Buscar**: selector `Modrinth | CurseForge`. Sin clave, CurseForge está desactivado con
  la etiqueta «Próximamente» y el texto «CurseForge llegará próximamente (falta la clave oficial
  de MercHosting)». Los plugins no muestran CurseForge (sus plugins son casi todos antiguos).
- **Crear servidor → Modpack**: tarjetas «Modpack de Modrinth» y «Modpack de CurseForge». Sin
  clave, la de CurseForge está desactivada con «Próximamente».
- El supervisor también rechaza cualquier petición a CurseForge sin clave, con el mismo mensaje.

## Modpacks de CurseForge (listos para cuando haya clave)
- Búsqueda en la clase «Modpacks» de Minecraft y lista de archivos de cada modpack.
- Se descarga el ZIP del modpack (verificado con su SHA-1) y se lee `manifest.json`: versión de
  Minecraft, loader exacto (`forge-…`, `neoforge-…`, `fabric-…`, `quilt-…`) y archivos.
- Los archivos se resuelven con la API (en lotes de 100):
  - solo se instalan **mods**; los resource packs y shaders son para el juego de cada jugador;
  - se saltan los archivos que CurseForge marca solo para cliente;
  - si el autor **no permite descargas desde otras apps**, no se descarga: se avisa con el nombre
    y la página del mod para bajarlo a mano y subirlo en Archivos › mods.
- Luego el mismo proceso que con Modrinth: todo se descarga y se verifica antes de tocar la
  carpeta, se instala el loader exacto y se copian los extras (`overrides`). No se copia la EULA ni
  `server.properties`. Si algo falla, la carpeta queda como estaba.

## Activar CurseForge cuando tengas la clave
Ver `docs/FASE_5_INCREMENTO_1.md` (archivo `curseforge-api-key.txt` en la carpeta de datos, o
variable `MERC_CURSEFORGE_API_KEY` al compilar). La app lo detecta sola: desaparece
«Próximamente».

## Protocolo (compatible con v1)
- `modpack_search`, `modpack_project` y `modpack_inspect` aceptan `source`.
- `create_server` acepta `modpack_source`.
- `creation_defaults` responde `curseforge` (si hay clave).
- Todos son opcionales y por defecto valen Modrinth.

## Pruebas
- Rust: 149. Nuevas:
  - manifiesto de CurseForge e identificadores de loader;
  - de punta a punta con una API de CurseForge falsa: crear un servidor Fabric desde un modpack
    con un mod normal, uno que el autor no deja distribuir (avisado, no descargado) y un resource
    pack (no instalado), más los extras.
- Interfaz: 71. Nuevas: «Próximamente» en Mods y en el asistente sin clave; con clave, el flujo
  completo usa CurseForge; los plugins no muestran CurseForge.
- Capturas: `docs/capturas/fase5c-*.png`.
