# Usenet
> NZB-based music acquisition via NZBgeek indexer and SABnzbd download client with filesystem polling for completion

**Status:** Active
**Code:** `crates/cassette-core/src/director/providers/usenet.rs`
**Provider ID:** `"usenet"`
**Trust Rank:** 30

## What It Does

The Usenet provider searches for music on the NZBgeek indexer, downloads NZB files, submits them to SABnzbd for processing, and polls the filesystem for completed audio files. It operates in a two-phase model: the provider handles search and NZB submission, while SABnzbd handles the actual Usenet download, assembly, and extraction.

Search queries the NZBgeek API in the Music category (cat=3000), scoring results by keyword match with strong bonuses for lossless indicators. The candidate ID is the NZB download link itself. During acquisition, the NZB file is downloaded, written to temp, and optionally submitted to SABnzbd via its multipart API. Completion is detected by polling configured scan roots for matching audio files.

## External Dependencies

| Dependency | Type | Endpoint |
|---|---|---|
| NZBgeek Search API | REST | `GET https://api.nzbgeek.info/api` |
| NZBgeek NZB Download | HTTPS | NZB download URL (from search results) |
| SABnzbd API | REST | `POST {sabnzbd_url}/api` (multipart, mode=addfile) |
| Local filesystem | Disk | Scan roots for completed downloads |

## Authentication & Credentials

NZBgeek authentication uses an API key passed as a query parameter (`apikey`) on all requests, both for search and NZB download.

SABnzbd authentication uses a separate API key passed as a form field in the multipart upload request. Both keys are stored in the provider configuration.

## Data Flow

### Search
1. `GET https://api.nzbgeek.info/api` with `t=search`, `cat=3000` (Music), `q={query}`, `apikey={key}`, `o=json`, `limit=5`
2. Parse results from `/channel/item` or `/item` array in response
3. Score each result: artist term +20pts, title term +30pts, "flac" +100pts, "24bit"/"24 bit" +50pts
4. Return top 5 results sorted by score
5. Candidate ID = NZB download link (from `link`, `enclosure/url`, or `guid` fields)

### Acquire
1. Download NZB file from candidate URL (with `apikey` query parameter)
2. Write NZB to temp directory as `"{artist}-{title}.nzb"`
3. If SABnzbd is configured: multipart POST to `{sabnzbd_url}/api` with:
   - `mode=addfile`
   - `apikey={sabnzbd_api_key}`
   - `cat=audio`
   - NZB file uploaded as `"cassette.nzb"` with MIME type `application/x-nzb`
4. Poll scan roots 24 times at 5-second intervals (2 minutes total) using walkdir for matching audio files
5. Copy found file to temp directory

## Capabilities

- Lossless audio discovery with strong FLAC/24-bit scoring bonuses
- NZB download and local storage (usable even without SABnzbd for manual processing)
- SABnzbd integration for automated download pipeline
- Filesystem polling for completion detection across multiple scan roots

## Configuration

| Setting | Source | Default | Description |
|---|---|---|---|
| `api_key` | Provider config | None | NZBgeek API key for search and NZB download |
| `sabnzbd_url` | Provider config | None | SABnzbd base URL (optional; if absent, NZB is saved but not submitted) |
| `sabnzbd_api_key` | Provider config | None | SABnzbd API key for file submission |
| `scan_roots` | Provider config | None | Filesystem paths where SABnzbd writes completed downloads |

## Limitations & Known Issues

- 2-minute polling timeout may be insufficient for large Usenet downloads that require assembly and extraction
- No SABnzbd queue or status monitoring; relies entirely on filesystem polling
- Search is limited to 5 results per query
- Candidate ID is a URL, making it opaque and not parseable for metadata
- No verification that SABnzbd accepted the NZB (fire-and-forget submission)
- No batch download support (`supports_batch: false`)
- Category is hardcoded to 3000 (Music) for search and "audio" for SABnzbd

## Untapped Potential

NZBgeek supports RSS feeds for automated monitoring, additional category filtering, and tvdbid/imdbid-style lookups. SABnzbd has a comprehensive API that is almost entirely unused: queue management (`/api?mode=queue`), download history (`/api?mode=history`), category rules, post-processing scripts, completion notifications via callbacks, priority management, pause/resume, and speed limiting. Using SABnzbd's history API for completion detection would be far more reliable than filesystem polling. Queue status could provide progress feedback and ETA information.

## Code Map

| File | Role |
|---|---|
| `crates/cassette-core/src/director/providers/usenet.rs` | Full provider implementation: NZBgeek search, NZB download, SABnzbd submission, filesystem polling |
