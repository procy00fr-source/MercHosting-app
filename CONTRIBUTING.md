# Contribuir a MercHosting

¡Gracias por ayudar! Algunas reglas para que todo siga siendo seguro y honesto:

1. **Nada falso.** Nada de botones decorativos, métricas inventadas, URLs ni perfiles inventados. Si algo no está disponible, se dice y se desactiva.
2. **Los datos de la persona son sagrados.** Nada borra ni sobrescribe mundos o archivos sin consentimiento y copia previa; lo «borrado» va a la papelera.
3. **Sin secretos en el repositorio.** Claves de API, la clave privada del actualizador y contraseñas nunca se suben. `curseforge-api-key.txt` está en `.gitignore`.
4. **Fuentes oficiales.** Las descargas salen de la fuente oficial y se comprueba su huella. Antes de integrar un servicio de terceros, revisa su API, licencia y términos.
5. **Compañeros:** nunca ejecutan comandos del sistema en la PC del dueño, y Playit no transporta la administración.
6. **Pruebas.** Antes de un *pull request*:
   ```
   cargo fmt --all
   cargo clippy --workspace --all-targets
   cargo test --workspace
   cd app && npx tsc --noEmit -p . && npx vitest run
   ```
   Si cambias `app/src/styles.css`, ejecuta `node scripts/generar-modo-claro.mjs` desde la raíz. Si cambias el protocolo, regenera el contrato con `MERC_UPDATE_CONTRACT=1 cargo test -p merc-ipc`.
7. **Textos** en español neutro y cercano. Para el inglés, ver [docs/TRADUCCION.md](docs/TRADUCCION.md).

Al contribuir aceptas que tu aporte se publique bajo la licencia del proyecto (GPL-3.0-or-later).
