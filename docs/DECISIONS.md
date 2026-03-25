# Cassette Decisions

This file records why the codebase is shaped the way it is so future agents inherit rationale, not just files.

**Last Updated**: 2026-03-25

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
