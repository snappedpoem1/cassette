# Audit: Lidarr

## Workspace Signal

- Not present in runtime code.
- Explicitly relevant as a downloader-orchestration reference.

## Complete Technical Blueprint

- Core utility: wanted-list management, indexer search, download-client handoff, import flow.
- Auth flow: service-local auth; integrations vary by indexer and downloader.
- Webhooks/events: it has a richer automation ecosystem than Cassette today, but the principle matters more than the tool.
- Strategic role: proof that "intent queue" and "download execution" should be separate layers.

## Autonomous Suggestions

- Steal Lidarr's separation of wanted-state, search, client handoff, and import completion.
- Do not inherit Lidarr's tendency to let external clients own too much truth; Cassette should persist everything locally.
- Use its model to sharpen `delta_queue` into a real sovereign work bus.

## Critical Failings

- Lidarr still inherits upstream indexer and downloader volatility.
- It is built for automation breadth, not necessarily the forensic audit depth Cassette wants.
- If Cassette copies Lidarr whole, it will import its assumptions along with its strengths.

## Sources

- https://lidarr.audio/docs/
- https://lidarr.audio/docs/api/
