<script lang="ts">
  import { api } from '$lib/api/tauri';
  import type { SpotifyAlbumSummary, SpotifyImportResult, SpotifyImportStatus } from '$lib/api/tauri';
  import { formatDuration } from '$lib/utils';
  import { open } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';

  let importResult: SpotifyImportResult | null = null;
  let isLoading = false;
  let error: string | null = null;
  let selectedAlbums = new Set<number>();
  let queuedCount: number | null = null;
  let queuedTrackEstimate: number | null = null;
  let filterMode: 'missing' | 'all' = 'missing';
  let minPlays = 3;
  let persistedStatus: SpotifyImportStatus | null = null;
  let selectedMissing = 0;

  onMount(async () => {
    try {
      persistedStatus = await api.getSpotifyImportStatus();
    } catch {
      persistedStatus = null;
    }
  });

  async function pickFolder() {
    const selected = await open({ directory: true, title: 'Select Spotify History Folder' });
    if (selected) {
      await loadHistory(selected as string);
    }
  }

  async function loadHistory(path: string) {
    isLoading = true;
    error = null;
    importResult = null;
    queuedCount = null;
    selectedAlbums = new Set();
    try {
      importResult = await api.parseSpotifyHistory(path);
      persistedStatus = await api.getSpotifyImportStatus();
    } catch (e) {
      error = String(e);
    } finally {
      isLoading = false;
    }
  }

  function toggleAlbum(idx: number) {
    if (selectedAlbums.has(idx)) {
      selectedAlbums.delete(idx);
    } else {
      selectedAlbums.add(idx);
    }
    selectedAlbums = new Set(selectedAlbums);
  }

  function selectAllVisible() {
    for (let i = 0; i < filtered.length; i++) {
      const globalIdx = importResult!.albums.indexOf(filtered[i]);
      if (!filtered[i].in_library) selectedAlbums.add(globalIdx);
    }
    selectedAlbums = new Set(selectedAlbums);
  }

  function deselectAll() {
    selectedAlbums = new Set();
  }

  async function queueSelected() {
    if (!importResult) return;
    const toQueue: SpotifyAlbumSummary[] = [];
    for (const idx of selectedAlbums) {
      const album = importResult.albums[idx];
      if (album && !album.in_library) toQueue.push(album);
    }
    if (toQueue.length === 0) return;
    try {
      queuedCount = await api.queueSpotifyAlbums(toQueue);
      queuedTrackEstimate = toQueue.reduce((sum, album) => sum + Math.max(1, album.play_count), 0);
      selectedAlbums = new Set();
    } catch (e) {
      error = String(e);
    }
  }

  function selectedMissingCount(): number {
    if (!importResult) return 0;
    let count = 0;
    for (const idx of selectedAlbums) {
      const album = importResult.albums[idx];
      if (album && !album.in_library) count += 1;
    }
    return count;
  }

  function msToHours(ms: number): string {
    const hours = ms / 3_600_000;
    if (hours >= 1) return `${hours.toFixed(1)}h`;
    const mins = ms / 60_000;
    return `${mins.toFixed(0)}m`;
  }

  $: filtered = importResult
    ? importResult.albums.filter(a => {
        if (filterMode === 'missing' && a.in_library) return false;
        if (a.play_count < minPlays) return false;
        return true;
      })
    : [];

  $: selectedMissing = selectedMissingCount();
</script>

<svelte:head><title>Import · Cassette</title></svelte:head>

<div class="import-page">
  <div class="page-header">
    <h2 style="flex:1">Import</h2>
  </div>

  <div class="import-section">
    <div class="import-source">
      <div class="source-label">Spotify Extended Streaming History</div>
      <div class="source-desc">
        Select the folder containing your <code>Streaming_History_Audio_*.json</code> files from Spotify's data export.
      </div>
      <div class="source-help">
        <strong>How this works:</strong>
        <ul>
          <li>1. Load your Spotify export JSON files.</li>
          <li>2. Review albums marked <em>Missing</em>.</li>
          <li>3. Queue selected albums for acquisition in Downloads.</li>
        </ul>
      </div>
      {#if persistedStatus}
        <div class="source-desc">
          Persisted rows: <strong>{persistedStatus.album_rows.toLocaleString()}</strong>
          {#if persistedStatus.last_imported_at}
            · Last import: <strong>{persistedStatus.last_imported_at}</strong>
          {/if}
        </div>
      {/if}
      <button class="btn btn-primary" on:click={pickFolder} disabled={isLoading}>
        {isLoading ? 'Parsing...' : 'Select Folder'}
      </button>
    </div>
  </div>

  {#if error}
    <div class="import-error">{error}</div>
  {/if}

  {#if queuedCount !== null}
    <div class="dl-notice">
      {queuedCount} album{queuedCount === 1 ? '' : 's'} queued for download
      {#if queuedTrackEstimate !== null}
        (about {queuedTrackEstimate} track request{queuedTrackEstimate === 1 ? '' : 's'}).
      {:else}
        .
      {/if}
    </div>
  {/if}

  {#if importResult}
    <div class="import-stats">
      <div class="stat-card">
        <div class="stat-num">{importResult.total_streams.toLocaleString()}</div>
        <div class="stat-lbl">Total Streams</div>
      </div>
      <div class="stat-card">
        <div class="stat-num">{importResult.unique_albums.toLocaleString()}</div>
        <div class="stat-lbl">Unique Albums</div>
      </div>
      <div class="stat-card">
        <div class="stat-num">{importResult.already_in_library}</div>
        <div class="stat-lbl">Already In Library</div>
      </div>
      <div class="stat-card">
        <div class="stat-num">{importResult.unique_albums - importResult.already_in_library}</div>
        <div class="stat-lbl">Missing</div>
      </div>
    </div>

    <div class="import-controls">
      <div class="filter-row">
        <label class="filter-item">
          <input type="radio" bind:group={filterMode} value="missing" /> Missing only
        </label>
        <label class="filter-item">
          <input type="radio" bind:group={filterMode} value="all" /> All
        </label>
        <label class="filter-item">
          Min plays: <input type="number" bind:value={minPlays} min={1} max={100} class="input" style="width:60px;padding:4px 6px;" />
        </label>
      </div>
      <div class="action-row">
        <button class="btn btn-ghost" on:click={selectAllVisible}>Select All ({filtered.filter(a => !a.in_library).length})</button>
        <button class="btn btn-ghost" on:click={deselectAll}>Deselect</button>
        {#if selectedMissing > 0}
          <button class="btn btn-primary" on:click={queueSelected}>
            Queue {selectedMissing} Album{selectedMissing === 1 ? '' : 's'}
          </button>
        {/if}
      </div>
    </div>

    <div class="album-table">
      <div class="table-header">
        <div class="col-check"></div>
        <div class="col-artist">Artist</div>
        <div class="col-album">Album</div>
        <div class="col-time">Listen Time</div>
        <div class="col-plays">Plays</div>
        <div class="col-status">Status</div>
      </div>
            {#each filtered as album, i}
        {@const globalIdx = importResult.albums.indexOf(album)}
        {#if album.in_library}
          <div class="table-row in-library">
            <div class="col-check">
              <span class="check-lib" title="In library">✓</span>
            </div>
            <div class="col-artist">{album.artist}</div>
            <div class="col-album">{album.album}</div>
            <div class="col-time">{msToHours(album.total_ms)}</div>
            <div class="col-plays">{album.play_count}</div>
            <div class="col-status">
              <span class="badge badge-success">In Library</span>
            </div>
          </div>
        {:else}
          <div
            class="table-row"
            class:selected={selectedAlbums.has(globalIdx)}
            role="button"
            tabindex="0"
            on:click={() => toggleAlbum(globalIdx)}
            on:keydown={(event) => {
              if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault();
                toggleAlbum(globalIdx);
              }
            }}
          >
            <div class="col-check">
              <input type="checkbox" checked={selectedAlbums.has(globalIdx)} on:click|stopPropagation={() => toggleAlbum(globalIdx)} />
            </div>
            <div class="col-artist">{album.artist}</div>
            <div class="col-album">{album.album}</div>
            <div class="col-time">{msToHours(album.total_ms)}</div>
            <div class="col-plays">{album.play_count}</div>
            <div class="col-status">
              <span class="badge badge-muted">Missing</span>
            </div>
          </div>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
.import-page { display: flex; flex-direction: column; min-height: 100%; }

.import-section { padding: 0 1.5rem 1rem; }
.import-source {
  background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius-sm);
  padding: 16px 20px; display: flex; flex-direction: column; gap: 8px; max-width: 560px;
}
.source-label { font-weight: 600; font-size: 0.95rem; }
.source-desc { font-size: 0.8rem; color: var(--text-secondary); line-height: 1.5; }
.source-desc code { background: var(--bg-active); padding: 2px 5px; border-radius: 3px; font-size: 0.78rem; }

.source-help {
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--accent) 9%, var(--bg-card));
  padding: 8px 10px;
  font-size: 0.78rem;
  color: var(--text-secondary);
}

.source-help strong {
  color: var(--text-primary);
}

.source-help ul {
  margin: 6px 0 0;
  padding-left: 16px;
}

.source-help li {
  margin: 2px 0;
}

.import-error {
  margin: 0 1.5rem 0.75rem; padding: 8px 12px; border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--error) 12%, var(--bg-card)); border: 1px solid var(--error);
  color: var(--error); font-size: 0.8rem;
}

.dl-notice {
  margin: 0 1.5rem 0.75rem; padding: 8px 12px; border-radius: var(--radius-sm);
  border: 1px solid var(--border-active); background: color-mix(in srgb, var(--accent) 12%, var(--bg-card));
  color: var(--text-primary); font-size: 0.8rem;
}

.import-stats {
  display: flex; gap: 12px; padding: 0 1.5rem 1rem; flex-wrap: wrap;
}
.stat-card {
  background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius-sm);
  padding: 12px 20px; text-align: center; min-width: 100px;
}
.stat-num { font-size: 1.4rem; font-weight: 700; color: var(--accent-bright); }
.stat-lbl { font-size: 0.7rem; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.05em; margin-top: 2px; }

.import-controls { padding: 0 1.5rem 0.75rem; display: flex; flex-direction: column; gap: 8px; }
.filter-row { display: flex; align-items: center; gap: 16px; font-size: 0.82rem; color: var(--text-secondary); }
.filter-item { display: flex; align-items: center; gap: 4px; cursor: pointer; }
.action-row { display: flex; align-items: center; gap: 8px; }

.album-table { padding: 0 1.5rem 2rem; }
.table-header, .table-row {
  display: grid; grid-template-columns: 32px 1.2fr 1.5fr 80px 60px 90px;
  align-items: center; gap: 8px; padding: 8px 12px; font-size: 0.82rem;
}
.table-header {
  color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.05em;
  font-size: 0.7rem; font-weight: 600; border-bottom: 1px solid var(--border);
}
.table-row {
  border-radius: var(--radius-sm); cursor: pointer; transition: background 0.1s;
}
.table-row:hover { background: var(--bg-hover); }
.table-row.in-library { opacity: 0.5; cursor: default; }
.table-row.selected { background: color-mix(in srgb, var(--accent) 10%, var(--bg-card)); }

.col-check { display: flex; align-items: center; justify-content: center; }
.check-lib { color: var(--accent); font-size: 0.85rem; }
.col-artist { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.col-album { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.col-time, .col-plays { text-align: center; color: var(--text-secondary); }
</style>

