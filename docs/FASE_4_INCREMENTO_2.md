# Fase 4 · incremento 2: mundos, reglas del juego, papelera, actividad y reinicio con aviso

## Mundos (pestaña nueva)
- Lista los mundos de la carpeta del servidor (cualquier carpeta con `level.dat`), con tamaño,
  fecha del último guardado y cuál está **activo** (`level-name`). En Paper/Spigot/Purpur,
  `<mundo>_nether` y `<mundo>_the_end` se muestran junto a su mundo y se copian/exportan con él.
- **Usar este mundo**: cambia `level-name` (con copia previa de `server.properties`).
- **Duplicar**: copia completa con otro nombre (no copia `session.lock`).
- **Exportar .zip**: a donde elijas. **Importar .zip**: busca el `level.dat` menos profundo, rechaza
  rutas que intenten salir de la carpeta, limita a 32 GB y 500 000 entradas, nunca pisa un mundo
  existente y, si algo falla, borra lo que alcanzó a extraer.
- **Mover a la papelera**: el mundo activo no se puede; los demás van a la papelera (no se borran).
- Todo esto pide el servidor **detenido**, para que el mundo esté guardado del todo.

## Reglas del juego
- 11 reglas con nombre claro: conservar inventario, mobs que modifican bloques, ciclo de día,
  regeneración, reaparecer sin pantalla de muerte, daño por caída/fuego/ahogo, phantoms, anunciar
  logros y velocidad de crecimiento (random tick, 0–4096).
- Se leen y cambian con el comando oficial `/gamerule` por la consola, así que piden el servidor
  **encendido**. Desde 1.21.11 Minecraft renombró las reglas (`keep_inventory`); MercHosting usa el
  nombre correcto según la versión. Solo se aceptan reglas de la lista y valores válidos.

## Papelera y copias (Archivos › «Papelera y copias»)
- Muestra lo que mandaste a la papelera y las versiones anteriores de archivos que editaste.
- **Restaurar** (servidor detenido): de la papelera, vuelve a su sitio; si ya hay algo con ese nombre
  no se pisa, se avisa. Una versión anterior reemplaza al archivo actual, pero antes se guarda el
  actual en las copias (se puede deshacer). **Borrar definitivamente** pide confirmación: es lo único que borra de verdad.

## Actividad (pestaña nueva)
Línea de tiempo de lo hecho con el servidor (encendidos, ajustes, jugadores, mundos, reglas,
archivos), leída de la auditoría local. Solo en tu PC.

## Reiniciar con aviso
«Reiniciar» ahora pregunta: **Reiniciar ahora** o **Avisar y reiniciar** (30 s, 1, 2 o 5 min).
El supervisor avisa por el chat (`say`) al empezar y en 5 min, 2 min, 1 min, 30, 10, 5, 3, 2 y 1 s y luego reinicia,
aunque cierres la ventana.

## Protocolo (compatible con v1)
`worlds`, `duplicate_world`, `export_world`, `import_world`, `set_active_world`, `trash_world`,
`saved_items`, `restore_saved`, `purge_saved`, `game_rules`, `set_game_rule`, `activity`,
`restart_with_notice`. Ejemplos en `docs/contratos/ipc-v1-ejemplos.json`.

## Pruebas
- Rust: 125. Nuevas: mundos (listar, compañeros nether/end, duplicar, exportar/importar ida y vuelta,
  zip malicioso con `..`, nombre ocupado), reglas (nombres viejos y nuevos, lectura de respuestas,
  valores inválidos), papelera (restaurar con conflicto, purgar). Punta a punta con el supervisor real
  y un servidor falso que responde a `/gamerule`: mundos, papelera, reglas, actividad y reinicio con aviso.
- Interfaz: 53 (9 nuevas). Capturas: `docs/capturas/fase4b-*.png`.

## Pendiente dentro de la fase 4
Modo mantenimiento (bloquear entradas nuevas), cancelar un reinicio con aviso ya iniciado, barra de
progreso para mundos muy grandes y más reglas del juego.
