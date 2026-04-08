# Phase 2: Music Surfaces Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Elevate the listening experience by promoting the Session Composer to a first-class route, wiring global search through the CommandPalette, adding a "New Arrivals" section, making playlists addable from context, and building the expanded Now Playing panel — turning Cassette from a capable tool into a place people want to return to.

**Architecture:** Light Rust additions (1 new Tauri command for recently-finalized tracks, 1 for add-to-playlist), primarily SvelteKit surface work. New `/session` route. CommandPalette extended to handle music search. These changes require Phase 1 to be complete first.

**Tech Stack:** SvelteKit, Svelte 5, TypeScript, Tauri 2 commands (Rust), existing `cassette-core` DB layer.

**Prerequisite:** Phase 1 complete (`2026-04-07-phase1-texture-and-polish.md`).

---

## File Map

| File | Change |
|------|--------|
| `ui/src/routes/session/+page.svelte` | New — Session Composer as full page |
| `ui/src/lib/components/SessionComposer.svelte` | Remove (extracted to page) |
| `ui/src/routes/+page.svelte` | Remove inline SessionComposer import, add New Arrivals section |
| `ui/src/lib/components/CommandPalette.svelte` | Extend to handle music search results with play/queue actions |
| `ui/src/lib/stores/commands.ts` | Add music search result handling |
| `ui/src/routes/playlists/+page.svelte` | Show "Add to playlist" from library context |
| `ui/src/lib/components/ContextActionRail.svelte` | Add "Add to playlist" action |
| `ui/src/lib/api/tauri.ts` | Add `getRecentlyFinalizedTracks`, `addTrackToPlaylist` |
| `src-tauri/src/commands/library.rs` | Add `get_recently_finalized_tracks` command |
| `src-tauri/src/commands/playlists.rs` | Add `add_track_to_playlist` command |
| `src-tauri/src/lib.rs` | Register new commands |
| `crates/cassette-core/src/db/mod.rs` | Add `get_recently_finalized_tracks` DB method |

---

### Task 1: Add `get_recently_finalized_tracks` to the DB and Tauri command surface

**Files:**
- Modify: `crates/cassette-core/src/db/mod.rs`
- Modify: `src-tauri/src/commands/library.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `ui/src/lib/api/tauri.ts`

"Recently finalized" = tracks with `added_at` within the last N days, ordered newest first. The `tracks` table has `added_at TEXT NOT NULL DEFAULT (datetime('now'))`.

- [ ] **Step 1: Write the failing test for the DB method**

Open `crates/cassette-core/src/db/mod.rs`. Find the test module at the bottom (search for `#[cfg(test)]`). Add this test:

```rust
#[test]
fn test_get_recently_finalized_tracks() {
    let db = CassetteDb::open_in_memory().expect("in-memory db");
    db.run_migrations().expect("migrations");
    
    // Insert a track with added_at = now
    db.conn.lock().unwrap().execute(
        "INSERT INTO tracks (path, title, artist, album, album_artist, duration_secs, file_size, format, added_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))",
        rusqlite::params!["/test/track.flac", "Test Track", "Test Artist", "Test Album", "Test Artist", 180.0, 1024000, "flac"],
    ).expect("insert");
    
    let recent = db.get_recently_finalized_tracks(7).expect("query");
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].title, "Test Track");
}
```

- [ ] **Step 2: Run the test to confirm it fails**

```bash
cd "c:/Cassette Music" && cargo test -p cassette-core test_get_recently_finalized_tracks 2>&1 | tail -15
```

Expected: FAIL — method does not exist yet.

- [ ] **Step 3: Add the DB method**

In `crates/cassette-core/src/db/mod.rs`, find `pub fn get_tracks` (around line 852) and add the new method directly after it:

```rust
/// Returns tracks added within the last `days` days, newest first.
/// Limit is capped at 50 to keep the result set bounded.
pub fn get_recently_finalized_tracks(&self, days: u32) -> Result<Vec<Track>> {
    let conn = self.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id,path,title,artist,album,album_artist,track_number,disc_number,
                year,duration_secs,sample_rate,bit_depth,bitrate_kbps,format,file_size,
                cover_art_path,isrc,musicbrainz_recording_id,musicbrainz_release_id,
                canonical_artist_id,canonical_release_id,quality_tier,content_hash,added_at
         FROM tracks
         WHERE datetime(added_at) >= datetime('now', '-' || ?1 || ' days')
         ORDER BY datetime(added_at) DESC
         LIMIT 50",
    )?;
    let rows = stmt.query_map(rusqlite::params![days], |row| {
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
```

Note: The `Track` row mapping must match the exact column projection used in the existing `get_tracks` method — verify the column order is identical.

- [ ] **Step 4: Run the test to confirm it passes**

```bash
cd "c:/Cassette Music" && cargo test -p cassette-core test_get_recently_finalized_tracks 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 5: Add the Tauri command**

In `src-tauri/src/commands/library.rs`, add at the end of the file:

```rust
#[tauri::command]
pub fn get_recently_finalized_tracks(
    state: State<'_, AppState>,
    days: Option<u32>,
) -> Vec<cassette_core::models::Track> {
    state
        .db
        .lock()
        .unwrap()
        .get_recently_finalized_tracks(days.unwrap_or(7))
        .unwrap_or_default()
}
```

- [ ] **Step 6: Register the command in `src-tauri/src/lib.rs`**

Find the `.invoke_handler(tauri::generate_handler![` call in `lib.rs`. Add `commands::library::get_recently_finalized_tracks` to the list (it will be on a new line following the existing library commands).

- [ ] **Step 7: Add the TypeScript API wrapper**

In `ui/src/lib/api/tauri.ts`, add to the `api` object:

```typescript
async getRecentlyFinalizedTracks(days?: number): Promise<Track[]> {
  return await invoke('get_recently_finalized_tracks', { days });
},
```

- [ ] **Step 8: Build and run tests**

```bash
cd "c:/Cassette Music" && cargo check --workspace 2>&1 | tail -5
cd "c:/Cassette Music" && cargo test --workspace 2>&1 | tail -10
```

Expected: both pass.

- [ ] **Step 9: Commit**

```bash
cd "c:/Cassette Music" && git add crates/cassette-core/src/db/mod.rs src-tauri/src/commands/library.rs src-tauri/src/lib.rs ui/src/lib/api/tauri.ts && git commit -m "feat(backend): add get_recently_finalized_tracks command for new arrivals surface"
```

---

### Task 2: Add `add_track_to_playlist` command

**Files:**
- Modify: `crates/cassette-core/src/db/mod.rs`
- Modify: `src-tauri/src/commands/playlists.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `ui/src/lib/api/tauri.ts`

Currently playlists only support `replace_playlist_tracks` (full replacement). We need append-single-track.

- [ ] **Step 1: Write the failing test**

In the `#[cfg(test)]` module of `db/mod.rs`:

```rust
#[test]
fn test_add_track_to_playlist() {
    let db = CassetteDb::open_in_memory().expect("in-memory db");
    db.run_migrations().expect("migrations");

    // Create a track
    db.conn.lock().unwrap().execute(
        "INSERT INTO tracks (path, title, artist, album, album_artist, duration_secs, file_size, format)
         VALUES ('/t.flac','T','A','B','A', 180.0, 1024, 'flac')",
        [],
    ).expect("insert track");
    let track_id: i64 = db.conn.lock().unwrap()
        .query_row("SELECT id FROM tracks WHERE path='/t.flac'", [], |r| r.get(0))
        .expect("track id");

    // Create a playlist
    let pl_id = db.create_playlist("Test", None, &[]).expect("create playlist");

    // Add the track
    db.add_track_to_playlist(pl_id, track_id).expect("add track");

    let items = db.get_playlist_items(pl_id).expect("items");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].track_id, track_id);
}
```

- [ ] **Step 2: Run to confirm FAIL**

```bash
cd "c:/Cassette Music" && cargo test -p cassette-core test_add_track_to_playlist 2>&1 | tail -10
```

Expected: FAIL — method does not exist.

- [ ] **Step 3: Add the DB method**

In `db/mod.rs`, after `replace_playlist_tracks`:

```rust
/// Appends a single track to a playlist at the next available position.
pub fn add_track_to_playlist(&self, playlist_id: i64, track_id: i64) -> Result<()> {
    let conn = self.conn.lock().unwrap();
    let next_pos: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_items WHERE playlist_id = ?1",
        rusqlite::params![playlist_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO playlist_items (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
        rusqlite::params![playlist_id, track_id, next_pos],
    )?;
    Ok(())
}
```

- [ ] **Step 4: Run test to confirm PASS**

```bash
cd "c:/Cassette Music" && cargo test -p cassette-core test_add_track_to_playlist 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 5: Add the Tauri command**

In `src-tauri/src/commands/playlists.rs`:

```rust
#[tauri::command]
pub fn add_track_to_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    track_id: i64,
) -> Result<(), String> {
    state
        .db
        .lock()
        .unwrap()
        .add_track_to_playlist(playlist_id, track_id)
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 6: Register in `src-tauri/src/lib.rs`**

Add `commands::playlists::add_track_to_playlist` to the `.invoke_handler` list.

- [ ] **Step 7: Add TypeScript wrapper**

In `ui/src/lib/api/tauri.ts`:

```typescript
async addTrackToPlaylist(playlistId: number, trackId: number): Promise<void> {
  await invoke('add_track_to_playlist', { playlistId, trackId });
},
```

- [ ] **Step 8: Build and test**

```bash
cd "c:/Cassette Music" && cargo test --workspace 2>&1 | tail -10
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -5
```

Expected: both pass.

- [ ] **Step 9: Commit**

```bash
cd "c:/Cassette Music" && git add crates/cassette-core/src/db/mod.rs src-tauri/src/commands/playlists.rs src-tauri/src/lib.rs ui/src/lib/api/tauri.ts && git commit -m "feat(backend): add add_track_to_playlist command for context rail integration"
```

---

### Task 3: Add "Add to Playlist" action to ContextActionRail

**Files:**
- Modify: `ui/src/lib/components/ContextActionRail.svelte`
- Modify: `ui/src/lib/stores/playlists.ts` (ensure playlists are loadable from rail)

- [ ] **Step 1: Add playlist imports and state to the rail**

In `ContextActionRail.svelte`, update the script block:

```svelte
<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { addToQueue, loadQueue, queueTracks } from '$lib/stores/queue';
  import { api, type Track, type Playlist } from '$lib/api/tauri';

  export let track: Track | null = null;
  export let album: { artist: string; title: string } | null = null;
  export let artistName: string | null = null;
  export let compact = false;

  const dispatch = createEventDispatcher<{ status: string }>();

  let busy = false;
  let message = '';
  let showPlaylistPicker = false;
  let playlists: Playlist[] = [];
  let loadingPlaylists = false;

  // ... keep all existing functions (playTrackNow, queueTrackNext, acquireTrack, etc.)
```

- [ ] **Step 2: Add `showAddToPlaylist` and `addToPlaylist` functions**

Add after the existing functions, before `$: hasContext`:

```typescript
async function showAddToPlaylist() {
  showPlaylistPicker = !showPlaylistPicker;
  if (showPlaylistPicker && playlists.length === 0) {
    loadingPlaylists = true;
    try {
      playlists = await api.getPlaylists();
    } finally {
      loadingPlaylists = false;
    }
  }
}

async function addToPlaylist(playlistId: number) {
  if (!track) return;
  await withBusy(async () => {
    await api.addTrackToPlaylist(playlistId, track!.id);
    setStatus('Added to playlist');
    showPlaylistPicker = false;
  });
}
```

- [ ] **Step 3: Add the "Add to Playlist" button and picker to the rail template**

In the `{#if track}` block inside `.rail-actions`, add after the existing track buttons:

```svelte
{#if track}
  <button class="rail-btn" disabled={busy} on:click={playTrackNow}>Play Track</button>
  <button class="rail-btn" disabled={busy} on:click={queueTrackNext}>Queue Track</button>
  <button class="rail-btn" disabled={busy} on:click={showAddToPlaylist}>+ Playlist</button>
  <button class="rail-btn rail-btn-acquire" disabled={busy} on:click={acquireTrack}>Acquire Track</button>
{/if}
```

After the `.rail-actions` div, add the picker:

```svelte
{#if showPlaylistPicker && track}
  <div class="playlist-picker">
    {#if loadingPlaylists}
      <div class="picker-loading">Loading playlists...</div>
    {:else if playlists.length === 0}
      <div class="picker-empty">No playlists yet — create one in Playlists.</div>
    {:else}
      {#each playlists as pl}
        <button class="picker-item" on:click={() => addToPlaylist(pl.id)}>
          {pl.name} <span class="picker-count">{pl.track_count} tracks</span>
        </button>
      {/each}
    {/if}
  </div>
{/if}
```

- [ ] **Step 4: Add picker styles**

Add to the `<style>` block:

```css
.playlist-picker {
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg-deep);
  overflow: hidden;
  margin-top: 2px;
}
.picker-loading, .picker-empty {
  padding: 8px 10px;
  font-size: 0.72rem;
  color: var(--text-muted);
}
.picker-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  padding: 7px 10px;
  font-size: 0.78rem;
  color: var(--text-secondary);
  background: none;
  border: none;
  border-top: 1px solid var(--border-dim);
  cursor: pointer;
  text-align: left;
  transition: background 0.1s, color 0.1s;
}
.picker-item:first-child { border-top: none; }
.picker-item:hover { background: var(--bg-hover); color: var(--text-primary); }
.picker-count { font-size: 0.68rem; color: var(--text-muted); }
```

- [ ] **Step 5: Build and verify**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

Expected: build passes.

- [ ] **Step 6: Commit**

```bash
cd "c:/Cassette Music" && git add ui/src/lib/components/ContextActionRail.svelte && git commit -m "feat(ui): add 'Add to Playlist' picker to ContextActionRail for tracks"
```

---

### Task 4: Promote Session Composer to `/session` route

**Files:**
- Create: `ui/src/routes/session/+page.svelte`
- Modify: `ui/src/routes/+page.svelte` (remove inline import)
- Modify: `ui/src/lib/components/Sidebar.svelte` (already has `/session` in Task 1 of Phase 1)

The Session Composer component already exists and works. This task extracts it to its own full page.

- [ ] **Step 1: Create the session page file**

Create `ui/src/routes/session/+page.svelte`:

```svelte
<script lang="ts">
  import SessionComposer from '$lib/components/SessionComposer.svelte';
</script>

<svelte:head><title>Session · Cassette</title></svelte:head>

<div class="session-page">
  <div class="page-header">
    <h2>Session Composer</h2>
    <p class="page-desc">Design explainable listening arcs with energy slope, era bias, and feedback learning.</p>
  </div>
  <div class="session-content">
    <SessionComposer />
  </div>
</div>

<style>
.session-page {
  display: flex;
  flex-direction: column;
  min-height: 100%;
  padding: 0;
}

.page-header {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 1.25rem 1.5rem 0.75rem;
  border-bottom: 1px solid var(--border);
}

.page-desc {
  font-size: 0.82rem;
  color: var(--text-secondary);
  line-height: 1.5;
}

.session-content {
  padding: 1.25rem 1.5rem;
  flex: 1;
  overflow-y: auto;
}
</style>
```

- [ ] **Step 2: Remove the inline SessionComposer from Home**

In `ui/src/routes/+page.svelte`, remove:
1. The `let SessionComposer` variable declaration and its lazy import in `onMount`
2. The `{#if SessionComposer}<svelte:component this={SessionComposer} />{/if}` block at the bottom of the template

Also remove from the `onMount` async block:
```typescript
const module = await import('$lib/components/SessionComposer.svelte');
SessionComposer = module.default;
```

And remove the `let SessionComposer` variable declaration entirely.

- [ ] **Step 3: Add a "Go to Session Composer" link on Home**

In `ui/src/routes/+page.svelte`, in the home hero `hero-actions` div, add a third action button:

```svelte
<div class="hero-actions">
  <button class="btn btn-primary" on:click={() => goto('/artists')}>Open artists</button>
  <button class="btn btn-ghost" on:click={() => goto('/session')}>Session composer</button>
  <button class="btn btn-ghost" on:click={() => goto('/downloads')}>Downloads</button>
  {#if current}
    <button class="btn btn-ghost" on:click={resumePlayback}>
      {$playbackState.is_playing ? 'Pause' : 'Resume'}
    </button>
  {/if}
</div>
```

- [ ] **Step 4: Build and verify**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

Expected: build passes.

- [ ] **Step 5: Commit**

```bash
cd "c:/Cassette Music" && git add ui/src/routes/session/+page.svelte ui/src/routes/+page.svelte && git commit -m "feat(ui): promote Session Composer to dedicated /session route"
```

---

### Task 5: Add "New Arrivals" section to Home

**Files:**
- Modify: `ui/src/routes/+page.svelte`

Uses the `getRecentlyFinalizedTracks` command from Task 1. Shows the last 8 tracks added to the library in the last 7 days.

- [ ] **Step 1: Add recentTracks state and load call**

In the Home page script, add:

```typescript
let recentTracks: import('$lib/api/tauri').Track[] = [];
```

In the `onMount` block, add `recentTracks` to the parallel fetch:

```typescript
const [missing, results, requests, trust, recent] = await Promise.all([
  api.getMissingSpotifyAlbums(10),
  api.getRecentTaskResults(12),
  api.listAcquisitionRequests(undefined, 32),
  api.getTrustReasonDistribution(6),
  api.getRecentlyFinalizedTracks(7),
]);
missingAlbums = missing;
recentResults = results;
recentRequests = requests;
trustDistribution = trust;
recentTracks = recent;
```

- [ ] **Step 2: Add the New Arrivals section to the template**

Add after the `<section class="home-band">` (while-you-were-away section) and before `<section class="home-columns">`:

```svelte
{#if recentTracks.length > 0}
  <section class="home-band">
    <div class="band-heading">
      <div>
        <div class="section-kicker">New in your library</div>
        <h2>Recently arrived</h2>
      </div>
      <button class="band-link" on:click={() => goto('/library')}>Browse library</button>
    </div>
    <div class="arrivals-grid">
      {#each recentTracks.slice(0, 8) as track}
        <div class="arrival-card">
          {#if track.cover_art_path}
            <img class="arrival-art" src={coverSrc(track.cover_art_path)} alt="" loading="lazy" />
          {:else}
            <div class="arrival-art-ph"></div>
          {/if}
          <div class="arrival-info">
            <div class="arrival-title">{track.title}</div>
            <div class="arrival-meta">{track.artist}</div>
            {#if track.quality_tier === 'lossless_hires' || track.quality_tier === 'lossless'}
              <span class="arrival-badge">Lossless</span>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  </section>
{/if}
```

- [ ] **Step 3: Add arrivals styles**

Add to the Home page `<style>` block:

```css
.arrivals-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  gap: 10px;
}

.arrival-card {
  display: flex;
  flex-direction: column;
  gap: 8px;
  border-radius: var(--radius);
  overflow: hidden;
  background: var(--bg-card);
  border: 1px solid var(--border);
  cursor: pointer;
  transition: border-color 0.15s, transform 0.15s;
}

.arrival-card:hover {
  border-color: var(--border-active);
  transform: translateY(-1px);
}

.arrival-art {
  width: 100%;
  aspect-ratio: 1;
  object-fit: cover;
  display: block;
}

.arrival-art-ph {
  width: 100%;
  aspect-ratio: 1;
  background: var(--bg-active);
}

.arrival-info {
  padding: 0 10px 10px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.arrival-title {
  font-size: 0.78rem;
  font-weight: 600;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.arrival-meta {
  font-size: 0.68rem;
  color: var(--text-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.arrival-badge {
  font-size: 0.62rem;
  color: var(--status-ok);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  margin-top: 2px;
}
```

- [ ] **Step 4: Build and verify**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

Expected: build passes.

- [ ] **Step 5: Commit**

```bash
cd "c:/Cassette Music" && git add ui/src/routes/+page.svelte && git commit -m "feat(ui): add New Arrivals section to Home using recently finalized tracks"
```

---

### Task 6: Wire global music search through CommandPalette

**Files:**
- Modify: `ui/src/lib/components/CommandPalette.svelte`
- Modify: `ui/src/lib/stores/commands.ts`

The CommandPalette already exists and has a `commands` store. We extend it so that when the user types into the palette, if no command matches, it falls back to searching tracks via the existing `api.searchTracks`.

- [ ] **Step 1: Read the current CommandPalette implementation**

```bash
cat "c:/Cassette Music/ui/src/lib/components/CommandPalette.svelte"
```

Note the current structure: input binding, command list rendering, keyboard nav. We will add a second results section for track search results below the command list.

- [ ] **Step 2: Add search state to CommandPalette**

In the `<script>` block of `CommandPalette.svelte`, add:

```typescript
import { api, type Track } from '$lib/api/tauri';
import { queueTracks } from '$lib/stores/queue';
import { goto } from '$app/navigation';

let trackResults: Track[] = [];
let searchingTracks = false;
let searchTimeout: ReturnType<typeof setTimeout> | null = null;

// Reactive: when query changes and doesn't match a command, search tracks
$: {
  if (searchTimeout) clearTimeout(searchTimeout);
  if ($query.length >= 2) {
    searchTimeout = setTimeout(async () => {
      searchingTracks = true;
      try {
        trackResults = await api.searchTracks($query);
      } catch {
        trackResults = [];
      } finally {
        searchingTracks = false;
      }
    }, 220);
  } else {
    trackResults = [];
  }
}

async function playTrackResult(track: Track) {
  await queueTracks([track], 0);
  closePalette();
}

async function queueTrackResult(track: Track) {
  await api.addToQueue(track.id);
  closePalette();
}
```

Note: `$query` and `closePalette` must already exist in the CommandPalette — check the existing implementation and use its actual variable names for the query string and close action.

- [ ] **Step 3: Add track results section to the template**

After the existing command list in the template, add:

```svelte
{#if trackResults.length > 0}
  <div class="palette-section-label">Tracks</div>
  {#each trackResults.slice(0, 8) as track}
    <div class="palette-track-result">
      <div class="ptr-info">
        <span class="ptr-title">{track.title}</span>
        <span class="ptr-meta">{track.artist}{track.album ? ` · ${track.album}` : ''}</span>
      </div>
      <div class="ptr-actions">
        <button class="ptr-btn" on:click={() => playTrackResult(track)}>Play</button>
        <button class="ptr-btn" on:click={() => queueTrackResult(track)}>Queue</button>
      </div>
    </div>
  {/each}
{:else if searchingTracks}
  <div class="palette-searching">Searching…</div>
{/if}
```

- [ ] **Step 4: Add track result styles**

Add to the `<style>` block:

```css
.palette-section-label {
  font-size: 0.62rem;
  text-transform: uppercase;
  letter-spacing: 0.1em;
  color: var(--text-muted);
  padding: 6px 12px 3px;
}

.palette-track-result {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 7px 12px;
  border-radius: var(--radius-sm);
  transition: background 0.1s;
}

.palette-track-result:hover { background: var(--bg-hover); }

.ptr-info {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.ptr-title {
  font-size: 0.82rem;
  font-weight: 500;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.ptr-meta {
  font-size: 0.68rem;
  color: var(--text-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.ptr-actions {
  display: flex;
  gap: 4px;
  flex-shrink: 0;
}

.ptr-btn {
  font-size: 0.68rem;
  padding: 2px 8px;
  border-radius: 999px;
  border: 1px solid var(--border);
  color: var(--text-secondary);
  background: var(--bg-hover);
  cursor: pointer;
  transition: background 0.1s, color 0.1s;
}

.ptr-btn:hover { background: var(--bg-active); color: var(--text-primary); }

.palette-searching {
  padding: 8px 12px;
  font-size: 0.78rem;
  color: var(--text-muted);
}
```

- [ ] **Step 5: Build and verify**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

Expected: build passes. If there are type errors about the CommandPalette's existing query store variable name, check the actual variable name in `stores/commands.ts` and use it.

- [ ] **Step 6: Commit**

```bash
cd "c:/Cassette Music" && git add ui/src/lib/components/CommandPalette.svelte ui/src/lib/stores/commands.ts && git commit -m "feat(ui): wire global music search into CommandPalette with play/queue actions"
```

---

### Task 7: Fix `getAlbums()` redundant fetch in artist view

**Files:**
- Modify: `ui/src/routes/artists/+page.svelte`

Currently `selectArtist()` calls `api.getAlbums()` to get all albums on every artist click. The `$albums` store is already loaded.

- [ ] **Step 1: Update `selectArtist` to use the store**

In `artists/+page.svelte`, update the import and the function:

```svelte
<script lang="ts">
  import { buildArtistClusters, clusterAlbumsForArtist, normalizeArtistKey, type ArtistCluster } from '$lib/artist-clusters';
  import { artists, albums } from '$lib/stores/library';
  // ... rest of imports

  async function selectArtist(cluster: ArtistCluster) {
    selectedArtist = cluster;
    selectedAlbum = null;
    albumTracks = [];
    // Use the already-loaded albums store instead of a fresh API call
    artistAlbums = clusterAlbumsForArtist($albums, cluster);
  }
```

Remove the `await api.getAlbums()` call entirely from `selectArtist`. This function no longer needs to be `async` but keeping it `async` is fine.

- [ ] **Step 2: Build and verify**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

Expected: build passes.

- [ ] **Step 3: Commit**

```bash
cd "c:/Cassette Music" && git add ui/src/routes/artists/+page.svelte && git commit -m "perf(ui): use albums store in artist view instead of redundant API call per selection"
```

---

### Task 8: Smoke test and integration verify

- [ ] **Step 1: Full cargo check**

```bash
cd "c:/Cassette Music" && cargo check --workspace 2>&1 | tail -5
```

Expected: `Finished` clean.

- [ ] **Step 2: Full workspace tests**

```bash
cd "c:/Cassette Music" && cargo test --workspace 2>&1 | tail -15
```

Expected: all tests pass.

- [ ] **Step 3: UI production build**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

Expected: build succeeds (pre-existing accessibility warning in downloads page is acceptable).

- [ ] **Step 4: Smoke test**

```powershell
cd "c:/Cassette Music" && .\scripts\smoke_desktop.ps1 2>&1 | tail -20
```

Expected: smoke passes.

- [ ] **Step 5: Commit if any fixes were needed**

```bash
cd "c:/Cassette Music" && git status
```

If anything adjusted:
```bash
git add -p && git commit -m "fix(ui/backend): phase-2 smoke test adjustments"
```

---

## Self-Review

**Spec coverage check:**
- [x] New Arrivals section → Task 5
- [x] Session Composer as first-class route → Task 4
- [x] Global search via CommandPalette → Task 6
- [x] Add to Playlist from context → Task 3
- [x] `getRecentlyFinalizedTracks` backend → Task 1
- [x] `add_track_to_playlist` backend → Task 2
- [x] Remove redundant `getAlbums()` call → Task 7

**Placeholder scan:** No TBDs. All code shown. All method names match their definitions.

**Type consistency:**
- `Track` type used in all tasks matches `ui/src/lib/api/tauri.ts`
- `Playlist` type (id: number, name: string, track_count: number) matches existing definition
- `add_track_to_playlist` takes `playlist_id: i64, track_id: i64` in Rust and `playlistId: number, trackId: number` in TS — Tauri camelCase conversion handles this
- `get_recently_finalized_tracks(days: Option<u32>)` returns `Vec<Track>` matching existing Track model
