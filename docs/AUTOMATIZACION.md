# Automatización: reinicio tras caídas y arrancar con Windows

Están en cada servidor › Ajustes › «Si se cae y al iniciar Windows». Las dos vienen **apagadas**.

## Reiniciar solo si se cae
- Si el servidor se cierra sin que nadie lo pida (estado «Caído»), el supervisor lo vuelve a encender tras **10, 30 y 60 segundos**.
- **Tope anti-bucle**: si se cae **3 veces en 10 minutos**, deja de intentarlo hasta que alguien lo encienda a mano. La alerta de caída dice cuándo se reiniciará, o que dejó de hacerlo y que conviene mirar el Diagnóstico.
- No reinicia en estos casos:
  - el servidor está archivado;
  - alguien lo encendió o lo cambió durante la espera;
  - la opción se apagó.
- Cada reinicio automático queda en la Actividad (`server.auto_restart`).
- Los intentos se cuentan en memoria: un reinicio del supervisor empieza de cero.

## Arrancar con Windows
- **«Arrancar MercHosting con Windows»** (vale para todos los servidores): registra el supervisor en el inicio de sesión del usuario, en `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, valor `MercHosting`.
  - Arranca en segundo plano, sin ventana, con `--al-iniciar-windows`.
  - Es solo del usuario actual y no necesita permisos de administrador. Quitarlo borra ese valor.
- **«Encender este servidor al iniciar Windows»** (por servidor): al arrancar así, el supervisor enciende los servidores marcados. Se salta los archivados y los que no tienen la EULA aceptada, y lo deja anotado.
- La app no tiene que estar abierta: al abrirla después, se conecta al supervisor que ya está funcionando.
- Fuera de Windows, la opción aparece desactivada con su explicación.

## Pruebas
- **De punta a punta con el supervisor real**:
  - una caída sin la opción activada no se reinicia;
  - con la opción, 3 caídas seguidas se reinician solas;
  - la 4.ª ya no, avisa en la alerta y encender a mano lo rearma;
  - arrancar con Windows registra el comando (`--data-dir` y `--al-iniciar-windows`);
  - al arrancar con ese argumento, el servidor marcado se enciende solo;
  - quitarlo borra el valor.
- El registro de Windows se sustituye en las pruebas por un archivo (`MERC_FAKE_RUN_KEY`, solo en compilaciones de depuración). El código del registro se comprobó contra las firmas reales de `windows-sys` 0.61, pero **falta probarlo en Windows**.
- **Interfaz**: activar el reinicio, activar el arranque con Windows y marcar el servidor; aviso de «dejó de reintentar»; fuera de Windows, desactivado y explicado.
