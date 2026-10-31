# Fase 1, incremento 4: identidad visual de MercHosting y métricas reales

Fecha: 30-10-2026 · Estado: **completado y probado en Linux con la app real; pendiente de abrir en Windows**

## 1. Pedido del usuario

- La consola parpadeaba cada pocos segundos.
- El diseño anterior se veía «estilo código».
- Usar su cubo como logo de MercHosting.
- Un panel único, con partículas, animación de entrada y botones con efectos al pasar el ratón. Referencias: paneles de hosting como Godlike, Bisect o Kinetic.

## 2. Parpadeo de la consola: causa y corrección

Causa: cada vez que se refrescaba la lista de servidores (1 s), la vista creaba de nuevo su cliente del supervisor. La consola tomaba eso como «otro servidor», se vaciaba y volvía a cargar entera.
Corrección: las líneas se leen en un *hook* (`useConsoleLines`) que solo se reinicia si cambia el servidor; además, el cliente se memoriza. Hay una prueba que lo comprueba: tras varios refrescos de la lista, la consola se pide desde el principio una sola vez.

## 3. Identidad visual

| Elemento | Decisión |
|---|---|
| Logo | Cubo del autor (`app/branding/logo-original.png`), vectorizado en 6 piezas (`app/src/components/logoPaths.ts`). |
| **El núcleo como estado** | El rombo central del logo es el indicador de cada servidor: verde en línea, ámbar latiendo al iniciar, rojo si falla, gris si está detenido. El logo de la barra lateral se pone verde si hay algún servidor en línea. |
| Entrada | El cubo se arma pieza a pieza (caras exteriores, tapa, piezas interiores y núcleo con destello), «MercHosting» aparece letra a letra y la pantalla se abre. Dura lo que tarde en conectar (mínimo 1,9 s). |
| Partículas | «Polvo de bloques»: rombos isométricos que suben despacio, con un leve efecto de profundidad al mover el ratón. Se pausan con la ventana oculta. |
| Portada del servidor | Una «isla» de bloques isométricos generada a partir del id del servidor (cada uno tiene la suya). Se tiñe según el estado: verde con bloques iluminados en línea, ámbar al iniciar, roja si falla. Es arte propio: no se usa ningún recurso de Minecraft. |
| Botones | Iniciar (verde), Detener (coral), Reiniciar, Forzar cierre y Quitar. Al pasar el ratón se elevan, brillan con su color y un destello cruza el botón; al pulsar, se hunden un poco. |
| Tipografía | Unbounded (marca, títulos, cifras), Manrope (texto) y JetBrains Mono solo en la consola. Licencia OFL, incluidas en la app. |
| Color | Noche petróleo, marfil y cobre/champán (directiva §11), con cristal esmerilado y relieves suaves. Verde, ámbar y coral solo para estados y acciones. |
| Accesibilidad | Si el sistema pide reducir movimiento, no hay intro, partículas, latidos ni brillos. Foco visible, pestañas y diálogos usables con teclado y etiquetas para lectores de pantalla. |

Icono de la app regenerado con el cubo en marfil y el núcleo cobre sobre petróleo (`app/branding/icon.svg`).

## 4. Datos reales nuevos (nada inventado)

| Dato | Fuente | Dónde se ve |
|---|---|---|
| CPU del servidor | `sysinfo`, proceso Java, cada 2 s, normalizado a % de **toda** la PC | Tarjeta «Procesador» con gráfica de los últimos 6 min |
| Memoria del servidor | `sysinfo`, memoria residente del proceso | Tarjeta «Memoria»: uso, lo asignado, RAM total de la PC y gráfica |
| Jugadores conectados | Líneas `Nombre joined/left the game` de la consola. El chat, `/say` y `/me` no pueden falsificarlas (con prueba) | Tarjeta «Jugadores» con `max-players` de server.properties |
| RAM total, núcleos | `sysinfo` / sistema | Textos de las tarjetas |
| IP de red local | Interfaz de salida del sistema (sin enviar paquetes); solo direcciones privadas | Dirección «Desde tu red (LAN)» con botón de copiar |

Con el servidor detenido, las tarjetas dicen «Inicia el servidor para ver su consumo» en lugar de mostrar ceros. Un servidor huérfano (sin consola) no cuenta jugadores y lo explica.

Protocolo ampliado sin romper compatibilidad: `metrics` y `system_info`; contrato JSON regenerado y comprobado en Rust y en TypeScript.

## 5. Pruebas

| Conjunto | Resultado |
|---|---|
| Rust (`cargo test --workspace`) | **74 pasan, 0 fallan**; clippy sin avisos |
| Interfaz (`vitest`) | **24 pasan, 0 fallan** |
| Tipos (`tsc --noEmit`) | Sin errores |

Nuevas: medición del propio proceso, detección de jugadores (y que no se pueda falsificar), métricas y jugadores con un proceso Java real, métricas y datos del sistema por el supervisor real, tarjetas con datos reales, «sin consumo» con el servidor detenido, LAN opcional, el parpadeo de la consola y la entrada animada.

**App real en pantalla virtual (Linux, Xvfb, sin aceleración gráfica)** con el supervisor real y procesos Java de prueba (FakeServer imita a Minecraft; no es Minecraft): se comprobaron la animación de entrada fotograma a fotograma, la portada con la isla según el estado, las métricas reales (43 MB, CPU, núcleos y RAM del equipo de pruebas), 3 jugadores detectados en la consola, la pestaña Consola con envío de comandos y el diálogo de importar. Capturas en `docs/capturas/inc4-*.png`.

Un hallazgo corregido durante la prueba visual: el color del núcleo del logo no se aplicaba, porque WebKit no admite `var()` dentro de atributos SVG. Ahora se aplica desde CSS.

## 6. Límites

- TPS, entidades y chunks no se muestran: necesitan un puente dentro del servidor (fase posterior). No se inventan.
- La lista de jugadores empieza a contar cuando MercHosting ve la consola. Si alguien entró antes de que el supervisor arrancara, no aparece hasta que vuelva a entrar.
- La conexión desde Internet (Playit) aún no existe; la app lo dice con un texto, sin botón.
- Rendimiento de las partículas y del desenfoque comprobado solo en Linux por software. En Windows (WebView2 con GPU) debería ir mejor, pero hay que confirmarlo.

## 7. Ajustes pedidos tras la primera revisión (30-10-2026)

**Paleta azul.** El usuario prefirió los azules de sus referencias al verde/petróleo:
- Fondo azul marino con brillos azul y violeta.
- Acento de marca azul eléctrico (`#4f7dff`): botón Iniciar, pestaña activa y núcleo del logo.
- Importar con degradado violeta → azul.
- «En línea» pasa de verde a **cian** (`#3fd2ff`). Ámbar (iniciando) y coral/rojo (detener/error) se mantienen, porque significan cosas distintas.
- La isla de bloques en línea es azul con destellos cian.
- Icono de la app regenerado con el núcleo azul. Esto se aparta de la paleta cobre de la directiva §11 a petición expresa del autor.

**Sonido de entrada** (`app/src/sound/introSound.ts`):
- **Sintetizado con Web Audio**, sin archivos de terceros.
- Estructura: *whoosh* mientras vuelan las piezas, un golpe de bloque por pieza (cada uno más agudo), barrido de encendido y acorde tipo campana (La mayor con novena) con bajo suave y reverberación cuando se enciende el núcleo, y chispas suaves con las letras.
- Una prueba compara los tiempos del sonido con los retrasos de la animación en `styles.css`, para que no se desincronicen.
- Muestra renderizada sin conexión en Chromium: `docs/sonido-entrada-muestra.wav` (3,4 s, pico 0,37, sin saturar). Los picos caen en 0,40–0,80 s (piezas) y 0,95 s (núcleo).
- Botón **«Sonido de entrada: activado/silenciado»** en la barra lateral; la elección se recuerda. Sin sonido si el sistema pide reducir movimiento.
- Windows/WebView2: se añadió `--autoplay-policy=no-user-gesture-required` (manteniendo los argumentos por defecto de Tauri) para que pueda sonar al abrir sin un clic previo. **Pendiente de confirmar en Windows**: en el entorno de pruebas no hay tarjeta de sonido, aunque la app intentó reproducirlo al abrirse.

Pruebas: Rust **74/74**, interfaz **28/28** (nuevas: sincronía del sonido con la animación, preferencia guardada y botón de silenciar).
