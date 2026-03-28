# Architectural Recommendations

Last audited: 2026-03-27

## Target Shape

Keep the current stack. Change the ownership boundaries.

### 1. Define a real acquisition request contract

Replace the effective `artist + title + optional album` contract with a typed request that supports:

- scope: track / album / artist / discography / selected-albums
- exact identity if known: MBID / source ID
- include filters
- exclude filters
- edition policy
- quality policy
- provider policy
- confirmation policy

Without this, the system cannot honestly support granular requests.

### 2. Make MusicBrainz the canonical identity spine

Store and reuse:

- artist MBID
- release-group MBID
- release MBID
- recording MBID

Then map every provider candidate back onto that spine where possible.

### 3. Converge on one persistence model

Best path:

- migrate the active app to use the richer `librarian`/`library` schema surfaces
- keep the small UI-oriented tables only as read models if needed

Do not continue growing two parallel truths.

### 4. Split planning from execution

Recommended stages:

1. normalize request
2. resolve canonical metadata
3. reuse cache/provenance memory
4. search providers
5. score candidates
6. optional manual review
7. acquire chosen candidate
8. validate
9. normalize/tag/enrich
10. import and persist provenance

### 5. Canonicalize provider responsibilities

| Concern | Owner |
| --- | --- |
| Canonical metadata identity | MusicBrainz |
| Secondary enrich | Discogs, Last.fm, Cover Art Archive |
| Premium direct acquisition | Qobuz first, Deezer second |
| P2P acquisition | slskd |
| NZB search | NZBGeek |
| NZB execution | SABnzbd |
| Torrent search | Jackett |
| Torrent resolve/hoster | Real-Debrid |
| Low-trust fallback | yt-dlp |
| Tag write/read | Lofty |
| Decode/validation | Symphonia + validation stack |

### 6. Surface explainability as a product feature

Every finalized acquisition should be able to answer:

- which canonical release was targeted
- which providers were searched
- which candidates were rejected and why
- why the winner scored highest
- whether the user approved or auto-selected it

That means candidate persistence is not optional.

### 7. Remove or retire conflicting abstractions

Prioritize:

- `director` as canonical orchestration layer
- retire or absorb `downloader/`
- remove `ProviderBridge` after migration
- keep stubs clearly labeled until real integrations exist
