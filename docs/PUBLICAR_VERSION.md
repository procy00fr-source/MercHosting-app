# Publicar versiones y recibir comentarios

## 1. Actualizaciones automáticas (GitHub Releases)

Cómo funciona:

- La app instalada mira cada vez que se abre `https://github.com/<usuario>/<repo-releases>/releases/latest/download/latest.json`. Si hay una versión nueva, aparece un «1» en el botón de Ayuda. En **Ayuda → Actualizar MercHosting** se ven las novedades y el botón para instalar.
- Antes de instalar se detienen los servidores guardando el mundo (con confirmación) y se cierra el supervisor. Así el instalador puede reemplazar sus archivos.
- Cada instalador va **firmado** con tu clave privada, y la app solo instala lo que esté firmado con ella. Si alguien sube algo falso, no se instala.
- Esta firma es la del actualizador de Tauri, **no** la firma de código de Windows. La primera vez que alguien descargue el instalador, SmartScreen puede avisar igualmente.
- Las compilaciones de desarrollo (`abrir-app.ps1`) no se actualizan solas: en Ayuda lo dicen.
- Mundos, servidores y ajustes no se tocan: están en `%LOCALAPPDATA%\MercHosting` y en las carpetas de cada servidor.

### Preparar (una sola vez)

1. **Crea el repositorio público de releases** en GitHub, por ejemplo `MercHosting-releases`. Basta con un README; el código no va ahí.
2. **Instala GitHub CLI** (https://cli.github.com) y en una terminal ejecuta `gh auth login`. La sesión la guarda gh, no el proyecto.
3. **Genera la clave de firma** en la carpeta `app` del proyecto:

   ```
   npx tauri signer generate -w "%USERPROFILE%\.tauri\merchosting.key"
   ```

   Ponle contraseña. Se crean dos archivos:
   - `merchosting.key` (**privada**): no se la pases a nadie, no la subas y guárdala también en otro lado (un USB, un gestor de contraseñas). **Si la pierdes, las apps ya instaladas no podrán actualizarse más.**
   - `merchosting.key.pub` (pública): esta sí se puede compartir.
4. **Completa `release.json`** en la raíz del proyecto:

   ```json
   {
     "releases_repo": "TU_USUARIO/MercHosting-releases",
     "updater_pubkey": "contenido de merchosting.key.pub (una línea)"
   }
   ```

   El script se niega a seguir si por error pegas la clave privada.

### Publicar una versión

1. Escribe las novedades en `CHANGELOG.md` bajo `## 0.2.0` (son las notas que verá la gente).
2. Clic derecho en `scripts/publicar-version.ps1` → «Ejecutar con PowerShell». Te pide la versión y la contraseña de la clave (no se ve al escribirla).
3. El script hace esto:
   - sube la versión en los tres archivos;
   - compila el supervisor y el instalador NSIS firmado;
   - genera `latest.json` y `SHA256SUMS.txt`;
   - crea la release `v0.2.0` en tu repositorio de releases.
   - Los archivos quedan también en `target/release-publicar/0.2.0/`.
4. Para probar sin publicar, responde «S» a «¿Solo compilar sin subir?».

Si un instalador sale mal, borra esa release en GitHub: las apps vuelven a ver la anterior como la última.

## 2. Comentarios (Google Forms)

Los comentarios se envían con un formulario de Google tuyo, así no hace falta un servidor ni hay claves dentro de la app:

- En la app aparece **Comentarios** en el menú. La persona escribe el tipo, el mensaje y, si quiere, cómo responderle, y elige si incluye la versión de MercHosting.
- La app abre tu formulario en el navegador **ya rellenado**. La persona lo revisa y pulsa «Enviar» en Google. Hasta entonces no se manda nada, y la app no adjunta nada más.
- A ti te llega un correo por cada respuesta, y todas quedan en la pestaña Respuestas, o en una hoja de cálculo si la vinculas.

### Crear el formulario

1. En https://forms.google.com crea un formulario «Comentarios de MercHosting» con 4 preguntas:
   - **Tipo**: opción múltiple con las opciones `Idea`, `Error` y `Otro`, escritas exactamente así;
   - **Mensaje**: párrafo;
   - **Contacto**: respuesta corta, no obligatoria;
   - **Versión**: respuesta corta, no obligatoria.
2. En **Configuración**, desactiva que se pida iniciar sesión, para que cualquiera pueda enviar.
3. En la pestaña **Respuestas → ⋮**, activa las notificaciones por correo de respuestas nuevas (en inglés: «Get email notifications for new responses»).
4. En el menú **⋮** del formulario, elige obtener el vínculo prellenado (en inglés: «Get pre-filled link») y rellena así:
   - Tipo: `Idea`;
   - Mensaje: `MENSAJE`;
   - Contacto: `CONTACTO`;
   - Versión: `VERSION`.

   Luego pulsa el botón para obtener el vínculo y cópialo.
5. Con ese vínculo se completa `app/src/feedback.ts` (`FEEDBACK_FORM`). Mientras esté en `null`, la sección Comentarios no aparece.
