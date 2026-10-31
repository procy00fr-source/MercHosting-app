# Permisos con LuckPerms

Esta pestaña cubre la fila 5 de `AUDITORIA_PENDIENTES.md` y el apartado 9 del apéndice técnico: listar grupos, usuarios, nodos, herencia, contextos y caducidades, y hacer cambios de forma verificable.

Está en **Juego → Permisos (LuckPerms)**. Es una pestaña avanzada: no aparece en el modo principiante. Además es solo del dueño, porque los compañeros no pueden ni leerla ni cambiarla.

## Cómo se detecta

MercHosting busca un `.jar` cuyo nombre contenga «luckperms»: primero en `plugins/` y, si no hay, en `mods/`. Según el código fuente de cada plataforma de LuckPerms, su carpeta de datos es:

| Dónde está el .jar | Carpeta de datos |
|---|---|
| `plugins/` (Paper, Purpur, Spigot e híbridos) | `plugins/LuckPerms` |
| `mods/` en Fabric o Quilt | `mods/luckperms` |
| `mods/` en Forge o NeoForge | `config/luckperms` |

Si no está instalado, la pestaña lo dice. Si el servidor admite plugins o mods, ofrece buscar LuckPerms en el catálogo.

## Cómo se leen los datos (verificable)

MercHosting **no abre la base de datos de LuckPerms**: por defecto es H2, un formato binario que el servidor tiene en uso. Hace lo siguiente:

1. Con el servidor encendido, manda `luckperms export merchosting-<marca de tiempo>`. `luckperms` es el nombre del comando en todas las plataformas y `lp` es un alias.
2. LuckPerms escribe `<nombre>.json.gz` en su carpeta de datos. Es el formato de su `Exporter`: `metadata`, `groups`, `tracks` y `users`, con cada nodo como `type`, `key`, `value`, `expiry` y `context`.
3. MercHosting espera hasta que el archivo esté completo (hasta 30 s), lo lee y lo borra.

Con el servidor apagado no se puede leer nada, y la pestaña lo explica.

Qué se muestra:

- **Grupos**: ordenados por peso, con su prefijo y de qué grupos heredan. También todos sus nodos: permisos, prefijos y metas. Cada nodo muestra si permite o niega, en qué contexto vale (`world=…`, `server=…`) y cuándo caduca, si es temporal.
- **Usuarios**: nombre, UUID, grupo principal, grupos fijos y permisos propios. Las herencias temporales o con contexto aparecen como nodos, con su caducidad.
- **Escalas de rangos** (tracks).

## Cambios

Solo estos cambios, cada uno traducido a **un único comando** con nombres validados:

| Cambio | Comando |
|---|---|
| Crear grupo | `luckperms creategroup <grupo>` (minúsculas, números, `_` y `-`, hasta 36 caracteres) |
| Borrar grupo | `luckperms deletegroup <grupo>` (el grupo `default` no se puede borrar) |
| Dar o negar un permiso | `luckperms group|user <x> permission set <nodo> true|false` |
| Quitar un permiso | `luckperms group|user <x> permission unset <nodo> [k=v…]` (con el mismo contexto que el nodo) |
| Añadir o quitar un grupo | `luckperms group|user <x> parent add|remove <grupo>` |

- A un usuario se lo indica por su UUID, tal como sale en el export, así no se confunden nombres.
- Un nodo no puede tener espacios, comillas, barras invertidas ni caracteres de control, y tiene hasta 200 caracteres. Por eso nada puede «colarse» en la consola.
- `group.x` se rechaza como permiso: para heredar un grupo hay que usar «Añadir grupo».

Cada cambio sigue estos pasos:

1. **Vista previa.** El supervisor hace un export y comprueba que el cambio tenga sentido: que el grupo exista, que no esté repetido, etc. Después muestra qué pasará, en una frase, y el comando exacto. No cambia nada hasta que se pulse «Aplicar».
2. **Copia.** Al aplicar, el export de ese momento se guarda en `instancias/<id>/luckperms-copias/<fecha y hora con milisegundos>.json.gz`, y se conservan las 10 últimas. Es el mismo formato que acepta `luckperms import`.
3. **Comando.** Se manda el comando validado.
4. **Comprobación.** Se hace otro export, hasta 3 veces, y se mira si el cambio aparece. Si aparece, se muestra «Hecho y comprobado». Si no, se avisa de que LuckPerms todavía no muestra el cambio y que conviene mirar la consola. No se da por hecho.
5. **Actividad.** Queda registrado como `luckperms.change`, con la frase, el comando y si se pudo comprobar.

## Pruebas

- `merc-admin` (`luckperms.rs`):
  - lectura de un export real comprimido con gzip, incluido uno a medio escribir;
  - validación de comandos;
  - comprobaciones previas y posteriores;
  - detección del plugin o mod en cada plataforma.
- `merc-supervisord` (`daemon.rs`, `luckperms_reads_previews_applies_and_verifies`): el servidor de prueba imita los comandos de LuckPerms y escribe el export con gzip como el real. La prueba recorre:
  - sin LuckPerms;
  - con LuckPerms y el servidor apagado;
  - lectura de los datos;
  - vista previa;
  - tres cambios comprobados;
  - cambios rechazados;
  - las copias, legibles y de antes de cada cambio;
  - la Actividad.
- Interfaz (`LuckPerms.test.tsx`):
  - sin LuckPerms;
  - con el servidor apagado;
  - grupos con herencia y contextos;
  - añadir un permiso con su vista previa;
  - nodo no válido;
  - crear un grupo;
  - grupos de un usuario;
  - aviso cuando el cambio no se pudo comprobar.

Falta probarlo con LuckPerms de verdad en Windows.
