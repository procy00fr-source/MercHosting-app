# Fase 9 · incremento 1: Diagnóstico (analizador de crashes y detector de conflictos)

Nueva pestaña **Diagnóstico** en cada servidor, y algo que faltaba: **Java y memoria** en Ajustes.

## Qué revisa
Se revisan las fichas de los jars (no solo su nombre), la versión real del Java configurado y lo que escribió el servidor:
- la consola de la última sesión;
- el final de `logs/latest.log`;
- el `crash-reports/*.txt` más reciente, si es de los últimos 7 días.

Arriba se muestra siempre **qué se revisó**, con la fecha de cada registro.

**Sin necesidad de arrancar** (fichas de los jars):
- **Mod para otro loader**: por ejemplo, uno solo de Fabric en NeoForge. Si está instalado Sinytra Connector, baja a confianza baja.
- **Mismo mod dos veces** (mismo ID en dos jars activos).
- **Dependencias obligatorias que faltan**, leídas de las fichas:
  - Fabric y Quilt: `depends`;
  - NeoForge: `type="required"`;
  - Forge: `mandatory=true`;
  - plugins: `depend` de `plugin.yml`.

  Cuentan los IDs que traen los jars incluidos (Fabric API y `jarjar`). Solo se comprueba que la dependencia exista, **no su versión**, y así se dice.
- **Mods solo para el cliente**: los que Fabric o Quilt declaran con `environment: client`.
- **Java demasiado antiguo** para la versión de Minecraft.
- **Mods o plugins en un servidor que no los carga.** Un archivo sin ficha de plugin en `plugins/`.

**En los registros** (patrones conocidos, con la línea exacta como prueba):
- **Java incompatible** (`UnsupportedClassVersionError`): calcula qué Java hace falta a partir de la versión de clase, por ejemplo 65 → Java 21.
- **Dependencias** según Fabric («requires …, which is missing») y según NeoForge/Forge («Mod ID: …, Actual version: '[MISSING]'» o una versión que no encaja).
- **Sin memoria** (`OutOfMemoryError`).
- **Puerto ocupado** (`FAILED TO BIND TO PORT`).
- **Clase de cliente en el servidor** («invalid dist DEDICATED_SERVER»).
- **Mod señalado** por el informe de error («Suspected Mods»).
- **Mixin que falla** en un mod.
- **Servidor trabado** (watchdog).
- **Partes del mundo ilegibles.**

Cada hallazgo tiene **nivel de confianza** (alta, media o baja), una explicación para novatos y su **prueba** (la línea del registro o el archivo, y de dónde sale). Si no se encuentra nada, se dice que eso **no garantiza** que todo esté bien.

## Arreglos (reversibles y confirmados)
| Arreglo | Qué hace |
|---|---|
| Desactivar un mod o plugin | Lo renombra a `.jar.disabled`; no se borra nada. Con el servidor detenido. |
| Buscar la dependencia | Abre Mods o Plugins con la búsqueda ya hecha en el catálogo. |
| Usar otro puerto | Propone un puerto que no use otro servidor y que se pueda abrir ahora. Se guarda una copia de `server.properties`. |
| Subir la memoria | Propone ×1,5 (al menos +1 GB), sin pasar del 75 % de la memoria de la PC. |
| Elegir Java N o más nuevo | Muestra los Java de la PC que sirven. Si no hay ninguno, explica cómo instalarlo (Eclipse Temurin, adoptium.net). |
| Abrir copias de seguridad | Para restaurar si parte del mundo está dañada. |

Cuando el servidor se cierra solo, aparece **«Ver diagnóstico»** en el aviso.

## Java y memoria (Ajustes)
Hasta ahora la memoria y el Java solo se elegían al crear el servidor. Ahora, con el servidor detenido:
- **Memoria máxima**: se comprueba que no pase del 90 % de la memoria de la PC.
- **Java**: el Java elegido se prueba de verdad (su versión real) y se rechaza si es más antiguo de lo que necesita esa versión de Minecraft.

Queda anotado en la Actividad.

## Compañeros
- Diagnóstico: permiso «Ver el historial».
- Cambiar la memoria: permiso «Ajustes».
- **Elegir un Java solo lo puede hacer el dueño**, porque es elegir un programa de su PC. Está en la lista de abusos que la prueba de compañeros comprueba.

## Pruebas
- `merc-admin`, 6 pruebas:
  - fichas de Fabric con jars internos y `environment`, de NeoForge, de Forge y de plugins (`depend` en línea y en lista);
  - sin problemas no hay hallazgos;
  - duplicados, dependencias, loader equivocado, solo cliente y Java viejo, con sus arreglos;
  - registros reales de NeoForge, Fabric y un crash report, con la línea exacta como prueba;
  - dependencias de plugins y plugins ignorados.
- Supervisor:
  - diagnóstico de un crash report real con su fuente;
  - cambiar la memoria;
  - rechazar un Java que no es una ruta completa o una memoria demasiado baja;
  - aceptar un Java real.
- UI, 5 pruebas:
  - del aviso de cierre inesperado al diagnóstico, con evidencias;
  - confirmación y desactivación reversible, y subir la memoria;
  - «Buscar» abre el catálogo ya buscando;
  - sin problemas no promete nada;
  - Java y memoria en Ajustes.

Capturas: `docs/capturas/fase9a-diagnostico.png` y `fase9a-java-memoria.png`.
