# Lane C Probe Runbook

Last updated: 2026-04-07
Covers: GAP-C02, GAP-C03, GAP-C04

## Purpose

Provide a reproducible bounded probe for SAB completion-polling readiness and LRCLIB endpoint behavior, plus a deterministic failure taxonomy for classification.

## Artifacts

- docs/probes/lane_c_probe_2026-04-07.json
- docs/probes/provider_probe_2026-04-07.txt
- docs/probes/lane_c_probe_2026-04-07_174445.json
- docs/probes/provider_probe_2026-04-07_174445.txt

## Repro Commands

```powershell
Set-Location "C:/Cassette Music"
./scripts/capture_provider_reliability_snapshot.ps1
```

```powershell
Set-Location "C:/Cassette Music"
cargo run -p cassette --bin provider_probe_cli
```

```powershell
Set-Location "C:/Cassette Music"
$ErrorActionPreference='Stop'
$dbPath = Join-Path $env:APPDATA 'dev.cassette.app/cassette.db'

function Get-Setting([string]$name) {
  $value = sqlite3 $dbPath "select value from settings where key='$name' limit 1;" 2>$null
  if ([string]::IsNullOrWhiteSpace($value)) { return $null }
  return $value.Trim()
}

$sabUrl = Get-Setting 'sabnzbd_url'
$sabKey = Get-Setting 'sabnzbd_api_key'

if ($sabUrl -and $sabKey) {
  Invoke-RestMethod -Method Get -Uri ($sabUrl.TrimEnd('/') + '/api') -Body @{ mode='queue'; apikey=$sabKey; output='json'; limit='1' } -TimeoutSec 20 | Out-Null
  Invoke-RestMethod -Method Get -Uri ($sabUrl.TrimEnd('/') + '/api') -Body @{ mode='history'; apikey=$sabKey; output='json'; limit='1' } -TimeoutSec 20 | Out-Null
}

Invoke-RestMethod -Method Get -Uri 'https://lrclib.net/api/get' -Body @{ artist_name='Simon & Garfunkel'; track_name='Bridge Over Troubled Water' } -TimeoutSec 20 | Out-Null
```

## Failure Taxonomy

### GAP-C02 (SAB completion proof)

- bounded-probe: SAB `queue` and `history` endpoints both reachable with configured credentials.
- unverified/config-missing: `sabnzbd_url` and/or `sabnzbd_api_key` absent in runtime settings.
- unverified/auth-failed: SAB endpoint reachable but authentication rejected.
- unverified/network: HTTP/connectivity errors while polling queue/history.

Current classification (2026-04-07):

- unverified/config-missing (see docs/probes/lane_c_probe_2026-04-07_174445.json and docs/probes/provider_probe_2026-04-07_174445.txt).

### GAP-C03 (LRCLIB verification)

- bounded-probe: LRCLIB endpoint returns lyrics payload for probe query.
- unverified/network: request failed or timed out.
- unverified/empty: endpoint reachable but no parseable payload.

Current classification (2026-04-07):

- bounded-probe with both plain and synced lyrics present (see docs/probes/lane_c_probe_2026-04-07_174445.json).

## Status Vocabulary

Provider reliability scope vocabulary used by canonical docs:

- local-proven
- bounded-probe
- unverified
