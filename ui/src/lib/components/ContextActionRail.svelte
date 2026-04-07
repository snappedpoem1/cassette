<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { addToQueue, loadQueue, queueTracks } from '$lib/stores/queue';
  import { api, type Track } from '$lib/api/tauri';

  export let track: Track | null = null;
  export let album: { artist: string; title: string } | null = null;
  export let artistName: string | null = null;
  export let compact = false;

  const dispatch = createEventDispatcher<{ status: string }>();

  let busy = false;
  let message = '';

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

  async function acquireTrack() {
    if (!track) {
      return;
    }
    await withBusy(async () => {
      await api.startDownload(track.artist, track.title, track.album || undefined);
      setStatus('Submitted track acquisition');
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

  async function acquireAlbum() {
    if (!album) {
      return;
    }
    await withBusy(async () => {
      await api.startAlbumDownloads([{ artist: album.artist, title: album.title }]);
      setStatus('Submitted album acquisition');
    });
  }

  async function acquireArtist() {
    if (!artistName) {
      return;
    }
    await withBusy(async () => {
      await api.startArtistDownloads(artistName);
      setStatus('Submitted artist acquisition');
    });
  }

  $: hasContext = !!track || !!album || !!artistName;
</script>

{#if hasContext}
  <div class="action-rail" class:compact>
    <div class="rail-label">Context Actions</div>

    <div class="rail-actions">
      {#if track}
        <button class="rail-btn" disabled={busy} on:click={playTrackNow}>Play Track</button>
        <button class="rail-btn" disabled={busy} on:click={queueTrackNext}>Queue Track</button>
        <button class="rail-btn rail-btn-acquire" disabled={busy} on:click={acquireTrack}>Acquire Track</button>
      {/if}

      {#if album}
        <button class="rail-btn" disabled={busy} on:click={playAlbumNow}>Play Album</button>
        <button class="rail-btn" disabled={busy} on:click={queueAlbum}>Queue Album</button>
        <button class="rail-btn rail-btn-acquire" disabled={busy} on:click={acquireAlbum}>Acquire Album</button>
      {/if}

      {#if artistName}
        <button class="rail-btn rail-btn-acquire" disabled={busy} on:click={acquireArtist}>Acquire Artist</button>
      {/if}
    </div>

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
</style>
