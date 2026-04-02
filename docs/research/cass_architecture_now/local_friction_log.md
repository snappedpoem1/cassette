# Local Friction Log

Generated: 2026-04-02

This log compares the researched architecture against the live Cassette databases and the actual `A:\music` library.

## Observed Local Reality

- Live runtime DB: `%APPDATA%\\dev.cassette.app\\cassette.db`
- Live sidecar DB: `%APPDATA%\\dev.cassette.app\\cassette_librarian.db`
- Library root: `A:\music`
- Files on disk under `A:\music`: 47,207
- Active runtime `tracks` rows: 45,739
- Sidecar `local_files` rows: 43,501

## Friction 1: The Identity Spine Exists On Paper, Not In Practice

- `tracks` has 45,739 rows.
- `isrc` populated: 0
- `musicbrainz_recording_id` populated: 0
- `musicbrainz_release_id` populated: 0
- `canonical_artist_id` populated: 0
- `canonical_release_id` populated: 0
- `canonical_artists` rows: 0
- `canonical_releases` rows: 0
- `acquisition_requests` rows: 0

What this means:

- Cassette has schema ambition for sovereignty but is still running mostly on text and path truth.
- MusicBrainz is active in code, yet the resulting identity never lands in the main runtime graph.
- Every provider bridge is therefore weaker than it should be.

What you are doing wrong:

- You are paying the complexity cost of canonical columns without harvesting any canonical benefit.
- The app can acquire and organize files, but it cannot yet govern them by durable identity.

## Friction 2: The Sidecar And Runtime Disagree About The Library Shape

- App paths missing on disk: 0
- Sidecar paths missing on disk: 23,383
- App-only paths vs sidecar: 25,694
- Sidecar-only paths vs app: 23,456

Observed path drift example:

- App DB path: `A:\music\$NOT\- TRAGEDY + (2020)\01 - YOU KNOW (INTRO).flac`
- Sidecar path: `A:\music\$NOT\2020 - - TRAGEDY +\01 - YOU KNOW (INTRO).flac`

Observed pattern:

- The app DB reflects post-organize canonical paths.
- The sidecar still carries large volumes of pre-organize or alternate-layout paths.

What this means:

- The coordinator can reason over stale file truth while the player/runtime points at current file truth.
- `delta_queue`, reconciliation, and organizer proof are more fragile than the docs suggest because the two databases are not converging fast enough after path mutation.

What you are doing wrong:

- You are letting the sidecar act as a durable control plane without forcing post-organize path convergence back into it.
- You have created a sovereign split-brain: one local truth for playback, another for reconciliation.

## Friction 3: The Sidecar Knows More Than The Runtime, But The Runtime Governs

Sidecar `local_files`:

- `quality_tier` populated: 43,428
- `content_hash` populated: 8,692
- linked `track_id`: 40,170
- unresolved links: 3,331
- integrity status:
  - `readable`: 40,170
  - `partial_metadata`: 3,258
  - `unreadable`: 73

Runtime `tracks`:

- `quality_tier` populated: 0
- `content_hash` populated: 0

What this means:

- The richer ingestion truth is stranded in the sidecar.
- The active runtime table is still too thin to support sovereign scoring, duplicate confidence, or AI-grade file certainty.

What you are doing wrong:

- You are validating like a sovereign system and persisting like a thin player database.
- The best evidence is not being promoted into the table the UI and core runtime actually trust.

## Friction 4: Track Number Recovery Is Still A Real Local Wound

- `tracks` with `track_number` null or zero: 1,859
- Files on disk named `00 - ...`: 390
- Files with current `Singles` directories on disk: 385

Observed examples:

- `A:\music\(Hed) P.E\Bartenders\00 - Bartenders.flac`
- `A:\music\2 Chainz\Welcome 2 Collegrove\00 - Welcome 2 Collegrove.flac`
- `A:\music\A Day to Remember\Common Courtesy\00 - Common Courtesy.flac`

What this means:

- The organizer safety proof was real and necessary.
- The library still contains enough unresolved numbering damage that acquisition and metadata repair should be identity-led, not filename-led.

What you are doing wrong:

- You are still too dependent on path conventions for truth recovery.
- AcoustID/Chromaprint should be doing this repair work, not folder names and luck.

## Friction 5: Desired State Is Too Weak For The Pipeline You Want

Sidecar `desired_tracks` right now:

- rows: 4
- source name: all `manual`
- track numbers: empty
- ISRC: empty

At the same time:

- `spotify_album_history` rows: 12,346
- marked `in_library`: 2,383

What this means:

- The intent layer is large, but the active durable work contract is currently starved and underspecified.
- Cassette's queue can move music, but it is not yet being fed a fully expressive identity-rich wanted state.

What you are doing wrong:

- You are carrying a strong backlog source without a strong request contract.
- The system still asks providers for "find me this song" when it should be asking for "resolve this known recording/release identity or the closest proven alias."

## Friction 6: Failure Provenance Is Better Than Before, But Still Too Thin At The End

- `director_task_history` rows: 4,752
- `Finalized`: 4,531
- `Failed`: 221
- rows with blank provider in terminal history: 221
- provider memory top failure classes:
  - `no_result`: 3,521
  - `search_error`: 3,251
  - `provider_busy`: 3,141

What this means:

- Candidate/provenance persistence is materially improved.
- But final failed history still collapses too much into "failed" without a clean provider-attribution trail in the terminal row.

What you are doing wrong:

- You are close to a full audit spine, but still dropping the last responsible provider identity on failed terminal records.

## Friction 7: Important Context Services Are Configured In Theory, Not In Runtime Truth

Missing settings keys in the live runtime DB:

- `lastfm_api_key`
- `discogs_token`
- `genius_token`

What this means:

- Last.fm, Discogs, and Genius are not part of the current strong path even where the settings model implies they could be.
- Discogs and Genius especially are still conceptual surfaces, not operational assets.

What you are doing wrong:

- You are exposing strategic surfaces in config before the runtime owns a real plan for them.

## Friction 8: Folder Reality Is Still Chaotic Enough To Punish Naive Logic

Top-level library examples:

- `$NOT`
- `$uicideboy$`
- `(Hed) P.E`
- `+44`
- `1000volts, Lit Lords, Redman, Jayceeoh`
- `2002-02-13 - Geogaddi [ALBUM] (FLAC)`

What this means:

- Any logic that assumes neat artist-folder semantics will keep lying.
- Cassette needs explicit canonical entities, alias tables, and source evidence because the filesystem is not normalized truth.

## Immediate Improvement Agenda

1. Promote MBIDs, canonical IDs, quality tier, and content hash into the active runtime truth, not just the sidecar.
2. Add a post-organize sidecar reconciliation pass that rewrites `local_files.file_path` against the new canonical paths.
3. Make fingerprinting real. `acoustid_fingerprint` should stop being a future-tense column.
4. Upgrade the request contract from `artist/title/album` text to canonical-or-alias identity plus provenance and exclusion memory.
5. Separate provider search, resolver execution, validation, and final import into explicitly queryable phases everywhere.
6. Fix failed terminal history so the last provider and failure class always survive to the final row.

## Bottom Line

Your current logic is strongest at acquisition resilience and safer organization. It is weakest exactly where sovereignty matters most: durable identity, cross-database convergence, and turning local evidence into governing truth.
