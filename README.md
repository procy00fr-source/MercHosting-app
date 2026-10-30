# MercHosting

> «Tu primer servidor no debería necesitar una mensualidad».

MercHosting es una aplicación de escritorio para Windows que permite crear y administrar servidores de
Minecraft que se ejecutan **en tu propia computadora**. Es gratuita: todas las funciones esenciales son gratis.
La Supporter Edition opcional (US$8, pago único) solo añadirá extras cosméticos.

## Estado actual

**Fase 1, incremento 1: núcleo sin interfaz.** Todavía no hay una aplicación que se pueda instalar.
Ver [`docs/FASE_1_INCREMENTO_1.md`](docs/FASE_1_INCREMENTO_1.md) para saber qué está hecho, qué se probó y qué falta.

## Estructura

| Carpeta | Contenido |
|---|---|
| `crates/merc-core` | Dominio: estados reales, confinamiento de rutas, detección de servidores, perfiles de arranque, requisitos de Java |
| `crates/merc-store` | SQLite con migraciones versionadas: instancias y auditoría |
| `crates/merc-supervisor` | Supervisor de procesos Java: arranque, readiness, parada limpia, crash y logs rotativos |
| `docs/especificacion` | Paquete de directivas PRIME Fase 2 (fuente de verdad del producto) |
| `scripts` | Ayudas de verificación para desarrolladores |

Planeado: `crates/merc-supervisor-bin` (proceso persistente + named pipe), `app/` (Tauri 2) y `ui/` (React + TypeScript).

## Desarrollo

Requisitos: Rust estable (1.80+) y, para las pruebas del supervisor, un JDK 17+ con `java`, `javac` y `jar` en el PATH.

```
cargo test --workspace
```

En Windows: `scripts\verificar-windows.ps1` comprueba las herramientas y ejecuta las pruebas.

## Licencia

Pendiente de elegir por el autor (el proyecto será de código abierto).
