<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { addToQueue, loadQueue, queueTracks } from '$lib/stores/queue';
  import { api, type Playlist, type Track } from '$lib/api/tauri';

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

  function setStatus(text: string) {
    message = text;
    dispatch('status', text);
    const clearAfter = setTimeout(() => {
      message = '';
      clearTimeout(clearAfter);
    }, 2200);
  }

  async function withBusy(action: () => Promise<void>) {
    if (busy) {
      return;
    }
    busy = true;
    try {
      await action();
    } finally {
      busy = false;
    }
  }

  async function playTrackNow() {
    if (!track) {
      return;
    }
    await withBusy(async () => {
      await queueTracks([track], 0);
      setStatus('Playing track now');
    });
  }

  async function queueTrackNext() {
    if (!track) {
      return;
    }
    await withBusy(async () => {
      await addToQueue(track);
      setStatus('Queued track');
    });
  }

  async function findTrack() {
    if (!track) {
      return;
    }
    await withBusy(async () => {
      await api.startDownload(track.artist, track.title, track.album || undefined);
      setStatus('Track added to the inbox');
    });
  }

  async function playAlbumNow() {
    if (!album) {
      return;
    }
    await withBusy(async () => {
      const tracks = await api.getAlbumTracks(album.artist, album.title);
      if (tracks.length === 0) {
        setStatus('No album tracks found');
        return;
      }
      await queueTracks(tracks, 0);
      setStatus('Playing album now');
    });
  }

  async function queueAlbum() {
    if (!album) {
      return;
    }
    await withBusy(async () => {
      const tracks = await api.getAlbumTracks(album.artist, album.title);
      if (tracks.length === 0) {
        setStatus('No album tracks found');
        return;
      }
      for (const item of tracks) {
        await api.addToQueue(item.id);
      }
      await loadQueue();
      setStatus('Queued album tracks');
    });
  }

  async function findAlbum() {
    if (!album) {
      return;
    }
    await withBusy(async () => {
      await api.startAlbumDownloads([{ artist: album.artist, title: album.title }]);
      setStatus('Album added to the inbox');
    });
  }

  async function fillArtistGaps() {
    if (!artistName) {
      return;
    }
    await withBusy(async () => {
      await api.startArtistDownloads(artistName);
      setStatus('Artist added to the inbox');
    });
  }

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
    if (!track) {
      return;
    }
    await withBusy(async () => {
      await api.addTrackToPlaylist(playlistId, track.id);
      setStatus('Added to playlist');
      showPlaylistPicker = false;
    });
  }

  $: hasContext = !!track || !!album || !!artistName;
</script>

{#if hasContext}
  <div class="action-rail" class:compact>
    <div class="rail-label">Quick Actions</div>

    <div class="rail-actions">
      {#if track}
        <button class="rail-btn" disabled={busy} on:click={playTrackNow}>Play Track</button>
        <button class="rail-btn" disabled={busy} on:click={queueTrackNext}>Queue Track</button>
        <button class="rail-btn" disabled={busy} on:click={showAddToPlaylist}>+ Playlist</button>
        <button class="rail-btn rail-btn-acquire" disabled={busy} on:click={findTrack}>Find Track</button>
      {/if}

      {#if album}
        <button class="rail-btn" disabled={busy} on:click={playAlbumNow}>Play Album</button>
        <button class="rail-btn" disabled={busy} on:click={queueAlbum}>Queue Album</button>
        <button class="rail-btn rail-btn-acquire" disabled={busy} on:click={findAlbum}>Find Album</button>
      {/if}

      {#if artistName}
        <button class="rail-btn rail-btn-acquire" disabled={busy} on:click={fillArtistGaps}>Fill Artist Gaps</button>
      {/if}
    </div>

    {#if showPlaylistPicker && track}
      <div class="playlist-picker">
        {#if loadingPlaylists}
          <div class="picker-loading">Loading playlists...</div>
        {:else if playlists.length === 0}
          <div class="picker-empty">No playlists yet. Create one in Playlists.</div>
        {:else}
          {#each playlists as pl}
            <button class="picker-item" disabled={busy} on:click={() => addToPlaylist(pl.id)}>
              {pl.name} <span class="picker-count">{pl.track_count} tracks</span>
            </button>
          {/each}
        {/if}
      </div>
    {/if}

    {#if message}
      <div class="rail-status">{message}</div>
    {/if}
  </div>
{/if}

<style>
  .action-rail {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--bg-card) 90%, var(--bg-base));
  }

  .action-rail.compact {
    padding: 8px;
    gap: 6px;
  }

  .rail-label {
    font-size: 0.68rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .rail-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .rail-btn {
    border: 1px solid var(--border);
    background: var(--bg-hover);
    color: var(--text-secondary);
    border-radius: 999px;
    padding: 4px 10px;
    font-size: 0.72rem;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }

  .rail-btn:hover {
    background: var(--bg-active);
    color: var(--text-primary);
  }

  .rail-btn-acquire {
    border-color: rgba(94, 196, 160, 0.35);
    color: var(--status-ok);
  }

  .rail-btn:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .rail-status {
    font-size: 0.72rem;
    color: var(--text-muted);
  }

  .playlist-picker {
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-deep);
    overflow: hidden;
    margin-top: 2px;
  }

  .picker-loading,
  .picker-empty {
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

  .picker-item:first-child {
    border-top: none;
  }

  .picker-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .picker-item:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .picker-count {
    font-size: 0.68rem;
    color: var(--text-muted);
  }
</style>
