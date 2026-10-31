# Publica una versión nueva de MercHosting (instalador firmado + actualización automática).
# Uso: clic derecho > "Ejecutar con PowerShell". Guía completa: docs/PUBLICAR_VERSION.md
Set-Location (Split-Path -Parent $PSScriptRoot)
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

$v = Read-Host "Versión nueva (por ejemplo 0.2.0)"
$secure = Read-Host "Contraseña de la clave de firma (Enter si no tiene)" -AsSecureString
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [Runtime.InteropServices.Marshal]::PtrToStringAuto(
    [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure))
$solo = Read-Host "¿Solo compilar sin subir a GitHub? (S/N)"
$nodeArgs = @("scripts/publicar-version.mjs", $v)
if ($solo -match '^[sS]') { $nodeArgs += "--sin-subir" }
node @nodeArgs
$code = $LASTEXITCODE
Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
Read-Host "Pulsa Enter para cerrar esta ventana"
exit $code
