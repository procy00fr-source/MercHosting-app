# Fase 9 · incremento 2: Centro de seguridad e invitación de jugadores

## Seguridad (pestaña nueva, solo para el dueño)
Comprobaciones con datos reales del servidor. Cada una se marca como **Peligro**, **Revisar**, **Para saber** o **Bien**, con una explicación y, si se puede, un arreglo a un clic que se confirma antes.

| Comprobación | De dónde sale | Arreglo |
|---|---|---|
| Cuentas oficiales (`online-mode`) | `server.properties` | Activarlo |
| Expuesto a Internet | Túneles de Playit hacia su puerto. Los puertos abiertos a mano en el router no se pueden ver, y así se dice. | — |
| Lista blanca si está expuesto, y `enforce-whitelist` | `server.properties` | Ir a Jugadores / activarlo |
| RCON (peligro si la contraseña tiene menos de 12 caracteres) y Query | `server.properties` | Desactivarlos |
| Compañeros con acceso y sus permisos de alto riesgo | Archivo de compañeros del anfitrión | Ir a Compañeros |
| Intentos denegados de compañeros (7 días) | Actividad | Ver la actividad |
| Última copia y copias automáticas | Copias y tareas programadas | Ir a Copias / Tareas |
| Claves dentro de la carpeta del servidor (`playit.toml`, llave de la PC, clave de CurseForge…) | Se comprueba si existen | Ir a Archivos |

- La pestaña no aparece para compañeros: muestra datos del dueño (sus compañeros, sus claves, su exposición). Del lado del anfitrión la petición se deniega por defecto.
- Los arreglos de ajustes usan la misma función que Ajustes, que guarda antes una copia de `server.properties`.

## Invitar jugadores (pestaña nueva en Juego)
Un mensaje listo para copiar con:
- **Dirección**. Por orden: las de Internet ya probadas, las de Internet sin probar (con aviso, y **no se puede copiar** hasta probarla en Conexión) y la de la red local (marcada «solo desde mi misma red»).
- **Versión de Minecraft y loader** (Fabric Loader, Quilt Loader, Forge o NeoForge, con su versión) cuando hace falta en el juego.
- **Mods que necesitan en su juego**, según el dato `client_side` de Modrinth: necesarios, opcionales, o «sin datos» (se avisa para que la persona los revise; no se adivina). Para cada mod se incluye el **enlace a su página oficial**, nunca el archivo, para no redistribuir contenido sin permiso.
- **Simple Voice Chat**, si el servidor lo tiene: opcional para hablar.
- **Lista blanca**: si está activada, el mensaje pide el nombre de Minecraft y la pestaña permite añadirlo ahí mismo.

La invitación **no da acceso a administrar**: eso son los compañeros, y así se explica.

Para esto, lo instalado ahora guarda además `client_side` y `page_url` (Modrinth; en CurseForge, la página) al reconocer cada archivo.

## Pruebas
- `merc-admin`: servidor público arriesgado (todas las alertas, con el peligro primero y su arreglo) y servidor privado tranquilo (todo en «Bien»).
- Supervisor, con datos reales: `online-mode=false`, RCON sin contraseña y `playit.toml` dentro del servidor salen como peligro; sin copias sale como aviso; al hacer una copia de verdad, pasa a «Bien».
- UI:
  - seguridad, con contador, arreglo confirmado y saltos a Copias y Compañeros;
  - el mensaje de invitación con dirección probada, NeoForge, el mod necesario con su enlace (no el que es solo del servidor) y el aviso de los mods sin datos;
  - añadir a la lista blanca;
  - una dirección sin probar no se puede copiar.

Capturas: `docs/capturas/fase9b-seguridad.png` y `fase9b-invitar.png`.
