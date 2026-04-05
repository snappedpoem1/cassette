param(
    [ValidateSet("Sandbox", "DisposableProfile", "AppDataReset")]
    [string]$Mode = "DisposableProfile",
    [switch]$RunTrustSpine
)

$ErrorActionPreference = "Stop"

function Write-Step {
    param([string]$Message)
    Write-Host "[cleanroom] $Message"
}

$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

$bundleMsi = Join-Path $repoRoot "target\release\bundle\msi\Cassette_0.1.0_x64_en-US.msi"
$bundleExe = Join-Path $repoRoot "target\release\bundle\nsis\Cassette_0.1.0_x64-setup.exe"

$appDataRoot = Join-Path $env:APPDATA "dev.cassette.app"
$runtimeDb = Join-Path $appDataRoot "cassette.db"
$sidecarDb = Join-Path $appDataRoot "cassette_librarian.db"

Write-Step "Validation mode: $Mode"
Write-Step "This script validates post-install clean-room state. It does not uninstall/reinstall automatically."

$failures = New-Object System.Collections.Generic.List[string]

if (-not (Test-Path $bundleMsi) -and -not (Test-Path $bundleExe)) {
    $failures.Add("No installer bundle found. Expected MSI or NSIS bundle under target/release/bundle.")
} else {
    Write-Step "Installer bundle check passed."
}

if (-not (Test-Path $runtimeDb)) {
    $failures.Add("Missing runtime DB: $runtimeDb")
} else {
    Write-Step "Found runtime DB: $runtimeDb"
}

if (-not (Test-Path $sidecarDb)) {
    $failures.Add("Missing sidecar DB: $sidecarDb")
} else {
    Write-Step "Found sidecar DB: $sidecarDb"
}

if ($RunTrustSpine) {
    $trustSpineScript = Join-Path $PSScriptRoot "verify_trust_spine.ps1"
    if (-not (Test-Path $trustSpineScript)) {
        $failures.Add("Missing trust-spine script: $trustSpineScript")
    } else {
        Write-Step "Running trust-spine verification..."
        & $trustSpineScript
        if ($LASTEXITCODE -ne 0) {
            $failures.Add("verify_trust_spine.ps1 exited with code $LASTEXITCODE")
        }
    }
}

if ($Mode -eq "AppDataReset") {
    Write-Step "AppDataReset mode is lower-confidence proof; record this explicitly in release notes."
}

if ($failures.Count -gt 0) {
    Write-Host ""
    Write-Host "Clean-room verification failed:" -ForegroundColor Red
    foreach ($failure in $failures) {
        Write-Host " - $failure" -ForegroundColor Red
    }
    exit 1
}

Write-Host ""
Write-Host "Clean-room verification passed for mode '$Mode'." -ForegroundColor Green
exit 0
