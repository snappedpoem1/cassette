# Deezer
> Encrypted stream acquisition from Deezer's CDN with Blowfish decryption and ARL-based authentication

**Status:** Active
**Code:** `crates/cassette-core/src/director/providers/deezer.rs`, `crates/cassette-core/src/director/providers/crypto.rs`
**Provider ID:** `"deezer"`
**Trust Rank:** 5

## What It Does

Deezer acquires audio by authenticating with an ARL session cookie, searching the public Deezer API for tracks, then downloading encrypted streams from Deezer's CDN and decrypting them in-place using Blowfish CBC. It supports FLAC and MP3 formats with automatic quality fallback.

Session bootstrap happens once via `OnceCell`: a client is created with the ARL cookie attached, then `deezer_get_user_data()` retrieves the `api_token` and `license_token` needed for subsequent private API calls. Because the session is in a `OnceCell<Result<...>>`, initialization is attempted exactly once per application lifetime.

The acquisition pipeline is a 5-step process: fetch track metadata, resolve media URL (trying FLAC first, then 320kbps MP3, then 128kbps MP3), download the encrypted stream, decrypt it in-place with a track-specific Blowfish key, and write the result to the temp directory. Cover art URLs are extracted during search for downstream metadata use.

## External Dependencies

| Dependency | Type | Endpoint |
|---|---|---|
| Deezer Public Search API | REST | `GET https://api.deezer.com/search/track` |
| Deezer Private User Data API | REST | Internal (requires ARL cookie) |
| Deezer Private Track Data API | REST | Internal (requires api_token) |
| Deezer Private Media URL API | REST | Internal (requires license_token) |
| Deezer CDN | HTTPS | Encrypted audio stream download |

## Authentication & Credentials

Authentication is ARL-based. The ARL (Authentication Request Lifecycle) token is a long-lived session cookie obtained from a logged-in Deezer browser session. It is set as a cookie on the HTTP client. From that, two derived tokens are obtained:

- **api_token** - used for private API calls like track data retrieval
- **license_token** - used for media URL resolution (quality negotiation)

These are cached in `DeezerSessionCache` inside a `OnceCell`, meaning the bootstrap runs exactly once. There is no automatic token refresh or re-authentication; if the ARL expires, the provider fails.

## Data Flow

### Search
1. `GET https://api.deezer.com/search/track?q={query}&limit=10`
2. Score each result: base 0.35 + artist match 0.35 + title match 0.25 + album match 0.05, clamped to 0.95
3. Extract cover art URL (prefers cover_xl > cover_big > cover_medium)
4. Return scored candidates

### Acquire
1. `deezer_get_track_data(client, api_token, track_id)` - fetch track_token and confirmed track_id
2. `deezer_get_media_url(client, license_token, track_token)` - negotiate quality: tries FLAC, then 320kbps MP3, then 128kbps MP3; returns (media_url, extension)
3. Download encrypted stream from CDN
4. `decrypt_deezer_stream(&mut data, &track_data.track_id)` - in-place Blowfish CBC decryption
5. Write decrypted file as `"deezer-{id}.{ext}"` to temp directory

## Capabilities

- Lossless audio acquisition (FLAC) with automatic quality fallback (320 > 128)
- Cover art URL extraction during search
- In-place stream decryption (no intermediate encrypted file on disk)
- Public API search (no authentication needed for search itself)

## Configuration

| Setting | Source | Default | Description |
|---|---|---|---|
| `deezer_arl` | RemoteProviderConfig | None | ARL session token from a logged-in Deezer browser session |

## Cryptography Details

All crypto lives in `crypto.rs`:

- **Algorithm:** Blowfish CBC
- **Master key:** `"g4el58wc0zvf9na1"`
- **IV:** `[0, 1, 2, 3, 4, 5, 6, 7]`
- **Chunk size:** 2048 bytes
- **Key derivation:** MD5(track_id) as hex string, then XOR first 16 chars with last 16 chars, then XOR with master key
- **Stripe pattern:** Every 3rd chunk (indices 0, 3, 6, 9, ...) is Blowfish-encrypted; all other chunks are plaintext
- **Additional:** An AES-256-CBC PKCS7 helper function exists in crypto.rs but is currently unused

## Limitations & Known Issues

- No automatic ARL refresh; provider fails silently when the ARL expires
- `OnceCell` session means a failed bootstrap is permanent until restart
- No batch download support (`supports_batch: false`)
- Search confidence weights title (0.25) and album (0.05) less than artist (0.35), which may underweight exact title matches
- Quality fallback is silent; no way to know if you got FLAC or 128kbps MP3 without inspecting the file

## Untapped Potential

The Deezer public API supports extensive metadata that is not currently used: artist pages, full discographies, related artists, editorial playlists, radio stations, genre browsing, and chart data. The private API (accessible with the ARL) additionally supports lyrics, user favorites, and recommendations. These could enable discovery features, lyrics display, and smarter duplicate detection. Album-level acquisition is also possible but not implemented.

## Code Map

| File | Role |
|---|---|
| `crates/cassette-core/src/director/providers/deezer.rs` | Provider implementation: session bootstrap, search, acquire, quality negotiation |
| `crates/cassette-core/src/director/providers/crypto.rs` | Blowfish CBC decryption, key derivation, stripe-pattern decryption logic, AES helper |
| `crates/cassette-core/src/sources.rs` | Shared source/credential resolution |
