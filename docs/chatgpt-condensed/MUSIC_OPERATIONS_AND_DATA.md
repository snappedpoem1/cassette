# Music Operations And Data

Last condensed: March 20, 2026

## Practical Workflow Preserved In The Export

The raw export was not only planning prose.
It also captured an operational workflow for building a music library from Spotify history and downloader tooling.

Practical chain:

1. Spotify listening history feeds a local SQLite database.
2. The database produces album and artist acquisition queues.
3. Batch Python wrappers launch acquisition attempts in parallel.
4. The wrappers call `proof_download.exe` in `C:\Cassette Music`.
5. If the Rust waterfall exhausts providers, a Python fallback script can try Qobuz then Deezer through streamrip.
6. Progress and failures are written back to SQLite and logs.

## Important Operational Files From The Export

### Database

`cassette_spotify.db` was the most important non-doc artifact in `C:\chatgpt`.

Captured schema highlights:

- `plays`: 127,572 rows
- `tracks`: 17,845 rows
- `download_queue`: 4,318 rows
- `album_queue`: 2,580 rows
- `artist_queue`: 1,283 rows
- `checkpoint_log`: 379 rows

Captured album queue state at condensation time:

- `done`: 2,575
- `failed`: 5

Remaining failed albums captured from the DB:

- Dead Poet Society - `-!-`
- Denzel Curry - `Nostalgic 64`
- Fenech-Soler - `Rituals`
- Godspeed You! Black Emperor - `G_d's Pee AT STATE'S END!`
- Right Away, Great Captain! - `The Church of the Good Thief`

### Acquisition Runner Scripts

The export had active scripts named:

- `cassette_downloader.py`
- `cassette_run_loop.py`
- `qobuz_dl.py`
- `adjacent_jams_downloader.py`
- `remix_downloader.py`

These scripts should be treated as historical operational evidence, not future runtime architecture.

### Spotify Extended Streaming History

The export retained Spotify's extended streaming history as raw source data spanning 2021 through early 2026.

### Library Folder Snapshot

`library_folders.txt` served as a materialized snapshot of the on-disk music library folder names.

## What The Export Proved

At minimum, the export proved:

- Spotify history had been ingested into a local database
- batch album acquisition was actively run on March 20, 2026
- the workflow completed 2,575 albums
- only 5 album targets remained unresolved at the time of review
- the operational bridge between `C:\chatgpt` and `C:\Cassette Music` was real
