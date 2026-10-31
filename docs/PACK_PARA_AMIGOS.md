# Pack para amigos

Cubre la fila 22 de `AUDITORIA_PENDIENTES.md` y el punto 21 de `17_FUNCIONES_EXTRA`: un manifiesto del cliente con versiones, huellas y enlaces oficiales, sin redistribuir mods.

Está en **Invitar jugadores → Pack para amigos**. Solo aparece en servidores con mods y solo en la app del dueño, porque guarda un archivo en su PC.

## Qué es

Un `.mrpack`: el formato de modpacks de Modrinth, `formatVersion` 1. Tus amigos lo abren con un launcher que importe modpacks de Modrinth, por ejemplo Modrinth App o Prism Launcher. Ese launcher instala la versión de Minecraft, el loader y los mods.

Dentro solo hay `modrinth.index.json`. Cada mod es:

- `path`: `mods/<archivo>`;
- `downloads`: el enlace oficial de `cdn.modrinth.com` que da la API de Modrinth para ese archivo exacto;
- `hashes`: SHA-1 y SHA-512;
- `fileSize`;
- `env`: client y server, según lo que declara el autor en Modrinth.

**No se copia ningún archivo de mod dentro del pack**, así que no se redistribuye nada. Los jugadores descargan cada mod de Modrinth y su launcher lo verifica con las huellas.

`dependencies` lleva `minecraft` y el loader, con la clave del formato y la versión exacta del servidor:

| Loader | Clave | Versión |
|---|---|---|
| Fabric | `fabric-loader` | versión del loader |
| Quilt | `quilt-loader` | versión del loader |
| Forge | `forge` | sin el prefijo `<mc>-` de su carpeta de librerías |
| NeoForge | `neoforge` | versión de NeoForge |

## Qué va y qué no

- **Va en el pack:** cada mod activo que está en Modrinth y cuyo autor no lo marca como «solo servidor», con un enlace de `cdn.modrinth.com` y SHA-512 conocidos. Los que el autor marca como opcionales en el juego van con `client: optional`.
- **A mano:** los mods de CurseForge (el formato solo admite descargas de Modrinth, GitHub y GitLab), los archivos sueltos y los que Modrinth no reconoce. Se listan con el motivo y, si se conoce, su página oficial.
- **Solo del servidor:** se nombran y no se incluyen.
- **No se puede hacer el pack** (se explica el motivo y no se ofrece guardar) si:
  - el servidor no usa mods;
  - es un híbrido de plugins y mods (Mohist o Arclight);
  - no se sabe la versión de Minecraft o del loader;
  - ningún mod se puede incluir.

## Cómo se arma

1. **Preparar el pack:** el supervisor lee la carpeta `mods`, reconoce cada archivo por su SHA-1 en Modrinth (`POST /version_files`) y en CurseForge si hay clave, y muestra el plan. Hace falta conexión con Modrinth; sin ella, se dice.
2. **Guardar el pack (.mrpack):** se elige dónde guardarlo. El supervisor vuelve a armar el plan con los datos de ese momento y escribe primero un archivo temporal, que después renombra. Queda en la Actividad como `friendpack.export`.

## Pruebas

- `merc-content` (`friendpack.rs`):
  - clave y versión del loader en cada plataforma;
  - qué va, qué va a mano y qué es solo del servidor;
  - se rechazan enlaces fuera de `cdn.modrinth.com`;
  - el pack escrito lo lee el mismo lector de `.mrpack` de MercHosting;
  - casos bloqueados.
- `merc-supervisord` (`daemon.rs`, `friend_pack_lists_official_links_and_writes_mrpack`):
  - con un Modrinth de prueba: un mod de Modrinth, uno solo de servidor y uno casero;
  - se rechaza un destino relativo;
  - el `.mrpack` guardado se lee y solo contiene el índice.
- Interfaz (`FriendPack.test.tsx`):
  - plan completo con página oficial y guardado;
  - pack bloqueado sin botón de guardar.
