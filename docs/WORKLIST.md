# Worklist

Last updated: March 21, 2026

## Execution Rule

Prioritize backend truth and download reliability over cosmetic expansion.
Keep cassette baseline primary flow tight while preserving advanced routes behind direct navigation.

## Current Status

- [x] Primary shell reduced to cassette-baseline navigation in sidebar (`Library`, `Downloads`, `Settings`)
- [x] Repo docs aligned to actual runtime paths (`cassette-core`, `ui`, `src-tauri`)
- [x] Build sanity revalidated (`cargo check`, `ui npm run build`)
- [x] Frontend a11y build warnings cleared for primary clickable row/card interactions
- [x] Desktop smoke checks pass (`scripts/smoke_desktop.ps1`)

## Next 5 Execution Tasks

1. [ ] Replace Deezer preview fallback with full native ARL-backed media acquisition in `download_runtime.rs`
2. [ ] Run and document live provider proofs on this machine:
       Deezer ARL acquisition, Tidal device OAuth acquisition, slskd transfer path
3. [ ] Convert acquisition worker flow to end-to-end async orchestration (remove remaining bridge-style stages)
4. [ ] Surface import and provider proof telemetry in the UI for quick runtime verification
5. [ ] Produce packaged-confidence checklist and execute clean-machine + long-session validation

## Deferred But Preserved

- `/playlists`, `/import`, and `/tools` routes remain implemented and callable.
- Reintroduction sequencing for broader Lyra-style intelligence surfaces is deferred until acquisition trust hardening completes.
