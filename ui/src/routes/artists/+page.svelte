<script lang="ts">
  import { artists } from '$lib/stores/library';
  import { api } from '$lib/api/tauri';
  import { queueTracks } from '$lib/stores/queue';
  import { formatDuration, coverSrc } from '$lib/utils';
  import type { Artist, Album, Track } from '$lib/api/tauri';

  let selectedArtist: Artist | null = null;
  let artistAlbums: Album[] = [];
  let selectedAlbum: Album | null = null;
  let albumTracks: Track[] = [];

  async function selectArtist(artist: Artist) {
    selectedArtist = artist;
    selectedAlbum = null;
    albumTracks = [];
    const all = await api.getAlbums();
    artistAlbums = all.filter((a) => a.artist === artist.name || a.artist.includes(artist.name));
  }

  async function selectAlbum(album: Album) {
    selectedAlbum = album;
    albumTracks = await api.getAlbumTracks(album.artist, album.title);
  }

  function back() {
    if (selectedAlbum) { selectedAlbum = null; albumTracks = []; }
    else { selectedArtist = null; artistAlbums = []; }
  }

  async function playAlbum(album: Album) {
    const tracks = await api.getAlbumTracks(album.artist, album.title);
    if (tracks.length) await queueTracks(tracks, 0);
  }
</script>

<svelte:head><title>Artists · Cassette</title></svelte:head>

<div class="artists-page">
  <div class="page-header">
    {#if selectedArtist}
      <button class="back-btn" on:click={back}>←</button>
      <h2>{selectedAlbum ? selectedAlbum.title : selectedArtist.name}</h2>
      {#if selectedAlbum}
        <button class="btn btn-primary" on:click={() => selectedAlbum && playAlbum(selectedAlbum)}>▶ Play</button>
      {/if}
    {:else}
      <h2>Artists</h2>
    {/if}
  </div>

  {#if !selectedArtist}
    <!-- Artist grid -->
    {#if $artists.length === 0}
      <div class="empty-state">
        <div class="empty-icon">🎤</div>
        <div class="empty-title">No artists yet</div>
        <div class="empty-body">Scan your library to find artists.</div>
      </div>
    {:else}
      <div class="artist-grid">
        {#each $artists as artist}
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div
            class="artist-card"
            role="button"
            tabindex="0"
            on:click={() => selectArtist(artist)}
            on:keydown={(event) => {
              if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault();
                selectArtist(artist);
              }
            }}
          >
            <div class="artist-avatar">{artist.name[0]?.toUpperCase() ?? '?'}</div>
            <div class="artist-name">{artist.name}</div>
            <div class="artist-meta">{artist.album_count} albums · {artist.track_count} tracks</div>
          </div>
        {/each}
      </div>
    {/if}

  {:else if !selectedAlbum}
    <!-- Artist albums -->
    {#if artistAlbums.length === 0}
      <div class="empty-state">
        <div class="empty-icon">💿</div>
        <div class="empty-title">No albums found</div>
      </div>
    {:else}
      <div class="album-grid">
        {#each artistAlbums as album}
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div
            class="album-card"
            role="button"
            tabindex="0"
            on:click={() => selectAlbum(album)}
            on:dblclick={() => playAlbum(album)}
            on:keydown={(event) => {
              if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault();
                selectAlbum(album);
              }
            }}
          >
            {#if album.cover_art_path}
              <img class="album-art" src={coverSrc(album.cover_art_path)} alt="cover" />
            {:else}
              <div class="album-art-placeholder">💿</div>
            {/if}
            <div class="album-info">
              <div class="album-title">{album.title}</div>
              <div class="album-meta">{album.year ?? ''} · {album.track_count} tracks</div>
            </div>
          </div>
        {/each}
      </div>
    {/if}

  {:else}
    <!-- Album tracks -->
    <div class="track-list" style="padding: 8px 1rem;">
      {#each albumTracks as track, i}
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div class="track-row" on:dblclick={() => queueTracks(albumTracks, i)}>
          <span class="track-num">{track.track_number ?? i + 1}</span>
          <div class="track-title">{track.title}</div>
          <div class="track-artist">{track.artist !== selectedArtist?.name ? track.artist : ''}</div>
          <span class="track-duration">{formatDuration(track.duration_secs)}</span>
          <span class="track-format">{track.format.toUpperCase()}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
.artists-page { display: flex; flex-direction: column; min-height: 100%; }

.back-btn {
  font-size: 1.1rem; color: var(--text-secondary); cursor: pointer;
  background: none; border: none; padding: 4px 8px; border-radius: var(--radius-sm);
  transition: color 0.1s;
}
.back-btn:hover { color: var(--text-primary); }

.artist-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: 16px; padding: 1.5rem;
}
.artist-card {
  display: flex; flex-direction: column; align-items: center; gap: 10px;
  padding: 20px 12px; border-radius: var(--radius);
  background: var(--bg-card); border: 1px solid var(--border);
  cursor: pointer; transition: transform 0.15s, box-shadow 0.15s;
  text-align: center;
}
.artist-card:hover { transform: translateY(-2px); box-shadow: 0 8px 24px rgba(0,0,0,0.4); border-color: var(--border-active); }
.artist-avatar {
  width: 64px; height: 64px; border-radius: 50%;
  background: linear-gradient(135deg, var(--accent-dim), var(--bg-active));
  display: flex; align-items: center; justify-content: center;
  font-size: 1.5rem; font-weight: 700; color: var(--accent-bright);
}
.artist-name { font-weight: 600; font-size: 0.85rem; word-break: break-word; }
.artist-meta { font-size: 0.72rem; color: var(--text-muted); }

.track-list { padding: 8px; }
</style>
