# Synthesis Matrix

Generated: 2026-04-02

## Governing Split

Cassette should stop treating every upstream as "just another provider." The better split is:

| Layer | Best Owners | Why |
|---|---|---|
| Canonical identity | MusicBrainz, AcoustID/Chromaprint, Cover Art Archive | Open IDs, release graph, artwork keyed to release |
| Intent and demand | Spotify, ListenBrainz, local history/imports | What you want or play, not what is canonically true |
| Secondary enrichment | Discogs, Last.fm, Genius, LRCLIB | Good context, weak canonical authority |
| Acquisition | Qobuz, Deezer, slskd, NZBGeek, SABnzbd, Jackett, Real-Debrid, yt-dlp | Get the file, then prove it |
| Local truth | Symphonia, Lofty, filesystem, SQLite | What you actually have and can audit |

## The Glue

### What Existing Tying Agents Already Prove

- Beets proves that metadata bridges belong in plugins/modules, not in one giant importer.
- Picard proves that fingerprint -> MBID -> tag write is the cleanest repair ladder for messy files.
- Lidarr proves that wanted-state, search, downloader handoff, and import are separate phases.
- Jackett proves tracker scraping should be abstracted.
- SABnzbd proves execution should be distinct from search.
- Real-Debrid proves resolve/unrestrict should be distinct from search.

### What They Still Get Wrong

- They often let external tools own too much truth.
- They do not prioritize Cassette-level audit completeness the way your architecture demands.
- They usually stop at import correctness, not lifelong provenance reuse.

## If A Can Do X, Why Not Y Here?

- If MusicBrainz can give Cassette a `release_group_mbid`, why are Spotify, Qobuz, Deezer, Discogs, and local files not all mapped to that one durable release family key?
- If AcoustID can bridge a bad local file to a recording, why is Cassette still letting empty tags and filename guesses do so much identity work?
- If Cover Art Archive is keyed off release MBIDs, why is artwork not downstream of canonical release selection every time?
- If ListenBrainz can give MBID-linked behavior, why is future recommendation context still imagined as raw text and not as structured graph evidence?
- If Jackett can normalize torrent search, why is Cassette still carrying a direct `apibay` dependency in the critical path?
- If SABnzbd exposes queue and history state, why is the Usenet lane still satisfied with directory polling as the primary completion truth?
- If Real-Debrid can tell you cached availability by hash, why is that not part of candidate scoring memory before Cassette commits to a slower lane?
- If Spotify is only good for intent, why is the desired-state schema not explicitly modeled as `intent_aliases` instead of leaking toward canonical truth?
- If Deezer and Qobuz can both return useful catalog metadata, why not snapshot that data locally and compare it against MusicBrainz rather than trusting either service in isolation?
- If Symphonia can prove codec/container/duration locally, why isn't that proof normalized into the same identity tables the rest of the system uses?

## Recommended Interlock

### 1. Identity First

- Create a canonical local graph:
  - `canonical_artists`
  - `canonical_releases`
  - `canonical_recordings`
  - `source_aliases`
  - `file_identities`
- Put MBIDs and fingerprints at the center.

### 2. Source Alias Layer

- Every provider-facing ID belongs in `source_aliases`.
- Alias examples:
  - Spotify track ID
  - Deezer track ID
  - Qobuz album/track ID
  - Discogs release/master ID
  - YouTube video ID
  - SoundCloud track URL/ID

### 3. Request Contract Upgrade

- Current Cassette still thinks too much in `artist + title + optional album`.
- The request contract should evolve toward:
  - desired recording or release MBID when known
  - source aliases when imported from upstream
  - quality floor
  - edition policy
  - exclusion memory
  - provenance trail

### 4. Candidate Review Memory

- Provider search results, candidate scoring, and negative outcomes already persist.
- The missing move is reuse:
  - skip exhausted provider/query combinations
  - surface prior candidate sets before re-search
  - let users bless or ban recurring candidates

## Best-Fit Roles By Service

| Service | Best Role | Wrong Role |
|---|---|---|
| MusicBrainz | canonical identity | acquisition |
| AcoustID/Chromaprint | identity recovery | rich metadata |
| Cover Art Archive | artwork after release selection | free-text art search |
| Spotify | intent/history | canonical truth |
| ListenBrainz | behavior graph | release authority |
| Discogs | variant disambiguation | primary identity |
| Last.fm | context/tags | release authority |
| LRCLIB | synced lyrics | identity |
| Qobuz | premium acquisition | long-term metadata authority |
| Deezer | secondary acquisition | sovereign truth |
| slskd | opportunistic acquisition | trusted metadata |
| NZBGeek | index search | truth |
| SABnzbd | execution | search |
| Jackett | search abstraction | resolver/importer |
| Real-Debrid | resolve/unrestrict | search authority |
| yt-dlp | emergency fallback | canonical anything |
| YouTube/SoundCloud | discovery/fallback | release truth |

## Architectural End State

- Cassette owns the graph.
- Providers become swappable acquisition or context adapters.
- The AI layer sits on top of:
  - MBID identity
  - fingerprint certainty
  - local validation truth
  - behavior history
  - candidate memory
  - operator override history

That is the difference between a downloader pile and a sovereign governor.
