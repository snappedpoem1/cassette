# Audit: Qobuz

## Workspace Signal

- Active provider in `director/providers/qobuz.rs`.
- Search and discography endpoints used in `sources.rs`.
- Machine config persists app ID, secret, hashed password, token, and secrets list.

## Complete Technical Blueprint

- Core utility: premium catalog search, artist albums, and high-quality acquisition.
- Auth flow: app credentials plus user-auth token or login flow.
- Webhooks/events: none.
- Public-doc reality: the clearest public sources are terms and app-guideline materials rather than a rich modern developer portal.

## Autonomous Suggestions

- Snapshot release metadata locally on successful search/acquire because upstream access is more brittle than MusicBrainz.
- Normalize Qobuz track/release IDs into a source-mapping table under a canonical MBID spine.
- Persist quality-specific availability so retries stop rediscovering the same lossless miss.

## Critical Failings

- Terms reserve strong control for Qobuz over API availability and restrictions.
- Geoblocking and account coupling cut against sovereignty.
- The app-secret-heavy surface makes long-term unattended durability weaker than open metadata systems.

## Sources

- https://static.qobuz.com/apps/api/QobuzAPI-TermsofUse.pdf
- https://static.qobuz.com/apps/api/Qobuz-AppsGuidelines-V1.0.pdf
