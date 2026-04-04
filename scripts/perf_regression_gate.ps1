param(
    [Parameter(Mandatory = $true)]
    [string]$CandidateResultPath,
    [string]$BaselinePath = "docs/perf/BASELINE.latest.json",
    [string]$BudgetPath = "docs/perf/BUDGETS.json"
)

$ErrorActionPreference = "Stop"

function Get-DeltaPct([double]$baseline, [double]$candidate) {
    if ($baseline -le 0) { return 0 }
    return (($candidate - $baseline) / $baseline) * 100.0
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $repoRoot

if (-not (Test-Path $CandidateResultPath)) {
    throw "Candidate result not found: $CandidateResultPath"
}
if (-not (Test-Path $BaselinePath)) {
    throw "Baseline not found: $BaselinePath"
}
if (-not (Test-Path $BudgetPath)) {
    throw "Budget file not found: $BudgetPath"
}

$candidate = Get-Content $CandidateResultPath -Raw | ConvertFrom-Json
$baseline = Get-Content $BaselinePath -Raw | ConvertFrom-Json
$budget = Get-Content $BudgetPath -Raw | ConvertFrom-Json

$baselineByName = @{}
foreach ($s in $baseline.scenarios) { $baselineByName[$s.name] = $s }

$rows = @()
$hasFail = $false

foreach ($s in $candidate.scenarios) {
    if (-not $baselineByName.ContainsKey($s.name)) {
        $rows += [pscustomobject]@{
            scenario = $s.name
            status = "warn"
            detail = "missing-baseline"
            median_delta_pct = 0
            p95_delta_pct = 0
        }
        continue
    }

    if (-not $budget.scenarios.PSObject.Properties.Name.Contains($s.name)) {
        $rows += [pscustomobject]@{
            scenario = $s.name
            status = "warn"
            detail = "missing-budget"
            median_delta_pct = 0
            p95_delta_pct = 0
        }
        continue
    }

    $b = $baselineByName[$s.name]
    $limits = $budget.scenarios.$($s.name)

    $medianDelta = [Math]::Round((Get-DeltaPct ([double]$b.median_seconds) ([double]$s.median_seconds)), 2)
    $p95Delta = [Math]::Round((Get-DeltaPct ([double]$b.p95_seconds) ([double]$s.p95_seconds)), 2)

    $status = "pass"
    $detail = "within-budget"

    if ($medianDelta -gt [double]$limits.failMedianPct -or $p95Delta -gt [double]$limits.failP95Pct) {
        $status = "fail"
        $detail = "exceeds-fail-threshold"
        $hasFail = $true
    } elseif ($medianDelta -gt [double]$limits.warnMedianPct -or $p95Delta -gt [double]$limits.warnP95Pct) {
        $status = "warn"
        $detail = "exceeds-warn-threshold"
    }

    $rows += [pscustomobject]@{
        scenario = $s.name
        status = $status
        detail = $detail
        median_delta_pct = $medianDelta
        p95_delta_pct = $p95Delta
    }
}

$rows | Format-Table -AutoSize | Out-String | Write-Host

if ($hasFail) {
    Write-Error "Performance regression gate failed."
    exit 1
}

Write-Host "Performance regression gate passed (no fail-level regressions)."
