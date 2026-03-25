$ErrorActionPreference = "Stop"

$existing = Get-Process slskd -ErrorAction SilentlyContinue
if (-not $existing) {
    Write-Output "slskd is not running"
    exit 0
}

$existing | Stop-Process -Force
Write-Output "slskd stopped"
