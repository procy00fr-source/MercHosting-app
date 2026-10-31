# Separador de mods cliente/servidor

Dónde está: **Mods → Separar cliente/servidor** (solo en servidores con mods).

## Interruptor (apagado por defecto)

- Cada servidor tiene su interruptor **Activar / Desactivar el separador**. Viene **apagado**.
- Apagado: no se revisa ningún mod, no hay sugerencias ni avisos.
- Encendido: se revisan los mods y, si la lista de instalados detecta mods que no funcionan en servidores, muestra un aviso con el botón **Revisarlos**.
- Ni encendido se mueve nada solo: **no se marca ningún mod automáticamente**. El botón «Marcar los sugeridos» los marca si tú lo pides, y siempre hay que confirmar.
- Apagarlo no devuelve los mods apartados: siguen en `mods-cuarentena/` y se devuelven activándolo otra vez.
- La preferencia se guarda en este equipo (como el modo principiante/avanzado).

## Qué hace

1. Revisa **todos** los `.jar` de `mods/` (también los desactivados) y decide de qué lado es cada uno:
   - **Solo cliente**: el catálogo (Modrinth o CurseForge) dice que no funciona en servidores, o la ficha del jar lo declara así (Fabric `environment: "client"`; Forge/NeoForge con todas sus dependencias base marcadas `side="CLIENT"`).
   - **Solo servidor**: el catálogo dice que no hace falta en el juego de los jugadores.
   - **Cliente y servidor**: el catálogo indica que va en los dos.
   - **Sin datos**: no hay información o las fuentes se contradicen (por ejemplo, el jar dice cliente pero el catálogo dice que sí va en servidores). En ese caso no se sugiere moverlo.
2. Cada mod muestra su **confianza** y el **porqué** (las pruebas concretas que se usaron).
3. Si otro mod activo que sí se queda en el servidor **depende** de él, se avisa y no se preselecciona.
4. Se **sugieren** (sin marcarlos) solo los de «solo cliente» con confianza alta y sin dependientes. Tú decides; nada se mueve sin confirmar.

## Cuarentena (reversible)

- Los mods elegidos se **mueven** a `mods-cuarentena/` dentro de la carpeta del servidor. No se borra nada.
- Se guarda un registro (`mods-cuarentena/merchosting-cuarentena.json`) con la fecha y el motivo.
- **Devolver** (uno o todos) los vuelve a `mods/`. Si ya existe un archivo con ese nombre en `mods/` (o su gemelo `.disabled`), no se sobrescribe: se avisa y no se mueve nada.
- La operación es todo o nada: si un archivo falla, ninguno se mueve.
- Requiere el **servidor apagado** (Minecraft carga los mods al arrancar).
- Queda en la Actividad (`mods.quarantine` / `mods.restore`).

## Límites honestos

- La clasificación depende de lo que declaran el autor del mod y los catálogos; puede haber errores. Por eso hay «Sin datos», la evidencia visible y la cuarentena reversible.
- Sin conexión, se usa sobre todo la ficha de cada jar y se avisa.
- Tras mover mods, enciende el servidor para comprobar que arranca bien; si algo falla, devuélvelos.
- Compañeros: estas operaciones requieren el permiso de Contenido.
