# Verificación de MercHosting en Windows (herramienta de desarrollo, no para usuarios finales).
# Uso: clic derecho > "Ejecutar con PowerShell", o desde la carpeta del proyecto:
#   powershell -ExecutionPolicy Bypass -File scripts\verificar-windows.ps1

$ErrorActionPreference = "Stop"
Set-Location (Split-Path -Parent $PSScriptRoot)

function Need($cmd, $hint) {
    if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) {
        Write-Host "FALTA: $cmd. $hint" -ForegroundColor Red
        return $false
    }
    Write-Host ("OK: {0} -> {1}" -f $cmd, (Get-Command $cmd).Source) -ForegroundColor Green
    return $true
}

$ok = $true
$ok = (Need "cargo" "Instala Rust desde https://rustup.rs") -and $ok
$ok = (Need "java" "Instala un JDK 21 (por ejemplo Eclipse Temurin)") -and $ok
$ok = (Need "javac" "Hace falta un JDK, no solo un JRE") -and $ok
$ok = (Need "jar" "Hace falta un JDK, no solo un JRE") -and $ok
if (-not $ok) { Read-Host "Faltan herramientas. Pulsa Enter para salir"; exit 1 }

$log = "verificacion-windows.log"
"MercHosting - verificacion $(Get-Date -Format o)" | Out-File $log -Encoding utf8
(cargo --version) | Out-File $log -Append -Encoding utf8
(cmd /c "java -version 2>&1") | Out-File $log -Append -Encoding utf8

Write-Host "Ejecutando pruebas (la primera vez compila dependencias y tarda)..."
cargo test --workspace 2>&1 | Tee-Object -FilePath $log -Append
$code = $LASTEXITCODE
if ($code -eq 0) { Write-Host "TODAS LAS PRUEBAS PASARON" -ForegroundColor Green }
else { Write-Host "HUBO FALLOS. Envia el archivo $log" -ForegroundColor Red }
Read-Host "Pulsa Enter para salir"
exit $code
