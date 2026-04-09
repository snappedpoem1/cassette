<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import { type CrateRecord } from '$lib/stores/rituals';
  import { formatDuration } from '$lib/utils';
  import { api } from '$lib/api/tauri';
  import { queueDuration } from '$lib/queue-ritual';
  import { loadLibrary, tracks } from '$lib/stores/library';
  import {
    clearQueue,
    cutQueueAfterPosition,
    holdQueuePosition,
    loadQueue,
    pinQueuePosition,
    playItemAfterCurrent,
    removeQueueItem,
  } from '$lib/stores/queue';
  import { playbackState, player } from '$lib/stores/player';
  import {
    crates,
    deleteQueueScene,
    liveQueueRitual,
    loadRituals,
    queueScenes,
    saveCrate,
    saveQueueScene,
    saveSessionRecord,
    updateLiveQueueRitual,
    emptyCrateFilter,
  } from '$lib/stores/rituals';
  import { queue } from '$lib/stores/queue';

  onMount(async () => {
    await Promise.all([loadQueue(), loadLibrary(), loadRituals()]);
  });

  let sceneName = '';
  let sceneNote = '';
  let cratePivotId = '';

  $: queueItems = $queue;
  $: currentPosition = $playbackState.queue_position;
  $: currentTrack = $playbackState.current_track;
  $: totalDuration = queueDuration(queueItems);
  $: pinnedCount = $liveQueueRitual.pinnedPositions.length;
  $: heldCount = $liveQueueRitual.heldPositions.length;
  $: currentCratePivot = $crates.find((crate) => crate.id === cratePivotId) ?? null;

  async function playFrom(position: number) {
    if (queueItems.length === 0) {
      return;
    }
    await api.reorderQueue(queueItems.map((item) => item.track_id), position);
    await loadQueue();
  }

  async function removeAt(position: number) {
    await removeQueueItem(position, Math.min(currentPosition, Math.max(0, queueItems.length - 2)));
  }

  async function resumeQueue() {
    if ($playbackState.current_track) {
      await player.toggle();
      return;
    }
    await playFrom(0);
  }

  async function saveCurrentScene() {
    if (!sceneName.trim() || queueItems.length === 0) {
      return;
    }
    await saveQueueScene({
      name: sceneName.trim(),
      note: sceneNote.trim(),
      trackIds: queueItems.map((item) => item.track_id),
      startIndex: currentPosition,
      pinnedPositions: $liveQueueRitual.pinnedPositions,
      heldPositions: $liveQueueRitual.heldPositions,
    });
    sceneName = '';
    sceneNote = '';
  }

  async function restoreScene(sceneId: string) {
    const scene = $queueScenes.find((item) => item.id === sceneId);
    if (!scene) {
      return;
    }
    await api.queueTracks(scene.trackIds, scene.startIndex);
    await updateLiveQueueRitual({
      pinnedPositions: scene.pinnedPositions,
      heldPositions: scene.heldPositions,
      lastPivotLabel: scene.name,
    });
    await loadQueue();
  }

  async function saveQueueAsCrate() {
    if (queueItems.length === 0) {
      return;
    }
    await saveCrate({
      name: sceneName.trim() || 'Queue slice',
      note: sceneNote.trim() || 'Saved from queue sculpting.',
      kind: 'temporary',
      source: 'manual',
      filter: emptyCrateFilter(),
      trackIds: queueItems.map((item) => item.track_id),
    });
  }

  async function saveQueueAsSession() {
    if (queueItems.length === 0) {
      return;
    }
    await saveSessionRecord({
      name: sceneName.trim() || 'Queue memory',
      note: sceneNote.trim(),
      trackIds: queueItems.map((item) => item.track_id),
      reasons: [],
      source: 'queue_scene',
      sourceRefId: null,
      branchOfId: null,
      modeSnapshot: null,
    });
  }

  async function pivotTail(trackIds: number[], label: string) {
    if (queueItems.length === 0) {
      return;
    }
    const prefix = queueItems.slice(0, currentPosition + 1).map((item) => item.track_id);
    const tail = trackIds.filter((trackId) => !prefix.includes(trackId));
    const nextTrackIds = [...prefix, ...tail];
    await api.queueTracks(nextTrackIds, currentPosition);
    await updateLiveQueueRitual({
      pinnedPositions: [],
      heldPositions: [],
      lastPivotLabel: label,
    });
    await loadQueue();
  }

  async function pivotToArtist() {
    if (!currentTrack) {
      return;
    }
    const artistTracks = $tracks
      .filter((track) => track.artist === currentTrack.artist)
      .sort((left, right) => (left.year ?? 0) - (right.year ?? 0) || left.id - right.id)
      .slice(0, 24)
      .map((track) => track.id);
    await pivotTail(artistTracks, `${currentTrack.artist} pivot`);
  }

  async function pivotToAlbum() {
    if (!currentTrack) {
      return;
    }
    const albumTracks = $tracks
      .filter((track) => track.artist === currentTrack.artist && track.album === currentTrack.album)
      .sort(
        (left, right) =>
          (left.disc_number ?? 1) - (right.disc_number ?? 1)
          || (left.track_number ?? 0) - (right.track_number ?? 0)
          || left.id - right.id
      )
      .map((track) => track.id);
    await pivotTail(albumTracks, `${currentTrack.album} pivot`);
  }

  async function pivotToCrate(crate: CrateRecord | null) {
    if (!crate) {
      return;
    }
    await pivotTail(crate.trackIds, `${crate.name} pivot`);
  }
</script>

<svelte:head><title>Queue - Cassette</title></svelte:head>

<div class="queue-page">
  <section class="queue-hero card">
    <div class="section-kicker">Sculpting ritual</div>
    <div class="queue-hero-row">
      <div class="queue-hero-copy">
        <h1>Shape the next run in real time</h1>
        <p>
          Pin what must stay near the front, hold what can wait, cut the tail when the mood turns,
          then save the whole shape as a queue scene worth returning to.
        </p>
      </div>
      <div class="queue-hero-actions">
        <button class="btn btn-primary" on:click={resumeQueue}>
          {$playbackState.current_track ? ($playbackState.is_playing ? 'Pause' : 'Resume') : 'Play from top'}
        </button>
        <button class="btn btn-secondary" on:click={() => goto('/playlists')}>Open playlists</button>
        <button class="btn btn-ghost" on:click={clearQueue} disabled={queueItems.length === 0}>Clear queue</button>
      </div>
    </div>

    <div class="queue-stats">
      <div class="queue-stat">
        <span class="queue-stat-label">Queued tracks</span>
        <strong>{queueItems.length}</strong>
      </div>
      <div class="queue-stat">
        <span class="queue-stat-label">Run time</span>
        <strong>{formatDuration(totalDuration)}</strong>
      </div>
      <div class="queue-stat">
        <span class="queue-stat-label">Pins / holds</span>
        <strong>{pinnedCount} / {heldCount}</strong>
      </div>
      <div class="queue-stat">
        <span class="queue-stat-label">Last pivot</span>
        <strong>{$liveQueueRitual.lastPivotLabel ?? 'None yet'}</strong>
      </div>
    </div>
  </section>

  {#if queueItems.length === 0}
    <section class="card queue-empty">
      <div class="empty-title">Nothing is lined up yet</div>
      <div class="empty-body">
        Start with the collection, a crate, an artist shelf, or a playlist, then come back here when
        the order needs hands-on shaping.
      </div>
      <div class="queue-empty-actions">
        <button class="btn btn-primary" on:click={() => goto('/collection')}>Open collection</button>
        <button class="btn btn-secondary" on:click={() => goto('/crates')}>Open crates</button>
      </div>
    </section>
  {:else}
    <div class="queue-layout">
      <section class="card queue-list-card">
        <div class="queue-list-head">
          <div>
            <div class="section-kicker subtle">Live sequence</div>
            <h2>Queue body</h2>
          </div>
          <div class="queue-list-meta">{queueItems.length} items / current slot {currentPosition + 1}</div>
        </div>

        <div class="queue-list">
          {#each queueItems as item, index}
            {@const isCurrent = index === currentPosition}
            {@const isPinned = $liveQueueRitual.pinnedPositions.includes(index)}
            {@const isHeld = $liveQueueRitual.heldPositions.includes(index)}
            <div class="queue-row" class:is-current={isCurrent}>
              <button class="queue-main" on:click={() => playFrom(index)}>
                <span class="queue-position">{isCurrent ? 'Now' : index + 1}</span>
                <span class="queue-copy">
                  <span class="queue-title">
                    {item.track?.title ?? 'Unavailable track'}
                    {#if isPinned}<span class="queue-chip">Pinned</span>{/if}
                    {#if isHeld}<span class="queue-chip queue-chip-muted">Held</span>{/if}
                  </span>
                  <span class="queue-meta">
                    {item.track?.artist ?? 'Missing metadata'}
                    {#if item.track?.album}
                      {' / '}{item.track.album}
                    {/if}
                  </span>
                </span>
                <span class="queue-duration">{formatDuration(item.track?.duration_secs ?? 0)}</span>
              </button>

              <div class="queue-actions">
                <button class="btn btn-ghost queue-row-btn" on:click={() => playItemAfterCurrent(index)}>After current</button>
                <button class="btn btn-ghost queue-row-btn" on:click={() => pinQueuePosition(index)}>Pin</button>
                <button class="btn btn-ghost queue-row-btn" on:click={() => holdQueuePosition(index)}>Hold</button>
                <button class="btn btn-ghost queue-row-btn" on:click={() => cutQueueAfterPosition(index)}>Cut after this</button>
                <button class="btn btn-ghost queue-row-btn" on:click={() => removeAt(index)}>Cut</button>
              </div>
            </div>
          {/each}
        </div>
      </section>

      <aside class="queue-side">
        <section class="card queue-scenes-card">
          <div class="section-kicker subtle">Queue scenes</div>
          <h3>Save this shape</h3>
          <div class="scene-form">
            <label>
              Scene name
              <input class="input" bind:value={sceneName} placeholder="Late train / red-eye / after midnight" />
            </label>
            <label>
              Scene note
              <textarea class="input textarea" bind:value={sceneNote} placeholder="Why this shape works."></textarea>
            </label>
          </div>
          <div class="queue-scenes-actions">
            <button class="btn btn-primary" on:click={saveCurrentScene}>Save scene</button>
            <button class="btn btn-secondary" on:click={saveQueueAsSession}>Save as session</button>
            <button class="btn btn-ghost" on:click={saveQueueAsCrate}>Save as crate</button>
          </div>

          {#if $queueScenes.length === 0}
            <div class="empty-state compact">
              <div class="empty-title">No saved scenes</div>
              <div class="empty-body">Save one when the queue finally lands in the right shape.</div>
            </div>
          {:else}
            <div class="scene-list">
              {#each $queueScenes as scene}
                <div class="scene-item">
                  <div class="scene-item-head">
                    <div>
                      <div class="scene-item-title">{scene.name}</div>
                      <div class="scene-item-meta">{scene.trackIds.length} tracks / start at {scene.startIndex + 1}</div>
                    </div>
                    <div class="scene-item-actions">
                      <button class="btn btn-ghost btn-small" on:click={() => restoreScene(scene.id)}>Restore</button>
                      <button class="btn btn-ghost btn-small" on:click={() => deleteQueueScene(scene.id)}>Delete</button>
                    </div>
                  </div>
                  {#if scene.note}
                    <p class="scene-item-note">{scene.note}</p>
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
        </section>

        <section class="card queue-pivot-card">
          <div class="section-kicker subtle">Pivots</div>
          <h3>Replace the tail with a new lane</h3>
          <div class="queue-scenes-actions">
            <button class="btn btn-primary" on:click={pivotToArtist} disabled={!currentTrack}>Pivot to artist</button>
            <button class="btn btn-secondary" on:click={pivotToAlbum} disabled={!currentTrack}>Pivot to album</button>
          </div>
          <label>
            Pivot to crate
            <select class="input" bind:value={cratePivotId}>
              <option value="">Choose a crate</option>
              {#each $crates as crate}
                <option value={crate.id}>{crate.name}</option>
              {/each}
            </select>
          </label>
          <button class="btn btn-ghost" on:click={() => pivotToCrate(currentCratePivot)} disabled={!currentCratePivot}>
            Pivot to saved crate
          </button>
        </section>
      </aside>
    </div>
  {/if}
</div>

<style>
  .queue-page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 18px;
  }

  .queue-hero,
  .queue-list-card,
  .queue-scenes-card,
  .queue-pivot-card {
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.02), transparent 24%),
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

  .queue-hero-row,
  .queue-list-head,
  .scene-item-head {
    display: flex;
    justify-content: space-between;
    gap: 16px;
  }

  .queue-hero-copy,
  .queue-list-head > div:first-child {
    display: grid;
    gap: 8px;
  }

  .queue-hero-copy h1 {
    font-size: clamp(1.9rem, 4vw, 3rem);
    line-height: 0.95;
  }

  .queue-hero-copy p,
  .queue-list-meta,
  .scene-item-meta {
    color: var(--text-secondary);
    line-height: 1.7;
  }

  .queue-hero-actions,
  .queue-empty-actions,
  .queue-scenes-actions,
  .scene-item-actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }

  .queue-stats {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
  }

  .queue-stat {
    padding: 12px 14px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: rgba(255, 255, 255, 0.02);
  }

  .queue-stat-label {
    display: block;
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
    margin-bottom: 6px;
  }

  .queue-layout {
    display: grid;
    grid-template-columns: minmax(0, 1.4fr) 380px;
    gap: 16px;
  }

  .queue-list-card,
  .queue-scenes-card,
  .queue-pivot-card,
  .queue-empty {
    padding: 18px;
  }

  .queue-side {
    display: grid;
    gap: 16px;
    align-content: start;
  }

  .queue-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .queue-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 10px;
    align-items: center;
    padding: 12px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: rgba(255, 255, 255, 0.02);
  }

  .queue-row.is-current {
    border-color: color-mix(in srgb, var(--primary) 42%, var(--border));
    background: color-mix(in srgb, var(--primary) 8%, var(--bg-base));
  }

  .queue-main {
    min-width: 0;
    display: grid;
    grid-template-columns: 50px minmax(0, 1fr) auto;
    gap: 12px;
    align-items: center;
    text-align: left;
  }

  .queue-position,
  .queue-duration {
    font-size: 0.72rem;
    color: var(--text-muted);
  }

  .queue-copy {
    min-width: 0;
    display: grid;
    gap: 3px;
  }

  .queue-title {
    color: var(--text-primary);
    font-size: 0.88rem;
    font-weight: 700;
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }

  .queue-meta {
    color: var(--text-secondary);
    font-size: 0.78rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .queue-chip {
    font-size: 0.6rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-accent);
    border: 1px solid rgba(139, 180, 212, 0.28);
    border-radius: 999px;
    padding: 2px 6px;
  }

  .queue-chip-muted {
    color: var(--text-secondary);
    border-color: rgba(255, 255, 255, 0.16);
  }

  .queue-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  .queue-row-btn,
  .btn-small {
    font-size: 0.72rem;
    padding: 4px 9px;
  }

  .scene-form {
    display: grid;
    gap: 10px;
  }

  .scene-form label,
  .queue-pivot-card label {
    display: grid;
    gap: 6px;
    font-size: 0.75rem;
    color: var(--text-secondary);
  }

  .textarea {
    min-height: 92px;
    resize: vertical;
  }

  .scene-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .scene-item {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 12px;
    display: grid;
    gap: 8px;
    background: rgba(255, 255, 255, 0.02);
  }

  .scene-item-title,
  .empty-title {
    color: var(--text-primary);
    font-weight: 700;
  }

  .scene-item-note,
  .empty-body {
    color: var(--text-secondary);
    line-height: 1.6;
  }

  .empty-state {
    border: 1px dashed var(--border);
    border-radius: var(--radius);
    padding: 14px;
    display: grid;
    gap: 6px;
  }

  .empty-state.compact {
    padding: 14px;
  }

  @media (max-width: 1120px) {
    .queue-layout,
    .queue-stats {
      grid-template-columns: 1fr;
    }

    .queue-row {
      grid-template-columns: 1fr;
    }
  }
</style>
