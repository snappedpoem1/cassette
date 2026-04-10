# Cassette TODO

**Method**: Prioritize by user impact, reliability risk, and execution clarity.  
**Rule**: If a task is not in this file, it is not committed project scope yet.  
**Last Updated**: 2026-04-10

Scope note:

- This TODO is for a single-owner personal project.
- Terms such as "shipping blocker" and related release language mean personal reliability/readiness blockers only.

Short execution board: see `HIT_LIST.md`.
Open gap execution board: see `OPEN_GAPS_EXECUTION_BOARD.md`.

Gap ID policy:

- Every new implementation or docs-hardening task must map to one `GAP-*` ID from `OPEN_GAPS_EXECUTION_BOARD.md`.
- If no `GAP-*` row exists, add one before starting work.
- Completion claims must include evidence link(s) or artifact path(s).

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

## Current Audited Sequence

These are the next highest-value tasks after the tool-convergence and scope-reset pass.
Do them in this order unless a higher-priority production issue interrupts:

1. Audit and correct canonical docs that still permit route-first or generic web-shell interpretation.
2. Publish the modular desktop workspace contract before broader shell implementation.
3. Audit the interaction spine so visible shell actions are proven truthful before more UX expansion.
4. Finish the listening-surface quality floor with encoding, contrast, and typography fixes.
5. Lock listening-first boundaries, language governance, and object model under the new shell direction.
6. Sequence workspace-shell implementation from persistent surfaces and resizing first, then selective real-window breakout.
7. Prove the first selective breakout with one persisted visualizer window before any broader detached-window push.

---

## P0

### [P0] [done] Capture the first bounded coordinator live proof

Why:

- The runtime shape is now in code: sidecar scan checkpoints, `delta_queue` claims,
  Director payload closure, post-run rescan, and guarded organizer subset logic.
- What is still missing is one bounded, inspectable real-machine proof that the loop works
  end-to-end.

What good looks like:

- A bounded run such as `engine_pipeline_cli --resume --limit 5` starts from a populated sidecar.
- `delta_queue` rows are claimed deterministically and resolved into Director `TrackTask`s.
- Successful terminal outcomes stamp `processed_at`.
- Retryable failures release claims without falsely closing the ticket.
- The post-run librarian sync closes at least one satisfied reconciliation gap.

Touchpoints:

- `src-tauri/src/bin/engine_pipeline_cli.rs`
- `crates/cassette-core/src/librarian/orchestrator.rs`
- Librarian sidecar DB tables: `local_files`, `desired_tracks`, `reconciliation_results`, `delta_queue`
- Director task-history persistence in the active runtime DB

Acceptance:

- [x] Queue claim fields (`claimed_at`, `claim_run_id`, `source_operation_id`) ensured in migrations
- [x] `engine_pipeline_cli` claims actionable rows and releases stale claims
- [x] `processed_at` is marked on successful terminal outcomes
- [x] Librarian sync bookends the coordinator run
- [x] Sidecar scan checkpoints and `full|resume|delta-only` scan modes exist, with unchanged files skipped on rerun
- [x] `generate_delta_queue` preserves claimed (mid-flight) rows — only unclaimed unprocessed rows are wiped
- [x] `mark_processed` preserves `claimed_at`/`claim_run_id` as audit trail; only sets `processed_at`
- [x] Claim-preservation behavior is regression-tested (`adapter_tests::mark_processed_preserves_claim_fields`, `generate_delta_preserves_claimed_rows`)
- [x] End-to-end proof captured: scan -> `delta_queue` populated -> acquisition -> queue state updated -> re-scan closes a gap
- [x] Proof artifact set recorded in `PROJECT_STATE.md` (2026-03-31: DENIAL IS A RIVER via Qobuz, Finalized, claim audit trail intact)

### [P0] [done] Capture coordinator recovery proof with interruption plus resume

Why:

- The code now has resumable sidecar scan state, stale-claim recovery, and deterministic queue leasing.
- That still needs one live proof showing that interruption does not force a restart from zero
  or strand queue work in a bad claim state.

What good looks like:

- An interrupted coordinator run is resumed with `--resume` and continues from durable scan checkpoints.
- Unchanged files are skipped on rerun.
- Stale `delta_queue` claims are reclaimed deterministically.
- Work that already succeeded is not reacquired.
- Retryable or interrupted work remains actionable after resume.

Touchpoints:

- `src-tauri/src/bin/engine_pipeline_cli.rs`
- `crates/cassette-core/src/librarian/scanner`
- Librarian sidecar DB tables: `scan_checkpoints`, `local_files`, `delta_queue`

Acceptance:

- [x] Resume semantics implemented in code
- [x] Queue claims are durable and stale-claim recovery exists
- [x] Live interruption/resume proof captured
- [x] Proof shows resumed scan skips unchanged files and recovers queue work cleanly

### [P0] [done] Finish organizer safety proof after staged track-number repair

Why:

- The repair ladder is implemented and organizer live-guarding is in place.
- The remaining risk is operational, not architectural: prove a bounded live organize pass is safe.

What good looks like:

- `organize_cli --dry-run` no longer proposes large classes of recoverable `00 - ...` renames.
- Unresolved rows are explicit and enumerable.
- A bounded live subset organizes safely without corrupting filenames.

Touchpoints:

- `src-tauri/src/bin/tag_rescue_cli.rs`
- `src-tauri/src/bin/organize_cli.rs`
- Active runtime DB `tracks` table

Acceptance:

- [x] Tag re-scan pass implemented (`tag_rescue_cli`)
- [x] `organize_cli --live` now hard-blocks suspicious mass `00 - ...` renames
- [x] Tag rescue run against the live DB captured (`updated=0`)
- [x] Staged recovery via `embedded_tag`, `filename_prefix`, and `album_pattern` implemented with explicit unresolved reporting
- [x] Bounded live organize proof captured on a safe subset
- [x] Post-proof unresolved set documented clearly

### [P0] [done] Prove audit completeness across organization and admission flows

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

- [x] Canonical audit-trace query surface exists for operation events plus gatekeeper audit rows
- [x] Regression coverage exists for the audit-trace query surface
- [x] Representative tests added or updated
- [x] Validation/logging proof is repeatable
- [x] Documentation updated if expectations change

### [P0] [done] Prove Deezer full-track acquisition end-to-end

Why:

- This was a hard shipping blocker. The path is now proven live on this machine.

Acceptance:

- [x] End-to-end live proof documented in `PROJECT_STATE.md`
- [x] Any remaining partial paths named and tracked

### [P0] [done] Prove pending-task crash recovery end-to-end

Why:

- The runtime now persists pending Director tasks and resubmits them on startup.
- This is proven through a deterministic startup-recovery probe plus filtering tests for stale
  pending rows versus newer terminal history.

Acceptance:

- [x] Startup recovery proof captured via `recovery_probe_cli`
- [x] Recovery ordering and stale-terminal filtering documented
- [x] `PROJECT_STATE.md` updated with exact observed behavior

---

## P1

### [P1] [done] GAP-I01 Audit and correct canonical docs that still permit route-first or generic web-shell interpretation

Why:

- Cassette's direction is currently vulnerable to being misread as a route-first listening web app inside a desktop wrapper.
- Several planning docs improved listening posture and visual language but still leave too much room for generic browser-shell implementation.
- The next implementation pass needs one explicit direction reset so future agents stop optimizing for pages, cards, and fixed rails as the end state.

Reference:

- `docs/MODULAR_DESKTOP_DIRECTION_RESET.md`
- `docs/SIGNATURE_SURFACES_PLAN.md`
- `docs/EXPERIENCE_BOUNDARY_MAP.md`
- `docs/VISUAL_SYSTEM_DIRECTION.md`

Acceptance:

- [x] Canonical docs point to one active shell direction
- [x] Older planning docs are annotated where their scope is narrower than the new direction contract
- [x] `PROJECT_STATE.md`, `PROJECT_INDEX.md`, `TODO.md`, and `OPEN_GAPS_EXECUTION_BOARD.md` no longer overstate current modular-desktop progress

Proof (2026-04-09):

- `docs/MODULAR_DESKTOP_DIRECTION_RESET.md` now defines the canonical direction reset and supersession rules for older route-first planning.
- `docs/SIGNATURE_SURFACES_PLAN.md`, `docs/EXPERIENCE_BOUNDARY_MAP.md`, and `docs/VISUAL_SYSTEM_DIRECTION.md` now carry explicit supersession notes where their older framing is narrower than the new shell contract.
- `docs/PROJECT_STATE.md`, `docs/PROJECT_INDEX.md`, `docs/TODO.md`, and `docs/OPEN_GAPS_EXECUTION_BOARD.md` now treat the current shell as a transition-state fixed shell rather than overstating modular-desktop completion.

### [P1] [done] GAP-I02 Publish the modular desktop workspace contract before broader shell implementation

Why:

- Cassette needs an implementation-ready shell contract before more UI work lands, or the repo will keep drifting back into route-led page expansion.
- The product target is a modular desktop listening environment with surfaces, layers, overlays, docking, persistence, and selective breakout windows.
- That contract does not yet exist in one canonical place.

Reference:

- `docs/MODULAR_DESKTOP_DIRECTION_RESET.md`
- `docs/MODULAR_WORKSPACE_EXECUTION_PLAN.md`
- `docs/GAP_I03_ACTION_SPINE_AUDIT_BRIEF.md`

Acceptance:

- [x] A canonical shell/workspace contract exists with surface types, docking rules, floating rules, overlay rules, preset model, persistence shape, and pop-out eligibility
- [x] The contract clearly distinguishes what stays inside the main shell first versus what may become a true Tauri window later
- [x] The next implementation agent can begin shell work without inferring the product shape from vibes

Proof (2026-04-09):

- `docs/MODULAR_WORKSPACE_CONTRACT.md` now defines the canonical shell model, surface taxonomy, region rules, state ownership, persistence contract, workstation-lid behavior, phased breakout order, and explicit owner-decision questions.

### [P1] [done] GAP-I03 Audit the interaction spine before shell expansion

Why:

- The core trust problem is not only visual. It is that shell actions may imply capability without proving real runtime behavior.
- If visible controls point but do not act, shell expansion and visual work will only deepen distrust.
- Cassette needs a proof pass on the action spine before large workspace changes.

Reference:

- `docs/MODULAR_DESKTOP_DIRECTION_RESET.md`

Acceptance:

- [x] Primary shell actions are traced end to end: UI intent -> Tauri command/event -> Rust state change -> UI reflection
- [x] Fake, drifting, or stale affordances are identified explicitly
- [x] Follow-up implementation tasks are filed for any untrusted control paths

Proof (2026-04-09):

- Canonical audit artifact added at `docs/GAP_I03_ACTION_SPINE_AUDIT_REPORT.md`.
- The audit classifies shell, playback, queue, library, utility-well, and workstation actions as `truthful`, `drifting`, `fake`, or `untested` instead of inheriting confidence from component existence.
- Follow-up implementation gaps were identified: `GAP-I04` (remove silent no-op and optimistic action drift) and `GAP-I05` (replace route-implied workstation/library surfaces with shell-owned workspace regions).

### [P1] [done] GAP-I04 Remove silent no-op and optimistic state drift from primary listening controls

Why:

- Primary controls currently hide failures behind empty `catch` branches and, in the case of seek and volume, can locally mutate UI state before runtime truth is confirmed.
- That makes the shell feel alive even when the action spine is lying.

Reference:

- `docs/GAP_I03_ACTION_SPINE_AUDIT_REPORT.md`

Acceptance:

- [x] Primary transport and shell controls surface bounded failure states instead of silent no-ops
- [x] Seek and volume no longer mutate local playback state ahead of confirmed runtime reflection
- [x] Queue/library/download loads distinguish load failure from genuine emptiness on primary surfaces

Proof (2026-04-09):

- `ui/src/lib/stores/player.ts` now records bounded transport errors instead of silently swallowing failed playback commands, and seek/volume no longer mutate local playback state optimistically.
- `ui/src/lib/stores/shell.ts` now records bounded shell-window failures instead of no-oping minimize/restore problems.
- `ui/src/lib/stores/queue.ts`, `ui/src/lib/stores/library.ts`, `ui/src/lib/stores/playlists.ts`, and `ui/src/lib/stores/downloads.ts` now expose explicit load-error state instead of always collapsing failures into empty data.
- Primary surfaces now render those states: `ui/src/lib/components/NowPlaying.svelte`, `ui/src/routes/+layout.svelte`, `ui/src/lib/components/QueuePanel.svelte`, `ui/src/routes/library/+page.svelte`, `ui/src/lib/components/RightSidebar.svelte`, and `ui/src/routes/downloads/+page.svelte`.
- Regression coverage added in `src-tauri/tests/pure_logic.rs` via `primary_transport_stops_optimistic_seek_and_volume_mutation` and `shell_and_primary_surfaces_show_bounded_load_or_action_failures`.
- Verification: `cargo test -p cassette --test pure_logic -- --nocapture`, `cargo check --workspace`, `npm run build`, `.\scripts\smoke_desktop.ps1`, and `.\scripts\check_docs_state.ps1` all passed on 2026-04-09.

### [P1] [done] GAP-I05 Replace route-implied workstation and library surfaces with shell-owned workspace regions

Why:

- The action audit confirmed that the current shell still overstates modularity by routing into pages instead of opening shell-owned surfaces.
- Workspace-foundation work needs one explicit conversion step from route theater to persistent regions.

Reference:

- `docs/MODULAR_WORKSPACE_CONTRACT.md`
- `docs/MODULAR_WORKSPACE_EXECUTION_PLAN.md`
- `docs/GAP_I03_ACTION_SPINE_AUDIT_REPORT.md`

Acceptance:

- [x] Library becomes a persistent shell-owned browser/filter surface with preview/detail behavior
- [x] Workstation becomes a shell-owned deck/lid instead of a full page route dependency
- [x] Command and sidebar entry points open surfaces or regions instead of overstating route jumps as modular behavior

Proof (2026-04-09):

- `ui/src/routes/+layout.svelte` now hosts a shell-owned library rail, persistent resize handles, a collapsible utility well, preset toggles, and the workstation deck instead of relying on routes alone to imply modularity.
- `ui/src/lib/stores/shell.ts` now owns persisted shell geometry and shell-surface state: `libraryRailWidth`, `utilityWellWidth`, `utilityWellCollapsed`, `utilityWellMode`, `workstationDeckOpen`, and `activeWorkspacePreset`.
- `ui/src/lib/components/LibraryRail.svelte` is the new Explorer-like resident browser/filter surface with tabs, filter, preview tray, and direct play/open actions.
- `ui/src/lib/components/WorkstationDeck.svelte` plus `ui/src/lib/components/WorkstationSurface.svelte` turn Workstation into a left sliding shell deck while keeping `ui/src/routes/workstation/+page.svelte` as an explicit compatibility fallback.
- `ui/src/lib/components/Sidebar.svelte`, `ui/src/lib/stores/commands.ts`, `ui/src/lib/components/AutomationDigestPanel.svelte`, and `ui/src/routes/+page.svelte` now open or target shell-owned surfaces instead of overstating route jumps as modular behavior.
- Regression coverage added in `src-tauri/tests/pure_logic.rs` via `shell_foundation_uses_persistent_library_rail_and_workstation_deck`.
- Verification: `cargo test -p cassette --test pure_logic -- --nocapture`, `cargo check --workspace`, `npm run build`, `.\scripts\smoke_desktop.ps1`, and `.\scripts\check_docs_state.ps1` all passed on 2026-04-09.

### [P1] [review] GAP-I06 Prove the first selective true-window breakout with a persisted visualizer window

Why:

- The owner-approved breakout order starts with `visualizer`, not with a generic pile of detached panels.
- The shell foundation is now real enough to prove one selected desktop window without jumping straight into multi-window sprawl.
- This work needs to establish the breakout pattern honestly: real Tauri window creation/focus, remembered geometry, and explicit copy that the current visualizer is decorative rather than audio-reactive.

Reference:

- `docs/MODULAR_WORKSPACE_CONTRACT.md`
- `docs/MODULAR_WORKSPACE_EXECUTION_PLAN.md`
- `docs/MODULAR_DESKTOP_DIRECTION_RESET.md`

Acceptance:

- [x] A real Tauri visualizer window can be opened or focused from the shell without creating duplicates
- [x] Visualizer window geometry persists and restores on reopen
- [x] The detached visualizer uses a stripped window surface rather than the full shell chrome
- [x] Copy remains honest that the current visualizer modes are decorative / preset-driven, not true audio-reactive analysis
- [x] Canonical docs record the exact proof scope and the remaining limitations

Proof (2026-04-09):

- `ui/src/lib/stores/shell.ts` now owns `openVisualizerWindow()` plus stored geometry reuse under `cassette.shell.visualizerWindowGeometry`.
- `ui/src/lib/stores/commands.ts` and `ui/src/lib/components/NowPlaying.svelte` now expose shell doorways for the detached visualizer window.
- `ui/src/routes/+layout.svelte` now strips the full shell for `/visualizer-window`, and `ui/src/routes/visualizer-window/+page.svelte` hosts the breakout surface with metadata, art, persisted geometry listeners, and explicit decorative-only copy.
- `src-tauri/capabilities/default.json` now grants the extra window capability scope for the `visualizer` label.
- Regression coverage added in `src-tauri/tests/pure_logic.rs` via `first_selective_breakout_uses_a_persisted_visualizer_window`.
- Verification: `cargo test -p cassette --test pure_logic -- --nocapture`, `cargo check --workspace`, `Set-Location ui; npm run build; Set-Location ..`, `.\scripts\smoke_desktop.ps1`, and `.\scripts\check_docs_state.ps1` all passed on 2026-04-09.
- Remaining review note: the native detached-window path is code- and build-proven in this session, but direct hands-on desktop click-through of open/reopen geometry behavior is still the last manual confirmation step.

### [P1] [done] GAP-F00 Finish the listening-surface quality floor

Why:

- The current repo has visible mojibake, weak contrast, remote font dependency, and avoidable a11y debt on primary surfaces.
- No later signature-surface rebuild should land on top of a visibly broken floor.

Reference:

- `docs/VISUAL_SYSTEM_DIRECTION.md`
- `docs/SIGNATURE_SURFACES_PLAN.md`
- `docs/MODULAR_DESKTOP_DIRECTION_RESET.md`

Acceptance:

- [x] Primary listening surfaces in `ui/src` have no mojibake
- [x] `ui/src/app.css` no longer relies on remote font loading
- [x] Contrast floor is raised on primary surfaces without reducing calmness
- [x] Primary surface a11y suppressions are reduced where controls are currently non-semantic
- [x] `npm run build` still passes

Proof (2026-04-09):

- `ui/src/app.css` now lifts the shared contrast floor by brightening `--text-secondary`, `--text-muted`, and `--border-dim` instead of papering over individual screens.
- Encoded comment junk was removed from `ui/src/app.css` and `ui/src/routes/+layout.svelte`, leaving the primary shell sources free of visible mojibake.
- `ui/src/lib/components/CommandPalette.svelte`, `ui/src/lib/components/NowPlayingExpanded.svelte`, and `ui/src/lib/components/QueuePanel.svelte` now use real interactive semantics instead of `svelte-ignore a11y` suppressions on the primary shell/listening surfaces.
- Verification: `cargo test -p cassette --test pure_logic -- --nocapture`, `cargo check --workspace`, `Set-Location ui; npm run build; Set-Location ..`, and `.\scripts\smoke_desktop.ps1` all passed on 2026-04-09.

### [P1] [done] GAP-F01 Lock listening-first boundaries, language governance, and object model before larger UI expansion

Why:

- Cassette has the right engine seriousness already. The current risk is surface drift.
- We need hard rules for navigation, vocabulary, and object identity before rebuilding Collection, Queue, Playlists, and Session.

Reference:

- `docs/SIGNATURE_SURFACES_PLAN.md`
- `docs/EXPERIENCE_BOUNDARY_MAP.md`
- `docs/VISUAL_SYSTEM_DIRECTION.md`
- `docs/OBJECT_MODEL_DECISIONS.md`
- `docs/MODULAR_DESKTOP_DIRECTION_RESET.md`

Acceptance:

- [x] Listening surfaces and Workstation are explicitly separated in shell/navigation
- [x] Primary surfaces do not use banned internal terms
- [x] Playlist, Crate, Session, and Queue Scene have separate definitions and explicit conversion rules
- [x] Anti-feature list for this cycle stays enforced in planning and review

Proof (2026-04-09):

- `docs/EXPERIENCE_BOUNDARY_MAP.md` now explicitly treats Workstation as the single control doorway, not a sidebar sitemap, and clarifies the owner-approved `Acquire`/`Acquisition` vocabulary rule.
- `docs/OBJECT_MODEL_DECISIONS.md` remains the canonical object boundary contract for Playlist, Crate, Session, and Queue Scene, with strict conversion rules and separate primary surfaces.
- `docs/SIGNATURE_SURFACES_PLAN.md` now reflects that Wave 1 shell navigation should expose listening surfaces plus Workstation only.
- `ui/src/lib/components/Sidebar.svelte` now shows listening surfaces plus a single Workstation doorway instead of exposing Downloads, Import, History, Tools, and Settings directly in the main shell nav.
- Verification: `Set-Location ui; npm run build; Set-Location ..`, `cargo check --workspace`, `cargo test -p cassette --test pure_logic -- --nocapture`, `.\scripts\smoke_desktop.ps1`, and `.\scripts\check_docs_state.ps1` all passed on 2026-04-09.

### [P1] [done] GAP-F02 Rebuild Collection, Album, and Artist around ownership and edition visibility

Why:

- `ui/src/routes/collection/+page.svelte` is still stats-first.
- Album lacks a dedicated edition ritual surface.
- Artist still reads more like browse than rediscovery.

Reference:

- `docs/SIGNATURE_SURFACES_PLAN.md`
- `docs/OBJECT_MODEL_DECISIONS.md`

Acceptance:

- [x] Collection becomes ownership-first with best-copy, archive-health, provenance, and edition cues
- [x] Dedicated Album surface exists
- [x] Artist surface exposes missing-from-artist and related-version rails
- [x] Charts remain subordinate to shelves and best-copy narratives

Proof (2026-04-08):

- `ui/src/routes/collection/+page.svelte` now leads with ownership shelves, best-copy framing, archive-health, provenance, and edition visibility instead of stats-first dashboarding.
- `ui/src/routes/albums/[albumId]/+page.svelte` is now the dedicated edition ritual surface with copy comparison, provenance cues, related versions, and family context.
- `ui/src/routes/artists/+page.svelte` now prioritizes owned albums, missing releases, and rediscovery rails.
- Shared ownership logic is centralized in `ui/src/lib/ownership.ts`.
- Verification: `cargo check --workspace`, `cargo test --workspace`, `npm run build`, and `.\scripts\smoke_desktop.ps1` all passed on 2026-04-08.

### [P1] [done] GAP-F03 Rebuild Playlists, Crates, and Queue for daily use

Why:

- Current playlists are only CRUD plus track list.
- Crates do not exist as first-class objects.
- Queue is functional but not sculptable.

Reference:

- `docs/SIGNATURE_SURFACES_PLAN.md`
- `docs/OBJECT_MODEL_DECISIONS.md`

Acceptance:

- [x] Playlists support notes, sections, arc labels, and variants
- [x] Crates exist as saved or temporary collection slices
- [x] Queue supports play after current, pin, hold, cut after this, and queue scenes
- [x] Saved queue scenes can be restored without breaking playback continuity

Proof (2026-04-08):

- `ui/src/routes/playlists/+page.svelte` now carries playlist notes, mood line, sectioned arc labels, variants, and direct handoff into Session and Crate flows.
- `ui/src/routes/crates/+page.svelte` adds first-class saved and temporary collection slices backed by additive authored-state persistence.
- `ui/src/routes/queue/+page.svelte` adds sculpting actions (`play after current`, `pin`, `hold`, `cut after this`) plus queue scene save/restore and pivot actions.
- Authored-state persistence for playlists, crates, queue scenes, and queue ritual state lives in `ui/src/lib/stores/rituals.ts`.
- Queue sculpting helpers live in `ui/src/lib/queue-ritual.ts`, and queue continuity is preserved through `ui/src/lib/stores/queue.ts`.
- Verification: `cargo check --workspace`, `cargo test --workspace`, `npm run build`, and `.\scripts\smoke_desktop.ps1` all passed on 2026-04-08.

### [P1] [done] GAP-F04 Rebuild Session and Now Playing as the emotional center

Why:

- Current Session is still framed as a tool.
- Current Now Playing is useful but not yet immersive.

Reference:

- `docs/SIGNATURE_SURFACES_PLAN.md`
- `docs/VISUAL_SYSTEM_DIRECTION.md`
- `docs/OBJECT_MODEL_DECISIONS.md`

Acceptance:

- [x] Session preserves memory, replay, and branching
- [x] Session can export/import to playlist cleanly
- [x] Now Playing gains stronger art emphasis, calmer chrome, provenance awareness, and focus mode
- [x] Reduced-motion parity is preserved

Proof (2026-04-08):

- `ui/src/lib/components/SessionComposer.svelte` now persists session memory, supports replay and branching, imports from playlists/crates/queue scenes, and exports arcs back into playlists.
- `ui/src/routes/session/+page.svelte` now frames Session as arc memory instead of tooling.
- `ui/src/lib/components/NowPlayingShrine.svelte` and `ui/src/routes/now-playing/+page.svelte` create a dedicated art-led shrine with provenance, context, lyrics, up-next, and calmer chrome.
- `ui/src/lib/components/NowPlaying.svelte`, `ui/src/routes/+layout.svelte`, and `ui/src/lib/components/Sidebar.svelte` now treat the immersion route as a first-class listening surface.
- Verification: `cargo check --workspace`, `cargo test --workspace`, `npm run build`, and `.\scripts\smoke_desktop.ps1` all passed on 2026-04-08.

### [P1] [done] GAP-F05 Refactor automation handoff into calm digest plus Workstation detail

Why:

- Downloads and related control surfaces currently leak too much operator posture into the main product feel.
- Cassette needs calm automation thresholds, not louder tooling.

Reference:

- `docs/SIGNATURE_SURFACES_PLAN.md`
- `docs/EXPERIENCE_BOUNDARY_MAP.md`

Acceptance:

- [x] Main app shows digest-level automation summary, not raw operator detail
- [x] Workstation holds deeper diagnostics, review, replay, import, and history
- [x] Calm thresholds are defined as silent, digest, soft attention, and explicit intervention
- [x] Blocked-work visibility remains high

Proof (2026-04-08):

- `ui/src/lib/automation-digest.ts` now defines the calm automation model and threshold ladder (`silent`, `digest`, `soft_attention`, `explicit_intervention`).
- `ui/src/lib/components/AutomationDigestPanel.svelte` provides the shared digest surface used across listening and workstation boundaries.
- `ui/src/routes/+page.svelte` now uses digest-level automation summary in Home instead of stats-first pressure framing.
- `ui/src/lib/components/RightSidebar.svelte` adds a `Room` rail with the calm automation digest while keeping queue/context adjacent.
- `ui/src/routes/workstation/+page.svelte` now holds the explicit threshold boundary, repair links, and deeper review entry points.
- `ui/src/routes/downloads/+page.svelte` keeps detailed diagnostics inside Workstation and tucks troubleshooting behind an explicit diagnostics toggle.
- Verification: `cargo check --workspace`, `cargo test --workspace`, `npm run build`, `.\scripts\check_docs_state.ps1`, and `.\scripts\smoke_desktop.ps1` all passed on 2026-04-08.

### [P1] [done] GAP-F06 Run the full visual system unification pass after the surface rebuild waves

Why:

- The shell currently mixes good components with generic utility styling.
- The final pass must unify hierarchy, spacing, density, playback-active behavior, and idle beauty after the structural work is done.

Reference:

- `docs/VISUAL_SYSTEM_DIRECTION.md`
- `docs/SIGNATURE_SURFACES_PLAN.md`

Acceptance:

- [x] Visual hierarchy is consistent across primary surfaces
- [x] Long-session readability is improved and verified
- [x] Playback-active shell behavior is calmer and more coherent
- [x] Keyboard-only and low-motion parity remain intact

Proof (2026-04-08):

- `ui/src/app.css` now carries the unified shell polish pass: stronger text contrast, calmer panel treatments, denser but more readable controls, and more coherent sidebar/topbar/now-playing hierarchy.
- `ui/src/lib/components/Sidebar.svelte` now separates `Listen` from `Control` explicitly in navigation.
- `ui/src/routes/settings/+page.svelte` replaces faux-button subnav elements with real buttons and adds clearer route framing.
- `ui/src/routes/import/+page.svelte`, `ui/src/routes/tools/+page.svelte`, `ui/src/routes/downloads/+page.svelte`, and `ui/src/routes/workstation/+page.svelte` now share the calmer workstation surface rhythm instead of reading like separate admin panels.
- `ui/src/lib/visualizer/presets.ts`, `ui/src/lib/components/PlaybackVisualizer.svelte`, and `ui/src/routes/settings/+page.svelte` now keep MilkDrop mode on the curated minimal Butterchurn pack and only load preset names when the tools pane actually needs them, which removes the stray client chunk warning without touching the listening surface contract.
- Verification: `cargo check --workspace`, `cargo test --workspace`, `npm run build`, `.\scripts\check_docs_state.ps1`, and `.\scripts\smoke_desktop.ps1` all passed on 2026-04-08.

### [P0] [done] GAP-G01 Restore playback continuity between the player runtime, queue state, and playlist launch

Why:

- The app could reach a jammed state at track end because the desktop runtime was not consuming player `TrackEnded` events.
- That broke trust on the most basic contract: if audio ends, the queue and shell need to either move forward or stop cleanly without stale "playing" state.

Acceptance:

- [x] The desktop runtime supervises player events instead of leaving `TrackEnded` unconsumed
- [x] End-of-track behavior either advances to the next queue item or marks playback stopped cleanly when the queue is exhausted
- [x] Playlist launch refreshes the visible queue surface so the shell matches the actual runtime queue
- [x] Regression coverage exists for the runtime listener and end-of-track contract

Proof (2026-04-09):

- `crates/cassette-core/src/player/mod.rs` now exposes `recv_event_timeout(...)` so the runtime can consume bounded player events without blocking forever.
- `src-tauri/src/state.rs` now spawns a player-event listener, synchronizes playback state on `Playing`/`Paused`/`Stopped`/`TrackEnded`/`Error`, auto-advances on track end, and records local playback when moving into the next queued item.
- `ui/src/lib/stores/playlists.ts` now reloads the queue store immediately after playlist playback starts.
- `src-tauri/tests/pure_logic.rs` now includes `player_runtime_listener_advances_or_stops_cleanly_on_track_end`.
- Verification: `cargo check --workspace`, `cargo test -p cassette --test pure_logic -- --nocapture`, `cargo test --workspace`, `npm run build`, `.\scripts\check_docs_state.ps1`, and `.\scripts\smoke_desktop.ps1` all passed on 2026-04-09.

### [P1] [done] GAP-G02 Tighten primary listening-surface action copy into direct button labels

Why:

- Several primary controls read like descriptions or route explanations instead of buttons you can actually hit.
- The listening shell should speak in actions, not helper prose.

Acceptance:

- [x] Primary listening buttons now use direct, short labels
- [x] Queue, collection, workstation, and action-rail controls no longer read like descriptive blurbs
- [x] Regression coverage exists for the tightened copy on key surfaces

Proof (2026-04-09):

- `ui/src/routes/+page.svelte`, `ui/src/routes/+layout.svelte`, `ui/src/routes/queue/+page.svelte`, `ui/src/routes/collection/+page.svelte`, `ui/src/lib/components/ContextActionRail.svelte`, `ui/src/lib/components/AutomationDigestPanel.svelte`, and `ui/src/lib/components/RightSidebar.svelte` now use direct labels such as `Play`, `Pause`, `Queue`, `Collection`, `Workstation`, `Commands`, `Play now`, `Add to queue`, and `Get track`.
- `src-tauri/tests/pure_logic.rs` now includes `primary_actions_use_direct_button_labels`.
- Verification: `cargo check --workspace`, `cargo test -p cassette --test pure_logic -- --nocapture`, `cargo test --workspace`, `npm run build`, `.\scripts\check_docs_state.ps1`, and `.\scripts\smoke_desktop.ps1` all passed on 2026-04-09.

### [P1] [done] Harden async and recovery behavior in acquisition flows

Why:

- Acquisition paths are where flaky networks, partial downloads, and timeouts converge.
- Fresh coordinator recovery/resume behavior is now live-proven again (2026-04-06) with deterministic stale-claim reclaim and bounded queue closure.

Acceptance:

- [x] Tests cover interruption/retry behavior already present in the Director suite
- [x] Retry/cooldown thresholds are config fields instead of only engine constants
- [x] Recovery behavior is explicit in queue claims, staged-download resume checks, and startup recovery filtering
- [x] Capture one fresh live recovery/resume proof with the coordinator path (`engine_pipeline_cli --resume --limit 1 --skip-fingerprint-backfill --skip-post-sync --skip-organize-subset` on 2026-04-06; stale seeded claim reclaimed and row processed)

### [P1] [done] Raise packaging and clean-machine confidence

Why:

- "Builds in this workspace" is not the same as "ready to ship."

Acceptance:

- [x] Install/build steps documented for a clean environment (`docs/CLEAN_MACHINE_CHECKLIST.md`, `docs/RELEASE_CHECKLIST.md`)
- [x] Gaps and assumptions recorded (`docs/CLEAN_MACHINE_CHECKLIST.md` Known Gap + `docs/RELEASE_CHECKLIST.md` Known Gaps)
- [x] Release checklist updated (`docs/RELEASE_CHECKLIST.md`)
- [x] Trust-spine verification script exists (`scripts/verify_trust_spine.ps1`)
- [x] `cargo tauri build` produces `.msi` and `.exe` installers — `default-run = "cassette"` added to `src-tauri/Cargo.toml` (2026-04-03)

Proof (2026-04-07):

- `scripts/verify_cleanroom_local.ps1` passed in DisposableProfile mode.
- Installer bundle check passed, and runtime DB plus sidecar DB presence checks passed in app-data.

### [P1] [done] Execute music-first system plan (Phase 1 spine)

Why:

- The core engine has deep capability, but UX still under-expresses it as one coherent system.
- Current direction is explicit: player as front door, acquisition as engine, librarian/organizer as stewards.

Reference:

- `docs/MUSIC_FIRST_SYSTEM_EXECUTION_PLAN.md`

Acceptance:

- [x] Home route is music-first with "while you were away" background summary
- [x] Always-visible compact system health strip exists (provider/service/queue/scan status)
- [x] Artist becomes default library worldview with improved featured-artist normalization behavior
- [x] Downloads reorganized into Missing/In Progress/Blocked/Completed lanes with plain-language status
- [x] No modal/popup spam for normal background automation
- [x] Existing playback/download behavior remains intact under baseline verification

Follow-on:

- [x] Phase 2 universal context-action surfaces (artist/album/track) shipped across Library, Artists, and Playlists via shared action rail (2026-04-07)
- [x] Phase 3 visualizer and appreciation stack v1 hardening (global disable + low-motion guard + richer signal lanes)
  Completed (2026-04-07): waveform and spectrum modes plus optional MilkDrop-style Butterchurn preset mode are live; global disable and low-motion/reduced-effects paths are persisted; frame-budget controls are bounded through configurable FPS cap and hidden-tab/idle throttling; appreciation signal lane is live in Now Playing.
- [x] Dynamic Glass and Mood System hardening (adaptive shell mood + strict fallback)
  Completed (2026-04-07): adaptive shell mood overlays now derive from now-playing artwork/identity with persisted enable/low-motion/intensity controls, static fallback defaults, and bounded non-disruptive transitions.
- [x] Session Composer v1 (explainable transitions + feedback loop + reusable modes)
  Completed (2026-04-07): Home now includes an explainable session arc composer with saved modes, transition reasoning, and replay/skip feedback persistence that updates future composition scoring.
- [x] Safe Extension Surface (capability-scoped model + health telemetry)
  Completed (2026-04-07): Settings now includes a capability-scoped extension registry (visual packs, enricher, provider adapter), deterministic-core access boundary is explicitly blocked, extension failures degrade in isolation, and extension health/telemetry is surfaced with persisted status counters.

### [P1] [done] Execute music-first system plan (Phase 0 contracts)

Why:

- The feature plan was strong but under-specified your true intent as an autonomous personal music system.
- Contract-first alignment prevents drift into disconnected feature work.

Reference:

- `docs/MUSIC_FIRST_SYSTEM_EXECUTION_PLAN.md` (Operating Contract, Experience Contract, Success Metrics)

Acceptance:

- [x] Operating Contract is treated as a hard gate for new UX/core work
- [x] Experience Contract is reflected in Phase 1 deliverables
- [x] "While you were away" narrative schema is defined and ready for implementation
- [x] Collection-intelligence taxonomy is fixed and reflected in downloads planning
- [x] KPI stubs exist for time-to-music, unchanged-file skip rate, auto-resolution rate, blocked-work visibility, intervention frequency, and explainability

### [P1] [done] Operationalize CPU-first startup scan and deferred GPU enrichment lane

Why:

- The performance direction is clear: high-throughput startup/background scanning should saturate CPU + I/O lanes, while GPU work should be reserved for enrichment tasks where acceleration is real.
- This closes the gap between current contract-level planning and concrete runtime execution policy.

Reference:

- `docs/MUSIC_FIRST_SYSTEM_EXECUTION_PLAN.md` (Execution Insight: CPU-First Scan, GPU-Deferred Enrichment)

Acceptance:

- [x] Startup/background scan path uses persistent incremental diffing and deterministic resume checkpoints
- [x] Unchanged-file skip behavior is measurable and recorded in telemetry captures
- [x] GPU work is routed to deferred enrichment queues (for example BPM/key/embedding lanes), not core scan/hash loops
- [x] Background saturation does not regress return-to-music responsiveness
- [x] WAL + queue claim behavior remains deterministic under load

Evidence (2026-04-06):

- Fresh multi-run capture recorded under `artifacts/perf/run-20260406-160911/results.json` (`-Runs 3 -WarmupRuns 1`).
- `scan_resume_queue_only` median/p95 is now sub-second while resume checkpoints keep startup in queue-only mode.
- Live resume probe showed deterministic checkpoint fast-path: `files_scanned=0` / `files_upserted=0` with `local_files=46503` known rows.
- Core scan/coordinator loops remain CPU+I/O lanes; GPU-heavy enrichment stays deferred/out-of-band from startup scan paths.

### [P1] [done] Execute Music OS Stage A convergence backbone

Why:

- The next strategic step is not isolated feature work; it is cross-layer convergence that turns Cassette into one coherent Music OS.
- Stage A establishes shared primitives (trust ledger, edition intelligence, policy profiles) that multiple later capabilities depend on.

Reference:

- `docs/CASSETTE_MUSIC_OS_IMPLEMENTATION_PLAN.md` (Stage A: Contract and Data Foundations)

Acceptance:

- [x] Trust Ledger v1 is implemented with queryable reason-coded mutation evidence across planner, director, and gatekeeper outcomes
- [x] Edition intelligence markers are threaded through request contract, planner filters, and runtime track inspection surfaces
- [x] Policy profiles (Playback-First, Balanced Auto, Aggressive Overnight) are implemented with deterministic, logged runtime behavior changes
- [x] Stage A outputs are reflected in Downloads/Home explainability and settings controls
  Trust Ledger reason cards are now surfaced in Downloads and Home; Edition Intelligence hints now surface in Downloads request rows and Library track inspection; Settings now exposes policy profile controls with immediate director hot-reload.
- [x] Stage A verification evidence is captured in `PROJECT_STATE.md` and `TELEMETRY.md`
  Trust Ledger, Edition Intelligence, and Policy Profile runtime verification evidence is captured and passing (`cargo check --workspace`, `cargo test --workspace`, `ui/npm run build`, and `scripts/smoke_desktop.ps1`).

### [P1] [done] Deliver modular desktop UX modernization (Winamp-inspired, concept-3 visual direction, no Lyra surfaces)

Why:

- Current UX hardening needs a single coherent target style and interaction model, not route-by-route tweaks.
- The redesigned shell must remain operationally dense while improving discoverability, keyboard-first flow, and Windows desktop integration.

Acceptance:

- [x] Iteration-1 implementation artifact created: `docs/UX_MODERNIZATION_ITERATION_01.md`
- [x] A modular UI shell exists with reusable boundaries for navigation, player, command palette, and feature panels
- [x] Visual language follows the Winamp-inspired plus concept-3 direction across Library, Downloads, and Settings
- [x] Lyra assistant surfaces and prompt language are removed from the active desktop UX path
- [x] Windows command system is implemented with centralized command registry and shortcut mappings for top actions
- [x] Minimized player mode exists as a persistent compact presence with restore behavior
- [x] Windows taskbar integration is wired for playback controls and correct minimize/restore behavior
- [x] Keyboard-first accessibility and focus visibility are preserved in redesigned flows
- [x] Route-level refactors keep desktop integration logic outside page components

### [P1] [done] Formalize performance baseline and regression budget

Why:

- The repo has qualitative confidence, but no strict performance contract yet.

Acceptance:

- [x] Core commands benchmarked or timed (scan, organize, validation, bounded coordinator run)
- [x] Baselines recorded in `TELEMETRY.md`
- [x] Regression thresholds documented

### [P1] [done] Close telemetry maturity lane with repeatable artifact+gate evidence

Why:

- Telemetry confidence should be tied to repeatable captures and explicit gate outcomes, not one-off numbers.

Acceptance:

- [x] Fresh trust verification run captured via `scripts/verify_trust_spine.ps1`
- [x] Fresh multi-run perf artifact captured via `scripts/perf_baseline_capture.ps1 -Runs 3 -WarmupRuns 1`
- [x] Candidate perf artifact validated via `scripts/perf_regression_gate.ps1 -CandidateResultPath ...`
- [x] Telemetry cadence/promotion policy documented in `TELEMETRY.md`

Proof (2026-04-07):

- `verify_trust_spine.ps1` passed including workspace tests, UI build, and strict smoke.
- Perf capture artifact: `artifacts/perf/run-20260406-232508/results.json`.
- Regression gate: pass across all tracked scenarios with no fail-level regressions.

### [P1] [done] Audit and correct tool-role documentation drift

Why:

- Tool roles were drifting between docs, settings labels, and runtime behavior.
- Planning against stale tool ownership is now a bigger risk than adding one more provider.

Acceptance:

- [x] `TOOL_AND_SERVICE_REGISTRY.md` matches current runtime truth
- [x] Canonical docs explicitly mark MusicBrainz as identity spine and Spotify as intent seed
- [x] Research/reference docs that diverge are marked non-canonical

### [P1] [done] Unify Spotify ingest lanes into one identity-first import path

Why:

- Spotify history summary import and direct desired-track import previously had different identity fidelity.
- ISRC-first reconciliation only works if ingest actually feeds the schema.

Acceptance:

- [x] Shared Spotify payload parser handles direct desired-track import shapes
- [x] Direct import now persists `source_track_id`, `source_album_id`, `source_artist_id`, `duration_ms`, best-effort `isrc`, and raw payload JSON
- [x] Album-summary queueing and direct desired-track intake share one canonical operator story
- [x] Replay proof shows improved reconciliation hit-rate on a fixed sample

Replay proof (2026-04-06):

- Seeded paired fixed sample in sidecar (`n=50` each) from deterministic runtime track IDs:
  - `spotify_replay_legacy`: minimal identity fields (artist + title only)
  - `spotify_replay_rich`: richer identity fields (artist + album + title + track/disc + duration)
- Ran bounded reconcile pass via `engine_pipeline_cli --resume --limit 0 --skip-post-sync --skip-organize-subset --skip-fingerprint-backfill`.
- Aggregated outcomes:
  - `spotify_replay_legacy`: `weak_match=50`
  - `spotify_replay_rich`: `strong_match=50`
- Seed rows were cleaned from `desired_tracks`, `reconciliation_results`, and `delta_queue` after capture.

### [P1] [done] Route all album expansion through the resilient resolver

Why:

- Album queueing had split logic and a MusicBrainz-only bias even though the fallback resolver already existed.

Acceptance:

- [x] Tauri album queueing uses the shared resolver (`MusicBrainz -> iTunes -> Spotify`)
- [x] `engine_pipeline_cli --import-spotify-missing` uses the shared resolver
- [x] `batch_download_cli` uses the shared resolver
- [x] Regression tests prove the shared resolver is the only album expansion path

### [P1] [done] Separate search owners from execution owners

Why:

- Torrent and Usenet lanes were blurring search and execution responsibilities.
- Clean ownership is required before a real planner stage is worth building.

Acceptance:

- [x] Jackett is the canonical torrent search owner in the Director
- [x] Real-Debrid direct search is disabled by default in the Director
- [x] `torrent_album_cli` only uses apibay behind an explicit fallback flag
- [x] SABnzbd completion now consults queue/history APIs before filesystem fallback

### [P1] [done] Promote canonical identity and source-alias persistence to the active path

Why:

- The control-plane schema can already carry much richer identity than some active intake/queue boundaries provide.
- The remaining weak point is release-group planning and queue-boundary discipline, not raw provider count.

Acceptance:

- [x] Runtime and sidecar persist canonical artist/release/recording and alias-evidence surfaces
- [x] Shared Spotify import now carries richer source IDs and best-effort ISRC
- [x] Active queue/request boundaries now preserve richer source-track/source-album/source-artist identity when available
- [x] Release-group identity is carried and queryable where planning needs edition-level decisions — **DONE 2026-04-06**: request signatures now include `musicbrainz_release_group_id`, and request alias persistence records `musicbrainz.release_group_id` for planner and director request boundaries.
- [x] No active queue boundary collapses back to `artist + title + optional album` when richer identity is already known — **DONE 2026-04-06**: regression coverage now proves release-group-only identity differences produce distinct request signatures and survive evidence/alias persistence.

### [P1] [done] Introduce a planner stage before byte acquisition

Why:

- Candidate search, memory reuse, review, and policy still sit too close to direct acquisition.
- The planner now supports review mutations and submit-on-approval for song and album/artist expansion queue submissions, and the `plan_and_submit` function is live in the canonical coordinator binary.

Acceptance:

- [x] Search/planning and byte acquisition are now distinct stages in the command surface
- [x] Candidate sets are persisted before acquire starts
- [x] Rationale can be queried before acquire begins
- [x] Review/policy APIs exist for approval, rejection, and rationale
- [x] Live coordinator proof captured: `engine_pipeline_cli --resume --limit 5 --skip-organize-subset --skip-post-sync` ran without crash or panic; planner path in binary confirmed live (2026-04-06)

### [P1] [done] Retire acquisition bypass lanes after planner cutover

Why:

- `batch_download_cli` and direct backlog submission paths still bypass the future planner surface.
- Those shortcuts are useful operator tools, but they should stop defining the product story.

Acceptance:

- [x] Bypass lanes are demoted, removed, or explicitly marked as operator-only debt
- [x] Canonical planner path is the default for UI/runtime queue submission

### [P1] [done] Reuse persisted provenance and candidate memory in runtime behavior

Why:

- The active runtime now persists request signatures, candidate sets, provider search outcomes,
  and negative-result memory.
- The product still behaves as if that memory is mostly write-only.

Acceptance:

- [x] At least one user-visible surface explains prior candidate or provider outcomes (debug panel: per-provider success/fail counts, recent task results with disposition and error)
- [x] Useful provider evidence is retained in normalized runtime tables instead of only nested `result_json` blobs (`provider_search_evidence`, `provider_candidate_evidence`, `provider_response_cache`, `identity_resolution_evidence`, `source_aliases`)
- [x] Failed terminal history rows retain provider attribution and `failure_class`
- [x] At least one runtime path reuses persisted search/candidate memory before re-querying
- [x] Exclusion or negative-result memory is wired into a real decision path
- [x] Adaptive provider nudge now reorders waterfall attempts from recent finalized provider memory with trust-rank floor protection and debug evidence logging — **DONE 2026-04-06**

### [P1] [done] Accumulate librarian fingerprint evidence without full-library reruns

Why:

- Gatekeeper could already compute fingerprints, but the librarian/control-plane side still did not
  retain that identity evidence as a first-class fact.
- The bounded fix is to persist `acoustid_fingerprint` in `local_files`, track per-file backfill
  attempt state, and backfill in small deterministic slices during sync instead of treating it as
  throwaway validation output.

Acceptance:

- [x] `local_files` persists `acoustid_fingerprint`
- [x] Gatekeeper admission writes `acoustid_fingerprint` back into `local_files`
- [x] Librarian sync exposes a bounded fingerprint backfill path
- [x] Regression coverage proves missing fingerprints are backfilled and stored
- [x] Unchanged fingerprint failures are suppressed instead of retried every sync
- [x] File mtime changes invalidate stale fingerprint state so rewritten files can be re-backfilled

### [P1] [done] Clean the remaining warning budget

Acceptance:

- [x] `cargo check --workspace` is warning-free
- [x] Real-Debrid dead fields resolved
- [x] CLI bin warnings caused by `state.rs` inclusion removed

### [P1] [done] Repair `cargo test --workspace` on Windows

Why:

- The old Windows failure was in the Tauri lib-test harness startup path, not in the underlying pure Rust logic.
- The fix was to move pure `src-tauri` assertions into `src-tauri/tests/pure_logic.rs` and stop treating the Tauri-linked lib harness as a workspace test dependency.

Acceptance:

- [x] Root cause identified and documented
- [x] `cargo test --workspace` passes again, or the test split is deliberately redesigned and documented

### [P1] [done] Add provider health awareness to the Director waterfall

Acceptance:

- [x] Health state is tracked per provider with a timestamp
- [x] The waterfall can skip known-down providers
- [x] Health changes are visible to the UI or logs

### [P1] [done] Resolve `downloader/` vs `director/providers/` overlap

Acceptance:

- [x] Decision recorded: `director/providers/` is the active runtime acquisition path
- [x] Dead code removed or clearly marked as historical
- [x] Module status in `PROJECT_INDEX.md` updated

### [P1] [done] Deepen active-runtime provenance persistence

Acceptance:

- [x] Candidate-set and candidate-item tables exist in the active runtime DB
- [x] Provider search outcomes and provider-negative memory persist on normalized request signatures
- [x] Terminal save path is transactional and covered by tests

---

## P2

### [P1] [done] Add canonical release identity persistence and a stronger request contract

Why:

- The next architecture step is no longer "invent a new pipeline."
- It is making the existing pipeline more exact about what it is trying to acquire and how identity
  is persisted across providers.

Acceptance:

- [x] Request contract supports more than `artist + title + optional album`
- [x] Runtime schema now includes canonical artist/release/recording and alias persistence surfaces (`canonical_artists`, `canonical_releases`, `canonical_recordings`, `source_aliases`)
- [x] MusicBrainz-backed artist/release-group/release/recording identity persistence plan is documented (`docs/REQUEST_CONTRACT_IDENTITY_PLAN.md`)
- [x] Follow-on implementation scope is recorded in `WORKLIST.md`
- [x] Release-group identity is used as a first-class planner decision/rationale lane
- [x] Command-boundary contract tests cover all request scopes and policy fields

### [P2] [done] Resolve `Album.id` stability

Why:

- Album and artist IDs were generated with Rust `DefaultHasher`, which is seeded and not
  stable across process restarts. This could invalidate UI selection state and request links.

Acceptance:

- [x] Decision recorded in `DECISIONS.md`
- [x] Deterministic stable IDs now exist for album/artist surfaces via BLAKE3-derived IDs
- [x] Regression tests prove IDs are stable across DB reopen

### [P2] [done] Implement `MetadataRepairOnly` acquisition strategy

Why:

- `MetadataRepairOnly` now resolves matching local tracks from runtime DB identity fields and
  applies in-place metadata repair without acquisition.

Acceptance:

- [x] Implemented in Director engine with runtime DB-backed local track matching
- [x] Tests cover missing runtime DB path, no-match failure, and successful repair path

### [P2] [done] Document and test long-session desktop behavior

Why:

- Media apps earn trust through stability over time, not just one clean smoke run.

Acceptance:

- [x] Soak-test procedure documented (`docs/SOAK_TEST_PROCEDURE.md`)
- [x] Baseline evidence run captured and known pain points recorded (`docs/SOAK_EVIDENCE.md`)

### [P2] [done] Integrate Cover Art Archive after canonical release selection

Acceptance:

- [x] Runtime artwork fallback now covers broader sibling-art names plus embedded-art cache extraction
- [x] Artwork fetch is tied to canonical release choice, not ad hoc provider metadata
- [x] Tag/embed flow documents when Cover Art Archive is used

### [P2] [done] Add a bounded lyrics refresh policy on top of the runtime lyrics cache

Acceptance:

- [x] Synced/plain lyrics now persist durably in the runtime DB and are reused before LRCLIB refetch
- [x] Cache refresh/expiry policy is explicit and documented
- [x] Optional prefetch lane exists for recently played or newly finalized tracks if it is still worth the complexity — **DONE 2026-04-06**: bounded background lyrics prefetch now seeds LRCLIB cache from recent play-history and recent finalized-task metadata candidates with strict per-run item and timeout caps.

### [P2] [done] Add a bounded runtime canonical backfill lane for older tracks

Acceptance:

- [x] Existing runtime `tracks` rows missing canonical artist/release IDs can be backfilled without a full-library rewrite
- [x] Backfill ordering is deterministic and bounded
- [x] Startup integration logs failures instead of aborting app boot

### [P2] [done] Surface runtime MusicBrainz identity in the desktop UI

Acceptance:

- [x] Library inspection exposes persisted MusicBrainz recording/release IDs
- [x] Canonical artist/release IDs are visible in the active desktop runtime
- [x] No extra DB or network fetch is required just to inspect already persisted identity

### [P2] [done] Make bundled slskd part of the desktop runtime lifecycle

Acceptance:

- [x] Cassette attempts to start bundled `slskd.exe` during desktop startup when the endpoint is not already reachable
- [x] Settings exposes runtime status plus refresh/restart/stop controls
- [x] Smoke tooling is updated so it exercises the managed `slskd` startup contract instead of checking port `5030` in isolation

### [P2] [done] Prove and document Discogs and Last.fm enrichment behavior end-to-end

Acceptance:

- [x] Canonical docs and reference docs consistently reflect current Discogs/Last.fm runtime behavior
- [x] Bounded end-to-end proof captured for enrichment outcomes in active flows

Proof: `enrich_probe_cli --limit 25` ran against the live runtime DB on 2026-04-07 with Discogs token and Last.fm API key configured. Result: `25 tracks probed | Discogs hits: 25/25 | Last.fm hits: 0/25` on sampled corpus; binary ran correctly and captured bounded credentialed behavior. `PROJECT_STATE.md` and `TOOL_AND_SERVICE_REGISTRY.md` updated with proof artifact.

### [P2] [done] Add dead-letter command center with replay path

Acceptance:

- [x] Runtime DB exposes grouped dead-letter summary for permanently failed/cancelled tasks
- [x] Tauri command surface exposes dead-letter summary and per-task replay actions
- [x] Downloads UI shows collapsible dead-letter groups with suggested fixes and retry buttons
- [x] Replay path routes through planner (`plan_acquisition` -> `approve_planned_request`) with replay lineage

### [P2] [done] Clarify Bandcamp scope as payload URL resolver and decide next-step ownership

Acceptance:

- [x] Docs explicitly state Bandcamp currently resolves payload-provided URLs only
- [x] Follow-up decision recorded: expand to full provider path or keep resolver-only scope (see `DECISIONS.md`, Decision 33)

### [P2] [done] Tighten metadata and enrichment operating story

Why:

- Metadata logic exists, but runtime ownership and lifecycle are still less explicit
  than core library flows.

Acceptance:

- [x] Current enrichment behavior documented
- [x] Future integration plan recorded without overstating readiness

---

## P3

### [P3] [done] Add richer provider health and troubleshooting views in UI

Acceptance:

- [x] Downloads command center shows a provider troubleshooting snapshot with down/unknown totals
- [x] Per-provider diagnostics now show status, last-check timestamp, runtime message, and actionable hint text
- [x] Troubleshooting hints incorporate provider configuration state and slskd runtime readiness

### [P3] [done] Revisit broader release automation once packaging proof is stable

Acceptance:

- [x] Manual release-candidate workflow exists (`.github/workflows/release-candidate.yml`)
- [x] Workflow runs CI gate and packaging, then uploads installers plus SHA256 manifest
- [x] Optional perf-gate path is available before packaging

### [P3] [done] Improve artist deep-link from library page

The earlier navigation gap has been fixed. Keep this here as history until a later cleanup pass
removes completed P3 items from the active backlog view.

---

## Completed Highlights

- Deezer full-track acquisition is live-proven on this machine.
- Pending-task startup recovery is proven through `recovery_probe_cli`.
- `engine_pipeline_cli` now uses durable `delta_queue` claims with sidecar scan checkpoints and `full|resume|delta-only` scan modes.
- `tag_rescue_cli` now performs staged DB repair via `embedded_tag`, `filename_prefix`, and `album_pattern`, with unresolved-row reporting.
- `organize_cli --live` now aborts on suspicious mass `00 - ...` renames.
- `director/providers/` is the canonical runtime acquisition path; `downloader/` is compatibility-only.
- Active runtime provenance now persists request signatures, candidate sets, provider searches, provider attempts, and negative-result memory.

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
