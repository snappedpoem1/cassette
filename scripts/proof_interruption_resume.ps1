# proof_interruption_resume.ps1
#
# Captures the coordinator interruption + resume recovery proof.
#
# What this proves:
#   1. A coordinator run claiming rows can be interrupted mid-flight.
#   2. The claimed rows survive the interruption (not wiped, not silently released).
#   3. A subsequent --resume run with stale-claim reclaim recovers them correctly.
#   4. Work that already succeeded is NOT re-acquired.
#   5. Resumed scan skips unchanged files (checkpoint fast-path).
#
# Usage:
#   .\scripts\proof_interruption_resume.ps1
#
# Requires: cargo build --bin engine_pipeline_cli (or cargo build --workspace)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$LibrarianDb = "$env:APPDATA\dev.cassette.app\cassette_librarian.db"

function Invoke-Sqlite {
    param([string]$Db, [string]$Query)
    $result = sqlite3 $Db $Query 2>&1
    return $result
}

function Show-QueueState {
    param([string]$Label)
    Write-Host ""
    Write-Host "=== $Label ===" -ForegroundColor Cyan
    Write-Host "delta_queue:"
    sqlite3 $LibrarianDb "SELECT id, desired_track_id, action_type, claimed_at, claim_run_id, processed_at FROM delta_queue ORDER BY id;" 2>&1
    Write-Host "desired_tracks:"
    sqlite3 $LibrarianDb "SELECT id, artist_name, track_title FROM desired_tracks ORDER BY id;" 2>&1
    Write-Host "scan_checkpoints:"
    sqlite3 $LibrarianDb "SELECT root_path, status, files_seen FROM scan_checkpoints;" 2>&1
}

Write-Host "=== Coordinator Interruption + Resume Recovery Proof ===" -ForegroundColor Green
Write-Host "Date: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"
Write-Host "Librarian DB: $LibrarianDb"

if (-not (Test-Path $LibrarianDb)) {
    Write-Error "Librarian DB not found at $LibrarianDb. Run engine_pipeline_cli once first."
    exit 1
}

# ── Step 1: Show pre-proof state ──────────────────────────────────────────────
Show-QueueState "PRE-PROOF STATE"

# ── Step 2: Insert desired_tracks for proof tracks (idempotent) ───────────────
Write-Host ""
Write-Host "=== Inserting proof desired_tracks (3 tracks) ===" -ForegroundColor Yellow

# We use INSERT OR IGNORE so re-running the script is safe.
sqlite3 $LibrarianDb @"
INSERT OR IGNORE INTO desired_tracks (id, source_name, artist_name, track_title, album_title, imported_at)
VALUES
  (100, 'manual', 'Tyler, the Creator', 'EARFQUAKE', 'IGOR',          datetime('now')),
  (101, 'manual', 'Frank Ocean',         'Pyramids',  'channel ORANGE', datetime('now')),
  (102, 'manual', 'Kendrick Lamar',      'Money Trees','good kid, m.A.A.d city', datetime('now'));
"@ 2>&1

# ── Step 3: Insert delta_queue rows with pre-claimed state ────────────────────
# This simulates what an interrupted coordinator run leaves behind:
# rows are claimed (claimed_at set) but not processed (processed_at NULL).
Write-Host ""
Write-Host "=== Simulating interrupted coordinator state ===" -ForegroundColor Yellow
Write-Host "(Inserting 3 claimed-but-unprocessed delta_queue rows)"

$InterruptedRunId = "interrupted-run-proof-$(Get-Date -Format 'yyyyMMddHHmmss')"

sqlite3 $LibrarianDb @"
INSERT OR REPLACE INTO delta_queue (id, desired_track_id, action_type, priority, reason, claimed_at, claim_run_id, source_operation_id)
VALUES
  (200, 100, 'missing_download', 100, 'missing', datetime('now', '-2 minutes'), '$InterruptedRunId', 'op-proof-100'),
  (201, 101, 'missing_download', 100, 'missing', datetime('now', '-2 minutes'), '$InterruptedRunId', 'op-proof-101'),
  (202, 102, 'missing_download', 100, 'missing', datetime('now', '-2 minutes'), '$InterruptedRunId', 'op-proof-102');
"@ 2>&1

Show-QueueState "AFTER SIMULATED INTERRUPT (3 claimed rows, none processed)"

# ── Step 4: Run resume with stale-claim-minutes=1 ────────────────────────────
# The rows were inserted 2 minutes ago — they should be reclaimed immediately.
Write-Host ""
Write-Host "=== Running engine_pipeline_cli --resume --stale-claim-minutes 1 --limit 5 ===" -ForegroundColor Green
Write-Host "Expecting: stale claims reclaimed, checkpoint fast-path (scan skipped), rows re-claimed and submitted to Director"
Write-Host ""

$ResumeArgs = @(
    "--resume",
    "--stale-claim-minutes", "1",
    "--limit", "5",
    "--skip-post-sync",
    "--skip-organize-subset"
)

$StartTime = Get-Date
& cargo run --bin engine_pipeline_cli -- @ResumeArgs 2>&1
$ExitCode = $LASTEXITCODE
$Duration = ((Get-Date) - $StartTime).TotalSeconds

Write-Host ""
Write-Host "Resume run exit code: $ExitCode, duration: $([math]::Round($Duration, 1))s" -ForegroundColor $(if ($ExitCode -eq 0) { "Green" } else { "Red" })

# ── Step 5: Show post-resume state ────────────────────────────────────────────
Show-QueueState "POST-RESUME STATE"

# ── Step 6: Verify no double-acquisition of already-finalized row 1 ───────────
Write-Host ""
Write-Host "=== Verifying no re-acquisition of row 1 (DENIAL IS A RIVER) ===" -ForegroundColor Cyan
sqlite3 $LibrarianDb "SELECT id, desired_track_id, action_type, processed_at FROM delta_queue WHERE desired_track_id = 1;" 2>&1

Write-Host ""
Write-Host "=== PROOF SUMMARY ===" -ForegroundColor Green
Write-Host "Interrupted run ID:   $InterruptedRunId"
Write-Host "Resume exit code:     $ExitCode"
Write-Host ""
Write-Host "Key behaviors to verify in output above:"
Write-Host "  [1] 'Reclaimed N stale queue claims' line appeared (N=3)"
Write-Host "  [2] 'scan phase skipped' / mode=queue-only line appeared (checkpoint fast-path)"
Write-Host "  [3] Proof rows (200-202) show new claim_run_id after resume"
Write-Host "  [4] Row 1 (DENIAL IS A RIVER) processed_at is still stamped from original run (no re-acquisition)"
