# MercHosting para celular (Android)

## Qué hace

La app del celular hace dos cosas:

1. **Mis PCs** (fase 1): **administra los servidores de tus PCs**. Esos servidores corren en tu PC o
   en tu servidor casero. El celular se conecta con el mismo canal cifrado de **Compañeros** (iroh,
   de punto a punto y con un código). No hay cuentas, panel web ni puertos abiertos.
2. **En este celular** (fase 2, solo Android de 64 bits ARM): **corre un servidor Java dentro del
   propio celular**, con Playit para que entren tus amigos. Ver [Servidor dentro del
   celular](#servidor-dentro-del-celular-fase-2).

Con el celular puedes:

- **Conectar una PC** pegando un código de invitación que generes en esa PC.
- **Ver tus PCs y sus servidores**, actualizarlos u olvidar una PC.
- Con **control total** de la PC (invitación «control total»):
  - crear servidores **Java** (Vanilla, Paper, Fabric…) y **Bedrock** en la PC;
  - activar **Playit** en la pestaña Conexión, vincularlo a tu cuenta (el enlace se abre en el
    navegador del celular) y ver la dirección para tus amigos.
- Con permisos de servidor: encender, apagar y reiniciar, usar la **consola**, ver y gestionar
  **jugadores**, ver y editar **archivos** de texto, y gestionar **addons**, mods, plugins, copias y
  propiedades, según lo que permita la PC.

Lo que **todavía no** hace:

- Subir o descargar archivos, ni exportar mundos o la consola: esos botones no aparecen en el
  celular, y el celular rechaza esas peticiones. Hazlo desde MercHosting en una PC.
- Escanear un código QR: el código se pega. Un lector QR necesitaría un complemento de terceros que
  no está verificado.
- Hospedar el servidor **oficial** de Bedrock en el propio celular (ver abajo por qué). Para
  jugadores de Bedrock se crea un servidor «Java + Bedrock» con Geyser.
- iOS: queda para el futuro. Hace falta una Mac, una cuenta de Apple Developer y pruebas en un iPhone.

## Seguridad

- Para la PC, el celular es **un compañero más**. La PC decide cada acción con los permisos que le
  dio a ese celular (`need()` en `crates/merc-ipc/src/remote.rs`) y anota todo en su Actividad.
  El celular no abre nada nuevo en la PC.
- **Playit desde el celular** solo funciona con **control total** de la PC (`Need::Host`). Por qué
  se permite:
  - el dueño ya se dio control total a sí mismo (crear y encender servidores, cambiar propiedades);
  - Playit solo transporta el **juego**: la administración nunca viaja por Playit;
  - el enlace de vinculación lo genera la PC y lo abres tú en el navegador del celular;
  - todo queda auditado.

  Un compañero normal sigue sin poder tocar Playit (hay pruebas que lo comprueban).
- La identidad del celular (su llave iroh) se guarda en `llave-dispositivo`, dentro de la carpeta
  privada de la app. Android la aísla de otras apps, y en sistemas Unix el archivo además se crea
  con permisos 0600. Si borras los datos de la app, el celular pasa a ser un dispositivo nuevo y
  necesitas otro código.
- Ni el repositorio ni el APK contienen claves. La clave de firma vive solo en tus secretos de GitHub.

## Servidor dentro del celular (fase 2)

### Qué se puede

- Tipos: **Paper** (recomendado), **Java + Bedrock**, **Vanilla**, **Purpur** y **Fabric**, de
  Minecraft **de la 1.18 a la 1.21** (Java 17 para 1.18–1.20.4 y Java 21 para 1.20.5–1.21.x). Las 26.x piden Java 25 y todavía no hay uno verificado para Android.
- **Java + Bedrock**: un Paper con **Geyser** y **Floodgate** (de GeyserMC, licencia MIT), instalados
  desde Modrinth (proyectos `geyser` y `floodgate`) con la huella que publica Modrinth. Entran
  jugadores de Java y de Bedrock (celular, consola, Windows) al mismo mundo; los de Bedrock no
  necesitan cuenta de Java. **Antes de descargar nada** se comprueba que Geyser y Floodgate tengan
  versión para el Minecraft elegido; si no, no se crea el servidor. Si falla la instalación de
  Geyser, el servidor recién creado (vacío) se quita. Los jugadores de Bedrock entran por la IP del
  celular y el puerto **UDP 19132** (o el de la configuración de Geyser). Por Internet hace falta un
  segundo túnel en playit.gg, de tipo «Minecraft Bedrock» (UDP), hacia ese puerto. No hay addons
  `.mcpack`: eso solo existe en el servidor oficial de Bedrock.
- **Mods y plugins de Modrinth** (Paper y Purpur: plugins; Fabric: mods; Vanilla: nada). Buscar
  (sin texto muestra lo más descargado para esa versión), instalar la versión estable más nueva que
  sirva con sus dependencias obligatorias, activar/desactivar (renombra a `.disabled`) y quitar (va
  a la papelera de la app). Se usa el mismo plan verificado que en la PC: todo se descarga a una
  carpeta aparte, se comprueba con su huella y solo entonces se pone en `plugins/` o `mods/`; lo que
  se reemplaza va a la papelera. Con el servidor encendido se puede buscar pero no cambiar nada.
  **CurseForge** aparece desactivado con «Próximamente»: necesita una clave de API oficial, que no
  va en la app.
- Celulares **Android arm64** (casi todos los actuales) con **al menos 3 GB de memoria**. La memoria
  del servidor se limita según el celular: como mucho el 45 % de la memoria total y siempre dejando
  2 GB para Android (máximo 6 GB). Por defecto, un cuarto de la memoria.
- Encender, **Detener** (guarda el mundo), **Forzar cierre** (solo si no responde), consola con
  comandos, copia de seguridad, cambiar la memoria y quitar el servidor (antes hace una copia y la
  carpeta pasa a la papelera de la app, no se borra).
- **Playit** dentro del celular: se enciende junto con el servidor, se vincula con TU cuenta de
  playit.gg (el enlace se abre en el navegador) y muestra la dirección del túnel. El túnel
  «Minecraft Java» se crea en playit.gg, igual que en la PC (la API de Playit no deja crearlo desde
  fuera).

### Qué NO se puede, y por qué

- **Servidor oficial de Bedrock**: Mojang solo lo publica para Windows y Linux de 64 bits x86. No hay
  versión para ARM ni para Android. Correrlo con un emulador de x86 (Box64/QEMU) es lento e
  inestable, así que no se ofrece. Para un Bedrock oficial con addons usa «Mis PCs»; en el celular
  usa «Java + Bedrock».
- **Forge, NeoForge, Quilt, Spigot y Arclight**: necesitan ejecutar un instalador o compilador de Java
  aparte (otro programa), y Android no deja ejecutar programas descargados (ver abajo).
- **Minecraft 1.17 o anteriores**: necesitan Java 8 o 16; solo se ofrecen 17 y 21 para Android.
- Celulares de 32 bits o x86: no hay un Java para Android verificado para ellos en esta versión.

### Avisos que hay que aceptar

Antes de crear un servidor hay que marcar **uno por uno** estos avisos (sin todos no se puede
seguir, y el núcleo en Rust lo vuelve a comprobar):

1. **Gasta mucha batería**: úsalo enchufado.
2. **Se calienta**: si se calienta mucho, Android baja la velocidad o cierra apps.
3. **Android puede cerrarlo** sin avisar (sobre todo con el ahorro de batería de algunas marcas).
4. **Mejor con cargador y wifi**.
5. **Gasta datos móviles** si no hay wifi (cada jugador consume tu plan).
6. **Rinde menos que una PC**: pensado para 2 a 5 amigos y pocos mods o plugins.
7. **Desgasta el celular**: horas de calor y carga envejecen la batería.

Además hay que aceptar la **EULA de Minecraft** (MercHosting nunca la acepta por ti).

### Cómo funciona por dentro

- Desde Android 10, una app **no puede ejecutar programas** guardados en su carpeta de datos. Por
  eso no se lanza `java` como en una PC: se cargan las bibliotecas del Java para Android con
  `dlopen` y se llama a `JLI_Launch` (el «main» del lanzador de OpenJDK), dentro de un proceso
  aparte de la app (`:server`). Es la misma técnica que usan PojavLauncher/Amethyst.
  Código: `crates/merc-phone/src/jli.rs`.
- Ese proceso es un **servicio en primer plano** (`ServerService`, tipo `specialUse`) con una
  notificación permanente y su botón **Detener**. Mientras corre mantiene el celular despierto (wake
  lock parcial) y el wifi activo. Si el servidor termina o Android lo cierra, el proceso se cierra
  entero y **no se vuelve a encender solo**. Código: `crates/tauri-plugin-mercphone/android`.
- La salida del servidor va a un archivo y los comandos entran por una tubería con nombre. El
  estado (encendiendo, en línea, apagando) se lee de esa salida, como en la PC.
- Los servidores se instalan con **el mismo instalador verificado** que en la PC (`merc-install`) y
  las copias de seguridad con el mismo módulo (`merc-admin`). Todo se guarda en la carpeta privada
  de la app (`celular/`). Si desinstalas la app o borras sus datos, **se borran los mundos**: haz
  copias y guárdalas fuera si te importan.

### Etiquetado de punteros (Android 11+)

En celulares arm64 con Android 11 o más nuevo, el asignador de memoria pone una etiqueta en el
byte alto de cada puntero. La JVM de escritorio (HotSpot) guarda punteros sin ese byte y, al
liberarlos, Android aborta el proceso con «Pointer tag for 0x… was truncated». Por eso el
manifiesto del plugin pone `android:allowNativeHeapPointerTagging="false"` en `<application>` (se
une al manifiesto de la app al compilar). Es lo mismo que hacen los lanzadores de Minecraft Java
para Android.

### Red: sin transporte nativo

El transporte de red nativo de Minecraft (epoll de Netty) está hecho para Linux de PC. En el
celular el juego se quedaba en «Pinging…» y «Joining». Por eso cada servidor del celular arranca
con `use-native-transport=false` en `server.properties` (se pone antes de cada encendido, sin tocar
las demás líneas) y con `-Dio.netty.transport.noNative=true`.

### Paper y el Java «internal»

El Java para Android dice ser `21.0.1-internal` (no es una versión de disponibilidad general) y
Paper se cierra con «Unsupported Java detected». En Paper y Purpur se añade
`-DPaper.IgnoreJavaVersion=true`, la propiedad que documenta Paper para saltarse ese chequeo (Paper
avisa de que entonces no da soporte). Vanilla y Fabric no hacen ese chequeo.

Además, con Paper el jugador se quedaba en «Joining world» (Vanilla entraba bien y el servidor
respondía a `list`). Paper comprime y cifra la conexión con las bibliotecas nativas de Velocity,
hechas para Linux de PC; en Paper y Purpur se añade `-Dvelocity.natives-disabled=true` para usar
las de Java, como Vanilla.

### De dónde sale cada cosa (verificado)

| Qué | Origen | Huella / versión | Licencia |
|---|---|---|---|
| Java 21 para Android | `https://github.com/AngelAuraMC/angelauramc-openjdk-build/releases/download/download_jre21/jre21-android-arm64.tar.xz` (el mismo que usa el lanzador Amethyst; su `release` dice `JAVA_VERSION="21.0.1"`, `OS_ARCH="aarch64"`) | SHA-256 `8d41ec401ee59f7722df60ed991f81ad146e130452804bfdd8a05d3436f7bbfe`, 28 675 516 bytes | GPLv2 con Classpath Exception (`legal/java.base/LICENSE` en el paquete) |
| Java 17 para Android | `…/download_jre17/jre17-android-arm64.tar.xz` | SHA-256 `e162c860fe05ee4a4e4af7606437419879f6c748386a7b09fa77d10db6a64091`, 26 900 804 bytes | ídem |
| Geyser y Floodgate | Catálogo de Modrinth (`api.modrinth.com/v2`, proyectos `geyser` y `floodgate`), la versión estable más nueva para Paper/Purpur y esa versión de Minecraft | SHA-1 que publica Modrinth para cada archivo | MIT (GeyserMC) |
| Agente de Playit | Código oficial `https://github.com/playit-cloud/playit-agent`, etiqueta `v1.2.0`, commit `7acf13568a99707dda73028234fdb376e45b15da` (el workflow comprueba el commit), compilado para `aarch64-linux-android` con el NDK | — | BSD-2-Clause, © 2022 Developed Methods LLC (texto completo en la app › Ayuda) |

- El Java **no va dentro del APK**: se descarga la primera vez que creas un servidor, se comprueba su
  huella y tamaño antes de usarlo, y se descomprime con las mismas reglas de seguridad que el Java de
  la PC (sin rutas ni enlaces que salgan de su carpeta).
- El agente de Playit **sí va dentro del APK** (como `libplayit.so`, porque la carpeta de bibliotecas
  nativas del APK es lo único que Android deja ejecutar). No se usa el binario `playit-linux-aarch64`
  publicado porque está enlazado con musl: en Android buscaría `/etc/resolv.conf`, que no existe, y
  se quedaría sin DNS.

### Qué hace el workflow de más (fase 2)

Después de generar el proyecto Android, el paso **«Agente de Playit para Android»**:

1. clona `playit-agent` en la etiqueta `v1.2.0` y comprueba que el commit es exactamente
   `7acf13568a99707dda73028234fdb376e45b15da`;
2. lo compila con `cargo build --release --locked -p playitd --target aarch64-linux-android` usando
   el clang del NDK del runner;
3. con `scripts/android-playit.mjs` comprueba que el resultado es un ejecutable ELF aarch64, lo copia
   como `libplayit.so` al plugin y activa `useLegacyPackaging` (para que Android extraiga las
   bibliotecas nativas al instalar y el agente exista como archivo).

Si ese paso falla, el workflow **sigue** y deja un aviso amarillo: el APK funciona, pero **sin Playit
dentro del celular** (la app lo dice). Tus amigos solo podrían entrar desde tu mismo wifi.

### Cómo probarlo en tu celular

1. Instala el APK nuevo (ver abajo) y abre **En este celular**.
2. Comprueba que muestra la memoria de tu celular y «Playit incluido».
3. Pulsa **Crear servidor Java en este celular**, marca los 7 avisos, elige Paper y la versión más
   nueva, deja la memoria recomendada, acepta la EULA y crea. Con wifi tarda unos minutos (Java ~28 MB
   y el servidor).
4. Pulsa **Encender**. Android pedirá permiso de notificaciones: acéptalo para ver «Detener». La
   primera vez Paper descarga además el servidor de Mojang y genera el mundo: puede tardar varios
   minutos en el celular.
5. Cuando diga **En línea**, entra desde Minecraft Java en otra PC del mismo wifi con la IP del
   celular (Ajustes › Wi-Fi › tu red) y el puerto 25565.
6. Para Playit: activa «Encender Playit junto con el servidor» con el servidor apagado, enciéndelo,
   pulsa **Vincular con playit.gg**, aprueba en el navegador, y crea un túnel «Minecraft Java» en
   playit.gg hacia el puerto 25565. La dirección aparece en la app.
7. Pulsa **Detener** (en la app o en la notificación) y comprueba que el mundo se guardó (vuelve a
   encender y mira que sigue igual).

Si algo falla, la consola de la app muestra los mensajes de MercHosting (`[MercHosting] …`) y del
servidor. Pásame esas líneas.

## Conectar el celular a tu PC

1. En la PC, abre MercHosting › **Compañeros** y crea una invitación. Para crear servidores y usar
   Playit, elige **control total**. En una PC sin pantalla, usa
   `merc compañeros invitar --control-total`.
2. Pasa el código al celular, por ejemplo con un mensaje a ti mismo.
3. En el celular, ve a **Conectar**, escribe un nombre para el celular y pega el código.

Cada código sirve una sola vez y caduca. La PC tiene que estar encendida, con MercHosting abierto o
en segundo plano, y conectada a Internet.

## Compilar el APK con GitHub Actions

### 1. Crea tu clave de firma (una sola vez)

En tu PC, con Java instalado (`keytool` viene con Java):

```
keytool -genkey -v -keystore ~/upload-keystore.jks -keyalg RSA -keysize 2048 -validity 10000 -alias upload
```

En Windows usa `%USERPROFILE%\upload-keystore.jks` en lugar de `~/upload-keystore.jks`.

Te pedirá una contraseña: **guárdala**. Guarda también el archivo `.jks` fuera del repositorio y haz
una copia de seguridad. Si lo pierdes, no podrás publicar actualizaciones que se instalen encima de
la versión anterior: habrá que desinstalar y volver a instalar.

Pasa la clave a base64:

- Linux o macOS: `base64 -w0 ~/upload-keystore.jks > keystore.b64` (en macOS, sin `-w0`)
- Windows (PowerShell):
  `[Convert]::ToBase64String([IO.File]::ReadAllBytes("$env:USERPROFILE\upload-keystore.jks")) | Set-Content keystore.b64`

### 2. Crea los secretos en GitHub

En el repositorio: **Settings › Secrets and variables › Actions › New repository secret**:

| Secreto | Valor |
|---|---|
| `ANDROID_KEY_BASE64` | El contenido de `keystore.b64` |
| `ANDROID_KEY_ALIAS` | `upload`, o el alias que usaste en `-alias` |
| `ANDROID_KEY_PASSWORD` | La contraseña que pusiste en keytool |
| `RELEASES_TOKEN` | El mismo que ya usas para Linux. Solo hace falta si quieres subir el APK a la release |

Borra `keystore.b64` cuando termines.

### 3. Lanza el workflow

Ve a **Actions › Android › Run workflow** y escribe la versión, por ejemplo `1.0.0`.

- Con **subir** desmarcado, el APK queda como artefacto `merchosting-android` del run, junto con
  `SHA256SUMS.txt`.
- Con **subir** marcado, el APK también se añade a la release `v1.0.0` de MercHosting-releases.
  Esa release tiene que existir ya (publícala antes desde Windows).

El workflow:

1. ejecuta las pruebas (también las de `merc-phone` y `scripts/android-playit.test.mjs`);
2. genera el proyecto Android (`tauri android init`) y los iconos;
3. compila el agente de Playit para Android (ver fase 2);
4. prepara la firma con `scripts/android-firma.mjs`, que sigue la guía oficial de Tauri;
5. compila con `tauri android build --apk --target aarch64`, para celulares ARM de 64 bits, que son
   casi todos los actuales;
6. borra la clave del runner.

## Instalar el APK en el celular

1. Descarga `MercHosting-movil-X.Y.Z-arm64.apk` en el celular. Si viene del artefacto del run,
   primero descomprime el .zip.
2. Ábrelo. Android pedirá permiso para **instalar apps de origen desconocido** desde esa app (el
   navegador o el gestor de archivos). Permítelo solo para esa instalación.
3. Si Play Protect avisa de que la app no es conocida, es normal: la app no está en Google Play.
   Comprueba que el SHA-256 coincide con `SHA256SUMS.txt` antes de instalar.
4. Para actualizar, instala el APK nuevo encima. Tiene que estar firmado con la misma clave.

Requiere Android 7.0 (API 24) o superior y un celular de **64 bits (arm64)**.

Si Android dice **«Se produjo un error durante el análisis del paquete»**, casi siempre el archivo
no es el APK o llegó incompleto: descomprime primero el .zip del artefacto, pásalo por cable USB (o
por WhatsApp como **Documento**, nunca como foto/video) y comprueba que pese lo mismo que en la PC.
También sale en celulares con Android 6 o anterior, o de 32 bits.

## Qué no está probado todavía

- **No se compiló para Android en este entorno**: no había red para instalar el destino
  `aarch64-linux-android` ni el NDK. Sí se comprobó que el código de escritorio y la parte común
  compilan, y que la parte del celular compila simulando `cfg(mobile)` en escritorio (salvo el atributo
  `mobile_entry_point`, que solo existe al compilar de verdad para el celular). La primera
  compilación real será la del workflow.
- **No se probó en un celular real**, ni la conexión iroh desde una red móvil (4G/5G) hasta una PC
  real.
- El parche de firma se probó contra una copia de la plantilla de `tauri android init`, no contra la
  plantilla real de la versión que instale el runner. Si Tauri la cambia, el script falla con un
  mensaje claro en lugar de generar un APK sin firmar.
- La ruta exacta del APK dentro de `gen/android/app/build/outputs/apk` no está documentada por
  Tauri. El workflow la busca y, si no encuentra un APK firmado, muestra la lista de los que sí se
  generaron.

### Fase 2 (servidor dentro del celular)

Comprobado en Linux (no en Android):

- El lanzador lanza una **JVM real** con `dlopen` + `JLI_Launch` en un proceso hijo, con la salida al
  archivo y los comandos por la tubería; un servidor de prueba en Java imprime la línea «Done» de
  Minecraft, recibe `say` y `stop` y se apaga; el estado pasa por Encendiendo → En línea → Apagado;
  el agente acompañante (Playit) muere con el proceso del servidor (`PR_SET_PDEATHSIG`).
- Los dos Java para Android descargados se descomprimen con las reglas de seguridad y traen
  `libjli.so`, `libjvm.so` y las demás bibliotecas que se cargan.
- `playitd` v1.2.0 compila desde su código con `--locked` (en Linux x86_64).

- Catálogo y Geyser: la lógica (tipo → plugins/mods, Geyser obligatorio para la versión, quitar a
  la papelera, plan con dependencias) se probó con un catálogo de prueba. **No** se probó contra la
  API real de Modrinth desde este entorno (sin acceso a la red), ni que Geyser arranque dentro del
  celular.

**No** comprobado (no hay SDK/NDK de Android ni un celular en este entorno):

- Que `JLI_Launch` arranque dentro del proceso `:server` de Android (es la técnica de Amethyst, con el
  mismo Java, pero no la ejecuté en un celular).
- Que el código Kotlin del plugin compile y que el servicio en primer plano `specialUse` arranque bien
  en Android 14/15, y que el permiso de notificaciones se pida en Android 13+.
- Que `playitd` compile para `aarch64-linux-android` (depende de `ring` y otras bibliotecas con C) y
  que funcione dentro del celular (DNS, socket local en la carpeta de la app).
- Que `useLegacyPackaging` se aplique con la plantilla real de `tauri android init`.
- Un servidor real de Minecraft completo: en este entorno no se pueden descargar los servidores de
  Mojang (bloqueado), así que la prueba de punta a punta con Vanilla está escrita pero marcada como
  `#[ignore]` (`cargo test -p merc-phone -- --ignored end_to_end`, necesita internet y Java 21).
- Rendimiento, temperatura y consumo reales.
