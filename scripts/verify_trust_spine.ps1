$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $repoRoot

Write-Host "[trust-spine] cargo check --workspace"
cargo check --workspace

Write-Host "[trust-spine] targeted request-contract tests"
cargo test -p cassette-core acquisition_request_ -- --nocapture

Write-Host "[trust-spine] targeted audit-trace test"
cargo test -p cassette-core explain_audit_trace_collects_operation_events_and_gatekeeper_rows -- --nocapture

Write-Host "[trust-spine] cassette-core suite"
cargo test -p cassette-core

Write-Host "[trust-spine] full workspace tests"
cargo test --workspace

Write-Host "[trust-spine] ui build"
Push-Location (Join-Path $repoRoot "ui")
npm run build
Pop-Location

Write-Host "[trust-spine] desktop smoke"
& (Join-Path $repoRoot "scripts\smoke_desktop.ps1")
