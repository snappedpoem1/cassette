# Lofty
> Audio metadata tag reading and writing for post-acquisition tagging, tag correction, and library scanning
**Status:** Proven Working
**Code:** `crates/cassette-core/src/metadata.rs`, `crates/cassette-core/src/library/mod.rs`, `crates/cassette-core/src/librarian/scanner/audio.rs`, `crates/cassette-core/src/director/validation.rs`

## What It Does
Handles all audio file tag operations across four contexts:
1. **Post-acquisition tagging:** `apply_metadata()` writes tags to newly downloaded files including artist, album, title, track/disc numbers, year, provenance comment, and embedded cover art.
2. **Tag fix application:** `apply_tag_fix()` writes MusicBrainz-sourced corrections to existing files.
3. **Library scanning:** Reads tags from audio files during library scan to populate track metadata.
4. **Quality comparison:** `read_track_metadata()` used by `replacement_should_win()` during finalization to compare existing vs new file quality.

## External Dependencies
- **lofty** Rust crate (tag reading/writing across formats)
- No external services or network calls (cover art download uses reqwest separately)

## Authentication & Credentials
None. Lofty is a local-only library.

## Data Flow
### Post-Acquisition Tagging (`apply_metadata`)
1. Open file with `Probe`, get `primary_tag_mut()` or fall back to `first_tag_mut()`
2. Set fields: artist, album, title, track_number (`set_track`), disc_number (`set_disk`), year (`set_year`)
3. Set comment field with provenance string: provider name, task_id, score
4. If `cover_art_url` provided: download image (max 15MB)
5. Detect image format: `0x89PNG` header -> PNG, `0xFF 0xD8 0xFF` header -> JPEG
6. Embed as `PictureType::CoverFront`
7. Save with `save_to_path` using `WriteOptions::default()`
8. Runs as spawned blocking task

### Tag Fix Application (`apply_tag_fix`)
1. Open file with `Probe`
2. Match fix field name: title, artist, album, year, track_number, album_artist
3. Set corresponding tag field
4. Save with `WriteOptions::default()`

### Library Scanning
1. Scanner walks library directory
2. Each audio file opened with lofty
3. Tags read: artist, album, title, track number, disc number, year, genre, etc.
4. Metadata populated into library track records

## Capabilities
- Read and write tags across all major audio formats
- Embed cover art (PNG and JPEG detection by magic bytes)
- Provenance tracking via comment field
- Field-level tag correction
- Primary tag type selection with fallback
- Quality metadata reading for replacement decisions

## Limitations & Known Issues
- Cover art capped at 15MB (large high-res scans may be rejected)
- Image format detection only handles PNG and JPEG (no WebP, BMP, etc.)
- Only writes basic fields; genre, composer, BPM, and other extended fields are not set during acquisition
- Uses `WriteOptions::default()` which may not preserve all existing tag data in edge cases
- No tag type preference logic (e.g., preferring ID3v2.4 over ID3v2.3)
- Provenance comment overwrites any existing comment field

## Untapped Potential
Lofty supports a much broader feature set than currently used:
- All tag formats: ID3v2 (all versions), Vorbis Comments, MP4 Atoms (iTunes), APE tags
- Extended fields: genre, BPM, composer, lyrics (embedded), conductor, copyright
- ReplayGain tags (read/write)
- Custom/user-defined tag frames
- Tag removal and cleanup
- Tag format conversion (e.g., ID3v2.3 to ID3v2.4)
- Multiple picture management (back cover, artist photo, booklet, etc.)
- Picture type enumeration beyond CoverFront
- Tag preservation during format operations

## Code Map
| Symbol | Location | Purpose |
|---|---|---|
| `apply_metadata()` | `crates/cassette-core/src/metadata.rs` | Post-download tagging + cover art embedding |
| `apply_tag_fix()` | `crates/cassette-core/src/metadata.rs` | MusicBrainz tag correction writer |
| `read_track_metadata()` | `crates/cassette-core/src/metadata.rs` | Tag reading for quality comparison |
| Library scanner | `crates/cassette-core/src/library/mod.rs` | Tag reading during library scan |
| Audio scanner | `crates/cassette-core/src/librarian/scanner/audio.rs` | Tag extraction for track records |
| `replacement_should_win()` | `crates/cassette-core/src/director/validation.rs` | Quality comparison via tag metadata |
