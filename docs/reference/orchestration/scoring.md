# Candidate Scoring
> Six-factor scoring system that ranks download candidates by metadata accuracy, duration match, codec quality, provider trust, validation status, and file size.

**Status:** Proven Working
**Code:** `crates/cassette-core/src/director/scoring.rs`

## What It Does

The scoring module provides a single pure function, `score_candidate`, that evaluates a download candidate against the target track and returns a numeric score breakdown plus a human-readable selection reason. The Director Engine uses these scores to decide which candidate to finalize -- either immediately (FirstValidWins) or after comparing all collected candidates (CompareAllCandidates).

The scoring is deterministic and stateless. Given the same inputs, it always produces the same score. Each of the six factors maps to a specific quality signal, and the total score is a simple sum of all factor points. Higher is better. The system is designed so that a perfect lossless match from a trusted provider with exact duration scores well above 100, while an invalid candidate with wrong duration scores deeply negative.

## Key Types

```rust
pub struct CandidateScore {
    pub total: i32,
    pub metadata_match_points: i32,
    pub duration_points: i32,
    pub codec_points: i32,
    pub provider_points: i32,
    pub validation_points: i32,
    pub size_points: i32,
}

pub struct SelectionReason {
    pub summary: String,             // e.g. "Selected Song via Qobuz with score 110"
    pub details: BTreeMap<String, String>,  // metadata_confidence, provider, quality, file_size, score_total
}
```

## How It Works

### Function Signature

```rust
pub fn score_candidate(
    target: &NormalizedTrack,
    provider: &ProviderDescriptor,
    candidate: &ProviderSearchCandidate,
    validation: &ValidationReport,
    quality_policy: &QualityPolicy,
) -> (CandidateScore, SelectionReason)
```

### Factor Breakdown

**1. metadata_match_points (0 to 40)**

`candidate.metadata_confidence * 40`, clamped to `[0.0, 1.0]` before multiplication, cast to i32. A confidence of 0.98 yields 39 points. This is the highest-weighted positive factor, reflecting the importance of getting the right track.

**2. duration_points (-25 to +25)**

Compares `target.duration_secs` against `validation.duration_secs`:
- Delta <= 1.5 seconds: **+25** (near-exact match)
- Delta within `quality_policy.max_duration_delta_secs`: **+10** (acceptable tolerance)
- Delta exceeds max: **-25** (likely wrong track or bad edit)
- Either duration unknown: **0** (no penalty, no reward)

**3. codec_points (0 to 20)**

Based on `validation.quality`:
- `Lossless`: **+20**
- `Lossy`: **+5**
- `Unknown`: **0**

**4. provider_points (0 to 20)**

`(20 - provider.trust_rank).max(0)`. Providers with lower `trust_rank` values score higher. A provider with `trust_rank=1` gets 19 points; `trust_rank=20` or higher gets 0.

**5. validation_points (-50 to +20)**

- Valid (`is_valid=true`): **+20**
- Invalid (`is_valid=false`): **-50** (heavy penalty to prevent selecting broken files)

**6. size_points (0 to 5)**

- File size > 5 MB: **+5**
- File size <= 5 MB: **0**

This is a minor tiebreaker that slightly favors substantive files over suspiciously small ones.

### Score Ranges

| Scenario | Approximate Score |
|---|---|
| Perfect lossless match, trusted provider | 100-130 |
| Good lossy match, mid-tier provider | 55-75 |
| Metadata mismatch, wrong duration | -30 to 0 |
| Invalid candidate | Below -30 |

### SelectionReason Details

The `details` BTreeMap always contains five keys:
- `metadata_confidence`: formatted to 2 decimal places
- `provider`: provider ID string
- `quality`: debug-formatted CandidateQuality enum
- `file_size`: raw byte count as string
- `score_total`: total score as string

## Configuration

| Setting | Default | Description |
|---|---|---|
| `quality_policy.max_duration_delta_secs` | None | Maximum acceptable duration difference in seconds |
| `quality_policy.minimum_duration_secs` | (set in validation) | Not used directly in scoring, but affects validation.is_valid |

The scoring weights (40, 25, 20, etc.) are hard-coded constants. There are no user-configurable weight overrides.

## Code Map

| File | Role |
|---|---|
| `crates/cassette-core/src/director/scoring.rs` | `score_candidate` function (78 lines code + 57 lines tests) |
| `crates/cassette-core/src/director/models.rs` | CandidateScore, SelectionReason, CandidateQuality, ProviderDescriptor, ProviderSearchCandidate, ValidationReport, NormalizedTrack |
| `crates/cassette-core/src/director/config.rs` | QualityPolicy with max_duration_delta_secs |
| `crates/cassette-core/src/director/engine.rs` | Consumer: calls score_candidate in try_provider and finalize_candidate |
