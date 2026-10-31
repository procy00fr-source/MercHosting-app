# Fase 6 · incremento 3: salud, alertas y modos de rendimiento

Con esto la **Fase 6 queda completa**: copias comprobadas y restauración (incremento 1), tareas
programadas (incremento 2) y salud/alertas/rendimiento (este).

## Salud y rendimiento (nueva sección de cada servidor)
Solo se muestra lo **medido**; lo que no se puede medir se dice.
- **Esta PC**: uso del procesador y de la memoria (los mide el supervisor cada 2 s; la pestaña se actualiza cada 2 s) y espacio
  libre en el disco del servidor y en el de las copias de seguridad, con barras que cambian de
  color al 80 % y al 92 %.
- **Servidor**: memoria del proceso frente a la asignada (`-Xmx`) y su procesador (cuando está
  encendido). Aviso: Java reserva memoria por adelantado, así que verla cerca del máximo no
  siempre significa que falte.
- **TPS**: el botón «Medir TPS» manda el comando del propio servidor y lee su respuesta:
  - Paper, Purpur, Spigot, Mohist y Arclight: `tps` (TPS de 1, 5 y 15 min; Purpur añade 5 s).
  - Forge: `forge tps`; NeoForge: `neoforge tps` (TPS medio y ms por tick).
  - Vanilla y Fabric no tienen ese comando: se dice que no está disponible; no se calcula un
    número.
- **Jugar y hostear en la misma PC**: si la memoria asignada al servidor supera la mitad de la de
  la PC, se avisa de que el juego y Windows pueden quedarse cortos; si no, consejos generales.
- **Modo de rendimiento**: Ligero, Equilibrado o Como Minecraft. Solo cambia opciones
  documentadas de `server.properties` (`view-distance`, `simulation-distance`,
  `entity-broadcast-range-percentage`). Antes de aplicar se ve una **vista previa** (ahora →
  nuevo); al aplicar se guarda copia del archivo y aparece **Deshacer el cambio**.
- **Alertas de este servidor**: las últimas 5.

## Alertas (campana en la barra)
- Las vigila el **supervisor** (revisa cada 5 s) aunque la ventana esté cerrada:
  - un servidor **se cierra inesperadamente** (grave), con el motivo si se conoce;
  - queda **poco espacio** en el disco de un servidor o de las copias (por defecto, menos de
    5 GB; se repite como mucho cada 6 h por disco);
  - la **memoria de la PC** pasa del 92 % durante 30 s seguidos (como mucho una vez
    por hora);
  - **falla una tarea programada** (por ejemplo, una copia).
- La campana muestra cuántas hay sin leer. Dentro: la lista, «Marcar todo como leído» y qué
  vigilar (cada tipo se puede apagar; umbral de disco en GB).
- **Notificaciones de Windows**: mientras MercHosting está abierto (también minimizado), cada
  alerta nueva sale como notificación. Con la ventana cerrada se acumulan y se ven al abrirla
  (las antiguas no se vuelven a notificar). Se pueden apagar en los ajustes de alertas. Usa el
  plugin oficial de notificaciones de Tauri.
- Se guardan las últimas 200 alertas.

## Protocolo (compatible con v1)
- Nuevas peticiones: `health`, `measure_tps`, `alerts`, `mark_alerts_read`,
  `set_alert_settings`; respuestas `health`, `tps`, `alerts`.

## Pruebas
- Rust: 168. Nuevas:
  - lectura del TPS de Paper (con colores y `*20.0`), Purpur, Forge y NeoForge; Vanilla y Fabric
    sin comando; archivo de alertas con tope;
  - en el supervisor, con un servidor Java de prueba que se presenta como Paper: salud con
    discos y memoria, TPS rechazado con el servidor apagado y medido encendido, ajustes de
    alertas validados, y una caída real que deja una alerta grave sin leer que luego se marca.
- Interfaz: 88. Nuevas: medidas de la PC y TPS medido; Vanilla sin TPS; modo de rendimiento con
  vista previa, aplicar y deshacer; aviso de memoria al jugar y hostear; campana con
  notificación de Windows solo de lo nuevo, marcar como leído, ajustes y notificaciones
  apagadas; las alertas viejas no se notifican al abrir.
- Capturas: `docs/capturas/fase6c-*.png`.

## Pendiente de probar en Windows
- La notificación de Windows real (el permiso lo pide Windows la primera vez).
- El TPS con un servidor Paper, Forge y NeoForge reales.
