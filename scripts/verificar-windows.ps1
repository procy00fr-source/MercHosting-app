# Verificación de MercHosting en Windows (herramienta de desarrollo, no para usuarios finales).
# Uso: clic derecho > "Ejecutar con PowerShell", o desde la carpeta del proyecto:
#   powershell -ExecutionPolicy Bypass -File scripts\verificar-windows.ps1
#
# Nota: no se usa $ErrorActionPreference = "Stop" porque en Windows PowerShell 5.1 convierte
# cualquier línea que cargo escriba en stderr (por ejemplo "Compiling ...") en un error fatal.

Set-Location (Split-Path -Parent $PSScriptRoot)
$log = Join-Path (Get-Location) "verificacion-windows.log"

function Log($text) {
    Write-Host $text
    Add-Content -Path $log -Value $text -Encoding UTF8
}

function Need($cmd, $hint) {
    $c = Get-Command $cmd -ErrorAction SilentlyContinue
    if (-not $c) { Log "FALTA: $cmd. $hint"; return $false }
    Log ("OK: {0} -> {1}" -f $cmd, $c.Source)
    return $true
}

Set-Content -Path $log -Value "MercHosting - verificacion $(Get-Date -Format o)" -Encoding UTF8
Log ("Windows: " + [System.Environment]::OSVersion.VersionString)
Log ("PowerShell: " + $PSVersionTable.PSVersion.ToString())

$ok = $true
$ok = (Need "cargo" "Instala Rust desde https://rustup.rs") -and $ok
$ok = (Need "java" "Instala un JDK 21 (por ejemplo Eclipse Temurin)") -and $ok
$ok = (Need "javac" "Hace falta un JDK, no solo un JRE") -and $ok
$ok = (Need "jar" "Hace falta un JDK, no solo un JRE") -and $ok
if (-not $ok) {
    Log "Faltan herramientas: no se ejecutaron las pruebas."
    Read-Host "Pulsa Enter para salir"
    exit 1
}

# cmd /c une stdout y stderr antes de que PowerShell los vea, así no se interpretan como errores.
cmd /c "cargo --version 2>&1" | ForEach-Object { Log $_ }
cmd /c "java -version 2>&1" | ForEach-Object { Log $_ }

Log ""
Log "Ejecutando pruebas (la primera vez compila dependencias y puede tardar varios minutos)..."
cmd /c "cargo test --workspace 2>&1" | ForEach-Object { Log $_ }
$code = $LASTEXITCODE

Log ""
if ($code -eq 0) {
    Write-Host "TODAS LAS PRUEBAS PASARON" -ForegroundColor Green
    Add-Content -Path $log -Value "RESULTADO: TODAS LAS PRUEBAS PASARON" -Encoding UTF8
} else {
    Write-Host "HUBO FALLOS (codigo $code). Envia el archivo verificacion-windows.log" -ForegroundColor Red
    Add-Content -Path $log -Value "RESULTADO: HUBO FALLOS (codigo $code)" -Encoding UTF8
}
Read-Host "Pulsa Enter para salir"
exit $code
