# Fase 1, incremento 3: primera ventana de MercHosting

Fecha: 30-10-2026 · Estado: **completado y probado en Linux con la app real; pendiente de abrir en Windows**

## 1. Qué hace la ventana

Solo funciones reales; no hay botones de módulos futuros.

- **Lista de servidores** con su estado real, que se actualiza cada segundo desde el supervisor.
- **Importar servidor**: selector de carpeta del sistema → detección (tipo, versión, loader, nivel de confianza, evidencias y advertencias) → nombre, memoria máxima y Java recomendado → registro. No modifica la carpeta.
- **Iniciar / Detener (con guardado) / Reiniciar / Forzar cierre / Quitar de la lista**. Cada botón aparece solo en los estados donde tiene sentido. «Forzar cierre» exige marcar «Entiendo que se puede perder lo que no se haya guardado». «Quitar de la lista» aclara que no se borran archivos.
- **Consola en vivo**: salida del servidor, filtro, «seguir el final», envío de comandos de Minecraft e historial con ↑/↓.
- **EULA**: si falta, la ventana explica qué es, ofrece «Leer la EULA» (abre https://aka.ms/MinecraftEULA) y solo la acepta si marcas «He leído la EULA de Minecraft y la acepto». El supervisor escribe `eula.txt` y lo deja auditado.
- **Servidor huérfano** (sigue vivo después de una caída del supervisor): se explica que la consola no se puede recuperar y cómo apagarlo guardando (`/stop` en el juego).
- **Detalles** con datos reales: puerto (en uso, según `server.properties` o por defecto), cómo entrar desde esta PC, PID, última salida, memoria, Java, carpeta y archivo de registro.
- **Sin conexión con el supervisor**: el mensaje explica el error y hay un botón para reintentar.

## 2. Arquitectura

```
app/                 Interfaz React + TypeScript (Vite)
app/src-tauri/       Ventana Tauri 2 (Rust): puente fino con el supervisor
crates/merc-ipc/     Protocolo + canal local + lanzador del supervisor (compartido con `merc`)
crates/merc-supervisord/  Supervisor persistente (ya existente)
docs/contratos/ipc-v1-ejemplos.json  Contrato JSON generado desde Rust y comprobado en TypeScript
```

- La ventana **no controla procesos Java**: solo habla con el supervisor. Si la cierras, los servidores siguen funcionando.
- Cada llamada abre una conexión corta y autenticada. Así, una parada larga no bloquea la consola.
- Permisos de Tauri mínimos: diálogo para abrir carpetas y abrir **una única URL** (la de la EULA).
- Protocolo ampliado sin romper compatibilidad: `accept_eula`, y en la lista `eula_accepted` y `configured_port`.
- Diseño: petróleo, marfil y cobre (directiva §11). Tipografías Instrument Sans y JetBrains Mono (solo en la consola), incluidas en la app con licencia OFL. Lámpara de estado con relieve como único elemento llamativo; la animación respeta «reducir movimiento». Foco visible, diálogos con teclado (Escape y foco atrapado) y etiquetas accesibles.
- Icono original: «lámpara-bloque» de cobre sobre petróleo (`app/branding/icon-1024.png`).

## 3. Pruebas (resultados reales)

| Conjunto | Resultado |
|---|---|
| Rust (`cargo test --workspace`) | **69 pasan, 0 fallan**; clippy sin avisos |
| Interfaz (`vitest`) | **19 pasan, 0 fallan** |
| Tipos (`tsc --noEmit`, TypeScript 7) | Sin errores |
| Contrato Rust ↔ TypeScript | Ambos lados validan el mismo archivo de ejemplos |

Pruebas nuevas destacadas:
- Supervisor: la EULA solo se acepta con `accepted: true`, nunca con el servidor encendido, y la importación no crea `eula.txt`.
- Interfaz: importación con Java 21 recomendado (no 25); carpeta sin servidor; ningún Java compatible; los botones según el estado; la EULA con casilla obligatoria; forzar cierre con confirmación; huérfano sin consola; consola sin duplicados, con límite de memoria e historial; mensajes de error del supervisor; quitar de la lista.
- Un error de accesibilidad real que encontraron las pruebas: una etiqueta `<label>` reemplazaba el nombre del botón «Elegir carpeta» para los lectores de pantalla. Corregido.

**App real en pantalla virtual (Linux, Xvfb).** Se usaron el ejecutable compilado con `tauri build`, el supervisor real y procesos Java reales de prueba (FakeServer imita a Minecraft; no es Minecraft):

| Comprobación | Resultado |
|---|---|
| La app arranca, inicia sola el supervisor y muestra los dos servidores importados con su estado | PASS |
| Servidor en marcha: «En línea desde hace N s», PID y puerto reales, consola con el historial | PASS |
| Comando escrito en la app → lo recibe el servidor | PASS |
| «Detener» → `stop`, guardado y «Última salida: código 0, detenido por ti» | PASS |
| **Se cierra la app de golpe (kill -9)**: el servidor sigue EN LÍNEA; al reabrir la app aparece con su consola | PASS |
| EULA: sin marcar la casilla no se escribe nada; al marcar y aceptar se crea `eula.txt` y se habilita «Iniciar» | PASS |

Las capturas de este incremento se reemplazaron por las del rediseño (incremento 4).

## 4. Cómo abrirla en Windows

`scripts\abrir-app.ps1` (clic derecho → Ejecutar con PowerShell): instala las dependencias de la interfaz, compila el supervisor y la app en modo release, ofrece crear un acceso directo en el escritorio y abre MercHosting. Usa tu carpeta de datos normal (`%LOCALAPPDATA%\MercHosting`).

`scripts\verificar-windows.ps1` ahora también ejecuta las pruebas de la interfaz y la comprobación de tipos.

## 5. Límites conocidos

- No hay instalador todavía (Fase 14): la app se compila desde el proyecto con el script.
- El identificador de la app (`app.merchosting.desktop`) es provisional, hasta que haya un dominio o repositorio oficial. Se usará solo para datos internos de Tauri.
- La ventana pregunta cada poco tiempo al supervisor (lista cada 1 s y consola cada 0,4 s), sin notificaciones automáticas. Es suficiente para pocos servidores.
- Aún no se puede crear un servidor desde cero ni editar la memoria o el Java de un servidor ya importado. Están planeados (asistente de creación, Fase 3).
- No se mide la RAM total de la PC; el texto de ayuda lo explica en lugar de inventar un número.
