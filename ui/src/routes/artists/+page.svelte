<script lang="ts">
  import { buildArtistClusters, clusterAlbumsForArtist, normalizeArtistKey, type ArtistCluster } from '$lib/artist-clusters';
  import { artists, albums } from '$lib/stores/library';
  import { api } from '$lib/api/tauri';
  import ContextActionRail from '$lib/components/ContextActionRail.svelte';
  import { queueTracks } from '$lib/stores/queue';
  import { formatDuration, coverSrc } from '$lib/utils';
  import type { Album, Track } from '$lib/api/tauri';

  function artistCoverArts(cluster: ArtistCluster, allAlbums: Album[], max = 4): string[] {
    return clusterAlbumsForArtist(allAlbums, cluster)
      .filter((a) => !!a.cover_art_path)
      .slice(0, max)
      .map((a) => coverSrc(a.cover_art_path!))
      .filter((src): src is string => !!src);
  }

  let selectedArtist: ArtistCluster | null = null;
  let artistAlbums: Album[] = [];
  let selectedAlbum: Album | null = null;
  let albumTracks: Track[] = [];

  $: artistClusters = buildArtistClusters($artists);

  async function selectArtist(cluster: ArtistCluster) {
    selectedArtist = cluster;
    selectedAlbum = null;
    albumTracks = [];

    artistAlbums = clusterAlbumsForArtist($albums, cluster);
  }

  async function selectAlbum(album: Album) {
    selectedAlbum = album;
    albumTracks = await api.getAlbumTracks(album.artist, album.title);
  }

  function back() {
    if (selectedAlbum) {
      selectedAlbum = null;
      albumTracks = [];
      return;
    }

    selectedArtist = null;
    artistAlbums = [];
  }

  async function playAlbum(album: Album) {
    const trackList = await api.getAlbumTracks(album.artist, album.title);
    if (trackList.length) {
      await queueTracks(trackList, 0);
    }
  }
</script>

<svelte:head><title>Artists · Cassette</title></svelte:head>

<div class="artists-page">
  <div class="page-header">
    {#if selectedArtist}
      <button class="back-btn" on:click={back}>Back</button>
      <h2>{selectedAlbum ? selectedAlbum.title : selectedArtist.primaryName}</h2>
      {#if selectedAlbum}
        <button class="btn btn-primary" on:click={() => selectedAlbum && playAlbum(selectedAlbum)}>Play</button>
      {/if}
    {:else}
      <h2>Artists</h2>
    {/if}
  </div>

  {#if !selectedArtist}
    {#if artistClusters.length === 0}
      <div class="empty-state">
        <div class="empty-title">No artists yet</div>
        <div class="empty-body">Scan your library to find artists.</div>
      </div>
    {:else}
      <div class="artist-grid">
        {#each artistClusters as cluster}
          {@const arts = artistCoverArts(cluster, $albums)}
          <div
            class="artist-card"
            role="button"
            tabindex="0"
            on:click={() => selectArtist(cluster)}
            on:keydown={(event) => {
              if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault();
                selectArtist(cluster);
              }
            }}
          >
            <div class="artist-mosaic">
              {#if arts.length >= 4}
                <div class="mosaic-grid">
                  {#each arts.slice(0, 4) as src}
                    <img class="mosaic-img" {src} alt="" loading="lazy" />
                  {/each}
                </div>
              {:else if arts.length >= 1}
                <img class="mosaic-single" src={arts[0]} alt="" loading="lazy" />
              {:else}
                <div class="artist-avatar-fallback">{cluster.primaryName[0]?.toUpperCase() ?? '?'}</div>
              {/if}
              <div class="artist-mosaic-overlay"></div>
            </div>
            <div class="artist-card-info">
              <div class="artist-name">{cluster.primaryName}</div>
              <div class="artist-meta">{cluster.albumCount} albums · {cluster.trackCount} tracks</div>
              {#if cluster.aliases.length > 1}
                <div class="artist-variants">{cluster.aliases.length} name variants</div>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  {:else if !selectedAlbum}
    <div style="padding: 0 1rem 0.5rem;">
      <ContextActionRail compact artistName={selectedArtist.primaryName} />
    </div>
    {#if artistAlbums.length === 0}
      <div class="empty-state">
        <div class="empty-title">No albums found</div>
      </div>
    {:else}
      <div class="album-grid">
        {#each artistAlbums as album}
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
              <div class="album-art-placeholder">Art</div>
            {/if}
            <div class="album-info">
              <div class="album-title">{album.title}</div>
              <div class="album-meta">{album.year ?? ''} · {album.track_count} tracks</div>
              {#if album.artist !== selectedArtist.primaryName}
                <div class="album-alias">{album.artist}</div>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  {:else}
    <div style="padding: 0 1rem 0.5rem;">
      <ContextActionRail
        compact
        album={{ artist: selectedAlbum.artist, title: selectedAlbum.title }}
        artistName={selectedArtist.primaryName}
      />
    </div>
    <div class="track-list" style="padding: 8px 1rem;">
      {#each albumTracks as track, i}
        <div class="track-row" role="button" tabindex="0" on:dblclick={() => queueTracks(albumTracks, i)}>
          <span class="track-num">{track.track_number ?? i + 1}</span>
          <div class="track-title">{track.title}</div>
          <div class="track-artist">{normalizeArtistKey(track.artist) !== selectedArtist.key ? track.artist : ''}</div>
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
  font-size: 0.8rem;
  color: var(--text-secondary);
  cursor: pointer;
  background: none;
  border: none;
  padding: 4px 8px;
  border-radius: var(--radius-sm);
  transition: color 0.1s;
}

.back-btn:hover { color: var(--text-primary); }

.artist-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: 16px;
  padding: 1.5rem;
}

.artist-card {
  display: flex;
  flex-direction: column;
  border-radius: var(--radius);
  background: var(--bg-card);
  border: 1px solid var(--border);
  cursor: pointer;
  transition: transform 0.15s, box-shadow 0.15s, border-color 0.15s;
  overflow: hidden;
  text-align: center;
}

.artist-card:hover {
  transform: translateY(-2px);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
  border-color: var(--border-active);
}

.artist-mosaic {
  position: relative;
  width: 100%;
  aspect-ratio: 1;
  overflow: hidden;
  background: var(--bg-active);
}

.mosaic-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  grid-template-rows: 1fr 1fr;
  width: 100%;
  height: 100%;
  gap: 1px;
}

.mosaic-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.mosaic-single {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.artist-avatar-fallback {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 2rem;
  font-weight: 700;
  color: var(--accent-bright);
  background: linear-gradient(135deg, var(--accent-dim), var(--bg-active));
}

.artist-mosaic-overlay {
  position: absolute;
  inset: 0;
  background: linear-gradient(to top, rgba(8, 11, 18, 0.72) 0%, transparent 55%);
  pointer-events: none;
}

.artist-card-info {
  padding: 10px 10px 12px;
}

.artist-name { font-weight: 600; font-size: 0.85rem; word-break: break-word; color: var(--text-primary); }
.artist-meta { font-size: 0.72rem; color: var(--text-muted); margin-top: 2px; }
.artist-variants { font-size: 0.68rem; color: var(--accent-bright); margin-top: 2px; }
.album-alias { margin-top: 2px; font-size: 0.7rem; color: var(--text-muted); }
.track-list { padding: 8px; }
</style>
