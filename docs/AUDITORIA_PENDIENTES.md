# Auditoría: especificación frente a lo construido (30-10-2026)

Revisión completa de `docs/especificacion` (las 30 directivas y el apéndice técnico) contra el código actual. Estados:
- **PARCIAL**: existe, pero falta parte de lo pedido.
- **PENDIENTE**: no existe.
- **BLOQUEADO**: depende de un tercero.
- **DESVÍO**: decisión del propietario, documentada.

## Contenido (Fase 5)
| # | Qué pide la especificación | Estado | Qué hay hoy |
|---|---|---|---|
| 1 | **Separador de mods cliente/servidor**: clasificar los mods (solo cliente, servidor, ambos, desconocido) con evidencia y revisión, y moverlos a una **cuarentena reversible**, nunca borrarlos | HECHO | Mods → «Separar cliente/servidor»: clasifica todos los mods de la carpeta con evidencia (catálogo y ficha del jar), confianza y dependencias; los mueve a `mods-cuarentena/` (reversible, nunca sobrescribe, con el servidor apagado). Aviso en la lista de instalados. Ver `docs/SEPARADOR_DE_MODS.md`. |
| 2 | Navegador de **resource packs** | HECHO | Ajustes → Paquete de recursos → «Buscar en Modrinth»: busca paquetes para el Minecraft del servidor, muestra versiones (solo deja usar las compatibles) y pone el enlace oficial de Modrinth; el supervisor lo descarga, comprueba que es un paquete y calcula su SHA-1. CurseForge no (sus enlaces de descarga no son estables para que los baje el juego). |
| 3 | Copia previa antes de instalar o actualizar un mod | PARCIAL | El jar viejo va a la papelera. «Actualizar todos» ofrece copia completa, pero instalar uno solo no la hace. |
| 4 | Arrastrar y soltar JARs | HECHO | Suelta `.jar` (o `.zip` en Datapacks) desde el Explorador sobre Mods/Plugins/Datapacks, o cualquier archivo sobre Archivos (a la carpeta abierta). Solo con el servidor apagado, nunca sobrescribe e informa qué se copió, qué falló y qué se ignoró. Falta probarlo en Windows. |
| 5 | Permisos y rangos de **LuckPerms** cuando está instalado | HECHO (LUCKPERMS.md) | Falta probarlo con LuckPerms real. |
| 6 | Mohist | BLOQUEADO | Sus descargas oficiales respondían 0 bytes. Está documentado en FASE_3. |

## Experiencia (Fase 2: nunca se hizo completa)
| # | Qué pide | Estado |
|---|---|---|
| 7 | **Tutorial inicial** que se pueda saltar y repetir, **tutoriales por módulo** que resalten los controles reales, y un menú «Ayuda y tutoriales» | HECHO (FASE_2_BIENVENIDA_Y_TUTORIALES.md) |
| 8 | **Bienvenida en el primer inicio**: qué es el hosting local, recursos de la PC y seguridad | HECHO (FASE_2_BIENVENIDA_Y_TUTORIALES.md) |
| 9 | **Modo principiante / avanzado** | HECHO (FASE_2_BIENVENIDA_Y_TUTORIALES.md) |
| 10 | Textos preparados para traducción (i18n) | HECHO (TRADUCCION.md) | Inglés en Apariencia → Idioma · Language (o automático si Windows no está en español). Diccionario español→inglés (~2 900 textos de la interfaz y del supervisor) aplicado sobre lo que pinta React, sin traducir datos de la persona. Fechas y números según el idioma. Puede faltar algún texto de diálogos poco frecuentes. |

## Creación y administración (Fases 3 y 4)
| # | Qué pide | Estado | Qué hay hoy |
|---|---|---|---|
| 11 | Asistente con icono, keepInventory y elegir una versión concreta del loader o de la compilación | HECHO (SERVIDORES_CLONAR_ARCHIVAR.md) | El icono está en «MOTD e icono». |
| 12 | **Clonar** y **archivar** servidores | HECHO (SERVIDORES_CLONAR_ARCHIVAR.md) | — |
| 13 | Consola: hora y nivel de cada línea con filtro, comandos favoritos y confirmación de comandos peligrosos | HECHO | Niveles Info/Avisos/Errores (vanilla, Forge, Paper y trazas de Java) con contador y filtro; hora opcional; favoritos por servidor que rellenan la línea sin enviarla; confirmación con explicación para stop, op, deop, ban, whitelist off, kill/clear/kick con @a/@e, save-off, reload, fill/clone y forceload add. |
| 14 | Archivos: editor con colores, mover y copiar, extraer ZIP de forma segura | HECHO | Colores para .properties/.cfg/.ini, .toml, JSON/.mcmeta y YAML (el texto no cambia). Mover y copiar a otra carpeta con selector; nunca sobrescribe ni deja meter una carpeta en sí misma; la copia es atómica. Extraer .zip en una carpeta nueva: ignora rutas fuera, absolutas, enlaces y nombres no válidos en Windows; límite de 50 000 entradas y 8 GB; todo o nada. |
| 15 | Jugadores: mensajes, modo de juego, teletransporte, última conexión y tiempo jugado, notas privadas | HECHO (JUGADORES.md) | El historial empieza desde que MercHosting vigila el servidor. |

## Operaciones (Fases 6 y 9)
| # | Qué pide | Estado | Qué hay hoy |
|---|---|---|---|
| 16 | **Reinicio automático tras un crash** con espera y tope (sin bucles) | HECHO (AUTOMATIZACION.md) | — |
| 17 | **Arrancar con Windows** (opcional, con permiso) | HECHO (AUTOMATIZACION.md) | Falta probarlo en Windows. |
| 18 | **Modo PC compartida**: avisar si el anfitrión también juega en la misma PC | PENDIENTE | — |
| 19 | **Paquete de soporte** con registros sin IPs ni claves | PENDIENTE | — |
| 20 | Alertas de túnel caído, copia fallida y actualización disponible | HECHO (ALERTAS.md) | La actualización de la propia app llega con la Fase 14. |
| 21 | Copias: carpeta de destino elegible (e incrementales cuando sea seguro) | PENDIENTE | — |
| 22 | **Pack para amigos**: manifiesto del cliente con enlaces oficiales | HECHO (PACK_PARA_AMIGOS.md) | Los mods de CurseForge se listan para bajar a mano (el formato no admite sus descargas). |
| 23 | Actualizar modpacks y la propia app | HECHO (ACTUALIZAR_MODPACK.md, PUBLICAR_VERSION.md) | Actualizaciones → Modpack: compara versión instalada, disco y versión nueva; respeta configs editados, mods quitados/desactivados y archivos propios; copia completa previa, descarga verificada fuera del servidor y lo viejo a la papelera. Solo misma versión de Minecraft y loader; el loader no se cambia (se avisa y se pide confirmar). Servidores anteriores: «Vincular un modpack». Falta probarlo con un modpack real. |

## Desvíos decididos por el propietario
- **Sin cuentas** (nombre y contraseña) ni servidor de identidad. Los compañeros se unen con un código de invitación de un solo uso y un canal P2P cifrado.
- **Supporter ligado a la clave de Gumroad**, no a una cuenta.
- **Sin votaciones de estética ni marcos de avatar**: necesitarían un servidor o cuentas.

## Fases que siguen (según ORDEN_DE_CREACION)
- 11: web oficial.
- 12: «Acerca de», redes, privacidad y changelog dentro de la app.
- 13: QA y pruebas con dos PCs reales.
- 14: instalador, firma y actualización segura.
- 15: publicación y cierre.

## Sin probar todavía en Windows real
- Playit e iroh entre dos PCs.
- Descarga real de Temurin.
- El icono en la barra de tareas.
- Varios flujos marcados como «pendiente de probar en Windows» en cada FASE_*.md.
