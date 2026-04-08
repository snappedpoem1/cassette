<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import type { AcquisitionRequestListItem, CollectionStats, SpotifyAlbumHistory, Track } from '$lib/api/tauri';
  import { api } from '$lib/api/tauri';
  import { albums, tracks, loadLibrary } from '$lib/stores/library';
  import { coverSrc } from '$lib/utils';
  import {
    buildAlbumOwnershipSummary,
    summarizeArtistMissing,
  } from '$lib/ownership';

  let stats: CollectionStats | null = null;
  let missingAlbums: SpotifyAlbumHistory[] = [];
  let requestHistory: AcquisitionRequestListItem[] = [];
  let recentTracks: Track[] = [];
  let loading = true;

  onMount(async () => {
    if ($albums.length === 0 || $tracks.length === 0) {
      await loadLibrary();
    }

    try {
      const [nextStats, nextMissing, nextRequests, nextRecentTracks] = await Promise.all([
        api.getCollectionStats(),
        api.getMissingSpotifyAlbums(120),
        api.listAcquisitionRequests(undefined, 400),
        api.getRecentlyFinalizedTracks(21),
      ]);
      stats = nextStats;
      missingAlbums = nextMissing;
      requestHistory = nextRequests;
      recentTracks = nextRecentTracks;
    } finally {
      loading = false;
    }
  });

  function albumKey(artist: string, title: string): string {
    return `${artist}|||${title}`;
  }

  $: trackBuckets = $tracks.reduce((map, track) => {
    const key = albumKey(track.album_artist, track.album);
    const bucket = map.get(key) ?? [];
    bucket.push(track);
    map.set(key, bucket);
    return map;
  }, new Map<string, Track[]>());

  $: ownership = $albums
    .map((album) => buildAlbumOwnershipSummary(
      album,
      trackBuckets.get(albumKey(album.artist, album.title)) ?? [],
      [],
      requestHistory,
    ));

  $: bestCopyShelf = ownership
    .filter((summary) => summary.losslessTrackCount > 0 || summary.hiResTrackCount > 0)
    .sort((a, b) =>
      b.hiResTrackCount - a.hiResTrackCount ||
      b.losslessTrackCount - a.losslessTrackCount ||
      a.missingMetadataCount - b.missingMetadataCount ||
      a.album.title.localeCompare(b.album.title)
    )
    .slice(0, 8);

  $: archiveHealthShelf = ownership
    .filter((summary) =>
      summary.archiveHealth !== 'strong' ||
      summary.missingMetadataCount > 0 ||
      summary.duplicateSlotCount > 0 ||
      !summary.album.cover_art_path
    )
    .sort((a, b) =>
      archiveScore(b) - archiveScore(a) ||
      b.duplicateSlotCount - a.duplicateSlotCount ||
      b.missingMetadataCount - a.missingMetadataCount
    )
    .slice(0, 8);

  $: editionShelf = ownership
    .filter((summary) => summary.edition.markers.length > 0 || (summary.edition.bucket && summary.edition.bucket !== 'standard'))
    .sort((a, b) =>
      b.edition.markers.length - a.edition.markers.length ||
      (b.album.year ?? 0) - (a.album.year ?? 0) ||
      a.album.title.localeCompare(b.album.title)
    )
    .slice(0, 8);

  $: recentAlbumKeys = new Set(
    recentTracks
      .filter((track) => track.album)
      .map((track) => albumKey(track.album_artist, track.album))
  );

  $: provenanceShelf = ownership
    .filter((summary) => summary.sourceProviders.length > 0 || recentAlbumKeys.has(albumKey(summary.album.artist, summary.album.title)))
    .sort((a, b) =>
      b.sourceProviders.length - a.sourceProviders.length ||
      Number(recentAlbumKeys.has(albumKey(b.album.artist, b.album.title))) - Number(recentAlbumKeys.has(albumKey(a.album.artist, a.album.title))) ||
      b.album.track_count - a.album.track_count
    )
    .slice(0, 8);

  $: artistGapRail = summarizeArtistMissing(
    missingAlbums.map((entry) => ({
      artist: entry.artist,
      album: entry.album,
      play_count: entry.play_count,
    }))
  ).slice(0, 6);

  $: qualityRows = buildRows(stats?.by_quality_tier ?? {});
  $: decadeRows = buildRows(stats?.by_decade ?? {});
  $: maxQualityCount = qualityRows[0]?.count ?? 0;
  $: maxDecadeCount = decadeRows[0]?.count ?? 0;

  function buildRows(record: Record<string, number>) {
    return Object.entries(record)
      .map(([label, count]) => ({ label, count }))
      .filter((row) => row.count > 0)
      .sort((a, b) => b.count - a.count || a.label.localeCompare(b.label));
  }

  function archiveScore(summary: typeof ownership[number]): number {
    let score = 0;
    if (summary.archiveHealth === 'fragile') score += 3;
    if (!summary.album.cover_art_path) score += 2;
    score += summary.duplicateSlotCount * 2;
    score += summary.missingMetadataCount;
    return score;
  }

  function percentOfTotal(count: number): string {
    if (!stats?.total_tracks) return '0%';
    return `${Math.round((count / stats.total_tracks) * 100)}%`;
  }

  function widthPercent(count: number, maxCount: number): string {
    if (!maxCount) return '0%';
    return `${Math.max(8, Math.round((count / maxCount) * 100))}%`;
  }

  function prettyTier(label: string): string {
    return label.replace(/_/g, ' ');
  }
</script>

<svelte:head><title>Collection - Cassette</title></svelte:head>

<div class="collection-page">
  <section class="collection-hero card">
    <div class="section-kicker">Ownership ritual</div>
    <div class="hero-row">
      <div class="hero-copy">
        <h1>Read the shelf before you read the numbers</h1>
        <p>
          Collection answers the collector questions first: where the best copies live, what needs
          care, which editions give the shelf texture, and where the gaps still tug at an artist.
        </p>
      </div>
      <div class="hero-actions">
        <button class="btn btn-primary" on:click={() => goto('/artists')}>Open artists</button>
        <button class="btn btn-secondary" on:click={() => goto('/queue')}>Open queue</button>
      </div>
    </div>

    <div class="hero-stats">
      <div class="hero-stat">
        <span class="stat-label">Tracks</span>
        <strong>{stats?.total_tracks?.toLocaleString() ?? 0}</strong>
      </div>
      <div class="hero-stat">
        <span class="stat-label">Albums</span>
        <strong>{stats?.total_albums?.toLocaleString() ?? 0}</strong>
      </div>
      <div class="hero-stat">
        <span class="stat-label">Lossless</span>
        <strong>{stats?.lossless_count?.toLocaleString() ?? 0}</strong>
      </div>
      <div class="hero-stat">
        <span class="stat-label">Missing artist gaps</span>
        <strong>{artistGapRail.length}</strong>
      </div>
    </div>
  </section>

  {#if loading}
    <section class="card loading-card">
      <div class="spinner"></div>
      <div class="empty-body">Loading ownership view...</div>
    </section>
  {:else}
    <section class="collection-grid">
      <article class="card shelf-panel">
        <div class="panel-head">
          <div>
            <div class="section-kicker">Best copy</div>
            <h2>Where the shelf is strongest</h2>
          </div>
        </div>
        <div class="album-shelf">
          {#each bestCopyShelf as summary}
            <button class="album-card mood-card" on:click={() => goto(`/albums/${summary.album.id}`)}>
              {#if summary.album.cover_art_path}
                <img class="album-art" src={coverSrc(summary.album.cover_art_path)} alt="" loading="lazy" />
              {:else}
                <div class="album-art-ph"></div>
              {/if}
              <span class="album-title">{summary.album.title}</span>
              <span class="album-meta">{summary.album.artist}</span>
              <span class="album-note">{summary.qualityLabel}</span>
            </button>
          {/each}
        </div>
      </article>

      <article class="card shelf-panel">
        <div class="panel-head">
          <div>
            <div class="section-kicker">Archive health</div>
            <h2>What still needs care</h2>
          </div>
        </div>
        <div class="line-stack">
          {#each archiveHealthShelf as summary}
            <button class="line-card mood-card" on:click={() => goto(`/albums/${summary.album.id}`)}>
              <span class="line-copy">
                <span class="line-title">{summary.album.artist} / {summary.album.title}</span>
                <span class="line-meta">{summary.archiveNotes[0] ?? 'Needs a closer look.'}</span>
              </span>
              <span class="line-badge" data-health={summary.archiveHealth}>{summary.archiveHealth}</span>
            </button>
          {/each}
        </div>
      </article>
    </section>

    <section class="collection-grid">
      <article class="card shelf-panel">
        <div class="panel-head">
          <div>
            <div class="section-kicker">Edition visibility</div>
            <h2>Versions that give the shelf texture</h2>
          </div>
        </div>
        <div class="line-stack">
          {#each editionShelf as summary}
            <button class="line-card mood-card" on:click={() => goto(`/albums/${summary.album.id}`)}>
              <span class="line-copy">
                <span class="line-title">{summary.album.artist} / {summary.album.title}</span>
                <span class="line-meta">{summary.edition.markers.join(', ') || prettyTier(summary.edition.bucket ?? 'edition')}</span>
              </span>
              <span class="line-badge">{prettyTier(summary.edition.bucket ?? 'edition')}</span>
            </button>
          {/each}
        </div>
      </article>

      <article class="card shelf-panel">
        <div class="panel-head">
          <div>
            <div class="section-kicker">Provenance</div>
            <h2>Albums with a recorded handoff</h2>
          </div>
        </div>
        <div class="line-stack">
          {#each provenanceShelf as summary}
            <button class="line-card mood-card" on:click={() => goto(`/albums/${summary.album.id}`)}>
              <span class="line-copy">
                <span class="line-title">{summary.album.artist} / {summary.album.title}</span>
                <span class="line-meta">
                  {summary.sourceProviders.length > 0
                    ? `Recorded from ${summary.sourceProviders.join(', ')}`
                    : 'Recently landed in the collection'}
                </span>
              </span>
              <span class="line-badge">{summary.sourceProviders.length > 0 ? 'Recorded' : 'Recent'}</span>
            </button>
          {/each}
        </div>
      </article>
    </section>

    <section class="collection-grid">
      <article class="card shelf-panel">
        <div class="panel-head">
          <div>
            <div class="section-kicker">Missing from artist</div>
            <h2>Where the pull is still strongest</h2>
          </div>
          <button class="panel-link" on:click={() => goto('/artists')}>Open artists</button>
        </div>
        <div class="line-stack">
          {#if artistGapRail.length === 0}
            <div class="panel-empty">No artist gaps are pressing right now.</div>
          {:else}
            {#each artistGapRail as entry}
              <button class="line-card mood-card" on:click={() => goto('/artists')}>
                <span class="line-copy">
                  <span class="line-title">{entry.artist}</span>
                  <span class="line-meta">{entry.missingAlbums} missing albums / {entry.playCount} plays pointing at the gap</span>
                </span>
                <span class="line-badge">{entry.missingAlbums}</span>
              </button>
            {/each}
          {/if}
        </div>
      </article>

      <article class="card shelf-panel">
        <div class="panel-head">
          <div>
            <div class="section-kicker">Charts</div>
            <h2>Subordinate reference</h2>
          </div>
        </div>

        <div class="bar-list">
          {#each qualityRows.slice(0, 5) as row}
            <div class="bar-row">
              <div class="bar-meta">
                <span class="bar-label">{prettyTier(row.label)}</span>
                <span class="bar-value">{row.count.toLocaleString()} / {percentOfTotal(row.count)}</span>
              </div>
              <div class="bar-track"><div class="bar-fill" style:width={widthPercent(row.count, maxQualityCount)}></div></div>
            </div>
          {/each}
        </div>

        <div class="bar-list secondary-bars">
          {#each decadeRows.slice(0, 5) as row}
            <div class="bar-row">
              <div class="bar-meta">
                <span class="bar-label">{row.label}</span>
                <span class="bar-value">{row.count.toLocaleString()} / {percentOfTotal(row.count)}</span>
              </div>
              <div class="bar-track"><div class="bar-fill accent" style:width={widthPercent(row.count, maxDecadeCount)}></div></div>
            </div>
          {/each}
        </div>
      </article>
    </section>
  {/if}
</div>

<style>
  .collection-page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 18px;
  }

  .collection-hero,
  .shelf-panel,
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

  .hero-actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }

  .hero-stats {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
    margin-top: 18px;
  }

  .hero-stat {
    padding: 12px 14px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: var(--bg-base);
  }

  .stat-label {
    display: block;
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
    margin-bottom: 6px;
  }

  .hero-stat strong {
    font-size: 1.1rem;
    color: var(--text-primary);
  }

  .collection-grid {
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

  .panel-link {
    color: var(--text-accent);
    font-size: 0.76rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .album-shelf {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
    gap: 10px;
  }

  .album-card {
    display: grid;
    gap: 6px;
    text-align: left;
  }

  .album-art,
  .album-art-ph {
    width: 100%;
    aspect-ratio: 1;
    border-radius: 12px;
    overflow: hidden;
  }

  .album-art {
    object-fit: cover;
  }

  .album-art-ph {
    background: var(--bg-active);
    border: 1px solid var(--border);
  }

  .album-title,
  .line-title {
    color: var(--text-primary);
    font-size: 0.84rem;
    font-weight: 600;
  }

  .album-meta,
  .album-note,
  .line-meta,
  .bar-value {
    color: var(--text-secondary);
    font-size: 0.75rem;
  }

  .line-stack,
  .bar-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .line-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 11px 12px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: var(--bg-base);
    text-align: left;
  }

  .line-copy {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    align-items: flex-start;
  }

  .line-badge {
    font-size: 0.7rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 4px 8px;
  }

  .line-badge[data-health='fragile'] {
    color: var(--warning);
  }

  .line-badge[data-health='steady'] {
    color: var(--text-accent);
  }

  .bar-row {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .bar-meta {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    align-items: baseline;
  }

  .bar-label {
    color: var(--text-primary);
    font-size: 0.78rem;
  }

  .bar-track {
    width: 100%;
    height: 8px;
    border-radius: 999px;
    background: var(--bg-active);
    overflow: hidden;
  }

  .bar-fill {
    height: 100%;
    border-radius: 999px;
    background: linear-gradient(90deg, var(--primary), rgba(139, 180, 212, 0.5));
  }

  .bar-fill.accent {
    background: linear-gradient(90deg, var(--accent), rgba(247, 180, 92, 0.45));
  }

  .secondary-bars {
    margin-top: 14px;
  }

  .loading-card {
    display: grid;
    justify-items: center;
    gap: 10px;
  }

  @media (max-width: 1040px) {
    .collection-grid,
    .hero-stats {
      grid-template-columns: 1fr;
    }

    .hero-row {
      flex-direction: column;
      align-items: flex-start;
    }
  }
</style>
