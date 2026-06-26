# efact-hardware-agent installer — Windows (PowerShell)
# Usage:
#   iwr -useb https://raw.githubusercontent.com/nubitio/efact-hardware-agent/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

$REPO    = "nubitio/efact-hardware-agent"
$BINARY  = "efact-hardware-agent.exe"
$LEGACY  = "efact-printer-agent.exe"
$ASSET   = "efact-hardware-agent-windows-x86_64.zip"

$INSTALL_DIR = "$env:LOCALAPPDATA\efact-hardware-agent"
$CONFIG_DIR  = "$env:APPDATA\efact-hardware-agent"
$LEGACY_CONFIG = "$env:APPDATA\efact-printer-agent"

function Write-Info  { Write-Host "[efact-hardware-agent] $args" -ForegroundColor Cyan }
function Write-Ok    { Write-Host "[efact-hardware-agent] $args" -ForegroundColor Green }
function Write-Err   { Write-Host "[efact-hardware-agent] $args" -ForegroundColor Red; exit 1 }

Write-Info "Fetching latest release..."
$release = Invoke-RestMethod "https://api.github.com/repos/$REPO/releases/latest"
$TAG = $release.tag_name
if (-not $TAG) { Write-Err "Could not determine latest release tag." }
Write-Info "Latest release: $TAG"

$TMP = Join-Path $env:TEMP "efact-hardware-agent-install"
if (Test-Path $TMP) { Remove-Item $TMP -Recurse -Force }
New-Item -ItemType Directory -Path $TMP | Out-Null

$DOWNLOAD_URL = "https://github.com/$REPO/releases/download/$TAG/$ASSET"
Write-Info "Downloading $ASSET..."
Invoke-WebRequest -Uri $DOWNLOAD_URL -OutFile (Join-Path $TMP $ASSET)
Expand-Archive -Path (Join-Path $TMP $ASSET) -DestinationPath $TMP -Force

Get-Process "efact-hardware-agent","efact-printer-agent" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue

New-Item -ItemType Directory -Path $INSTALL_DIR -Force | Out-Null
Copy-Item (Join-Path $TMP $BINARY) (Join-Path $INSTALL_DIR $BINARY) -Force
Copy-Item (Join-Path $TMP $BINARY) (Join-Path $INSTALL_DIR $LEGACY) -Force

New-Item -ItemType Directory -Path $CONFIG_DIR -Force | Out-Null

if (-not (Test-Path (Join-Path $CONFIG_DIR "config.toml"))) {
    if (Test-Path (Join-Path $LEGACY_CONFIG "config.toml")) {
        Copy-Item (Join-Path $LEGACY_CONFIG "config.toml") (Join-Path $CONFIG_DIR "config.toml")
        Write-Info "Migrated config from $LEGACY_CONFIG"
    } else {
        Copy-Item (Join-Path $TMP "config.toml") (Join-Path $CONFIG_DIR "config.toml")
    }
}

$exampleSrc = Join-Path $TMP "config.toml.example"
if (Test-Path $exampleSrc) {
    Copy-Item $exampleSrc (Join-Path $CONFIG_DIR "config.toml.example") -Force
    Write-Ok "Reference config: $(Join-Path $CONFIG_DIR 'config.toml.example')"
}

$TASK_NAME = "efact-hardware-agent"
$action = New-ScheduledTaskAction -Execute (Join-Path $INSTALL_DIR $BINARY)
$trigger = New-ScheduledTaskTrigger -AtLogOn
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable
Register-ScheduledTask -TaskName $TASK_NAME -Action $action -Trigger $trigger -Settings $settings -Force | Out-Null
Unregister-ScheduledTask -TaskName "efact-printer-agent" -Confirm:$false -ErrorAction SilentlyContinue
Start-ScheduledTask -TaskName $TASK_NAME

Write-Ok "efact-hardware-agent $TAG installed successfully."
Write-Ok "Agent running on http://127.0.0.1:8765"
Write-Ok "Config: $(Join-Path $CONFIG_DIR 'config.toml')"