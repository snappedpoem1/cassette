# yt-dlp
> Audio extraction from YouTube and SoundCloud via the yt-dlp command-line binary

**Status:** Active
**Code:** `crates/cassette-core/src/director/providers/ytdlp.rs`
**Provider ID:** `"yt_dlp"`
**Trust Rank:** 50

## What It Does

The yt-dlp provider wraps the yt-dlp command-line tool to extract audio from YouTube and SoundCloud. It is the simplest remote provider: search returns two hardcoded candidate templates (one for YouTube, one for SoundCloud), and acquisition spawns a yt-dlp subprocess to download and extract audio.

Search does not actually query any service. It first verifies the yt-dlp binary is available by running `--version`, then returns two static candidates with the search queries pre-formatted for yt-dlp's built-in search syntax. The actual search and download happen together during the acquire phase when yt-dlp resolves the search query to a specific video/track.

On Windows, the subprocess is spawned with `CREATE_NO_WINDOW` (creation_flags 0x08000000) to prevent a console window from flashing. Output detection is file-based: after yt-dlp completes, the provider scans the temp directory for the newest file by modification time.

## External Dependencies

| Dependency | Type | Endpoint |
|---|---|---|
| yt-dlp binary | CLI subprocess | Local binary (configurable path, default `"yt-dlp"`) |
| YouTube | Web (via yt-dlp) | `ytsearch1:{query} audio` |
| SoundCloud | Web (via yt-dlp) | `scsearch1:{query}` |

## Authentication & Credentials

No authentication is required. yt-dlp handles any site-specific authentication internally. No cookies, tokens, or API keys are configured.

## Data Flow

### Search
1. Run `yt-dlp --version` to verify binary availability
2. Return 2 hardcoded candidates:
   - `"ytsearch1:{query} audio"` (YouTube) with confidence 0.60
   - `"scsearch1:{query}"` (SoundCloud) with confidence 0.50

### Acquire
1. Spawn yt-dlp subprocess with arguments:
   - `--extract-audio` - extract audio stream
   - `--audio-format best` - best available audio format
   - `--audio-quality 0` - highest quality
   - `--no-playlist` - single item only
   - `--max-downloads 1` - stop after first download
   - `-o {temp_dir}/{stem}.%(ext)s` - output template
2. On Windows: set creation_flags to 0x08000000 (CREATE_NO_WINDOW)
3. Wait for subprocess to complete
4. Scan temp directory for newest file by modification time
5. Return path to detected output file

## Capabilities

- Audio extraction from YouTube
- Audio extraction from SoundCloud
- Best-quality audio selection
- Windowless subprocess on Windows
- No authentication or API keys required

## Configuration

| Setting | Source | Default | Description |
|---|---|---|---|
| Binary path | Provider config | `"yt-dlp"` | Path to the yt-dlp executable |

## Limitations & Known Issues

- Does not support lossless audio (`supports_lossless: false`); YouTube and SoundCloud serve lossy formats
- No batch download support (`supports_batch: false`)
- Search does not actually search; it returns hardcoded templates, so confidence values are static guesses
- Only two sources (YouTube, SoundCloud) despite yt-dlp supporting 1000+ extractors
- Output detection relies on newest file by mtime, which is fragile if other processes write to the temp directory
- No metadata extraction from yt-dlp's `--dump-json` output
- No error parsing from yt-dlp stderr; failures are opaque
- No progress reporting during download

## Untapped Potential

yt-dlp is an extraordinarily capable tool that is barely utilized. It supports over 1000 site extractors including Bandcamp (which has lossless), Vimeo, Dailymotion, and many more. The `--dump-json` flag provides rich metadata (title, artist, album, duration, thumbnail, upload date, description) that could feed the metadata pipeline. Format selection (`--format`) could target specific quality levels. Other unused features: SponsorBlock integration for skipping non-music segments, chapter extraction, thumbnail download, subtitle/lyrics download, cookie-based authentication for premium content, rate limiting, proxy support, and output templates with metadata-derived filenames. Bandcamp support alone would add a legitimate lossless source.

## Code Map

| File | Role |
|---|---|
| `crates/cassette-core/src/director/providers/ytdlp.rs` | Full provider implementation: binary verification, candidate generation, subprocess management, output detection |
