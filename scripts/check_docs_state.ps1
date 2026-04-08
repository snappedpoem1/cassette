param()

$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

function Read-Doc([string]$relativePath) {
    $path = Join-Path $repoRoot $relativePath
    if (-not (Test-Path $path)) {
        throw "Missing doc: $relativePath"
    }
    return Get-Content $path -Raw
}

$checks = @()

$decisions = Read-Doc "docs/DECISIONS.md"
$projectState = Read-Doc "docs/PROJECT_STATE.md"
$registry = Read-Doc "docs/TOOL_AND_SERVICE_REGISTRY.md"

$checks += [pscustomobject]@{
    Name = "Decision 36 no stale slskd socket-only wording"
    Passed = -not ($decisions -match "still checks port `5030` independently")
    Failure = "DECISIONS.md still claims smoke verification is a standalone port-5030 check."
}

$checks += [pscustomobject]@{
    Name = "Project state includes managed slskd runtime probe contract"
    Passed = ($projectState -match "slskd_runtime_probe_cli")
    Failure = "PROJECT_STATE.md is missing the managed slskd runtime probe contract reference."
}

$checks += [pscustomobject]@{
    Name = "Registry uses canonical status vocabulary"
    Passed = (
        ($registry -match "local-proven") -and
        ($registry -match "bounded-probe") -and
        ($registry -match "unverified") -and
        -not ($registry -match "Proven Working|Implemented but Unverified|Partially Wired|Stub/Placeholder|Legacy/Compatibility Only|Doc-Only Idea|Dead/Conflicting")
    )
    Failure = "TOOL_AND_SERVICE_REGISTRY.md status legend is not aligned to local-proven|bounded-probe|unverified."
}

$checks += [pscustomobject]@{
    Name = "Cover Art Archive row reflects runtime fallback wiring"
    Passed = (
        ($registry -match "Cover Art Archive") -and
        ($registry -match "Runtime metadata tagging fallback") -and
        -not ($registry -match "Mentioned only|Not currently wired")
    )
    Failure = "Cover Art Archive row does not match current wired fallback behavior."
}

$checks | Select-Object Name, Passed | Format-Table -AutoSize

$failed = $checks | Where-Object { -not $_.Passed }
if ($failed.Count -gt 0) {
    Write-Host ""
    Write-Host "[docs-check] failures:" -ForegroundColor Red
    foreach ($item in $failed) {
        Write-Host " - $($item.Name): $($item.Failure)" -ForegroundColor Red
    }
    exit 1
}

Write-Host ""
Write-Host "[docs-check] all checks passed" -ForegroundColor Green
