param(
	[switch]$RunCleanroomLocal,
	[ValidateSet("Sandbox", "DisposableProfile", "AppDataReset")]
	[string]$CleanroomMode = "DisposableProfile"
)

$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $repoRoot

function Invoke-Step {
	param(
		[string]$Label,
		[scriptblock]$Action
	)

	Write-Host "[trust-spine] $Label"
	& $Action
	if ($LASTEXITCODE -ne 0) {
		throw "Step failed: $Label (exit code $LASTEXITCODE)"
	}
}

Invoke-Step -Label "cargo check --workspace" -Action {
	cargo check --workspace
}

Invoke-Step -Label "targeted request-contract tests" -Action {
	cargo test -p cassette-core acquisition_request_ -- --nocapture
}

Invoke-Step -Label "targeted audit-trace test" -Action {
	cargo test -p cassette-core explain_audit_trace_collects_operation_events_and_gatekeeper_rows -- --nocapture
}

Invoke-Step -Label "cassette-core suite" -Action {
	cargo test -p cassette-core
}

Invoke-Step -Label "full workspace tests" -Action {
	cargo test --workspace
}

Invoke-Step -Label "ui build" -Action {
	Push-Location (Join-Path $repoRoot "ui")
	try {
		npm run build
		if ($LASTEXITCODE -ne 0) {
			throw "npm run build failed with exit code $LASTEXITCODE"
		}
	}
	finally {
		Pop-Location
	}
}

Invoke-Step -Label "desktop smoke" -Action {
	& (Join-Path $repoRoot "scripts\smoke_desktop.ps1") -Strict
}

Invoke-Step -Label "docs consistency" -Action {
	& (Join-Path $repoRoot "scripts\check_docs_state.ps1")
}

if ($RunCleanroomLocal) {
	Invoke-Step -Label "clean-room local verification ($CleanroomMode)" -Action {
		& (Join-Path $repoRoot "scripts\verify_cleanroom_local.ps1") -Mode $CleanroomMode
	}
}
