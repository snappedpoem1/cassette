param(
    [switch]$Strict,
    [switch]$RequireSlskd
)

$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent $PSScriptRoot
$slskdProbe = $null
$checks = [ordered]@{
    "Repo .env" = Test-Path (Join-Path $projectRoot ".env")
    "A:\Music exists" = Test-Path "A:\Music"
    "A:\Staging exists" = Test-Path "A:\Staging"
    "Bundled slskd.exe" = Test-Path (Join-Path $projectRoot "binaries\slskd\slskd.exe")
    "App DB exists" = Test-Path (Join-Path $env:APPDATA "dev.cassette.app\cassette.db")
}

try {
    $probeOutput = cargo run --quiet -p cassette --bin slskd_runtime_probe_cli -- --json 2>$null
    if ($LASTEXITCODE -eq 0 -and $probeOutput) {
        $slskdProbe = $probeOutput | ConvertFrom-Json
        $checks["Managed slskd runtime ready"] = [bool]$slskdProbe.probe_status.ready
    } else {
        $checks["Managed slskd runtime ready"] = $false
    }
} catch {
    $checks["Managed slskd runtime ready"] = $false
}

$checks.GetEnumerator() | ForEach-Object {
    [pscustomobject]@{
        Check = $_.Key
        Passed = $_.Value
    }
} | Format-Table -AutoSize

if ($slskdProbe) {
    Write-Host ""
    Write-Host "[smoke] managed slskd probe"
    [pscustomobject]@{
        Url = $slskdProbe.probe_status.url
        Ready = [bool]$slskdProbe.probe_status.ready
        SpawnedByProbe = [bool]$slskdProbe.probe_status.spawned_by_app
        StoppedAfterProbe = [bool]$slskdProbe.stopped_after_probe
        AppDir = $slskdProbe.probe_status.app_dir
        DownloadsDir = $slskdProbe.probe_status.downloads_dir
        Message = $slskdProbe.probe_status.message
    } | Format-List
}

if ($Strict) {
    $requiredChecks = @(
        "Repo .env",
        "A:\Music exists",
        "A:\Staging exists",
        "Bundled slskd.exe",
        "App DB exists"
    )
    if ($RequireSlskd) {
        $requiredChecks += "Managed slskd runtime ready"
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
