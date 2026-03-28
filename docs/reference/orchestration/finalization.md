# Finalization
> File placement and deduplication logic that moves validated candidates from temp directories into the organized music library with sanitized paths.

**Status:** Proven Working
**Code:** `crates/cassette-core/src/director/finalize.rs`

## What It Does

The finalization module handles the last mile of the download pipeline: building the canonical library path from track metadata, enforcing duplicate policies, and atomically moving the validated file from its temporary location into the music library. It also provides the replacement comparison logic used when `ReplaceIfBetter` duplicate policy is active.

Path construction follows a strict `{Artist}/{Album}/{prefix} - {Title}.{ext}` convention with all filesystem-unsafe characters sanitized. The module handles cross-filesystem moves gracefully by falling back to copy-then-delete when a direct rename fails (common when temp and library live on different drives).

The replacement logic compares quality tiers (Lossless > Lossy > Unknown), then bitrate, then raw file size, ensuring that an upgrade only happens when the incoming candidate is measurably better than what already exists.

## Key Types

```rust
pub struct FinalizedTrack {
    pub path: PathBuf,
    pub replaced_existing: bool,
    pub provenance: ProvenanceRecord,
}

pub struct ProvenanceRecord {
    pub task_id: String,
    pub source_metadata: NormalizedTrack,
    pub selected_provider: String,
    pub score_reason: SelectionReason,
    pub validation_summary: ValidationReport,
    pub final_path: PathBuf,
    pub acquired_at: DateTime<Utc>,
}

pub struct CandidateSelection {
    pub provider_id: String,
    pub temp_path: PathBuf,
    pub score: CandidateScore,
    pub reason: SelectionReason,
    pub validation: ValidationReport,
    pub cover_art_url: Option<String>,
}

pub enum DuplicatePolicy {
    KeepExisting,      // error if destination file exists
    ReplaceIfBetter,   // overwrite only if incoming candidate wins comparison
}
```

## How It Works

### Path Building (build_final_path)

```rust
pub fn build_final_path(library_root: &Path, target: &NormalizedTrack, extension: &str) -> PathBuf
```

Constructs: `{library_root}/{artist}/{album}/{prefix} - {title}.{ext}`

**Artist resolution:** Uses `album_artist` if present and non-empty, otherwise falls back to `artist`. Empty result becomes `"Unknown Artist"`.

**Album:** Uses `target.album`, defaulting to `"Unknown Album"` if absent or empty.

**Title:** Uses `target.title`, defaulting to `"Unknown Title"` if empty.

**Track prefix:**
- If `track_number` is present and `disc_number > 1`: `"{disc:02}-{track:02}"` (e.g., `02-05`)
- If `track_number` is present and disc is 1 or absent: `"{track:02}"` (e.g., `05`)
- If no track number: `"00"`

**Extension:** Lowercased from the temp file's extension, defaulting to `"bin"`.

### sanitize_component

Replaces these characters with `_`: `/ \ : * ? " < > | \0`

Then trims whitespace and strips trailing dots. This handles Windows filesystem restrictions and prevents path traversal.

### finalize_selected_candidate

```rust
pub async fn finalize_selected_candidate(
    library_root: PathBuf,
    selection: CandidateSelection,
    target: NormalizedTrack,
    duplicate_policy: DuplicatePolicy,
    provenance: ProvenanceRecord,
) -> Result<FinalizedTrack, FinalizationError>
```

Runs in a `spawn_blocking` context:

1. Creates parent directories for the destination path
2. **Duplicate check:**
   - `KeepExisting`: returns `FinalizationError::DestinationExists` if the file already exists
   - `ReplaceIfBetter`: calls `replacement_should_win()` to compare. If the existing file wins, returns `FinalizationError::ReplacementRejected`. Otherwise deletes the existing file and sets `replaced_existing = true`.
3. **Move strategy:** Attempts `std::fs::rename()` first. If rename fails (cross-filesystem), falls back to `std::fs::copy()` followed by `std::fs::remove_file()` on the source.
4. Returns `FinalizedTrack` with the final path, replacement flag, and updated provenance record.

### replacement_should_win

```rust
fn replacement_should_win(existing_path: &Path, selection: &CandidateSelection) -> bool
```

Compares the incoming candidate against the file already at the destination:

1. Reads existing track metadata via `library::read_track_metadata`
2. **Quality tier comparison:** Lossless(2) > Lossy(1) > Unknown(0). If tiers differ, higher tier wins.
3. **Bitrate comparison (same quality):** Incoming bitrate estimated as `file_size * 8 / duration / 1000` kbps. Higher bitrate wins.
4. **File size comparison (same bitrate):** Larger file wins.
5. **No existing metadata available:** Falls back to raw file size comparison (larger wins).

## Configuration

| Setting | Default | Description |
|---|---|---|
| `library_root` | (from DirectorConfig) | Root directory of the organized music library |
| `duplicate_policy` | KeepExisting | Whether to skip or replace existing files |

## Code Map

| File | Role |
|---|---|
| `crates/cassette-core/src/director/finalize.rs` | `build_final_path`, `finalize_selected_candidate`, `replacement_should_win`, `sanitize_component` (191 lines code + 115 lines tests) |
| `crates/cassette-core/src/director/models.rs` | FinalizedTrack, CandidateSelection, ProvenanceRecord, NormalizedTrack, CandidateScore, SelectionReason, ValidationReport |
| `crates/cassette-core/src/director/error.rs` | FinalizationError enum with DestinationExists, ReplacementRejected, MoveFailed |
| `crates/cassette-core/src/director/config.rs` | DuplicatePolicy enum |
| `crates/cassette-core/src/library/mod.rs` | `read_track_metadata` used by replacement_should_win |
| `crates/cassette-core/src/director/engine.rs` | Consumer: calls finalize_selected_candidate in finalize_candidate |
