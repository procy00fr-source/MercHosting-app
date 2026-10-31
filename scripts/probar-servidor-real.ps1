# Prueba de MercHosting con un servidor de Minecraft REAL (herramienta de desarrollo).
# Uso: clic derecho > "Ejecutar con PowerShell".
#
# Qué hace: compila, pide la carpeta de una COPIA de tu servidor, la importa (sin modificar
# archivos), la inicia sin CMD, espera a que esté EN LÍNEA de verdad, te deja entrar a jugar,
# envía un comando, la detiene con guardado y deja todo registrado en prueba-servidor-real.log.
# Usa una carpeta de datos de prueba separada; no toca nada más de tu PC.

Set-Location (Split-Path -Parent $PSScriptRoot)
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$log = Join-Path (Get-Location) "prueba-servidor-real.log"
Set-Content -Path $log -Value "MercHosting - prueba con servidor real $(Get-Date -Format o)" -Encoding UTF8

function Log($text) {
    Write-Host $text
    Add-Content -Path $log -Value $text -Encoding UTF8
}
# Windows bloquea los .exe en ejecución: un supervisor de una prueba anterior impediría recompilar.
function Close-OldSupervisors {
    $exe = Join-Path (Get-Location) "target\debug\merc-supervisord.exe"
    $old = Get-Process merc-supervisord -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe }
    if (-not $old) { return }
    Write-Host "Hay un supervisor de una prueba anterior abierto (PID $($old.Id -join ', '))." -ForegroundColor Yellow
    Write-Host "Si dejaste un servidor de PRUEBA encendido con él, ese servidor seguirá funcionando." -ForegroundColor Yellow
    $ans = Read-Host "¿Cerrar ese supervisor de prueba para poder recompilar? (S/N)"
    if ($ans -match '^[sS]') {
        $old | Stop-Process -Force
        Start-Sleep -Milliseconds 500
        Log "Supervisor de prueba anterior cerrado."
    } else {
        Log "No se cerró el supervisor anterior; la compilación puede fallar con 'acceso denegado'."
    }
}

function Title($text) {
    Write-Host ""
    Write-Host "=== $text ===" -ForegroundColor Cyan
    Add-Content -Path $log -Value "`r`n=== $text ===" -Encoding UTF8
}
function Finish($code) {
    Log ""
    Log "Registro guardado en: $log"
    Read-Host "Pulsa Enter para salir"
    exit $code
}

Title "1. Compilando MercHosting"
Close-OldSupervisors
cmd /c "cargo build -p merc-supervisord 2>&1" | ForEach-Object { Log $_ }
if ($LASTEXITCODE -ne 0) { Log "La compilación falló."; Finish 1 }
$merc = Join-Path (Get-Location) "target\debug\merc.exe"
$data = Join-Path $env:TEMP ("MercHosting-prueba-" + (Get-Date -Format "yyyyMMdd-HHmmss"))
Log "Datos de prueba: $data"

function Merc {
    & $merc --data-dir $data @args 2>&1 | ForEach-Object { Log ("$_") }
    return $LASTEXITCODE
}

Title "2. Elige la carpeta del servidor"
Write-Host "IMPORTANTE: elige una COPIA de tu servidor, no el original." -ForegroundColor Yellow
Write-Host "Al jugar, el mundo cambia como en cualquier partida normal." -ForegroundColor Yellow
Add-Type -AssemblyName System.Windows.Forms
$dlg = New-Object System.Windows.Forms.FolderBrowserDialog
$dlg.Description = "Elige la carpeta de una COPIA de tu servidor de Minecraft"
if ($dlg.ShowDialog() -ne [System.Windows.Forms.DialogResult]::OK) { Log "Cancelado."; Finish 1 }
$server = $dlg.SelectedPath
Log "Carpeta: $server"

Title "3. Java encontrado"
Merc java | Out-Null

Title "4. Detección (sin modificar nada)"
if ((Merc detectar $server) -ne 0) { Finish 1 }
$ok = Read-Host "¿La detección es correcta? (S/N)"
if ($ok -notmatch '^[sS]') { Log "Detenido por el usuario tras la detección."; Merc apagar-supervisor | Out-Null; Finish 1 }

$ram = Read-Host "¿Cuánta RAM para el servidor en MB? (Enter = 4096)"
if (-not $ram) { $ram = "4096" }

Title "5. Importando"
if ((Merc importar $server --nombre "PruebaReal" --ram $ram) -ne 0) { Merc apagar-supervisor | Out-Null; Finish 1 }

Title "6. Iniciando (sin CMD)"
if ((Merc iniciar PruebaReal) -ne 0) { Merc apagar-supervisor | Out-Null; Finish 1 }
Log "Esperando a que el servidor confirme que está listo (los modpacks pueden tardar varios minutos)..."
$t0 = Get-Date
$r = Merc esperar PruebaReal ONLINE --max 900
$secs = [int]((Get-Date) - $t0).TotalSeconds
if ($r -ne 0) {
    Log "El servidor no llegó a EN LÍNEA ($secs s). Últimas líneas de la consola:"
    Merc consola PruebaReal --ultimas 40 | Out-Null
    Merc estado PruebaReal | Out-Null
    Merc forzar-cierre PruebaReal --confirmo | Out-Null
    Start-Sleep 3
    Merc apagar-supervisor | Out-Null
    Finish 1
}
Log "EN LÍNEA tras $secs s."
Merc estado PruebaReal | Out-Null
Merc consola PruebaReal --ultimas 12 | Out-Null

Title "7. Comando por la consola"
Merc comando PruebaReal say Hola desde MercHosting | Out-Null
Start-Sleep 2
Merc consola PruebaReal --ultimas 3 | Out-Null

Title "8. Tu turno"
Write-Host "Abre Minecraft y entra a: localhost  (si cambiaste el puerto: localhost:PUERTO)" -ForegroundColor Green
Write-Host "Esta ventana NO es la consola del servidor: el servidor lo mantiene el supervisor de MercHosting." -ForegroundColor Green
$jugo = Read-Host "Cuando termines, escribe S si pudiste entrar o N si no, y pulsa Enter"
Log "¿Pudo entrar al servidor?: $jugo"

Title "9. Deteniendo con guardado"
Merc detener PruebaReal --espera 180 | Out-Null
Merc consola PruebaReal --ultimas 8 | Out-Null
Merc estado PruebaReal | Out-Null

Title "10. Limpieza de la prueba"
Merc quitar PruebaReal | Out-Null
Merc apagar-supervisor | Out-Null
Log "Los archivos de tu servidor no se movieron ni se borraron. Datos de prueba en: $data"
Write-Host "PRUEBA TERMINADA" -ForegroundColor Green
Finish 0
