# Fase 6 · incremento 1: copias de seguridad comprobadas y restauración

Nueva sección **Copias de seguridad** en la barra de cada servidor.

## Hacer una copia
- **Copia del mundo**: el mundo activo (`level-name`) con su Nether y su End si están aparte
  (Paper, Spigot, Purpur). **Copia completa**: toda la carpeta del servidor.
- Nota opcional («Antes del evento», «Antes de mods nuevos»…).
- Con el **servidor detenido** la copia es exacta.
- Con el **servidor encendido**, MercHosting manda `save-off` (pausa el guardado automático) y
  `save-all flush`, y espera a que el juego diga «Saved the game». Copia y, pase lo que pase,
  manda `save-on`. Los jugadores siguen jugando.
  - Si el juego no confirma el guardado en 90 s, la copia se hace igual pero queda marcada:
    «Encendido y sin confirmar el guardado: puede tener cambios a medias». Nunca se presenta como
    consistente una copia que no lo es.
  - Si MercHosting perdió la consola (no puede mandar comandos), no copia en caliente: pide
    detener el servidor.
- No se copian `session.lock` ni las carpetas temporales de MercHosting ni enlaces simbólicos.
- Antes de empezar se comprueba que haya espacio en el disco.

## Comprobada siempre
- Cada copia es un `.zip` con un manifiesto (`merchosting-backup.json`): cada archivo con su
  tamaño y su **SHA-256**.
- Al terminar, la copia se **vuelve a leer entera** y se compara con el manifiesto. Si no
  coincide, se borra y se avisa: no queda una copia mala en la lista.
- **Verificar** repite esa comprobación cuando quieras (por ejemplo, meses después). Una copia
  dañada se marca en rojo y no se puede restaurar.

## Restaurar
- Solo con el servidor detenido.
- Antes de tocar nada, se hace una copia **«Antes de restaurar»** de lo que hay ahora (mismo
  alcance). Si no se puede hacer, no se restaura.
- Se verifica la copia elegida, se extrae a una carpeta temporal dentro del servidor comprobando
  cada huella, y solo al final se cambian las carpetas. Si algo falla, todo vuelve a su sitio.
- Lo que no está en la copia no se toca (por ejemplo, una copia del mundo no cambia mods ni
  ajustes).

## Guardar y limpiar
- Las copias se guardan **fuera** de la carpeta del servidor:
  `%LOCALAPPDATA%\MercHosting\instancias\<id>\backups` («Ver carpeta» la abre).
- **Limpieza automática**: se conservan las N copias más nuevas (10 por defecto). Al hacer una
  nueva se borran las más viejas que pasen de N. Las **protegidas** (candado) nunca se borran
  solas ni se pueden borrar sin quitar antes la protección. La copia «Antes de restaurar» no
  dispara la limpieza (podría borrar la que estás restaurando).
- Borrar una copia a mano pide confirmación.
- Todo queda en Actividad (crear, verificar, restaurar, borrar, proteger).

## Protocolo (compatible con v1)
- Nuevas peticiones: `backups`, `create_backup`, `verify_backup`, `restore_backup`,
  `delete_backup`, `protect_backup`, `set_backup_settings`; respuesta `backups`.
- `world_op` (progreso y cancelación) ahora también informa `backup`, `verify` y `restore`.

## Pruebas
- Rust: 161. Nuevas:
  - copia del mundo verificada (sin `session.lock`, con el Nether) y restauración que deja las
    carpetas vacías y no toca lo que no estaba en la copia;
  - copia dañada a propósito: la verificación lo detecta y la restauración no cambia nada;
  - limpieza: conserva las más nuevas y las protegidas; cancelar no deja nada a medias;
  - en el supervisor, con un servidor Java de prueba: copia en frío, copia en caliente
    (`save-off` → «Saved the game» → `save-on` en ese orden), restaurar encendido rechazado,
    restaurar detenido con copia previa, verificar, proteger, borrar y ajuste de retención.
- Interfaz: 79. Nuevas: copia con nota y restauración, copia en caliente sin poder restaurar,
  copia dañada, proteger/borrar y ajuste de cuántas conservar.
- Capturas: `docs/capturas/fase6a-*.png`.

## Lo siguiente (Fase 6)
- Tareas programadas: copias automáticas, reinicios con aviso, comandos y mantenimiento.
- Salud y alertas: CPU/RAM/disco, caídas, avisos de espacio y de copias fallidas.
