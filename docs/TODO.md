# Cassette TODO

**Method**: Prioritize by user impact, reliability risk, and execution clarity.  
**Rule**: If a task is not in this file, it is not committed project scope yet.  
**Last Updated**: 2026-03-24

---

## Legend

Priority:

- `P0` critical shipping blocker
- `P1` important hardening work
- `P2` worthwhile improvement
- `P3` backlog or exploratory work

Status:

- `todo`
- `in_progress`
- `blocked`
- `review`
- `done`

---

## P0

### [P0] [todo] Prove audit completeness across organization and admission flows

Why:

- Cassette’s core promise is lineage and traceability.
- Validation support exists, but this remains a hard shipping gate.

What good looks like:

- Representative file flows produce complete operation and event trails.
- We can explain what happened to a file without guesswork.
- Validation/logging checks fail loudly if audit coverage regresses.

Suggested touchpoints:

- `crates/cassette-core/src/custodian`
- `crates/cassette-core/src/gatekeeper`
- `crates/cassette-core/src/library`
- `crates/cassette-core/src/validation`

Acceptance:

- [ ] Narrow tests added or updated
- [ ] Validation/logging proof is repeatable
- [ ] Documentation updated if expectations change

### [P0] [todo] Increase provider live-proof confidence on this machine

Why:

- Current repo docs still call out incomplete provider proof, especially for full-track provider paths.

What good looks like:

- Provider statuses are observable
- Known partial paths are documented precisely
- Failure modes are visible and recoverable

Acceptance:

- [ ] Repro steps documented
- [ ] Current machine-specific blockers recorded
- [ ] TODO narrowed into provider-specific follow-ups if needed

---

## P1

### [P1] [todo] Harden async and recovery behavior in acquisition flows

Why:

- Acquisition paths are where flaky networks, partial downloads, and timeouts converge.

Focus:

- retry behavior
- cancellation safety
- recovery after interruption
- temp/staging cleanup guarantees

Acceptance:

- [ ] Tests cover interruption or retry paths
- [ ] Any retry thresholds are named and documented
- [ ] Recovery behavior is explicit, not implied

### [P1] [todo] Formalize performance baseline and regression budget

Why:

- The repo has qualitative confidence, but not a strict performance contract yet.

Acceptance:

- [ ] Core commands benchmarked or timed
- [ ] Baselines recorded in [TELEMETRY.md](/c:/Cassette%20Music/docs/TELEMETRY.md)
- [ ] Regression thresholds documented

### [P1] [todo] Raise packaging and clean-machine confidence

Why:

- “Builds in this workspace” is not the same as “ready to ship”.

Acceptance:

- [ ] Install/build steps documented for a clean environment
- [ ] Gaps and assumptions recorded
- [ ] Release checklist updated

---

## P2

### [P2] [todo] Expand API and command-surface documentation

Why:

- The command surface is broad enough that new agents benefit from a tighter map.

Acceptance:

- [ ] Public commands and their responsibilities are documented
- [ ] Key input/output expectations are listed

### [P2] [todo] Document and test long-session desktop behavior

Why:

- Media apps earn trust through stability over time, not just one clean smoke run.

Acceptance:

- [ ] Soak-test procedure documented
- [ ] Known leaks, stalls, or recovery pain points recorded if found

### [P2] [todo] Tighten metadata and enrichment operating story

Why:

- Metadata logic exists, but the runtime ownership and lifecycle are still less explicit than core library flows.

Acceptance:

- [ ] Current enrichment behavior documented
- [ ] Future integration plan recorded without overstating readiness

---

## P3

### [P3] [todo] Improve advanced-route discoverability without cluttering primary navigation

### [P3] [todo] Add richer provider health and troubleshooting views in UI

### [P3] [todo] Revisit broader release automation once packaging proof is stable

---

## Completed / Confirmed True

These are not active TODOs; they are baseline facts observed in current repo docs:

- Workspace structure is recovered and functional
- `cargo check` has passed in this workspace
- `ui` production build has passed in this workspace
- Desktop smoke checks are available through `scripts/smoke_desktop.ps1`
- Library, downloads, playlists, import, settings, playback, and organizer surfaces are implemented

---

## Operating Notes For Agents

When you pick up a task:

1. Update status from `todo` to `in_progress` if you are actively working it.
2. Keep the task scoped.
3. Add linked file paths or commands if you discover the task is narrower than written.
4. Move it to `review` only after verification.
5. Mark `done` only after code and docs both reflect reality.

If you notice a new problem but are not fixing it now, add it here with enough context for the next agent to act without rediscovery.
