# Jugadores: acciones, historial y notas

Este documento cubre la fila 15 de `AUDITORIA_PENDIENTES.md`. Lo que ya existía (lista blanca, OP, kick, ban y perdón) no cambia.

## Acciones con un jugador conectado

En la pestaña **Jugadores**, cada jugador conectado tiene un botón **Más…**. Abre una ventana con tres acciones:

| Acción | Comando que manda MercHosting | Límites |
|---|---|---|
| Mensaje privado | `tell <jugador> <texto>` | Una sola línea de 1 a 200 caracteres, sin caracteres de control. |
| Modo de juego | `gamemode <survival\|creative\|adventure\|spectator> <jugador>` | Cada modo se explica en la ventana. |
| Teletransportar a otro jugador | `tp <jugador> <destino>` | El destino tiene que ser un nombre válido. La lista solo muestra jugadores conectados. |
| Teletransportar a coordenadas | `tp <jugador> X Y Z` | Solo números enteros. X y Z no pueden pasar de ±30 000 000, ni Y de ±4096. |

- El comando lo arma el supervisor a partir de un tipo cerrado (`PlayerCommand`). La interfaz no manda texto libre a la consola.
- El supervisor valida dos condiciones. Si alguna no se cumple, rechaza la acción con un mensaje claro:
  - el servidor tiene que estar en línea;
  - el jugador tiene que estar conectado en ese momento.
- Cada acción queda en la auditoría como `player.command`. Del mensaje privado se registra que se mandó, pero **no** se guarda el texto.
- Permisos de los compañeros: necesitan el permiso **Jugadores** para usar las acciones, ver el historial y editar notas. Borrar el historial está reservado al dueño. Esto sigue la regla de negar por defecto de `merc_ipc::remote::need()`.

## Historial de conexiones

- Archivo: `instancias/<id>/jugadores.json`.
- Cómo se llena:
  - El ciclo de salud, que corre cada 5 s, compara la lista de jugadores conectados con la vuelta anterior.
  - Cuando alguien entra, se abre una sesión y se cuenta como una conexión.
  - Cuando sale, se suma el tiempo de esa sesión.
- Para cada jugador se muestran:
  - primera vez visto;
  - última conexión (o «Conectado» si está en el servidor ahora);
  - número de sesiones;
  - tiempo jugado total;
  - nota.
- El historial solo cubre lo que MercHosting vio con el servidor en marcha. **No** lee registros antiguos, así que no inventa datos de antes.
- Tope de 1000 jugadores. Si se pasa, se descartan primero los más antiguos que no tengan nota.
- **Borrar historial y notas** pide confirmación y no toca el mundo ni las listas del servidor.

## Notas privadas

- Cada jugador del historial puede tener una nota de hasta 500 caracteres. Si guardás la nota vacía, se borra.
- Las notas solo se guardan en esta PC y el jugador nunca las ve.

## Pruebas

- `merc-admin` (`players.rs`): validación de comandos, límites de coordenadas, conteo de sesiones y tiempo jugado, notas.
- `merc-supervisord` (`daemon.rs`, `player_history_notes_and_commands`): el servidor de prueba simula entradas y salidas. La prueba revisa el historial, las notas, los comandos rechazados con el servidor apagado o el jugador desconectado, y el borrado.
- Interfaz (`PlayersExtra.test.tsx`): mensaje, cambio de modo, teletransporte a coordenadas con validación, tabla del historial, notas y borrado con confirmación.
