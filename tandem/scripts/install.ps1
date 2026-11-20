# Tandem installer for Windows.
#
#   powershell -c "irm https://raw.githubusercontent.com/beagle-tools/beagle-tools/main/tandem/scripts/install.ps1 | iex"
#
# Downloads the latest release's NSIS installer and runs it silently.
# After the first install, Tandem updates itself in-app.
$ErrorActionPreference = "Stop"

$url = "https://github.com/beagle-tools/beagle-tools/releases/download/beagle-tandem-latest/Tandem-windows-x64-setup.exe"
$setup = Join-Path $env:TEMP "Tandem-setup.exe"

Write-Host "[tandem] downloading installer..." -ForegroundColor Cyan
Invoke-WebRequest -Uri $url -OutFile $setup

# Check the download against the release's published .sha256 before running it.
$sumFile = "$setup.sha256"
Invoke-WebRequest -Uri "$url.sha256" -OutFile $sumFile
$want = ((Get-Content -Raw $sumFile) -split '\s+')[0].ToLower()
Remove-Item $sumFile -ErrorAction SilentlyContinue
$got = (Get-FileHash -Algorithm SHA256 -Path $setup).Hash.ToLower()
if (-not $want -or $want -ne $got) {
  Remove-Item $setup -ErrorAction SilentlyContinue
  throw "[tandem] checksum mismatch for the installer (expected $want, got $got) - download discarded"
}
Write-Host "[tandem] checksum ok" -ForegroundColor Cyan

Write-Host "[tandem] running installer..." -ForegroundColor Cyan
Start-Process -FilePath $setup -ArgumentList "/S" -Wait

Remove-Item $setup -ErrorAction SilentlyContinue
Write-Host "[tandem] done - Tandem is installed. Future updates happen in-app." -ForegroundColor Green
