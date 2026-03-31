param(
    [string]$ManifestPath,
    [string]$LogRoot = "tmp\\cleanup_apply",
    [switch]$WhatIf
)

$ErrorActionPreference = "Stop"

if (-not $ManifestPath) {
    throw "ManifestPath is required."
}

if (-not (Test-Path -LiteralPath $ManifestPath)) {
    throw "Manifest not found: $ManifestPath"
}

$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$runDir = Join-Path $LogRoot $stamp
New-Item -ItemType Directory -Force -Path $runDir | Out-Null

$rows = Import-Csv -LiteralPath $ManifestPath
$eligible = $rows | Where-Object { $_.apply_eligible -eq "true" }
$actions = New-Object System.Collections.Generic.List[object]
$rollback = New-Object System.Collections.Generic.List[object]

function Add-Action {
    param(
        [string]$Action,
        [string]$Source,
        [string]$Target,
        [string]$Result,
        [string]$Reason
    )

    $actions.Add([pscustomobject]@{
        action = $Action
        source_path = $Source
        target_path = $Target
        result = $Result
        reason = $Reason
        timestamp = (Get-Date).ToString("o")
    }) | Out-Null
}

foreach ($row in $eligible) {
    $source = [string]$row.source_path
    $target = [string]$row.target_path
    $action = [string]$row.action
    $reason = [string]$row.reason

    if ([string]::IsNullOrWhiteSpace($target)) {
        Add-Action -Action $action -Source $source -Target $target -Result "invalid_target" -Reason $reason
        continue
    }

    if (-not (Test-Path -LiteralPath $source)) {
        Add-Action -Action $action -Source $source -Target $target -Result "missing_source" -Reason $reason
        continue
    }

    try {
        $sourceResolved = (Resolve-Path -LiteralPath $source).Path
    }
    catch {
        Add-Action -Action $action -Source $source -Target $target -Result "resolve_failed" -Reason $reason
        continue
    }

    try {
        if (Test-Path -LiteralPath $target) {
            $targetResolved = (Resolve-Path -LiteralPath $target).Path
            if ($sourceResolved -eq $targetResolved) {
                Add-Action -Action $action -Source $source -Target $target -Result "already_in_place" -Reason $reason
                continue
            }

            Add-Action -Action $action -Source $source -Target $target -Result "destination_exists" -Reason $reason
            continue
        }
    }
    catch {
        Add-Action -Action $action -Source $source -Target $target -Result "invalid_target" -Reason $reason
        continue
    }

    if ($WhatIf) {
        Add-Action -Action $action -Source $source -Target $target -Result "whatif" -Reason $reason
        continue
    }

    $targetParent = Split-Path -Parent $target
    if ($targetParent -and -not (Test-Path -LiteralPath $targetParent)) {
        New-Item -ItemType Directory -Force -Path $targetParent | Out-Null
    }

    try {
        Move-Item -LiteralPath $source -Destination $target -Force
        Add-Action -Action $action -Source $source -Target $target -Result "moved" -Reason $reason
        $rollback.Add([pscustomobject]@{
            source_path = $target
            target_path = $source
            action = "ROLLBACK_MOVE"
        }) | Out-Null
    }
    catch {
        Add-Action -Action $action -Source $source -Target $target -Result "move_failed" -Reason ($reason + " :: " + $_.Exception.Message)
    }
}

$actionsCsv = Join-Path $runDir "apply_actions.csv"
$rollbackCsv = Join-Path $runDir "rollback_manifest.csv"
$summaryJson = Join-Path $runDir "apply_summary.json"

$actions | Export-Csv -NoTypeInformation -Encoding UTF8 -LiteralPath $actionsCsv
$rollback | Export-Csv -NoTypeInformation -Encoding UTF8 -LiteralPath $rollbackCsv

$summary = [ordered]@{
    manifest_path = $ManifestPath
    total_eligible = @($eligible).Count
    moved = @($actions | Where-Object { $_.result -eq "moved" }).Count
    already_in_place = @($actions | Where-Object { $_.result -eq "already_in_place" }).Count
    destination_exists = @($actions | Where-Object { $_.result -eq "destination_exists" }).Count
    missing_source = @($actions | Where-Object { $_.result -eq "missing_source" }).Count
    move_failed = @($actions | Where-Object { $_.result -eq "move_failed" }).Count
    whatif = @($actions | Where-Object { $_.result -eq "whatif" }).Count
    actions_csv = $actionsCsv
    rollback_csv = $rollbackCsv
}
$summary | ConvertTo-Json -Depth 4 | Set-Content -Encoding UTF8 -LiteralPath $summaryJson

Write-Output "Cleanup apply phase complete."
Write-Output "Summary: $summaryJson"
Write-Output "Actions: $actionsCsv"
Write-Output "Rollback: $rollbackCsv"
