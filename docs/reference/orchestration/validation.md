# Audio Validation
> Symphonia-based audio validation pipeline that verifies file integrity, detects format by magic bytes, checks duration plausibility, and classifies codec quality.

**Status:** Proven Working
**Code:** `crates/cassette-core/src/director/validation.rs`

## What It Does

The validation module provides a single async entry point, `validate_candidate`, that takes a downloaded file path, the target track metadata, and a quality policy, then produces a comprehensive `ValidationReport` or a specific `ValidationError`. It is called by the Director Engine after every successful file acquisition and before scoring.

Validation runs as a blocking task via `tokio::task::spawn_blocking` since it performs synchronous file I/O and Symphonia decoding. The pipeline performs five sequential checks: file size sanity, HTML payload detection (common CDN error pages), minimum size enforcement, Symphonia container probing, and format signature verification. If all checks pass, it then evaluates duration plausibility and classifies the audio quality tier.

The module is intentionally strict. Any file that fails the hard checks (empty, HTML, too small, unreadable container, extension mismatch) returns an error immediately. Duration and quality issues are recorded as `ValidationIssue` entries, and the report's `is_valid` field is false if any issues exist.

## Key Types

```rust
pub struct ValidationReport {
    pub is_valid: bool,
    pub format_name: Option<String>,
    pub duration_secs: Option<f64>,
    pub audio_readable: bool,
    pub header_readable: bool,
    pub extension_ok: bool,
    pub file_size: u64,
    pub quality: CandidateQuality,
    pub issues: Vec<ValidationIssue>,
}

pub struct ValidationIssue {
    pub code: String,     // e.g. "duration_too_short", "duration_mismatch", "duration_unknown"
    pub message: String,
}

pub enum CandidateQuality {
    Lossless,  // flac, wav, aiff, wv, ape
    Lossy,     // mp3, aac, m4a, ogg, opus
    Unknown,   // unrecognized extension
}

// ValidationError variants:
// EmptyFile, HtmlPayload, Rejected { message },
// UnreadableContainer { message }, ExtensionMismatch { expected, actual },
// ImplausibleDuration { message }
```

## How It Works

### Async Entry Point

```rust
pub async fn validate_candidate(
    path: PathBuf,
    target: NormalizedTrack,
    quality_policy: QualityPolicy,
) -> Result<ValidationReport, ValidationError>
```

Spawns `validate_candidate_blocking` via `tokio::task::spawn_blocking`.

### Validation Steps (sequential)

**Step 1 -- File size check:**
Read file metadata. If size is 0 bytes, return `ValidationError::EmptyFile`.

**Step 2 -- HTML payload sniff:**
Read the first 256 bytes of the file. Convert to lowercase string. If it contains `<html` or `<!doctype html`, return `ValidationError::HtmlPayload`. This catches CDN error pages and gateway timeouts that providers sometimes return as the file body.

**Step 3 -- Minimum size enforcement:**
If file size is less than 1024 bytes (1 KB), return `ValidationError::Rejected` with a descriptive message.

**Step 4 -- Symphonia probe:**
Open the file as a `MediaSourceStream`. Create a `Hint` from the file extension. Call `symphonia::default::get_probe().format()` to probe the container. If probing fails, return `ValidationError::UnreadableContainer`. Extract the default audio track. Compute duration as `n_frames / sample_rate` (both must be non-zero).

**Step 5 -- Signature format detection:**
Detect the actual format from the first bytes read in Step 2 using magic byte signatures:

| Magic Bytes | Detected Format |
|---|---|
| `fLaC` at offset 0 | flac |
| `ID3` at offset 0 | mp3 |
| `OggS` at offset 0 | ogg (or opus if `OpusHead` at bytes 28-36) |
| `RIFF` at 0 + `WAVE` at 8 | wav |
| `ftyp` at bytes 4-8 | m4a |

If a signature is detected and it does not match the file extension, return `ValidationError::ExtensionMismatch`.

**Step 6 -- Duration validation:**
If duration was extracted:
- If duration <= 0 or less than `quality_policy.minimum_duration_secs`: issue `duration_too_short`
- If target has a known duration and `max_duration_delta_secs` is configured, and the delta exceeds the maximum: issue `duration_mismatch`

If duration could not be extracted: issue `duration_unknown`.

**Step 7 -- Quality classification by extension:**
| Extensions | Quality |
|---|---|
| flac, wav, aiff, wv, ape | Lossless |
| mp3, aac, m4a, ogg, opus | Lossy |
| anything else | Unknown |

### Report Finalization

`is_valid` is true only when the `issues` list is empty. If issues exist, the function returns `Err(ValidationError::ImplausibleDuration)` with all issue messages joined by semicolons.

## Configuration

| Setting | Default | Description |
|---|---|---|
| `quality_policy.minimum_duration_secs` | (from config) | Minimum acceptable track duration in seconds |
| `quality_policy.max_duration_delta_secs` | None | Maximum acceptable difference between target and actual duration |

## Code Map

| File | Role |
|---|---|
| `crates/cassette-core/src/director/validation.rs` | `validate_candidate`, `validate_candidate_blocking`, `detect_signature_format` (163 lines code + 56 lines tests) |
| `crates/cassette-core/src/director/models.rs` | ValidationReport, ValidationIssue, CandidateQuality, NormalizedTrack |
| `crates/cassette-core/src/director/error.rs` | ValidationError enum with EmptyFile, HtmlPayload, Rejected, UnreadableContainer, ExtensionMismatch, ImplausibleDuration |
| `crates/cassette-core/src/director/config.rs` | QualityPolicy struct |
| `crates/cassette-core/src/director/engine.rs` | Consumer: calls validate_candidate in try_provider after file acquisition |
