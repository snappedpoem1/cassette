$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent $PSScriptRoot
$targetDir = Join-Path $projectRoot "binaries\slskd"
$zipPath = Join-Path $env:TEMP "slskd-win-x64.zip"
$releaseApi = "https://api.github.com/repos/slskd/slskd/releases/latest"

New-Item -ItemType Directory -Path $targetDir -Force | Out-Null

$release = Invoke-RestMethod -Uri $releaseApi -Headers @{ "User-Agent" = "Cassette-Recovery" }
$asset = $release.assets | Where-Object { $_.name -match "win-x64\.zip$" } | Select-Object -First 1

if (-not $asset) {
    throw "Could not find a Windows x64 slskd asset in the latest GitHub release."
}

Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $zipPath -UseBasicParsing

if (Test-Path (Join-Path $targetDir "slskd.exe")) {
    Remove-Item (Join-Path $targetDir "slskd.exe") -Force
}

Expand-Archive -Path $zipPath -DestinationPath $targetDir -Force
Remove-Item $zipPath -Force

Get-ChildItem $targetDir
