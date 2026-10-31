# Prueba de CREACIÓN de servidores con las fuentes oficiales REALES (herramienta de desarrollo).
# Uso: clic derecho > "Ejecutar con PowerShell".
#
# Qué hace: compila, consulta las listas oficiales de versiones (Mojang, PaperMC, Fabric y
# NeoForge), crea un servidor nuevo en una carpeta de PRUEBA dentro de %TEMP% (descarga
# verificada), lo enciende, te deja entrar, lo detiene con guardado y te pregunta si borrar la
# carpeta de prueba. Todo queda en prueba-creacion.log. No toca tus otros servidores.

Set-Location (Split-Path -Parent $PSScriptRoot)
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$log = Join-Path (Get-Location) "prueba-creacion.log"
Set-Content -Path $log -Value "MercHosting - prueba de creación $(Get-Date -Format o)" -Encoding UTF8

function Log($text) {
    Write-Host $text
    Add-Content -Path $log -Value $text -Encoding UTF8
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
# Windows bloquea los .exe en ejecución: un supervisor de una prueba anterior impediría recompilar.
function Close-OldSupervisors {
    $exe = Join-Path (Get-Location) "target\debug\merc-supervisord.exe"
    $old = Get-Process merc-supervisord -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe }
    if (-not $old) { return }
    Write-Host "Hay un supervisor de una prueba anterior abierto (PID $($old.Id -join ', '))." -ForegroundColor Yellow
    $ans = Read-Host "¿Cerrarlo para poder recompilar? (S/N)"
    if ($ans -match '^[sS]') {
        $old | Stop-Process -Force
        Start-Sleep -Milliseconds 500
        Log "Supervisor de prueba anterior cerrado."
    }
}

Title "1. Compilando MercHosting"
Close-OldSupervisors
cmd /c "cargo build -p merc-supervisord 2>&1" | ForEach-Object { Log $_ }
if ($LASTEXITCODE -ne 0) { Log "La compilación falló."; Finish 1 }
$merc = Join-Path (Get-Location) "target\debug\merc.exe"
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$data = Join-Path $env:TEMP ("MercHosting-prueba-" + $stamp)
$servers = Join-Path $env:TEMP ("MercHosting-servidores-prueba-" + $stamp)
# El supervisor que arranque `merc` hereda esta carpeta por defecto para servidores nuevos.
$env:MERC_SERVERS_DIR = $servers
Log "Datos de prueba: $data"
Log "Servidores de prueba: $servers"

function Merc {
    & $merc --data-dir $data @args 2>&1 | ForEach-Object { Log ("$_") }
    return $LASTEXITCODE
}

Title "2. Listas oficiales de versiones"
$latest = @{}
foreach ($tipo in "vanilla", "paper", "purpur", "spigot", "fabric", "quilt", "forge", "neoforge", "arclight") {
    Log "--- $tipo ---"
    $lines = & $merc --data-dir $data versiones $tipo 2>&1
    $lines | Select-Object -First 6 | ForEach-Object { Log ("$_") }
    if ($LASTEXITCODE -ne 0) { Log "No se pudo leer la lista de $tipo."; continue }
    $first = ($lines | Where-Object { "$_" -match "última" } | Select-Object -First 1)
    if ($first) { $latest[$tipo] = ("$first" -split '\s+')[0] }
}

Title "3. Java encontrado"
Merc java | Out-Null

Title "4. Qué servidor crear"
$tipo = Read-Host "Tipo (vanilla, paper, purpur, spigot, fabric, quilt, forge, neoforge, arclight) [Enter = paper]"
if (-not $tipo) { $tipo = "paper" }
$def = $latest[$tipo]
$ver = Read-Host "Versión de Minecraft [Enter = $def]"
if (-not $ver) { $ver = $def }
if (-not $ver) { Log "No hay versión elegida."; Finish 1 }
$ram = Read-Host "RAM en MB [Enter = 4096]"
if (-not $ram) { $ram = "4096" }

Write-Host ""
Write-Host "Para encender un servidor de Minecraft hay que aceptar la EULA de Minecraft:" -ForegroundColor Yellow
Write-Host "https://aka.ms/MinecraftEULA" -ForegroundColor Yellow
$eula = Read-Host "¿La has leído y la aceptas? (S/N)"
$eulaArgs = @()
if ($eula -match '^[sS]') { $eulaArgs = @("--acepto-eula"); Log "EULA aceptada por el usuario." } else { Log "EULA no aceptada: se crea pero no se enciende." }

Title "5. Creando $tipo $ver"
$t0 = Get-Date
$r = Merc crear $tipo $ver --nombre "PruebaCreacion" --ram $ram @eulaArgs
$secs = [int]((Get-Date) - $t0).TotalSeconds
if ($r -ne 0) { Log "La creación falló tras $secs s."; Merc apagar-supervisor | Out-Null; Finish 1 }
Log "Creado en $secs s."
Merc lista | Out-Null

if ($eulaArgs.Count -gt 0) {
    Title "6. Iniciando"
    if ((Merc iniciar PruebaCreacion) -ne 0) { Merc apagar-supervisor | Out-Null; Finish 1 }
    Log "Esperando a EN LÍNEA (el primer arranque genera el mundo y puede tardar)..."
    $r = Merc esperar PruebaCreacion ONLINE --max 900
    if ($r -ne 0) {
        Log "No llegó a EN LÍNEA. Últimas líneas:"
        Merc consola PruebaCreacion --ultimas 40 | Out-Null
        Merc forzar-cierre PruebaCreacion --confirmo | Out-Null
        Start-Sleep 3
        Merc apagar-supervisor | Out-Null
        Finish 1
    }
    Merc estado PruebaCreacion | Out-Null
    Title "7. Tu turno"
    Write-Host "Abre Minecraft $ver y entra a: localhost:PUERTO (el puerto sale arriba en 'Puerto')." -ForegroundColor Green
    $jugo = Read-Host "Cuando termines, escribe S si pudiste entrar o N si no"
    Log "¿Pudo entrar?: $jugo"
    Title "8. Deteniendo con guardado"
    Merc detener PruebaCreacion --espera 180 | Out-Null
    Merc consola PruebaCreacion --ultimas 6 | Out-Null
}

Title "9. Limpieza"
Merc quitar PruebaCreacion | Out-Null
Merc apagar-supervisor | Out-Null
$del = Read-Host "¿Borrar la carpeta de PRUEBA $servers ? (S/N)"
if ($del -match '^[sS]') {
    Remove-Item -Recurse -Force $servers
    Log "Carpeta de prueba borrada."
} else {
    Log "La carpeta de prueba se queda en: $servers"
}
Write-Host "PRUEBA TERMINADA" -ForegroundColor Green
Finish 0
