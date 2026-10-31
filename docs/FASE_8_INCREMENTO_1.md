# Fase 8 · incremento 1: Compañeros entre PCs (sin cuentas)

Nueva sección **Compañeros** en la barra lateral. Un amigo que también tiene MercHosting en su PC puede administrar un servidor tuyo, solo con los permisos que tú marques.

## Decisión del dueño del proyecto: sin cuentas ni servidor central
La especificación pedía un registro con usuario y contraseña en un servicio central (Argon2id, recuperación por códigos…). Ese servicio costaría unos US$5 al mes (Cloudflare Workers de pago o un VPS). El dueño del proyecto pidió que **no haya que pagar nada**, así que se hizo así:

- **Sin cuentas.** No se guarda ningún usuario en ningún sitio. Cada PC tiene una **llave propia**, que se crea una vez y se guarda en la carpeta de datos de MercHosting, nunca dentro de un servidor. Esa llave es su identidad.
- **Invitación con código.** El dueño crea un código de **un solo uso** que caduca (30 min a 24 h) para un servidor y unos permisos. El compañero lo pega en su MercHosting.
  - El código lleva la llave pública de la PC anfitriona y un secreto de 16 bytes. En el anfitrión solo se guarda el **SHA-256** del secreto, y se compara en tiempo constante.
  - Se limitan los intentos fallidos: 5 por PC y 30 en total cada 10 minutos.
- **Lo que se pierde frente a las cuentas:** el compañero queda ligado a **su PC**; si cambia de PC necesita un código nuevo. No hay buscador de usuarios: el código se pasa por un chat privado.

## El canal
- **iroh 1.4.1** (n0, MIT/Apache-2.0): QUIC con TLS 1.3, autenticado por la llave de cada PC. **Cifrado de punta a punta.**
- Intenta conexión **directa** entre las dos PCs, atravesando el router. Según iroh, eso funciona en torno al 90 % de las veces. Si no puede, usa los **relés públicos gratuitos de n0**, que solo reenvían bytes cifrados.
- Política de n0 sobre esos relés: seguirán siendo gratis para planes gratuitos y de aficionados, pero **sin garantía de funcionamiento**, con límite de velocidad y solo para la última versión de iroh. Si algún día hiciera falta, se puede poner un relé propio sin cambiar nada más.
- Para encontrar a la otra PC se usa la búsqueda pública de n0 (también gratuita). El código incluye además el relé del anfitrión, para no depender solo de esa búsqueda.
- Es un canal aparte: **nunca** usa el túnel de Minecraft ni Playit.
- La PC anfitriona tiene que estar encendida, conectada y con el supervisor de MercHosting en marcha. No hay encendido remoto.

## Permisos (decide la PC anfitriona)
- Hay 21 permisos separados:
  - **Servidor:** estado, iniciar, detener, reiniciar.
  - **Consola:** leer, comandos, historial.
  - **Jugadores:** jugadores, lista blanca, OP.
  - **Archivos:** ver lista, leer, editar, borrar.
  - **Contenido:** mods y plugins (JAR).
  - **Copias:** ver y crear, restaurar, borrar.
  - **Mundo y ajustes:** mundos, ajustes, conexión.
- **Alto riesgo**, nunca agrupados con la lectura: comandos, OP, editar, borrar, mods y plugins, y restaurar copias.
- **Roles:** Observador, Moderador, Operador, Administrador y Personalizado. Son atajos: se puede ajustar cada permiso. «Ver el estado» va siempre.
- **Denegar por defecto** (`merc-ipc/src/remote.rs`): cada petición del protocolo tiene asignado el permiso que exige, y lo que no está en la lista se deniega. Solo el dueño puede:
  - aceptar la EULA, crear, quitar o importar servidores, cerrar a la fuerza y apagar el supervisor;
  - Playit, compañeros y alertas globales;
  - y todo lo que usa rutas de su PC: subir, descargar, exportar e importar.
- **Comprobaciones en cada petición:**
  - se leen los permisos **actuales** del compañero en ese servidor;
  - solo puede seguir o cancelar las tareas en curso que inició él;
  - la lista de servidores se filtra a los suyos;
  - de las respuestas se quitan las rutas completas de la PC (por ejemplo, el nombre de usuario de Windows).
- Los archivos siguen limitados a la carpeta del servidor, con la misma protección que el uso local (sin `..`, enlaces simbólicos ni uniones fuera del servidor).
- **Revocar es inmediato:**
  - quitar un compañero o cambiarle permisos corta su conexión;
  - pausar el acceso cierra todas las conexiones y rechaza las nuevas.
- **Historial:** todo lo que hace un compañero se anota en la Actividad del servidor como `companero:<nombre>`, con la acción y el resultado. Lo denegado también se anota. Las lecturas que la app repite cada segundo no se anotan.
- Al unirse un compañero llega una alerta (y la notificación de Windows, si está activada).

## Advertencias
- Al dar o ampliar permisos de archivos, comandos o mods aparece el **texto literal obligatorio** de la especificación. Debajo va la lista de lo que se concede, con un aviso reforzado para:
  - **JAR:** Java los ejecuta en la PC con el usuario de Windows, y MercHosting no puede aislarlos;
  - **comandos:** puede enviar cualquier comando de Minecraft, pero no programas de Windows.
- Hay que marcar «Entiendo los riesgos y confío en esta persona» para poder seguir.

## En la PC del compañero
- Compañeros › «Administrar servidores de un amigo»: pega el código y su nombre. Ve las PCs de sus amigos (con su huella) y los servidores a los que tiene acceso.
- **Abrir** usa la misma vista que un servidor propio, pero:
  - solo muestra las secciones que permiten sus permisos;
  - sin botones de rutas locales (subir, descargar, exportar, abrir carpeta);
  - sin EULA, quitar ni forzar cierre.

  Cada petición viaja a la PC del amigo, que es la que decide.

## Pruebas
- `merc-ipc`: roles, y que las peticiones del dueño se deniegan por defecto.
- `merc-remote`:
  - dos nodos hablan por el canal cifrado;
  - el anfitrión ve la llave real del compañero;
  - revocar corta la conexión;
  - desactivar rechaza a todos;
  - los códigos se leen bien aunque vengan con espacios, minúsculas o el relé incluido.
- Supervisor, prueba de extremo a extremo con **dos supervisores reales**:
  - invitar exige activar el acceso, y el secreto no se guarda en claro;
  - el código es de un solo uso;
  - la lista sale filtrada y sin rutas.
- **Abusos de la especificación**, que fallan todos en el anfitrión:
  - comandos o iniciar sin permiso;
  - pedir el estado de otro servidor o de un ID inventado;
  - apagar el supervisor, quitar el servidor o aceptar la EULA;
  - leer la llave de la PC con `..`, primero sin permiso y luego con permiso de lectura (lo rechaza la protección de rutas);
  - crear invitaciones o reenviar peticiones a otra PC;
  - seguir una tarea ajena;
  - usar el permiso de consola después de que se lo quitaran;
  - seguir usando la sesión después de ser quitado, o con el acceso pausado.
- UI, 3 pruebas:
  - advertencia literal y confirmación obligatoria para crear el código;
  - ampliar permisos de archivos y quitar a un compañero;
  - unirse con un código y abrir el servidor del amigo solo con sus secciones.
- **Lo que no se pudo probar aquí:** el modo Internet (relés y búsqueda de n0). La red de este entorno de desarrollo no deja llegar a esos servicios, así que las pruebas usan conexión local directa. Hay que probarlo entre dos PCs reales.

## Pendiente (siguientes incrementos)
- ~~Descargar y subir archivos, y exportar mundos y registros, entre PCs~~: hecho en el incremento 2.
- Historial global de compañeros y centro de seguridad (Fase 9).

Capturas: `docs/capturas/fase8a-companeros.png`, `fase8a-invitar.png` y `fase8a-advertencia.png`.
