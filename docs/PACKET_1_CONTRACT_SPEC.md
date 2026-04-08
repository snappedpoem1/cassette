# Packet 1 Contract Spec

Last updated: 2026-04-07
Covers: `GAP-A01`, `GAP-A02`, `GAP-B01`
Status: active implementation spec

---

## Purpose

Define executable contract details for Packet 1 lanes:

- release-group planner rationale lane (`GAP-A01`)
- canonical edition object (`GAP-A02`)
- include/exclude request grammar (`GAP-B01`)

This spec is the source for implementation and regression tests in Packet 1.

---

## A01: Release-Group Planner Rationale Contract

### Required rationale fields

Planner rationale output must include:

- `musicbrainz_release_group_id`
- `identity_confidence` (enum: `high`, `medium`, `low`)
- `edition_policy` (requested policy at plan-time)
- `edition_match_outcome` (enum: `match`, `mismatch`, `insufficient_evidence`)
- `candidate_count_considered`

### Contract rule

- If `musicbrainz_release_group_id` exists in request envelope, rationale must echo it in planner output.
- Missing field is a contract failure.

### Regression tests

- Snapshot test for planner rationale payload containing `musicbrainz_release_group_id`.
- Negative test for request envelope with missing release-group ID where rationale must explicitly state `insufficient_evidence`.

---

## A02: Canonical Edition Object Contract

### Edition object shape

```json
{
  "edition": {
    "policy": "prefer_standard|exclude_live|exclude_deluxe|allow_remaster|strict",
    "markers": {
      "is_live": false,
      "is_deluxe": false,
      "is_remaster": false,
      "country": null,
      "label": null,
      "catalog_number": null
    },
    "evidence": {
      "source": "musicbrainz|discogs|provider_payload|inferred",
      "confidence": "high|medium|low"
    }
  }
}
```

### Contract rule

- Planner request persistence and candidate persistence must round-trip the `edition` object without losing keys.
- Unknown values are allowed only as `null`, never by dropping fields.

### Regression tests

- Round-trip test: request -> persistence -> read API retains all edition keys.
- Equality test: persisted edition object must be structurally equal to submitted object when no enrichment override happens.

---

## B01: Include/Exclude Request Grammar Contract

### Grammar (JSON payload)

```json
{
  "scope": "selected_albums",
  "targets": {
    "include": [
      { "artist": "...", "album": "...", "release_group_id": null }
    ],
    "exclude": [
      { "artist": "...", "album": "...", "release_group_id": null }
    ]
  },
  "edition_policy": "prefer_standard"
}
```

### Validation rules

- `include` must contain at least 1 entry.
- `exclude` may be empty.
- Same normalized identity cannot exist in both include and exclude.
- Ambiguous entries (missing both artist and release_group_id) are invalid.

### Error contract

- Return explicit `validation_error` with a stable reason code:
  - `include_empty`
  - `include_exclude_conflict`
  - `ambiguous_album_identity`

### Regression tests

- Accept valid include-only and include+exclude payloads.
- Reject include/exclude conflicts with reason code.
- Reject ambiguous identities with reason code.

---

## Touchpoint Targets

- `crates/cassette-core/src/acquisition.rs`
- `crates/cassette-core/src/models/*`
- `crates/cassette-core/src/librarian/db/*`
- `src-tauri/src/commands/downloads.rs`
- `ui/src/routes/downloads/*`

---

## Verification Commands

```powershell
cargo check --workspace
cargo test --workspace
Set-Location ui; npm run build; Set-Location ..
```
