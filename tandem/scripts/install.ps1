# Tandem installer for Windows.
#
#   powershell -c "irm https://raw.githubusercontent.com/beagle-tools/beagle-tools/main/tandem/scripts/install.ps1 | iex"
#
# Downloads the latest release's NSIS installer and runs it silently.
# After the first install, Tandem updates itself in-app.
$ErrorActionPreference = "Stop"

$url = "https://github.com/beagle-tools/beagle-tools/releases/download/tandem-latest/Tandem-windows-x64-setup.exe"
$setup = Join-Path $env:TEMP "Tandem-setup.exe"

Write-Host "[tandem] downloading installer..." -ForegroundColor Cyan
Invoke-WebRequest -Uri $url -OutFile $setup

Write-Host "[tandem] running installer..." -ForegroundColor Cyan
Start-Process -FilePath $setup -ArgumentList "/S" -Wait

Remove-Item $setup -ErrorAction SilentlyContinue
Write-Host "[tandem] done - Tandem is installed. Future updates happen in-app." -ForegroundColor Green
