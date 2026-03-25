# Cassette Telemetry

This file tracks what we know about build, runtime, and operational confidence.

**Last Updated**: 2026-03-24

---

## Current Baseline

Documented in existing repo docs:

- Rust workspace: `cargo check` has passed
- UI: `npm run build` in `ui` has passed
- Desktop smoke checks: `scripts/smoke_desktop.ps1` has passed

These are confidence signals, not yet a full performance budget.

---

## Quality Gates

Minimum ongoing checks:

- `cargo check`
- `cargo test`
- `ui` production build
- desktop smoke script

Operational checks to strengthen over time:

- validation/logging completeness
- provider failure recovery
- temp/staging cleanup correctness
- long-session desktop stability

---

## Metrics To Track

### Build Health

- Rust compile success
- Rust test success
- UI build success
- warning count

### Runtime Health

- desktop smoke success
- provider status visibility
- validation pass/fail
- number of orphaned or weakly-attributed operations, if any

### Performance

Targets are still being formalized. When benchmarking begins, record:

- scan duration
- organize duration
- validation duration
- startup time
- UI responsiveness regressions

---

## Known Gaps

- No formal benchmark suite is recorded here yet.
- No numeric regression budget is enforced yet.
- Provider reliability is still machine- and configuration-dependent.
- Packaging confidence is not yet represented as a repeatable telemetry artifact.

---

## Update Policy

Update this file when:

- a benchmark is added
- a command meaningfully slows down or speeds up
- a new reliability gate is introduced
- a confidence claim is verified or disproven
