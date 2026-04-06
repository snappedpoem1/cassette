# Cassette Music Operating System Implementation Plan

Last updated: 2026-04-06
Owner: Christian (single-owner personal project)

## Purpose

Meld current Cassette strengths (trust, pipeline depth, recovery, and music-first UX) into one integrated operating model:

- one identity spine
- one explainability spine
- one automation policy spine
- one appreciation surface

Target: not "player plus downloader," but a personal Music OS where discovery, acquisition, playback, stewardship, and beauty behave as one system.

## Operating Laws

1. Five-Use Multiplier Rule

Any new capability should serve at least five useful surfaces when practical, for example:

- runtime decision quality
- user explainability
- planner optimization
- telemetry and regression detection
- recovery and post-mortem analysis

If a capability cannot provide broad leverage, treat it as optional or experimental.

2. Isolation-When-Harmful Rule

If a capability adds instability, jitter, or trust risk, isolate it behind:

- feature flags
- strict timeout budgets
- bounded queue depth
- deterministic fallback path
- explicit reason-code telemetry

3. Deterministic Core Rule

Acquisition closure, queue claims, file placement, and audit lineage are always deterministic and never delegated to experimental lanes.

4. Explainability-First Rule

Each major mutation must answer in plain language:

- what happened
- why this path was chosen
- what alternatives were rejected
- what the user can do next

## Integration Graph (Where New Things Connect)

1. Trust Ledger and Explainability Spine

Connects to:

- Director candidate scoring and finalization
- Planner rationale surfaces
- Downloads lanes (blocked and completed detail)
- Home "while you were away" narrative
- Audit and validation tooling

Reused for:

- provenance cards
- replacement rationale
- dead-letter diagnostics
- policy tuning
- support and debugging

2. Edition Intelligence Engine

Connects to:

- MusicBrainz canonical identity tables
- request contract and planner filters
- replacement policy in Director
- metadata repair and tag enrichment
- library browse faceting

Reused for:

- upgrade candidate ranking
- clean versus explicit preference
- remaster versus original preference
- conflict resolution
- quality arbitration context

3. Quality Arbitration Layer

Connects to:

- candidate scoring in Director
- gatekeeper admit or quarantine
- replacement decisions
- telemetry regression budgets
- user-facing confidence labels

Reused for:

- transcode suspicion control
- fallback quality policy
- post-acquisition trust scoring
- policy mode behavior
- automated retries

4. Visual and Appreciation Engine

Connects to:

- now-playing surface
- lyrics sync cache and rendering
- color extraction and dynamic theming
- visualizer mode registry
- audio feature backfill outputs

Reused for:

- waveform and seek UX
- spectrum and spectrogram modes
- session mood transitions
- accessibility-friendly low-motion views
- branded Music OS identity

5. Recovery and Dead-Letter Spine

Connects to:

- delta_queue and pending tasks
- provider-specific retry policies
- blocked lane UX
- scheduler windows
- trust and telemetry evidence

Reused for:

- unattended overnight runs
- clean restart continuity
- deterministic reprocessing
- provider health adaptation
- intervention minimization

## Ordered Implementation Program

## Stage A: Contract and Data Foundations (start here)

1. Trust Ledger v1

Implement normalized mutation evidence rows and reason codes for planner, director, and gatekeeper outcomes.

Acceptance:

- one query can reconstruct end-to-end request outcome
- Home and Downloads can render plain-language rationale cards
- telemetry includes reason-code distribution

2. Edition Intelligence v1

Thread release-group and edition markers through request contract, planner, and runtime track records.

Acceptance:

- planner can filter by edition preference policy
- replacement path can explain "why this edition won"
- edition fields are visible in track inspection UI

3. Policy Profile System

Implement explicit profiles: Playback-First, Balanced Auto, Aggressive Overnight.

Acceptance:

- profile switch alters bounded concurrency, retry cadence, and enrichment scheduling
- profile changes are logged and explainable
- deterministic defaults are preserved after restart

## Stage B: Autonomous Intelligence and Reliability

4. Adaptive Provider Orchestrator

Use persisted outcome memory to tune provider search order by context while preserving deterministic tie-breakers.

Acceptance:

- provider ordering adapts within bounded policy rules
- stale or low-confidence memory never overrides hard identity constraints
- explainability reflects adaptive decisions

5. Dead-Letter Command Center

Add a first-class dead-letter lane with grouped failure classes, suggested fixes, and one-click replay paths.

Acceptance:

- permanently failed tasks are isolated from active queue lanes
- each dead-letter item includes next best action
- replay path preserves original request signature and lineage

6. Long-Session Reliability Harness

Add repeatable 8 to 24 hour soak scenarios for playback plus background acquisition plus scan cycles.

Acceptance:

- soak report captures underruns, memory drift, queue drift, and provider failure envelopes
- regression gates can fail on soak degradation
- known instability signatures are tracked in telemetry

## Stage C: Appreciation and Beauty Layer

7. Visualizer and Appreciation Stack v1

Ship waveform plus spectrum modes first, then optional high-fidelity shader mode.

Acceptance:

- visualizer frame budget does not regress playback stability
- low-motion and reduced-effects modes exist
- visualizer can be disabled globally without affecting playback

8. Dynamic Glass and Mood System

Add adaptive theme mooding from artwork and audio features with strict fallback.

Acceptance:

- system supports static fallback theme when effects unavailable
- contrast and readability pass accessibility checks
- background effects are bounded for CPU and GPU use

9. Session Composer

Generate listening arcs using key, BPM, energy slope, and personal history.

Acceptance:

- generated sessions include explainable transition logic
- skip and replay feedback updates future arc generation
- sessions can be saved as reusable modes

## Stage D: Platform and Extension Model

10. Safe Extension Surface

Define a capability-scoped extension model for visual packs, enrichers, and provider adapters.

Acceptance:

- extension capabilities are explicit and sandboxed
- extension failures are isolated from deterministic core lanes
- extension telemetry and health are surfaced in settings

## Reuse and Isolation Matrix

When adding a new capability, evaluate both gates before merge:

1. Reuse gate

Does this capability directly improve at least five of the following:

- planner quality
- acquisition quality
- playback or UX quality
- explainability
- telemetry and regression detection
- recovery and retries
- settings or policy controls
- troubleshooting visibility

2. Isolation gate

Can this capability fail without harming:

- playback continuity
- deterministic queue closure
- file safety and finalization integrity
- audit lineage completeness

If no, isolate or defer.

## Core Dependency Order

Implement in this order to avoid backtracking:

1. Trust Ledger v1
2. Edition Intelligence v1
3. Policy Profiles
4. Adaptive Provider Orchestrator
5. Dead-Letter Command Center
6. Long-Session Reliability Harness
7. Visualizer and Appreciation Stack v1
8. Dynamic Glass and Mood System
9. Session Composer
10. Safe Extension Surface

## Verification Contract

For each stage:

1. targeted tests for changed modules
2. baseline commands pass
3. one deterministic failure-path proof captured
4. docs updated in TODO, HIT_LIST, PROJECT_STATE, TELEMETRY, and DECISIONS where applicable

## Baseline Commands

```powershell
cargo check --workspace
cargo test --workspace
Set-Location ui; npm install; npm run build; Set-Location ..
.\scripts\smoke_desktop.ps1
```

## Near-Term Bootstrap (First Two Weeks)

1. Stand up Trust Ledger v1 schema plus query surface.
2. Add edition markers to request contract and planner filters.
3. Expose policy profile switch in settings and log profile changes.
4. Add dead-letter lane skeleton in Downloads with failure-class grouping.
5. Capture first long-session soak report baseline.

This bootstrap creates the minimum Music OS spine that later visual and intelligence layers can safely attach to.
