# Strategy Planner
> Maps acquisition strategies to concrete provider orderings, selection modes, and quality requirements.

**Status:** Proven Working
**Code:** `crates/cassette-core/src/director/strategy.rs`

## What It Does

The StrategyPlanner translates a high-level `AcquisitionStrategy` (attached to each TrackTask) into a concrete `StrategyPlan` that the Director Engine uses to drive the waterfall. Each strategy defines which providers to try first, whether to finalize on the first valid candidate or collect all candidates for comparison, and whether lossless quality is mandatory.

The planner is stateless (`#[derive(Default, Clone)]`) and takes no configuration of its own. It receives the task, the list of available provider descriptors, and the director config (currently unused but reserved for future tuning). Provider ordering is determined by hard-coded priority maps per strategy, with a fallback to `trust_rank` sort for unrecognized strategies.

## Key Types

```rust
#[derive(Debug, Clone)]
pub struct StrategyPlan {
    pub strategy: AcquisitionStrategy,
    pub provider_order: Vec<String>,           // provider IDs in waterfall order
    pub collect_multiple_candidates: bool,     // gather all or stop at first valid
    pub selection_mode: CandidateSelectionMode, // FirstValidWins | CompareAllCandidates
    pub require_lossless: bool,                // reject non-lossless candidates
}

#[derive(Debug, Default, Clone)]
pub struct StrategyPlanner;

// CandidateSelectionMode (from models.rs):
// - FirstValidWins: finalize immediately on first valid candidate
// - CompareAllCandidates: collect all valid candidates, pick highest score
```

## How It Works

### Provider Ordering by Strategy

Each strategy sorts the provider descriptor list by a hard-coded priority map. Lower numbers mean higher priority.

**Standard / DiscographyBatch** -- Mainstream albums, quality-first:
| Priority | Provider |
|---|---|
| 0 | Qobuz |
| 1 | Deezer |
| 2 | Local Archive |
| 3 | Usenet |
| 4 | Real Debrid |
| 5 | Slskd |
| 6 | yt-dlp |

**HighQualityOnly / RedownloadReplaceIfBetter** -- Lossless only, yt-dlp excluded:
| Priority | Provider |
|---|---|
| 0 | Qobuz |
| 1 | Deezer |
| 2 | Usenet |
| 3 | Local Archive |
| 4 | Real Debrid |
| 5 | Slskd |
| 99 | yt-dlp (effectively excluded by require_lossless) |

**ObscureFallbackHeavy** -- Rare/out-of-print, deep catalog promoted:
| Priority | Provider |
|---|---|
| 0 | Local Archive |
| 1 | Real Debrid |
| 2 | Slskd |
| 3 | Usenet |
| 4 | Deezer |
| 5 | Qobuz |
| 6 | yt-dlp |

**SingleTrackPriority** -- Individual tracks/remixes, yt-dlp promoted for community content:
| Priority | Provider |
|---|---|
| 0 | Deezer |
| 1 | Qobuz |
| 2 | yt-dlp |
| 3 | Slskd |
| 4 | Real Debrid |
| 5 | Local Archive |

**Default (MetadataRepairOnly, etc.)** -- Falls back to `trust_rank` sort order from provider descriptors.

### Selection Modes

| Strategy | Selection Mode |
|---|---|
| Standard | FirstValidWins |
| SingleTrackPriority | FirstValidWins |
| HighQualityOnly | CompareAllCandidates |
| DiscographyBatch | CompareAllCandidates |
| RedownloadReplaceIfBetter | CompareAllCandidates |
| ObscureFallbackHeavy | CompareAllCandidates |

### collect_multiple_candidates

Set to `true` for: HighQualityOnly, DiscographyBatch, RedownloadReplaceIfBetter, ObscureFallbackHeavy.

Set to `false` (implicitly) for: Standard, SingleTrackPriority.

When false and selection mode is FirstValidWins, the waterfall finalizes immediately on the first valid candidate from the highest-priority provider.

### require_lossless

Set to `true` for: HighQualityOnly, RedownloadReplaceIfBetter.

When enabled, the engine quarantines any candidate that does not validate as `CandidateQuality::Lossless` and continues to the next provider or candidate.

## Configuration

| Setting | Default | Description |
|---|---|---|
| `task.strategy` | Standard | The AcquisitionStrategy attached to each TrackTask |

The planner itself has no configurable settings. Provider ordering is determined entirely by the strategy. The `_config: &DirectorConfig` parameter is accepted but currently unused, reserved for future per-strategy tuning knobs.

## Code Map

| File | Role |
|---|---|
| `crates/cassette-core/src/director/strategy.rs` | StrategyPlanner and StrategyPlan (120 lines code + 129 lines tests) |
| `crates/cassette-core/src/director/models.rs` | AcquisitionStrategy enum, CandidateSelectionMode enum, ProviderDescriptor struct |
| `crates/cassette-core/src/director/engine.rs` | Consumer: calls `planner.plan()` in `process_task`, uses plan fields throughout waterfall |
