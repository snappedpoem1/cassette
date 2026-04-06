<script lang="ts">
  import { onMount } from 'svelte';
  import {
    tracks, albums, artists,
    activeTab, searchQuery, searchResults, isSearching,
    loadLibrary, search,
  } from '$lib/stores/library';
  import { queueTracks } from '$lib/stores/queue';
  import { goto } from '$app/navigation';
  import { formatDuration, formatAudioSpec, coverSrc, debounce, tintFromHex } from '$lib/utils';
  import type { Album, Artist, Track } from '$lib/api/tauri';
  import { api } from '$lib/api/tauri';

  let selectedAlbum: Album | null = null;
  let albumTracks: Track[] = [];
  let loadingAlbumTracks = false;

  const debouncedSearch = debounce((q: string) => search(q), 300);

  let searchInput = '';
  $: debouncedSearch(searchInput);

  async function openAlbum(album: Album) {
    selectedAlbum = album;
    loadingAlbumTracks = true;
    albumTracks = await api.getAlbumTracks(album.artist, album.title);
    loadingAlbumTracks = false;
  }

  function closeAlbum() {
    selectedAlbum = null;
    albumTracks = [];
  }

  async function playAlbum(album: Album) {
    const tracks = await api.getAlbumTracks(album.artist, album.title);
    if (tracks.length) await queueTracks(tracks, 0);
  }

  async function playTrack(trackList: Track[], index: number) {
    await queueTracks(trackList, index);
  }
</script>

<svelte:head><title>Library · Cassette</title></svelte:head>

<div class="library-page">
  <!-- Header -->
  <div class="page-header">
    <h2 style="flex:1">Library</h2>
    <div class="search-wrap">
      <span class="search-icon">🔍</span>
      <input
        class="input search-input"
        type="text"
        placeholder="Search tracks, artists, albums…"
        bind:value={searchInput}
      />
      {#if $isSearching}
        <span class="search-spinner"><div class="spinner" style="width:14px;height:14px;border-width:2px"></div></span>
      {/if}
    </div>
  </div>

  <!-- Search results overlay -->
  {#if searchInput.trim() && $searchResults.length > 0}
    <div class="search-results">
      <div class="sr-label">{$searchResults.length} results for "{searchInput}"</div>
      {#each $searchResults as track, i}
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div class="track-row" on:dblclick={() => playTrack($searchResults, i)}>
          <span class="track-num">{i + 1}</span>
          <div class="track-title">{track.title}</div>
          <div class="track-artist">{track.artist} · {track.album}</div>
          <span class="track-duration">{formatDuration(track.duration_secs)}</span>
          <span class="track-format">{track.format.toUpperCase()}</span>
        </div>
      {/each}
    </div>
  {:else if searchInput.trim() && !$isSearching}
    <div class="empty-state" style="padding:2rem;">
      <div class="empty-icon">🔍</div>
      <div class="empty-title">No results</div>
      <div class="empty-body">Nothing matched "{searchInput}"</div>
    </div>
  {:else}
    <!-- Tabs -->
    <div class="tabs">
      <button class="tab" class:active={$activeTab === 'albums'}  on:click={() => activeTab.set('albums')}>Albums</button>
      <button class="tab" class:active={$activeTab === 'tracks'}  on:click={() => activeTab.set('tracks')}>Tracks</button>
      <button class="tab" class:active={$activeTab === 'artists'} on:click={() => activeTab.set('artists')}>Artists</button>
    </div>

    <!-- Albums tab -->
    {#if $activeTab === 'albums'}
      {#if selectedAlbum}
        <!-- Album detail view -->
        {@const detailTint = tintFromHex(selectedAlbum.dominant_color_hex)}
        <div class="album-detail">
          <!-- Blurred backdrop -->
          {#if selectedAlbum.cover_art_path}
            <div
              class="album-detail-backdrop"
              style="background-image:url({coverSrc(selectedAlbum.cover_art_path)});background-color:{detailTint.bg};"
            ></div>
          {:else}
            <div class="album-detail-backdrop" style="background:{detailTint.bg};"></div>
          {/if}
          <div class="album-detail-header">
            <button class="back-btn" on:click={closeAlbum}>← Albums</button>
            <div class="album-detail-art">
              {#if selectedAlbum.cover_art_path}
                <img src={coverSrc(selectedAlbum.cover_art_path)} alt="cover" />
              {:else}
                <div class="album-detail-art-ph">💿</div>
              {/if}
            </div>
            <div class="album-detail-info">
              <h1>{selectedAlbum.title}</h1>
              <div class="album-detail-artist">{selectedAlbum.artist}</div>
              <div class="album-detail-meta">
                {#if selectedAlbum.year}{selectedAlbum.year} · {/if}{selectedAlbum.track_count} tracks
              </div>
              <button class="btn btn-primary" style="margin-top:12px;" on:click={() => playAlbum(selectedAlbum!)}>
                ▶ Play Album
              </button>
            </div>
          </div>

          {#if loadingAlbumTracks}
            <div class="empty-state"><div class="spinner"></div></div>
          {:else}
            <div class="track-list">
              {#each albumTracks as track, i}
                <!-- svelte-ignore a11y-no-static-element-interactions -->
                <div class="track-row" on:dblclick={() => playTrack(albumTracks, i)}>
                  <span class="track-num">{track.track_number ?? i + 1}</span>
                  <div class="track-title">{track.title}</div>
                  <div class="track-artist">{track.artist !== selectedAlbum?.artist ? track.artist : ''}</div>
                  <span class="track-duration">{formatDuration(track.duration_secs)}</span>
                  <span class="track-format">{track.format.toUpperCase()}</span>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {:else}
        <!-- Album grid -->
        {#if $albums.length === 0}
          <div class="empty-state">
            <div class="empty-icon">💿</div>
            <div class="empty-title">No albums yet</div>
            <div class="empty-body">Add a library root in Settings and scan to import your music.</div>
          </div>
        {:else}
          <div class="album-grid">
            {#each $albums as album}
              {@const tint = tintFromHex(album.dominant_color_hex)}
              <!-- svelte-ignore a11y-no-static-element-interactions -->
              <div
                class="album-card"
                role="button"
                tabindex="0"
                on:click={() => openAlbum(album)}
                on:dblclick={() => playAlbum(album)}
                on:keydown={(event) => {
                  if (event.key === 'Enter' || event.key === ' ') {
                    event.preventDefault();
                    openAlbum(album);
                  }
                }}
              >
                {#if album.cover_art_path}
                  <img class="album-art" src={coverSrc(album.cover_art_path)} alt="cover" />
                {:else}
                  <div class="album-art-placeholder">💿</div>
                {/if}
                <div class="album-info" style="background:{tint.bg};">
                  <div class="album-title" style="color:{tint.titleColor};">{album.title}</div>
                  <div class="album-artist">{album.artist}</div>
                  <div class="album-meta">{album.year ?? ''}{album.year && album.track_count ? ' · ' : ''}{album.track_count} tracks</div>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      {/if}

    <!-- Tracks tab -->
    {:else if $activeTab === 'tracks'}
      {#if $tracks.length === 0}
        <div class="empty-state">
          <div class="empty-icon">🎵</div>
          <div class="empty-title">No tracks yet</div>
          <div class="empty-body">Scan your library from Settings.</div>
        </div>
      {:else}
        <div class="track-list">
          {#each $tracks as track, i}
            <!-- svelte-ignore a11y-no-static-element-interactions -->
            <div class="track-row" on:dblclick={() => playTrack($tracks, i)}>
              <span class="track-num">{i + 1}</span>
              <div class="track-title">{track.title}</div>
              <div class="track-artist">{track.artist}</div>
              <span class="track-duration">{formatDuration(track.duration_secs)}</span>
              <span class="track-format">{track.format.toUpperCase()}</span>
            </div>
          {/each}
        </div>
      {/if}

    <!-- Artists tab -->
    {:else}
      {#if $artists.length === 0}
        <div class="empty-state">
          <div class="empty-icon">🎤</div>
          <div class="empty-title">No artists yet</div>
        </div>
      {:else}
        <div class="artist-list">
          {#each $artists as artist}
            <!-- svelte-ignore a11y-no-static-element-interactions -->
            <div
              class="artist-row"
              role="button"
              tabindex="0"
              on:click={() => goto('/artists')}
              on:keydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); goto('/artists'); } }}
            >
              <div class="artist-avatar">{artist.name[0]?.toUpperCase()}</div>
              <div class="artist-info">
                <div class="artist-name">{artist.name}</div>
                <div class="artist-meta">{artist.album_count} albums · {artist.track_count} tracks</div>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  {/if}
</div>

<style>
.library-page { display: flex; flex-direction: column; min-height: 100%; }

.page-header { background: linear-gradient(to bottom, var(--bg-base) 70%, transparent); }

.search-wrap {
  position: relative; display: flex; align-items: center; flex: 1; max-width: 360px;
}
.search-icon { position: absolute; left: 10px; font-size: 0.85rem; pointer-events: none; }
.search-input { padding-left: 32px !important; }
.search-spinner { position: absolute; right: 10px; }

.search-results { padding: 0 1rem 1rem; }
.sr-label { font-size: 0.8rem; color: var(--text-muted); padding: 8px 16px 4px; }

.track-list { padding: 8px; }

.album-detail { padding: 1.5rem; position: relative; overflow: hidden; }
.album-detail-backdrop {
  position: absolute; inset: 0; z-index: 0;
  background-size: cover; background-position: center;
  filter: blur(60px) brightness(0.35) saturate(1.4);
  transform: scale(1.1);
  pointer-events: none;
}
.album-detail > *:not(.album-detail-backdrop) { position: relative; z-index: 1; }
.album-detail-header {
  display: flex; align-items: flex-end; gap: 20px;
  margin-bottom: 24px;
}
.back-btn {
  position: absolute; top: 1rem; left: 1rem;
  font-size: 0.85rem; color: var(--text-secondary);
  cursor: pointer; background: none; border: none;
  transition: color 0.1s;
}
.back-btn:hover { color: var(--text-primary); }
.album-detail-art {
  width: 160px; height: 160px; flex-shrink: 0;
  border-radius: var(--radius); overflow: hidden;
  box-shadow: 0 8px 32px rgba(0,0,0,0.5);
}
.album-detail-art img { width: 100%; height: 100%; object-fit: cover; }
.album-detail-art-ph {
  width: 100%; height: 100%;
  background: var(--bg-active);
  display: flex; align-items: center; justify-content: center; font-size: 3rem;
}
.album-detail-info h1 { font-size: 1.6rem; }
.album-detail-artist { color: var(--text-secondary); font-size: 1rem; margin-top: 4px; }
.album-detail-meta   { color: var(--text-muted); font-size: 0.85rem; margin-top: 4px; }

.artist-list { padding: 8px 1rem; display: flex; flex-direction: column; gap: 4px; }
.artist-row {
  display: flex; align-items: center; gap: 14px;
  padding: 10px 12px; border-radius: var(--radius-sm);
  transition: background 0.1s; cursor: pointer;
}
.artist-row:hover { background: var(--bg-hover); }
.artist-avatar {
  width: 40px; height: 40px; border-radius: 50%;
  background: var(--bg-active);
  display: flex; align-items: center; justify-content: center;
  font-size: 1rem; font-weight: 700; color: var(--accent-bright); flex-shrink: 0;
}
.artist-name { font-weight: 600; font-size: 0.9rem; }
.artist-meta { font-size: 0.75rem; color: var(--text-secondary); margin-top: 2px; }
</style>
