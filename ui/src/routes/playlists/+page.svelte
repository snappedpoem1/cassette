<script lang="ts">
  import { onMount } from 'svelte';
  import ContextActionRail from '$lib/components/ContextActionRail.svelte';
  import { formatDuration } from '$lib/utils';
  import type { Playlist, Track } from '$lib/api/tauri';
  import {
    activePlaylistId,
    activePlaylistItems,
    createPlaylist,
    deletePlaylist,
    loadPlaylistItems,
    loadPlaylists,
    playlists,
    playPlaylist,
  } from '$lib/stores/playlists';
  import {
    authorshipForPlaylist,
    emptyCrateFilter,
    loadRituals,
    removePlaylistSection,
    removePlaylistVariant,
    saveCrate,
    savePlaylistSection,
    savePlaylistVariant,
    saveSessionRecord,
    type PlaylistSection,
    type PlaylistVariant,
    updatePlaylistAuthorship,
  } from '$lib/stores/rituals';
  import { hydrateTrackIds, sectionSlices, totalDurationSecs } from '$lib/ritual-helpers';
  import { loadLibrary, tracks } from '$lib/stores/library';
  import { replaceQueueTrackIds } from '$lib/stores/queue';

  onMount(async () => {
    await Promise.all([loadPlaylists(), loadRituals(), loadLibrary()]);
  });

  let creating = false;
  let newName = '';
  let newDesc = '';
  let confirmDeleteId: number | null = null;
  let selectedTrack: Track | null = null;

  let noteDraft = '';
  let moodDraft = '';
  let noteSyncGuard: number | null = null;

  let sectionTitle = '';
  let sectionArcLabel = '';
  let sectionNote = '';
  let sectionStart = 1;
  let sectionEnd = 4;

  let variantName = '';
  let variantNote = '';
  let variantSource = 'full';

  $: activePlaylist = $playlists.find((playlist) => playlist.id === $activePlaylistId) ?? null;
  $: authorship = $activePlaylistId ? authorshipForPlaylist($activePlaylistId) : null;
  $: playlistTrackIds = $activePlaylistItems.map((item) => item.track_id);
  $: sectionViews = authorship ? sectionSlices(authorship.sections, $activePlaylistItems) : [];
  $: playlistRuntime = totalDurationSecs($activePlaylistItems.map((item) => item.track).filter((track): track is Track => Boolean(track)));
  $: playlistTrackCount = $activePlaylistItems.length;
  $: selectedSectionForVariant = variantSource === 'full'
    ? null
    : sectionViews.find((view) => view.section.id === variantSource) ?? null;

  $: if ($activePlaylistId !== noteSyncGuard) {
    noteSyncGuard = $activePlaylistId;
    noteDraft = authorship?.note ?? '';
    moodDraft = authorship?.mood ?? '';
    selectedTrack = null;
    const maxIndex = Math.max(1, playlistTrackCount);
    sectionStart = 1;
    sectionEnd = Math.min(4, maxIndex);
    variantSource = 'full';
  }

  async function handleCreate() {
    if (!newName.trim()) {
      return;
    }
    const id = await createPlaylist(newName.trim(), newDesc.trim() || null);
    newName = '';
    newDesc = '';
    creating = false;
    await loadPlaylistItems(id);
  }

  async function handleDelete(pl: Playlist) {
    if (confirmDeleteId === pl.id) {
      await deletePlaylist(pl.id);
      confirmDeleteId = null;
      return;
    }

    confirmDeleteId = pl.id;
    setTimeout(() => {
      if (confirmDeleteId === pl.id) {
        confirmDeleteId = null;
      }
    }, 2800);
  }

  async function saveVoice() {
    if (!$activePlaylistId) {
      return;
    }
    await updatePlaylistAuthorship($activePlaylistId, {
      note: noteDraft.trim(),
      mood: moodDraft.trim(),
    });
  }

  function sectionTrackIds(section: PlaylistSection): number[] {
    return playlistTrackIds.slice(section.startIndex, section.endIndex + 1);
  }

  async function addSection() {
    if (!$activePlaylistId || playlistTrackCount === 0 || !sectionTitle.trim()) {
      return;
    }
    const maxIndex = Math.max(1, playlistTrackCount);
    await savePlaylistSection($activePlaylistId, {
      title: sectionTitle.trim(),
      arcLabel: sectionArcLabel.trim(),
      note: sectionNote.trim(),
      startIndex: Math.max(0, Math.min(sectionStart - 1, maxIndex - 1)),
      endIndex: Math.max(0, Math.min(sectionEnd - 1, maxIndex - 1)),
    });
    sectionTitle = '';
    sectionArcLabel = '';
    sectionNote = '';
  }

  async function addVariant() {
    if (!$activePlaylistId || !variantName.trim()) {
      return;
    }

    const trackIds = selectedSectionForVariant
      ? selectedSectionForVariant.items.map((item) => item.track_id)
      : playlistTrackIds;

    if (trackIds.length === 0) {
      return;
    }

    await savePlaylistVariant($activePlaylistId, {
      name: variantName.trim(),
      note: variantNote.trim(),
      trackIds,
    });
    variantName = '';
    variantNote = '';
    variantSource = 'full';
  }

  async function playVariant(variant: PlaylistVariant) {
    await replaceQueueTrackIds(variant.trackIds, 0);
  }

  async function turnVariantIntoSession(variant: PlaylistVariant) {
    await saveSessionRecord({
      name: variant.name,
      note: variant.note,
      trackIds: variant.trackIds,
      reasons: [],
      source: 'playlist',
      sourceRefId: `${$activePlaylistId}:${variant.id}`,
      branchOfId: null,
      modeSnapshot: null,
    });
  }

  async function turnPlaylistIntoSession() {
    if (!activePlaylist || playlistTrackIds.length === 0) {
      return;
    }
    await saveSessionRecord({
      name: activePlaylist.name,
      note: noteDraft.trim(),
      trackIds: playlistTrackIds,
      reasons: [],
      source: 'playlist',
      sourceRefId: `${activePlaylist.id}`,
      branchOfId: null,
      modeSnapshot: null,
    });
  }

  async function savePlaylistAsCrate() {
    if (!activePlaylist || playlistTrackIds.length === 0) {
      return;
    }
    await saveCrate({
      name: `${activePlaylist.name} crate`,
      note: noteDraft.trim() || 'Saved from playlist authorship.',
      kind: 'saved',
      source: 'playlist',
      filter: { ...emptyCrateFilter(), playlistId: activePlaylist.id },
      trackIds: playlistTrackIds,
    });
  }

  function variantTracks(variant: PlaylistVariant): Track[] {
    return hydrateTrackIds(variant.trackIds, new Map($tracks.map((track) => [track.id, track])));
  }
</script>

<svelte:head><title>Playlists - Cassette</title></svelte:head>

<div class="playlists-page">
  <section class="playlist-hero card">
    <div class="section-kicker">Authorship ritual</div>
    <div class="playlist-hero-row">
      <div class="playlist-hero-copy">
        <h1>Write the run, not just the list</h1>
        <p>
          Playlist shape lives in the notes, section breaks, and alternate cuts. Keep one authored
          line, then spin variants when the night wants a different turn.
        </p>
      </div>
      <div class="playlist-hero-actions">
        <button class="btn btn-primary" on:click={() => (creating = !creating)}>New playlist</button>
        {#if activePlaylist}
          <button class="btn btn-secondary" on:click={turnPlaylistIntoSession}>Send to session</button>
          <button class="btn btn-ghost" on:click={savePlaylistAsCrate}>Save as crate</button>
        {/if}
      </div>
    </div>
  </section>

  {#if creating}
    <section class="card create-form">
      <label>
        Title
        <input class="input" bind:value={newName} placeholder="Late train / side B" />
      </label>
      <label>
        Notes
        <textarea class="input textarea" bind:value={newDesc} placeholder="What this run is for."></textarea>
      </label>
      <div class="create-actions">
        <button class="btn btn-primary" on:click={handleCreate}>Create playlist</button>
        <button class="btn btn-ghost" on:click={() => (creating = false)}>Cancel</button>
      </div>
    </section>
  {/if}

  <div class="playlists-layout">
    <aside class="playlist-list card">
      <div class="playlist-list-head">
        <div>
          <div class="section-kicker subtle">Library of authored runs</div>
          <h2>Playlists</h2>
        </div>
      </div>

      {#if $playlists.length === 0}
        <div class="empty-state">
          <div class="empty-title">No playlists yet</div>
          <div class="empty-body">Start one when a run deserves a title, a note, and a return path.</div>
        </div>
      {:else}
        <div class="playlist-list-scroll">
          {#each $playlists as playlist}
            {@const active = playlist.id === $activePlaylistId}
            <div
              class="playlist-item mood-card"
              class:active
              role="button"
              tabindex="0"
              on:click={() => loadPlaylistItems(playlist.id)}
              on:keydown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  loadPlaylistItems(playlist.id);
                }
              }}
            >
              <span class="playlist-item-head">
                <span class="playlist-name">{playlist.name}</span>
                <span class="playlist-count">{playlist.track_count}</span>
              </span>
              <span class="playlist-meta">
                {formatDuration(totalDurationSecs(hydrateTrackIds(
                  $activePlaylistId === playlist.id ? $activePlaylistItems.map((item) => item.track_id) : [],
                  new Map($tracks.map((track) => [track.id, track]))
                ))) || 'Authored run'}
              </span>
              <span class="playlist-actions">
                <button class="playlist-action-link" on:click|stopPropagation={() => playPlaylist(playlist.id)}>Play</button>
                <button
                  class="playlist-action-link danger"
                  on:click|stopPropagation={() => handleDelete(playlist)}
                >
                  {confirmDeleteId === playlist.id ? 'Sure?' : 'Delete'}
                </button>
              </span>
            </div>
          {/each}
        </div>
      {/if}
    </aside>

    <section class="playlist-main">
      {#if !activePlaylist}
        <div class="card empty-state roomy">
          <div class="empty-title">Choose a playlist</div>
          <div class="empty-body">Open one from the left to shape its voice, sections, and alternate cuts.</div>
        </div>
      {:else}
        <section class="card authorship-card">
          <div class="authorship-head">
            <div>
              <div class="section-kicker">{authorship?.mood || 'Current voice'}</div>
              <h2>{activePlaylist.name}</h2>
              <div class="authorship-meta">
                {playlistTrackCount} tracks / {formatDuration(playlistRuntime)}
              </div>
            </div>
            <div class="authorship-actions">
              <button class="btn btn-primary" on:click={() => playPlaylist(activePlaylist.id)}>Play run</button>
              <button class="btn btn-secondary" on:click={saveVoice}>Save notes</button>
            </div>
          </div>

          <div class="authorship-grid">
            <label>
              Playlist note
              <textarea
                class="input textarea"
                bind:value={noteDraft}
                placeholder="What holds this run together?"
              ></textarea>
            </label>
            <label>
              Mood line
              <input class="input" bind:value={moodDraft} placeholder="low glow / commuter comedown / side A spring" />
            </label>
          </div>
        </section>

        <section class="section-grid">
          <div class="card section-card">
            <div class="section-head">
              <div>
                <div class="section-kicker subtle">Arc labels</div>
                <h3>Sections</h3>
              </div>
            </div>

            <div class="section-form">
              <label>
                Title
                <input class="input" bind:value={sectionTitle} placeholder="Open window / side turn / after midnight" />
              </label>
              <label>
                Arc label
                <input class="input" bind:value={sectionArcLabel} placeholder="lift / settle / last call" />
              </label>
              <label>
                Start slot
                <input class="input" type="number" min="1" max={Math.max(1, playlistTrackCount)} bind:value={sectionStart} />
              </label>
              <label>
                End slot
                <input class="input" type="number" min="1" max={Math.max(1, playlistTrackCount)} bind:value={sectionEnd} />
              </label>
              <label class="section-form-span">
                Section note
                <textarea class="input textarea" bind:value={sectionNote} placeholder="Why this handoff matters."></textarea>
              </label>
              <button class="btn btn-primary" on:click={addSection} disabled={playlistTrackCount === 0}>Add section</button>
            </div>

            {#if sectionViews.length === 0}
              <div class="empty-state compact">
                <div class="empty-title">No sections yet</div>
                <div class="empty-body">Label the parts that change the emotional temperature.</div>
              </div>
            {:else}
              <div class="section-list">
                {#each sectionViews as view}
                  <div class="section-item">
                    <div class="section-item-head">
                      <div>
                        <div class="section-item-title">{view.section.title}</div>
                        <div class="section-item-meta">
                          {view.section.arcLabel || 'Arc label pending'} / slots {view.section.startIndex + 1}-{view.section.endIndex + 1}
                        </div>
                      </div>
                      <div class="section-item-actions">
                        <button class="btn btn-ghost btn-small" on:click={() => replaceQueueTrackIds(sectionTrackIds(view.section), 0)}>Play cut</button>
                        <button class="btn btn-ghost btn-small" on:click={() => removePlaylistSection(activePlaylist.id, view.section.id)}>Remove</button>
                      </div>
                    </div>
                    {#if view.section.note}
                      <p class="section-item-note">{view.section.note}</p>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          </div>

          <div class="card section-card">
            <div class="section-head">
              <div>
                <div class="section-kicker subtle">Alternate cuts</div>
                <h3>Variants</h3>
              </div>
            </div>

            <div class="section-form">
              <label>
                Variant name
                <input class="input" bind:value={variantName} placeholder="Night bus version / lighter bridge" />
              </label>
              <label>
                Source slice
                <select class="input" bind:value={variantSource}>
                  <option value="full">Full playlist</option>
                  {#each sectionViews as view}
                    <option value={view.section.id}>{view.section.title}</option>
                  {/each}
                </select>
              </label>
              <label class="section-form-span">
                Variant note
                <textarea class="input textarea" bind:value={variantNote} placeholder="Why this version exists."></textarea>
              </label>
              <button class="btn btn-primary" on:click={addVariant} disabled={playlistTrackCount === 0}>Save variant</button>
            </div>

            {#if !authorship || authorship.variants.length === 0}
              <div class="empty-state compact">
                <div class="empty-title">No variants yet</div>
                <div class="empty-body">Save a shorter cut, a side-path, or a section-only version here.</div>
              </div>
            {:else}
              <div class="variant-list">
                {#each authorship.variants as variant}
                  <div class="variant-item">
                    <div class="variant-item-head">
                      <div>
                        <div class="variant-item-title">{variant.name}</div>
                        <div class="variant-item-meta">
                          {variant.trackIds.length} tracks / {formatDuration(totalDurationSecs(variantTracks(variant)))}
                        </div>
                      </div>
                      <div class="variant-item-actions">
                        <button class="btn btn-ghost btn-small" on:click={() => playVariant(variant)}>Play</button>
                        <button class="btn btn-ghost btn-small" on:click={() => turnVariantIntoSession(variant)}>Session</button>
                        <button class="btn btn-ghost btn-small" on:click={() => removePlaylistVariant(activePlaylist.id, variant.id)}>Remove</button>
                      </div>
                    </div>
                    {#if variant.note}
                      <p class="variant-item-note">{variant.note}</p>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        </section>

        <section class="card playlist-track-card">
          <div class="playlist-track-head">
            <div>
              <div class="section-kicker subtle">Main sequence</div>
              <h3>Track order</h3>
            </div>
            <div class="playlist-track-meta">{playlistTrackCount} entries</div>
          </div>

          {#if selectedTrack}
            <div class="track-rail">
              <ContextActionRail
                compact
                track={selectedTrack}
                album={selectedTrack.album ? { artist: selectedTrack.artist, title: selectedTrack.album } : null}
                artistName={selectedTrack.artist}
              />
            </div>
          {/if}

          <div class="track-list">
            {#each $activePlaylistItems as item, index}
              <button class="track-row mood-card" on:click={() => (selectedTrack = item.track)}>
                <span class="track-index">{index + 1}</span>
                <span class="track-copy">
                  <span class="track-title">{item.track?.title ?? 'Unknown track'}</span>
                  <span class="track-meta">{item.track?.artist ?? 'Unknown artist'} / {item.track?.album ?? 'No album'}</span>
                </span>
                <span class="track-duration">{formatDuration(item.track?.duration_secs ?? 0)}</span>
              </button>
            {/each}
          </div>
        </section>
      {/if}
    </section>
  </div>
</div>

<style>
  .playlists-page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 18px;
  }

  .playlist-hero,
  .create-form,
  .authorship-card,
  .section-card,
  .playlist-track-card,
  .playlist-list {
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.02), transparent 26%),
      var(--bg-card);
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

  .playlist-hero-row,
  .authorship-head,
  .playlist-track-head {
    display: flex;
    justify-content: space-between;
    gap: 16px;
  }

  .playlist-hero-copy,
  .authorship-head > div:first-child {
    display: grid;
    gap: 8px;
  }

  .playlist-hero-copy h1 {
    font-size: clamp(1.9rem, 4vw, 3rem);
    line-height: 0.95;
  }

  .playlist-hero-copy p,
  .authorship-meta,
  .playlist-track-meta {
    color: var(--text-secondary);
    line-height: 1.7;
  }

  .playlist-hero-actions,
  .authorship-actions,
  .create-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: flex-start;
  }

  .create-form,
  .authorship-card,
  .section-card,
  .playlist-track-card {
    display: grid;
    gap: 14px;
  }

  .create-form label,
  .authorship-grid label,
  .section-form label {
    display: grid;
    gap: 6px;
    font-size: 0.75rem;
    color: var(--text-secondary);
  }

  .textarea {
    min-height: 92px;
    resize: vertical;
  }

  .playlists-layout {
    display: grid;
    grid-template-columns: 320px minmax(0, 1fr);
    gap: 16px;
    min-height: 0;
  }

  .playlist-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
  }

  .playlist-list-scroll,
  .track-list,
  .section-list,
  .variant-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
    overflow-y: auto;
  }

  .playlist-item,
  .track-row {
    width: 100%;
    text-align: left;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 12px;
    background: rgba(255, 255, 255, 0.02);
    display: grid;
    gap: 6px;
    transition: border-color 0.15s, background 0.15s, transform 0.15s;
  }

  .playlist-item:hover,
  .track-row:hover {
    border-color: var(--border-active);
    background: rgba(139, 180, 212, 0.06);
    transform: translateY(-1px);
  }

  .playlist-item.active {
    border-color: rgba(247, 180, 92, 0.35);
    background: rgba(247, 180, 92, 0.07);
  }

  .playlist-item-head,
  .playlist-actions,
  .track-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }

  .playlist-name,
  .track-title,
  .section-item-title,
  .variant-item-title {
    font-weight: 700;
    color: var(--text-primary);
  }

  .playlist-count,
  .track-index,
  .track-duration {
    font-size: 0.74rem;
    color: var(--text-muted);
  }

  .playlist-meta,
  .track-meta,
  .section-item-meta,
  .variant-item-meta {
    font-size: 0.76rem;
    color: var(--text-secondary);
  }

  .playlist-action-link {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }

  .playlist-action-link.danger {
    color: color-mix(in srgb, var(--error) 80%, white);
  }

  .playlist-main,
  .section-grid {
    display: grid;
    gap: 16px;
  }

  .authorship-grid,
  .section-form {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }

  .section-form-span {
    grid-column: 1 / -1;
  }

  .section-item,
  .variant-item {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 12px;
    display: grid;
    gap: 8px;
    background: rgba(255, 255, 255, 0.02);
  }

  .section-item-head,
  .variant-item-head {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }

  .section-item-actions,
  .variant-item-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .btn-small {
    padding: 4px 9px;
    font-size: 0.72rem;
  }

  .section-item-note,
  .variant-item-note {
    color: var(--text-secondary);
    line-height: 1.6;
    font-size: 0.82rem;
  }

  .track-rail {
    padding-bottom: 4px;
  }

  .track-copy {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 3px;
  }

  .track-title,
  .track-meta {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
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

  .empty-state.roomy {
    min-height: 220px;
    align-content: center;
  }

  .empty-title {
    font-weight: 700;
    color: var(--text-primary);
  }

  .empty-body {
    line-height: 1.65;
  }

  @media (max-width: 1120px) {
    .playlists-layout,
    .section-grid,
    .authorship-grid,
    .section-form {
      grid-template-columns: 1fr;
    }
  }
</style>
