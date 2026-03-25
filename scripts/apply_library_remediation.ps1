param(
    [string]$AuditReportPath = "tmp/library_audit_report.json",
    [string]$DeadspotReportPath = "tmp/library_deadspot_report.json",
    [string]$QuarantineRoot = "A:\music_quarantine_manual",
    [switch]$WhatIf
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path $AuditReportPath)) {
    throw "Audit report not found: $AuditReportPath"
}
if (-not (Test-Path $DeadspotReportPath)) {
    throw "Deadspot report not found: $DeadspotReportPath"
}

$audit = Get-Content -Raw $AuditReportPath | ConvertFrom-Json
$dead = Get-Content -Raw $DeadspotReportPath | ConvertFrom-Json

$runStamp = Get-Date -Format "yyyyMMdd-HHmmss"
$quarantineBase = Join-Path $QuarantineRoot $runStamp
$logDir = Join-Path "tmp/remediation" $runStamp
New-Item -ItemType Directory -Force -Path $logDir | Out-Null
New-Item -ItemType Directory -Force -Path $quarantineBase | Out-Null

$hardStatuses = @('ZeroByte','DecodeFailed','UnreadableContainer','HtmlOrTextPayload','UnsupportedFormat')
$deadspotSet = @{}
foreach ($d in $dead.examples) {
    $deadspotSet[[string]$d.path] = $true
}

$hardSet = @{}
foreach ($invalid in $audit.invalid_examples) {
    $path = [string]$invalid.path
    $status = [string]$invalid.status
    if ($hardStatuses -contains $status) {
        $hardSet[$path] = $true
    }
}

$actions = New-Object System.Collections.Generic.List[object]

function Move-Safely {
    param(
        [string]$Source,
        [string]$Destination,
        [string]$Action,
        [string]$Reason
    )

    $record = [ordered]@{
        action = $Action
        source = $Source
        destination = $Destination
        reason = $Reason
        result = ""
        timestamp = (Get-Date).ToString("o")
    }

    if (-not (Test-Path -LiteralPath $Source)) {
        $record.result = "missing_source"
        $actions.Add([pscustomobject]$record)
        return
    }

    try {
        if (Test-Path -LiteralPath $Destination) {
            $record.result = "destination_exists"
            $actions.Add([pscustomobject]$record)
            return
        }
    }
    catch {
        $record.result = "invalid_destination_path"
        $actions.Add([pscustomobject]$record)
        return
    }

    if ($WhatIf) {
        $record.result = "whatif"
        $actions.Add([pscustomobject]$record)
        return
    }

    $parent = Split-Path -Parent $Destination
    if ($parent -and -not (Test-Path -LiteralPath $parent)) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }

    try {
        Move-Item -LiteralPath $Source -Destination $Destination -Force
        $record.result = "moved"
    }
    catch {
        $record.result = "move_failed: $($_.Exception.Message)"
    }
    $actions.Add([pscustomobject]$record)
}

# 1) Quarantine hard failures.
foreach ($item in $audit.invalid_examples) {
    $status = [string]$item.status
    if ($hardStatuses -contains $status) {
        $source = [string]$item.path
        $name = Split-Path -Leaf $source
        $hash = [Math]::Abs($source.GetHashCode())
        $dest = Join-Path $quarantineBase (Join-Path $status ("{0}_{1}" -f $hash, $name))
        Move-Safely -Source $source -Destination $dest -Action "quarantine_hard_failure" -Reason $status
    }
}

# 2) Quarantine deadspot/silent placeholder files.
foreach ($item in $dead.examples) {
    $source = [string]$item.path
    $name = Split-Path -Leaf $source
    $hash = [Math]::Abs($source.GetHashCode())
    $dest = Join-Path $quarantineBase (Join-Path "deadspots" ("{0}_{1}" -f $hash, $name))
    Move-Safely -Source $source -Destination $dest -Action "quarantine_deadspot" -Reason (([string[]]$item.reasons) -join "; ")
}

# 3) Move misplaced files to canonical destinations, skipping any now quarantined.
foreach ($item in $audit.misplaced_examples) {
    $source = [string]$item.source_path
    $dest = [string]$item.expected_path

    if ($deadspotSet.ContainsKey($source)) {
        continue
    }

    if ($hardSet.ContainsKey($source)) {
        continue
    }

    Move-Safely -Source $source -Destination $dest -Action "move_to_canonical" -Reason "misplaced"
}

$actionsCsv = Join-Path $logDir "remediation_actions.csv"
$summaryJson = Join-Path $logDir "remediation_summary.json"
$actions | Export-Csv -NoTypeInformation -Encoding UTF8 $actionsCsv

$summary = [ordered]@{
    run_stamp = $runStamp
    quarantine_root = $quarantineBase
    total_actions = $actions.Count
    moved = @($actions | Where-Object { $_.result -eq 'moved' }).Count
    missing_source = @($actions | Where-Object { $_.result -eq 'missing_source' }).Count
    destination_exists = @($actions | Where-Object { $_.result -eq 'destination_exists' }).Count
    failed = @($actions | Where-Object { $_.result -like 'move_failed*' }).Count
    whatif = @($actions | Where-Object { $_.result -eq 'whatif' }).Count
    actions_csv = $actionsCsv
}
$summary | ConvertTo-Json -Depth 5 | Set-Content -Encoding UTF8 $summaryJson

Write-Output "Remediation complete."
Write-Output "Summary: $summaryJson"
Write-Output "Actions: $actionsCsv"
