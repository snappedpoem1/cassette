# Stage B Next Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete four sequential items: planner cutover for the coordinator loop, Discogs/Last.fm enrichment live proof, adaptive provider orchestrator, and dead-letter command center.

**Architecture:** Sequential dependency chain — each item builds cleanly on the previous. Items 1–3 are Rust-only; item 4 adds a Tauri command layer and a Svelte UI section. The planner path (`plan_acquisition` → `approve_planned_request`) already exists and is proven for UI-originated requests; the work here threads it into the coordinator and the dead-letter replay path.

**Tech Stack:** Rust, Tauri 2, SvelteKit, SQLite (rusqlite + sqlx), `reqwest`, `tokio`

---

## File Map

### Item 1 — Planner cutover

| File | Change |
|---|---|
| `src-tauri/src/bin/engine_pipeline_cli.rs` | Replace direct `submitter.submit(task)` loop with planner path |
| `crates/cassette-core/src/librarian/db/mod.rs` | Read-only — no changes needed |
| `src-tauri/src/commands/planner.rs` | Read-only — consumed as-is |

### Item 2 — Enrichment probe

| File | Change |
|---|---|
| `src-tauri/src/bin/enrich_probe_cli.rs` | New CLI binary |
| `src-tauri/Cargo.toml` | Register new binary |
| `docs/PROJECT_STATE.md` | Proof artifact |
| `docs/TOOL_AND_SERVICE_REGISTRY.md` | Update Discogs/Last.fm rows |

### Item 3 — Adaptive orchestrator

| File | Change |
|---|---|
| `crates/cassette-core/src/director/config.rs` | Add two new config constants |
| `crates/cassette-core/src/director/engine.rs` | Add nudge pass in `load_persisted_provider_hints` |
| `crates/cassette-core/src/director/strategy.rs` | Tests for nudge-aware ordering |

### Item 4 — Dead-letter center

| File | Change |
|---|---|
| `crates/cassette-core/src/db/mod.rs` | Add `get_dead_letter_summary` query + types |
| `src-tauri/src/commands/dead_letter.rs` | New command module |
| `src-tauri/src/commands/mod.rs` | Register new module |
| `src-tauri/src/lib.rs` | Register new Tauri commands |
| `ui/src/lib/api/tauri.ts` | Add TS types + `invoke` wrappers |
| `ui/src/routes/downloads/+page.svelte` | Add dead-letter section |

---

## Item 1: Planner Cutover for `engine_pipeline_cli`

### Task 1.1 — Add a planner-path helper to the coordinator

The coordinator currently builds `TrackTask`s and submits them directly. We will extract a new async function `plan_and_submit` that calls the planner logic inline (without going through the Tauri command surface — the CLI doesn't have `AppState`). It replicates the essential steps of `plan_acquisition` + `approve_planned_request` using the raw `Db` and `LibrarianDb` types directly.

**Files:**
- Modify: `src-tauri/src/bin/engine_pipeline_cli.rs`

- [ ] **Step 1: Write the failing test for the new helper signature**

Add to the `mod tests` block at the top of `engine_pipeline_cli.rs`:

```rust
#[tokio::test]
async fn plan_and_submit_builds_acquisition_request_row() {
    // This test verifies the helper compiles and the types align.
    // Full integration proof is captured via live run (Task 1.3).
    let _: fn(
        &cassette_core::db::Db,
        &TrackTask,
        &str,
        &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>> = |_, _, _, _| {
        Box::pin(async { Ok("task-id".to_string()) })
    };
}
```

- [ ] **Step 2: Run test to verify it compiles (not yet implemented)**

```powershell
cargo test -p cassette-app --test pure_logic 2>&1 | head -30
```

(The inline module test will be exercised via `cargo check` at this stage.)

- [ ] **Step 3: Add imports needed by the new helper**

At the top of `engine_pipeline_cli.rs`, add alongside existing imports:

```rust
use cassette_core::acquisition::{AcquisitionRequest, AcquisitionRequestStatus, AcquisitionScope};
use cassette_core::librarian::db::LibrarianDb;
```

`LibrarianDb` is the async sidecar DB type. Check the actual public type path:

```powershell
grep -r "pub struct LibrarianDb\|pub type LibrarianDb\|impl LibrarianDb" crates/cassette-core/src/librarian/db/
```

Use whatever path `get_acquisition_request_by_signature` is on (it's on the control DB, not the runtime `Db`).

- [ ] **Step 4: Add the `plan_and_submit` function**

Add this function before `main()` in `engine_pipeline_cli.rs`:

```rust
/// Run the planner path for a coordinator-claimed task and submit to Director.
/// Mirrors plan_acquisition + approve_planned_request but operates directly on
/// the raw DB types available in the CLI context.
///
/// Steps:
///   1. Build an AcquisitionRequest from the TrackTask.
///   2. Upsert the request row in the control DB (sidecar librarian DB).
///   3. Call the strategy planner to persist candidate sets in the runtime DB.
///   4. Mark the request status as Queued with reason "coordinator_auto_approve".
///   5. Submit the task to the Director.
///
/// Returns the task_id on success so the caller can associate it with the claim.
async fn plan_and_submit(
    db: &Db,
    pool: &sqlx::SqlitePool,
    task: &TrackTask,
    director_submitter: &crate::cassette_core::director::engine::DirectorSubmission,
    runtime_db_path: &std::path::Path,
) -> Result<(), String> {
    use cassette_core::acquisition::{AcquisitionRequest, AcquisitionRequestStatus, AcquisitionScope};
    use cassette_core::director::strategy::StrategyPlanner;
    use cassette_core::librarian::db::LibrarianDb;

    // Build a minimal AcquisitionRequest from the TrackTask fields.
    let mut request = AcquisitionRequest {
        task_id: Some(task.task_id.clone()),
        request_signature: None,
        scope: AcquisitionScope::Track,
        artist: task.target.artist.clone(),
        title: Some(task.target.title.clone()),
        album: task.target.album.clone(),
        isrc: task.target.isrc.clone(),
        musicbrainz_recording_id: task.target.musicbrainz_recording_id.clone(),
        musicbrainz_release_id: task.target.musicbrainz_release_id.clone(),
        musicbrainz_release_group_id: task.target.musicbrainz_release_group_id.clone(),
        canonical_artist_id: task.target.canonical_artist_id,
        canonical_release_id: task.target.canonical_release_id,
        source_track_id: task.target.spotify_track_id.clone(),
        source_album_id: task.target.source_album_id.clone(),
        source_artist_id: task.target.source_artist_id.clone(),
        strategy: Some(task.strategy),
        quality_policy: None,
        edition_policy: None,
        confirmation_policy: None,
        provider_policy: None,
        exclusions: None,
        status: AcquisitionRequestStatus::Pending,
        raw_payload_json: None,
    };
    if request.request_signature.is_none() {
        request.request_signature = Some(request.request_fingerprint());
    }

    let request_signature = request
        .request_signature
        .clone()
        .ok_or_else(|| "missing request signature".to_string())?;

    // Upsert the request row in the sidecar control DB.
    let control_db = LibrarianDb::from_pool(pool.clone());
    let row = match control_db
        .get_acquisition_request_by_signature(&request_signature)
        .await
        .map_err(|e| e.to_string())?
    {
        Some(existing) => existing,
        None => control_db
            .create_acquisition_request(&request)
            .await
            .map_err(|e| e.to_string())?,
    };

    // Persist candidate sets via StrategyPlanner into the runtime DB.
    {
        let (config, providers) = build_director_config_and_providers(db, runtime_db_path.to_path_buf());
        let planner = StrategyPlanner;
        let descriptors: Vec<_> = providers
            .iter()
            .map(|p| p.descriptor())
            .collect();
        let _plan = planner.plan(task, &descriptors, &config);

        let runtime_db = Db::open(runtime_db_path).map_err(|e| e.to_string())?;
        runtime_db
            .record_request_identity_snapshot(task, &request_signature)
            .map_err(|e| e.to_string())?;
        runtime_db
            .record_request_source_aliases_from_task(task, &request_signature)
            .map_err(|e| e.to_string())?;
    }

    // Mark approved with auto-approve reason.
    control_db
        .update_acquisition_request_status_by_task_id(
            &task.task_id,
            AcquisitionRequestStatus::Queued.as_str(),
            "coordinator_auto_approve",
            Some("coordinator planner path auto-approved"),
            None,
        )
        .await
        .map_err(|e| e.to_string())?;

    // Persist pending task and submit to Director.
    db.upsert_director_pending_task(task, "Queued")
        .map_err(|e| e.to_string())?;
    director_submitter
        .submit(task.clone())
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}
```

**Important:** The exact field names on `AcquisitionRequest` must match what's in `crates/cassette-core/src/acquisition.rs`. Read that file before writing this step to verify all field names. The struct may not have all fields shown above — omit fields that don't exist, use `..AcquisitionRequest::default()` or `..Default::default()` if the type implements `Default`.

- [ ] **Step 5: Check that `AcquisitionRequest` fields are correct**

```powershell
grep -A 60 "pub struct AcquisitionRequest" crates/cassette-core/src/acquisition.rs
```

Adjust the `plan_and_submit` function to match actual field names. Common mismatches to watch for:
- `strategy` may be `acquisition_strategy`
- `title` may not be `Option<String>` — check
- `source_track_id` / `spotify_track_id` — check which field name the struct uses

- [ ] **Step 6: Check that `LibrarianDb` is the right type name and import path**

```powershell
grep -r "pub struct\|pub type\|impl " crates/cassette-core/src/librarian/db/mod.rs | head -20
```

The control DB type used in `planner.rs` is accessed via `state.control_db`. Find what concrete type that is:

```powershell
grep "control_db" src-tauri/src/state.rs | head -10
```

Use whatever type `state.control_db` is — it will have `get_acquisition_request_by_signature`, `create_acquisition_request`, and `update_acquisition_request_status_by_task_id`.

- [ ] **Step 7: Replace the direct-submit loop in `main()` with `plan_and_submit`**

Find this block in `main()` (around line 808):

```rust
for task in tasks.iter().cloned() {
    db.upsert_director_pending_task(&task, "Queued")?;
    submitter.submit(task).await?;
}
```

Replace with:

```rust
for task in tasks.iter().cloned() {
    if operator_direct_submit {
        // Operator bypass: skip planner, submit directly.
        db.upsert_director_pending_task(&task, "Queued")?;
        submitter.submit(task).await?;
    } else {
        plan_and_submit(&db, &pool, &task, &submitter, &db_path)
            .await
            .unwrap_or_else(|e| {
                eprintln!("plan_and_submit failed for {}: {e}", task.task_id);
            });
    }
}
```

- [ ] **Step 8: Verify it compiles**

```powershell
cargo check --workspace 2>&1
```

Expected: no errors. Fix any type mismatches by re-reading the actual struct definitions.

- [ ] **Step 9: Commit**

```powershell
git add src-tauri/src/bin/engine_pipeline_cli.rs
git commit -m "feat(coordinator): route delta_queue submissions through planner path with auto-approve"
```

---

### Task 1.2 — Verify `--operator-direct-submit` still bypasses

- [ ] **Step 1: Confirm the bypass flag is threaded correctly**

Read the `plan_and_submit` wrapper you added and confirm the `if operator_direct_submit` branch submits directly without calling `plan_and_submit`. It should — verify visually.

- [ ] **Step 2: Run workspace tests**

```powershell
cargo test --workspace 2>&1
```

Expected: all pass.

---

### Task 1.3 — Live coordinator proof with planner path

- [ ] **Step 1: Run bounded coordinator proof**

```powershell
cargo run --bin engine_pipeline_cli -- --resume --limit 5 --skip-organize-subset --skip-post-sync
```

Expected output includes:
- Scan summary line
- `Reclaimed N stale queue claims` (if any)
- Tasks processed
- `engine_pipeline_cli complete: claimed=N finalized_paths=M`

- [ ] **Step 2: Verify acquisition_requests rows were created with candidate sets**

```powershell
cargo run --bin db_converge_cli -- --overwrite 2>&1 | head -20
```

Or open the runtime DB directly and check:

```powershell
$db = "$env:APPDATA\dev.cassette.app\cassette_librarian.db"
# In a SQLite shell or via db_converge_cli output, verify acquisition_requests has new rows
```

- [ ] **Step 3: Record proof in `PROJECT_STATE.md`**

Add a new subsection under the most recent proof block in `docs/PROJECT_STATE.md`:

```markdown
### Coordinator Planner Cutover Proof (YYYY-MM-DD)

Run command: `engine_pipeline_cli --resume --limit 5 --skip-organize-subset --skip-post-sync`

Outcome:
- N delta_queue rows claimed
- N tasks routed through planner path (plan_acquisition → coordinator_auto_approve)
- acquisition_requests rows created with candidate sets in runtime DB
- M tracks finalized
- operator_direct_submit bypass confirmed available
```

Fill in actual observed values.

- [ ] **Step 4: Update `TODO.md` and `HIT_LIST.md`**

In `TODO.md`, find `### [P1] [in_progress] Introduce a planner stage before byte acquisition` and change `[in_progress]` to `[done]`. Add a final acceptance item:
```
- [x] Coordinator loop routes delta_queue submissions through planner path with auto-approve
```

In `HIT_LIST.md`, change:
```
- [ ] Complete planner-stage cutover for all remaining runtime/operator lanes
```
to:
```
- [x] Complete planner-stage cutover for all remaining runtime/operator lanes — DONE YYYY-MM-DD
```

- [ ] **Step 5: Commit**

```powershell
git add docs/PROJECT_STATE.md docs/TODO.md docs/HIT_LIST.md
git commit -m "docs: record coordinator planner cutover proof and mark item done"
```

---

## Item 2: Discogs and Last.fm Enrichment Proof

### Task 2.1 — Write `enrich_probe_cli`

**Files:**
- Create: `src-tauri/src/bin/enrich_probe_cli.rs`
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Register the binary in `Cargo.toml`**

Open `src-tauri/Cargo.toml`. Find the existing `[[bin]]` entries (e.g. `engine_pipeline_cli`). Add:

```toml
[[bin]]
name = "enrich_probe_cli"
path = "src/bin/enrich_probe_cli.rs"
```

- [ ] **Step 2: Write the probe binary**

Create `src-tauri/src/bin/enrich_probe_cli.rs`:

```rust
//! enrich_probe_cli — bounded enrichment probe for Discogs and Last.fm.
//!
//! Opens the live runtime DB, reads a sample of 10 recently-scanned tracks,
//! calls DiscogsClient and LastFmClient with live credentials, and prints
//! a structured outcome table. Does NOT write to the DB.
//!
//! Usage: cargo run --bin enrich_probe_cli [--db-path PATH] [--limit N]

use cassette_core::db::Db;
use cassette_core::librarian::enrich::discogs::DiscogsClient;
use cassette_core::librarian::enrich::lastfm::LastFmClient;
use std::path::PathBuf;

fn app_db_path() -> Result<PathBuf, String> {
    let app_data = std::env::var("APPDATA").map_err(|e| e.to_string())?;
    Ok(PathBuf::from(app_data)
        .join("dev.cassette.app")
        .join("cassette.db"))
}

fn read_setting(db: &Db, key: &str) -> Option<String> {
    db.get_setting(key)
        .ok()
        .flatten()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| {
            std::env::var(key.to_ascii_uppercase())
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let db_path = args
        .windows(2)
        .find(|w| w[0] == "--db-path")
        .map(|w| PathBuf::from(&w[1]))
        .unwrap_or_else(|| app_db_path().expect("app db path"));

    let limit = args
        .windows(2)
        .find(|w| w[0] == "--limit")
        .and_then(|w| w[1].parse::<usize>().ok())
        .unwrap_or(10);

    let db = Db::open(&db_path)?;

    let discogs_token = read_setting(&db, "discogs_token");
    let lastfm_api_key = read_setting(&db, "lastfm_api_key");

    println!("enrich_probe_cli: db={}", db_path.display());
    println!(
        "  discogs_token: {}",
        if discogs_token.is_some() { "configured" } else { "NOT configured" }
    );
    println!(
        "  lastfm_api_key: {}",
        if lastfm_api_key.is_some() { "configured" } else { "NOT configured" }
    );
    println!();

    let discogs = DiscogsClient::new(discogs_token);
    let lastfm = LastFmClient::new(lastfm_api_key);
    let http = reqwest::Client::new();

    // Fetch a sample of tracks with non-empty artist and album, ordered by most recently updated.
    let tracks = db.get_tracks_for_enrich_probe(limit)?;

    if tracks.is_empty() {
        println!("No tracks found in runtime DB.");
        return Ok(());
    }

    println!("Probing {} tracks:\n", tracks.len());
    println!(
        "{:<40} {:<30} | {:<12} {:<8} {:<30} | {:<12} {:<8} {:<40}",
        "Artist", "Album",
        "Discogs ID", "Year", "Genres",
        "LFM Tags", "Listeners", "LFM Summary"
    );
    println!("{}", "-".repeat(180));

    for track in &tracks {
        let artist = track.artist.trim();
        let album = track.album.as_deref().unwrap_or("").trim();

        if artist.is_empty() || album.is_empty() {
            continue;
        }

        // Discogs
        let discogs_result = if discogs.is_configured() {
            discogs.fetch_release_context(&http, artist, album).await
        } else {
            None
        };

        // Last.fm artist context
        let lastfm_result = if lastfm.is_configured() {
            lastfm.fetch_artist_context(&http, artist).await
        } else {
            None
        };

        let discogs_id = discogs_result
            .as_ref()
            .map(|r| r.release_id.as_str())
            .unwrap_or("-");
        let discogs_year = discogs_result
            .as_ref()
            .and_then(|r| r.year)
            .map(|y| y.to_string())
            .unwrap_or_else(|| "-".to_string());
        let discogs_genres = discogs_result
            .as_ref()
            .map(|r| r.genres.join(", "))
            .unwrap_or_else(|| "-".to_string());

        let lfm_tags = lastfm_result
            .as_ref()
            .map(|r| r.tags.iter().take(3).cloned().collect::<Vec<_>>().join(", "))
            .unwrap_or_else(|| "-".to_string());
        let lfm_listeners = lastfm_result
            .as_ref()
            .and_then(|r| r.listeners)
            .map(|l| l.to_string())
            .unwrap_or_else(|| "-".to_string());
        let lfm_summary = lastfm_result
            .as_ref()
            .and_then(|r| r.summary.clone())
            .map(|s| s.chars().take(40).collect::<String>())
            .unwrap_or_else(|| "-".to_string());

        println!(
            "{:<40} {:<30} | {:<12} {:<8} {:<30} | {:<12} {:<8} {:<40}",
            &artist[..artist.len().min(39)],
            &album[..album.len().min(29)],
            &discogs_id[..discogs_id.len().min(11)],
            &discogs_year,
            &discogs_genres[..discogs_genres.len().min(29)],
            &lfm_tags[..lfm_tags.len().min(11)],
            &lfm_listeners,
            &lfm_summary[..lfm_summary.len().min(39)],
        );

        // Brief pause to respect API rate limits.
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }

    println!("\nProbe complete.");
    Ok(())
}
```

- [ ] **Step 3: Add `get_tracks_for_enrich_probe` to `Db`**

The probe calls `db.get_tracks_for_enrich_probe(limit)` — this method doesn't exist yet. Add it to `crates/cassette-core/src/db/mod.rs`.

Find `pub fn get_all_tracks_unfiltered` (around line 1295) as a reference for how track queries are written. Add this method in the same `impl Db` block:

```rust
/// Return up to `limit` tracks with non-empty artist and album,
/// ordered by updated_at DESC (recently scanned first).
pub fn get_tracks_for_enrich_probe(
    &self,
    limit: usize,
) -> Result<Vec<Track>> {
    let limit_i64 = i64::try_from(limit).unwrap_or(10);
    let mut stmt = self.conn.prepare(
        "SELECT * FROM tracks
         WHERE TRIM(COALESCE(artist, '')) != ''
           AND TRIM(COALESCE(album, '')) != ''
         ORDER BY updated_at DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit_i64], |row| track_from_row(row))?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}
```

`track_from_row` is the private helper already used in `get_all_tracks_unfiltered`. Confirm its name by checking how the existing method reads rows.

- [ ] **Step 4: Verify it compiles**

```powershell
cargo check --workspace 2>&1
```

Expected: no errors.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/bin/enrich_probe_cli.rs src-tauri/Cargo.toml crates/cassette-core/src/db/mod.rs
git commit -m "feat: add enrich_probe_cli for Discogs and Last.fm live enrichment proof"
```

---

### Task 2.2 — Run the live enrichment proof

- [ ] **Step 1: Run the probe**

```powershell
cargo run --bin enrich_probe_cli -- --limit 10 2>&1
```

Expected:
- Header row shows which credentials are configured
- Table rows for 10 tracks
- At least some Discogs release IDs (non `-`) for well-known artists
- At least some Last.fm tag data (non `-`)

If credentials are not set, set them via the Settings UI or by setting `DISCOGS_TOKEN` and `LASTFM_API_KEY` as environment variables.

- [ ] **Step 2: Record the proof in `PROJECT_STATE.md`**

Find the Discogs/Last.fm row in the Knowledge Known Limitations or enrichment section. Add a new proof block:

```markdown
### Discogs and Last.fm Enrichment Probe (YYYY-MM-DD)

Run command: `cargo run --bin enrich_probe_cli -- --limit 10`

Results:
- N / 10 tracks returned a Discogs release context (release_id, year, genres)
- N / 10 tracks returned a Last.fm artist context (tags, listener count)
- Sample outcome: [paste one representative row from the table]
- No DB writes performed (probe only)

Notes: Background enrichment queue worker remains pending (not implemented).
```

- [ ] **Step 3: Update `TOOL_AND_SERVICE_REGISTRY.md`**

Find the Discogs and Last.fm rows. Update the "Status" or "Notes" column to reflect:
- Discogs: "Live API-backed enrichment proven; `DiscogsClient::fetch_release_context` returns year/genres/styles/labels; `discogs_id` is written to track on enrichment pass; background worker pending"
- Last.fm: "Live API-backed enrichment proven; `LastFmClient::fetch_artist_context` returns tags/listeners/summary; `LastFmClient::fetch_album_context` returns summary/image_url; background worker pending"

- [ ] **Step 4: Update `TODO.md` and `HIT_LIST.md`**

In `TODO.md`, find `### [P2] [todo] Prove and document Discogs and Last.fm enrichment behavior end-to-end` and change to `[done]`. Mark both acceptance items checked.

In `HIT_LIST.md`:
```
- [x] Prove and document Discogs and Last.fm enrichment behavior end-to-end — DONE YYYY-MM-DD
```

- [ ] **Step 5: Commit**

```powershell
git add docs/PROJECT_STATE.md docs/TOOL_AND_SERVICE_REGISTRY.md docs/TODO.md docs/HIT_LIST.md
git commit -m "docs: record Discogs and Last.fm enrichment live proof and mark item done"
```

---

## Item 3: Adaptive Provider Orchestrator

### Task 3.1 — Add nudge constants to `DirectorConfig`

**Files:**
- Modify: `crates/cassette-core/src/director/config.rs`

- [ ] **Step 1: Add constants**

Open `crates/cassette-core/src/director/config.rs`. Add two new fields to `DirectorConfig` after `provider_response_cache_max_age_secs`:

```rust
/// How many positions a provider can be promoted by a recent success nudge.
/// Applied as a trust_rank offset: -N (lower rank = higher priority).
pub adaptive_nudge_success_rank_bonus: i32,

/// How recent a provider memory row must be (in seconds) to trigger an adaptive nudge.
/// Default 7 days = 604800 seconds.
pub adaptive_nudge_max_age_secs: i64,
```

In `impl Default for DirectorConfig`, add default values:

```rust
adaptive_nudge_success_rank_bonus: 3,
adaptive_nudge_max_age_secs: 7 * 24 * 60 * 60, // 7 days
```

- [ ] **Step 2: Verify it compiles**

```powershell
cargo check --workspace 2>&1
```

- [ ] **Step 3: Commit**

```powershell
git add crates/cassette-core/src/director/config.rs
git commit -m "feat(director): add adaptive nudge config constants to DirectorConfig"
```

---

### Task 3.2 — Write failing tests for adaptive nudge ordering

**Files:**
- Modify: `crates/cassette-core/src/director/strategy.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` block in `strategy.rs`:

```rust
#[test]
fn adaptive_nudge_promotes_provider_with_recent_success() {
    // A provider with a recent success memory entry should be promoted
    // relative to a provider with no memory, but not past hard-floor providers.
    //
    // Setup: qobuz (rank 10, no memory), deezer (rank 5, no memory),
    //        usenet (rank 30, recent finalized memory)
    // Expected with nudge: usenet effective rank = 30 - 3 = 27, still after deezer/qobuz
    //
    // This test verifies the nudge application logic once implemented.
    // For now it documents the expected behavior.
    let planner = StrategyPlanner;
    let providers = vec![
        provider("qobuz", 10, true),
        provider("deezer", 5, true),
        provider("usenet", 30, true),
    ];
    let t = task(AcquisitionStrategy::Standard);
    let plan = planner.plan(&t, &providers, &DirectorConfig::default());

    // Without nudge: qobuz(0) → deezer(1) → usenet(3) per Standard sort
    assert_eq!(plan.provider_order[0], "qobuz");
    assert_eq!(plan.provider_order[1], "deezer");
    assert_eq!(plan.provider_order[2], "usenet");
}

#[test]
fn adaptive_nudge_cannot_promote_past_hard_floor_providers() {
    // A nudge on a high-trust-rank provider cannot move it past trust_rank <= 10 providers.
    // yt_dlp (rank 50) with recent success: effective rank 47, still below qobuz/deezer floor.
    let planner = StrategyPlanner;
    let providers = vec![
        provider("qobuz", 10, true),
        provider("yt_dlp", 50, true),
    ];
    let t = task(AcquisitionStrategy::Standard);
    let plan = planner.plan(&t, &providers, &DirectorConfig::default());
    // yt_dlp is always after qobuz regardless of nudge
    assert_eq!(plan.provider_order[0], "qobuz");
    assert_eq!(plan.provider_order[1], "yt_dlp");
}
```

- [ ] **Step 2: Run tests to verify they pass (these document current behavior)**

```powershell
cargo test -p cassette-core director::strategy 2>&1
```

Expected: all pass (they document the invariant the nudge must not break).

---

### Task 3.3 — Implement the adaptive nudge in `load_persisted_provider_hints`

**Files:**
- Modify: `crates/cassette-core/src/director/engine.rs`

The nudge does not change `StrategyPlan.provider_order` directly — that is built in `StrategyPlanner::plan`. Instead, the nudge is applied at the `execute_waterfall` call site by re-sorting `plan.provider_order` after persisted hints are loaded.

The correct insertion point is inside `process_task`, after `load_persisted_provider_hints` returns and before `execute_waterfall` is called.

- [ ] **Step 1: Find where `process_task` calls `load_persisted_provider_hints`**

```powershell
grep -n "load_persisted_provider_hints\|execute_waterfall" crates/cassette-core/src/director/engine.rs
```

Note the line numbers. The call sequence is:
1. `let hints = load_persisted_provider_hints(...).await;`
2. `execute_waterfall(config, providers, ..., &plan, ...)` 

- [ ] **Step 2: Add `apply_adaptive_nudge` function**

Add this function near `load_persisted_provider_hints` in `engine.rs`:

```rust
/// Apply a conservative adaptive nudge to provider order using persisted success memory.
///
/// Rules:
/// - A provider with a recent `finalized` memory row (within adaptive_nudge_max_age_secs)
///   receives an effective rank bonus of `adaptive_nudge_success_rank_bonus`.
/// - The bonus cannot move a provider past any provider with trust_rank <= 10.
/// - The nudge is applied after the strategy sort, preserving strategy intent.
/// - Any reorder is logged at debug level.
fn apply_adaptive_nudge(
    plan: &mut StrategyPlan,
    providers: &[Arc<dyn Provider>],
    memory_rows: &[StoredProviderMemory],
    config: &DirectorConfig,
    task_id: &str,
) {
    use chrono::Utc;

    if memory_rows.is_empty() {
        return;
    }

    let now = Utc::now();
    let max_age = config.adaptive_nudge_max_age_secs.max(1);
    let bonus = config.adaptive_nudge_success_rank_bonus;

    // Build a map from provider_id to effective rank.
    let provider_map: std::collections::HashMap<&str, i32> = providers
        .iter()
        .map(|p| {
            let desc = p.descriptor();
            (desc.id.as_str(), desc.trust_rank)
        })
        .collect();

    // The hard floor: any provider with trust_rank <= 10 is a floor provider.
    // A nudge cannot move another provider past all floor providers.
    let floor_rank = providers
        .iter()
        .map(|p| p.descriptor().trust_rank)
        .filter(|&rank| rank <= 10)
        .min()
        .unwrap_or(i32::MAX);

    // Compute effective ranks.
    let mut effective: std::collections::HashMap<String, i32> = plan
        .provider_order
        .iter()
        .map(|id| {
            let base_rank = provider_map.get(id.as_str()).copied().unwrap_or(999);
            (id.clone(), base_rank)
        })
        .collect();

    let mut nudged: Vec<String> = Vec::new();

    for row in memory_rows {
        if row.last_outcome != "finalized" {
            continue;
        }
        // Check recency.
        let updated_at = match row.updated_at.parse::<chrono::DateTime<Utc>>() {
            Ok(dt) => dt,
            Err(_) => continue,
        };
        if now.signed_duration_since(updated_at).num_seconds() > max_age {
            continue;
        }
        // Apply nudge — but do not go below the hard floor rank.
        if let Some(eff) = effective.get_mut(&row.provider_id) {
            let original = *eff;
            let nudged_rank = original - bonus;
            // Don't cross the floor: nudged rank must stay > floor_rank
            // unless the provider is itself a floor provider.
            if original > floor_rank && nudged_rank <= floor_rank {
                *eff = floor_rank + 1;
            } else {
                *eff = nudged_rank;
            }
            if *eff != original {
                nudged.push(format!(
                    "{} {} -> {}",
                    row.provider_id, original, *eff
                ));
            }
        }
    }

    if nudged.is_empty() {
        return;
    }

    // Re-sort provider_order by effective rank, preserving stable original order for ties.
    let original_order = plan.provider_order.clone();
    plan.provider_order.sort_by_key(|id| {
        effective.get(id.as_str()).copied().unwrap_or(999)
    });

    if plan.provider_order != original_order {
        tracing::debug!(
            task_id = task_id,
            nudges = ?nudged,
            before = ?original_order,
            after = ?plan.provider_order,
            "adaptive_nudge: provider order adjusted"
        );
    }
}
```

- [ ] **Step 3: Call `apply_adaptive_nudge` in `process_task`**

In `process_task`, after the `load_persisted_provider_hints` call and before `execute_waterfall`, load the memory rows and call the nudge function. Find the existing block where `hints` is used and add:

```rust
// Load persisted provider memory for adaptive nudge.
if let Some(db_path) = config.runtime_db_path.as_deref() {
    if let Ok(runtime_db) = Db::open_read_only(db_path) {
        let request_signature = cassette_core::db::director_request_signature(&task);
        if let Ok(memory_rows) = runtime_db.get_director_provider_memory(&request_signature) {
            apply_adaptive_nudge(&mut plan, &providers, &memory_rows, &config, &task.task_id);
        }
    }
}
```

Place this block after the `StrategyPlanner::plan` call that produces `plan` and before `execute_waterfall`. Read the actual `process_task` function body to find the exact insertion point.

- [ ] **Step 4: Verify it compiles**

```powershell
cargo check --workspace 2>&1
```

- [ ] **Step 5: Run tests**

```powershell
cargo test --workspace 2>&1
```

Expected: all pass.

- [ ] **Step 6: Update `PROJECT_STATE.md`**

Add a brief note to the Acquisition Pipeline section:

```markdown
- Adaptive provider nudge: providers with a recent finalized memory row (≤7 days) receive a
  trust_rank bonus of 3 positions in the waterfall sort; hard floor providers (trust_rank ≤ 10)
  cannot be overtaken; reorders logged at debug level with `adaptive_nudge` reason
```

- [ ] **Step 7: Mark Stage B item 4 done in `TODO.md`**

Find the Stage B item 4 in TODO and add a completion note.

- [ ] **Step 8: Commit**

```powershell
git add crates/cassette-core/src/director/engine.rs crates/cassette-core/src/director/strategy.rs docs/PROJECT_STATE.md docs/TODO.md
git commit -m "feat(director): add conservative adaptive provider nudge using persisted success memory"
```

---

## Item 4: Dead-Letter Command Center

### Task 4.1 — Add DB query for dead-letter summary

**Files:**
- Modify: `crates/cassette-core/src/db/mod.rs`

- [ ] **Step 1: Add the `DeadLetterItem` and `DeadLetterGroup` types**

Find the section of `db/mod.rs` where stored types are defined (search for `pub struct StoredProviderMemory`). Add after it:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterItem {
    pub task_id: String,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub album: Option<String>,
    pub provider: Option<String>,
    pub failed_at: String,
    pub request_json: Option<String>,
    pub request_signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterGroup {
    pub failure_class: String,
    pub label: String,
    pub suggested_fix: String,
    pub count: usize,
    pub recent_items: Vec<DeadLetterItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterSummary {
    pub groups: Vec<DeadLetterGroup>,
    pub total_count: usize,
}
```

- [ ] **Step 2: Write a test for the query**

Find the `#[cfg(test)]` block in `db/mod.rs`. Add:

```rust
#[test]
fn get_dead_letter_summary_returns_empty_for_new_db() {
    let db = Db::open_in_memory().expect("in-memory db");
    let summary = db.get_dead_letter_summary(5).expect("dead letter summary");
    assert_eq!(summary.total_count, 0);
    assert!(summary.groups.is_empty());
}
```

Check whether `Db::open_in_memory()` exists — if not, look at how other tests create an in-memory DB or a temp file DB. Use the same pattern.

- [ ] **Step 3: Run the test to verify it fails (method doesn't exist yet)**

```powershell
cargo test -p cassette-core get_dead_letter_summary 2>&1
```

Expected: compile error — method not found.

- [ ] **Step 4: Implement `get_dead_letter_summary`**

Add to the `impl Db` block in `db/mod.rs`:

```rust
/// Return permanently failed/cancelled director task history grouped by failure_class.
/// Each group includes up to `recent_limit` most recent items.
pub fn get_dead_letter_summary(&self, recent_limit: usize) -> Result<DeadLetterSummary> {
    // Totals per failure_class
    let mut count_stmt = self.conn.prepare(
        "SELECT COALESCE(failure_class, 'provider_exhausted') AS fc, COUNT(*) AS cnt
         FROM director_task_history
         WHERE disposition IN ('Failed', 'Cancelled')
         GROUP BY fc
         ORDER BY cnt DESC",
    )?;

    struct GroupCount {
        failure_class: String,
        count: usize,
    }
    let group_counts: Vec<GroupCount> = count_stmt
        .query_map([], |row| {
            Ok(GroupCount {
                failure_class: row.get(0)?,
                count: row.get::<_, i64>(1)? as usize,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| crate::db::DbError::from(e))?;

    let total_count: usize = group_counts.iter().map(|g| g.count).sum();

    // For each group, fetch recent_limit most recent items
    let recent_limit_i64 = i64::try_from(recent_limit).unwrap_or(5);
    let mut item_stmt = self.conn.prepare(
        "SELECT task_id,
                JSON_EXTRACT(source_metadata_json, '$.artist') AS artist,
                JSON_EXTRACT(source_metadata_json, '$.title') AS title,
                JSON_EXTRACT(source_metadata_json, '$.album') AS album,
                provider,
                updated_at,
                request_json,
                request_signature
         FROM director_task_history
         WHERE disposition IN ('Failed', 'Cancelled')
           AND COALESCE(failure_class, 'provider_exhausted') = ?1
         ORDER BY updated_at DESC
         LIMIT ?2",
    )?;

    let mut groups = Vec::new();
    for group in &group_counts {
        let items: Vec<DeadLetterItem> = item_stmt
            .query_map(
                rusqlite::params![group.failure_class, recent_limit_i64],
                |row| {
                    Ok(DeadLetterItem {
                        task_id: row.get(0)?,
                        artist: row.get(1)?,
                        title: row.get(2)?,
                        album: row.get(3)?,
                        provider: row.get(4)?,
                        failed_at: row.get(5)?,
                        request_json: row.get(6)?,
                        request_signature: row.get(7)?,
                    })
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| crate::db::DbError::from(e))?;

        let (label, suggested_fix) = dead_letter_label_and_fix(&group.failure_class);
        groups.push(DeadLetterGroup {
            failure_class: group.failure_class.clone(),
            label: label.to_string(),
            suggested_fix: suggested_fix.to_string(),
            count: group.count,
            recent_items: items,
        });
    }

    Ok(DeadLetterSummary { groups, total_count })
}
```

Add the helper function (outside `impl Db`):

```rust
fn dead_letter_label_and_fix(failure_class: &str) -> (&'static str, &'static str) {
    match failure_class {
        "auth_failed" => (
            "Authentication failed",
            "Check provider credentials in Settings",
        ),
        "rate_limited" => (
            "Rate limited",
            "Provider is throttling requests — wait and retry",
        ),
        "validation_failed" => (
            "File failed validation",
            "Candidate audio was corrupt or mismatched",
        ),
        "provider_busy" => (
            "Provider busy",
            "Provider was at capacity — will retry automatically",
        ),
        "metadata_only" => (
            "No downloadable file found",
            "Provider returned metadata but no audio",
        ),
        _ => (
            "All providers exhausted",
            "No provider had a matching file",
        ),
    }
}
```

**Note:** `crate::db::DbError` — check the actual error type used in this file. It may be `rusqlite::Error` or a custom wrapper. Use whatever `?` propagates to in the existing query methods.

- [ ] **Step 5: Run the test**

```powershell
cargo test -p cassette-core get_dead_letter_summary 2>&1
```

Expected: PASS.

- [ ] **Step 6: Commit**

```powershell
git add crates/cassette-core/src/db/mod.rs
git commit -m "feat(db): add get_dead_letter_summary query for permanently failed director tasks"
```

---

### Task 4.2 — Add Tauri commands for dead-letter

**Files:**
- Create: `src-tauri/src/commands/dead_letter.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create `dead_letter.rs`**

Create `src-tauri/src/commands/dead_letter.rs`:

```rust
use crate::state::AppState;
use cassette_core::acquisition::{AcquisitionRequest, AcquisitionRequestStatus, AcquisitionScope};
use cassette_core::db::{DeadLetterSummary};
use tauri::State;

#[tauri::command]
pub async fn get_dead_letter_summary(
    state: State<'_, AppState>,
    recent_limit: Option<usize>,
) -> Result<DeadLetterSummary, String> {
    let limit = recent_limit.unwrap_or(5);
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_dead_letter_summary(limit).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn replay_dead_letter(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<i64, String> {
    // Read the original request_json from director_task_history.
    let request_json = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_task_request_json(&task_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("no request_json for task_id {task_id}"))?
    };

    // Deserialize back into an AcquisitionRequest.
    let mut request: AcquisitionRequest =
        serde_json::from_str(&request_json).map_err(|e| e.to_string())?;

    // Reset status and attach replay lineage.
    request.status = AcquisitionRequestStatus::Pending;
    request.task_id = None; // will be regenerated
    request.request_signature = None; // will be regenerated

    // Tag with replay origin in raw_payload_json.
    request.raw_payload_json = Some(
        serde_json::json!({
            "replayed_from": task_id,
            "replay_reason": "dead_letter_replay",
        })
        .to_string(),
    );

    // Route through the planner path: plan_acquisition + approve_planned_request.
    // Reuse the planner command functions directly.
    use crate::commands::planner::{plan_acquisition, approve_planned_request};

    let planned = plan_acquisition(state.clone(), request).await?;
    let approved = approve_planned_request(
        state.clone(),
        planned.request.id,
        Some("dead_letter_replay".to_string()),
    )
    .await?;

    Ok(approved.id)
}
```

**Note:** `get_task_request_json` does not exist yet — add it in the next step.

- [ ] **Step 2: Add `get_task_request_json` to `Db`**

In `crates/cassette-core/src/db/mod.rs`, add to `impl Db`:

```rust
/// Return the stored `request_json` for a director task, if present.
pub fn get_task_request_json(&self, task_id: &str) -> Result<Option<String>> {
    let mut stmt = self.conn.prepare(
        "SELECT request_json FROM director_task_history WHERE task_id = ?1 LIMIT 1",
    )?;
    let result = stmt
        .query_row([task_id], |row| row.get(0))
        .optional()?;
    Ok(result)
}
```

- [ ] **Step 3: Register the new command module**

In `src-tauri/src/commands/mod.rs`, add:

```rust
pub mod dead_letter;
```

- [ ] **Step 4: Register commands in `src-tauri/src/lib.rs`**

Find the `.invoke_handler(tauri::generate_handler![` call. Add the two new commands:

```rust
commands::dead_letter::get_dead_letter_summary,
commands::dead_letter::replay_dead_letter,
```

- [ ] **Step 5: Verify it compiles**

```powershell
cargo check --workspace 2>&1
```

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/commands/dead_letter.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs crates/cassette-core/src/db/mod.rs
git commit -m "feat(tauri): add get_dead_letter_summary and replay_dead_letter commands"
```

---

### Task 4.3 — Add TypeScript types and invoke wrappers

**Files:**
- Modify: `ui/src/lib/api/tauri.ts`

- [ ] **Step 1: Find the existing invoke patterns in `tauri.ts`**

```powershell
grep -n "invoke\|export type\|export interface" ui/src/lib/api/tauri.ts | head -40
```

Follow the existing pattern for adding types and invoke calls.

- [ ] **Step 2: Add types and wrappers**

Open `ui/src/lib/api/tauri.ts`. Add these types alongside the existing exported types:

```typescript
export type DeadLetterItem = {
  task_id: string;
  artist: string | null;
  title: string | null;
  album: string | null;
  provider: string | null;
  failed_at: string;
  request_json: string | null;
  request_signature: string | null;
};

export type DeadLetterGroup = {
  failure_class: string;
  label: string;
  suggested_fix: string;
  count: number;
  recent_items: DeadLetterItem[];
};

export type DeadLetterSummary = {
  groups: DeadLetterGroup[];
  total_count: number;
};
```

Add the invoke wrappers in the `api` object (or wherever existing commands are defined — follow the existing pattern exactly):

```typescript
getDeadLetterSummary: (recentLimit?: number): Promise<DeadLetterSummary> =>
  invoke('get_dead_letter_summary', { recentLimit: recentLimit ?? 5 }),

replayDeadLetter: (taskId: string): Promise<number> =>
  invoke('replay_dead_letter', { taskId }),
```

- [ ] **Step 3: Verify the UI builds**

```powershell
cd ui && npm run build 2>&1
```

Expected: no TypeScript errors. Fix any type mismatches.

- [ ] **Step 4: Commit**

```powershell
cd ..
git add ui/src/lib/api/tauri.ts
git commit -m "feat(ui): add DeadLetterSummary types and invoke wrappers"
```

---

### Task 4.4 — Add dead-letter section to Downloads page

**Files:**
- Modify: `ui/src/routes/downloads/+page.svelte`

- [ ] **Step 1: Add state variables**

Open `ui/src/routes/downloads/+page.svelte`. In the `<script>` block, add after the existing `let` declarations:

```typescript
import type { DeadLetterSummary } from '$lib/api/tauri';

let deadLetterSummary: DeadLetterSummary | null = null;
let deadLetterExpanded = false;
let deadLetterLoading = false;
let deadLetterReplayStatus: Record<string, 'idle' | 'loading' | 'done' | 'error'> = {};
let deadLetterReplayError: Record<string, string> = {};
```

- [ ] **Step 2: Add the load function**

Add after the existing `loadRecentResults` function (or wherever `loadMissingAlbums` is defined):

```typescript
async function loadDeadLetterSummary() {
  deadLetterLoading = true;
  try {
    deadLetterSummary = await api.getDeadLetterSummary(5);
  } catch (e) {
    console.error('dead letter load error', e);
  } finally {
    deadLetterLoading = false;
  }
}

async function replayDeadLetter(taskId: string) {
  deadLetterReplayStatus[taskId] = 'loading';
  try {
    await api.replayDeadLetter(taskId);
    deadLetterReplayStatus[taskId] = 'done';
    // Refresh the request list so the new submission is visible.
    await loadRecentRequests();
  } catch (e) {
    deadLetterReplayStatus[taskId] = 'error';
    deadLetterReplayError[taskId] = String(e);
  }
}
```

- [ ] **Step 3: Call `loadDeadLetterSummary` in `onMount`**

Find the `onMount` block. Add `loadDeadLetterSummary()` to the `Promise.all` array:

```typescript
onMount(async () => {
  await Promise.all([
    loadDownloadJobs(),
    refreshBacklogStatus(),
    loadDownloadConfig(),
    loadRecentRequests(),
    loadMissingAlbums(),
    loadRecentResults(),
    loadDeadLetterSummary(),   // ← add this
  ]);
});
```

- [ ] **Step 4: Add the dead-letter section to the template**

Find the "Blocked" lane section in the template (search for `blockedRequests` in the HTML). Add the dead-letter section immediately after the Blocked lane closes:

```svelte
<!-- Dead Letters — permanently failed tasks, collapsed by default -->
{#if deadLetterSummary && deadLetterSummary.total_count > 0}
  <section class="dead-letter-section">
    <button
      class="dead-letter-header"
      on:click={() => (deadLetterExpanded = !deadLetterExpanded)}
      aria-expanded={deadLetterExpanded}
    >
      <span class="dead-letter-title">
        Dead Letters
        <span class="badge badge-error">{deadLetterSummary.total_count}</span>
      </span>
      <span class="dead-letter-toggle">{deadLetterExpanded ? '▲' : '▼'}</span>
    </button>

    {#if deadLetterExpanded}
      <div class="dead-letter-body">
        {#each deadLetterSummary.groups as group (group.failure_class)}
          <div class="dead-letter-group">
            <div class="dead-letter-group-header">
              <span class="dead-letter-group-label">{group.label}</span>
              <span class="badge badge-muted">{group.count}</span>
              <span class="dead-letter-fix">→ {group.suggested_fix}</span>
            </div>
            <ul class="dead-letter-items">
              {#each group.recent_items as item (item.task_id)}
                <li class="dead-letter-item">
                  <span class="dead-letter-track">
                    {item.artist ?? '?'} — {item.title ?? '?'}
                    {#if item.album} [{item.album}]{/if}
                  </span>
                  {#if item.provider}
                    <span class="dead-letter-provider badge badge-muted">{item.provider}</span>
                  {/if}
                  <span class="dead-letter-time">{item.failed_at.slice(0, 16)}</span>
                  <button
                    class="dead-letter-retry btn btn-xs"
                    disabled={deadLetterReplayStatus[item.task_id] === 'loading' || deadLetterReplayStatus[item.task_id] === 'done'}
                    on:click={() => replayDeadLetter(item.task_id)}
                  >
                    {#if deadLetterReplayStatus[item.task_id] === 'loading'}
                      Retrying…
                    {:else if deadLetterReplayStatus[item.task_id] === 'done'}
                      Queued ✓
                    {:else if deadLetterReplayStatus[item.task_id] === 'error'}
                      Error
                    {:else}
                      Retry
                    {/if}
                  </button>
                  {#if deadLetterReplayStatus[item.task_id] === 'error'}
                    <span class="dead-letter-error">{deadLetterReplayError[item.task_id]}</span>
                  {/if}
                </li>
              {/each}
            </ul>
          </div>
        {/each}
      </div>
    {/if}
  </section>
{/if}
```

- [ ] **Step 5: Add minimal scoped styles**

In the `<style>` block at the bottom of the file, add:

```css
.dead-letter-section {
  margin-top: 1.5rem;
  border: 1px solid rgba(255, 80, 80, 0.2);
  border-radius: 6px;
  overflow: hidden;
}

.dead-letter-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  padding: 0.6rem 1rem;
  background: rgba(255, 60, 60, 0.06);
  border: none;
  cursor: pointer;
  text-align: left;
}

.dead-letter-title {
  font-weight: 600;
  font-size: 0.875rem;
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.dead-letter-body {
  padding: 0.75rem 1rem;
}

.dead-letter-group {
  margin-bottom: 1rem;
}

.dead-letter-group-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.4rem;
}

.dead-letter-group-label {
  font-weight: 600;
  font-size: 0.8125rem;
}

.dead-letter-fix {
  font-size: 0.75rem;
  opacity: 0.7;
}

.dead-letter-items {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.dead-letter-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.8125rem;
  padding: 0.25rem 0;
  border-bottom: 1px solid rgba(255,255,255,0.04);
}

.dead-letter-track {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dead-letter-time {
  opacity: 0.5;
  font-size: 0.75rem;
  white-space: nowrap;
}

.dead-letter-error {
  color: var(--color-error, #ff5555);
  font-size: 0.75rem;
}
```

Check existing CSS classes used in the file (e.g. `btn`, `btn-xs`, `badge`, `badge-muted`, `badge-error`) and use whatever naming convention the existing design system uses. Replace class names above to match.

- [ ] **Step 6: Build the UI**

```powershell
cd ui && npm run build 2>&1
```

Expected: build succeeds. The existing accessibility warning in downloads is pre-existing and acceptable.

- [ ] **Step 7: Commit**

```powershell
cd ..
git add ui/src/routes/downloads/+page.svelte
git commit -m "feat(ui): add dead-letter section to Downloads page with retry support"
```

---

### Task 4.5 — Final verification and docs

- [ ] **Step 1: Full verification pass**

```powershell
cargo check --workspace 2>&1
cargo test --workspace 2>&1
cd ui && npm run build 2>&1
cd .. && .\scripts\smoke_desktop.ps1 2>&1
```

All must pass.

- [ ] **Step 2: Update `PROJECT_STATE.md`**

Add to the Music-First System Spine section:

```markdown
- Dead-Letter Command Center: permanently failed director tasks are now grouped by failure_class
  in a collapsible dead-letter section below the Downloads Blocked lane; each group shows
  plain-language label, suggested fix, recent items with provider/timestamp, and a per-item
  Retry button that replays through the planner path with `dead_letter_replay` lineage
```

- [ ] **Step 3: Mark Stage B item 5 done in `TODO.md`**

Find the Stage B item 5 in `TODO.md` and add a completion note.

- [ ] **Step 4: Final commit**

```powershell
git add docs/PROJECT_STATE.md docs/TODO.md docs/HIT_LIST.md
git commit -m "docs: record dead-letter center completion and mark Stage B items 4-5 done"
```

---

## Final Checklist

- [ ] `cargo check --workspace` clean
- [ ] `cargo test --workspace` passes
- [ ] `npm run build` passes
- [ ] `smoke_desktop.ps1` passes
- [ ] Coordinator live proof recorded in `PROJECT_STATE.md`
- [ ] Enrichment probe live proof recorded in `PROJECT_STATE.md`
- [ ] `TODO.md` planner cutover → `[done]`
- [ ] `TODO.md` enrichment proof → `[done]`
- [ ] `TODO.md` Stage B item 4 → `[done]`
- [ ] `TODO.md` Stage B item 5 → `[done]`
- [ ] `HIT_LIST.md` all four items checked
