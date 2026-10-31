# Fase 6 · incremento 2: tareas programadas

Nueva sección **Tareas programadas** en la barra de cada servidor. Las ejecuta el supervisor,
así que funcionan aunque cierres la ventana de MercHosting (la PC tiene que estar encendida).

## Qué se puede programar
- **Copia del mundo** o **copia completa** (igual que la manual: comprobada; con el servidor
  encendido pausa y reanuda el guardado). Quedan como «Programada» en Copias de seguridad y
  respetan la limpieza automática.
- **Reiniciar** o **apagar**, con aviso en el chat (sin aviso, 1, 2, 5 o 10 min) y qué hacer si
  hay jugadores conectados:
  - **No hacerlo esa vez** (por defecto): se anota como omitida, con los nombres de quienes
    estaban jugando;
  - **Avisarles y hacerlo igual**: cuenta atrás en el chat (al menos 30 s) y luego lo hace.
- **Encender** el servidor (si ya estaba encendido, se anota y no hace nada).
- **Comando de consola** (sin la barra). `stop`, `save-off`, `save-on`… no se pueden programar
  como comando: para eso están sus propias tareas, que avisan y respetan a los jugadores.
- **Mensaje en el chat** (`say`).
- Los comandos, mensajes, reinicios y apagados se saltan si el servidor está apagado.

## Cuándo
- **A ciertas horas**: una o varias horas (hasta 24) y los días de la semana que quieras
  (ninguno marcado = todos).
- **Cada cierto tiempo**: entre 5 minutos y 7 días.
- Las horas son las del **reloj de esta PC** (la pestaña muestra la zona, por ejemplo UTC−05:00).
- Si a la hora la PC o el supervisor estaban apagados o dormidos (más de 10 min de retraso), la
  tarea **no se ejecuta tarde**: se anota «No se ejecutó…» y se espera a la siguiente.
- Una tarea nunca se solapa consigo misma.

## En la lista
- Qué hace, cuándo, la próxima ejecución y el resultado de la última (Hecha, Omitida o Falló,
  con el motivo).
- Pausar/activar, **Ejecutar ahora**, editar y borrar (con confirmación).
- Sugerencias para empezar: «Copia diaria a las 04:00», «Reinicio diario con aviso» y «Anuncio
  cada hora» (se abren en el editor para revisarlas antes de guardar).
- Todo queda en Actividad: crear, cambiar, borrar, ejecutar (sola o a mano), omitida y falló.

## Protocolo (compatible con v1)
- Nuevas peticiones: `tasks`, `save_task`, `delete_task`, `run_task_now`; respuesta `tasks`.
- El aviso en el chat del reinicio programado usa la misma cuenta atrás que «Reiniciar con
  aviso», que ahora también sabe apagar.

## Pruebas
- Rust: 165. Nuevas:
  - próxima ejecución a ciertas horas, ciertos días y cada N minutos (zona horaria de Lima);
    validación de horas, días, intervalos y comandos prohibidos; tareas pasadas de hora;
  - en el supervisor, con un servidor Java de prueba y el programador acelerado: la tarea de
    encender se ejecuta a su hora y calcula la siguiente; la que pasó hace una hora se omite;
    «ejecutar ahora» de un comando; el reinicio «si no hay nadie» se omite con un jugador
    conectado; copia programada en caliente; pausar y borrar; todo en Actividad.
- Interfaz: 82. Nuevas: crear desde una sugerencia con días, pausar y ejecutar a mano; reinicio
  con aviso y política de jugadores cada 6 h (con validación); comando prohibido, editar y
  borrar.
- Capturas: `docs/capturas/fase6b-*.png`.

## Lo siguiente (Fase 6)
- Salud y alertas: CPU/RAM/disco de la PC y del servidor, caídas, poco espacio, copias o tareas
  fallidas, con notificaciones de Windows configurables.
