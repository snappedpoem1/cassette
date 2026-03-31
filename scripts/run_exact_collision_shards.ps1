param(
    [string]$ManifestDir = "tmp/active_music_manifest_post_safe",
    [string]$OutputDir = "tmp/remediation/exact-collision-resolution-sharded",
    [string]$QuarantineRoot = "A:\music\_Cassette_Quarantine\exact_collision_duplicates",
    [int]$ShardCount = 4,
    [int]$WorkersPerShard = 4,
    [int]$CheckpointEvery = 100,
    [switch]$Apply,
    [switch]$Resume,
    [switch]$StopExisting
)

$ErrorActionPreference = "Stop"

if ($ShardCount -lt 1) {
    throw "ShardCount must be >= 1"
}

if ($WorkersPerShard -lt 1) {
    throw "WorkersPerShard must be >= 1"
}

if ($StopExisting) {
    $existing = Get-CimInstance Win32_Process |
        Where-Object { $_.CommandLine -like "*resolve_exact_collision_duplicates.py*" }
    foreach ($proc in $existing) {
        try {
            Stop-Process -Id $proc.ProcessId -Force -ErrorAction Stop
        }
        catch {
            Write-Warning "Failed to stop existing resolver PID $($proc.ProcessId): $($_.Exception.Message)"
        }
    }
}

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

$launched = New-Object System.Collections.Generic.List[object]

for ($i = 0; $i -lt $ShardCount; $i++) {
    $args = @(
        "-3",
        "scripts\resolve_exact_collision_duplicates.py",
        "--manifest-dir", $ManifestDir,
        "--output-dir", $OutputDir,
        "--quarantine-root", $QuarantineRoot,
        "--workers", $WorkersPerShard,
        "--checkpoint-every", $CheckpointEvery,
        "--shard-count", $ShardCount,
        "--shard-index", $i
    )

    if ($Apply) {
        $args += "--apply"
    }

    if ($Resume) {
        $args += "--resume"
    }

    $process = Start-Process -FilePath "py" -ArgumentList $args -WorkingDirectory "C:\Cassette Music" -PassThru
    $launched.Add([pscustomobject]@{
        shard_index = $i
        pid = $process.Id
        args = ($args -join " ")
        started_at = (Get-Date).ToString("o")
    })
}

$launchLog = Join-Path $OutputDir "launch_log.csv"
$launched | Export-Csv -NoTypeInformation -Encoding UTF8 $launchLog

Write-Output "Launched $ShardCount shard workers."
Write-Output "Launch log: $launchLog"
$launched | Format-Table -AutoSize
