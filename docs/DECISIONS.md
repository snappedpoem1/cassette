# Cassette Decisions

This file records why the codebase is shaped the way it is so future agents inherit rationale, not just files.

**Last Updated**: 2026-04-08

---

## Decision 0: Personal-Use Scope, Not Productization

**Status**: approved (confirmed 2026-04-06)  
**Rationale**:

- Cassette is a single-owner system built for personal workflows and trust.
- Project language that sounds like product release planning must be interpreted as personal readiness/reliability gates.
- This keeps implementation and documentation focused on utility, reversibility, auditability, and maintainability rather than commercialization narratives.

**Tradeoffs**:

- Some common engineering shorthand (for example, "shipping blocker") can read as product-centric unless clarified.
- Documentation discipline is required to keep this framing explicit.

**Revisit Condition**:

- Only if project ownership and intended usage model materially change.

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

- If lifecycle semantics change enough that start-event emission is no longer the right audit boundary

---

## Decision 12: `delta_queue` Is The Durable Acquisition Control Plane

**Status**: approved (applied 2026-03-30)  
**Rationale**:

- Librarian reconciliation already produces actionable queue rows with deterministic priority.
- SQLite-backed queue state survives crashes and supports idempotent retries without needing an in-memory bus.
- The new coordinator path (`engine_pipeline_cli`) can claim, process, release, and close work entirely against durable queue state.

**Tradeoffs**:

- More queue-state columns (`processed_at`, `claimed_at`, `claim_run_id`, `source_operation_id`)
- Slightly more coordination logic in the CLI/runtime layer

**Revisit Condition**:

---

## Decision 13: Trust Ledger v1 Derives Reason Codes From Existing Evidence Tables

**Status**: approved (applied 2026-04-06)  
**Rationale**:

- Cassette already persisted the core evidence needed for trust reconstruction across `acquisition_request_events`, `director_task_history`, candidate review rows, `operation_events`, and `gatekeeper_audit_log`.
- Adding a second trust-ledger storage table this early would duplicate truth and create drift risk.
- Deriving normalized reason codes at read time keeps Trust Ledger v1 reversible, auditable, and small enough to land without blocking Stage A momentum.

**Tradeoffs**:

- Reason codes are only as good as the normalization layer, so vocabulary drift must be tested.
- Some request-list views use lightweight derivation and therefore show a thinner evidence picture than full lineage views.

**Revisit Condition**:

- Revisit if Trust Ledger queries become too expensive, if planner/runtime/gatekeeper vocabularies diverge beyond easy normalization, or if Stage B dead-letter workflows need persisted trust snapshots for offline replay.

- If a stronger durable work bus supersedes queue-lease semantics while preserving auditability.

---

## Decision 13: Contract-First Music System Planning

**Status**: approved (confirmed 2026-04-06)  
**Rationale**:

- Feature-only roadmaps were insufficient to encode the true product intent.
- Cassette now uses two explicit planning contracts:
  - Operating Contract (autonomy, persistence, safe aggression, in-app service ownership, explainability)
  - Experience Contract (music-first calm shell, artist-first worldview, collection-intelligence center, non-intrusive depth)
- These contracts reduce scope drift and force UX/core cohesion.

**Tradeoffs**:

- Slightly more planning overhead before implementation.
- Requires docs and backlog discipline to map tasks to contract clauses.

**Revisit Condition**:

- If user priorities materially shift away from autonomy-first, artist-first, and music-first system behavior.

---

## Decision 14: Organizer Safety Is Enforced Before Live Mutation

**Status**: approved (applied 2026-03-30)  
**Rationale**:

- The runtime DB contained zeroed `track_number` values for files that were already named correctly on disk.
- Allowing `organize_cli --live` to blindly trust that state would mass-rename valid files to `00 - Title`.
- `tag_rescue_cli` now uses a staged recovery ladder (`embedded_tag`, `filename_prefix`, `album_pattern`) and, together with the live zero-track rename guard, preserves reversibility and prevents avoidable damage.

**Tradeoffs**:

- One more preflight/repair step before broad organize passes
- Live organize can now hard-fail instead of proceeding with suspect moves

**Revisit Condition**:

- Only after DB truth is proven stable enough that the guard becomes noise instead of safety

---

## Decision 15: `director/providers/` Is The Canonical Runtime Downloader Path

**Status**: approved (applied 2026-03-30)  
**Rationale**:

- The active runtime acquisition flow lives in Director, not the older `downloader/` path.
- Keeping that ambiguous makes hardening work land in the wrong place and confuses future agents.
- The legacy `downloader/` compatibility module and `ProviderBridge` adapter were retired in GAP-D03 (2026-04-07).

**Tradeoffs**:

- Validation needed a local adapter replacement before compatibility-module deletion
- Requires docs and future deletion/marking work

**Revisit Condition**:

- If legacy downloader code is revived intentionally as part of a new runtime design

---

## Decision 16: Sidecar Scan Progress Must Be Durable And Cheap To Resume

**Status**: approved (applied 2026-03-30)
**Rationale**:

- A live first-pass scan against the real library is too expensive to restart from zero after interruption.
- The sidecar DB now persists per-root `scan_checkpoints` plus `local_files.file_mtime_ms` so resume and delta-only scans can skip unchanged files deterministically.
- `engine_pipeline_cli` now treats `--resume` as a precise scan-mode shorthand instead of bypassing the librarian blindly.

**Tradeoffs**:

- More sidecar schema (`scan_checkpoints`, `file_mtime_ms`)
- Resume correctness depends on deterministic root/file ordering and checkpoint writes

**Revisit Condition**:

- Only if the control plane stops using a sidecar SQLite scan index

---

## Decision 17: Startup Recovery Must Prefer Newer Terminal History Over Stale Pending Rows

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

## Decision 18: Terminal Director History Must Preserve The Original Request Payload

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

## Decision 19: Active Runtime Provenance Should Converge On A Request-Signature Spine

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

## Decision 20: Useful Provider Evidence Is Retained Even When The Immediate Task Does Not Need It

**Status**: approved (applied 2026-04-02)
**Rationale**:

- If the Director asks for one narrow fact and a provider returns several more useful identity or candidate facts,
  discarding that surplus guarantees repeated re-discovery later.
- The active runtime now persists search evidence, candidate evidence, response snapshots, identity evidence, and
  source aliases as separate tables instead of treating `result_json` as the only durable memory.
- This keeps Cassette on the sovereignty path: evidence learned once becomes queryable later for review, reuse, and AI context.

**Tradeoffs**:

- Larger runtime tables
- More persistence work on terminal result saves
- Cache/retention pruning policy now matters

**Revisit Condition**:

- If evidence volume becomes materially expensive, tighten TTL/pruning only for the truly ephemeral cache rows rather than reverting to write-only memory

---

## Decision 21: Organizer Moves Must Converge App And Sidecar Path Truth Together

**Status**: approved (applied 2026-04-02)
**Rationale**:

- Updating only `tracks.path` after a live move left the sidecar `local_files.file_path` behind, which created a long-lived split-brain between the runtime library view and the control-plane scan index.
- Organizer flows now apply path updates to both databases in the same convergence pass and displace stale conflicting sidecar rows at the destination path.
- This keeps organize passes auditable and prevents fresh sidecar drift from being introduced by routine file moves.

**Tradeoffs**:

- Organizer code now depends on the sidecar DB being present
- Cross-database updates are still not truly atomic because SQLite transactions cannot span both files

**Revisit Condition**:

- If the runtime and sidecar ever converge onto one authoritative DB, collapse this bridge into a single transactional path

---

## Decision 22: Persisted Provider Memory Must Act As A Runtime Control Plane, Not A Write-Only Audit Trail

**Status**: approved (applied 2026-04-02)
**Rationale**:

- Cassette was already persisting negative-result memory and provider response snapshots, but the Director still behaved like every request was brand new.
- Fresh dead-end memory now short-circuits providers before network search, and fresh cached candidate payloads can hydrate the in-memory search cache for identical requests.
- This keeps the system deterministic under repetition: if Cassette already learned that a provider is a dead lane for a specific request signature, it should not keep paying the search cost to relearn it.

**Tradeoffs**:

- Reuse policy now depends on freshness windows and careful failure-class handling
- Bad cache hygiene could preserve stale provider knowledge longer than intended
- Director config now needs runtime DB awareness

**Revisit Condition**:

- If provider catalogs or search semantics become volatile enough that current freshness windows are too sticky, narrow the TTLs rather than removing persisted reuse entirely

---

## Decision 23: Fingerprint Evidence Must Accumulate In `local_files` Through Bounded Backfill

**Status**: approved (applied 2026-04-02)
**Rationale**:

- Acoustic identity is too expensive to keep discovering from scratch and too important to leave stranded inside Gatekeeper-only flows.
- `local_files` now persists `acoustid_fingerprint`, backfill attempt state, and the source file mtime used for the last fingerprint decision; Gatekeeper writes fingerprints during admission, and librarian sync backfills missing fingerprints in deterministic bounded parallel slices.
- This preserves sovereignty without turning every normal scan into a full-library fingerprint marathon.

**Tradeoffs**:

- Sync runs now have a small extra CPU/decode budget when backfill is enabled
- Backfill state management is more complex because unchanged failures must be suppressed without hiding legitimately rewritten files
- Full convergence still takes multiple bounded passes on large libraries

**Revisit Condition**:

- If a future canonical backfill worker supersedes this path, keep the same storage contract and move only the execution surface

---

## Decision 24: The Sidecar Owns Acquisition Request Lifecycle Before Any Physical DB Merge

**Status**: approved (applied 2026-04-02)
**Rationale**:

- Cassette already had the right execution spine in Director and the right durable control-plane primitives in the sidecar, but request intent was still being created ad hoc in the Tauri command layer.
- `cassette_librarian.db` now owns acquisition requests, request timeline events, and canonical planning identities (`canonical_artists`, `canonical_releases`, `canonical_recordings`) so request intent exists before execution starts.
- `cassette.db` remains the playback/runtime projection and terminal-result store for now; convergence means linking the two stores by `task_id`, `request_signature`, `desired_track_id`, and `source_operation_id`, not collapsing them blindly.

**Tradeoffs**:

- Request data is now split across two DBs during the convergence period
- Event listeners must keep the sidecar timeline in sync with Director progress and terminal outcomes
- Some legacy runtime tables (such as the older runtime `acquisition_requests`) are now transitional rather than canonical

**Revisit Condition**:

- When the runtime can deliberately cut over more read/write ownership without reintroducing split-brain behavior, not before

---

## Decision 25: Windows Workspace Tests Must Not Depend On The Tauri Lib Harness

**Status**: approved (applied 2026-04-02)
**Rationale**:

- The failing `cassette_lib-*` unit-test executable imported `comctl32!TaskDialogIndirect` but did not carry the desktop manifest that activates Common Controls v6, so Windows aborted the process with `STATUS_ENTRYPOINT_NOT_FOUND` before any Rust test code ran.
- The failure was in the GUI-linked harness startup path, not in the pure command/bootstrap logic we actually needed to verify.
- Pure `src-tauri` assertions now live in `src-tauri/tests/pure_logic.rs`, and the Tauri lib target is no longer part of `cargo test --workspace`.

**Tradeoffs**:

- A small slice of `src-tauri` logic had to be factored into pure helper modules (`now_playing`, `spotify_history`, `pending_recovery`, `runtime_bootstrap`)
- GUI-shell behavior still needs smoke or desktop-level verification rather than ordinary unit tests

**Revisit Condition**:

- If the repo gains a reliable manifest-aware Windows GUI test harness, the test split can be reconsidered without putting the workspace gate back on a flaky startup path

---

## Decision 26: Jackett Is The Torrent Search Owner; Real-Debrid Is The Resolver

**Status**: approved (applied 2026-04-03)
**Rationale**:

- The old Real-Debrid provider searched TPB/apibay directly. That confounded search ownership with resolve ownership and limited torrent coverage to a single indexer.
- `JackettProvider` searches all Jackett-configured indexers via Torznab (cat=3000) and returns magnet URIs. It uses the Real-Debrid API internally to resolve and download, exactly as the existing `RealDebridProvider` does. This separates the search concern from the resolve concern without splitting the Director provider trait.
- The `torrent_album_cli` already had Jackett search wired as a fallback; the Director waterfall now gets the same capability for per-track acquisition.
- `RealDebridProvider` is kept as a standalone provider using TPB/apibay so it continues to work for users without Jackett configured.

**Tradeoffs**:

- Requires both `JACKETT_URL` + `JACKETT_API_KEY` and `REAL_DEBRID_KEY` to activate.
- JackettProvider duplicates the RD client/poll/unrestrict/download logic from RealDebridProvider. This duplication is intentional for now — the two providers have different search paths and the shared logic is not complex enough to justify an extraction yet.

**Revisit Condition**:

- If a Jackett-native acquire path (NZB direct, not RD) becomes desirable, split the acquire into a separate `TorrentResolver` trait.

---

## Decision 27: Torrent CLI Failures Feed Back Into The Coordinator Via The Sidecar

**Status**: approved (applied 2026-04-03)
**Rationale**:

- `torrent_album_cli` failures (no torrent found) were silently logged and forgotten. Those albums then remained in the Spotify backlog with no automatic retry path.
- The sidecar already has the right primitives: `desired_tracks` + `delta_queue(missing_download)` is exactly how the coordinator discovers work.
- `--seed-sidecar` expands failed albums via MusicBrainz into per-track `desired_tracks` entries, then enqueues them as `missing_download`. The next `engine_pipeline_cli --resume` run claims and resolves them via Qobuz/Deezer/slskd — providers that are far more reliable for catalog content than TPB.

**Tradeoffs**:

- Requires a MusicBrainz lookup per failed album (1 req/sec rate limit respected).
- Albums already in `desired_tracks` are skipped to prevent duplicate queue entries.
- The feedback loop is explicit (`--seed-sidecar` flag), not automatic, so the user controls when to run it.

**Revisit Condition**:

- If the coordinator grows a native "album backlog" mode that handles this automatically, the CLI flag can be deprecated.

---

## Decision 28: `default-run = "cassette"` Is Required For Tauri Build

**Status**: approved (applied 2026-04-03)
**Rationale**:

- `src-tauri/Cargo.toml` has multiple `[[bin]]` entries (CLI tools). Without `default-run`, Tauri cannot determine which binary is the app entrypoint and fails with "failed to find main binary".
- Adding `default-run = "cassette"` to the `[package]` section resolves this. The bundler now targets `src-tauri/src/main.rs` correctly.
- `cargo tauri build` now produces `Cassette_0.1.0_x64_en-US.msi` and `Cassette_0.1.0_x64-setup.exe`.

**Tradeoffs**:

- None — this is a zero-cost correctness fix.

**Revisit Condition**:

- If the package is ever renamed, update `default-run` to match.

---

## Decision 29: Cassette Is Identity-First, Not A Generic Downloader Bundle

**Status**: approved (applied 2026-04-03)
**Rationale**:

- The repo already has enough providers. The larger risk is unclear ownership, not missing integrations.
- MusicBrainz is the canonical identity spine for artist/release-group/release/recording truth.
- Spotify is the intent/import seed and source-alias input, not canonical truth.
- Qobuz, Deezer, slskd, Usenet, Jackett, Real-Debrid, and yt-dlp are acquisition adapters or evidence sources, not identity owners.

**Tradeoffs**:

- Some existing command shortcuts and docs now need to be reshaped around the identity-first planner story.
- Provider metadata must be treated as corroborating evidence unless it resolves back to canonical identity.

**Revisit Condition**:

- Only if Cassette intentionally changes product scope away from local-library governance and into generic downloader tooling.

---

## Decision 30: Keep The Dual-Store Runtime Shape Intentionally

**Status**: approved (applied 2026-04-03)
**Rationale**:

- `cassette_librarian.db` is now the canonical control-plane and identity/planning store.
- `cassette.db` remains the playback/runtime cache and active desktop-state store.
- `db_converge_cli` is useful as a migration/export and proof tool, but a single-file runtime is not the near-term architecture goal.

**Tradeoffs**:

- Cross-store convergence must remain explicit and auditable.
- Some runtime surfaces still need better projection of sidecar truth.

**Revisit Condition**:

- If the planner/review surface and runtime cache can be unified without sacrificing auditability, reversibility, or operational clarity.

---

## Decision 31: Planner Surface Ships Read-Only Before Approval Mutations

**Status**: approved (applied 2026-04-03)
**Rationale**:

- Cassette needed a real planner stage before acquisition, but not another command-only shortcut that jumps straight to bytes.
- The first safe cut is read-only: normalize intent, search providers, persist candidate sets, and expose rationale before acquire begins.
- Approval/rejection mutations can come after the candidate-set and rationale surfaces are proven durable and auditable.

**Tradeoffs**:

- Bypass lanes still exist for some CLI/operator flows until planner-backed submission becomes the default.
- The planner currently focuses on request-scoped evidence and provider search results; richer approval policy is still ahead.

**Revisit Condition**:

- Once approval/rejection mutation APIs and planner-backed default submission are live.

---

## Decision 32: Bypass Lanes Are Temporary Operator Debt, Not Product Architecture

**Status**: approved (applied 2026-04-03)
**Rationale**:

- `batch_download_cli`, backlog submission shortcuts, and similar direct-acquire flows are still useful for operator recovery and live probing.
- They should not remain the canonical acquisition story now that request planning, candidate persistence, and rationale surfaces exist.
- Keeping those shortcuts alive as permanent first-class flows would let command-level convenience keep outrunning the planner and identity model.

**Tradeoffs**:

- Some existing operator workflows stay faster in the short term than the intended product path.
- The repo must carry explicit debt markers until planner-backed submission becomes the default.

**Revisit Condition**:

---

## Decision 33: Keep Bandcamp In Resolver-Only Scope For Now

**Status**: approved (applied 2026-04-06)
**Rationale**:

- Current runtime behavior already supports Bandcamp URLs provided in request payloads, which closes the immediate hard-fail gap without introducing a second acquisition owner.
- Expanding Bandcamp to a full search/acquire provider now would dilute the identity-first convergence focus and increase maintenance surface while higher-priority planner/identity work remains open.
- Resolver-only scope keeps ownership clear: Bandcamp contributes payload URL resolution, while canonical acquisition ownership stays with the active provider waterfall.

**Tradeoffs**:

- Bandcamp content discovery is still dependent on payload-supplied URLs rather than first-class search coverage.
- Users do not yet get Bandcamp as an equivalent search lane beside Qobuz/Deezer/Jackett/Usenet flows.

**Revisit Condition**:

- Revisit only after planner-stage cutover and release-group identity threading are complete, and only if a concrete usage need shows resolver-only scope is insufficient.

- Once planner-backed submission and review are the default UI/runtime path and the remaining bypass commands are either demoted or retired.

---

## Decision 33: Album/Artist Projection IDs Must Be Deterministic Across Restarts

**Status**: approved (applied 2026-04-03)
**Rationale**:

- UI album/artist projections need stable IDs for selection state, deep links, and cache keys.
- `DefaultHasher` values are not stable across process restarts because hashing state is seeded.
- Album and artist projection IDs now derive from normalized key material via BLAKE3, then map to a positive 63-bit integer.

**Tradeoffs**:

- IDs remain synthetic rather than first-class relational keys
- Hash collisions are still theoretically possible, though low probability for current scale
- Any legacy cached IDs produced by `DefaultHasher` are invalidated once at upgrade

**Revisit Condition**:

- If album/release entities receive first-class persisted IDs in the canonical schema, migrate projection surfaces to those authoritative keys

---

## Decision 34: Runtime Lyrics Should Cache By Normalized Track Identity

**Status**: approved (applied 2026-04-06)
**Rationale**:

- Lyrics were previously fetched ad hoc from LRCLIB during now-playing lookups and then discarded.
- That made synced/plain lyric availability nondeterministic across repeated plays and paid the network cost over and over for the same track.
- The runtime now persists lyrics by normalized artist/title/album identity, treats rows older than 30 days as stale, and refreshes stale or partial rows on the next now-playing lookup.

**Tradeoffs**:

- There is still no background lyrics prefetch worker, so refresh remains user-path/on-demand.
- The cache is identity-based, not a perfect one-row-per-file mapping, so normalization quality matters.

**Revisit Condition**:

- If Cassette grows a background enrichment worker or a stronger canonical recording spine for lyrics ownership, keep the durable cache behavior and move only the refresh/execution surface

---

## Decision 35: Runtime Metadata Tagging Should Fall Back To Cover Art Archive For MusicBrainz Releases

**Status**: approved (applied 2026-04-06)
**Rationale**:

- Provider candidates do not consistently return usable artwork, even when Cassette already knows the canonical MusicBrainz release ID.
- The tag-writing path already has the final normalized task payload, so it can make a narrow deterministic fallback decision without changing provider search or scoring behavior.
- When provider art is absent, Cassette now tries Cover Art Archive front-art URLs derived from the MusicBrainz release ID before giving up on embedded artwork.

**Tradeoffs**:

- This adds one more network fallback in the metadata-tagging path for MusicBrainz-backed releases.
- Non-MusicBrainz release IDs intentionally skip the fallback to avoid guessing across providers.

**Revisit Condition**:

- If canonical releases begin persisting richer artwork ownership or multiple-artwork selection policy, keep the deterministic fallback order and move only the source of truth

---

## Decision 36: Desktop Runtime Should Own Bundled slskd Lifecycle

**Status**: approved (applied 2026-04-06)
**Rationale**:

- Treating `slskd` as an external prerequisite made Cassette feel like a checklist instead of one desktop system.
- The repo already ships a bundled `slskd.exe`, and the desktop app already knows the runtime settings and local machine defaults needed to launch it.
- Cassette now attempts to start bundled `slskd.exe` during app startup when the configured endpoint is not already reachable, and the settings view exposes runtime status plus restart/stop controls.

**Tradeoffs**:

- Desktop startup now owns one more child-process lifecycle and one more place where local machine state can fail.
- Smoke verification now uses the managed runtime probe path (`slskd_runtime_probe_cli`) and no longer relies on a standalone port-5030 socket check.

**Revisit Condition**:

- If `slskd` is eventually replaced, embedded more deeply, or moved behind a different local broker, preserve the “Cassette owns the daemon lifecycle” contract rather than returning to manual background babysitting

---

## Decision 37: Home Is The Music-First Front Door; Deep Library Lives Behind Dedicated Routes

**Status**: approved (applied 2026-04-06)
**Rationale**:

- The old `/` route was still acting like a generic library browser even after the product direction was explicitly reset around music-first daily use.
- A dedicated Home route lets Cassette summarize playback, background work, and missing-music state without deleting the deeper library tooling.
- Moving deep browse behavior to `/library` preserves the inspection-heavy library view, while `/artists` remains the artist-first collection lens and `/downloads` remains the command center.

**Tradeoffs**:

- One more route exists in the renderer, so navigation labels and tests had to move with it.
- Some users may need a moment to relearn that `Home` is the dashboard and `Library` is the deep browser.

**Revisit Condition**:

- If the desktop shell ever converges to a different primary information architecture, keep the music-first front-door contract unless there is a stronger daily-use replacement.

---

## Decision 38: Smoke Verification Must Reuse The Managed slskd Runtime Contract

**Status**: approved (applied 2026-04-06)
**Rationale**:

- Cassette now owns bundled `slskd` lifecycle, so a bare `localhost:5030` socket ping was no longer an honest proof of the real runtime contract.
- Verification now uses `slskd_runtime_probe_cli`, which calls the same `SlskdRuntimeManager` startup path that the desktop app uses for bundled `slskd.exe`.
- This keeps smoke evidence aligned with the actual machine contract: resolved URL, app-dir, downloads-dir, readiness, and whether Cassette had to spawn the daemon.

**Tradeoffs**:

- Smoke verification now depends on one small Rust probe binary instead of a pure PowerShell socket check.
- The probe validates the managed runtime contract, but it is still not identical to a full desktop-session proof with the Tauri shell open.

**Revisit Condition**:

- If managed daemon ownership moves out of the desktop runtime or the startup contract changes, keep the verification tied to the real owner instead of drifting back to infrastructure-only checks.

---

## Decision 39: Spotify Intake Is One Operator Story With Multiple Entry Modes

**Status**: approved (applied 2026-04-06)
**Rationale**:

- Spotify history album backlog and direct desired-track JSON import were both useful, but presenting them as separate ad hoc lanes made the intake story feel split-brain.
- Cassette now treats them as two entry modes into one identity-first Spotify intake surface: album backlog for broad collection recovery, direct desired-track payloads for richer exact-track evidence.
- The control-plane source contract now defaults direct payload imports to the `spotify_library` lane so the sidecar and coordinator keep speaking the same operator language.

**Tradeoffs**:

- The intake UI is a little denser because it now explains both entry modes in one place.
- The remaining replay-proof work is still separate; a unified operator story does not itself prove reconciliation hit-rate gains.

**Revisit Condition**:

- If Spotify intake splits into materially different execution owners in the future, preserve one visible operator story unless the runtime truth genuinely diverges.

---

## Decision 40: Music OS Convergence Uses Shared Multipliers And Isolation Boundaries

**Status**: approved (applied 2026-04-06)
**Rationale**:

- Cassette has reached the point where isolated feature additions produce diminishing returns unless they converge onto shared system spines.
- The operating model now explicitly prioritizes capabilities that multiply across at least five surfaces when practical (for example planner quality, acquisition quality, UX explainability, telemetry/regression analysis, and recovery diagnostics).
- New high-impact capabilities that can destabilize core behavior (for example heavy visual effects, GPU enrichment, adaptive orchestration) must be isolated behind deterministic fallback boundaries so playback continuity, queue closure determinism, file safety, and audit completeness are never compromised.
- This codifies the transition from "modular feature set" to "personal Music OS" without relaxing core trust guarantees.

**Tradeoffs**:

- More up-front planning and dependency sequencing before implementation work lands.
- Some attractive features may be delayed if they fail either the multiplier test or the isolation test.

**Revisit Condition**:

- If runtime architecture changes enough that shared multipliers no longer provide cross-layer leverage, or if a new core safety model supersedes current deterministic isolation boundaries.

---

## Decision 41: Listening Surfaces Stay Primary; Workstation Is One Click Away But Secondary

**Status**: approved (applied 2026-04-08)
**Rationale**:

- Cassette's real product value is not the operator console. It is the listening environment.
- Downloads, import, repair, diagnostics, and replay are necessary, but they should not dominate the shell or define the product voice.
- The active surface plan therefore separates Tier 1 listening surfaces from a secondary Workstation control area.

**Tradeoffs**:

- Some existing routes and command labels now need regrouping and renaming.
- Review and diagnostic flows must stay discoverable even as the shell gets calmer.

**Revisit Condition**:

- Only if the listening/runtime boundary changes so materially that Workstation is no longer the right secondary container.

---

## Decision 42: Playlist, Crate, Session, And Queue Scene Are Distinct Product Objects

**Status**: approved (applied 2026-04-08)
**Rationale**:

- Cassette cannot deliver strong playlist, queue, and session surfaces while those objects remain blurry.
- Playlist is authored order, Crate is collection slice, Session is arc with memory, and Queue Scene is a saveable queue snapshot.
- Explicit conversions prevent hidden mutation and reduce future surface drift.

**Tradeoffs**:

- The runtime needs a few additive storage and command surfaces instead of one vague "saved list" abstraction.
- Implementation must resist the temptation to silently convert one object into another.

**Revisit Condition**:

- Only if a future storage model can preserve the same distinctions without losing explicit conversions.

---

## Decision 43: Primary UI Must Translate Internal Runtime Terms Into Human Language

**Status**: approved (applied 2026-04-08)
**Rationale**:

- Internal terms such as planner, director, control-plane, candidate set, and dead letter are implementation truths, not surface language.
- Using them on primary listening surfaces makes Cassette feel like tooling instead of a place.
- The listening shell now follows explicit language governance, while Workstation may expose deeper detail with calmer translations.

**Tradeoffs**:

- Some current UI strings, route labels, and command palette entries must be renamed together.
- Documentation and UI reviews now need vocabulary checks in addition to behavior checks.

**Revisit Condition**:

- If the internal runtime language changes materially, update the translation map instead of leaking the new terms into primary surfaces.

---

## Decision 44: Wave 3 And Wave 4 Authored Listening State Lands As Additive Settings-Backed Persistence First

**Status**: approved (applied 2026-04-08)
**Rationale**:

- Playlists, crates, queue scenes, live queue sculpting state, and session memory all needed durable authored behavior to make the listening surfaces real.
- Expanding backend schema just to ship authorship and emotional-center surfaces would have violated the current mission constraints and delayed the actual product turn.
- The existing settings persistence layer was already durable, local-first, and reversible enough for additive UI-side authored state.
- Wave 3 and Wave 4 therefore persist authored overlays in settings through `ui/src/lib/stores/rituals.ts`, while continuing to rely on the proven runtime playback, queue, and playlist commands underneath.

**Tradeoffs**:

- Authored-state persistence is currently UI-owned rather than normalized into the runtime DB.
- Cross-object querying is thinner than it would be with a dedicated relational model.
- Future deeper audit surfaces may need richer persistence if authored state starts driving automation instead of just listening surfaces.

**Revisit Condition**:

- Revisit only when a signature surface needs stronger cross-device history, richer audit playback, or deeper planner/runtime participation than the settings-backed layer can honestly support.

---

## Decision 45: Calm Automation Uses A Four-Threshold Boundary, With Workstation Holding Explicit Intervention

**Status**: approved (applied 2026-04-08)
**Rationale**:

- Cassette needed a clear answer to when background work should stay invisible, when it should become recap, and when it should ask for a deliberate decision.
- Without that boundary, listening surfaces drift back toward operator dashboards and Workstation loses its purpose.
- The UI now follows a four-threshold model: `silent`, `digest`, `soft attention`, and `explicit intervention`.
- Listening surfaces may show digest and soft-attention summaries. Explicit intervention belongs in Workstation and related control routes.

**Tradeoffs**:

- Some detailed state is now one click deeper than before.
- Threshold logic must stay plain and stable or the UI will feel inconsistent.
- New workstation routes need to respect the same boundary instead of smuggling raw internals back into primary listening views.

**Revisit Condition**:

- Revisit only if background-work behavior changes materially enough that the four-threshold model stops matching real daily use, or if a stronger listening-shell digest proves necessary without compromising calmness.

---

## Decision 46: Cassette's UI Direction Is A Modular Desktop Listening Environment, Not Route-First Web-Shell Expansion

**Status**: approved (applied 2026-04-09)
**Rationale**:

- The repo already improved listening language, visual warmth, and calm automation posture, but those improvements still left too much room for a generic route-first desktop-web-app interpretation.
- The actual product target is a modular desktop listening environment with persistent surfaces, layers, resizing, layout memory, and selective true-window breakout where it earns the complexity.
- Cassette's Rust backend, trust spine, and Tauri host remain the correct base. The failure is not "wrong stack"; it is "wrong primary UI abstraction."
- Routes may remain useful for deep links and focused surfaces, but they are no longer the main product model. Surface state, layout state, and truthful action behavior take precedence over page growth.
- The first implementation gate is an interaction-spine audit: visible controls must be proven truthful before more shell ambition lands.

**Tradeoffs**:

- Existing planning docs that focused on route and surface rebuilds need to be read through the narrower lens of content and ritual, not shell architecture.
- A modular desktop shell is more demanding than a pleasant route shell, and true multi-window behavior must be staged carefully to avoid state-sync and focus chaos.
- Some recently completed UI work remains useful, but it should not be misrepresented as the end-state architecture.

**Revisit Condition**:

- Revisit only if Cassette intentionally abandons the modular desktop target or moves to a fundamentally different host/runtime that invalidates the current shell strategy.

---

## Decision 47: Build One Strong Workspace Shell Before Selective True-Window Breakout

**Status**: approved (applied 2026-04-09)
**Rationale**:

- The owner's target includes both layered in-shell behavior and the possibility of real break-apart desktop windows.
- Jumping straight to "everything is a separate OS window" would front-load focus, z-order, restore, and state-sync complexity before Cassette has a trustworthy shell contract.
- The right sequence is therefore: prove the action spine, build one strong workspace shell with persistence and resize behavior, then allow selected modules to become true Tauri windows where that improves the experience.
- This keeps the Rust backend and Tauri host while rejecting the generic route-first web-shell interpretation.

**Tradeoffs**:

- Some detached-window magic is delayed in exchange for a more stable and more coherent first shell.
- The main shell has to carry more design and interaction weight up front.

**Revisit Condition**:

- Revisit only if owner direction changes to require true detached windows as the first milestone instead of a staged breakout strategy.

---

## Decision 48: Library And Workstation Must Be Shell-Owned Surfaces, With Routes Kept As Compatibility Only

**Status**: approved (applied 2026-04-09)  
**Rationale**:

- The direction reset and action-spine audit made it clear that route jumps were overstating modular behavior.
- Cassette now treats the library browser/filter surface and the Workstation surface as shell-owned regions first.
- Routes such as `/library` and `/workstation` may remain useful for fallback or focused views, but they no longer define the core shell architecture.
- This keeps the product aligned with the owner conversation: persistent surfaces, fewer fake modular cues, and an explicit bridge from the old route-led shell into the real workspace model.

**Tradeoffs**:

- The repo now carries both shell-owned surfaces and compatibility routes for a while.
- Some UI logic must be shared between route wrappers and shell surfaces to avoid divergence.

**Revisit Condition**:

- Revisit only if the owner intentionally wants routes to become the primary shell model again, or if the shell later converges far enough that the compatibility routes can be retired cleanly.

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
