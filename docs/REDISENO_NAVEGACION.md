# Rediseño de la navegación

Pedido: la lista de servidores ocupaba demasiado sitio y todo se veía muy apretado. Referencia: un
panel con barra lateral de secciones agrupadas, cabecera con datos en vivo y resumen en cuadrícula.

## Qué cambió
- **Botón «Servidores»** en la barra lateral, en lugar de la lista fija. Lleva a una página con
  todos los servidores en tarjetas grandes (estado, tipo, versión y puerto), más «Crear» e «Importar».
  Si solo tienes un servidor, la app entra directamente en él al abrirse.
- Dentro de un servidor, la **barra lateral** muestra:
  - arriba, el servidor abierto (pulsando se vuelve a «Servidores»);
  - las secciones agrupadas: **Principal** (Resumen, Consola, Archivos, Ajustes, Actividad) y
    **Juego** (Jugadores, Mundos, Mods/Plugins). Jugadores lleva un punto cuando hay mantenimiento.
- **Lado de la barra**: por defecto a la derecha, como elegiste. El botón del pie la pasa a la
  izquierda (como en la imagen de referencia) y la app lo recuerda.
- **Cabecera** de cada sección: nombre de la sección, nombre del servidor, estado y tipo, y a la
  derecha los datos en vivo (tiempo encendido, CPU, memoria usada de la asignada y jugadores).
  Todos son medidas reales del supervisor; si el servidor está apagado se ve «—».
- **Resumen** en cuadrícula:
  - consola con su barra de comandos;
  - a la derecha: Iniciar/Reiniciar/Detener/Forzar cierre, las direcciones para copiar y una
    tarjeta con la escena de cubos;
  - debajo: Procesador, Memoria y Jugadores con sus gráficas, y los detalles. «Quitar de la lista»
    está en los detalles.
- Más aire en general: márgenes de 30–36 px, separación de 18–22 px entre bloques y textos más grandes.

## Pruebas
Interfaz: 73 (2 nuevas: lista de servidores, abrir uno, secciones y volver; cambiar de lado la
barra y que se recuerde). Rust sin cambios. Capturas: `docs/capturas/rediseno-*.png`.
