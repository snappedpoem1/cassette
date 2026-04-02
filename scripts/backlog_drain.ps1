# backlog_drain.ps1
# Waterfall acquisition coordinator for the Spotify missing-album backlog.
# Order: torrent (RD/TPB) -> Soulseek (slskd) -> Deezer/Qobuz (engine_pipeline_cli)
# Each provider only touches albums not yet in_library.
# Runs continuously, reporting deltas after each provider pass.

param(
    [int]$MinPlays    = 3,
    [int]$TorrentBatch = 50,
    [int]$SlskdBatch   = 30,
    [int]$DeezerBatch  = 100,
    [int]$Rounds       = 999,
    [switch]$DryRun
)

$BinDir  = "$PSScriptRoot\..\target\release"
$DB      = "$env:APPDATA\dev.cassette.app\cassette.db"
$DryFlag = if ($DryRun) { "--dry-run" } else { "" }
$LogDir  = "$PSScriptRoot\..\logs"
New-Item -ItemType Directory -Force -Path $LogDir | Out-Null

function Get-Remaining {
    $count = & sqlite3 $DB "SELECT COUNT(*) FROM spotify_album_history WHERE in_library=0 AND play_count>=$MinPlays;" 2>&1
    return [int]$count
}

function Get-InLibrary {
    $count = & sqlite3 $DB "SELECT COUNT(*) FROM spotify_album_history WHERE in_library=1;" 2>&1
    return [int]$count
}

function Write-Status($msg) {
    $ts = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Write-Host "[$ts] $msg"
}

function Run-Provider($name, $cmd, $cmdArgs, $logFile) {
    Write-Status "=== $name pass starting ==="
    $before = Get-InLibrary

    $argStr = ($cmdArgs | ForEach-Object { "$_" } | Where-Object { $_ -ne "" -and $null -ne $_ }) -join " "
    Write-Status "  cmd: $cmd $argStr"

    # Start-Process with file redirection avoids pipe-hang with Rust binaries on Windows
    $proc = Start-Process -FilePath $cmd `
        -ArgumentList $argStr `
        -RedirectStandardOutput $logFile `
        -RedirectStandardError  "$logFile.err" `
        -NoNewWindow -PassThru
    # Wait up to 4 hours; -1 means no timeout
    $proc.WaitForExit(-1) | Out-Null
    $exit = $proc.ExitCode

    # Print log to console
    if (Test-Path $logFile) { Get-Content $logFile | ForEach-Object { Write-Host "  $_" } }

    $after  = Get-InLibrary
    $gained = $after - $before

    Write-Status "=== $name pass complete: +$gained in_library (exit=$exit) ==="
    return $gained
}

# ── main loop ─────────────────────────────────────────────────────────────────

Write-Status "backlog_drain starting | min_plays=$MinPlays | dry_run=$DryRun"
Write-Status "Remaining: $(Get-Remaining) albums | In library: $(Get-InLibrary)"

for ($round = 1; $round -le $Rounds; $round++) {
    $remaining = Get-Remaining
    if ($remaining -eq 0) {
        Write-Status "Backlog fully drained. Done."
        break
    }

    Write-Status ""
    Write-Status "────────────────────────────────────────────────"
    Write-Status "ROUND $round | Remaining: $remaining | In library: $(Get-InLibrary)"
    Write-Status "────────────────────────────────────────────────"

    $ts = Get-Date -Format "yyyyMMdd_HHmmss"

    # ── Pass 1: Torrent (Real-Debrid + TPB/Jackett) ───────────────────────────
    $torrentArgs = @("--limit", $TorrentBatch, "--min-plays", $MinPlays)
    if ($DryRun) { $torrentArgs = $torrentArgs + @("--dry-run") }
    $g1 = Run-Provider "TORRENT" "$BinDir\torrent_album_cli.exe" $torrentArgs `
        "$LogDir\torrent_$ts.log"

    Start-Sleep -Seconds 10

    # ── Pass 2: Soulseek (slskd) ─────────────────────────────────────────────
    $slskdArgs = @("--limit", $SlskdBatch, "--min-plays", $MinPlays)
    if ($DryRun) { $slskdArgs = $slskdArgs + @("--dry-run") }
    $g2 = Run-Provider "SOULSEEK" "$BinDir\slskd_album_cli.exe" $slskdArgs `
        "$LogDir\slskd_$ts.log"

    Start-Sleep -Seconds 10

    # ── Pass 3: Deezer/Qobuz (engine_pipeline_cli --import-spotify-missing) ──
    # Note: engine_pipeline_cli does not support --dry-run; skip in dry mode
    $engineArgs = @("--import-spotify-missing", "--min-plays", $MinPlays, "--limit", $DeezerBatch)
    if (-not $DryRun) {
        $g3 = Run-Provider "DEEZER/QOBUZ" "$BinDir\engine_pipeline_cli.exe" $engineArgs `
            "$LogDir\engine_$ts.log"
    } else {
        Write-Status "=== DEEZER/QOBUZ pass skipped (dry-run mode) ==="
        $g3 = 0
    }

    $roundGained = $g1 + $g2 + $g3
    $remaining   = Get-Remaining

    Write-Status ""
    Write-Status "ROUND $round SUMMARY"
    Write-Status "  Torrent:      +$g1"
    Write-Status "  Soulseek:     +$g2"
    Write-Status "  Deezer/Qobuz: +$g3"
    Write-Status "  Total gained: +$roundGained"
    Write-Status "  Remaining:    $remaining"

    if ($roundGained -eq 0) {
        Write-Status "No progress this round. Waiting 10 min before retrying..."
        Start-Sleep -Seconds 600
    } else {
        # Brief pause between rounds
        Start-Sleep -Seconds 30
    }
}

Write-Status "backlog_drain finished. Final in_library: $(Get-InLibrary)"
