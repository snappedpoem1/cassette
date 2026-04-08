param(
    [string]$Tag = ""
)

$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $repoRoot

if ([string]::IsNullOrWhiteSpace($Tag)) {
    $Tag = Get-Date -Format "yyyy-MM-dd_HHmmss"
}

$probeDir = Join-Path $repoRoot "docs/probes"
if (-not (Test-Path $probeDir)) {
    New-Item -ItemType Directory -Path $probeDir | Out-Null
}

$providerProbePath = Join-Path $probeDir ("provider_probe_{0}.txt" -f $Tag)
$laneProbePath = Join-Path $probeDir ("lane_c_probe_{0}.json" -f $Tag)

Write-Host "[provider-snapshot] running provider_probe_cli..."
$providerProbeOutput = cmd /c "cargo run -p cassette --bin provider_probe_cli 2>&1"
$providerExitCode = $LASTEXITCODE
$providerProbeOutput | Out-File -FilePath $providerProbePath -Encoding utf8
if ($providerExitCode -ne 0) {
    throw "provider_probe_cli failed with exit code $providerExitCode"
}

$dbPath = Join-Path $env:APPDATA "dev.cassette.app/cassette.db"

function Get-Setting([string]$name) {
    $value = sqlite3 $dbPath "select value from settings where key='$name' limit 1;" 2>$null
    if ([string]::IsNullOrWhiteSpace($value)) { return $null }
    return $value.Trim()
}

function Invoke-SabProbe([string]$sabUrl, [string]$sabKey) {
    if ([string]::IsNullOrWhiteSpace($sabUrl) -or [string]::IsNullOrWhiteSpace($sabKey)) {
        return [pscustomobject]@{
            configured = $false
            queue_probe = $null
            history_probe = $null
            classification = "unverified"
            notes = @("Missing sabnzbd_url and/or sabnzbd_api_key in runtime settings.")
        }
    }

    try {
        $queue = Invoke-RestMethod -Method Get -Uri ($sabUrl.TrimEnd('/') + '/api') -Body @{ mode='queue'; apikey=$sabKey; output='json'; limit='1' } -TimeoutSec 20
        $history = Invoke-RestMethod -Method Get -Uri ($sabUrl.TrimEnd('/') + '/api') -Body @{ mode='history'; apikey=$sabKey; output='json'; limit='1' } -TimeoutSec 20
        return [pscustomobject]@{
            configured = $true
            queue_probe = [pscustomobject]@{ ok = $true; status = "ok"; sample = $queue.status }
            history_probe = [pscustomobject]@{ ok = $true; status = "ok"; sample = $history.status }
            classification = "bounded-probe"
            notes = @("SAB queue/history endpoints reachable with configured credentials.")
        }
    }
    catch {
        return [pscustomobject]@{
            configured = $true
            queue_probe = $null
            history_probe = $null
            classification = "unverified"
            notes = @("SAB probe failed: $($_.Exception.Message)")
        }
    }
}

function Invoke-LrclibProbe() {
    try {
        $lrclib = Invoke-RestMethod -Method Get -Uri 'https://lrclib.net/api/get' -Body @{ artist_name='Simon & Garfunkel'; track_name='Bridge Over Troubled Water' } -TimeoutSec 20
        $hasPlain = -not [string]::IsNullOrWhiteSpace($lrclib.plainLyrics)
        $hasSynced = -not [string]::IsNullOrWhiteSpace($lrclib.syncedLyrics)

        return [pscustomobject]@{
            http_probe = [pscustomobject]@{
                ok = $true
                has_plain = $hasPlain
                has_synced = $hasSynced
            }
            classification = "bounded-probe"
            notes = @("LRCLIB HTTP endpoint reachable and returned lyrics payload for probe query.")
        }
    }
    catch {
        return [pscustomobject]@{
            http_probe = [pscustomobject]@{
                ok = $false
                has_plain = $false
                has_synced = $false
            }
            classification = "unverified"
            notes = @("LRCLIB probe failed: $($_.Exception.Message)")
        }
    }
}

$sabUrl = Get-Setting "sabnzbd_url"
$sabKey = Get-Setting "sabnzbd_api_key"

$snapshot = [pscustomobject]@{
    generated_at_utc = (Get-Date).ToUniversalTime().ToString("o")
    db_path = $dbPath
    source_provider_probe = [System.IO.Path]::GetFileName($providerProbePath)
    sab = Invoke-SabProbe -sabUrl $sabUrl -sabKey $sabKey
    lrclib = Invoke-LrclibProbe
}

$snapshot | ConvertTo-Json -Depth 8 | Out-File -FilePath $laneProbePath -Encoding utf8

Write-Host "[provider-snapshot] wrote $providerProbePath"
Write-Host "[provider-snapshot] wrote $laneProbePath"