param(
    [string]$Root = "A:\\music",
    [string]$OutputRoot = "tmp\\cleanup_pipeline",
    [string]$QuarantineRoot,
    [switch]$ApplySafe,
    [switch]$WhatIf,
    [switch]$SkipDeadspotAudit,
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $Root)) {
    throw "Library root not found: $Root"
}

$Root = (Resolve-Path -LiteralPath $Root).Path
$repoRoot = (Resolve-Path -LiteralPath ".").Path

$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$runDir = Join-Path $OutputRoot $stamp
$manifestDir = Join-Path $runDir "manifest"
New-Item -ItemType Directory -Force -Path $manifestDir | Out-Null

if (-not $QuarantineRoot) {
    $rootParent = Split-Path -Parent $Root
    if (-not $rootParent) {
        $rootParent = "."
    }
    $QuarantineRoot = Join-Path $rootParent "_Cassette_Quarantine"
}

if (-not $SkipBuild) {
    Write-Output "Building cassette-core examples..."
    cargo build -p cassette-core --examples
}

$auditExe = Join-Path $repoRoot "target\\debug\\examples\\audit_library_placement_and_size.exe"
$deadspotExe = Join-Path $repoRoot "target\\debug\\examples\\audit_audio_deadspots.exe"
$manifestExe = Join-Path $repoRoot "target\\debug\\examples\\build_library_cleanup_manifest.exe"

foreach ($exe in @($auditExe, $manifestExe)) {
    if (-not (Test-Path -LiteralPath $exe)) {
        throw "Expected executable not found: $exe"
    }
}

Write-Output "Running placement/validation audit..."
$auditJob = Start-Job -ScriptBlock {
    param($Exe, $LibraryRoot)
    & $Exe $LibraryRoot
    [pscustomobject]@{ exit_code = $LASTEXITCODE }
} -ArgumentList $auditExe, $Root

$deadspotJob = $null
if (-not $SkipDeadspotAudit) {
    if (-not (Test-Path -LiteralPath $deadspotExe)) {
        throw "Expected executable not found: $deadspotExe"
    }
    Write-Output "Running deadspot audit in parallel..."
    $deadspotJob = Start-Job -ScriptBlock {
        param($Exe, $LibraryRoot)
        & $Exe $LibraryRoot
        [pscustomobject]@{ exit_code = $LASTEXITCODE }
    } -ArgumentList $deadspotExe, $Root
}

$auditOutput = Receive-Job -Job $auditJob -Wait -AutoRemoveJob
$auditExit = $auditOutput | Where-Object { $_ -is [pscustomobject] -and $_.PSObject.Properties.Name -contains 'exit_code' } | Select-Object -Last 1
$auditOutput | Where-Object { -not ($_ -is [pscustomobject] -and $_.PSObject.Properties.Name -contains 'exit_code') } | ForEach-Object { Write-Output $_ }
if (-not $auditExit -or $auditExit.exit_code -ne 0) {
    throw "Placement audit failed with exit code $($auditExit.exit_code)"
}

if ($deadspotJob) {
    $deadspotOutput = Receive-Job -Job $deadspotJob -Wait -AutoRemoveJob
    $deadspotExit = $deadspotOutput | Where-Object { $_ -is [pscustomobject] -and $_.PSObject.Properties.Name -contains 'exit_code' } | Select-Object -Last 1
    $deadspotOutput | Where-Object { -not ($_ -is [pscustomobject] -and $_.PSObject.Properties.Name -contains 'exit_code') } | ForEach-Object { Write-Output $_ }
    if (-not $deadspotExit -or $deadspotExit.exit_code -ne 0) {
        throw "Deadspot audit failed with exit code $($deadspotExit.exit_code)"
    }
}

Write-Output "Building grouped cleanup manifest..."
& $manifestExe $Root $manifestDir $QuarantineRoot
if ($LASTEXITCODE -ne 0) {
    throw "Manifest builder failed with exit code $LASTEXITCODE"
}

$auditReport = "tmp\\library_audit_report.json"
$deadspotReport = "tmp\\library_deadspot_report.json"
if (Test-Path -LiteralPath $auditReport) {
    Copy-Item -LiteralPath $auditReport -Destination (Join-Path $runDir "library_audit_report.json") -Force
}
if (Test-Path -LiteralPath $deadspotReport) {
    Copy-Item -LiteralPath $deadspotReport -Destination (Join-Path $runDir "library_deadspot_report.json") -Force
}

$manifestPath = Join-Path $manifestDir "manifest_rows.csv"

if ($ApplySafe) {
    Write-Output "Applying safe rows from manifest..."
    $applyArgs = @{
        ManifestPath = $manifestPath
    }
    if ($WhatIf) {
        $applyArgs.WhatIf = $true
    }
    & (Join-Path $repoRoot "scripts\\apply_cleanup_manifest.ps1") @applyArgs
}
else {
    Write-Output "Preview complete. Review:"
    Write-Output "  $(Join-Path $manifestDir 'manifest_debrief.txt')"
    Write-Output "  $(Join-Path $manifestDir 'group_plan.csv')"
    Write-Output "  $(Join-Path $manifestDir 'manifest_rows.csv')"
    Write-Output "Then re-run with -ApplySafe to move only apply-eligible rows."
}
