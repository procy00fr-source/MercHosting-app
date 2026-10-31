# Fase 4 · incremento 3: mantenimiento, cancelar reinicio, progreso de mundos, más reglas, carpeta y borrar servidor

Con esto se cierra la fase 4 (administración local).

## Ver carpeta del servidor (Archivos)
Botón **Ver carpeta del servidor**: abre en el Explorador de Windows la carpeta que estás viendo
(la raíz o una subcarpeta). La ruta la da el supervisor a partir del servidor; la interfaz no puede
pedir abrir otras carpetas y se rechazan `..`, unidades y rutas absolutas.

## Quitar un servidor y borrar su carpeta
- «Quitar de la lista» sigue sin borrar nada por defecto.
- Nueva casilla **«También borrar la carpeta del servidor»**. Solo si la marcas, la carpeta va a la
  **Papelera de reciclaje de Windows** (no se borra definitivamente: se puede restaurar desde ahí).
  También van a la Papelera la papelera y las copias internas de MercHosting de ese servidor.
- Primero se manda a la Papelera y **solo si eso funciona** se quita de la lista.
- No se borra si: es la raíz de un disco o está demasiado arriba; contiene tu carpeta de usuario,
  Escritorio, Documentos, Descargas, OneDrive, los datos de MercHosting o la carpeta «Servidores»;
  comparte carpeta con otro servidor de la lista; es un enlace; o no parece un servidor (sin
  `server.properties`, `eula.txt` ni `.jar`).
- Si la carpeta es más grande de lo que admite la Papelera, Windows pide confirmación para borrarla
  definitivamente (indicador `FOF_WANTNUKEWARNING`); si dices que no, no se borra ni se quita nada.
  *Pendiente de comprobar en Windows.*

## Modo mantenimiento (pestaña Jugadores)
- Con el servidor encendido: **Activar mantenimiento** usa el comando oficial `whitelist on` y avisa
  por el chat. Solo entran los de la lista blanca (en Minecraft, los operadores normalmente también).
  Los conectados siguen jugando, salvo que tengas `enforce-whitelist=true` (se avisa en pantalla).
- **Terminar mantenimiento** deja la lista blanca como estaba antes (si ya estaba activada, sigue
  activada). Con el servidor apagado lo hace cambiando `white-list` en `server.properties` (con copia).
- El estado se guarda en `%LOCALAPPDATA%\MercHosting\instancias\<id>\mantenimiento.json`, así que
  sobrevive a cerrar la app o reiniciar el supervisor. Un aviso azul lo muestra en todas las pestañas.

## Cancelar un reinicio con aviso
Mientras corre la cuenta atrás se ve un aviso «El servidor se reiniciará en …» con **Cancelar
reinicio**. Al cancelar se avisa por el chat («Reinicio cancelado») y el servidor sigue encendido.

## Progreso de mundos
Duplicar, exportar e importar muestran una barra con lo copiado (MB de MB) y **Cancelar**. Al
cancelar se deshace lo hecho a medias (carpeta nueva, .zip temporal). Mientras dura no se puede
encender el servidor ni empezar otra operación de mundos.

## Más reglas del juego (21 en total, agrupadas)
Nuevas: PvP, porcentaje que debe dormir, daño por congelarse, clima, aparición de criaturas, patrullas,
comerciante errante, objetos de criaturas y de bloques, mensajes de muerte. Equivalencias de nombre
antiguo → nuevo (25w44a / 1.21.11) contrastadas en varias fuentes públicas que coinciden (tablas de
reglas de 26.x y el mod «Classic Gamerule», que mapea los nombres viejos a los nuevos). Las reglas que no existen en tu versión (por ejemplo `pvp` en versiones antiguas) no se
muestran; se indica cuántas no respondieron.

## Protocolo (compatible con v1)
`admin_state`, `set_maintenance`, `cancel_restart`, `cancel_world_op`; `unregister` acepta
`delete_files` (por defecto `false`). Ejemplos en `docs/contratos/ipc-v1-ejemplos.json`.

## Pruebas
- Rust: 128. Nuevas: progreso y cancelación de duplicar/exportar/importar (sin restos), rutas del
  botón de carpeta, y una prueba de punta a punta con el supervisor real: mantenimiento (comandos
  `whitelist on/off`), reinicio avisado cancelado, carpeta que no parece servidor (no se borra) y
  borrado a una papelera de prueba.
- Interfaz: 59 (6 nuevas). Capturas: `docs/capturas/fase4c-*.png`.
- No probado todavía en Windows: la Papelera de reciclaje real y el botón del Explorador.
