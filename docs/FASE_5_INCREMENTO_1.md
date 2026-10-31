# Fase 5 · incremento 1: mods y plugins desde Modrinth y CurseForge

Cada servidor que admite contenido tiene una pestaña nueva: **Mods** (Fabric, Quilt, Forge,
NeoForge), **Plugins** (Paper, Purpur, Spigot) o **las dos** (Arclight, con aviso de
compatibilidad experimental). Vanilla no tiene ninguna.

## Instalados
- Lista los `.jar` de `mods/` o `plugins/`. Cada archivo se reconoce **por su huella**: SHA-1 en
  Modrinth y huella MurmurHash2 en CurseForge. Así se sabe el nombre, la versión y si hay
  **actualización** compatible, aunque el servidor se haya importado.
- **Desactivar/activar**: cambia `x.jar` ↔ `x.jar.disabled` (Minecraft y Bukkit solo cargan
  `.jar`). No se borra nada.
- **Quitar**: va a la papelera de MercHosting (se restaura desde Archivos › Papelera y copias).
- Sin conexión se siguen viendo los archivos; se avisa de que faltan los datos del catálogo.

## Buscar e instalar
- Búsqueda filtrada a lo compatible: loader del servidor y versión de Minecraft. En mods de
  Modrinth, solo los que funcionan en servidor (`server_side` obligatorio u opcional).
- Ficha con descripción, descargas, licencia, versiones compatibles (las beta/alfa se marcan) y
  registro de cambios. Los textos de los catálogos se muestran como texto, nunca como HTML.
- **Plan antes de instalar** («Qué se va a instalar»): versión elegida, dependencias obligatorias
  resueltas de forma recursiva (sin ciclos), las que ya tienes, las opcionales (no se instalan
  solas) y lo que se reemplaza. Bloquea si:
  - es solo para el cliente (el juego de cada jugador);
  - es incompatible con algo instalado;
  - falta una dependencia para tu versión;
  - ya tienes esa misma versión;
  - el catálogo no publica la huella para verificarla;
  - en CurseForge, el autor no permite descargas desde otras apps (se explica cómo hacerlo a mano).
- **Instalación**: con el servidor detenido.
  - El supervisor recalcula el plan (no se fía del de la interfaz).
  - Descarga todo a una carpeta temporal fuera del servidor y verifica cada SHA-1.
  - Solo entonces manda a la papelera lo que se reemplaza y mueve los archivos a su sitio.
  - Si algo falla o cancelas, el servidor queda igual.
- Barra de progreso, cancelación y registro en Actividad.

## Modrinth
API pública v2, sin cuenta ni clave, con el `User-Agent` de MercHosting que pide Modrinth. Lo
instalado se consulta por lotes (una petición para todas las huellas).

## CurseForge (requiere clave de API)
CurseForge exige una **clave de API** que Overwolf concede a cada aplicación tras revisar una
solicitud ([cómo pedirla](https://support.curseforge.com/support/solutions/articles/9000208346)).
La clave **no va en el repositorio**. Mientras no haya clave, CurseForge no aparece en la app (no
hay botones que no funcionen). Cuando la tengas:
- **Para probar en tu PC**: crea `%LOCALAPPDATA%\MercHosting\curseforge-api-key.txt` con la clave
  (una sola línea). El supervisor la lee al momento, sin reiniciar. El archivo está en `.gitignore`.
- **Para las versiones que publiques**: compila con la variable de entorno
  `MERC_CURSEFORGE_API_KEY` y se incrusta en el ejecutable.

En esta versión, CurseForge se usa solo para **mods**. Los plugins de Bukkit en CurseForge son
casi todos antiguos; para plugins está Modrinth (y más adelante Hangar).

## Protocolo (compatible con v1)
`content_platforms`, `content_search`, `content_project`, `content_plan`, `content_install`,
`content_installed`, `content_toggle`, `content_remove`. El campo `source` (`modrinth` o
`curseforge`) es opcional y por defecto vale `modrinth`.

## Otros cambios
- La animación de inicio dura al menos 4,5 s para que se lea el eslogan (más grande). Ya conectada,
  un clic o una tecla la salta.
- Los diálogos se dibujan sobre toda la ventana (antes se desplazaban si la pestaña tenía scroll).

## Pruebas
- Rust: 142. Nuevas:
  - plataformas por tipo de servidor;
  - nombres de archivo seguros;
  - plan con dependencias recursivas, ciclos, instaladas, incompatibles, solo cliente, actualización y misma versión;
  - activar/desactivar;
  - MurmurHash2;
  - lectura de las respuestas documentadas de las dos APIs;
  - de punta a punta con servidores HTTP falsos (búsqueda, plan, descarga verificada, descarga dañada que no toca nada, reconocimiento por huella, autor que no permite distribución, clave solo en cabecera);
  - supervisor real: pestañas, rechazo de CurseForge sin clave, instalación con dependencia, servidor encendido bloquea, desactivar, quitar a la papelera, actividad.
- Interfaz: 67 (8 nuevas). Capturas: `docs/capturas/fase5-*.png`.
- **No probado contra los servicios reales** desde aquí (este entorno no llega a Modrinth ni a
  CurseForge): la primera prueba real es en tu PC.

## Pendiente (siguientes incrementos de la fase 5)
Modpacks y server packs (Modrinth `.mrpack` y CurseForge), Hangar para plugins de Paper,
datapacks y resource packs, comparar y revertir actualizaciones en bloque.
