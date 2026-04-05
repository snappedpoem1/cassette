param(
    [switch]$Strict,
    [switch]$RequireSlskd
)

$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent $PSScriptRoot
$checks = [ordered]@{
    "Repo .env" = Test-Path (Join-Path $projectRoot ".env")
    "A:\Music exists" = Test-Path "A:\Music"
    "A:\Staging exists" = Test-Path "A:\Staging"
    "Bundled slskd.exe" = Test-Path (Join-Path $projectRoot "binaries\slskd\slskd.exe")
    "App DB exists" = Test-Path (Join-Path $env:APPDATA "dev.cassette.app\cassette.db")
}

try {
    $port = Test-NetConnection -ComputerName localhost -Port 5030 -WarningAction SilentlyContinue
    $checks["slskd localhost:5030"] = $port.TcpTestSucceeded
} catch {
    $checks["slskd localhost:5030"] = $false
}

$checks.GetEnumerator() | ForEach-Object {
    [pscustomobject]@{
        Check = $_.Key
        Passed = $_.Value
    }
} | Format-Table -AutoSize

if ($Strict) {
    $requiredChecks = @(
        "Repo .env",
        "A:\Music exists",
        "A:\Staging exists",
        "Bundled slskd.exe",
        "App DB exists"
    )
    if ($RequireSlskd) {
        $requiredChecks += "slskd localhost:5030"
    }

    $failed = @()
    foreach ($name in $requiredChecks) {
        if (-not $checks[$name]) {
            $failed += $name
        }
    }

    if ($failed.Count -gt 0) {
        Write-Host ""
        Write-Host "[smoke] required checks failed:" -ForegroundColor Red
        $failed | ForEach-Object { Write-Host " - $_" -ForegroundColor Red }
        exit 1
    }
}
