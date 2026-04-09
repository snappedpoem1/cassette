# Open Gaps Execution Board

Last updated: 2026-04-08
Owner: Christian (Capn)

Scope note:

- This board is for a single-owner personal system.
- "Release" and "shipping" language means personal reliability/readiness on this machine.

---

## Purpose

Turn known documentation/runtime gaps into executable, non-overlapping work with:

- explicit gap IDs
- owner lanes
- recommended execution agents
- measurable acceptance checks
- anti-regression gates that prevent drift back into old states

This board complements:

- `TODO.md` (priority and acceptance)
- `HIT_LIST.md` (short execution board)
- `PROJECT_STATE.md` (runtime truth)
- `PACKET_1_EXECUTION_LOG.md` (active Packet 1 execution evidence)
- `PACKET_2_EXECUTION_LOG.md` (active Packet 2 execution evidence)
- `PACKET_3_EXECUTION_LOG.md` (active Packet 3 execution evidence)

---

## Status Legend

- `todo`
- `in_progress`
- `blocked`
- `review`
- `done`

---

## Canonical Gap Taxonomy

Every open gap must map to exactly one class:

- `Data Model`: persistence/identity/shape constraints block capability.
- `UX Contract`: missing request grammar, review controls, or approval boundaries.
- `Integration`: building blocks exist but are not wired into an end-to-end path.
- `Proof`: behavior exists but does not have repeatable, current evidence.
- `Legacy Debt`: compatibility or bypass surfaces that should not remain canonical.

Classification rule:

- one gap ID -> one taxonomy class -> one owner lane.
- if a gap seems multi-class, split it into multiple `GAP-*` rows.

---

## Anti-Regression Rules (Global)

1. No merge without one mapped `GAP-*` ID.
2. No status claim without a dated proof artifact path.
3. No low-trust provider fallback in normal flow without explicit review approval.
4. No planner contract change without request/response snapshot tests.
5. Canonical docs must agree: `PROJECT_INDEX.md`, `PROJECT_STATE.md`, `TODO.md`, `TOOL_AND_SERVICE_REGISTRY.md`.

---

## Dependency Graph

- Lane 0 must complete before Lanes A/B/C are marked `in_progress`.
- Lanes A/B/C can run in parallel.
- Lane D can start only after A and B acceptance checks pass.
- Lane E runs continuously and must be green before closing any lane.

---

## Lane 0 - Contract Lock (required first)

| Gap ID | Task | Status | Recommended Agent | Acceptance Check |
|---|---|---|---|---|
| GAP-000 | Freeze canonical gap taxonomy (Data Model / UX Contract / Integration / Proof / Legacy Debt) in docs | done | SE: Tech Writer | Every active gap has exactly one class and one owner lane |
| GAP-001 | Add mandatory gap-ID tagging policy for implementation and docs updates | done | SE: Tech Writer | `TODO.md` and this board link all open work to `GAP-*` IDs |

Execution notes:

- Keep this lane docs-only.
- Do not change runtime behavior here.

Evidence:

- Taxonomy is now codified in this file under `Canonical Gap Taxonomy`.
- GAP-ID policy is now codified in `TODO.md` and `AGENT_CODEX.md`.

---

## Lane A - Identity And Data Model Closure (parallel)

| Gap ID | Task | Status | Recommended Agent | Acceptance Check |
|---|---|---|---|---|
| GAP-A01 | Promote release-group identity to planner-visible first-class lane | done | SWE | Planner rationale includes release-group identity and policy outcomes |
| GAP-A02 | Introduce canonical edition object in active request/planner flow | done | SWE | Request/planner persistence stores edition attributes queryably |
| GAP-A03 | Add unified cross-source identity mapping (MBID/source aliases) in active path | done | SWE | Equivalent requests across sources resolve to one canonical identity envelope |
| GAP-A04 | Add durable preference/exclusion persistence model by request identity | done | SWE | Exclusions survive restart and are reused for equivalent requests |

Verification gates:

- Request signature/release-group regression tests pass.
- Snapshot tests prove rationale stability for equivalent requests.

---

## Lane B - Planner UX Contract And Approval Gates (parallel)

| Gap ID | Task | Status | Recommended Agent | Acceptance Check |
|---|---|---|---|---|
| GAP-B01 | Implement include/exclude request grammar for album/edition targeting | done | Context Architect -> SWE | Planner accepts explicit include/exclude payload and rejects ambiguous input |
| GAP-B02 | Expose candidate review as formal pre-acquisition contract | done | SWE | Candidate set is visible and selectable before byte acquisition |
| GAP-B03 | Require explicit approval for low-trust fallback paths (yt-dlp / TPB/RD) | done | SWE | No low-trust path executes in standard mode without approval artifact |
| GAP-B04 | Add preflight validation contract before final download decision | done | SWE | Preflight result is persisted and displayed in review flow |

Verification gates:

- Contract tests for parser and command boundaries.
- Policy-gate tests proving low-trust denial without approval.

---

## Lane C - Provider Reliability And Proof Habit (parallel)

| Gap ID | Task | Status | Recommended Agent | Acceptance Check |
|---|---|---|---|---|
| GAP-C01 | Convert provider statuses to repeatable probe-based evidence entries | done | QA | Each provider has dated pass/fail artifact with verification scope |
| GAP-C02 | Close SAB completion proof uncertainty with bounded end-to-end runbook | done | QA | SAB completion flow has one reproducible proof artifact and failure taxonomy |
| GAP-C03 | Resolve LRCLIB verification ambiguity with direct probe and docs evidence | done | QA | LRCLIB status moves from unverified to bounded-proven or explicitly constrained |
| GAP-C04 | Standardize status scopes (`local-proven`, `bounded-probe`, `unverified`) | done | SE: Tech Writer | Registry/reference docs use one consistent status vocabulary |

Verification gates:

- Probe outputs are stored and linked in docs.
- Stale evidence (> agreed window) downgrades status automatically in docs policy.

---

## Lane D - Legacy Lane Retirement (depends on A + B)

| Gap ID | Task | Status | Recommended Agent | Acceptance Check |
|---|---|---|---|---|
| GAP-D01 | Enforce planner-first submission for normal user flows | done | Technical Debt Remediation Plan -> SWE | Normal entrypoints cannot bypass planner contract |
| GAP-D02 | Keep direct-submit paths operator-only with explicit flags | done | SWE | Direct-submit commands reject non-operator mode |
| GAP-D03 | Retire compatibility surfaces after call-site migration (`downloader/`, `ProviderBridge`) | done | SWE | Active runtime path no longer imports legacy compatibility modules |

Verification gates:

- Command-level tests for operator-only bypass behavior.
- Compile/test sweep ensures no accidental legacy imports in active path.

---

## Lane E - Canonical Documentation Convergence (continuous)

| Gap ID | Task | Status | Recommended Agent | Acceptance Check |
|---|---|---|---|---|
| GAP-E01 | Resolve canonical-doc contradictions for slskd smoke proof wording | done | SE: Tech Writer | `PROJECT_STATE.md` and `DECISIONS.md` state one consistent verification story |
| GAP-E02 | Resolve Cover Art Archive wiring status conflicts | done | SE: Tech Writer | Registry/reference align with observed runtime/tagging fallback behavior |
| GAP-E03 | Add docs consistency gate to regular workflow (`scripts/check_docs_state.ps1`) | done | SE: DevOps/CI | Docs check is required before closing any `GAP-*` lane |

Verification gates:

- Docs checker passes.
- No contradictory status lines remain across canonical docs.

---

## Lane F - Signature Surface Rebuild (reopened 2026-04-08)

Purpose:

- move the active mission from backend seriousness to authored listening surfaces without breaking Cassette's trust spine

| Gap ID | Task | Status | Recommended Agent | Acceptance Check |
|---|---|---|---|---|
| GAP-F00 | Finish Wave 0 quality floor on primary listening surfaces | todo | SWE + Design Engineer | No mojibake, no remote font dependency, stronger contrast, reduced primary-surface a11y debt |
| GAP-F01 | Lock listening-first boundaries, language governance, and object model | todo | Product Engineer + IA Lead | Workstation is secondary, banned internal terms are removed from primary surfaces, object model is explicit |
| GAP-F02 | Rebuild Collection, Album, and Artist around ownership and edition ritual | done | Product Engineer | Collection answers ownership questions first; Album has dedicated edition surface; Artist supports rediscovery rails |
| GAP-F03 | Rebuild Playlists, Crates, and Queue for daily-use authorship and sculpting | done | Product Engineer | Authored playlists, first-class crates, queue sculpt actions, queue scene save/restore |
| GAP-F04 | Rebuild Session and Now Playing as the emotional center | done | Product Engineer + Design Engineer | Session memory/replay exists and Now Playing is art-led, provenance-aware, and calmer |
| GAP-F05 | Move automation detail behind a Workstation digest boundary | done | Product Engineer | Main app uses calm digest thresholds while Workstation holds review, replay, diagnostics, import, and history |
| GAP-F06 | Run the final visual system unification pass | done | Design Engineer | Hierarchy, density, spacing, and playback-active shell behavior read as one authored system |

Dependencies:

- `GAP-F00` must land before visual or authorship-heavy rebuilds.
- `GAP-F01` must land before `GAP-F02` through `GAP-F05`.
- `GAP-F06` lands last.

Primary planning docs:

- `docs/SIGNATURE_SURFACES_PLAN.md`
- `docs/EXPERIENCE_BOUNDARY_MAP.md`
- `docs/VISUAL_SYSTEM_DIRECTION.md`
- `docs/OBJECT_MODEL_DECISIONS.md`

Wave 2-4 evidence (2026-04-08):

- `GAP-F02` ownership surfaces: `ui/src/routes/collection/+page.svelte`, `ui/src/routes/albums/[albumId]/+page.svelte`, `ui/src/routes/artists/+page.svelte`, `ui/src/lib/ownership.ts`
- `GAP-F03` daily-use authorship surfaces: `ui/src/routes/playlists/+page.svelte`, `ui/src/routes/crates/+page.svelte`, `ui/src/routes/queue/+page.svelte`, `ui/src/lib/stores/rituals.ts`, `ui/src/lib/queue-ritual.ts`, `ui/src/lib/stores/queue.ts`
- `GAP-F04` emotional surfaces: `ui/src/lib/components/SessionComposer.svelte`, `ui/src/routes/session/+page.svelte`, `ui/src/lib/components/NowPlayingShrine.svelte`, `ui/src/routes/now-playing/+page.svelte`, `ui/src/lib/components/NowPlaying.svelte`
- `GAP-F05` calm automation boundary: `ui/src/lib/automation-digest.ts`, `ui/src/lib/components/AutomationDigestPanel.svelte`, `ui/src/routes/+page.svelte`, `ui/src/lib/components/RightSidebar.svelte`, `ui/src/routes/workstation/+page.svelte`, `ui/src/routes/downloads/+page.svelte`
- `GAP-F06` visual system pass: `ui/src/app.css`, `ui/src/lib/components/Sidebar.svelte`, `ui/src/routes/settings/+page.svelte`, `ui/src/routes/import/+page.svelte`, `ui/src/routes/tools/+page.svelte`
- Verification: `cargo check --workspace`, `cargo test --workspace`, `npm run build`, and `.\scripts\smoke_desktop.ps1` all passed on 2026-04-08

---

## Suggested Execution Packets (Non-Mutually-Exclusive)

Packet 1 (start immediately after Lane 0):

- Lane A: `GAP-A01`, `GAP-A02`
- Lane B: `GAP-B01`
- Lane C: `GAP-C01`

Packet 1 execution mode:

- Run `GAP-A01`, `GAP-A02`, `GAP-B01`, and `GAP-C01` as parallel lanes.
- Do not block one lane on another unless an explicit schema contract collision appears.
- If a collision appears, freeze only the conflicting file set and continue the other lanes.

### Packet 1 Active Checklist

#### GAP-A01 (Release-group planner lane)

- [x] Define planner rationale fields that must include `musicbrainz_release_group_id`.
- [x] Wire release-group field through request-contract boundary and planner output.
- [x] Add regression test proving release-group appears in rationale snapshot.
- [x] Record proof artifact path in this board.

Primary touchpoints:

- `crates/cassette-core/src/acquisition.rs`
- `crates/cassette-core/src/librarian/db/*`
- `src-tauri/src/commands/downloads.rs`

#### GAP-A02 (Canonical edition object)

- [x] Define edition object shape (edition policy + markers + source evidence).
- [x] Persist edition object through planner request/candidate persistence.
- [x] Surface edition object in review payload read path.
- [x] Add regression test proving object persists and round-trips.

Primary touchpoints:

- `crates/cassette-core/src/models/*`
- `crates/cassette-core/src/librarian/db/*`
- `src-tauri/src/commands/downloads.rs`

#### GAP-B01 (Include/exclude grammar)

- [x] Define request grammar contract for explicit include/exclude album targeting.
- [x] Add command-side input validation and ambiguity rejection.
- [x] Add parser/contract tests for valid and invalid forms.
- [x] Add one UI/API example payload in docs.

Primary touchpoints:

- `src-tauri/src/commands/downloads.rs`
- `ui/src/routes/downloads/*`
- `docs/REQUEST_CAPABILITY_MATRIX.md`

#### GAP-C01 (Provider evidence cadence)

- [x] Create evidence table template (provider, scope, date, artifact path, outcome).
- [x] Populate first pass from existing known proofs and bounded probes.
- [x] Mark stale/missing evidence rows explicitly.
- [x] Link evidence table from `TOOL_AND_SERVICE_REGISTRY.md`.

Primary touchpoints:

- `docs/TOOL_AND_SERVICE_REGISTRY.md`
- `docs/PROJECT_STATE.md`
- `docs/TELEMETRY.md`

Packet 1 proof artifacts:

- Code implementation: `src-tauri/src/commands/planner.rs`
- Contract spec: `docs/PACKET_1_CONTRACT_SPEC.md`
- Provider evidence ledger: `docs/PROVIDER_EVIDENCE_LEDGER.md`
- Verification run: `cargo test -p cassette --lib planner` (14 passed, 0 failed, 2026-04-07)

Packet 2:

- Lane A: `GAP-A03`, `GAP-A04`
- Lane B: `GAP-B02`, `GAP-B03`
- Lane C: `GAP-C02`, `GAP-C03`, `GAP-C04`

Packet 2 proof artifacts (current):

- Code implementation: `src-tauri/src/commands/planner.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/commands/dead_letter.rs`
- `GAP-A03`: planner now derives a canonical `identity_envelope_key` and uses it as default `request_signature` for cross-source identity unification.
- `GAP-A04`: exclusion memory now keys off the canonical identity envelope, preserving reuse across equivalent requests and restarts.
- `GAP-B02`: `get_review_contract` command exposes formal pre-acquisition review contract.
- `GAP-B03`: low-trust provider selection now requires explicit `approve_low_trust` approval token in approval notes.
- `GAP-B04`: review preflight contract now evaluates candidate/search readiness, persists `review_preflight` events, surfaces preflight in `get_review_contract`, and blocks approval on failed preflight.
- `GAP-C02`: bounded SAB completion probe runbook and failure taxonomy recorded in `docs/LANE_C_PROBE_RUNBOOK.md`; current state is explicitly constrained as `unverified` when SAB credentials are missing.
- `GAP-C03`: direct LRCLIB probe captured in `docs/probes/lane_c_probe_2026-04-07.json` and classified `bounded-probe`.
- `GAP-C04`: provider status scope vocabulary unified to `local-proven|bounded-probe|unverified` across registry/reference docs.
- Provider reliability cadence hardening (2026-04-07): repeatable snapshot capture added via `scripts/capture_provider_reliability_snapshot.ps1` with timestamped artifacts (`docs/probes/provider_probe_2026-04-07_174445.txt`, `docs/probes/lane_c_probe_2026-04-07_174445.json`) and reflected in `docs/LANE_C_PROBE_RUNBOOK.md` + `docs/PROVIDER_EVIDENCE_LEDGER.md`.
- Planner explainability tightening (2026-04-07): shared preflight reason-code contract now threads through planner candidate-set, review-contract, rationale, and approval error surfaces in `src-tauri/src/commands/planner.rs`.
- `GAP-D01`: normal backlog entrypoint no longer permits non-operator direct Director submission.
- `GAP-D02`: operator-only direct-submit guard enforced via `operator_direct_submit=true` and command-level unit test.
- `GAP-E01`: Decision 36 wording aligned with managed runtime smoke contract in `docs/DECISIONS.md` and `docs/PROJECT_STATE.md`.
- `GAP-E02`: Cover Art Archive registry row now reflects active runtime fallback wiring and canonical status vocabulary.
- `GAP-E03`: docs consistency gate added as `scripts/check_docs_state.ps1` and wired into `scripts/verify_trust_spine.ps1`.
- Verification run: `cargo test -p cassette --lib planner` (17 passed, 0 failed, 2026-04-07)
- Verification run: `cargo test -p cassette --lib planner` (20 passed, 0 failed, 2026-04-07)
- Verification run: `cargo test -p cassette --lib commands::downloads::tests::operator_direct_submit_gate_defaults_to_disabled` (1 passed, 0 failed, 2026-04-07)
- Verification run: `./scripts/check_docs_state.ps1` (all checks passed, 2026-04-07)
- Verification run: `cargo check --workspace` (pass, 2026-04-07)

Packet 3 (requires A+B green):

- Lane D: `GAP-D01`, `GAP-D02`, `GAP-D03`
- Lane E: `GAP-E01`, `GAP-E02`, `GAP-E03`

---

## Completion Criteria (Board-Level)

This board is complete when:

- all `GAP-*` rows are `done`
- every row has evidence links or test artifact references
- canonical docs contain no unresolved contradictions
- legacy compatibility paths are either retired or explicitly operator-only and documented
- provider reliability claims are evidence-backed and scope-labeled
