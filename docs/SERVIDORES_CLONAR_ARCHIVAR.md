# Servidores: clonar, archivar, versión del loader y keepInventory

## Clonar
En el Resumen del servidor › Detalles, botón **«Clonar»**. Solo con el servidor detenido, así el mundo queda guardado del todo.
- Copia todo (mundo, mods, plugins y ajustes) a una **carpeta nueva junto a la original**: «Nombre (copia)», o «(2)», «(3)»… sin pisar nada.
- No se copian `logs/`, `crash-reports/`, los candados de sesión (`session.lock`) ni los enlaces simbólicos.
- La copia recibe **otro puerto** de Minecraft: uno libre, que no usa otro servidor y que se puede abrir. Si tiene Simple Voice Chat con puerto propio, también recibe **otro puerto de voz**. Así pueden estar encendidos a la vez.
- Es un trabajo con progreso real (bytes copiados) y se puede detener. **Si falla o se cancela, se borra la copia a medias.**
- Al terminar, se abre la copia. Queda en la Actividad de los dos servidores.

## Archivar
Botón **«Archivar»** en Detalles, también con el servidor detenido.
- Un servidor archivado **no se puede encender**: ni desde la app, ni desde tareas programadas, ni por compañeros.
- En la lista aparece aparte, en «Archivados (n)».
- La carpeta **no se toca**.
- «Desarchivar» lo devuelve a la normalidad.

## Asistente de creación
- **Versión del loader**: para Fabric, Quilt, Forge y NeoForge se listan las versiones publicadas por la fuente oficial, con las beta marcadas. Por defecto se usa «Recomendada».
  - Forge solo publica la recomendada y la última de cada versión de Minecraft, y se ofrecen esas.
  - Paper y Purpur usan su última compilación estable. Para cambiarla está el centro de Actualizaciones.
- **Conservar el inventario al morir (keepInventory)**: es una regla del juego y no existe hasta que hay mundo. Se guarda y el supervisor la aplica **sola, la primera vez que el servidor queda en línea** (con el nombre correcto según la versión, `keep_inventory` desde 1.21.11). Queda en la Actividad.
- Las reglas se validan antes de empezar a crear: una regla inventada se rechaza.

## Pruebas
- **Rust**: listas de versiones (Fabric, Quilt, NeoForge, Forge); clonado que omite registros y candados, no pisa carpetas y limpia al cancelar.
- **Rust, de punta a punta** con el supervisor real: crear con keepInventory (aplicada al quedar en línea); clonar (carpeta nueva, otro puerto, sin `logs/`); archivar (no enciende) y desarchivar; regla inventada rechazada.
- **Interfaz**: elegir la versión de Fabric y keepInventory; clonar y abrir la copia; archivar, lista aparte y desarchivar.
