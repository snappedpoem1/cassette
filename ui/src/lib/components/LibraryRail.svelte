<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import type { Album, Artist, Track } from '$lib/api/tauri';
  import { albums, artists, tracks, activeTab, loadLibrary, libraryLoadError } from '$lib/stores/library';
  import { addToQueue, queueTracks } from '$lib/stores/queue';
  import { coverSrc, formatDuration } from '$lib/utils';

  type PreviewSelection =
    | { kind: 'album'; album: Album }
    | { kind: 'track'; track: Track }
    | { kind: 'artist'; artist: Artist }
    | null;

  let filterQuery = '';
  let previewSelection: PreviewSelection = null;

  onMount(async () => {
    if ($albums.length === 0 && $tracks.length === 0 && $artists.length === 0) {
      await loadLibrary();
    }
  });

  $: normalizedQuery = filterQuery.trim().toLowerCase();
  $: filteredAlbums = $albums
    .filter((album) =>
      !normalizedQuery ||
      `${album.artist} ${album.title} ${album.year ?? ''}`.toLowerCase().includes(normalizedQuery)
    )
    .slice(0, 36);
  $: filteredTracks = $tracks
    .filter((track) =>
      !normalizedQuery ||
      `${track.artist} ${track.album} ${track.title}`.toLowerCase().includes(normalizedQuery)
    )
    .slice(0, 48);
  $: filteredArtists = $artists
    .filter((artist) => !normalizedQuery || artist.name.toLowerCase().includes(normalizedQuery))
    .slice(0, 36);

  $: {
    if ($activeTab === 'albums' && !previewSelection && filteredAlbums.length > 0) {
      previewSelection = { kind: 'album', album: filteredAlbums[0] };
    } else if ($activeTab === 'tracks' && !previewSelection && filteredTracks.length > 0) {
      previewSelection = { kind: 'track', track: filteredTracks[0] };
    } else if ($activeTab === 'artists' && !previewSelection && filteredArtists.length > 0) {
      previewSelection = { kind: 'artist', artist: filteredArtists[0] };
    }
  }

  $: if ($activeTab === 'albums' && previewSelection?.kind === 'album') {
    previewSelection = filteredAlbums.find((album) => album.id === previewSelection.album.id)
      ? previewSelection
      : filteredAlbums[0]
        ? { kind: 'album', album: filteredAlbums[0] }
        : null;
  }

  $: if ($activeTab === 'tracks' && previewSelection?.kind === 'track') {
    previewSelection = filteredTracks.find((track) => track.id === previewSelection.track.id)
      ? previewSelection
      : filteredTracks[0]
        ? { kind: 'track', track: filteredTracks[0] }
        : null;
  }

  $: if ($activeTab === 'artists' && previewSelection?.kind === 'artist') {
    previewSelection = filteredArtists.find((artist) => artist.id === previewSelection.artist.id)
      ? previewSelection
      : filteredArtists[0]
        ? { kind: 'artist', artist: filteredArtists[0] }
        : null;
  }

  function selectAlbum(album: Album): void {
    previewSelection = { kind: 'album', album };
  }

  function selectTrack(track: Track): void {
    previewSelection = { kind: 'track', track };
  }

  function selectArtist(artist: Artist): void {
    previewSelection = { kind: 'artist', artist };
  }

  async function playAlbum(album: Album): Promise<void> {
    const albumTracks = $tracks
      .filter((track) => (track.album_artist || track.artist) === album.artist && track.album === album.title)
      .sort(
        (a, b) =>
          (a.disc_number ?? 1) - (b.disc_number ?? 1) ||
          (a.track_number ?? 0) - (b.track_number ?? 0) ||
          a.path.localeCompare(b.path)
      );
    if (albumTracks.length > 0) {
      await queueTracks(albumTracks, 0);
    }
  }

  async function playTrack(track: Track): Promise<void> {
    await queueTracks([track], 0);
  }

  async function queueTrack(track: Track): Promise<void> {
    await addToQueue(track);
  }

  function albumForTrack(track: Track): Album | null {
    if (!track.album) {
      return null;
    }

    return (
      $albums.find(
        (album) =>
          album.title === track.album &&
          album.artist === (track.album_artist || track.artist)
      ) ?? null
    );
  }

  function coverForTrack(track: Track): string | null {
    return track.cover_art_path ? coverSrc(track.cover_art_path) : null;
  }

  function coverForAlbum(album: Album): string | null {
    return album.cover_art_path ? coverSrc(album.cover_art_path) : null;
  }

  function previewCover(): string | null {
    if (previewSelection?.kind === 'album') {
      return coverForAlbum(previewSelection.album);
    }
    if (previewSelection?.kind === 'track') {
      return coverForTrack(previewSelection.track);
    }
    return null;
  }
</script>

<div class="library-rail">
  <div class="rail-head">
    <div class="rail-kicker">Browser rail</div>
    <input
      class="input rail-search"
      type="text"
      placeholder="Filter collection"
      bind:value={filterQuery}
    />
  </div>

  <div class="rail-tabs" role="tablist" aria-label="Library rail modes">
    <button class="rail-tab" class:active={$activeTab === 'albums'} on:click={() => activeTab.set('albums')}>
      Albums
    </button>
    <button class="rail-tab" class:active={$activeTab === 'tracks'} on:click={() => activeTab.set('tracks')}>
      Tracks
    </button>
    <button class="rail-tab" class:active={$activeTab === 'artists'} on:click={() => activeTab.set('artists')}>
      Artists
    </button>
  </div>

  {#if $libraryLoadError}
    <div class="rail-error">{$libraryLoadError}</div>
  {/if}

  <div class="rail-list">
    {#if $activeTab === 'albums'}
      {#if filteredAlbums.length === 0}
        <div class="rail-empty">No albums match this filter yet.</div>
      {:else}
        {#each filteredAlbums as album}
          <button
            class="rail-row"
            class:active={previewSelection?.kind === 'album' && previewSelection.album.id === album.id}
            on:click={() => selectAlbum(album)}
            on:dblclick={() => goto(`/albums/${album.id}`)}
          >
            <span class="rail-row-title">{album.title}</span>
            <span class="rail-row-meta">{album.artist}{#if album.year} / {album.year}{/if}</span>
          </button>
        {/each}
      {/if}
    {:else if $activeTab === 'tracks'}
      {#if filteredTracks.length === 0}
        <div class="rail-empty">No tracks match this filter yet.</div>
      {:else}
        {#each filteredTracks as track}
          <button
            class="rail-row"
            class:active={previewSelection?.kind === 'track' && previewSelection.track.id === track.id}
            on:click={() => selectTrack(track)}
            on:dblclick={() => playTrack(track)}
          >
            <span class="rail-row-title">{track.title}</span>
            <span class="rail-row-meta">{track.artist}{#if track.album} / {track.album}{/if}</span>
          </button>
        {/each}
      {/if}
    {:else}
      {#if filteredArtists.length === 0}
        <div class="rail-empty">No artists match this filter yet.</div>
      {:else}
        {#each filteredArtists as artist}
          <button
            class="rail-row"
            class:active={previewSelection?.kind === 'artist' && previewSelection.artist.id === artist.id}
            on:click={() => selectArtist(artist)}
            on:dblclick={() => goto('/artists')}
          >
            <span class="rail-row-title">{artist.name}</span>
            <span class="rail-row-meta">{artist.album_count} albums / {artist.track_count} tracks</span>
          </button>
        {/each}
      {/if}
    {/if}
  </div>

  <div class="rail-preview">
    <div class="rail-preview-head">
      <span>Preview</span>
    </div>

    {#if previewSelection?.kind === 'album'}
      <div class="rail-preview-body">
        {#if previewCover()}
          <img class="rail-preview-art" src={previewCover()!} alt="" />
        {:else}
          <div class="rail-preview-art ph">Art</div>
        {/if}
        <div class="rail-preview-copy">
          <div class="rail-preview-title">{previewSelection.album.title}</div>
          <div class="rail-preview-meta">{previewSelection.album.artist}</div>
          <div class="rail-preview-meta">
            {previewSelection.album.track_count} tracks{#if previewSelection.album.year} / {previewSelection.album.year}{/if}
          </div>
        </div>
        <div class="rail-preview-actions">
          <button class="btn btn-primary" on:click={() => goto(`/albums/${previewSelection.album.id}`)}>Open</button>
          <button class="btn btn-secondary" on:click={() => playAlbum(previewSelection.album)}>Play</button>
        </div>
      </div>
    {:else if previewSelection?.kind === 'track'}
      <div class="rail-preview-body">
        {#if previewCover()}
          <img class="rail-preview-art" src={previewCover()!} alt="" />
        {:else}
          <div class="rail-preview-art ph">Art</div>
        {/if}
        <div class="rail-preview-copy">
          <div class="rail-preview-title">{previewSelection.track.title}</div>
          <div class="rail-preview-meta">{previewSelection.track.artist}</div>
          <div class="rail-preview-meta">
            {previewSelection.track.album || 'Single'} / {formatDuration(previewSelection.track.duration_secs)}
          </div>
        </div>
        <div class="rail-preview-actions">
          <button class="btn btn-primary" on:click={() => playTrack(previewSelection.track)}>Play</button>
          <button class="btn btn-secondary" on:click={() => queueTrack(previewSelection.track)}>Queue</button>
          {#if albumForTrack(previewSelection.track)}
            <button class="btn btn-ghost" on:click={() => goto(`/albums/${albumForTrack(previewSelection.track)!.id}`)}>
              Album
            </button>
          {/if}
        </div>
      </div>
    {:else if previewSelection?.kind === 'artist'}
      <div class="rail-preview-body artist-preview">
        <div class="rail-preview-art ph artist-mark">{previewSelection.artist.name[0]?.toUpperCase() ?? '?'}</div>
        <div class="rail-preview-copy">
          <div class="rail-preview-title">{previewSelection.artist.name}</div>
          <div class="rail-preview-meta">{previewSelection.artist.album_count} albums</div>
          <div class="rail-preview-meta">{previewSelection.artist.track_count} tracks</div>
        </div>
        <div class="rail-preview-actions">
          <button class="btn btn-primary" on:click={() => goto('/artists')}>Open</button>
        </div>
      </div>
    {:else}
      <div class="rail-empty">Pick something from the rail to keep its details in view.</div>
    {/if}
  </div>
</div>

<style>
  .library-rail {
    display: grid;
    grid-template-rows: auto auto auto minmax(0, 1fr) auto;
    height: 100%;
    min-height: 0;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.015), transparent 20%),
      rgba(7, 10, 16, 0.96);
  }

  .rail-head {
    display: grid;
    gap: 8px;
    padding: 12px 12px 10px;
    border-bottom: 1px solid var(--border-dim);
  }

  .rail-kicker {
    font-size: 0.64rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--accent-bright);
    font-weight: 700;
  }

  .rail-search {
    font-size: 0.8rem;
  }

  .rail-tabs {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    padding: 0 8px;
    border-bottom: 1px solid var(--border-dim);
  }

  .rail-tab {
    padding: 10px 8px;
    border-bottom: 2px solid transparent;
    color: var(--text-muted);
    font-size: 0.72rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .rail-tab.active {
    color: var(--text-accent);
    border-bottom-color: rgba(var(--mood-accent-rgb), 0.9);
  }

  .rail-error {
    margin: 8px;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    border: 1px solid rgba(255, 143, 143, 0.25);
    background: rgba(120, 24, 24, 0.14);
    color: var(--status-error, #ffb4b4);
    font-size: 0.74rem;
  }

  .rail-list {
    overflow: auto;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-height: 0;
  }

  .rail-row {
    display: grid;
    gap: 2px;
    padding: 10px 10px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    text-align: left;
    color: var(--text-secondary);
    background: transparent;
  }

  .rail-row:hover,
  .rail-row.active {
    background: rgba(var(--mood-accent-rgb), 0.08);
    border-color: rgba(var(--mood-accent-rgb), 0.16);
    color: var(--text-primary);
  }

  .rail-row-title {
    font-size: 0.82rem;
    font-weight: 600;
    color: inherit;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .rail-row-meta {
    font-size: 0.72rem;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .rail-preview {
    border-top: 1px solid var(--border-dim);
    background: rgba(6, 8, 16, 0.82);
  }

  .rail-preview-head {
    padding: 8px 12px;
    font-size: 0.64rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }

  .rail-preview-body {
    display: grid;
    gap: 10px;
    padding: 0 12px 12px;
  }

  .rail-preview-art {
    width: 100%;
    aspect-ratio: 1;
    border-radius: var(--radius);
    object-fit: cover;
    background: var(--bg-card);
    border: 1px solid var(--border);
  }

  .rail-preview-art.ph {
    display: grid;
    place-items: center;
    color: var(--text-muted);
  }

  .rail-preview-copy {
    display: grid;
    gap: 2px;
  }

  .rail-preview-title {
    font-size: 0.9rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  .rail-preview-meta {
    font-size: 0.76rem;
    color: var(--text-secondary);
  }

  .rail-preview-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .artist-mark {
    font-size: 2rem;
    font-weight: 700;
    color: var(--accent-bright);
  }

  .rail-empty {
    padding: 12px;
    color: var(--text-secondary);
    font-size: 0.78rem;
    line-height: 1.6;
  }
</style>
