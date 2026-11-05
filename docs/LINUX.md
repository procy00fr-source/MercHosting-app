# MercHosting en Linux

MercHosting también funciona como app de escritorio en Linux, con ventana, igual que en Windows. **Todavía no se ha probado en una distribución real**: compila y pasa las pruebas automáticas en Ubuntu, pero ninguna distribución está confirmada aún. Si lo pruebas, cuenta en los comentarios qué distro usas y qué falló.

## Requisitos

- Linux de 64 bits (x86_64) con escritorio gráfico.
- El paquete `.deb` es para Debian, Ubuntu y derivadas (Linux Mint, Pop!_OS…). Pide `libwebkit2gtk-4.1-0` y `libgtk-3-0`, que el gestor de paquetes instala solo.
- El `AppImage` sirve en otras distribuciones si tienen WebKitGTK 4.1.
- Java: si falta, MercHosting descarga Eclipse Temurin (`.tar.gz` oficial de Adoptium, con su huella SHA-256) en su propia carpeta. También reconoce el Java de la distribución (`/usr/lib/jvm`) y el de SDKMAN!.

## Instalar

- **.deb:** `sudo apt install ./MercHosting-linux-amd64.deb`, o doble clic para abrirlo con el instalador de paquetes.
- **AppImage:** `chmod +x MercHosting-linux-x86_64.AppImage` y ábrelo. Algunas distribuciones necesitan `libfuse2` para abrir cualquier AppImage.

- **Servidor casero sin escritorio:** `MercHosting-servidor-casero-linux-x86_64.tar.gz` trae solo `merc` y `merc-supervisord` (sin librerías gráficas). Mira [SERVIDOR_CASERO.md](SERVIDOR_CASERO.md). El `.deb` también instala `merc` en `/usr/bin`.

Comprueba la descarga con `sha256sum` y compárala con `SHA256SUMS-linux.txt` de la misma release.

## Qué funciona distinto

| Función | En Linux |
| --- | --- |
| Actualizarse sola | Solo el AppImage. El `.deb` avisa de la versión nueva y te lleva a la página de releases. |
| Arrancar al iniciar sesión | Con un archivo en `~/.config/autostart/merchosting.desktop` (estándar XDG). |
| No suspender la PC | Con `systemd-inhibit`. Si tu sistema no lo tiene, la app lo dice. Mientras está activo, el escritorio puede avisarte de que MercHosting impide suspender. |
| Playit.gg | **Incluido (Linux x86_64)**: el agente oficial v1.2.0 (`playit-linux-amd64` de la release oficial de playit-cloud/playit-agent), descargado y comprobado con su SHA-256 `602dea32…78d0` y su tamaño. En otras arquitecturas (ARM) no hay agente verificado: usa su [guía oficial](https://playit.gg/support/run-on-linux) o abre el puerto en tu router. |
| Servidores Bedrock | Mojang solo da soporte oficial en Ubuntu 22.04 o más nuevo. |
| Spigot (BuildTools) | Necesita Git instalado (`sudo apt install git`); en Linux BuildTools no lo descarga. |
| Forge y NeoForge | Usan `run.sh` y `unix_args.txt`, como manda su instalador oficial. |
| Papelera | La papelera del escritorio (estándar freedesktop). |

Carpetas:

- datos de la app y copias de seguridad: `~/.local/share/MercHosting` (o `$XDG_DATA_HOME/MercHosting`);
- servidores nuevos: `~/MercHosting/Servidores`.
- En el AppImage, el supervisor se copia a `~/.local/share/MercHosting/bin/`, porque el AppImage se desmonta al cerrarse y el supervisor tiene que seguir funcionando.

## Publicar la versión de Linux (para el autor)

Los paquetes de Linux se compilan en GitHub Actions (Ubuntu): no hace falta tener Linux. El workflow es `.github/workflows/linux.yml` del repositorio del código.

### Una sola vez: los secretos

En el repositorio **MercHosting-app**: *Settings → Secrets and variables → Actions → New repository secret*.

1. **`TAURI_SIGNING_PRIVATE_KEY`**: el contenido de tu `%USERPROFILE%\.tauri\merchosting.key`. Es la MISMA clave con la que firmas las versiones de Windows. Nunca la pegues en ningún archivo del repositorio.
2. **`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`**: su contraseña. Si no tiene, crea el secreto vacío.
3. **`RELEASES_TOKEN`**: un token de GitHub *fine-grained* con acceso **solo** al repositorio `procy00fr-source/MercHosting-releases` y permiso **Contents: Read and write**. Se crea en *GitHub → Settings → Developer settings → Personal access tokens → Fine-grained tokens*.
4. **`MERC_CURSEFORGE_API_KEY`**: la clave de API de CurseForge que te dio Overwolf. Queda dentro del programa al compilar; sin ella, la versión de Linux sale con CurseForge en «Próximamente» (el workflow avisa). No la pegues en ningún archivo ni se la pases a nadie: las condiciones de CurseForge no lo permiten.

### Cada versión

1. Publica la versión de Windows como siempre (`node scripts/publicar-version.mjs X.Y.Z`): crea la release y su `latest.json`.
2. Sube tus cambios con `git push`, para que GitHub tenga el código de esa versión.
3. En GitHub: *Actions → Linux → Run workflow*, escribe la versión (`X.Y.Z`) y ejecútalo.
4. El workflow pasa las pruebas, compila, firma y añade a la misma release:
   - el `.deb` y el `AppImage`, también con nombres fijos (`MercHosting-linux-amd64.deb`, `MercHosting-linux-x86_64.AppImage`) para enlazarlos desde la web;
   - `SHA256SUMS-linux.txt`;
   - el `latest.json` con Linux añadido (sin tocar la parte de Windows).

Para probar sin publicar, desmarca «Subir a la release»: deja los archivos como artefacto del workflow.
