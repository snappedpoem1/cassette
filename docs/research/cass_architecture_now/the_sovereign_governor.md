# The Sovereign Governor

Generated: 2026-04-02

Cassette becomes sovereign the moment it stops asking any external service to define what your library *is*. External services should describe, suggest, or supply. Cassette should decide.

## Governing Architecture

This would lead me to believe this... the real governing center is not the downloader stack. It is a local identity graph where every artist, release, recording, file, alias, fingerprint, and provider attempt can be reduced to one queryable chain.

This would lead me to believe this... MusicBrainz should be the canonical identity spine, AcoustID/Chromaprint should be the acoustic recovery spine, and ListenBrainz should become the behavior spine.

This would lead me to believe this... Spotify belongs on the intent edge, not the truth edge. It tells Cassette what you wanted, played, or queued, but never what the definitive release is.

This would lead me to believe this... Qobuz, Deezer, slskd, NZBGeek, SABnzbd, Jackett, Real-Debrid, and yt-dlp should all collapse into one normalized acquisition plane with identical evidence capture, regardless of how messy their upstream behavior is.

This would lead me to believe this... Symphonia and Lofty are not just support libraries. They are the local proof engines that let Cassette say, with a straight face, "this file is real, this metadata was changed, and here is why."

## The Required Local Graph

Core entities:

- `canonical_artists`
- `canonical_releases`
- `canonical_recordings`
- `source_aliases`
- `file_identities`
- `file_artifacts`
- `request_signatures`
- `candidate_sets`
- `provider_attempts`
- `operator_overrides`

This would lead me to believe this... once those entities are durable, every service can be demoted from "authority" to "input."

## How The Services Interlock

### Identity

- MusicBrainz gives MBIDs, release groups, release sequencing, label, country, and tracklist structure.
- AcoustID/Chromaprint gives Cassette a way to recover identity when files are ugly.
- Cover Art Archive gives artwork after identity is stable.
- Discogs can refine edition and pressing nuance.

This would lead me to believe this... release identity should be chosen before artwork, lyrics, or mass tag mutation, every single time.

### Intent

- Spotify import/history tells Cassette what you care about.
- ListenBrainz can later add cross-service listen behavior and recommendations.
- Local playback history tells Cassette what actually stuck.

This would lead me to believe this... intent should be modeled as pressure on the acquisition queue, not as metadata truth.

### Acquisition

- Qobuz and Deezer cover high-confidence premium catalog lanes.
- slskd covers opportunistic P2P retrieval.
- NZBGeek plus SABnzbd cover Usenet discovery and execution.
- Jackett should abstract torrent indexers.
- Real-Debrid should resolve magnets and hoster links, not define search truth.
- yt-dlp should remain a red-tag fallback lane for difficult or niche content.

This would lead me to believe this... Cassette's acquisition system should be able to explain not only why a file was chosen, but why every rejected file was rejected.

### Validation And Admission

- Symphonia proves the media is readable and technically plausible.
- Lofty applies structured metadata once identity confidence is high enough.
- The filesystem and SQLite audit trail provide reversibility.

This would lead me to believe this... admission should be treated like customs, not like a copy step.

## What Makes This A Governor Instead Of A Client

This would lead me to believe this... a client asks Spotify, Qobuz, or Deezer what exists and trusts the answer. A governor stores the answer, compares it against every other answer, remembers past contradictions, and decides locally.

This would lead me to believe this... sovereignty is not just offline access. It is durable memory, deterministic reconciliation, and provider replaceability.

This would lead me to believe this... the local database should eventually know:

- what you wanted
- what upstreams claimed
- what you actually acquired
- what the file acoustically is
- what tags were changed
- why a placement decision happened
- which candidate lost and why

## AI-Ready Intersections

The future AI layer will be strongest where these datasets intersect:

- MBID identity + fingerprint certainty
- provider candidate history + operator overrides
- Spotify intent + local playback completion
- ListenBrainz behavior + local skips/play counts
- Discogs edition nuance + Qobuz/Deezer availability
- LRCLIB lyric timing + validated local recordings
- Symphonia technical truth + acquisition quality tiers

This would lead me to believe this... the best AI Cassette is not a chatbot bolted onto a player. It is a planner sitting on a structured local graph with evidence, memory, and reversible actions.

This would lead me to believe this... an AI governor could eventually answer:

- "Which version of this album do I actually own?"
- "Why did Cassette choose this Deezer file over that Qobuz result?"
- "Which missing albums are high-confidence wins from Real-Debrid cache right now?"
- "Which files should be re-acquired because they are unnumbered, fingerprint-unknown, and quality-poor?"
- "Which candidate should be blocked forever because I rejected it three times?"

## Non-Negotiable Rules For That Future

This would lead me to believe this... the AI layer must never mutate files or queue work without writing an inspectable request, evidence bundle, and outcome trail.

This would lead me to believe this... every provider decision should remain reversible, and every AI suggestion should be able to cite the exact facts that produced it.

This would lead me to believe this... the current split-brain between runtime `tracks` and sidecar `local_files` has to be resolved before an AI layer can be trusted.

## Final Position

This would lead me to believe this... Cassette is already close to being the sovereign governing body of your listening, but only if it finishes the last hard move: promoting identity, evidence, and convergence to first-class runtime truth instead of leaving them stranded in side tables, stubs, and implied future work.
