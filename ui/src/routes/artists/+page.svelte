<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import { buildArtistClusters, clusterAlbumsForArtist, type ArtistCluster } from '$lib/artist-clusters';
  import type { AcquisitionRequestListItem, SpotifyAlbumHistory, Track } from '$lib/api/tauri';
  import { api } from '$lib/api/tauri';
  import { albums, artists, tracks, loadLibrary } from '$lib/stores/library';
  import { queueTracks } from '$lib/stores/queue';
  import { coverSrc } from '$lib/utils';
  import {
    buildAlbumOwnershipSummary,
    missingAlbumsForArtist,
    normalizeAlbumFamily,
    normalizeArtistKey,
    relatedVersionsForArtist,
    summarizeArtistMissing,
  } from '$lib/ownership';

  function artistCoverArts(cluster: ArtistCluster, max = 4): string[] {
    return clusterAlbumsForArtist($albums, cluster)
      .filter((album) => !!album.cover_art_path)
      .slice(0, max)
      .map((album) => coverSrc(album.cover_art_path!))
      .filter((src): src is string => !!src);
  }

  let loading = true;
  let selectedArtist: ArtistCluster | null = null;
  let missingAlbums: SpotifyAlbumHistory[] = [];
  let requestHistory: AcquisitionRequestListItem[] = [];

  onMount(async () => {
    if ($albums.length === 0 || $artists.length === 0 || $tracks.length === 0) {
      await loadLibrary();
    }
    try {
      const [nextMissing, nextRequests] = await Promise.all([
        api.getMissingSpotifyAlbums(160),
        api.listAcquisitionRequests(undefined, 400),
      ]);
      missingAlbums = nextMissing;
      requestHistory = nextRequests;
    } catch {
      missingAlbums = [];
      requestHistory = [];
    } finally {
      loading = false;
    }
  });

  $: artistClusters = buildArtistClusters($artists);
  $: missingSummary = summarizeArtistMissing(
    missingAlbums.map((entry) => ({
      artist: entry.artist,
      album: entry.album,
      play_count: entry.play_count,
    }))
  );
  $: missingByArtist = missingSummary.reduce((map, entry) => {
    map.set(normalizeArtistKey(entry.artist), entry);
    return map;
  }, new Map<string, { artist: string; missingAlbums: number; playCount: number }>());

  $: selectedAlbums = selectedArtist ? clusterAlbumsForArtist($albums, selectedArtist) : [];
  $: selectedAlbumSummaries = selectedAlbums.map((album) =>
    buildAlbumOwnershipSummary(
      album,
      $tracks.filter((track) => track.album_artist === album.artist && track.album === album.title),
      [],
      requestHistory,
    )
  );
  $: selectedMissingAlbums = selectedArtist
    ? missingAlbumsForArtist(
        missingAlbums.map((entry) => ({
          artist: entry.artist,
          album: entry.album,
          play_count: entry.play_count,
        })),
        selectedArtist.primaryName
      )
    : [];
  $: selectedVersionFamilies = selectedArtist
    ? buildVersionFamilies(relatedVersionsForArtist($albums, selectedArtist.primaryName))
    : [];

  function buildVersionFamilies(artistAlbums: typeof $albums) {
    const families = new Map<string, typeof $albums>();
    for (const album of artistAlbums) {
      const key = normalizeAlbumFamily(album.title);
      const bucket = families.get(key) ?? [];
      bucket.push(album);
      families.set(key, bucket);
    }
    return [...families.entries()]
      .filter(([, bucket]) => bucket.length > 1)
      .map(([family, bucket]) => ({
        family,
        albums: [...bucket].sort((a, b) => (a.year ?? 0) - (b.year ?? 0) || a.title.localeCompare(b.title)),
      }))
      .sort((a, b) => b.albums.length - a.albums.length || a.family.localeCompare(b.family));
  }

  function selectArtist(cluster: ArtistCluster) {
    selectedArtist = cluster;
  }

  function clearArtist() {
    selectedArtist = null;
  }

  async function playArtist(cluster: ArtistCluster) {
    const artistTracks = $tracks
      .filter((track) => normalizeArtistKey(track.album_artist || track.artist) === cluster.key)
      .sort((a, b) => a.album.localeCompare(b.album) || (a.disc_number ?? 1) - (b.disc_number ?? 1) || (a.track_number ?? 0) - (b.track_number ?? 0));
    if (artistTracks.length > 0) {
      await queueTracks(artistTracks, 0);
    }
  }

  function openAlbum(albumId: number) {
    void goto(`/albums/${albumId}`);
  }
</script>

<svelte:head><title>Artists - Cassette</title></svelte:head>

<div class="artists-page">
  {#if !selectedArtist}
    <section class="artist-hero card">
      <div class="section-kicker">Rediscovery ritual</div>
      <div class="hero-row">
        <div class="hero-copy">
          <h1>Open the collection by artist, not by utility</h1>
          <p>
            Artists should feel like doors back into your own memory: what is already in hand,
            which records still tug at you, and where alternate versions start changing the story.
          </p>
        </div>
      </div>
    </section>

    {#if loading}
      <section class="card loading-card">
        <div class="spinner"></div>
        <div class="empty-body">Loading artist view...</div>
      </section>
    {:else if artistClusters.length === 0}
      <section class="card loading-card">
        <div class="empty-title">No artists yet</div>
        <div class="empty-body">Scan your library and artist clusters will appear here.</div>
      </section>
    {:else}
      <section class="artist-grid">
        {#each artistClusters as cluster}
          {@const arts = artistCoverArts(cluster)}
          {@const gap = missingByArtist.get(cluster.key)}
          <button class="artist-card card mood-card" on:click={() => selectArtist(cluster)}>
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
            </div>
            <div class="artist-copy">
              <div class="artist-name">{cluster.primaryName}</div>
              <div class="artist-meta">{cluster.albumCount} albums / {cluster.trackCount} tracks</div>
              {#if gap}
                <div class="artist-gap">{gap.missingAlbums} missing albums / {gap.playCount} plays pointing at them</div>
              {/if}
            </div>
          </button>
        {/each}
      </section>
    {/if}
  {:else}
    <section class="artist-detail-hero card">
      <button class="back-link" on:click={clearArtist}>Back to artists</button>
      <div class="section-kicker">Rediscovery ritual</div>
      <div class="hero-row">
        <div class="hero-copy">
          <h1>{selectedArtist.primaryName}</h1>
          <div class="artist-subhead">
            {selectedArtist.albumCount} albums / {selectedArtist.trackCount} tracks
            {#if selectedMissingAlbums.length > 0}
              {' / '}{selectedMissingAlbums.length} still missing
            {/if}
          </div>
          <p>
            Read the artist as a shelf, a gap map, and a version family. The point here is not to
            browse everything at once. It is to spot what deserves another listen or another search.
          </p>
          <div class="hero-actions">
            <button class="btn btn-primary" on:click={() => playArtist(selectedArtist!)}>Play artist</button>
            <button class="btn btn-secondary" on:click={() => goto('/collection')}>Back to collection</button>
          </div>
        </div>
      </div>
    </section>

    <section class="artist-columns">
      <article class="card artist-panel">
        <div class="panel-head">
          <div>
            <div class="section-kicker">On the shelf</div>
            <h2>Owned albums</h2>
          </div>
        </div>
        <div class="album-list">
          {#each selectedAlbumSummaries as summary}
            <button class="album-line mood-card" on:click={() => openAlbum(summary.album.id)}>
              <span class="album-line-copy">
                <span class="album-line-title">{summary.album.title}</span>
                <span class="album-line-meta">{summary.qualityLabel}{#if summary.edition.markers.length > 0} / {summary.edition.markers.slice(0, 2).join(', ')}{/if}</span>
              </span>
              <span class="album-line-year">{summary.album.year ?? '-'}</span>
            </button>
          {/each}
        </div>
      </article>

      <article class="card artist-panel">
        <div class="panel-head">
          <div>
            <div class="section-kicker">Missing from artist</div>
            <h2>Albums still tugging at the shelf</h2>
          </div>
        </div>
        <div class="album-list">
          {#if selectedMissingAlbums.length === 0}
            <div class="panel-empty">No missing albums are calling attention here right now.</div>
          {:else}
            {#each selectedMissingAlbums as missing}
              <div class="album-line static-line">
                <span class="album-line-copy">
                  <span class="album-line-title">{missing.album}</span>
                  <span class="album-line-meta">{missing.play_count} plays in history / still missing locally</span>
                </span>
              </div>
            {/each}
          {/if}
        </div>
      </article>
    </section>

    <section class="card artist-panel">
      <div class="panel-head">
        <div>
          <div class="section-kicker">Related versions</div>
          <h2>Families on the shelf</h2>
        </div>
      </div>
      <div class="family-list">
        {#if selectedVersionFamilies.length === 0}
          <div class="panel-empty">No version families are visible for this artist yet.</div>
        {:else}
          {#each selectedVersionFamilies as family}
            <div class="family-card">
              <div class="family-title">{family.albums[0].title}</div>
              <div class="family-albums">
                {#each family.albums as album}
                  <button class="family-album mood-card" on:click={() => openAlbum(album.id)}>
                    <span>{album.title}</span>
                    <span>{album.year ?? '-'}</span>
                  </button>
                {/each}
              </div>
            </div>
          {/each}
        {/if}
      </div>
    </section>
  {/if}
</div>

<style>
  .artists-page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 18px;
  }

  .artist-hero,
  .artist-detail-hero,
  .artist-panel,
  .artist-card,
  .loading-card {
    padding: 20px;
  }

  .section-kicker {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--accent-bright);
    font-weight: 700;
  }

  .hero-row {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 18px;
    margin-top: 10px;
  }

  .hero-copy {
    display: grid;
    gap: 10px;
    max-width: 60ch;
  }

  .hero-copy h1 {
    font-size: clamp(2rem, 4vw, 3rem);
    line-height: 0.96;
  }

  .hero-copy p,
  .panel-empty {
    color: var(--text-secondary);
    font-size: 0.86rem;
    line-height: 1.7;
  }

  .artist-subhead,
  .artist-meta,
  .artist-gap,
  .album-line-meta {
    color: var(--text-secondary);
    font-size: 0.78rem;
  }

  .artist-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(190px, 1fr));
    gap: 12px;
  }

  .artist-card {
    display: grid;
    gap: 12px;
    text-align: left;
  }

  .artist-mosaic {
    width: 100%;
    aspect-ratio: 1;
    border-radius: 14px;
    overflow: hidden;
    background: var(--bg-active);
  }

  .mosaic-grid {
    width: 100%;
    height: 100%;
    display: grid;
    grid-template-columns: 1fr 1fr;
    grid-template-rows: 1fr 1fr;
    gap: 1px;
  }

  .mosaic-img,
  .mosaic-single {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .artist-avatar-fallback {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    font-size: 2rem;
    font-weight: 700;
    color: var(--accent-bright);
  }

  .artist-copy {
    display: grid;
    gap: 4px;
  }

  .artist-name,
  .album-line-title,
  .family-title {
    color: var(--text-primary);
    font-size: 0.92rem;
    font-weight: 700;
  }

  .back-link {
    width: fit-content;
    color: var(--text-secondary);
    font-size: 0.8rem;
    letter-spacing: 0.04em;
  }

  .hero-actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
    margin-top: 4px;
  }

  .artist-columns {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 14px;
  }

  .panel-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 14px;
  }

  .album-list,
  .family-list,
  .family-albums {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .album-line,
  .family-card,
  .family-album {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-base);
  }

  .album-line,
  .family-album {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    text-align: left;
    padding: 11px 12px;
  }

  .static-line {
    cursor: default;
  }

  .album-line-copy {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    align-items: flex-start;
  }

  .album-line-year {
    color: var(--text-muted);
    font-size: 0.74rem;
  }

  .family-card {
    padding: 12px;
    display: grid;
    gap: 10px;
  }

  .family-title {
    text-transform: capitalize;
  }

  .family-album span:last-child {
    color: var(--text-muted);
    font-size: 0.74rem;
  }

  .loading-card {
    display: grid;
    justify-items: center;
    gap: 10px;
  }

  @media (max-width: 920px) {
    .artist-columns {
      grid-template-columns: 1fr;
    }
  }
</style>
