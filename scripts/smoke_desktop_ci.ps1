param()

$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent $PSScriptRoot

Write-Host "[smoke-ci] cargo test -p cassette --test pure_logic"
Set-Location $projectRoot
cargo test -p cassette --test pure_logic -- --nocapture
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "[smoke-ci] ui check/build"
Set-Location (Join-Path $projectRoot "ui")
npm run check
if ($LASTEXITCODE -ne 0) {
  Set-Location $projectRoot
  exit $LASTEXITCODE
}

npm run build
$exitCode = $LASTEXITCODE
Set-Location $projectRoot
if ($exitCode -ne 0) { exit $exitCode }

Write-Host "[smoke-ci] PASS"
