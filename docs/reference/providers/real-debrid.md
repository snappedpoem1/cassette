# Real-Debrid
> Torrent-based lossless music acquisition via Real-Debrid cloud downloading with Pirate Bay search and instant availability caching

**Status:** Active
**Code:** `crates/cassette-core/src/director/providers/real_debrid.rs`
**Provider ID:** `"real_debrid"`
**Trust Rank:** 80

## What It Does

Real-Debrid is a cloud torrent provider that searches The Pirate Bay for FLAC music torrents, checks Real-Debrid's cache for instant availability, and uses Real-Debrid's infrastructure to download and unrestrict torrent content. It then downloads the resolved files locally and selects the best audio match.

The provider has sophisticated quality filtering through `SeedingQualifiers`: minimum seeder requirements, maximum torrent size limits, preferred format patterns, and reject patterns to filter out low-quality or non-audio content. Cached torrents (instantly available on Real-Debrid's servers) are prioritized over uncached ones to minimize wait times.

The full acquisition pipeline handles magnet submission, file selection, download polling (up to 10 minutes), link unrestriction, file download, optional archive extraction (7z for zip/rar/7z), and intelligent audio file selection from the results. This makes it the provider with the longest and most complex acquire flow.

## External Dependencies

| Dependency | Type | Endpoint |
|---|---|---|
| The Pirate Bay Search | REST | `GET https://apibay.org/q.php` |
| Real-Debrid Instant Availability | REST | `GET https://api.real-debrid.com/rest/1.0/torrents/instantAvailability/{hashes}` |
| Real-Debrid Add Magnet | REST | `POST https://api.real-debrid.com/rest/1.0/torrents/addMagnet` |
| Real-Debrid Select Files | REST | `POST https://api.real-debrid.com/rest/1.0/torrents/selectFiles/{id}` |
| Real-Debrid Torrent Info | REST | `GET https://api.real-debrid.com/rest/1.0/torrents/info/{id}` |
| Real-Debrid Unrestrict Link | REST | `POST https://api.real-debrid.com/rest/1.0/unrestrict/link` |
| 7z binary | CLI | Archive extraction for zip/rar/7z files |

## Authentication & Credentials

Authentication uses a Bearer token passed as a default header on the reqwest client. The API key is configured once and used for all Real-Debrid API calls. The client is configured with a 60-second request timeout.

No authentication is needed for The Pirate Bay search (apibay.org). A spoofed `User-Agent: Mozilla/5.0` is used for Pirate Bay requests.

## Data Flow

### Search
1. `GET https://apibay.org/q.php?q={encoded}+FLAC&cat={cat}` with User-Agent Mozilla/5.0
2. Try category 104 (FLAC) first, then category 101 (Music)
3. Construct magnet links from info_hash with 3 tracker URLs
4. Filter by SeedingQualifiers: min 3 seeders, max 10GB, reject patterns `[mp3 128, mp3 192, web-dl, video]`
5. Score: artist terms +20pts, title terms +30pts, format bonus +50pts (for preferred format matches), seeder bonus +2/seeder (max 50)
6. Check instant availability: `GET /rest/1.0/torrents/instantAvailability/{hashes}` (batch, up to ~40 hashes)
7. Response format: `{HASH: {rd: [{file_id: {filename, filesize}}]}}`
8. Sort cached torrents first, then by score

### Acquire
1. Check instant availability for the selected torrent
2. `POST /rest/1.0/torrents/addMagnet` with `magnet={magnet}` (form-encoded)
3. `POST /rest/1.0/torrents/selectFiles/{id}` with `files=all` (form-encoded)
4. Poll `GET /rest/1.0/torrents/info/{id}` up to 120 times at 5-second intervals (10 minutes max)
5. Wait for status: `downloaded`, or abort on `error`/`dead`/`virus`
6. For each resolved link: `POST /rest/1.0/unrestrict/link` with `link={link}` → returns download URL, filename, filesize
7. Download each file to `rd-download/` subdirectory
8. Attempt 7z extraction for zip/rar/7z archives
9. `find_best_audio_file()`: walkdir scoring - base 10, artist match +20, title match +30, FLAC +50
10. Copy best match to temp directory

## Capabilities

- Lossless torrent acquisition with quality filtering
- Instant availability checking (batch, up to ~40 hashes per request)
- Automatic archive extraction (7z for zip/rar/7z)
- Intelligent audio file selection from multi-file torrents
- Seeder-based quality scoring
- Reject pattern filtering to exclude low-quality or non-audio content
- Cached torrent prioritization for faster downloads

## Configuration

| Setting | Source | Default | Description |
|---|---|---|---|
| `api_key` | Provider config | None | Real-Debrid API key (Bearer token) |

### Hardcoded Qualifiers (SeedingQualifiers)

| Qualifier | Value | Description |
|---|---|---|
| `min_seeders` | 3 | Minimum seeders required |
| `max_torrent_size` | 10 GB | Maximum torrent size |
| `prefer_formats` | `[flac, 24bit, 24-bit, lossless]` | Format patterns that receive score bonuses |
| `reject_patterns` | `[mp3 128, mp3 192, web-dl, video]` | Patterns that cause immediate rejection |

## Limitations & Known Issues

- Pirate Bay search is the only torrent source; no multi-indexer support
- 10-minute download timeout may be insufficient for large uncached torrents
- `files=all` is always selected; no intelligent file selection to avoid downloading video or non-audio content
- 7z binary must be available on the system PATH for archive extraction
- Error mapping is coarse: 401/403 → AuthFailed, 429 → RateLimited, everything else → Network
- No batch download support (`supports_batch: false`)
- SeedingQualifiers are hardcoded, not configurable
- User-Agent spoofing for Pirate Bay may break if they implement bot detection

## Untapped Potential

The Real-Debrid API supports several unused features: a hosters list endpoint for direct link debridding (not just torrents), traffic/quota information for usage monitoring, user info for account status, device-based OAuth authentication as an alternative to API keys, streaming/transcoding endpoints for direct playback, downloads history management, and torrent deletion for cleanup. Currently only the torrent path is used; hoster link unrestriction could open up direct download sites as sources. Traffic/quota checking could prevent failures when the account is exhausted.

## Code Map

| File | Role |
|---|---|
| `crates/cassette-core/src/director/providers/real_debrid.rs` | Full provider implementation: Pirate Bay search, instant availability, magnet submission, download polling, link unrestriction, file download, archive extraction, audio file selection |
