# Fase 8 · incremento 2: Archivos entre PCs de compañeros

Con los permisos adecuados, un compañero ya puede llevarse archivos del servidor de su amigo a su PC y subir archivos desde su PC. Todo viaja por el mismo canal cifrado de compañeros.

## Qué se puede hacer desde la PC del compañero
| Acción | Permiso que exige |
|---|---|
| Descargar un archivo del servidor (botón de descarga en Archivos) | Leer y descargar archivos |
| Subir un archivo (Archivos › Subir archivo) | Editar, crear, renombrar, subir y restaurar archivos (alto riesgo) |
| Exportar un mundo a .zip (con el servidor detenido) | Mundos |
| Descargar una copia de seguridad (botón nuevo «Descargar» en Copias) | Ver y crear copias de seguridad |
| Exportar el registro de consola | Leer la consola |

- Los diálogos para elegir archivo o destino son los de **la PC del compañero**: lo que baja llega a su PC y lo que sube sale de ella.
- Importar un mundo desde un .zip y abrir carpetas sigue siendo cosa del dueño en su PC.
- El botón «Descargar» de las copias también sirve en tus propios servidores: guarda el .zip donde elijas.

## Cómo viaja (lo decide la PC del dueño)
- **Descargar:**
  1. El compañero pide «prepara esto». La PC del dueño comprueba el permiso **en ese momento** y genera el archivo con las mismas reglas que en el uso local. Por ejemplo, la ruta tiene que quedar dentro de la carpeta del servidor (sin `..` ni enlaces que salgan) y el mundo tiene que estar detenido para exportarlo.
  2. El archivo se deja en una carpeta temporal de MercHosting, **fuera del servidor**, junto con su **SHA-256**.
  3. El compañero lo pide en trozos de 1 MiB. Solo puede pedir lo que preparó él.
  4. Al terminar se borra del lado del dueño. En la PC del compañero se escribe primero como `.merchosting-parte` y solo se renombra si la huella coincide; si no, se borra.
- **Subir:**
  1. Trozos de 1 MiB, en orden y con tope de 2 GB. Exige el servidor detenido y un nombre de archivo sin carpetas.
  2. El último trozo lleva la huella. La PC del dueño comprueba que llegó entero y que coincide **antes** de colocarlo en el servidor, y lo coloca con la misma función que la subida local (que no sobrescribe sin copia).
- **Límites:** 3 descargas y 2 subidas a medias por compañero. Lo que nadie termina se borra a las 2 horas, y al arrancar el supervisor se limpian los restos.
- Todo queda en la Actividad del servidor a nombre de `companero:<nombre>`. Lo denegado también.

## Pruebas
- Supervisor (dos supervisores reales):
  - descargar `server.properties` y comparar bytes;
  - rechazar mundos y copias sin permiso, y archivos de otro servidor;
  - intentar salir de la carpeta con `..` (lo rechaza la protección de rutas);
  - subir sin permiso (rechazado);
  - con permiso, subir y volver a bajar un archivo de 2,5 MiB (varios trozos) y comprobar que llega idéntico;
  - comprobar que no quedan restos en la carpeta de datos del dueño.
- UI: el compañero descarga un archivo del amigo a su PC. Sin permiso de edición no aparecen «Subir archivo», «Nueva carpeta», renombrar ni papelera, y nunca aparece «Ver carpeta del servidor».
