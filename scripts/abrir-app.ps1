# Compila y abre la aplicación de escritorio de MercHosting (versión de desarrollo).
# Uso: clic derecho > "Ejecutar con PowerShell".
#
# Usa tu carpeta de datos normal (%LOCALAPPDATA%\MercHosting): los servidores que importes
# aquí son los de verdad. No se instala nada en el sistema; todo queda en la carpeta del proyecto.

Set-Location (Split-Path -Parent $PSScriptRoot)
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$log = Join-Path (Get-Location) "abrir-app.log"
Set-Content -Path $log -Value "MercHosting - abrir app $(Get-Date -Format o)" -Encoding UTF8

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
    Read-Host "Pulsa Enter para cerrar esta ventana"
    exit $code
}
function Run($cmdline) {
    cmd /c "$cmdline 2>&1" | ForEach-Object { Log $_ }
    return $LASTEXITCODE
}

# Windows bloquea los .exe en ejecución: hay que cerrar la app y el supervisor de esta carpeta
# antes de recompilarlos. Si hay servidores encendidos, se pide detenerlos primero.
function Close-Running {
    $root = (Get-Location).Path
    $procs = Get-Process merc-supervisord, merchosting-app -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -and $_.Path.StartsWith((Join-Path $root "target"), [System.StringComparison]::OrdinalIgnoreCase) }
    if (-not $procs) { return }
    $sup = $procs | Where-Object { $_.ProcessName -eq "merc-supervisord" }
    Write-Host "MercHosting ya está abierto desde esta carpeta. Para actualizarlo hay que cerrarlo." -ForegroundColor Yellow
    if ($sup) {
        Write-Host "Si tienes servidores ENCENDIDOS, deténlos primero desde la app (botón Detener)." -ForegroundColor Yellow
        Write-Host "Si cierras el supervisor con servidores encendidos, seguirán funcionando, pero sin consola." -ForegroundColor Yellow
    }
    $ans = Read-Host "¿Cerrar MercHosting ahora para actualizarlo? (S/N)"
    if ($ans -notmatch '^[sS]') { Log "No se cerró; no se puede recompilar."; Finish 1 }
    $procs | Stop-Process -Force
    Start-Sleep -Milliseconds 800
    Log "MercHosting cerrado para actualizar."
}

Title "1. Comprobando herramientas"
foreach ($t in @("cargo", "node", "npm")) {
    $c = Get-Command $t -ErrorAction SilentlyContinue
    if (-not $c) { Log "FALTA: $t"; Finish 1 }
    Log ("OK: {0} -> {1}" -f $t, $c.Source)
}
Run "node --version" | Out-Null
# El canal de compañeros (iroh) necesita Rust 1.91 o más nuevo.
$rv = (cmd /c "rustc --version" 2>$null)
if ($rv -match 'rustc 1\.(\d+)') {
    if ([int]$Matches[1] -lt 91) {
        Log "Tu Rust ($rv) es antiguo; se actualiza con rustup (solo la primera vez)."
        if ((Run "rustup update stable") -ne 0) { Log "No se pudo actualizar Rust. Ejecuta 'rustup update' y vuelve a intentarlo."; Finish 1 }
    }
}
Close-Running

Title "2. Instalando dependencias de la interfaz (la primera vez tarda)"
if ((Run "npm --prefix app ci --no-audit --no-fund") -ne 0) { Log "Falló npm ci."; Finish 1 }

Title "3. Compilando el supervisor"
if ((Run "cargo build --release -p merc-supervisord") -ne 0) { Log "Falló la compilación del supervisor."; Finish 1 }

Title "4. Compilando la aplicación (la primera vez puede tardar varios minutos)"
Push-Location app
$r = Run "npx tauri build --no-bundle"
Pop-Location
if ($r -ne 0) { Log "Falló la compilación de la aplicación."; Finish 1 }

$exe = Join-Path (Get-Location) "target\release\merchosting-app.exe"
if (-not (Test-Path $exe)) { Log "No se encontró $exe"; Finish 1 }
Log "Aplicación: $exe"

Title "5. Acceso directo (opcional)"
$lnk = Join-Path ([Environment]::GetFolderPath("Desktop")) "MercHosting (desarrollo).lnk"
if (-not (Test-Path $lnk)) {
    $ans = Read-Host "¿Crear un acceso directo en el escritorio? (S/N)"
    if ($ans -match '^[sS]') {
        $sh = New-Object -ComObject WScript.Shell
        $s = $sh.CreateShortcut($lnk)
        $s.TargetPath = $exe
        $s.WorkingDirectory = Split-Path $exe
        $s.Description = "MercHosting (versión de desarrollo)"
        $s.Save()
        Log "Acceso directo creado: $lnk"
    }
} else {
    Log "El acceso directo ya existe: $lnk"
}

Title "6. Abriendo MercHosting"
Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe)
Log "Listo. Puedes cerrar esta ventana; la aplicación sigue abierta."
Write-Host "LISTO" -ForegroundColor Green
Finish 0
