<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type Track } from '$lib/api/tauri';
  import { buildTrackMap, filterTracksForCrate, hydrateTrackIds, totalDurationSecs } from '$lib/ritual-helpers';
  import { formatDuration } from '$lib/utils';
  import { loadLibrary, tracks } from '$lib/stores/library';
  import {
    crates,
    deleteCrate,
    emptyCrateFilter,
    loadRituals,
    saveCrate,
    saveSessionRecord,
    type CrateFilter,
    type CrateRecord,
  } from '$lib/stores/rituals';
  import { replaceQueueTrackIds } from '$lib/stores/queue';
  import { createPlaylist, loadPlaylists, playlists } from '$lib/stores/playlists';

  onMount(async () => {
    await Promise.all([loadLibrary(), loadRituals(), loadPlaylists()]);
  });

  let selectedCrateId: string | null = null;
  let crateName = '';
  let crateNote = '';
  let crateKind: CrateRecord['kind'] = 'saved';
  let filterDraft: CrateFilter = emptyCrateFilter();
  let playlistFilterTrackIds = new Set<number>();
  let playlistFilterGuard: number | null = null;

  $: trackMap = buildTrackMap($tracks);
  $: selectedCrate = $crates.find((crate) => crate.id === selectedCrateId) ?? null;
  $: previewTracks = filterTracksForCrate($tracks, filterDraft, playlistFilterTrackIds.size > 0 ? playlistFilterTrackIds : null);
  $: previewTrackIds = previewTracks.map((track) => track.id);
  $: previewDuration = totalDurationSecs(previewTracks);

  $: if (filterDraft.playlistId !== playlistFilterGuard) {
    playlistFilterGuard = filterDraft.playlistId;
    void refreshPlaylistFilter();
  }

  async function refreshPlaylistFilter() {
    if (!filterDraft.playlistId) {
      playlistFilterTrackIds = new Set<number>();
      return;
    }
    try {
      const items = await api.getPlaylistItems(filterDraft.playlistId);
      playlistFilterTrackIds = new Set(items.map((item) => item.track_id));
    } catch {
      playlistFilterTrackIds = new Set<number>();
    }
  }

  function resetDraft() {
    selectedCrateId = null;
    crateName = '';
    crateNote = '';
    crateKind = 'saved';
    filterDraft = emptyCrateFilter();
  }

  function loadCrateIntoDraft(crate: CrateRecord) {
    selectedCrateId = crate.id;
    crateName = crate.name;
    crateNote = crate.note;
    crateKind = crate.kind;
    filterDraft = { ...crate.filter };
  }

  async function saveDraftCrate() {
    if (!crateName.trim() || previewTrackIds.length === 0) {
      return;
    }
    const source = filterDraft.playlistId
      ? 'playlist'
      : filterDraft.albumQuery
        ? 'album'
        : filterDraft.artistQuery
          ? 'artist'
          : filterDraft.format
            ? 'format'
            : filterDraft.qualityTier
              ? 'quality'
              : 'manual';

    const id = await saveCrate({
      id: selectedCrateId ?? undefined,
      name: crateName.trim(),
      note: crateNote.trim(),
      kind: crateKind,
      source,
      filter: { ...filterDraft },
      trackIds: previewTrackIds,
    });
    selectedCrateId = id;
  }

  async function playCrate(crate: CrateRecord) {
    await replaceQueueTrackIds(crate.trackIds, 0);
  }

  async function turnCrateIntoSession(crate: CrateRecord) {
    await saveSessionRecord({
      name: crate.name,
      note: crate.note,
      trackIds: crate.trackIds,
      reasons: [],
      source: 'crate',
      sourceRefId: crate.id,
      branchOfId: null,
      modeSnapshot: null,
    });
  }

  async function turnCrateIntoPlaylist(crate: CrateRecord) {
    await createPlaylist(crate.name, crate.note || null, crate.trackIds);
  }

  function crateTracks(crate: CrateRecord): Track[] {
    return hydrateTrackIds(crate.trackIds, trackMap);
  }

  function playlistLabel(playlistId: number | null): string {
    if (!playlistId) {
      return 'Manual slice';
    }
    const playlist = $playlists.find((item) => item.id === playlistId);
    return playlist?.name ?? 'Playlist slice';
  }
</script>

<svelte:head><title>Crates - Cassette</title></svelte:head>

<div class="crates-page">
  <section class="card crates-hero">
    <div class="section-kicker">Flexible slices</div>
    <div class="crates-hero-row">
      <div class="crates-hero-copy">
        <h1>Cut shelves into living crates</h1>
        <p>
          A crate is a saved or temporary slice of the collection. Pull one by artist, format,
          quality, or playlist, then send it into the queue or session when you need a faster start.
        </p>
      </div>
      <div class="crates-hero-actions">
        <button class="btn btn-secondary" on:click={resetDraft}>New slice</button>
      </div>
    </div>
  </section>

  <div class="crates-layout">
    <aside class="card crate-list">
      <div class="crate-list-head">
        <div>
          <div class="section-kicker subtle">Saved and temporary</div>
          <h2>Crates</h2>
        </div>
      </div>

      {#if $crates.length === 0}
        <div class="empty-state">
          <div class="empty-title">No crates yet</div>
          <div class="empty-body">Save a slice when you want a shelf that is lighter than a playlist.</div>
        </div>
      {:else}
        <div class="crate-list-scroll">
          {#each $crates as crate}
            {@const active = crate.id === selectedCrateId}
            <div
              class="crate-item"
              class:active
              role="button"
              tabindex="0"
              on:click={() => loadCrateIntoDraft(crate)}
              on:keydown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  loadCrateIntoDraft(crate);
                }
              }}
            >
              <span class="crate-item-head">
                <span class="crate-name">{crate.name}</span>
                <span class="crate-kind">{crate.kind}</span>
              </span>
              <span class="crate-meta">
                {crate.trackIds.length} tracks / {formatDuration(totalDurationSecs(crateTracks(crate)))}
              </span>
              <span class="crate-source">{playlistLabel(crate.filter.playlistId)}</span>
              <span class="crate-actions">
                <button class="crate-action-link" on:click|stopPropagation={() => playCrate(crate)}>Play</button>
                <button class="crate-action-link danger" on:click|stopPropagation={() => deleteCrate(crate.id)}>Delete</button>
              </span>
            </div>
          {/each}
        </div>
      {/if}
    </aside>

    <section class="crate-main">
      <section class="card crate-builder">
        <div class="crate-builder-head">
          <div>
            <div class="section-kicker subtle">{selectedCrateId ? 'Edit crate' : 'Build crate'}</div>
            <h2>{selectedCrateId ? 'Refine the slice' : 'Shape a new slice'}</h2>
          </div>
          <button class="btn btn-primary" on:click={saveDraftCrate} disabled={previewTrackIds.length === 0}>
            {selectedCrateId ? 'Update crate' : 'Save crate'}
          </button>
        </div>

        <div class="crate-builder-grid">
          <label>
            Crate name
            <input class="input" bind:value={crateName} placeholder="Needle drop / rainy side room / hi-res only" />
          </label>
          <label>
            Type
            <select class="input" bind:value={crateKind}>
              <option value="saved">Saved crate</option>
              <option value="temporary">Temporary crate</option>
            </select>
          </label>
          <label>
            Artist filter
            <input class="input" bind:value={filterDraft.artistQuery} placeholder="Artist contains..." />
          </label>
          <label>
            Album filter
            <input class="input" bind:value={filterDraft.albumQuery} placeholder="Album contains..." />
          </label>
          <label>
            Format
            <select class="input" bind:value={filterDraft.format}>
              <option value="">Any</option>
              <option value="flac">FLAC</option>
              <option value="mp3">MP3</option>
              <option value="m4a">M4A</option>
              <option value="wav">WAV</option>
            </select>
          </label>
          <label>
            Quality tier
            <select class="input" bind:value={filterDraft.qualityTier}>
              <option value="">Any</option>
              <option value="lossless">Lossless</option>
              <option value="hi_res_lossless">Hi-res lossless</option>
              <option value="lossy">Lossy</option>
            </select>
          </label>
          <label>
            Year from
            <input class="input" type="number" bind:value={filterDraft.yearFrom} placeholder="1997" />
          </label>
          <label>
            Year to
            <input class="input" type="number" bind:value={filterDraft.yearTo} placeholder="2025" />
          </label>
          <label class="crate-builder-span">
            Playlist slice
            <select class="input" bind:value={filterDraft.playlistId}>
              <option value={null}>None</option>
              {#each $playlists as playlist}
                <option value={playlist.id}>{playlist.name}</option>
              {/each}
            </select>
          </label>
          <label class="crate-builder-span">
            Crate note
            <textarea class="input textarea" bind:value={crateNote} placeholder="Why this slice exists."></textarea>
          </label>
        </div>
      </section>

      <section class="card crate-preview">
        <div class="crate-preview-head">
          <div>
            <div class="section-kicker subtle">Current slice</div>
            <h3>{previewTrackIds.length} tracks ready</h3>
          </div>
          <div class="crate-preview-actions">
            <button class="btn btn-primary" on:click={() => replaceQueueTrackIds(previewTrackIds, 0)} disabled={previewTrackIds.length === 0}>Play preview</button>
            <button
              class="btn btn-secondary"
              on:click={() => saveSessionRecord({
                name: crateName.trim() || 'Untitled crate session',
                note: crateNote.trim(),
                trackIds: previewTrackIds,
                reasons: [],
                source: 'crate',
                sourceRefId: selectedCrateId,
                branchOfId: null,
                modeSnapshot: null,
              })}
              disabled={previewTrackIds.length === 0}
            >
              Send to session
            </button>
          </div>
        </div>
        <div class="crate-preview-meta">{formatDuration(previewDuration)} / {playlistLabel(filterDraft.playlistId)}</div>

        {#if previewTracks.length === 0}
          <div class="empty-state compact">
            <div class="empty-title">No tracks match</div>
            <div class="empty-body">Loosen the filters until the slice feels like something you would actually reach for.</div>
          </div>
        {:else}
          <div class="crate-preview-list">
            {#each previewTracks.slice(0, 16) as track}
              <div class="crate-track">
                <div>
                  <div class="crate-track-title">{track.title}</div>
                  <div class="crate-track-meta">{track.artist} / {track.album}</div>
                </div>
                <div class="crate-track-duration">{formatDuration(track.duration_secs)}</div>
              </div>
            {/each}
            {#if previewTracks.length > 16}
              <div class="crate-more">+ {previewTracks.length - 16} more tracks in this slice</div>
            {/if}
          </div>
        {/if}
      </section>

      {#if selectedCrate}
        <section class="card crate-actions-panel">
          <div class="section-kicker subtle">Conversion handoff</div>
          <h3>{selectedCrate.name}</h3>
          <div class="crate-actions-row">
            <button class="btn btn-primary" on:click={() => playCrate(selectedCrate)}>Play crate</button>
            <button class="btn btn-secondary" on:click={() => turnCrateIntoSession(selectedCrate)}>Save as session</button>
            <button class="btn btn-ghost" on:click={() => turnCrateIntoPlaylist(selectedCrate)}>Save as playlist</button>
          </div>
        </section>
      {/if}
    </section>
  </div>
</div>

<style>
  .crates-page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 18px;
  }

  .section-kicker {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--accent-bright);
    font-weight: 700;
  }

  .section-kicker.subtle {
    color: var(--text-muted);
  }

  .crates-hero,
  .crate-list,
  .crate-builder,
  .crate-preview,
  .crate-actions-panel {
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.02), transparent 24%),
      var(--bg-card);
  }

  .crates-hero-row,
  .crate-builder-head,
  .crate-preview-head {
    display: flex;
    justify-content: space-between;
    gap: 16px;
  }

  .crates-hero-copy,
  .crate-builder-head > div:first-child {
    display: grid;
    gap: 8px;
  }

  .crates-hero-copy h1 {
    font-size: clamp(1.9rem, 4vw, 3rem);
    line-height: 0.95;
  }

  .crates-hero-copy p,
  .crate-preview-meta {
    color: var(--text-secondary);
    line-height: 1.7;
  }

  .crates-layout {
    display: grid;
    grid-template-columns: 320px minmax(0, 1fr);
    gap: 16px;
  }

  .crate-list,
  .crate-main,
  .crate-builder,
  .crate-preview,
  .crate-actions-panel {
    display: grid;
    gap: 14px;
  }

  .crate-list-scroll,
  .crate-preview-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    overflow-y: auto;
    min-height: 0;
  }

  .crate-item,
  .crate-track {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: rgba(255, 255, 255, 0.02);
    padding: 12px;
  }

  .crate-item {
    width: 100%;
    text-align: left;
    display: grid;
    gap: 6px;
    transition: border-color 0.15s, background 0.15s;
  }

  .crate-item:hover {
    border-color: var(--border-active);
    background: rgba(139, 180, 212, 0.06);
  }

  .crate-item.active {
    border-color: rgba(247, 180, 92, 0.35);
    background: rgba(247, 180, 92, 0.07);
  }

  .crate-item-head,
  .crate-actions,
  .crate-track {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    align-items: center;
  }

  .crate-name,
  .crate-track-title {
    color: var(--text-primary);
    font-weight: 700;
  }

  .crate-kind,
  .crate-action-link,
  .crate-track-duration {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }

  .crate-action-link.danger {
    color: color-mix(in srgb, var(--error) 80%, white);
  }

  .crate-meta,
  .crate-source,
  .crate-track-meta {
    color: var(--text-secondary);
    font-size: 0.76rem;
  }

  .crate-builder-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }

  .crate-builder-grid label {
    display: grid;
    gap: 6px;
    font-size: 0.75rem;
    color: var(--text-secondary);
  }

  .crate-builder-span {
    grid-column: 1 / -1;
  }

  .textarea {
    min-height: 92px;
    resize: vertical;
  }

  .crate-preview-actions,
  .crate-actions-row {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }

  .crate-more {
    color: var(--text-muted);
    font-size: 0.76rem;
    padding: 8px 4px 0;
  }

  .empty-state {
    border: 1px dashed var(--border);
    border-radius: var(--radius);
    padding: 18px;
    display: grid;
    gap: 6px;
    color: var(--text-secondary);
  }

  .empty-state.compact {
    padding: 14px;
  }

  .empty-title {
    color: var(--text-primary);
    font-weight: 700;
  }

  .empty-body {
    line-height: 1.65;
  }

  @media (max-width: 1120px) {
    .crates-layout,
    .crate-builder-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
