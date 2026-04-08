# Phase 3: Collector Depth Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the surfaces that make Cassette feel like a serious collector's system — play history, collection statistics, the expanded Now Playing panel, player event-push architecture, artist gap detection, and the cleanup of the remaining operator debt. After Phase 3, Cassette feels like it belongs to someone with taste and treats them as such.

**Architecture:** Mix of Rust backend additions (new DB queries, event emission for player state), SvelteKit new routes, and targeted architectural cleanups (legacy module removal, operator CLI relocation). Each task is independently shippable.

**Tech Stack:** Rust (Tauri 2 event emission, cassette-core DB layer), SvelteKit, TypeScript.

**Prerequisite:** Phase 1 and Phase 2 complete.

---

## File Map

| File | Change |
|------|--------|
| `ui/src/routes/history/+page.svelte` | New — play history and most-played |
| `ui/src/routes/collection/+page.svelte` | New — collection statistics dashboard |
| `ui/src/lib/components/NowPlayingExpanded.svelte` | New — expanded now playing overlay |
| `ui/src/lib/components/NowPlaying.svelte` | Trigger expanded overlay on art click |
| `ui/src/routes/artists/+page.svelte` | Show gap count ("N albums not in library") on artist page |
| `src-tauri/src/commands/library.rs` | Add `get_play_history`, `get_collection_stats`, `get_artist_gap` |
| `src-tauri/src/commands/player.rs` | Emit `player_state_changed` event on every state mutation |
| `src-tauri/src/lib.rs` | Register new commands |
| `ui/src/lib/stores/player.ts` | Switch from polling to event subscription |
| `ui/src/lib/api/tauri.ts` | Add new API wrappers |
| `crates/cassette-core/src/db/mod.rs` | Add `get_play_history`, `get_collection_stats`, `get_artist_spotify_gap` |
| `crates/cassette-core/src/lib.rs` | Remove `pub mod downloader` (dead code) |

---

### Task 1: Remove the `downloader` legacy re-export module

**Files:**
- Modify: `crates/cassette-core/src/lib.rs`
- Possibly: any import sites of `cassette_core::downloader`

This module is a compatibility re-export that the TODO marks as resolved and the git status shows as deleted in staging.

- [ ] **Step 1: Find all import sites of the downloader module**

```bash
grep -rn "cassette_core::downloader\|use crate::downloader\|mod downloader" "c:/Cassette Music" --include="*.rs" | grep -v "target/"
```

List every hit. These are the files that need updating.

- [ ] **Step 2: Remove `pub mod downloader` from lib.rs**

Open `crates/cassette-core/src/lib.rs`. Find and delete the line:

```rust
pub mod downloader;
```

- [ ] **Step 3: Fix any import sites found in Step 1**

For each file that imports from `cassette_core::downloader` or `crate::downloader`, update the import to point directly to the canonical type location. The types re-exported by `downloader` come from `director/providers/` — check the actual `downloader/mod.rs` content (if still present) to see what it re-exports, then update each import to its canonical path.

If `crates/cassette-core/src/downloader/mod.rs` still exists on disk, delete it:
```bash
rm "c:/Cassette Music/crates/cassette-core/src/downloader/mod.rs"
```

- [ ] **Step 4: Build to verify no remaining references**

```bash
cd "c:/Cassette Music" && cargo check --workspace 2>&1 | grep -i "downloader\|error"
```

Expected: no downloader-related errors.

- [ ] **Step 5: Commit**

```bash
cd "c:/Cassette Music" && git add -A && git commit -m "chore(core): remove downloader legacy re-export module — director/providers is canonical"
```

---

### Task 2: Player event emission — replace polling with push

**Files:**
- Modify: `src-tauri/src/commands/player.rs`
- Modify: `src-tauri/src/lib.rs` (ensure AppHandle is accessible)
- Modify: `ui/src/lib/stores/player.ts`

Currently the frontend polls player state on a timer. We add a Tauri event emission on every state-mutating player command so the frontend can subscribe instead.

- [ ] **Step 1: Add a `PlaybackStateChanged` event emission helper**

In `src-tauri/src/commands/player.rs`, add an import for AppHandle and a helper function:

```rust
use tauri::{AppHandle, Emitter, State};

fn emit_playback_state(app: &AppHandle, state: &crate::state::AppState) {
    let ps = state.playback_state.lock().unwrap().clone();
    let _ = app.emit("playback_state_changed", &ps);
}
```

- [ ] **Step 2: Add `AppHandle` parameter to each state-mutating command and call `emit_playback_state`**

Update `player_play`, `player_pause`, `player_toggle`, `player_seek`, `player_set_volume`, `player_next`, `player_prev`, and `player_load` to accept `app: AppHandle` and call `emit_playback_state(&app, &state)` at the end of each command.

Example for `player_play`:

```rust
#[tauri::command]
pub fn player_play(app: AppHandle, state: State<'_, AppState>) {
    // ... existing body unchanged ...
    emit_playback_state(&app, &state);
}
```

Apply the same `app: AppHandle` addition and `emit_playback_state` call to each of the other listed commands. Do not change any existing logic — only add the parameter and the emit call at the end.

- [ ] **Step 3: Verify the build compiles**

```bash
cd "c:/Cassette Music" && cargo check --workspace 2>&1 | grep "error"
```

Expected: no errors. (Warnings about unused variables from prior work are acceptable.)

- [ ] **Step 4: Update the player store to listen for events**

Open `ui/src/lib/stores/player.ts`. Find `startPlayerPoll` and `stopPlayerPoll`. Modify them to also set up event subscription:

```typescript
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

let unlistenPlayerEvent: UnlistenFn | null = null;

export async function startPlayerEventListener() {
  if (unlistenPlayerEvent) return;
  unlistenPlayerEvent = await listen<PlaybackState>('playback_state_changed', (event) => {
    playbackState.set(event.payload);
    isPlaying.set(event.payload.is_playing);
    if (!get(isSeeking)) {
      progressPct.set(
        event.payload.duration_secs > 0
          ? event.payload.position_secs / event.payload.duration_secs
          : 0,
      );
    }
  });
}

export function stopPlayerEventListener() {
  if (unlistenPlayerEvent) {
    unlistenPlayerEvent();
    unlistenPlayerEvent = null;
  }
}
```

You will need `import { get } from 'svelte/store';` at the top if not already present.

- [ ] **Step 5: Call `startPlayerEventListener` in layout `onMount`**

In `ui/src/routes/+layout.svelte`, update the player startup:

```svelte
import { startPlayerPoll, stopPlayerPoll, startPlayerEventListener, stopPlayerEventListener } from '$lib/stores/player';

onMount(() => {
  void loadDynamicGlassPrefs();
  startPlayerPoll();                 // keep polling as fallback for position updates
  void startPlayerEventListener();   // primary: event-driven state sync
  // ... rest of onMount
});

onDestroy(() => {
  stopPlayerPoll();
  stopPlayerEventListener();
  stopDownloadSupervision();
});
```

The poll continues to run for position slider updates (which need sub-second granularity); the event listener handles all state transitions (play/pause/track change/volume) immediately.

- [ ] **Step 6: Build and verify**

```bash
cd "c:/Cassette Music" && cargo check --workspace 2>&1 | tail -5
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -5
```

Expected: both pass.

- [ ] **Step 7: Commit**

```bash
cd "c:/Cassette Music" && git add src-tauri/src/commands/player.rs ui/src/lib/stores/player.ts ui/src/routes/+layout.svelte && git commit -m "feat(player): emit playback_state_changed events from Rust — frontend subscribes instead of polling-only"
```

---

### Task 3: Add play history and collection stats DB queries

**Files:**
- Modify: `crates/cassette-core/src/db/mod.rs`
- Modify: `src-tauri/src/commands/library.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `ui/src/lib/api/tauri.ts`

The DB already has `play_count` on `tracks`, `artist_play_history`, and `song_play_history` tables. We expose queries for:
1. Most played tracks (for history page)
2. Recently played tracks (from `last_played` on tracks)
3. Collection stats (totals by format, by decade, quality distribution)

- [ ] **Step 1: Write failing tests for the new DB methods**

Add to the `#[cfg(test)]` module in `db/mod.rs`:

```rust
#[test]
fn test_get_most_played_tracks() {
    let db = CassetteDb::open_in_memory().expect("in-memory db");
    db.run_migrations().expect("migrations");

    db.conn.lock().unwrap().execute(
        "INSERT INTO tracks (path,title,artist,album,album_artist,duration_secs,file_size,format,play_count)
         VALUES ('/a.flac','A','Art','Alb','Art',180.0,1024,'flac',5)",
        [],
    ).expect("insert");

    let top = db.get_most_played_tracks(10).expect("query");
    assert_eq!(top.len(), 1);
    assert_eq!(top[0].title, "A");
}

#[test]
fn test_get_collection_stats() {
    let db = CassetteDb::open_in_memory().expect("in-memory db");
    db.run_migrations().expect("migrations");

    db.conn.lock().unwrap().execute(
        "INSERT INTO tracks (path,title,artist,album,album_artist,duration_secs,file_size,format,year)
         VALUES ('/a.flac','A','Art','Alb','Art',180.0,1024,'flac',1995)",
        [],
    ).expect("insert");

    let stats = db.get_collection_stats().expect("stats");
    assert_eq!(stats.total_tracks, 1);
    assert!(stats.by_format.contains_key("flac"));
    assert!(stats.by_decade.contains_key("1990s"));
}
```

- [ ] **Step 2: Run tests to confirm FAIL**

```bash
cd "c:/Cassette Music" && cargo test -p cassette-core test_get_most_played_tracks test_get_collection_stats 2>&1 | tail -15
```

Expected: FAIL.

- [ ] **Step 3: Add the `CollectionStats` struct to `cassette-core/src/models.rs` or `db/mod.rs`**

Add the struct near the top of `db/mod.rs` (or in `models.rs` — use whichever the existing model types live in):

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct CollectionStats {
    pub total_tracks: i64,
    pub total_albums: i64,
    pub total_duration_secs: f64,
    pub by_format: std::collections::HashMap<String, i64>,
    pub by_decade: std::collections::HashMap<String, i64>,
    pub by_quality_tier: std::collections::HashMap<String, i64>,
    pub lossless_count: i64,
    pub hires_count: i64,
}
```

- [ ] **Step 4: Add DB methods**

In `db/mod.rs`, add after the `get_recently_finalized_tracks` method from Phase 2:

```rust
/// Returns tracks ordered by play_count descending, limit is capped at 100.
pub fn get_most_played_tracks(&self, limit: u32) -> Result<Vec<Track>> {
    let conn = self.conn.lock().unwrap();
    let effective_limit = limit.min(100) as i64;
    let mut stmt = conn.prepare(
        "SELECT id,path,title,artist,album,album_artist,track_number,disc_number,
                year,duration_secs,sample_rate,bit_depth,bitrate_kbps,format,file_size,
                cover_art_path,isrc,musicbrainz_recording_id,musicbrainz_release_id,
                canonical_artist_id,canonical_release_id,quality_tier,content_hash,added_at
         FROM tracks
         WHERE play_count > 0
         ORDER BY play_count DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(rusqlite::params![effective_limit], |row| {
        Ok(Track {
            id: row.get(0)?,
            path: row.get(1)?,
            title: row.get(2)?,
            artist: row.get(3)?,
            album: row.get(4)?,
            album_artist: row.get(5)?,
            track_number: row.get(6)?,
            disc_number: row.get(7)?,
            year: row.get(8)?,
            duration_secs: row.get(9)?,
            sample_rate: row.get(10)?,
            bit_depth: row.get(11)?,
            bitrate_kbps: row.get(12)?,
            format: row.get(13)?,
            file_size: row.get(14)?,
            cover_art_path: row.get(15)?,
            isrc: row.get(16)?,
            musicbrainz_recording_id: row.get(17)?,
            musicbrainz_release_id: row.get(18)?,
            canonical_artist_id: row.get(19)?,
            canonical_release_id: row.get(20)?,
            quality_tier: row.get(21)?,
            content_hash: row.get(22)?,
            added_at: row.get(23).unwrap_or_default(),
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

/// Returns aggregate collection statistics.
pub fn get_collection_stats(&self) -> Result<CollectionStats> {
    let conn = self.conn.lock().unwrap();
    
    let total_tracks: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tracks", [], |r| r.get(0)
    ).unwrap_or(0);
    
    let total_albums: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT artist || '|' || album) FROM tracks", [], |r| r.get(0)
    ).unwrap_or(0);
    
    let total_duration_secs: f64 = conn.query_row(
        "SELECT COALESCE(SUM(duration_secs), 0.0) FROM tracks", [], |r| r.get(0)
    ).unwrap_or(0.0);

    // Format distribution
    let mut by_format = std::collections::HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT LOWER(format), COUNT(*) FROM tracks GROUP BY LOWER(format)"
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        for row in rows.flatten() {
            by_format.insert(row.0, row.1);
        }
    }

    // Decade distribution (from `year` column)
    let mut by_decade = std::collections::HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT (year / 10) * 10 as decade, COUNT(*) FROM tracks WHERE year IS NOT NULL AND year > 1900 GROUP BY decade"
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
        for row in rows.flatten() {
            let label = format!("{}s", row.0);
            by_decade.insert(label, row.1);
        }
    }

    // Quality tier distribution
    let mut by_quality_tier = std::collections::HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT COALESCE(quality_tier, 'unknown'), COUNT(*) FROM tracks GROUP BY quality_tier"
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        for row in rows.flatten() {
            by_quality_tier.insert(row.0, row.1);
        }
    }

    let lossless_count = by_format.get("flac").copied().unwrap_or(0)
        + by_format.get("wav").copied().unwrap_or(0)
        + by_format.get("aiff").copied().unwrap_or(0)
        + by_format.get("alac").copied().unwrap_or(0);

    let hires_count = by_quality_tier.get("lossless_hires").copied().unwrap_or(0);

    Ok(CollectionStats {
        total_tracks,
        total_albums,
        total_duration_secs,
        by_format,
        by_decade,
        by_quality_tier,
        lossless_count,
        hires_count,
    })
}
```

- [ ] **Step 5: Run tests to confirm PASS**

```bash
cd "c:/Cassette Music" && cargo test -p cassette-core test_get_most_played_tracks test_get_collection_stats 2>&1 | tail -10
```

Expected: both PASS.

- [ ] **Step 6: Add Tauri commands**

In `src-tauri/src/commands/library.rs`:

```rust
#[tauri::command]
pub fn get_most_played_tracks(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Vec<cassette_core::models::Track> {
    state
        .db
        .lock()
        .unwrap()
        .get_most_played_tracks(limit.unwrap_or(50))
        .unwrap_or_default()
}

#[tauri::command]
pub fn get_collection_stats(
    state: State<'_, AppState>,
) -> Result<cassette_core::db::CollectionStats, String> {
    state
        .db
        .lock()
        .unwrap()
        .get_collection_stats()
        .map_err(|e| e.to_string())
}
```

Note: if `CollectionStats` is defined in `db/mod.rs`, the path is `cassette_core::db::CollectionStats`. If you move it to `models.rs`, the path is `cassette_core::models::CollectionStats` — use whichever you chose in Step 3.

- [ ] **Step 7: Register in `src-tauri/src/lib.rs`**

Add `commands::library::get_most_played_tracks` and `commands::library::get_collection_stats` to the `.invoke_handler` list.

- [ ] **Step 8: Add TypeScript wrappers**

In `ui/src/lib/api/tauri.ts`, add the `CollectionStats` interface and the API methods:

```typescript
export interface CollectionStats {
  total_tracks: number;
  total_albums: number;
  total_duration_secs: number;
  by_format: Record<string, number>;
  by_decade: Record<string, number>;
  by_quality_tier: Record<string, number>;
  lossless_count: number;
  hires_count: number;
}

// In the api object:
async getMostPlayedTracks(limit?: number): Promise<Track[]> {
  return await invoke('get_most_played_tracks', { limit });
},
async getCollectionStats(): Promise<CollectionStats> {
  return await invoke('get_collection_stats');
},
```

- [ ] **Step 9: Build and test**

```bash
cd "c:/Cassette Music" && cargo test --workspace 2>&1 | tail -10
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -5
```

Expected: both pass.

- [ ] **Step 10: Commit**

```bash
cd "c:/Cassette Music" && git add crates/cassette-core/src/db/mod.rs src-tauri/src/commands/library.rs src-tauri/src/lib.rs ui/src/lib/api/tauri.ts && git commit -m "feat(backend): add get_most_played_tracks and get_collection_stats commands"
```

---

### Task 4: Play History page

**Files:**
- Create: `ui/src/routes/history/+page.svelte`
- Modify: `ui/src/lib/components/Sidebar.svelte` (add History link)

- [ ] **Step 1: Create the history page**

Create `ui/src/routes/history/+page.svelte`:

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type Track } from '$lib/api/tauri';
  import { queueTracks } from '$lib/stores/queue';
  import { formatDuration, coverSrc } from '$lib/utils';

  let mostPlayed: Track[] = [];
  let recentlyPlayed: Track[] = [];
  let loading = true;
  let activeTab: 'recent' | 'mostplayed' = 'recent';

  onMount(async () => {
    try {
      const [recent, top] = await Promise.all([
        api.getRecentlyFinalizedTracks(30),  // last 30 days as "recent"
        api.getMostPlayedTracks(50),
      ]);
      recentlyPlayed = recent;
      mostPlayed = top;
    } finally {
      loading = false;
    }
  });

  async function playTrack(list: Track[], index: number) {
    await queueTracks(list, index);
  }
</script>

<svelte:head><title>History · Cassette</title></svelte:head>

<div class="history-page">
  <div class="page-header">
    <h2 style="flex:1">Listening History</h2>
  </div>

  <div class="tabs">
    <button class="tab" class:active={activeTab === 'recent'} on:click={() => activeTab = 'recent'}>
      Recently Added
    </button>
    <button class="tab" class:active={activeTab === 'mostplayed'} on:click={() => activeTab = 'mostplayed'}>
      Most Played
    </button>
  </div>

  {#if loading}
    <div class="empty-state" style="padding:2rem;">
      <div class="empty-title">Loading…</div>
    </div>
  {:else if activeTab === 'recent'}
    {#if recentlyPlayed.length === 0}
      <div class="empty-state" style="padding:2rem;">
        <div class="empty-title">Nothing yet</div>
        <div class="empty-body">Tracks added to your library in the last 30 days will appear here.</div>
      </div>
    {:else}
      <div class="track-list">
        {#each recentlyPlayed as track, i}
          <div
            class="track-row"
            role="button"
            tabindex="0"
            on:dblclick={() => playTrack(recentlyPlayed, i)}
            on:keydown={(e) => { if (e.key === 'Enter') playTrack(recentlyPlayed, i); }}
          >
            {#if track.cover_art_path}
              <img class="track-art" src={coverSrc(track.cover_art_path)} alt="" />
            {:else}
              <div class="track-art-ph"></div>
            {/if}
            <div class="track-title">{track.title}</div>
            <div class="track-artist">{track.artist}{track.album ? ` · ${track.album}` : ''}</div>
            <span class="track-duration">{formatDuration(track.duration_secs)}</span>
            <span class="track-format">{track.format?.toUpperCase()}</span>
          </div>
        {/each}
      </div>
    {/if}
  {:else}
    {#if mostPlayed.length === 0}
      <div class="empty-state" style="padding:2rem;">
        <div class="empty-title">No play history yet</div>
        <div class="empty-body">Play some music and your most-listened tracks will appear here.</div>
      </div>
    {:else}
      <div class="track-list">
        {#each mostPlayed as track, i}
          <div
            class="track-row"
            role="button"
            tabindex="0"
            on:dblclick={() => playTrack(mostPlayed, i)}
            on:keydown={(e) => { if (e.key === 'Enter') playTrack(mostPlayed, i); }}
          >
            {#if track.cover_art_path}
              <img class="track-art" src={coverSrc(track.cover_art_path)} alt="" />
            {:else}
              <div class="track-art-ph"></div>
            {/if}
            <div class="track-title">{track.title}</div>
            <div class="track-artist">{track.artist}{track.album ? ` · ${track.album}` : ''}</div>
            <span class="track-duration">{formatDuration(track.duration_secs)}</span>
            <span class="track-format">{track.format?.toUpperCase()}</span>
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
.history-page { display: flex; flex-direction: column; min-height: 100%; }

.tabs {
  display: flex;
  padding: 0 1.5rem;
  border-bottom: 1px solid var(--border);
}

.tab {
  padding: 10px 16px;
  font-size: 0.85rem;
  font-weight: 500;
  color: var(--text-secondary);
  background: none;
  border: none;
  border-bottom: 2px solid transparent;
  margin-bottom: -1px;
  cursor: pointer;
  transition: color 0.15s;
}

.tab:hover { color: var(--text-primary); }
.tab.active { color: var(--accent-bright); border-bottom-color: var(--accent); }

.track-list { padding: 8px; }

.track-row {
  display: grid;
  grid-template-columns: 36px 1fr 1.4fr auto auto;
  align-items: center;
  gap: 10px;
  padding: 6px 10px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background 0.1s;
}

.track-row:hover { background: var(--bg-hover); }

.track-art, .track-art-ph {
  width: 36px; height: 36px;
  border-radius: 3px;
  object-fit: cover;
  flex-shrink: 0;
  background: var(--bg-active);
}

.track-title {
  font-size: 0.84rem;
  font-weight: 500;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.track-artist {
  font-size: 0.74rem;
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.track-duration, .track-format {
  font-size: 0.72rem;
  color: var(--text-muted);
  white-space: nowrap;
}
</style>
```

- [ ] **Step 2: Add History to sidebar nav**

In `Sidebar.svelte` (Task 1 of Phase 1 added `/session`), add a History link to `coreLinks` after Playlists:

```typescript
{
  href: '/history',
  label: 'History',
  icon: `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="1 4 1 10 7 10"/><path d="M3.51 15a9 9 0 1 0 .49-3.99"/></svg>`,
},
```

- [ ] **Step 3: Build and verify**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

Expected: build passes.

- [ ] **Step 4: Commit**

```bash
cd "c:/Cassette Music" && git add ui/src/routes/history/+page.svelte ui/src/lib/components/Sidebar.svelte && git commit -m "feat(ui): add listening history page with recently added and most played views"
```

---

### Task 5: Collection statistics page

**Files:**
- Create: `ui/src/routes/collection/+page.svelte`
- Modify: `ui/src/lib/components/Sidebar.svelte` (add Collection link)

- [ ] **Step 1: Create the collection page**

Create `ui/src/routes/collection/+page.svelte`:

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type CollectionStats } from '$lib/api/tauri';
  import { formatDuration } from '$lib/utils';

  let stats: CollectionStats | null = null;
  let loading = true;
  let error: string | null = null;

  onMount(async () => {
    try {
      stats = await api.getCollectionStats();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  });

  function formatTotalDuration(secs: number): string {
    const days = Math.floor(secs / 86400);
    const hrs = Math.floor((secs % 86400) / 3600);
    if (days > 0) return `${days}d ${hrs}h`;
    const mins = Math.floor((secs % 3600) / 60);
    return `${hrs}h ${mins}m`;
  }

  $: decadeEntries = stats
    ? Object.entries(stats.by_decade).sort(([a], [b]) => a.localeCompare(b))
    : [];

  $: formatEntries = stats
    ? Object.entries(stats.by_format)
        .sort(([, a], [, b]) => b - a)
        .slice(0, 8)
    : [];

  $: maxDecadeCount = decadeEntries.reduce((m, [, v]) => Math.max(m, v), 1);
</script>

<svelte:head><title>Collection · Cassette</title></svelte:head>

<div class="collection-page">
  <div class="page-header">
    <h2 style="flex:1">Your Collection</h2>
  </div>

  {#if loading}
    <div class="empty-state" style="padding:2rem;"><div class="empty-title">Loading…</div></div>
  {:else if error}
    <div class="empty-state" style="padding:2rem;"><div class="empty-title">Could not load stats</div><div class="empty-body">{error}</div></div>
  {:else if stats}
    <div class="stats-grid">
      <div class="stat-hero-card">
        <div class="stat-hero-value">{stats.total_tracks.toLocaleString()}</div>
        <div class="stat-hero-label">Tracks</div>
      </div>
      <div class="stat-hero-card">
        <div class="stat-hero-value">{stats.total_albums.toLocaleString()}</div>
        <div class="stat-hero-label">Albums</div>
      </div>
      <div class="stat-hero-card">
        <div class="stat-hero-value">{stats.lossless_count.toLocaleString()}</div>
        <div class="stat-hero-label">Lossless files</div>
      </div>
      <div class="stat-hero-card accent">
        <div class="stat-hero-value">{stats.hires_count.toLocaleString()}</div>
        <div class="stat-hero-label">Hi-Res files</div>
      </div>
      <div class="stat-hero-card span2">
        <div class="stat-hero-value">{formatTotalDuration(stats.total_duration_secs)}</div>
        <div class="stat-hero-label">Total listening time</div>
      </div>
    </div>

    <div class="section-block">
      <div class="section-kicker">By era</div>
      <h3>Decade distribution</h3>
      <div class="decade-chart">
        {#each decadeEntries as [decade, count]}
          <div class="decade-row">
            <span class="decade-label">{decade}</span>
            <div class="decade-bar-track">
              <div
                class="decade-bar-fill"
                style="width:{Math.round((count / maxDecadeCount) * 100)}%"
              ></div>
            </div>
            <span class="decade-count">{count.toLocaleString()}</span>
          </div>
        {/each}
      </div>
    </div>

    <div class="section-block">
      <div class="section-kicker">By format</div>
      <h3>Format breakdown</h3>
      <div class="format-list">
        {#each formatEntries as [fmt, count]}
          <div class="format-row">
            <span class="format-badge">{fmt.toUpperCase()}</span>
            <span class="format-count">{count.toLocaleString()} tracks</span>
            <div class="format-bar-track">
              <div
                class="format-bar-fill"
                style="width:{Math.round((count / stats.total_tracks) * 100)}%"
              ></div>
            </div>
            <span class="format-pct">{Math.round((count / stats.total_tracks) * 100)}%</span>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
.collection-page { display: flex; flex-direction: column; min-height: 100%; padding: 0; }

.stats-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 12px;
  padding: 1.25rem 1.5rem 0.75rem;
}

.stat-hero-card {
  padding: 16px 18px;
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: var(--bg-card);
}

.stat-hero-card.accent {
  border-color: color-mix(in srgb, var(--primary) 30%, var(--border));
  background: color-mix(in srgb, var(--primary) 6%, var(--bg-card));
}

.stat-hero-card.span2 {
  grid-column: span 2;
}

.stat-hero-value {
  font-size: 1.8rem;
  font-weight: 800;
  color: var(--text-primary);
  line-height: 1;
  margin-bottom: 5px;
}

.stat-hero-label {
  font-size: 0.72rem;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.07em;
}

.section-block {
  padding: 1rem 1.5rem;
  border-top: 1px solid var(--border);
}

.section-block h3 { margin: 4px 0 14px; font-size: 1rem; }

.section-kicker {
  font-size: 0.66rem;
  text-transform: uppercase;
  letter-spacing: 0.12em;
  color: var(--accent);
  font-weight: 700;
}

.decade-chart { display: flex; flex-direction: column; gap: 8px; max-width: 540px; }

.decade-row {
  display: grid;
  grid-template-columns: 52px 1fr 60px;
  align-items: center;
  gap: 10px;
}

.decade-label { font-size: 0.74rem; color: var(--text-secondary); text-align: right; }

.decade-bar-track {
  height: 6px;
  background: var(--bg-active);
  border-radius: 99px;
  overflow: hidden;
}

.decade-bar-fill {
  height: 100%;
  background: var(--primary);
  border-radius: 99px;
  transition: width 0.3s ease;
}

.decade-count { font-size: 0.72rem; color: var(--text-muted); text-align: right; }

.format-list { display: flex; flex-direction: column; gap: 8px; max-width: 540px; }

.format-row {
  display: grid;
  grid-template-columns: 52px 110px 1fr 40px;
  align-items: center;
  gap: 10px;
}

.format-badge {
  font-size: 0.68rem;
  font-weight: 700;
  color: var(--primary);
  background: rgba(139,180,212,0.1);
  border: 1px solid rgba(139,180,212,0.2);
  border-radius: 4px;
  padding: 2px 5px;
  text-align: center;
}

.format-count { font-size: 0.74rem; color: var(--text-secondary); }

.format-bar-track {
  height: 4px;
  background: var(--bg-active);
  border-radius: 99px;
  overflow: hidden;
}

.format-bar-fill {
  height: 100%;
  background: var(--accent);
  border-radius: 99px;
}

.format-pct { font-size: 0.68rem; color: var(--text-muted); text-align: right; }

@media (max-width: 900px) {
  .stats-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .stat-hero-card.span2 { grid-column: span 2; }
}
</style>
```

- [ ] **Step 2: Add Collection to sidebar nav**

In `Sidebar.svelte`, add to `coreLinks` after History:

```typescript
{
  href: '/collection',
  label: 'Collection',
  icon: `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="6" height="6"/><rect x="9" y="3" width="6" height="6"/><rect x="16" y="3" width="6" height="6"/><rect x="2" y="10" width="6" height="6"/><rect x="9" y="10" width="6" height="6"/><rect x="16" y="10" width="6" height="6"/><rect x="2" y="17" width="6" height="6"/><rect x="9" y="17" width="6" height="6"/><rect x="16" y="17" width="6" height="6"/></svg>`,
},
```

- [ ] **Step 3: Build and verify**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

Expected: build passes.

- [ ] **Step 4: Commit**

```bash
cd "c:/Cassette Music" && git add ui/src/routes/collection/+page.svelte ui/src/lib/components/Sidebar.svelte && git commit -m "feat(ui): add collection statistics page with decade/format/quality distribution"
```

---

### Task 6: Expanded Now Playing overlay

**Files:**
- Create: `ui/src/lib/components/NowPlayingExpanded.svelte`
- Modify: `ui/src/lib/components/NowPlaying.svelte`
- Modify: `ui/src/routes/+layout.svelte`

Clicking the album art in the NowPlaying bar opens a full-screen overlay with large artwork, track info, lyrics if available, and the queue preview. Pressing Escape or clicking the backdrop closes it.

- [ ] **Step 1: Create `NowPlayingExpanded.svelte`**

Create `ui/src/lib/components/NowPlayingExpanded.svelte`:

```svelte
<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { playbackState, isPlaying, progressPct, isSeeking, seekPreview, player, nowPlayingContext } from '$lib/stores/player';
  import { queue } from '$lib/stores/queue';
  import { formatDuration, coverSrc } from '$lib/utils';

  const dispatch = createEventDispatcher<{ close: void }>();

  function close() { dispatch('close'); }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') close();
  }

  $: track = $playbackState.current_track;
  $: ctx = $nowPlayingContext;
  $: pos = $playbackState.position_secs;
  $: dur = $playbackState.duration_secs;
  $: pct = $progressPct;

  let seekBarEl: HTMLDivElement;
  function clamp(v: number, min: number, max: number) { return Math.max(min, Math.min(max, v)); }

  function onSeekMouseDown(e: MouseEvent) {
    isSeeking.set(true);
    const rect = seekBarEl.getBoundingClientRect();
    seekPreview.set(clamp((e.clientX - rect.left) / rect.width, 0, 1));
    const onMove = (ev: MouseEvent) => seekPreview.set(clamp((ev.clientX - rect.left) / rect.width, 0, 1));
    const onUp = async (ev: MouseEvent) => {
      await player.seek(clamp((ev.clientX - rect.left) / rect.width, 0, 1));
      isSeeking.set(false);
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    };
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  }
</script>

<svelte:window on:keydown={onKeydown} />

<!-- svelte-ignore a11y-no-static-element-interactions -->
<div class="np-expanded-backdrop" on:click={close}>
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="np-expanded-panel" on:click|stopPropagation>
    <button class="np-close-btn" on:click={close} aria-label="Close">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
      </svg>
    </button>

    <div class="np-expanded-body">
      <!-- Art column -->
      <div class="np-exp-art-col">
        {#if track?.cover_art_path}
          <img class="np-exp-art" src={coverSrc(track.cover_art_path)} alt="cover" />
        {:else}
          <div class="np-exp-art-ph"></div>
        {/if}
      </div>

      <!-- Info column -->
      <div class="np-exp-info-col">
        <div class="np-exp-title">{track?.title ?? '—'}</div>
        <div class="np-exp-artist">{track?.artist ?? 'No track playing'}</div>
        {#if track?.album}
          <div class="np-exp-album">{track.album}{track.year ? ` · ${track.year}` : ''}</div>
        {/if}

        {#if ctx?.artist_tags?.length}
          <div class="np-exp-tags">
            {#each ctx.artist_tags.slice(0, 4) as tag}
              <span class="np-tag">{tag}</span>
            {/each}
          </div>
        {/if}

        <!-- Controls -->
        <div class="np-exp-controls">
          <button class="exp-ctrl" on:click={() => player.prev()}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><polygon points="19 20 9 12 19 4 19 20"/><line x1="5" y1="19" x2="5" y2="5" stroke="currentColor" stroke-width="2" fill="none"/></svg>
          </button>
          <button class="exp-ctrl exp-play" on:click={() => player.toggle()}>
            {#if $isPlaying}
              <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/></svg>
            {:else}
              <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3"/></svg>
            {/if}
          </button>
          <button class="exp-ctrl" on:click={() => player.next()}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><polygon points="5 4 15 12 5 20 5 4"/><line x1="19" y1="5" x2="19" y2="19" stroke="currentColor" stroke-width="2" fill="none"/></svg>
          </button>
        </div>

        <!-- Seek bar -->
        <div class="np-exp-seek-wrap">
          <span class="np-exp-time">{formatDuration(pos)}</span>
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div class="np-exp-seek" bind:this={seekBarEl} on:mousedown={onSeekMouseDown}>
            <div class="np-exp-seek-fill" style="width:{pct*100}%"></div>
          </div>
          <span class="np-exp-time">{formatDuration(dur)}</span>
        </div>

        <!-- Lyrics snippet -->
        {#if ctx?.lyrics}
          <div class="np-exp-lyrics">
            <div class="np-exp-lyrics-label">Lyrics</div>
            <div class="np-exp-lyrics-text">{ctx.lyrics.slice(0, 400)}{ctx.lyrics.length > 400 ? '…' : ''}</div>
          </div>
        {/if}
      </div>

      <!-- Queue preview -->
      <div class="np-exp-queue-col">
        <div class="np-exp-queue-label">Up Next</div>
        {#each $queue.slice($playbackState.queue_position + 1, $playbackState.queue_position + 7) as item}
          <div class="np-exp-queue-item">
            <span class="np-exp-qi-title">{item.track?.title ?? 'Unknown'}</span>
            <span class="np-exp-qi-artist">{item.track?.artist ?? ''}</span>
          </div>
        {/each}
        {#if $queue.length === 0}
          <div class="np-exp-queue-empty">Queue is empty</div>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
.np-expanded-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1000;
  background: rgba(6, 8, 16, 0.88);
  backdrop-filter: blur(16px);
  display: flex;
  align-items: center;
  justify-content: center;
}

.np-expanded-panel {
  position: relative;
  width: min(92vw, 1060px);
  max-height: 82vh;
  background: var(--bg-card);
  border: 1px solid var(--border-active);
  border-radius: var(--radius-lg);
  overflow: hidden;
  box-shadow: 0 32px 80px rgba(0,0,0,0.7);
}

.np-close-btn {
  position: absolute;
  top: 14px; right: 14px;
  z-index: 2;
  width: 28px; height: 28px;
  border-radius: 50%;
  background: rgba(255,255,255,0.06);
  border: 1px solid var(--border);
  color: var(--text-muted);
  display: flex; align-items: center; justify-content: center;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}
.np-close-btn:hover { background: rgba(255,255,255,0.12); color: var(--text-primary); }

.np-expanded-body {
  display: grid;
  grid-template-columns: 280px 1fr 220px;
  gap: 0;
  height: 100%;
}

.np-exp-art-col {
  padding: 28px 20px 28px 28px;
}

.np-exp-art, .np-exp-art-ph {
  width: 100%;
  aspect-ratio: 1;
  border-radius: var(--radius);
  object-fit: cover;
  box-shadow: 0 12px 40px rgba(0,0,0,0.6);
  background: var(--bg-active);
}

.np-exp-info-col {
  padding: 28px 20px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  overflow-y: auto;
}

.np-exp-title {
  font-size: 1.5rem;
  font-weight: 800;
  color: var(--text-primary);
  line-height: 1.1;
}

.np-exp-artist {
  font-size: 1rem;
  color: var(--text-secondary);
}

.np-exp-album {
  font-size: 0.82rem;
  color: var(--text-muted);
}

.np-exp-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 4px;
}

.np-tag {
  font-size: 0.68rem;
  color: var(--primary);
  background: rgba(139,180,212,0.1);
  border: 1px solid rgba(139,180,212,0.2);
  border-radius: 999px;
  padding: 2px 8px;
}

.np-exp-controls {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 8px;
}

.exp-ctrl {
  display: flex; align-items: center; justify-content: center;
  width: 36px; height: 36px;
  border-radius: 50%;
  color: var(--text-secondary);
  background: none; border: none; cursor: pointer;
  transition: background 0.15s, color 0.15s;
}
.exp-ctrl:hover { background: var(--bg-hover); color: var(--text-primary); }

.exp-play {
  width: 42px; height: 42px;
  background: var(--primary) !important;
  color: var(--bg-deep) !important;
  box-shadow: 0 2px 12px rgba(139,180,212,0.3);
}
.exp-play:hover { background: #a0c8e8 !important; }

.np-exp-seek-wrap {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 4px;
}

.np-exp-time {
  font-size: 0.72rem;
  color: var(--text-muted);
  white-space: nowrap;
  min-width: 36px;
}

.np-exp-seek {
  flex: 1;
  height: 4px;
  background: var(--bg-active);
  border-radius: 99px;
  cursor: pointer;
  position: relative;
}

.np-exp-seek-fill {
  position: absolute;
  left: 0; top: 0; bottom: 0;
  background: var(--primary);
  border-radius: 99px;
  pointer-events: none;
}

.np-exp-lyrics {
  margin-top: 8px;
  padding: 12px;
  background: var(--bg-base);
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
}

.np-exp-lyrics-label {
  font-size: 0.62rem;
  text-transform: uppercase;
  letter-spacing: 0.1em;
  color: var(--text-muted);
  margin-bottom: 8px;
}

.np-exp-lyrics-text {
  font-size: 0.82rem;
  line-height: 1.7;
  color: var(--text-secondary);
  white-space: pre-line;
}

.np-exp-queue-col {
  border-left: 1px solid var(--border);
  padding: 20px 14px;
  overflow-y: auto;
}

.np-exp-queue-label {
  font-size: 0.62rem;
  text-transform: uppercase;
  letter-spacing: 0.1em;
  color: var(--text-muted);
  margin-bottom: 10px;
}

.np-exp-queue-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 7px 8px;
  border-radius: var(--radius-sm);
  transition: background 0.1s;
  cursor: pointer;
}

.np-exp-queue-item:hover { background: var(--bg-hover); }

.np-exp-qi-title {
  font-size: 0.78rem;
  font-weight: 500;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.np-exp-qi-artist {
  font-size: 0.68rem;
  color: var(--text-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.np-exp-queue-empty {
  font-size: 0.76rem;
  color: var(--text-muted);
  padding: 8px;
}
</style>
```

- [ ] **Step 2: Add expanded state trigger to NowPlaying**

In `NowPlaying.svelte`, add:

```svelte
<script lang="ts">
  // ... existing imports and code ...
  let expanded = false;
  function openExpanded() { expanded = true; }
  function closeExpanded() { expanded = false; }
</script>

<!-- Add import in the template if NowPlayingExpanded is used inline: -->
{#if expanded}
  {#await import('$lib/components/NowPlayingExpanded.svelte') then { default: Expanded }}
    <svelte:component this={Expanded} on:close={closeExpanded} />
  {/await}
{/if}
```

In the `.np-art` div, make it clickable:

```svelte
<div class="np-art" role="button" tabindex="0" on:click={openExpanded} style="cursor:pointer;" title="Expand now playing">
  {#if track?.cover_art_path}
    <img src={coverSrc(track.cover_art_path)} alt="cover" />
  {:else}
    <div class="np-art-ph">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" opacity="0.4">
        <path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>
      </svg>
    </div>
  {/if}
</div>
```

- [ ] **Step 3: Build and verify**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

Expected: build passes.

- [ ] **Step 4: Commit**

```bash
cd "c:/Cassette Music" && git add ui/src/lib/components/NowPlayingExpanded.svelte ui/src/lib/components/NowPlaying.svelte && git commit -m "feat(ui): add expanded now playing overlay with art, lyrics, and queue preview"
```

---

### Task 7: Full verification pass

- [ ] **Step 1: Full cargo check**

```bash
cd "c:/Cassette Music" && cargo check --workspace 2>&1 | tail -5
```

Expected: `Finished` — no errors.

- [ ] **Step 2: Full workspace tests**

```bash
cd "c:/Cassette Music" && cargo test --workspace 2>&1 | tail -15
```

Expected: all tests pass.

- [ ] **Step 3: Full UI build**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

Expected: build succeeds.

- [ ] **Step 4: Smoke test**

```powershell
cd "c:/Cassette Music" && .\scripts\smoke_desktop.ps1 2>&1 | tail -20
```

Expected: smoke passes.

- [ ] **Step 5: Verify trust spine**

```powershell
cd "c:/Cassette Music" && .\scripts\verify_trust_spine.ps1 2>&1 | tail -20
```

Expected: trust spine passes.

- [ ] **Step 6: Final commit**

```bash
cd "c:/Cassette Music" && git status
```

If anything adjusted:
```bash
git add -p && git commit -m "fix: phase-3 verification adjustments"
```

---

## Self-Review

**Spec coverage:**
- [x] Remove `downloader` legacy module → Task 1
- [x] Player event emission → Task 2
- [x] Play history + most played DB + Tauri commands → Task 3
- [x] Play history page → Task 4
- [x] Collection statistics page → Task 5
- [x] Expanded Now Playing overlay → Task 6
- [x] Full verification pass → Task 7

**Placeholder scan:** No TBDs. All code shown inline. No "implement later" language.

**Type consistency:**
- `CollectionStats` defined in Task 3 Step 3/8 matches its use in Task 5
- `emit_playback_state` uses `PlaybackState` from existing `cassette_core::models`
- `startPlayerEventListener` listens for `'playback_state_changed'` event — matches emission in Task 2
- `get_most_played_tracks` returns `Vec<Track>` using the same Track row mapping pattern as `get_recently_finalized_tracks` from Phase 2

**Notes for the implementing agent:**
- The `CollectionStats` struct needs `serde::Serialize` and `serde::Deserialize` derives — make sure `serde` with the `derive` feature is in `cassette-core/Cargo.toml` (it should already be, check before assuming)
- If `db/mod.rs` does not already expose `CollectionStats` publicly, add `pub use crate::db::CollectionStats;` to `lib.rs` so the Tauri command can reference it cleanly
- The `NowPlayingExpanded` component uses `player.prev()`, `player.next()`, `player.toggle()`, `player.seek()` from the existing player store — verify these method names match `ui/src/lib/stores/player.ts` exactly before implementing
