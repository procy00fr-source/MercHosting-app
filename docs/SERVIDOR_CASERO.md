# Servidor casero sin pantalla

Puedes dejar MercHosting funcionando en una PC vieja o un servidor sin monitor y manejarlo desde
MercHosting en tu PC de siempre. La conexión es la de **Compañeros**: directa entre las dos PCs,
cifrada y sin cuentas. No hay panel web ni hace falta abrir puertos en el router.

## Qué necesitas

- La PC del servidor casero, con Linux x86_64 (Ubuntu o Debian recomendados) y conexión a Internet.
  También sirve Windows (más abajo).
- Tu PC normal con MercHosting.
- Saber abrir una terminal en el servidor casero (o entrar por SSH).

Todavía no hemos probado esto en ninguna distribución concreta fuera de nuestras pruebas
automáticas. Si algo falla, cuéntanos tu distribución y versión.

## Paso a paso (Linux)

### 1. Instala MercHosting en el servidor casero

Elige una de estas dos opciones desde las releases oficiales
(https://github.com/procy00fr-source/MercHosting-releases/releases/latest):

- **Sin escritorio** (recomendado para un servidor): `MercHosting-servidor-casero-linux-x86_64.tar.gz`.
  Solo trae los dos programas de terminal (`merc` y `merc-supervisord`), sin librerías gráficas.

  ```sh
  mkdir -p ~/merchosting && cd ~/merchosting
  tar -xzf ~/Descargas/MercHosting-servidor-casero-linux-x86_64.tar.gz
  ```

  Comprueba la huella con `SHA256SUMS-linux.txt` de la misma release (`sha256sum -c`).

- **Con el .deb** (si esa PC tiene escritorio): `sudo apt install ./MercHosting-linux-amd64.deb`.
  Trae la app y también `merc` en `/usr/bin`.

### 2. Déjalo funcionando en segundo plano

```sh
./merc servicio instalar      # o «merc servicio instalar» si usaste el .deb
```

Esto crea un servicio de usuario de systemd (`~/.config/systemd/user/merchosting.service`) y lo
activa. No necesita sudo. MercHosting arranca solo al encender la PC y se vuelve a abrir si se
cae. Los servidores marcados con «Encender este servidor al iniciar» se encienden solos.

Para que funcione **sin que nadie inicie sesión** (por ejemplo, tras un corte de luz), systemd
necesita «lingering». Eso sí pide permisos de administrador, así que MercHosting no lo hace por
ti. Te muestra el comando para ejecutarlo una vez:

```sh
sudo loginctl enable-linger TU_USUARIO
```

Comprueba que todo está bien con `merc servicio estado`.

### 3. Crea el código de control total

```sh
./merc compañeros invitar --control-total
```

Te muestra un código `MH1-…` que vale 30 minutos (cámbialo con `--minutos 60`) y sirve **una
sola vez**. No lo compartas.

### 4. Pégalo en tu PC

En MercHosting de tu PC: **Compañeros › Conectar a mi servidor casero** (dentro de
«Administrar servidores de un amigo»). Pega el código, escribe tu nombre y pulsa **Unirme**.

Ahí aparece tu servidor casero con **Crear servidor aquí**. Desde ese momento lo manejas como si
fuera local: crear servidores, encender, consola, archivos, mods, copias, etc.

## Qué puede hacer la otra PC con el control total

- Ver y administrar **todos** los servidores del servidor casero, también los que se creen después.
- **Crear servidores** (tipos, versiones y modpacks de siempre) y aceptar la EULA de Minecraft.
  La acepta la persona en su pantalla con la misma casilla de siempre, y queda en la Actividad
  quién y cuándo.
- Descargar Java (Eclipse Temurin) en el servidor casero si falta.
- Subir y descargar archivos de los servidores.

## Qué NO puede hacer nunca (seguridad)

- **Elegir dónde se escribe.** El servidor nuevo va siempre a una carpeta nueva dentro de la
  carpeta de servidores del casero (`~/MercHosting/Servidores/<nombre>`). La ruta que mande la
  otra PC se ignora.
- **Elegir qué programa se ejecuta.** El Java lo elige el casero entre los suyos: el que pide esa
  versión de Minecraft. Java 8 exacto para Forge antiguo; un JDK para Spigot. Cambiar el Java de
  un servidor sigue siendo solo local.
- Ejecutar comandos del sistema. Solo comandos de Minecraft en la consola del servidor.
- Leer ni escribir fuera de las carpetas de los servidores. Ver rutas de esa PC: se borran de las
  respuestas.
- Gestionar compañeros (crear invitaciones, dar permisos), Playit, apagar el supervisor, quitar
  o borrar servidores, forzar el cierre, LuckPerms o el arranque con el sistema. Eso se hace en
  el propio casero con `merc` (`merc quitar`, `merc forzar-cierre --confirmo`, etc.).
- Importar addons por ruta. En Bedrock, sube el `.mcpack` con Archivos o hazlo en el casero.

Cada acción queda en la Actividad del servidor con el nombre de quien la hizo.

## Ver y quitar el acceso

```sh
./merc compañeros lista              # quién tiene acceso, conectado o no, e invitaciones abiertas
./merc compañeros quitar "Mi PC"      # por nombre o por huella; corta la conexión al momento
./merc compañeros pausar             # nadie puede conectarse hasta «merc compañeros activar»
```

La huella de cada PC (`ABCD-EFGH-IJKL`) sirve para comprobar de palabra que es la tuya.

## Jugar desde Internet

Compañeros solo sirve para administrar. Para que tus amigos **jueguen** desde fuera de tu red,
activa Playit en el propio servidor casero (Linux x86_64) desde la terminal:

```
./merc playit activar      # descarga el agente oficial (comprobado) y lo arranca
./merc playit vincular     # te da un enlace de playit.gg: ábrelo en tu celular o PC y aprueba
./merc playit estado       # direcciones de tus túneles (créalos en playit.gg)
```

En playit.gg crea el túnel «Minecraft Java» (o «Minecraft Bedrock») hacia el puerto del
servidor. Después, la pestaña Conexión que ves desde tu PC muestra esa dirección y la prueba.
Playit es solo para jugar: la administración va siempre por Compañeros.

## Apagarlo o quitarlo

```sh
./merc servicio quitar      # deja de arrancar solo; no borra servidores ni datos
./merc apagar-supervisor    # cierra MercHosting si no hay servidores encendidos
```

Los datos están en `~/.local/share/MercHosting` y los servidores en `~/MercHosting/Servidores`.

## En Windows

Funciona igual, pero desde la app: en el casero abre MercHosting y ve a **Compañeros › Control
total (servidor casero)**. En Ajustes de cualquier servidor activa **Arrancar MercHosting con el
sistema**. `merc servicio instalar` en Windows hace eso mismo (al iniciar sesión). Para que
arranque sin que nadie inicie sesión, Windows necesita el inicio de sesión automático; eso se
configura en Windows, no en MercHosting.
