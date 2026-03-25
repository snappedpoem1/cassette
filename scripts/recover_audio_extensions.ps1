param(
    [string]$Root = "A:\music",
    [string]$DatabasePath = "cassette.db",
    [string]$ReportRoot = "A:\music_admin\recovery_reports",
    [switch]$WhatIf
)

$ErrorActionPreference = "Stop"

function Get-AudioExtensionFromSignature {
    param(
        [byte[]]$Bytes,
        [string]$Path
    )

    if ($Bytes.Length -lt 12) {
        return $null
    }

    if ($Bytes[0] -eq 0x66 -and $Bytes[1] -eq 0x4C -and $Bytes[2] -eq 0x61 -and $Bytes[3] -eq 0x43) {
        return ".flac"
    }

    if ($Bytes[0] -eq 0x49 -and $Bytes[1] -eq 0x44 -and $Bytes[2] -eq 0x33) {
        return ".mp3"
    }

    if ($Bytes[0] -eq 0xFF -and ($Bytes[1] -band 0xE0) -eq 0xE0) {
        return ".mp3"
    }

    if ($Bytes[0] -eq 0x4F -and $Bytes[1] -eq 0x67 -and $Bytes[2] -eq 0x67 -and $Bytes[3] -eq 0x53) {
        return ".ogg"
    }

    if ($Bytes[4] -eq 0x66 -and $Bytes[5] -eq 0x74 -and $Bytes[6] -eq 0x79 -and $Bytes[7] -eq 0x70) {
        return ".m4a"
    }

    if ($Bytes[0] -eq 0x52 -and $Bytes[1] -eq 0x49 -and $Bytes[2] -eq 0x46 -and $Bytes[3] -eq 0x46 -and
        $Bytes[8] -eq 0x57 -and $Bytes[9] -eq 0x41 -and $Bytes[10] -eq 0x56 -and $Bytes[11] -eq 0x45) {
        return ".wav"
    }

    return $null
}

function Quote-Sql {
    param([string]$Value)

    if ($null -eq $Value) {
        return "NULL"
    }

    return "'" + $Value.Replace("'", "''") + "'"
}

if (-not (Test-Path -LiteralPath $Root)) {
    throw "Root path not found: $Root"
}

if (-not (Test-Path -LiteralPath $DatabasePath)) {
    throw "Database not found: $DatabasePath"
}

$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$runDir = Join-Path $ReportRoot $stamp
New-Item -ItemType Directory -Force -Path $runDir | Out-Null

$candidates = Get-ChildItem $Root -Recurse -File -ErrorAction SilentlyContinue | Where-Object {
    $_.Extension -eq ".bin" -or $_.Extension -like ".codectype(*)"
}

$actions = New-Object System.Collections.Generic.List[object]
$sqlStatements = New-Object System.Collections.Generic.List[string]

foreach ($file in $candidates) {
    $bytes = [System.IO.File]::ReadAllBytes($file.FullName)
    $targetExtension = Get-AudioExtensionFromSignature -Bytes $bytes -Path $file.FullName
    $targetPath = $null
    $result = "skipped_unknown_signature"
    $reason = "unrecognized signature"

    if ($null -ne $targetExtension) {
        $targetPath = [System.IO.Path]::ChangeExtension($file.FullName, $targetExtension)
        if ($targetPath -ieq $file.FullName) {
            $result = "already_correct_extension"
            $reason = "extension already matches signature"
        }
        elseif (Test-Path -LiteralPath $targetPath) {
            $result = "skipped_destination_exists"
            $reason = "target path already exists"
        }
        elseif ($WhatIf) {
            $result = "whatif"
            $reason = "preview only"
        }
        else {
            Rename-Item -LiteralPath $file.FullName -NewName ([System.IO.Path]::GetFileName($targetPath))
            $sql = "UPDATE local_files SET file_path = {0}, extension = {1}, updated_at = CURRENT_TIMESTAMP WHERE file_path = {2};" -f `
                (Quote-Sql $targetPath), `
                (Quote-Sql $targetExtension.TrimStart('.')), `
                (Quote-Sql $file.FullName)
            $sqlStatements.Add($sql) | Out-Null
            $result = "renamed"
            $reason = "signature matched $targetExtension"
        }
    }

    $actions.Add([pscustomobject]@{
        source_path = $file.FullName
        target_path = $targetPath
        original_extension = $file.Extension
        detected_extension = $targetExtension
        result = $result
        reason = $reason
        timestamp = (Get-Date).ToString("o")
    }) | Out-Null
}

$actionsCsv = Join-Path $runDir "recovered_extensions.csv"
$summaryJson = Join-Path $runDir "recovered_extensions_summary.json"
$sqlPath = Join-Path $runDir "sync_local_files.sql"

$actions | Export-Csv -NoTypeInformation -Encoding UTF8 $actionsCsv
$sqlStatements | Set-Content -Encoding UTF8 $sqlPath

if (-not $WhatIf -and $sqlStatements.Count -gt 0) {
    Get-Content -LiteralPath $sqlPath | sqlite3 $DatabasePath
}

$summary = [ordered]@{
    root = $Root
    database = $DatabasePath
    run_dir = $runDir
    total_candidates = $actions.Count
    renamed = @($actions | Where-Object { $_.result -eq "renamed" }).Count
    skipped_destination_exists = @($actions | Where-Object { $_.result -eq "skipped_destination_exists" }).Count
    skipped_unknown_signature = @($actions | Where-Object { $_.result -eq "skipped_unknown_signature" }).Count
    already_correct_extension = @($actions | Where-Object { $_.result -eq "already_correct_extension" }).Count
    whatif = @($actions | Where-Object { $_.result -eq "whatif" }).Count
    actions_csv = $actionsCsv
    sql_path = $sqlPath
}

$summary | ConvertTo-Json -Depth 5 | Set-Content -Encoding UTF8 $summaryJson

Write-Output "Recovery complete."
Write-Output "Summary: $summaryJson"
Write-Output "Actions: $actionsCsv"
