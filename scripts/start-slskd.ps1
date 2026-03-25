$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent $PSScriptRoot
$envPath = Join-Path $projectRoot ".env"
$bundledExe = Join-Path $projectRoot "binaries\slskd\slskd.exe"
$localAppDir = Join-Path $env:LOCALAPPDATA "slskd"
$localExe = Join-Path $localAppDir "slskd.exe"
$localYaml = Join-Path $localAppDir "slskd.yml"
$appDir = Join-Path $projectRoot ".slskd"
$downloadsDir = "A:\Music"
$incompleteDir = Join-Path $appDir "incomplete"
$webUrl = "http://localhost:5030"

function Read-KeyValueFile([string]$path) {
    $map = @{}
    if (-not (Test-Path $path)) {
        return $map
    }

    foreach ($line in Get-Content $path) {
        if ([string]::IsNullOrWhiteSpace($line) -or $line.StartsWith("#") -or -not $line.Contains("=")) {
            continue
        }

        $parts = $line -split "=", 2
        $map[$parts[0].Trim()] = $parts[1].Trim()
    }

    return $map
}

function Read-SlskdYamlValue([string]$path, [string]$key) {
    if (-not (Test-Path $path)) {
        return $null
    }

    foreach ($line in Get-Content $path) {
        $trimmed = $line.Trim()
        if ($trimmed.StartsWith("${key}:")) {
            return $trimmed.Substring($key.Length + 1).Trim().Trim("'").Trim('"')
        }
    }

    return $null
}

$envMap = Read-KeyValueFile $envPath
$soulseekUser = $envMap["SOULSEEK_USERNAME"]
$soulseekPass = $envMap["SOULSEEK_PASSWORD"]
$slskdUser = $envMap["SLSKD_USER"]
$slskdPass = $envMap["SLSKD_PASSWORD"]

if ([string]::IsNullOrWhiteSpace($soulseekUser)) {
    $soulseekUser = Read-SlskdYamlValue $localYaml "username"
}
if ([string]::IsNullOrWhiteSpace($soulseekPass)) {
    $soulseekPass = Read-SlskdYamlValue $localYaml "password"
}
if ([string]::IsNullOrWhiteSpace($slskdUser)) {
    $slskdUser = "slskd"
}
if ([string]::IsNullOrWhiteSpace($slskdPass)) {
    $slskdPass = "slskd"
}

$exePath = $null
$launchMode = $null

if (Test-Path $bundledExe) {
    $exePath = $bundledExe
    $launchMode = "bundled"
} elseif (Test-Path $localExe) {
    $exePath = $localExe
    $launchMode = "local"
} else {
    throw "slskd.exe was not found. Place the Windows binary at $bundledExe or install slskd under $localAppDir."
}

$existing = Get-Process slskd -ErrorAction SilentlyContinue
if ($existing) {
    $existing | Select-Object ProcessName, Id, Path
    Write-Output "slskd already running at $webUrl"
    exit 0
}

New-Item -ItemType Directory -Path $appDir, $incompleteDir -Force | Out-Null

if ($launchMode -eq "bundled") {
    if ([string]::IsNullOrWhiteSpace($soulseekUser) -or [string]::IsNullOrWhiteSpace($soulseekPass)) {
        throw "Bundled slskd requires SOULSEEK_USERNAME and SOULSEEK_PASSWORD in .env, or a populated $localYaml."
    }

    $args = @(
        "--app-dir", $appDir,
        "--http-port", "5030",
        "--no-https",
        "--username", $slskdUser,
        "--password", $slskdPass,
        "--downloads", $downloadsDir,
        "--incomplete", $incompleteDir,
        "--slsk-username", $soulseekUser,
        "--slsk-password", $soulseekPass,
        "--slsk-listen-port", "50400",
        "--no-share-scan",
        "--no-logo"
    )

    $process = Start-Process -FilePath $exePath -ArgumentList $args -WorkingDirectory (Split-Path $exePath -Parent) -WindowStyle Hidden -PassThru
} else {
    $process = Start-Process -FilePath $exePath -WorkingDirectory (Split-Path $exePath -Parent) -WindowStyle Hidden -PassThru
}

Start-Sleep -Seconds 6

if (-not (Get-Process -Id $process.Id -ErrorAction SilentlyContinue)) {
    throw "slskd exited during startup"
}

$reachable = Test-NetConnection -ComputerName localhost -Port 5030 -WarningAction SilentlyContinue
if (-not $reachable.TcpTestSucceeded) {
    throw "slskd started but localhost:5030 is not reachable"
}

Get-Process -Id $process.Id | Select-Object ProcessName, Id, Path
Write-Output "slskd ready at $webUrl"
