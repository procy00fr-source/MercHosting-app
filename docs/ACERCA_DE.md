# Acerca de, licencia y documentos públicos

- **Acerca de** (barra lateral): versión, autor, licencia GPL-3.0-or-later con su texto completo, enlace al código fuente, publicaciones, novedades (de `CHANGELOG.md`), política de privacidad (`docs/PRIVACIDAD.md` / `docs/PRIVACIDAD.en.md`), componentes de terceros con su licencia (`app/src/about/terceros.json`), redes del autor, Apoyar y Comentarios.
- **Código fuente:** `app/src/about/links.ts` → `SOURCE_URL`. Mientras sea `null`, el botón sale desactivado con una explicación. Al publicar el repositorio, pon ahí su URL y añade su dominio a `app/src-tauri/capabilities/default.json` si no es `github.com/procy00fr-source/*`.
- **GPL-3.0:** quien reciba el instalador tiene derecho al código fuente de esa versión. Publica el repositorio (o el código de cada versión) antes o junto con la versión que lo anuncie.
- **Licencias de terceros:** `node scripts/licencias-terceros.mjs` regenera `docs/LICENCIAS_DE_TERCEROS.md` y la lista de la app, y avisa si aparece una licencia que no esté en la lista de compatibles con GPL-3.0. Hazlo cuando cambien las dependencias.
- **Guía de instalación:** `docs/INSTALACION.md`.
