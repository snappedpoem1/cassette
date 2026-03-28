# Cassette Decisions

This file records why the codebase is shaped the way it is so future agents inherit rationale, not just files.

**Last Updated**: 2026-03-28

---

## Decision 1: Rust Workspace With A Shared Core Crate

**Status**: approved  
**Rationale**:

- Keeps business logic out of the Tauri shell
- Allows validation and CLI tooling to reuse the same domain code
- Makes runtime layers easier to reason about

**Tradeoffs**:

- More workspace structure
- Slightly heavier cross-crate navigation

**Revisit Condition**:

- If the split creates more duplication than clarity

---

## Decision 2: Local-First SQLite Storage

**Status**: approved  
**Rationale**:

- Fits a private, personal-library use case
- Supports offline-capable operation
- Avoids external infrastructure

**Tradeoffs**:

- Single-machine assumptions
- No distributed coordination
- Backup discipline matters more

**Revisit Condition**:

- If multi-machine sync becomes a real requirement

---

## Decision 3: Tauri Desktop Shell Plus SvelteKit UI

**Status**: approved  
**Rationale**:

- Good fit for desktop affordances plus rich UI
- Keeps filesystem-aware behavior in the shell while retaining a flexible frontend

**Tradeoffs**:

- Two runtime layers to debug
- Frontend and shell verification both matter

**Revisit Condition**:

- If packaging or plugin constraints become a recurring bottleneck

---

## Decision 4: Pipeline-Oriented Library Domain

**Status**: approved  
**Rationale**:

- Librarian, Custodian, Orchestrator, Director, and Gatekeeper create understandable responsibility boundaries
- Phase-oriented logic supports auditability and recovery

**Tradeoffs**:

- More orchestration glue
- More docs needed to keep boundaries clear

**Revisit Condition**:

- If runtime behavior drifts so far that the boundaries stop being useful

---

## Decision 5: Safety Through Staging, Quarantine, And Validation

**Status**: approved  
**Rationale**:

- Real files deserve defensive handling
- Staging and quarantine support reversible flows
- Validation gates reduce silent corruption

**Tradeoffs**:

- More disk I/O
- More directories and state transitions

**Revisit Condition**:

- Never lightly; any relaxation needs explicit justification

---

## Decision 6: Auditability As A First-Class Requirement

**Status**: approved  
**Rationale**:

- Cassette's value is not just automation; it is provable automation
- Logging and lineage make trust and debugging possible

**Tradeoffs**:

- Operational overhead
- More schema and verification complexity

**Revisit Condition**:

- Never as a casual speed optimization

---

## Decision 7: Recovery And Locking Inside The Core Domain

**Status**: approved  
**Rationale**:

- Operations, locking, recovery, and observability belong close to domain state
- The `library` module acts as the operational spine

**Tradeoffs**:

- More complexity inside the core crate
- Requires disciplined docs to stay understandable

**Revisit Condition**:

- If the manager layer becomes too broad or tightly coupled

---

## Decision 8: Provider Diversity Over Single-Source Dependence

**Status**: approved  
**Rationale**:

- Acquisition is more resilient with multiple source lanes
- Different providers fail in different ways

**Tradeoffs**:

- Broader maintenance surface
- More edge cases
- Proof is machine- and config-dependent

**Revisit Condition**:

- If a provider becomes too brittle or costly to maintain

---

## Decision 9: Documentation Must Reflect Observed Runtime Truth

**Status**: approved  
**Rationale**:

- Historical plan text drifts quickly in an active repo
- Agents need present-tense facts, not stale aspirations

**Tradeoffs**:

- Documentation upkeep is part of engineering work

**Revisit Condition**:

- Never; documentation drift is more expensive than maintenance

---

## Decision 10: Use Real Parsers For External Config Files

**Status**: approved (applied 2026-03-25)
**Rationale**:

- `load_streamrip_config` previously used a hand-rolled line scanner that could not handle
  TOML section headers, multi-line values, or inline comments.
- The `toml` crate is the correct tool. It was added to `src-tauri/Cargo.toml`.
- The YAML scanner for `slskd.yml` was left in place — the format is a flat `key: value` file
  that the scanner handles correctly. If the format grows more complex, replace with `serde_yaml`.

**Tradeoffs**:

- One additional dependency (`toml = "0.8"`)
- Parsing errors now produce a logged warning instead of silently producing empty config

**Revisit Condition**:

- If slskd config format becomes more complex, switch its parser too

---

## Decision 11: Operation Start Events Are Part Of The Audit Trail

**Status**: approved (confirmed 2026-03-27)
**Rationale**:

- `start_operation()` intentionally emits `operation_started` into `operation_events`
- That means an operation with one explicit business event should produce at least two audit events
- The library test was updated to reflect the intended audit trail instead of treating the start event as accidental duplication

**Tradeoffs**:

- Event counts are slightly higher than a naive "only explicit events" model
- Tests must assert lifecycle semantics, not raw convenience counts

**Revisit Condition**:

- Only if the audit model is intentionally redesigned

---

## Decision 12: Startup Recovery Must Prefer Newer Terminal History Over Stale Pending Rows

**Status**: approved (applied 2026-03-28)
**Rationale**:

- `director_pending_tasks` and `director_task_history` can both exist briefly if the app crashes
  after a terminal result is persisted but before the pending row is deleted.
- Recovery cannot just blacklist a few dispositions by task ID; stable task keys are reused in some
  flows, so older failed history must not suppress a newer retry.
- Startup recovery now compares pending-row timestamps against the latest terminal history per task
  and only resubmits rows that are still newer than any terminal result.

**Tradeoffs**:

- Slightly more recovery logic at startup
- Recovery correctness now depends on timestamp ordering staying queryable and deterministic

**Revisit Condition**:

- If the runtime moves from task-level replay to deeper phase checkpointing

---

## Decision 13: Terminal Director History Must Preserve The Original Request Payload

**Status**: approved (applied 2026-03-28)
**Rationale**:

- `director_task_history` previously preserved result/provenance state but could lose the original
  request intent for failed or cancelled tasks once the pending row was deleted.
- The active runtime now copies the original `TrackTask` JSON and strategy into terminal history
  so request intent remains queryable after completion.
- This is not full candidate/request memory yet, but it gives the current runtime a stronger and
  more durable audit spine without waiting for full schema convergence.

**Tradeoffs**:

- Slightly larger terminal history rows
- The runtime still lacks durable full candidate-set persistence and richer negative-result memory

**Revisit Condition**:

- If the active runtime moves to a richer normalized request/candidate schema

---

## Decision 14: Active Runtime Provenance Should Converge On A Request-Signature Spine

**Status**: approved (applied 2026-03-28)
**Rationale**:

- Persisting only `director_task_history.result_json` was not enough to explain why a task failed,
  which candidates were rejected, or which providers should be treated as recently exhausted.
- The active runtime now carries a normalized `request_signature` across pending tasks, terminal history,
  candidate sets, provider searches, provider attempts, and provider-negative memory.
- This keeps the current Tauri runtime on a richer provenance path without waiting for a wholesale move onto
  the larger librarian/library schema.

**Tradeoffs**:

- More runtime tables and larger terminal writes
- Persistence is now materially better than reuse; candidate-review UX and TTL-driven query skipping still need wiring

**Revisit Condition**:

- If the active runtime fully converges onto the richer normalized reconciliation schema already present elsewhere in the repo

---

## Deferred Decisions

### Distributed / Multi-Machine Coordination

**Status**: deferred  
**Reason**:

- Current assumptions are local and single-machine

### Full Release Automation

**Status**: deferred  
**Reason**:

- Packaging confidence should come before automation polish

### Richer Background Metadata Integration

**Status**: deferred  
**Reason**:

- Important, but still secondary to auditability and runtime hardening

---

## Explicitly Rejected Patterns

- Silent error swallowing
- Destructive file mutation without rollback or verification
- Unnamed thresholds in reconciliation or provider logic
- Docs that claim work is complete when it is only planned
- Broad refactors hidden inside urgent bug-fix work

---

## How To Use This File

- Add new architectural decisions here.
- If you overturn an older decision, record why.
- If a TODO depends on a design choice, link it back here.
