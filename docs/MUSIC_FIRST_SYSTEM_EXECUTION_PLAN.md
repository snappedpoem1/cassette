# Music-First System Execution Plan

Last updated: 2026-04-06
Owner: Christian (single-owner personal project)

## Purpose

Turn the current Cassette architecture into one coherent personal music system where:

- player is the front door
- acquisition is the engine
- librarian/organizer are the stewards of trust

This plan merges:

- current runtime truth from `PROJECT_STATE.md`
- active backlog constraints from `TODO.md`
- architecture recommendations from recent concurrency/startup scans
- user direction from the UX/core Q&A session

## Product Direction (Locked)

Working line:

My Music, My Rules, One App / One Music.

Operational interpretation:

- Music-first home experience always wins over admin surfaces.
- Deep system power remains available, but hidden by default.
- Background automation should be aggressive when confidence is high.
- Auditability, reversibility, and deterministic behavior remain non-negotiable.

## Operating Contract (New Baseline)

This is not a command-following app; it is an autonomous personal music system.

1. Autonomy by default:
   high-confidence actions run in background without repeated prompts.
2. Human-readable intelligence:
   system explains outcomes in plain language, not raw machine logs.
3. Persistent incremental behavior:
   unchanged files and already-resolved work are skipped deterministically.
4. In-app ownership:
   required local services are managed inside Cassette.
5. Safe aggression:
   maximize throughput with bounded concurrency and explicit rollback/audit edges.

## Experience Contract (New Baseline)

1. Music-first calm shell:
   return to playback quickly; system depth is available on demand.
2. Artist-first worldview:
   artist is primary navigation lens; featured-artist clutter is normalization debt.
3. Collection intelligence center:
   missing, blocked, retry, upgrade, and rationale are visible in one place.
4. While-you-were-away narrative:
   background changes are summarized as concise daily-use messages.
5. No interruptive noise:
   avoid modal/popup spam for normal autonomous operations.

## Success Metrics (Experience + Engine)

Track these for each phase:

1. Time-to-music:
   app launch to resumed playback action.
2. Unchanged-file skip rate:
   percentage of files skipped during startup/background scans.
3. Auto-resolution rate:
   percentage of metadata/acquisition tasks resolved without user intervention.
4. Blocked-work visibility rate:
   percentage of blocked items with plain-language reason and next action.
5. Intervention frequency:
   number of prompts/review interruptions per day.
6. Trust explainability score:
   percentage of major mutations with human-readable "what happened and why".

## Non-Negotiables

1. No pop-up spam for normal background work.
2. Artist is the default library worldview.
3. Missing music and recovery are first-class UX, not buried tools.
4. Services required by Cassette (for example slskd) are app-managed inside Cassette.
5. Every significant automated mutation has plain-language rationale and traceability.

## Target Experience Model

### Layer 1: Music-First Home

- Landing view prioritizes now-playing and quick listen actions.
- "While you were away" summary explains background work in human terms.
- A compact system health strip shows key runtime status without taking over the screen.

### Layer 2: Artist-First Library

- Artist view is default.
- Featured artist fragmentation is treated as metadata normalization debt, not a browse model.
- Album and track views are deep lenses under artist identity, with context actions.

### Layer 3: Missing Music Command Center

- Unified lanes: Missing, In Progress, Blocked, Completed, Upgrade Candidates.
- Any artist/album/track can trigger fill/complete/upgrade actions.
- Blockers and retries are visible with plain-language reasons.

### Layer 4: Trust and Stewardship

- Metadata, artwork, lyrics, provenance, and service health are inspectable on demand.
- Confidence and rationale are shown as understandable summaries, not raw logs.

## Phased Delivery Plan

## Phase 0 (Immediate): Contract Hardening

Goal:

Lock behavior contracts before additional feature layering.

Deliverables:

1. Operating Contract and Experience Contract are canonical and referenced by TODO/HIT_LIST.
2. "While you were away" message schema and severity bands are defined.
3. Collection-intelligence taxonomy is fixed:
   Missing, In Progress, Blocked, Completed, Upgrade Candidates.
4. Baseline KPI capture stubs exist in telemetry docs/scripts.

Acceptance gates:

1. New work items map to these contracts explicitly.
2. Metrics can be captured consistently for before/after comparisons.
3. No new UX work bypasses contract requirements.

Implementation status (2026-04-06):

- Executed in the active desktop UI and canonical docs.
- `docs/TODO.md`, `docs/HIT_LIST.md`, and `docs/TELEMETRY.md` now reflect the contract surfaces and KPI stubs.
- The "while you were away" narrative schema is implemented on the Home route and documented below.

## Phase 1 (Now): Product Spine Unification

Goal:

Make Cassette feel like one app with a music-first center and background intelligence.

Deliverables:

1. Home route reframe to now-playing-first + "while you were away" summary card.
2. Persistent bottom status strip with provider/service/queue/scan signals.
3. Artist-first default navigation and guardrails against featured-artist clutter.
4. Downloads route reorganized into Missing/In Progress/Blocked/Completed lanes.

Acceptance gates:

1. Opening app returns to music-first view in one action.
2. Background events are visible without modal interruptions.
3. Missing music state is understandable at a glance.
4. Existing playback and download flows remain functional.

Implementation status (2026-04-06):

- Executed in the desktop shell and renderer.
- `/` is now the music-first Home route.
- `/library` preserves the deep library browser, while `/artists` remains the artist-first collection lens.
- The shell carries a persistent bottom status strip for provider/service/queue/scan signals.
- Downloads now presents Missing, In Progress, Blocked, and Completed lanes with request-level review detail.

## While-You-Were-Away Schema

Narrative message contract:

1. `tone`: one of `steady`, `watch`, `action`
2. `title`: one-scan headline describing what changed
3. `detail`: plain-language explanation of what happened or what needs attention

Current generation order:

1. completed handoffs
2. still-running work
3. blocked or review-needed work
4. missing backlog state
5. quiet fallback when no recent activity exists

Tone bands:

- `steady`: no user action needed; system is progressing or settled
- `watch`: useful awareness; likely worth opening Downloads soon
- `action`: blocked or failed work has crossed the threshold where intervention is warranted

## Phase 2 (Next): Universal Action Surfaces

Goal:

Enable "do it here" actions on artist, album, and track objects.

Deliverables:

1. Unified context actions across artist/album/track:
   - fill missing
   - queue discography
   - replace with matching-or-better quality
   - repair metadata
   - inspect provenance
2. Action outcome toasts/messages use plain-language summaries.
3. Ambiguous/high-risk actions route to review panels.

Acceptance gates:

1. Same operation can be triggered from all relevant objects.
2. Action results and failures are logged and human-readable.
3. No destructive action bypasses existing safety controls.

## Phase 3: Trust UX and Automation Policy

Goal:

Expose deep system intelligence without overwhelming daily listening flow.

Deliverables:

1. Confidence labels and "why" summaries for major automation decisions.
2. Policy modes in settings:
   - Aggressive Auto
   - Auto + Review on Ambiguity
   - Review-First
3. Metadata depth panels for rationale, aliases, and evidence lineage.

Acceptance gates:

1. Users can explain what the system did and why from UI context.
2. Ambiguous cases are reviewable, not silently over-mutated.
3. Listening flow remains uninterrupted.

## Phase 4: Background Engine Saturation (High-Performance Personal Rig)

Goal:

Maximize throughput while keeping deterministic control-plane behavior.

Deliverables:

1. Startup background incremental scan cycle (resume/delta-first).
2. Event-driven scan queue with debounce/coalescing.
3. Adaptive bounded concurrency profile tuned for high-end hardware.
4. Connection/session reuse standardization across provider lanes.

Acceptance gates:

1. Unchanged files are skipped persistently and predictably.
2. Startup scan/acquisition work runs in background without blocking playback UX.
3. Queue leases and audit trails remain deterministic under load.

## Performance Profile: Aggressive Personal Rig (Default Candidate)

Initial profile target (subject to telemetry gate):

1. Global worker concurrency floor 12, cap up to 32 depending on cores.
2. Fingerprint backfill concurrency floor 8, cap up to 24.
3. Provider parallel search budget 4 with per-provider caps.
4. Resume/delta scan default for startup background loops.
5. Search/provider cache TTLs tuned for reduced reconnect churn.

Guardrails:

1. Queue claim/mark semantics must remain serialized and idempotent.
2. Final file placement/conflict resolution remains deterministic.
3. Any aggressive defaults require telemetry evidence and easy rollback.

## Execution Insight: CPU-First Scan, GPU-Deferred Enrichment

Operational implementation guidance for high-throughput local rigs:

1. Treat discovery, metadata extraction, and hashing as CPU/I/O lanes.
   - Saturate these with bounded async I/O plus CPU worker pools.
   - Do not offload these paths to GPU; transfer overhead is counterproductive.
2. Reserve GPU for post-admission enrichment lanes only.
   - Run BPM/key inference, audio embeddings, and optional stem analysis in a low-priority deferred queue.
   - Keep playback and acquisition responsiveness higher priority than enrichment throughput.
3. Make startup behavior persistently incremental.
   - Use durable file-state signatures (at minimum `mtime` + `size`) to skip unchanged files deterministically.
   - Resume from durable checkpoints after interruption; do not restart whole-library scans.
4. Preserve UI responsiveness under heavy background work.
   - Keep SQLite in WAL mode and enforce deterministic queue-claim semantics.
   - Background cycles must never block return-to-music behavior.

GPU execution gates and fallback policy:

1. Enable GPU enrichment only when all gates pass:
   - compatible device/runtime detected at startup
   - no active playback underrun pressure
   - enrichment queue depth exceeds minimum batching threshold
2. Force CPU fallback when any gate fails:
   - driver/runtime unavailable or unhealthy
   - device memory pressure crosses safety threshold
   - enrichment task misses bounded timeout budget
3. Fallback behavior must be deterministic and auditable:
   - task remains in the same identity lane and retry policy
   - provider/acquisition priorities are unaffected
   - reason code is persisted for telemetry and troubleshooting

## Verification and Evidence Requirements

For each phase PR/pass:

1. Targeted tests for changed modules (happy path, failure path, determinism).
2. Baseline gates:
   - `cargo check --workspace`
   - `cargo test --workspace`
   - `ui`: `npm run build`
3. Desktop soak checks updated in `SOAK_TEST_PROCEDURE.md` as relevant.
4. Runtime truth documentation updated in `PROJECT_STATE.md` and backlog in `TODO.md`.

## KPI Stub Ownership

Baseline KPI stub ownership now lives in `docs/TELEMETRY.md` under the music-first experience metrics section:

1. time-to-music
2. unchanged-file skip rate
3. auto-resolution rate
4. blocked-work visibility rate
5. intervention frequency
6. trust explainability score

## Agent Execution Sequence

1. `SE: Architect`:
   define safe concurrency boundaries, UX-to-core integration points, acceptance criteria.
2. `SWE`:
   implement Phase 1 and then Phase 2 with minimal-risk diffs.
3. `QA`:
   execute stress/soak and determinism validation.
4. `SE: Security`:
   review service-management and background automation risk boundaries.

## Out of Scope For This Pass

1. Multi-machine/cloud sync strategy.
2. Productization/commercialization workflows.
3. Broad schema convergence beyond current dual-store boundaries.

## Music OS Expansion Link

The music-first plan above remains the behavioral contract and UX spine.
The next-order system convergence program (trust ledger, edition intelligence, policy profiles,
adaptive orchestrator behavior, dead-letter command center, appreciation stack, and extension safety)
is sequenced in:

- `docs/CASSETTE_MUSIC_OS_IMPLEMENTATION_PLAN.md`

Execution rule:

1. Follow this plan for contract constraints.
2. Follow the Music OS plan for cross-layer order-of-operations.
3. If the two conflict, preserve determinism, auditability, and file safety first.

## Immediate Start Checklist

1. Execute Stage A in `docs/CASSETTE_MUSIC_OS_IMPLEMENTATION_PLAN.md` (Trust Ledger v1, Edition Intelligence v1, Policy Profiles).
2. Implement the P1 CPU-first startup scan and deferred GPU enrichment lane.
3. Capture one fresh live coordinator recovery/resume proof under current runtime conditions.
4. Refresh telemetry and project-state evidence after Stage A and the runtime proofs pass.
