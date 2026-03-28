# The Agent Codex
## How To Operate On The Cassette Project

If you are an agent touching this codebase, this is your operating manual. Read it first. Follow it unless a higher-priority instruction conflicts, and document the reason when you diverge.

Canonical docs live under `docs/`. If another instruction says "read TODO.md" or similar, resolve that to `docs/TODO.md` in this repo.

---

## Core Principles

### 1. Auditability Over Cleverness

Every meaningful system action should be reconstructible later.

- Prefer explicit operation events over silent branches.
- Do not mutate files or operational state without enough metadata to explain what happened.
- If a decision matters for trust or debugging, it probably deserves a log record.

### 2. Determinism Over Convenience

Given the same input and configuration, Cassette should behave predictably.

- Use stable ordering and deterministic tie-breakers.
- Name thresholds and explain them.
- Avoid "whatever comes first" logic in matching, scoring, or reconciliation.

### 3. Reversibility Before Mutation

Cassette touches real files.

- Prefer staged or copy-verify-delete flows over direct destructive moves.
- Assume interruption can happen between any two steps.
- Keep rollback and cleanup behavior obvious.

### 4. Explicit Over Implicit

Future agents should not need tribal knowledge to understand a change.

- Use names that describe intent.
- Explain why a rule exists, not just what the code is doing.
- Keep module boundaries visible.

### 5. Defensive Over Optimistic

Assume:

- providers fail
- files are malformed
- metadata is incomplete
- users interrupt work
- the database may contain partial state after an aborted operation

Build for recovery, not just for the happy path.

---

## Startup Workflow

Before substantial work:

1. Read [PROJECT_INDEX.md](PROJECT_INDEX.md)
2. Read [PROJECT_STATE.md](PROJECT_STATE.md)
3. Read [AGENT_BRIEFING.md](AGENT_BRIEFING.md)
4. Read [TODO.md](TODO.md)
5. Read [DECISIONS.md](DECISIONS.md) for relevant rationale
6. Review [PATTERNS.md](PATTERNS.md)
7. Run baseline verification if the task is not trivial

Baseline commands:

```bash
cargo check
cargo test
cd ui && npm run build
# Windows: .\scripts\smoke_desktop.ps1
```

These commands are the baseline verification surface, not a promise that the workspace is currently green. Check `PROJECT_STATE.md` or re-run them when the task depends on current pass/fail status.

---

## Task Execution

### Phase 1: Understand

Before editing:

- read the full task
- identify the affected layer: `ui/`, `src-tauri/`, or `crates/cassette-core/`
- look for existing patterns nearby
- inspect related tests first

### Phase 2: Plan

Write a short plan to cover:

- what changes
- what could break
- how you will verify
- what assumptions you are making

### Phase 3: Implement

- keep scope tight
- preserve existing conventions unless there is a clear reason to improve them
- do not slip unrelated refactors into reliability work
- prefer small, auditable diffs

### Phase 4: Verify

Run the narrowest meaningful checks:

- Rust domain changes: targeted tests, then broader tests if needed
- Tauri wiring changes: `cargo check`
- UI changes: `npm run build` in `ui/`
- operational flow changes: smoke and validation commands where practical

### Phase 5: Document

Update docs whenever:

- architecture changes
- operating assumptions change
- priorities change
- a known gap is resolved or newly discovered

---

## Anti-Patterns

### Silent Failure

Bad:

- swallowing errors
- returning defaults that hide broken behavior
- skipping logs on recoverable failure

Preferred:

- typed errors
- contextual logging
- enough persisted state to retry safely

### Magic Thresholds

Bad:

- unnamed confidence floors
- unexplained retry counts
- hidden timeout behavior

Preferred:

- named constants
- brief rationale near the rule

### Scope Creep

Bad:

- fixing unrelated issues "while you are here"
- mixing broad cleanup with a production-hardening fix

Preferred:

- record adjacent work in [TODO.md](TODO.md)
- ship the current task cleanly

### Stale Documentation

Bad:

- changing runtime behavior without updating canonical docs
- copying historical plan text into present-tense status docs without checking reality

Preferred:

- state observable facts
- label estimates as estimates
- keep future work in TODOs, not in current-status sections

---

## Code Style And Conventions

Rust expectations:

- prefer typed errors with `thiserror` where it improves clarity
- keep async boundaries clear
- favor deterministic ordering
- name thresholds and retry policies
- add tests for risky or failure-heavy paths
- do not suppress errors with `let _ = ...` when the failure is actionable - log it

SQL expectations:

- use parameterized queries with `?` placeholders
- wrap multi-step mutations in a transaction
- do not make schema changes without a migration story

---

## Testing Expectations

Every risky change should answer:

- What is the happy path?
- What is the failure path?
- What happens after interruption or partial state?
- Is behavior deterministic across repeat runs?

Common verification commands:

```bash
cargo test
cargo run -p cassette-core --bin cassette -- validate --help
```

For operational work, prefer sandbox or test-mode validation before any production-mode path.

---

## Red Flags

Stop and realign if your change involves:

- schema changes without a migration story
- destructive file operations without rollback logic
- new external dependencies without justification
- provider changes without failure-path coverage
- reconciliation changes without determinism checks
- undocumented config additions

---

## Handoff Rules

Before handing work off:

- update [TODO.md](TODO.md) if status or priority changed
- update [DECISIONS.md](DECISIONS.md) if you made an architectural choice
- update [TELEMETRY.md](TELEMETRY.md) if confidence or baselines changed
- say clearly what was verified and what was not

The next agent should not need your memory to continue.

---

## When In Doubt

Bias toward:

- smaller diff
- stronger audit trail
- more explicit naming
- better recovery behavior
- current reality over inherited assumptions

---

**Last Updated**: 2026-03-27
