# Fase 4 · incremento 1: jugadores, ajustes y archivos

Cada servidor tiene tres pestañas nuevas, además de **Exportar registro** en la consola.

## Jugadores
- Conectados (según la consola), lista blanca, operadores (con nivel), baneados (con motivo) e IPs
  baneadas, leídos de los archivos que escribe Minecraft (`whitelist.json`, `ops.json`,
  `banned-players.json`, `banned-ips.json`). Un archivo dañado se avisa; uno que falta = lista vacía.
- Acciones: añadir/quitar de la lista blanca, dar/quitar OP, expulsar, banear y perdonar. Se hacen
  con los **comandos oficiales** de Minecraft por su consola, así el propio juego resuelve el UUID y
  guarda sus archivos. Por eso piden el servidor encendido (se explica en pantalla).
- Dar OP, expulsar y banear piden confirmación; expulsar y banear permiten un motivo.
- El nombre se valida (letras, números y `_`, máximo 16) y el motivo es una sola línea: no se puede
  colar otro comando. Nunca se ejecuta nada del sistema operativo. Todo queda auditado.

## Ajustes (`server.properties`)
- Controles claros agrupados en General, Acceso, Mundo y Rendimiento (solo las opciones que tiene
  el archivo); el resto, tal cual, en **Avanzado**.
- Validación en la interfaz y en el supervisor (números en rango, opciones conocidas, sin saltos de
  línea; `level-name` no puede salir de la carpeta).
- Guardado seguro: se conservan comentarios y orden, los valores se escriben escapados como lo hace
  Java (`ñ`), se guarda antes una copia, se escribe con archivo temporal + renombrado y, si otro
  programa cambió el archivo desde que se abrió, no se pisa: se avisa y se ofrece recargar.
- Con el servidor encendido se puede guardar; se avisa de que se aplica al reiniciar.

## Archivos
- Explorador limitado a la carpeta del servidor: no se puede salir con `..`, rutas absolutas,
  nombres reservados de Windows ni enlaces/junctions (se muestran, pero no se abren).
- Ver archivos de texto (hasta 2 MB, UTF-8), editarlos, crear carpetas, renombrar, subir (hasta 2 GB)
  y descargar una copia a donde elijas.
- **Nada se borra**: «Mover a la papelera» lo lleva a `%LOCALAPPDATA%\MercHosting\instancias\<id>\papelera`.
  Antes de sobrescribir un archivo se copia a `...\<id>\copias`. Si la carpeta parece un mundo, se avisa.
- Con el servidor encendido solo se puede ver y descargar (Minecraft tiene los archivos abiertos).

## Protocolo (compatible con v1)
`properties`, `set_properties`, `players`, `player_action`, `list_files`, `read_file`, `write_file`,
`make_dir`, `rename_file`, `trash_file`, `upload_file`, `download_file`, `export_console`.
Ejemplos en `docs/contratos/ipc-v1-ejemplos.json`.

## Pruebas
- Rust: 109 (`merc-admin`: formato `.properties` con escapes, continuaciones y duplicados; listas de
  jugadores; comandos que no admiten inyección; explorador con copias, conflictos, papelera, subida,
  descarga y enlaces). Prueba de punta a punta con el supervisor real: cambia `server.properties`,
  rechaza valores inválidos y marcas viejas, usa la papelera, bloquea cambios con el servidor
  encendido, envía `op` por la consola y exporta el registro.
- Interfaz: 44 (8 nuevas). Capturas: `docs/capturas/fase4-*.png`.

## Pendiente (siguientes incrementos)
Gamerules, mundos (duplicar, exportar, importar), restaurar desde la papelera y las copias en la
interfaz, mantenimiento con aviso a jugadores y la actividad cronológica (la auditoría ya se guarda).
