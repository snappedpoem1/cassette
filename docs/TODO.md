# Cassette TODO

**Method**: Prioritize by user impact, reliability risk, and execution clarity.
**Rule**: If a task is not in this file, it is not committed project scope yet.
**Last Updated**: 2026-03-28

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

### [P0] [done] Prove Deezer full-track acquisition end-to-end

Why:

- This was a hard shipping blocker. The path is now proven live on this machine.

What good looks like:

- A real track (artist, album, title) acquires from Deezer at full quality.
- The acquisition is visible in the downloads dashboard with a `Done` status.
- The resulting file passes validation (not a preview-length clip).

Touchpoints:

- `crates/cassette-core/src/director/providers/deezer.rs`
- `crates/cassette-core/src/director/providers/crypto.rs`
- `crates/cassette-core/src/sources.rs` (deezer_get_media_url, deezer_get_track_data)

Acceptance:

- [x] End-to-end live proof documented in PROJECT_STATE.md
- [x] Any remaining partial paths named and tracked

### [P0] [todo] Prove audit completeness across organization and admission flows

Why:

- Lineage and traceability are core promises of Cassette. Validation support exists,
  but this must remain a provable shipping gate, not an assumption.

What good looks like:

- Representative file flows produce complete operation and event trails.
- We can explain what happened to a file without guesswork.
- Validation/logging checks fail loudly if audit coverage regresses.

Touchpoints:

- `crates/cassette-core/src/custodian`
- `crates/cassette-core/src/gatekeeper`
- `crates/cassette-core/src/library`
- `crates/cassette-core/src/validation`

Acceptance:

- [ ] Representative tests added or updated
- [ ] Validation/logging proof is repeatable
- [ ] Documentation updated if expectations change

### [P0] [done] Prove pending-task crash recovery end-to-end

Why:

- The runtime now persists pending director tasks and resubmits them on startup.
- This is now proven through a deterministic startup-recovery probe plus filtering tests for stale
  pending rows versus newer terminal history.

What good looks like:

- Pending rows rehydrate visible jobs before resubmission
- Resubmission order is deterministic (`created_at`, `task_id`)
- Stale pending rows do not resurrect when a newer terminal history row already exists
- A resumed task can finalize successfully after startup recovery

Touchpoints:

- `crates/cassette-core/src/db/mod.rs`
- `crates/cassette-core/src/director/engine.rs`
- `src-tauri/src/state.rs`
- `src-tauri/src/bin/recovery_probe_cli.rs`

Acceptance:

- [x] Startup recovery proof captured via `recovery_probe_cli`
- [x] Recovery ordering and stale-terminal filtering documented
- [x] PROJECT_STATE.md updated with exact observed behavior

---

## P1

### [P1] [todo] Harden async and recovery behavior in acquisition flows

Why:

- Acquisition paths are where flaky networks, partial downloads, and timeouts converge.
  Current behavior under interruption or retry is not formally tested.

Focus:

- retry behavior and backoff
- cancellation safety
- recovery after interruption
- temp/staging cleanup guarantees

Acceptance:

- [ ] Tests cover at least one interruption or retry path
- [ ] Retry thresholds are named constants with documented rationale
- [ ] Recovery behavior is explicit, not implied

### [P1] [done] Clean the remaining warning budget

Why:

- The build and test gates are green, and `cargo check --workspace` is now warning-free.
- CLI bins now import shared library state instead of path-including `state.rs`, and the
  Real-Debrid dead-field warnings are gone.

Acceptance:

- [x] `cargo check --workspace` is warning-free
- [x] Real-Debrid dead fields resolved
- [x] CLI bin warnings caused by `state.rs` inclusion removed

### [P1] [done] Add provider health awareness to the director waterfall

Why:

- The director now tracks provider health, emits health events, and skips known-down providers
  while keeping unknown or stale providers eligible for normal execution.

Acceptance:

- [x] Health state is tracked per provider with a timestamp
- [x] The waterfall can skip known-down providers
- [x] Health changes are visible to the UI or logs

### [P1] [todo] Raise packaging and clean-machine confidence

Why:

- "Builds in this workspace" is not the same as "ready to ship."

Acceptance:

- [ ] Install/build steps documented for a clean environment
- [ ] Gaps and assumptions recorded
- [ ] Release checklist updated

### [P1] [todo] Resolve `downloader/` vs `director/providers/` overlap

Why:

- Two partially parallel implementation paths exist for slskd, usenet, and others.
  `director/providers/` is the active path. `downloader/` contains older implementations.
  This creates confusion about which code is canonical.

Acceptance:

- [ ] Decision recorded: keep or remove `downloader/` module
- [ ] Dead code removed or clearly marked as historical
- [ ] Module status in PROJECT_INDEX.md updated

### [P1] [todo] Formalize performance baseline and regression budget

Why:

- The repo has qualitative confidence, but no strict performance contract yet.

Acceptance:

- [ ] Core commands benchmarked or timed (scan, organize, validation)
- [ ] Baselines recorded in TELEMETRY.md
- [ ] Regression thresholds documented

### [P1] [done] Deepen active-runtime provenance persistence

Why:

- The runtime now persists request signatures, full candidate sets, normalized provider search outcomes,
  and durable negative-result memory instead of only terminal result blobs.

Acceptance:

- [x] Candidate-set and candidate-item tables exist in the active runtime DB
- [x] Provider search outcomes and provider-negative memory persist on normalized request signatures
- [x] Terminal save path is transactional and covered by tests

---

## P2

### [P2] [todo] Resolve `Album.id` stability

Why:

- `get_albums()` uses `ROW_NUMBER() OVER (...)` to synthesize an ID rather than a real
  database primary key. Album IDs are not stable across queries if data changes.
  Any code that caches or compares album IDs by value is fragile.

Options:

- Add a real `albums` table with persistent primary keys
- Or audit all call sites and confirm no code assumes stable Album IDs between queries

Acceptance:

- [ ] Decision recorded in DECISIONS.md
- [ ] Either a stable ID exists, or all callers are confirmed safe without one

### [P2] [todo] Implement `MetadataRepairOnly` acquisition strategy

Why:

- The `MetadataRepairOnly` variant of `AcquisitionStrategy` is explicitly stubbed in
  `director/engine.rs` with a note that it is "intentionally stubbed in phase 1."
  Phase 1 is no longer new.

Acceptance:

- [ ] Implemented and tested, or explicitly removed from the strategy enum

### [P2] [todo] Document and test long-session desktop behavior

Why:

- Media apps earn trust through stability over time, not just one clean smoke run.

Acceptance:

- [ ] Soak-test procedure documented
- [ ] Known leaks, stalls, or recovery pain points recorded if found

### [P2] [todo] Tighten metadata and enrichment operating story

Why:

- Metadata logic exists, but runtime ownership and lifecycle are still less explicit
  than core library flows.

Acceptance:

- [ ] Current enrichment behavior documented
- [ ] Future integration plan recorded without overstating readiness

---

## P3

### [P3] [todo] Improve artist deep-link from library page

Currently the Artists tab in the library page navigates to `/artists` (the full artists page)
but does not deep-link into a specific artist. The `/artists` page itself supports full
drill-down. This is a navigation convenience improvement, not a broken feature.

### [P3] [todo] Add richer provider health and troubleshooting views in UI

### [P3] [todo] Revisit broader release automation once packaging proof is stable

---

## Completed / Fixed

### [done] Merge duplicate decode loops in player

`decode_loop` and `decode_loop_seek` in `src/player/mod.rs` were ~200 lines of duplicated code.
Merged into one `decode_loop(... seek_to: Option<f64>)` function. Both call sites updated.

### [done] Fix silent command drop in Player::send()

`Player::send()` used `let _ = self.cmd_tx.try_send(cmd)` — silently dropping commands when
the channel was full. Now emits `tracing::warn!` on failure so drops are visible in logs.

### [done] Wrap replace_spotify_album_history in a transaction

The function deleted all history rows then re-inserted in a loop with no transaction.
A failure mid-way would leave the table partially empty with no recovery path. Fixed.

### [done] Wrap prune_missing_tracks deletes in a transaction

Individual deletes were issued without a transaction. Batched the missing track IDs and wrapped
the delete loop in a single `BEGIN IMMEDIATE ... COMMIT` transaction.

### [done] Replace hand-rolled TOML parser with the toml crate

`load_streamrip_config` in `src-tauri/src/state.rs` used a line-scanner that could not handle
nested sections, multi-line values, or inline comments. Replaced with `toml::Table` parsing.
Added `toml = "0.8"` to `src-tauri/Cargo.toml`. The YAML parser for slskd config (a simple
`key: value` format) was left as a line scanner — it handles that format correctly.

### [done] Fix artist navigation in library page Artists tab

Artist rows used `<a href="/artists">` — a static link that navigated to the artists list but
carried no information about which artist was clicked. Replaced with `on:click={() => goto('/artists')}`
using `$app/navigation`. The `/artists` route has full drill-down; this navigates correctly.

---

## Operating Notes For Agents

When you pick up a task:

1. Update status from `todo` to `in_progress` if you are actively working it.
2. Keep the task scoped.
3. Add linked file paths or commands if you discover the task is narrower than written.
4. Move it to `review` only after verification.
5. Mark `done` only after code and docs both reflect reality.

If you notice a new problem but are not fixing it now, add it here with enough context for
the next agent to act without rediscovery.
