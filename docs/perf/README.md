# Performance Baseline Artifacts

This folder contains the formal performance contract for Cassette.

## Files

- `BUDGETS.json`: Regression thresholds per scenario.
- `BASELINE.latest.json`: Current approved baseline timings.

## Workflow

1. Capture candidate timings:

```powershell
.\scripts\perf_baseline_capture.ps1 -Runs 3 -WarmupRuns 1
```

2. Compare candidate vs baseline:

```powershell
.\scripts\perf_regression_gate.ps1 -CandidateResultPath artifacts\perf\run-<timestamp>\results.json
```

3. Promote candidate baseline when accepted:

```powershell
Copy-Item artifacts\perf\run-<timestamp>\results.json docs\perf\BASELINE.latest.json -Force
```
