# Fase 7 · incremento 2: Simple Voice Chat y estado por capas

La pestaña **Conexión** ahora detecta Simple Voice Chat, explica su túnel de voz y muestra el
estado de la conexión por capas.

## Qué se comprobó antes de integrarlo
- Documentación oficial del mod (modrepo.de, wiki «Server config»):
  - la configuración está en `config/voicechat/voicechat-server.properties` (Fabric, Forge, NeoForge y Quilt) o en `plugins/voicechat/voicechat-server.properties` (Paper y Spigot);
  - `port` es el puerto **UDP** de la voz (24454 por defecto; `-1` usa el de Minecraft, cosa que el autor desaconseja porque puede cerrar el servidor);
  - `bind_address` vacío usa el `server-ip`;
  - `voice_host` es la dirección que el servidor anuncia a los clientes y admite `host:puerto`.
- Guía oficial de Playit (playit.gg/support/svc-minecraft): se crea un túnel de tipo **«MC: Simple Voice Chat»** hacia `127.0.0.1:<puerto de la voz>`, y la `IP:puerto` del túnel va en `voice_host`.
- Playit no permite crear túneles desde otras aplicaciones con una cuenta gratuita (ya visto en el incremento 1), así que el túnel de voz también es un paso guiado. MercHosting **no crea, no cambia y no borra túneles**.
- La voz (UDP, protocolo propio del mod) **no se puede probar desde fuera**. La UI lo dice y recomienda `/voicechat test <jugador>` en el juego.

## Detección
- Se reconoce Simple Voice Chat por la **ficha del jar**, no por el nombre del archivo:
  - `fabric.mod.json` o `quilt.mod.json` con id `voicechat`;
  - `mods.toml` o `neoforge.mods.toml` con `modId="voicechat"`;
  - `plugin.yml` o `paper-plugin.yml` con `name: voicechat`.

  Así no se confunde con un addon. También detecta si está desactivado (`.jar.disabled`).
- El **puerto real** se lee siempre del archivo; nunca se supone 24454.
- La consola de esta sesión (y si hace falta `logs/latest.log`, si se escribió después de encender) dice si la voz arrancó:
  - «Voice chat server started at 0.0.0.0:24454», o «Server started at port …» en versiones antiguas;
  - o si falló al abrir su puerto: «Failed to bind», «Address already in use».

## Diagnóstico «Minecraft funciona pero la voz no»
Cada problema trae su explicación y, cuando se puede, su arreglo:
- Mod desactivado, o sin configuración todavía: hay que encenderlo una vez.
- Puerto `-1`, o el mismo que Minecraft. También cuando choca con otro servidor tuyo (su puerto de Minecraft o de voz). Se propone un puerto libre (desde 24454) con **«Usar el puerto N para la voz»**.
- La consola dice que no pudo abrir el puerto, o que la voz no arrancó.
- `bind_address` con un valor que puede bloquear a los de fuera.
- Playit apagado: la voz ya va en tu red; para amigos de fuera hay que activar Playit.
- **Falta el túnel de voz**: guía de 3 pasos (tipo «Simple Voice Chat», `127.0.0.1`, el puerto real) y botón para abrir la cuenta de playit.gg.
- Túnel desactivado por Playit, o túnel que no informa su puerto público.
- **`voice_host` vacío o distinto** de la dirección del túnel. El botón **«Usar <IP:puerto> para la voz»** abre una confirmación que muestra el valor anterior y el nuevo.
- Configuración cambiada después de encender el servidor, o la voz escucha en otro puerto. En ese caso, **«Reiniciar avisando a los jugadores (1 min)»**, con el reinicio con aviso que ya existía.
- Siempre: cada jugador necesita el mod Simple Voice Chat en su Minecraft.
- Si quitaste el mod y queda un túnel hacia su puerto, se avisa de que puedes desactivarlo o borrarlo en playit.gg.

## Escribir la configuración (`SetVoiceConfig`)
- `voice_host` solo acepta la dirección de un **túnel de voz activo detectado** hacia el puerto de la voz, o vacío. No se aceptan direcciones inventadas.
- `port` debe estar entre 1024 y 65535 y no puede ser el de Minecraft ni el de otro servidor.
- Se cambia solo esa línea; los comentarios y el resto del archivo se conservan. Antes se guarda una **copia del archivo anterior** en el historial de MercHosting, que se puede restaurar desde Archivos › Copias. Queda anotado en Actividad.
- Los cambios se aplican al reiniciar. La UI lo indica, porque compara la hora del archivo con la hora de encendido.

## Estado por capas
Una tarjeta **«Estado ahora»** con cada capa en **Operativo**, **Degradado**, **Caído** o **Desconocido**, junto a la hora de su última comprobación:
1. **Servidor (Java)**: estado real del proceso.
2. **Agente de Playit**: lo que informa el propio agente (vinculado, sin vincular, límite del plan, errores, avisos).
3. **Túnel de Minecraft**: si existe, si está activo y adónde va.
4. **Prueba desde Internet**: la última prueba real de la dirección pública, con su latencia medida y su hora. Sin prueba, aparece como «Desconocido».
5. **Chat de voz** (si está instalado): operativo solo si la consola confirma que escucha y no hay problemas pendientes.

No se inventan pings ni datos de salud: lo que no se puede medir aparece como «Desconocido».

## Pruebas
- `merc-admin`, 5 pruebas: detección por ficha (también contra un addon y un jar desactivado), puerto real y `-1`, escritura que conserva comentarios, lectura de la consola (formato nuevo, formato antiguo y error al abrir el puerto), y `voice_host` y `bind_address`.
- Supervisor: la prueba de Playit ahora también:
  - instala el mod;
  - comprueba el puerto real, el túnel de voz y `host_missing`;
  - comprueba las capas;
  - rechaza una dirección inventada y el puerto de Minecraft;
  - escribe `voice_host`, conserva el comentario y verifica la copia en el historial.
- UI, 2 pruebas nuevas:
  - capas, tarjeta de voz y el túnel de voz que no se confunde con un túnel de Minecraft hacia otro puerto;
  - confirmación antes de escribir, guía del túnel y reinicio con aviso.
- Contrato IPC: `set_voice_config`, `voice` y `layers` (campos nuevos con valor por defecto, compatibles hacia atrás).

Capturas: `docs/capturas/fase7b-estado.png` y `docs/capturas/fase7b-voz.png`.
