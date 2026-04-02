# Audit: SABnzbd

## Workspace Signal

- Active execution side of Cassette's Usenet lane in `director/providers/usenet.rs`.
- URL and API key persisted in settings.

## Complete Technical Blueprint

- Core utility: queue and download execution for NZBs.
- Auth flow: API key.
- Webhooks/events: API plus script hooks; Cassette currently leans on filesystem completion polling.
- Important split: SABnzbd executes NZBs; it does not solve discovery or canonical metadata.

## Autonomous Suggestions

- Use queue/history/status endpoints instead of treating the filesystem as the only completion signal.
- Persist SAB job IDs and map them back to request signatures.
- Archive the NZB plus final importer audit record to keep the lane reversible.

## Critical Failings

- If Cassette only polls folders, it loses queue-level causality.
- SAB does not remove the need for indexer provenance.
- It is an executor, not a truth source.

## Sources

- https://sabnzbd.org/wiki/configuration/4.5/api
- https://sabnzbd.org/wiki/faq
