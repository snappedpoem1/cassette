param(
    [int]$Runs = 3,
    [int]$WarmupRuns = 1,
    [string]$OutputPath = "",
    [switch]$SkipOrganizeScenario
)

$ErrorActionPreference = "Stop"

function Get-Median([double[]]$values) {
    if (-not $values -or $values.Count -eq 0) { return 0 }
    $sorted = $values | Sort-Object
    $count = $sorted.Count
    if ($count % 2 -eq 1) { return [double]$sorted[[int]($count / 2)] }
    $a = [double]$sorted[[int]($count / 2) - 1]
    $b = [double]$sorted[[int]($count / 2)]
    return (($a + $b) / 2.0)
}

function Get-P95([double[]]$values) {
    if (-not $values -or $values.Count -eq 0) { return 0 }
    $sorted = $values | Sort-Object
    $idx = [Math]::Ceiling(0.95 * $sorted.Count) - 1
    if ($idx -lt 0) { $idx = 0 }
    if ($idx -ge $sorted.Count) { $idx = $sorted.Count - 1 }
    return [double]$sorted[$idx]
}

function Invoke-TimedScenario {
    param(
        [string]$Name,
        [string]$Command,
        [string]$RepoRoot,
        [string]$AppDataRoot,
        [int]$Runs,
        [int]$WarmupRuns
    )

    Write-Host "[perf] scenario=$Name warmup=$WarmupRuns runs=$Runs"

    $wrapped = "Set-Location '$RepoRoot';`$env:APPDATA='$AppDataRoot'; $Command"

    for ($i = 0; $i -lt $WarmupRuns; $i++) {
        Write-Host "  warmup $($i + 1)/$WarmupRuns"
        & powershell -NoProfile -ExecutionPolicy Bypass -Command $wrapped | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw "Warmup failed for scenario '$Name' with exit code $LASTEXITCODE"
        }
    }

    $durations = @()
    for ($i = 0; $i -lt $Runs; $i++) {
        Write-Host "  run $($i + 1)/$Runs"
        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        & powershell -NoProfile -ExecutionPolicy Bypass -Command $wrapped | Out-Null
        $exitCode = $LASTEXITCODE
        $sw.Stop()
        if ($exitCode -ne 0) {
            throw "Scenario '$Name' failed with exit code $exitCode"
        }
        $durations += [Math]::Round($sw.Elapsed.TotalSeconds, 3)
    }

    return [pscustomobject]@{
        name = $Name
        command = $Command
        runs = $Runs
        warmup_runs = $WarmupRuns
        durations_seconds = $durations
        median_seconds = [Math]::Round((Get-Median $durations), 3)
        p95_seconds = [Math]::Round((Get-P95 $durations), 3)
        min_seconds = [Math]::Round((($durations | Measure-Object -Minimum).Minimum), 3)
        max_seconds = [Math]::Round((($durations | Measure-Object -Maximum).Maximum), 3)
    }
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $repoRoot

$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$runDir = Join-Path $repoRoot "artifacts\perf\run-$timestamp"
New-Item -ItemType Directory -Path $runDir -Force | Out-Null

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path $runDir "results.json"
}

$scratchAppData = Join-Path $runDir "appdata"
$scratchCassetteDir = Join-Path $scratchAppData "dev.cassette.app"
New-Item -ItemType Directory -Path $scratchCassetteDir -Force | Out-Null

$sourceAppData = Join-Path $env:APPDATA "dev.cassette.app"
if (Test-Path $sourceAppData) {
    foreach ($dbName in @("cassette.db", "cassette_librarian.db")) {
        $src = Join-Path $sourceAppData $dbName
        if (Test-Path $src) {
            Copy-Item $src (Join-Path $scratchCassetteDir $dbName) -Force
        }
    }
}

$scenarios = @(
    [pscustomobject]@{
        name = "scan_resume_queue_only"
        command = "cargo run -p cassette --bin engine_pipeline_cli -- --resume --limit 0 --skip-post-sync --skip-organize-subset --skip-fingerprint-backfill"
    },
    [pscustomobject]@{
        name = "validation_targeted_suite"
        command = "cargo test -p cassette-core validation::logging::tests:: -- --nocapture"
    },
    [pscustomobject]@{
        name = "bounded_coordinator_limit5"
        command = "cargo run -p cassette --bin engine_pipeline_cli -- --resume --limit 5 --skip-post-sync --skip-organize-subset --skip-fingerprint-backfill"
    }
)

if (-not $SkipOrganizeScenario) {
    $scenarios += [pscustomobject]@{
        name = "organize_dry_run"
        command = "cargo run -p cassette --bin organize_cli -- --dry-run"
    }
}

$results = @()
foreach ($scenario in $scenarios) {
    $results += Invoke-TimedScenario -Name $scenario.name -Command $scenario.command -RepoRoot $repoRoot -AppDataRoot $scratchAppData -Runs $Runs -WarmupRuns $WarmupRuns
}

$payload = [pscustomobject]@{
    version = 1
    captured_at_utc = (Get-Date).ToUniversalTime().ToString("o")
    machine = [pscustomobject]@{
        computer_name = $env:COMPUTERNAME
        processor_count = [Environment]::ProcessorCount
        os_version = [Environment]::OSVersion.VersionString
        powershell_version = $PSVersionTable.PSVersion.ToString()
    }
    run = [pscustomobject]@{
        runs = $Runs
        warmup_runs = $WarmupRuns
        repo_root = $repoRoot
        scratch_appdata = $scratchAppData
    }
    scenarios = $results
}

$payload | ConvertTo-Json -Depth 8 | Set-Content -Path $OutputPath -Encoding UTF8

Write-Host "[perf] wrote results: $OutputPath"
