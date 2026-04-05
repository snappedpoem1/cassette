Param()

$ErrorActionPreference = "Stop"

Write-Host "[ci-gate] cargo check --workspace"
cargo check --workspace
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "[ci-gate] cargo test -p cassette --test pure_logic"
cargo test -p cassette --test pure_logic -- --nocapture
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "[ci-gate] npm ci / check / build"
Push-Location "ui"
npm ci
if ($LASTEXITCODE -ne 0) {
  Pop-Location
  exit $LASTEXITCODE
}

npm run check
if ($LASTEXITCODE -ne 0) {
  Pop-Location
  exit $LASTEXITCODE
}

npm run build
$exitCode = $LASTEXITCODE
Pop-Location
if ($exitCode -ne 0) { exit $exitCode }

Write-Host "[ci-gate] smoke_desktop_ci"
powershell -ExecutionPolicy Bypass -File scripts/smoke_desktop_ci.ps1
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "[ci-gate] PASS"
