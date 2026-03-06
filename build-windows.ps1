# Noah Windows Build Script
# Usage (from project root):
#   powershell -File build-windows.ps1              # Full release build
#   powershell -File build-windows.ps1 -Check       # Compile check only (fast)
#   powershell -File build-windows.ps1 -SkipInstall # Skip pnpm install

param(
    [switch]$Check,
    [switch]$SkipInstall
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

# Ensure cargo/rustc are on PATH (rustup default location)
$cargobin = Join-Path $env:USERPROFILE ".cargo\bin"
if (Test-Path $cargobin) {
    $env:PATH = "$cargobin;$env:PATH"
}

# Ensure node is on PATH (nvm4w puts it in AppData\Roaming\nvm\<version>)
$nvmDir = Join-Path $env:APPDATA "nvm"
if (Test-Path $nvmDir) {
    $nodeVer = Get-ChildItem $nvmDir -Directory | Where-Object { $_.Name -match '^v\d' } |
        Sort-Object Name -Descending | Select-Object -First 1
    if ($nodeVer) {
        $env:PATH = "$($nodeVer.FullName);$env:PATH"
    }
}

# Ensure pnpm is on PATH
$pnpmHome = Join-Path $env:LOCALAPPDATA "pnpm"
if (Test-Path $pnpmHome) {
    $env:PATH = "$pnpmHome;$env:PATH"
}

# Signing key
$keyFile = Join-Path $env:USERPROFILE ".tauri\noah.key"
if (Test-Path $keyFile) {
    $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $keyFile -Raw
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "searchformeaning"
} else {
    Write-Host "WARNING: Signing key not found at $keyFile" -ForegroundColor Yellow
    Write-Host "  Build will fail unless TAURI_SIGNING_PRIVATE_KEY is already set."
}

# --- Check-only mode: just compile the Rust crate ---
if ($Check) {
    Write-Host "==> Compile-checking itman-desktop..."
    cargo check -p itman-desktop
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    Write-Host "==> Compile check passed." -ForegroundColor Green
    exit 0
}

# --- Full release build ---
Write-Host "==> Pulling latest..."
git pull

if (-not $SkipInstall) {
    Write-Host "==> Installing dependencies..."
    pnpm install --frozen-lockfile
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

Write-Host "==> Building..."
node scripts/release.mjs --build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# Print upload instructions
$conf = Get-Content apps\desktop\src-tauri\tauri.conf.json | ConvertFrom-Json
$v = $conf.version

Write-Host ""
Write-Host "==> Done! Artifacts in target\release\bundle\" -ForegroundColor Green
Write-Host ""
Write-Host "From Mac, upload with:"
Write-Host "  scp xulea@100.87.199.115:C:/Users/xulea/src/itman/target/release/bundle/nsis/Noah_${v}_x64-setup.exe /tmp/"
Write-Host "  scp xulea@100.87.199.115:C:/Users/xulea/src/itman/target/release/bundle/msi/Noah_${v}_x64_en-US.msi /tmp/"
Write-Host "  gh release upload v${v} /tmp/Noah_${v}_x64-setup.exe /tmp/Noah_${v}_x64_en-US.msi --clobber"
