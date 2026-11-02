# Instalar MercHosting

## Requisitos

- Windows 10 u 11 de 64 bits. En Linux, mira [LINUX.md](LINUX.md) (app de escritorio, todavía sin probar en una distribución real).
- Memoria: el servidor usa la que le asignes (2–4 GB para pocos amigos en Vanilla o Paper; 6 GB o más para modpacks grandes). Deja suficiente para el sistema y, si juegas en la misma PC, para Minecraft.
- Espacio en disco para los servidores y sus copias de seguridad.
- La PC tiene que estar encendida para que tus amigos jueguen: el servidor corre en ella.
- Java: si falta, MercHosting lo instala desde Eclipse Temurin (gratis y oficial).

## Descargar

Descárgalo solo de la web oficial de MercHosting o de las publicaciones de GitHub del proyecto (`procy00fr-source/MercHosting-releases`). Cada versión publica `SHA256SUMS.txt`; puedes comprobar el instalador en PowerShell:

```
Get-FileHash .\MercHosting-setup.exe -Algorithm SHA256
```

El resultado tiene que coincidir con la línea de `SHA256SUMS.txt`.

## Instalar (Windows)

En Linux, los pasos están en [LINUX.md](LINUX.md).

1. Abre `MercHosting-setup.exe`. Se instala solo para tu usuario y no pide permisos de administrador.
2. **SmartScreen:** el instalador todavía no lleva firma de código de Windows, así que la primera vez Windows puede mostrar «Windows protegió tu PC». Es el aviso para programas nuevos que aún no tienen reputación. Si lo descargaste del sitio oficial y la huella coincide, pulsa **Más información → Ejecutar de todas formas**. No desactives Windows Defender ni SmartScreen.
3. Al crear el primer servidor, Windows puede preguntar si permites a Java en la red. Para que entren amigos desde tu casa, permite las **redes privadas**. Para amigos de fuera, usa Playit desde la pestaña Conexión, que no necesita abrir puertos.

## Primer servidor

**Crear servidor** → elige el tipo (Vanilla, Paper, Fabric, NeoForge…) y la versión → acepta la EULA de Minecraft si estás de acuerdo (MercHosting nunca la acepta por ti) → **Iniciar**. La bienvenida y **Ayuda y tutoriales** te guían paso a paso.

## Actualizar

MercHosting avisa cuando hay una versión nueva (Ayuda y tutoriales → Actualizaciones). Antes de instalar, detiene tus servidores guardando el mundo y solo instala versiones firmadas por el autor.

## Desinstalar

En Linux: `sudo apt remove merc-hosting` (paquete .deb) o borra el AppImage; las carpetas que se conservan están en [LINUX.md](LINUX.md).

En Windows: **Configuración de Windows → Aplicaciones → MercHosting → Desinstalar.** Tus servidores, mundos y copias **no se borran**:

- servidores: `%USERPROFILE%\MercHosting\Servidores` (o donde los creaste);
- datos de la app y copias de seguridad: `%LOCALAPPDATA%\MercHosting`.

Bórralos a mano solo si ya no los quieres.

## Problemas comunes

- **El servidor no arranca:** abre **Diagnóstico**. Revisa mods, Java y los registros, y te dice qué hacer.
- **Mis amigos no pueden entrar:** pestaña **Conexión** → «Probar conexión».
- **Pocos recursos:** pestaña **Salud** (memoria, CPU, TPS y disco).
