# Provider Evidence Ledger

Last updated: 2026-04-07
Covers: `GAP-C01`
Status vocabulary: `local-proven`, `bounded-probe`, `unverified`

Latest snapshot artifacts:

- `docs/probes/provider_probe_2026-04-07_174445.txt`
- `docs/probes/lane_c_probe_2026-04-07_174445.json`

---

## Purpose

Provide repeatable provider status evidence entries with:

- verification scope
- last evidence date
- artifact/source path
- current outcome

This ledger is the canonical evidence sheet for provider reliability posture.

---

## Evidence Table

| Provider | Outcome | Verification scope | Last evidence date | Evidence source |
|---|---|---|---|---|
| local_archive | local-proven | local machine runtime behavior | 2026-04-07 | `docs/TOOL_AND_SERVICE_REGISTRY.md` |
| deezer | local-proven | live full-track proof on this machine | 2026-04-07 | `docs/PROJECT_STATE.md`, `docs/HIT_LIST.md` |
| qobuz | unverified | implementation present, current credential/session dependent | 2026-04-07 | `docs/TELEMETRY.md`, `docs/TOOL_AND_SERVICE_REGISTRY.md`, `docs/probes/provider_probe_2026-04-07_174445.txt` |
| slskd | unverified | managed runtime probe exists, broader proof pending | 2026-04-07 | `docs/TELEMETRY.md`, `docs/TOOL_AND_SERVICE_REGISTRY.md`, `docs/probes/provider_probe_2026-04-07_174445.txt` |
| usenet | unverified | handoff wired, end-to-end proof pending | 2026-04-07 | `docs/TELEMETRY.md`, `docs/TOOL_AND_SERVICE_REGISTRY.md`, `docs/probes/provider_probe_2026-04-07_174445.txt`, `docs/probes/lane_c_probe_2026-04-07_174445.json` |
| jackett | unverified | active provider, broader live proof pending | 2026-04-07 | `docs/PROJECT_STATE.md`, `docs/TOOL_AND_SERVICE_REGISTRY.md` |
| real_debrid | local-proven | live-proven on this machine and runtime owner clarified | 2026-04-07 | `docs/PROJECT_STATE.md`, `docs/TOOL_AND_SERVICE_REGISTRY.md` |
| yt_dlp | local-proven | fallback path proven in local workflow | 2026-04-07 | `docs/TOOL_AND_SERVICE_REGISTRY.md`, `docs/HIT_LIST.md` |
| bandcamp_source | unverified | resolver-only scope, no first-class search lane | 2026-04-07 | `docs/DECISIONS.md`, `docs/TOOL_AND_SERVICE_REGISTRY.md` |

---

## Stale Evidence Policy

- If evidence date is older than 30 days, mark row as `stale` in notes and downgrade status to `unverified` until refreshed.
- Every status change must cite at least one canonical docs source path.

---

## Refresh Checklist

1. Run provider probe(s) and record artifact path(s).
2. Update row outcome and date.
3. Update linked canonical docs status text.
4. Re-run docs consistency check.
