# Alertas: túnel caído, copia fallida y actualización disponible

Cubre la fila 20 de `AUDITORIA_PENDIENTES.md` y el punto 18 de `17_FUNCIONES_EXTRA`.

Ya existían estas alertas:

- caída de un servidor;
- poco disco;
- memoria de la PC casi llena;
- tarea programada fallida;
- compañeros.

Este cambio añade tres alertas con detección real. Cada una se puede apagar en **Alertas → Qué vigilar**. Los ajustes guardados antes de este cambio se leen con las tres nuevas activadas.

| Alerta | Interruptor | Cuándo salta |
|---|---|---|
| **El túnel de Playit no funciona** (aviso) | `tunnel` | Antes MercHosting vio el túnel funcionando para un servidor encendido, y ahora el problema dura más de 90 s. Hay problema si el agente no responde, no está en marcha (no vinculado, clave rechazada, límite de la cuenta, error) o Playit desactivó el túnel de ese puerto. Avisa una vez por caída. |
| **El túnel de Playit volvió a funcionar** (info) | `tunnel` | Cuando se recupera tras haber avisado. |
| **Falló una copia** (aviso) | `backups` | Falla una copia automática: programada, previa a actualizar o previa a restaurar. El mensaje es el error real. Una copia manual que falla ya muestra su error en pantalla, así que no genera alerta. |
| **Hay una actualización** (info) | `updates` | El supervisor revisa los servidores no archivados que el Centro de actualizaciones sabe actualizar (Paper, Purpur, Fabric): primero 2 minutos después de arrancar y luego cada 6 horas. Avisa **una vez por versión nueva**: lo recuerda en `instancias/<id>/aviso-actualizacion.txt`. Sin conexión no avisa y lo vuelve a intentar en la siguiente vuelta. |

Notas:

- La comprobación del túnel usa lo que informa el agente oficial de Playit por su canal local, con los puertos de los servidores encendidos. Es la función `merc_net::playit::tunnel_check`. Si Playit nunca se configuró, o no hay servidores encendidos, no hay nada que vigilar y no salta ninguna alerta.
- Antes, una tarea de copia que fallaba daba «tarea fallida». Ahora da «copia fallida» y no las dos.
- La actualización de la propia app de MercHosting llega con la Fase 14 (instalador).

## Pruebas

- `merc-admin`: los ajustes antiguos se leen con las alertas nuevas activadas, y cada interruptor apaga solo la suya.
- `merc-net` (`tunnel_check_by_port`): túnel funcionando, otro puerto, agente que no responde, túnel desactivado por Playit, cuenta desvinculada.
- `merc-supervisord`:
  - `tunnel_down_alert_after_grace_and_recovery`: con el agente de Playit de prueba, el túnel deja de estar vinculado, avisa una sola vez tras la espera y, al volver, avisa de que volvió.
  - `scheduled_tasks_run_skip_and_miss`: una copia programada que falla da «copia fallida» y no «tarea fallida».
  - `create_paper_server_then_start_it`: aparece la compilación 50, avisa una sola vez y el mensaje dice «42 → 50».
- Interfaz (`Health.test.tsx`): los interruptores nuevos se guardan.
