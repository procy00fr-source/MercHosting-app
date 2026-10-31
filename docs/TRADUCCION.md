# Idiomas (español e inglés)

MercHosting está escrito en español. El inglés se elige en **Apariencia → Idioma · Language** (o se usa solo si Windows no está en español y nadie eligió nada). Al cambiar, la ventana se recarga; el supervisor y los servidores siguen en marcha.

## Cómo funciona

- `app/src/i18n/en.json`: diccionario español → inglés. `exact` (textos completos) y `patterns` (textos con valores: `Hay {0} mods` → `Found {0} mods`). Los huecos se traducen a su vez si son textos conocidos, y las listas `a · b · c` se traducen por partes.
- `app/src/i18n/dom.ts`: traduce lo que React pinta (textos y `placeholder`, `title`, `aria-label`, `alt`) y sigue sus cambios. **No** traduce: consola, editor, campos de texto, rutas (`.mono`), ni nada dentro de `translate="no"` (nombres de servidores, mundos y jugadores). Lo que no está en el diccionario se queda en español: nunca se inventa una traducción.
- Los mensajes del supervisor (errores, registros de trabajos, diagnóstico) también pasan por el diccionario al mostrarse.
- Fechas y números usan el formato del idioma elegido (`uiLocale()`).
- Las notificaciones de Windows se traducen con `t()`.

## Añadir o corregir textos

1. Extraer los textos actuales:
   ```
   npm i --prefix /tmp/ts5 typescript@5
   TS5=/tmp/ts5/node_modules/typescript node scripts/i18n/extract-ui.cjs i18n-ui.json
   python3 scripts/i18n/extract-rust.py i18n-rust.json
   python3 scripts/i18n/missing.py i18n-ui.json i18n-rust.json
   ```
2. Añadir a `en.json` las entradas que falten (texto en español exacto como clave; los valores con `{0}`, `{1}`… en `patterns`).
3. Para textos que el código arma con trozos en español elegidos al vuelo (plurales como `servidor${n === 1 ? "" : "es"}`), conviene que el elemento tenga solo texto (sin etiquetas dentro): así se traduce la frase entera con un patrón.

## Límites conocidos

- Textos que aparecen solo en diálogos poco frecuentes pueden faltar; se ven en español hasta añadirlos.
- Lo que escriben las personas (notas, MOTD, nombres) no se traduce.
