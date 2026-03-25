# Cassette Patterns

This file records the code, naming, testing, and documentation patterns that should feel normal in Cassette.

**Last Updated**: 2026-03-25

---

## Naming Patterns

### Rust Modules

- use `snake_case`
- prefer responsibility-revealing names over abstract names

Examples:

- `library`
- `gatekeeper`
- `orchestrator`
- `validation`

### Functions

Prefer intent-revealing names:

- `run_full_validation`
- `get_operation_summary`
- `start_album_downloads`

Avoid vague names like:

- `process`
- `handle`
- `do_work`

### Constants

- use `SCREAMING_SNAKE_CASE`
- name thresholds by meaning, not just units

---

## Rust Patterns

### Typed Errors

Prefer explicit error enums or contextual `anyhow` usage where appropriate. Do not hide real failure reasons.

### Deterministic Ordering

If ordering or candidate selection matters:

- define the rule
- keep it stable
- test it

### Recovery-Friendly Flows

When file or provider work spans multiple steps:

- persist enough state to resume
- clean temp paths deliberately
- make interruption behavior testable

### Narrow Responsibility

Keep responsibilities visible:

- UI state in UI and Tauri layers
- domain logic in `cassette-core`
- persistence and operational concerns close to `library`

---

## SQL And Database Patterns

- keep schema intent explicit
- prefer queries that are easy to reason about during debugging
- preserve lineage and operation visibility wherever practical
- do not make schema changes without a migration story

---

## Testing Patterns

Match test scope to change scope:

- pure logic change: unit tests
- orchestration change: integration-oriented coverage
- command-surface change: compile and wire checks
- UI change: `npm run build`

High-value test themes:

- interruption and recovery
- provider failures
- filesystem edge cases
- deterministic reconciliation
- audit and logging completeness

---

## Documentation Patterns

When behavior changes:

- update the nearest canonical doc, not just code comments
- prefer factual language over aspirational language
- distinguish between `implemented`, `verified`, and `planned`

Use status words consistently:

- `implemented`: code exists
- `verified`: behavior was recently tested or observed
- `planned`: not runtime truth yet

---

## UI Patterns

Current primary navigation is centered on:

- Library
- Downloads
- Settings

When updating UI:

- preserve primary-flow clarity
- keep advanced tools accessible without bloating the main shell
- avoid interactions the desktop shell cannot support cleanly

---

## Operational Patterns

For long-running or stateful operations:

- create clear start and finish boundaries
- surface progress where possible
- prefer resumable work units
- never assume network stability

For file mutation:

- validate before admitting
- quarantine with a clear reason when rejecting
- avoid irreversible shortcuts

---

## Anti-Patterns

- broad "cleanup" commits with no clear outcome
- silent retries with no visibility
- stale docs after behavior changes
- new dependencies without recording why
- unrelated refactors folded into production-hardening work
