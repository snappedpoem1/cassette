# Audit: MusicBrainz Picard

## Workspace Signal

- Not used directly.
- Strong architectural analogue to the file-recovery and MBID-tagging work Cassette wants.

## Complete Technical Blueprint

- Core utility: clustering, AcoustID fingerprinting, MBID resolution, tag writing.
- Auth flow: none required for local tag work; upstream MB/AcoustID behavior drives network access.
- Webhooks/events: plugin hooks rather than webhooks.
- Strategic role: best-in-class reference for file-to-MBID recovery.

## Autonomous Suggestions

- Copy the order of operations: scan -> cluster -> fingerprint -> MBID resolve -> write tags -> preserve evidence.
- Store confidence and evidence per fix, not just final tag values.
- Add Picard-style "candidate review" lanes in Cassette before destructive mass normalization.

## Critical Failings

- Picard is a specialist, not a sovereign control plane.
- It solves identity tagging far better than acquisition orchestration.
- If you imitate only the tag write and not the evidence chain, you miss the real lesson.

## Sources

- https://picard.musicbrainz.org/
- https://picard-docs.musicbrainz.org/en/appendices/plugins_api.html
- https://picard-docs.musicbrainz.org/v2.4/en/config/options_fingerprinting.html
