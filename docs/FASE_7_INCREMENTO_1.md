# Fase 7 · incremento 1: Conexión y Playit.gg para Minecraft

Nueva sección **Conexión** en cada servidor: cómo entran los amigos.

## En tu misma red
- Dirección local (`192.168.x.x:puerto`) con **Copiar** y **Probar**.

## Amigos por Internet con Playit.gg
Qué se comprobó antes de integrarlo (regla: no asumir que algo está permitido solo porque
existe un programa o una API):
- El **agente oficial** de Playit es código abierto (BSD-2-Clause,
  `github.com/playit-cloud/playit-agent`). La versión 1.2.0 publica `playitd-windows-x86_64.exe`,
  un agente sin ventana con canal local privado (`--socket-path`) y archivo de secreto propio
  (`--secret-path`).
- La **vinculación** usa el flujo oficial del propio agente (`playit-cli claim`): un código, el
  enlace `https://playit.gg/claim/<código>` y el canje por la clave del agente.
- La **API pública documentada** de Playit (docs.playit.gg) es de pago (Premium) y solo cubre
  cortafuegos: **no permite crear túneles** con una cuenta gratuita. Por eso MercHosting no crea
  túneles por su cuenta ni automatiza páginas privadas: guía a la persona para crear el túnel en
  playit.gg (una vez) y lo reconoce en cuanto el agente lo recibe.

Flujo:
1. **Activar Playit** explica que es un servicio externo y gratuito, que se usa la cuenta propia
   de la persona y que la PC tiene que estar encendida. MercHosting descarga el agente oficial
   1.2.0 de GitHub y comprueba su **SHA-256** (`ecd1c6c2…3331`) antes de ejecutarlo.
2. El **supervisor** mantiene el agente en marcha en segundo plano, sin ventana (aunque cierres
   MercHosting), con su propio canal privado: no choca con una instalación de Playit que ya
   tengas. Si se cae, lo vuelve a arrancar (esperando cada vez un poco más).
3. **Vincular mi cuenta de Playit**: se abre el enlace oficial en el navegador; la persona entra
   con su cuenta (o la crea) y aprueba. MercHosting recibe la clave y se la entrega al agente por
   su canal local; **el propio agente la guarda** en su archivo (MercHosting no la escribe en
   ningún otro sitio ni la muestra).
4. Si falta el túnel de ese servidor, una guía de 3 pasos: abrir la cuenta (enlace que da el
   propio agente), crear un túnel **Minecraft Java** hacia `127.0.0.1:<puerto del servidor>` y
   volver. MercHosting lo detecta solo (se revisa cada 3 s).
5. La **dirección pública** aparece con «Sin probar». **Probar conexión** hace un ping real de la
   lista de servidores de Minecraft por la dirección pública (resolviendo el registro SRV de
   `*.joinmc.link` con `nslookup` de Windows, sin ventana) y muestra la latencia **medida**, la
   versión y los jugadores. El botón **Copiar** solo se activa cuando la prueba funciona.
6. **Túnel hacia otro puerto** (por ejemplo, el túnel va a 25565 y el servidor usa 25566): se
   explica y se ofrecen dos arreglos: «Usar el puerto 25565 en este servidor» (con el servidor
   detenido; cambia `server-port` con copia previa, y no se ofrece si ese puerto es de otro
   servidor registrado) o cambiar el puerto local del túnel en playit.gg › Origin Configuration.
   La prueba de conexión anuncia en el saludo el nombre que escribe el jugador (como el juego),
   también tras resolver el SRV, para que Playit sepa a qué túnel va.
7. Varios servidores: cada uno muestra solo los túneles que van a **su** puerto; los demás
   túneles de la cuenta se listan aparte y no se tocan.
8. Avisos del propio Playit (por ejemplo, verificar el correo) se muestran con su enlace.
9. **Desvincular esta PC** (con confirmación) y **Desactivar Playit** (apaga el agente; la
   vinculación se conserva).

## Protocolo (compatible con v1)
- Nuevas peticiones: `playit_status`, `playit_enable`, `playit_disable`, `playit_link`,
  `playit_unlink`, `playit_login_url`, `connection`, `test_address`.
- Respuestas: `playit`, `playit_login_url`, `connection`.

## Pruebas
- Rust: 175. Nuevas:
  - canal del agente (estado, túneles, avisos, secreto no válido) y respuestas de la API de
    vinculación; códigos y enlaces;
  - ping real de Minecraft contra un servidor falso (versión, jugadores, texto con colores),
    servidor que no es Minecraft, puerto cerrado, direcciones no válidas y registros SRV (salida
    de `nslookup` de Windows y de Linux);
  - en el supervisor: agente falso por canal local + API de vinculación falsa: vincular (la
    clave llega al agente), túneles filtrados por el puerto del servidor, prueba fallida anotada
    sin inventar éxito, enlace de la cuenta y desvincular.
- Interfaz: 93 (con el caso del túnel hacia otro puerto, y cuando ese puerto es de otro servidor).
- Interfaz (antes): 91. Nuevas: misma red (probar y copiar); activar, vincular con el enlace oficial,
  guía del túnel y dirección que solo se copia tras probarla; desvincular con confirmación y
  desactivar.
- Capturas: `docs/capturas/fase7a-*.png`.

## Pendiente de probar en Windows
- La descarga y el arranque del agente real, la vinculación con una cuenta de playit.gg real y la
  prueba de la dirección pública.

## Lo siguiente (Fase 7)
- Simple Voice Chat: detectar el mod o plugin, leer su puerto UDP real, guiar el túnel UDP,
  escribir `voice_host` con copia previa y diagnóstico «Minecraft funciona pero la voz no».
- Monitor por capas: agente, túnel, servidor y voz, con su estado y la hora de la última consulta.
