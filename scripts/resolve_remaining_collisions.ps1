param(
    [string]$AuditReportPath = "tmp/library_audit_report.json",
    [string]$QuarantineRoot = "A:\music_quarantine_manual",
    [switch]$WhatIf
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $AuditReportPath)) {
    throw "Audit report not found: $AuditReportPath"
}

$audit = Get-Content -Raw $AuditReportPath | ConvertFrom-Json
$run = (Get-Date -Format 'yyyyMMdd-HHmmss') + '-collision-resolve'
$logDir = Join-Path 'tmp/remediation' $run
$quarantineBase = Join-Path $QuarantineRoot $run
New-Item -ItemType Directory -Force -Path $logDir | Out-Null
New-Item -ItemType Directory -Force -Path $quarantineBase | Out-Null

$hardStatuses = @('ZeroByte','DecodeFailed','UnreadableContainer','HtmlOrTextPayload','UnsupportedFormat')
$actions = New-Object System.Collections.Generic.List[object]

function Add-Action {
    param([string]$Action,[string]$Source,[string]$Destination,[string]$Reason,[string]$Result)
    $actions.Add([pscustomobject]@{
        action = $Action
        source = $Source
        destination = $Destination
        reason = $Reason
        result = $Result
        timestamp = (Get-Date).ToString('o')
    })
}

function Sanitize-Component {
    param([string]$Name)
    if ([string]::IsNullOrWhiteSpace($Name)) { return '_' }
    $safe = $Name -replace '[<>:"/\\|?*]', '_'
    $safe = $safe.Trim().TrimEnd('.')
    if ([string]::IsNullOrWhiteSpace($safe)) { return '_' }
    return $safe
}

function Make-SafePath {
    param([string]$Path)
    $qualifier = Split-Path -Path $Path -Qualifier
    $relative = $Path.Substring($qualifier.Length).TrimStart('\')
    $parts = $relative -split '\\'
    $safeParts = @()
    foreach ($part in $parts) {
        $safeParts += (Sanitize-Component $part)
    }
    return (Join-Path $qualifier ($safeParts -join '\'))
}

function Next-AltPath {
    param([string]$Path)
    $dir = Split-Path -Parent $Path
    $name = [System.IO.Path]::GetFileNameWithoutExtension($Path)
    $ext = [System.IO.Path]::GetExtension($Path)
    $i = 1
    while ($true) {
        $candidate = Join-Path $dir ("{0} (alt{1}){2}" -f $name, $i, $ext)
        if (-not (Test-Path -LiteralPath $candidate)) {
            return $candidate
        }
        $i++
        if ($i -gt 9999) {
            throw "Could not find available alt path for $Path"
        }
    }
}

function Move-Safe {
    param([string]$Source,[string]$Destination,[string]$Action,[string]$Reason)

    if (-not (Test-Path -LiteralPath $Source)) {
        Add-Action $Action $Source $Destination $Reason 'missing_source'
        return $false
    }

    if ($WhatIf) {
        Add-Action $Action $Source $Destination $Reason 'whatif'
        return $true
    }

    $parent = Split-Path -Parent $Destination
    if ($parent -and -not (Test-Path -LiteralPath $parent)) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }

    try {
        Move-Item -LiteralPath $Source -Destination $Destination -Force
        Add-Action $Action $Source $Destination $Reason 'moved'
        return $true
    }
    catch {
        Add-Action $Action $Source $Destination $Reason ("move_failed: " + $_.Exception.Message)
        return $false
    }
}

# 1) Quarantine remaining hard + suspicious files first.
foreach ($item in $audit.invalid_examples) {
    $status = [string]$item.status
    if ($hardStatuses -contains $status) {
        $source = [string]$item.path
        $leaf = Split-Path -Leaf $source
        $hash = [Math]::Abs($source.GetHashCode())
        $dest = Join-Path $quarantineBase (Join-Path $status ("{0}_{1}" -f $hash, (Sanitize-Component $leaf)))
        [void](Move-Safe $source $dest 'quarantine_hard_failure' $status)
    }
}

foreach ($item in $audit.suspicious_size_examples) {
    $source = [string]$item.path
    $leaf = Split-Path -Leaf $source
    $hash = [Math]::Abs($source.GetHashCode())
    $dest = Join-Path $quarantineBase (Join-Path 'suspicious_size' ("{0}_{1}" -f $hash, (Sanitize-Component $leaf)))
    [void](Move-Safe $source $dest 'quarantine_suspicious_size' (([string[]]$item.reasons) -join '; '))
}

# 2) Resolve remaining misplaced files with collision-aware behavior.
$idx = 0
$total = @($audit.misplaced_examples).Count
foreach ($item in $audit.misplaced_examples) {
    $idx++
    if ($idx % 250 -eq 0) {
        Write-Output ("Processed {0}/{1} misplaced entries" -f $idx, $total)
    }

    $source = [string]$item.source_path
    $destRaw = [string]$item.expected_path

    if (-not (Test-Path -LiteralPath $source)) {
        Add-Action 'move_to_canonical' $source $destRaw 'misplaced' 'missing_source'
        continue
    }

    $dest = $destRaw
    try {
        # Validate by touching path APIs; if path explodes, sanitize it.
        [void](Split-Path -Path $dest -Parent)
    }
    catch {
        $dest = Make-SafePath $destRaw
    }

    if ($WhatIf) {
        Add-Action 'move_to_canonical' $source $dest 'misplaced' 'whatif'
        continue
    }

    $destExists = $false
    try {
        $destExists = Test-Path -LiteralPath $dest
    }
    catch {
        $dest = Make-SafePath $dest
        try {
            $destExists = Test-Path -LiteralPath $dest
        }
        catch {
            Add-Action 'move_to_canonical' $source $destRaw 'misplaced' 'invalid_destination_path'
            continue
        }
    }

    if ($destExists) {
        # Destination exists: compare hashes.
        try {
            $srcHash = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
            $dstHash = (Get-FileHash -LiteralPath $dest -Algorithm SHA256).Hash
            if ($srcHash -eq $dstHash) {
                $leaf = Split-Path -Leaf $source
                $hash = [Math]::Abs($source.GetHashCode())
                $dupDest = Join-Path $quarantineBase (Join-Path 'duplicates' ("{0}_{1}" -f $hash, (Sanitize-Component $leaf)))
                [void](Move-Safe $source $dupDest 'quarantine_duplicate' 'same_hash_as_destination')
                continue
            }
            else {
                $alt = Next-AltPath $dest
                [void](Move-Safe $source $alt 'move_to_canonical_alt' 'destination_exists_different_hash')
                continue
            }
        }
        catch {
            $alt = Next-AltPath $dest
            [void](Move-Safe $source $alt 'move_to_canonical_alt' 'hash_compare_failed')
            continue
        }
    }

    [void](Move-Safe $source $dest 'move_to_canonical' 'misplaced')
}

$csv = Join-Path $logDir 'collision_resolve_actions.csv'
$summaryPath = Join-Path $logDir 'collision_resolve_summary.json'
$actions | Export-Csv -NoTypeInformation -Encoding UTF8 $csv

$summary = [ordered]@{
    run_stamp = $run
    quarantine_root = $quarantineBase
    total_actions = $actions.Count
    moved = @($actions | Where-Object { $_.result -eq 'moved' }).Count
    missing_source = @($actions | Where-Object { $_.result -eq 'missing_source' }).Count
    failed = @($actions | Where-Object { $_.result -like 'move_failed*' }).Count
    whatif = @($actions | Where-Object { $_.result -eq 'whatif' }).Count
    actions_csv = $csv
}
$summary | ConvertTo-Json -Depth 5 | Set-Content -Encoding UTF8 $summaryPath

Write-Output "Collision-aware remediation complete."
Write-Output "Summary: $summaryPath"
Write-Output "Actions: $csv"
