# Compila DevLogica Tools per Windows e prepara la cartella "dist".
# Uso (PowerShell, nella cartella del progetto):  .\packaging\windows\build.ps1
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..\..")

cargo build --release
New-Item -ItemType Directory -Force -Path dist | Out-Null
Copy-Item target\release\devlogica-tools.exe "dist\DevLogica Tools.exe" -Force
Write-Host "Creato: dist\DevLogica Tools.exe"
