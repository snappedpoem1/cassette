# Local Archive
> Filesystem-based audio discovery and acquisition from local directory roots with substring matching

**Status:** Active
**Code:** `crates/cassette-core/src/director/providers/local_archive.rs`
**Provider ID:** `"local_archive"`
**Trust Rank:** 0 (highest trust)

## What It Does

Local Archive is the highest-trust provider, scanning configured local directories for audio files that match the requested artist and title. It is the only provider that supports batch operations, and the simplest in terms of acquisition: a direct file copy with no transformation, decryption, or network activity.

Search runs a blocking directory walk (`walkdir` via `spawn_blocking`) over all configured root paths, checking each file against a normalized substring match. Both the query (artist + title) and the filename are reduced to lowercase alphanumeric characters, and the query terms are checked as substrings of the filename. Any audio file that matches receives a flat confidence of 0.85. File type detection uses `is_audio_path()` to check extensions.

Acquisition is a simple `tokio::fs::copy` from the source path to the temp directory. No processing, conversion, or validation is applied beyond the copy itself.

## External Dependencies

| Dependency | Type | Endpoint |
|---|---|---|
| Local filesystem | Disk | Configured root directories |

## Authentication & Credentials

None. Local filesystem access only.

## Data Flow

### Search
1. For each configured root path:
   - `spawn_blocking` with `walkdir` directory traversal
   - For each file: check `is_audio_path()` for valid audio extension
   - Normalize filename to lowercase alphanumeric
   - Normalize artist + title query to lowercase alphanumeric
   - Check if query terms are substrings of normalized filename
   - Matching files get confidence 0.85
2. Return all matching candidates

### Acquire
1. `tokio::fs::copy` from source path to temp directory
2. No transformation or processing applied

## Capabilities

- Highest trust rank (0) - results from local archive are preferred over all remote providers
- Batch support (`supports_batch: true`) - the only provider with this capability
- Lossless support - preserves original file format
- Zero network dependency
- No authentication required
- Instant acquisition (local copy only)

## Configuration

| Setting | Source | Default | Description |
|---|---|---|---|
| `roots` | Provider config | None | `Vec<PathBuf>` of directories to scan for audio files |

## Limitations & Known Issues

- Substring matching on filenames is imprecise; "art" would match "artist" and "heart"
- Fixed confidence of 0.85 for all matches regardless of match quality
- No metadata reading from file tags; matching is filename-only
- Full directory walk on every search; no indexing or caching
- `spawn_blocking` walkdir can be slow on large directory trees or network-mounted storage
- No quality or format scoring; a 128kbps MP3 ranks equal to a 24-bit FLAC
- No duplicate detection across roots

## Untapped Potential

The local archive could be significantly smarter. Reading existing ID3/Vorbis/FLAC tags would enable matching on artist, album, track number, and title metadata rather than just filenames. File quality detection (bitrate, sample rate, bit depth) could feed into scoring so lossless files rank higher than lossy ones. An in-memory index built on first scan (or persisted to disk) would make subsequent searches near-instant instead of re-walking the entire directory tree. Duplicate detection across roots could surface redundant copies. Content hashing could enable deduplication with the main library.

## Code Map

| File | Role |
|---|---|
| `crates/cassette-core/src/director/providers/local_archive.rs` | Full provider implementation: directory walking, substring matching, file copy |
