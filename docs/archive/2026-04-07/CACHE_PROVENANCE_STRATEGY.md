# Cache And Provenance Strategy

Last audited: 2026-03-27

## Current Truth

The active app DB stores final task history, but not the memory needed for durable acquisition planning.

### What the active runtime stores now

| Surface | Current state |
| --- | --- |
| Final task result | Stored in `director_task_history` |
| Request signatures | Stored in `director_pending_tasks`, `director_task_history`, and candidate/provider tables |
| Track library | Stored in `tracks` |
| User/provider settings | Stored in `settings` |
| Spotify album import summaries | Stored in `spotify_album_history` |
| Query signatures | Present for director track tasks via `request_signature` |
| Candidate sets | Stored in `director_candidate_sets` and `director_candidate_items` |
| Negative search results | Stored in `director_provider_searches` and `director_provider_memory` |
| Canonical source IDs | Missing |
| User exclusion rules | Missing |
| Provider freshness TTLs | Missing |

## What should be cached

### 1. Identity Cache

Persist and reuse:

- normalized artist key
- MusicBrainz artist MBID
- normalized release-group key
- MusicBrainz release-group MBID
- MusicBrainz release MBID
- source-specific IDs:
  - Spotify artist/album/track IDs
  - Qobuz album/track IDs
  - Deezer album/track IDs
  - Discogs release/master IDs
  - local-file/content-hash mappings

These should be effectively long-lived and only invalidated by explicit metadata refresh or conflict correction.

### 2. Query Cache

Persist per normalized request signature:

- request type: track / album / artist / discography / selected-albums
- normalized artist/title/album keys
- exclusion policy snapshot
- source(s) queried
- timestamp
- resulting canonical IDs discovered
- candidate-set cache key

Suggested TTLs:

- canonical metadata lookups: 30 days
- provider search results: 1-7 days depending on provider volatility
- negative results: 6-24 hours

### 3. Candidate Cache

Persist full candidate sets, not just winners:

- provider ID
- provider candidate ID
- candidate text and source metadata
- quality hints
- validation summary
- score breakdown
- rejection reason
- whether user reviewed or overrode it

This is required for:

- "show me candidates before acquisition"
- "reuse prior metadata/search results"
- "prove why this result was chosen"

### 4. Attempt and Failure Memory

Persist:

- provider attempted
- request signature
- failure class: auth / no result / validation fail / timeout / rate limited
- timestamp
- retryability flag
- backoff-until timestamp

This should stop the app from hammering the same failing provider repeatedly inside the same planning window.

### 5. User Preference Memory

Persist:

- excluded album titles
- excluded edition qualifiers like `live`, `remaster`, `deluxe`, `commentary`
- preferred providers by request class
- minimum quality preferences
- artist-specific overrides
- prior chosen edition for the same release-group

### 6. Provenance Memory

Persist:

- canonical release chosen and why
- all candidate scores considered
- final provider selected
- validation report snapshot
- tagging/enrichment operations applied
- file hashes and codec properties
- final local path

## What must never be re-queried unnecessarily

These should be treated as authoritative until invalidated:

- canonical MBID mappings for already confirmed artists/releases
- local file hashes and codec properties unless file mtime/hash changes
- prior chosen release for a confirmed user decision
- negative provider result inside its TTL window
- provider auth/session material while valid in-memory

## Minimal schema additions

If the active app DB remains the runtime owner, add at least:

| Table | Purpose |
| --- | --- |
| `entity_identity_map` | canonical artist/release-group/release/recording IDs plus per-source IDs |
| `query_cache` | normalized request signature, timestamps, freshness, canonical result |
| `candidate_sets` | one row per planning run |
| `candidate_items` | all candidates with scores, validation, and rejection reasons |
| `provider_attempts` | durable attempt/failure memory with retry semantics |
| `user_selection_rules` | explicit include/exclude and provider preference memory |
| `release_decisions` | final chosen release/edition per request or per release-group |
| `file_fingerprints` | content hash, codec properties, ownership mapping |

## Better Alternative

Do not invent a second new schema if avoidable. The repo already has richer primitives in the `librarian` and `library` schemas. The best move is to converge the active app runtime onto those richer memory surfaces rather than continuing to keep them separate.
