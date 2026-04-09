<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import { onMount } from 'svelte';
  import type { AcquisitionRequestListItem, TrackIdentityContext } from '$lib/api/tauri';
  import { api } from '$lib/api/tauri';
  import { albums, tracks, loadLibrary } from '$lib/stores/library';
  import { queueTracks } from '$lib/stores/queue';
  import { coverSrc, formatDuration } from '$lib/utils';
  import {
    buildAlbumOwnershipSummary,
    compareTracksForBestCopy,
    qualityRank,
    relatedVersionsForArtist,
  } from '$lib/ownership';

  let loading = true;
  let requestHistory: AcquisitionRequestListItem[] = [];
  let identities = new Map<number, TrackIdentityContext | null>();

  $: albumId = Number($page.params.albumId);
  $: album = $albums.find((entry) => entry.id === albumId) ?? null;
  $: albumTracks = album
    ? $tracks
        .filter((track) => track.album_artist === album.artist && track.album === album.title)
        .sort((a, b) => (a.disc_number ?? 1) - (b.disc_number ?? 1) || (a.track_number ?? 0) - (b.track_number ?? 0) || a.path.localeCompare(b.path))
    : [];
  $: albumIdentityList = albumTracks.map((track) => identities.get(track.id) ?? null);
  $: ownership = album ? buildAlbumOwnershipSummary(album, albumTracks, albumIdentityList, requestHistory) : null;
  $: relatedVersions = album ? relatedVersionsForArtist($albums, album.artist, album.title, album.id).slice(0, 6) : [];
  $: groupedProviders = ownership?.sourceProviders.join(', ') ?? '';

  onMount(async () => {
    if ($albums.length === 0 || $tracks.length === 0) {
      await loadLibrary();
    }
    try {
      requestHistory = await api.listAcquisitionRequests(undefined, 400);
    } catch {
      requestHistory = [];
    }
    await loadIdentityContext();
    loading = false;
  });

  async function loadIdentityContext() {
    if (!albumTracks.length) {
      identities = new Map();
      return;
    }

    const next = new Map<number, TrackIdentityContext | null>();
    const results = await Promise.all(
      albumTracks.map(async (track) => {
        try {
          return [track.id, await api.getTrackIdentityContext(track.id)] as const;
        } catch {
          return [track.id, null] as const;
        }
      })
    );
    for (const [trackId, identity] of results) {
      next.set(trackId, identity);
    }
    identities = next;
  }

  async function playAlbum() {
    if (!albumTracks.length) {
      return;
    }
    await queueTracks(albumTracks, 0);
  }

  async function playTrackSlot(startIndex: number) {
    await queueTracks(albumTracks, startIndex);
  }

  function editionLabel(bucket: string | null): string {
    if (!bucket) return 'Standard';
    return bucket.replace(/_/g, ' ');
  }

  function qualityChip(trackId: number): string {
    const track = albumTracks.find((entry) => entry.id === trackId);
    if (!track) return 'Unknown';
    if (qualityRank(track) >= 5) return 'Hi-Res';
    if (qualityRank(track) >= 4) return 'Lossless';
    if (qualityRank(track) >= 3) return 'Strong lossy';
    return 'Standard';
  }
</script>

<svelte:head><title>{album ? `${album.title} - Cassette` : 'Album - Cassette'}</title></svelte:head>

<div class="album-page">
  {#if loading}
    <section class="card album-loading">
      <div class="spinner"></div>
      <div class="empty-body">Loading album ritual...</div>
    </section>
  {:else if !album || !ownership}
    <section class="card album-loading">
      <div class="empty-title">Album not found</div>
      <div class="empty-body">This edition is not in the local collection right now.</div>
      <button class="btn btn-primary" on:click={() => goto('/collection')}>Back to collection</button>
    </section>
  {:else}
    <section class="album-hero card">
      <button class="back-link" on:click={() => goto('/collection')}>Back to collection</button>

      <div class="album-hero-grid">
        <div class="album-art-shell">
          {#if album.cover_art_path}
            <img class="album-art" src={coverSrc(album.cover_art_path)} alt={`Cover art for ${album.title}`} />
          {:else}
            <div class="album-art-ph">No Art</div>
          {/if}
        </div>

        <div class="album-copy">
          <div class="section-kicker">Edition ritual</div>
          <h1>{album.title}</h1>
          <div class="album-subhead">{album.artist}{#if album.year} / {album.year}{/if} / {album.track_count} tracks</div>
          <p>
            Best-copy cues, edition context, and archive notes sit together here so you can read an
            album as something you own, not just something you can play.
          </p>

          <div class="album-chips">
            <span class="album-chip album-chip-strong">{ownership.qualityLabel}</span>
            <span class="album-chip">{editionLabel(ownership.edition.bucket)}</span>
            {#if ownership.edition.markers.length > 0}
              <span class="album-chip">{ownership.edition.markers.slice(0, 2).join(', ')}</span>
            {/if}
            {#if groupedProviders}
              <span class="album-chip">Provenance: {groupedProviders}</span>
            {/if}
          </div>

          <div class="album-actions">
            <button class="btn btn-primary" on:click={playAlbum}>Play album</button>
            <button class="btn btn-secondary" on:click={() => goto('/artists')}>Open artist</button>
          </div>
        </div>
      </div>

      <div class="album-stats">
        <div class="album-stat">
          <span class="stat-label">Best-copy slots</span>
          <strong>{ownership.bestTrackCount}</strong>
        </div>
        <div class="album-stat">
          <span class="stat-label">Duplicate slots</span>
          <strong>{ownership.duplicateSlotCount}</strong>
        </div>
        <div class="album-stat">
          <span class="stat-label">Lossless tracks</span>
          <strong>{ownership.losslessTrackCount}</strong>
        </div>
        <div class="album-stat">
          <span class="stat-label">Thin metadata</span>
          <strong>{ownership.missingMetadataCount}</strong>
        </div>
      </div>
    </section>

    <section class="album-columns">
      <article class="card album-panel">
        <div class="panel-head">
          <div>
            <div class="section-kicker">Best copy</div>
            <h2>Track slots</h2>
          </div>
        </div>

        <div class="slot-list">
          {#each ownership.slots as slot}
            {@const startIndex = albumTracks.findIndex((track) => track.id === slot.bestTrack.id)}
            <div class="slot-card">
              <button class="slot-main" on:click={() => playTrackSlot(startIndex)}>
                <span class="slot-index">
                  {slot.discNumber > 1 ? `${slot.discNumber}.${slot.trackNumber || '-'}` : slot.trackNumber || '-'}
                </span>
                <span class="slot-copy">
                  <span class="slot-title">{slot.title}</span>
                  <span class="slot-meta">{formatDuration(slot.bestTrack.duration_secs)} / {qualityChip(slot.bestTrack.id)}</span>
                </span>
                {#if slot.tracks.length > 1}
                  <span class="slot-badge">{slot.tracks.length} copies</span>
                {/if}
              </button>

              <div class="slot-version-list">
                {#each [...slot.tracks].sort(compareTracksForBestCopy) as version}
                  <div class="slot-version" class:is-best={version.id === slot.bestTrack.id}>
                    <span>{version.format.toUpperCase()}</span>
                    <span>{qualityChip(version.id)}</span>
                    <span>{version.bit_depth ? `${version.bit_depth}-bit` : version.bitrate_kbps ? `${version.bitrate_kbps}kbps` : 'audio'}</span>
                    <span>{version.id === slot.bestTrack.id ? 'Best copy' : 'Alt copy'}</span>
                  </div>
                {/each}
              </div>
            </div>
          {/each}
        </div>
      </article>

      <article class="card album-panel">
        <div class="panel-head">
          <div>
            <div class="section-kicker">Archive health</div>
            <h2>Notes on this edition</h2>
          </div>
        </div>

        <div class="archive-health" data-health={ownership.archiveHealth}>
          <strong>{ownership.archiveHealth}</strong>
          <span>{ownership.archiveNotes.length > 0 ? ownership.archiveNotes[0] : 'No immediate archive issues detected.'}</span>
        </div>

        <div class="note-list">
          {#if ownership.archiveNotes.length === 0}
            <div class="panel-empty">Cassette has enough metadata and copy quality here to treat this album as steady on the shelf.</div>
          {:else}
            {#each ownership.archiveNotes as note}
              <div class="note-line">{note}</div>
            {/each}
          {/if}
        </div>

        <div class="panel-subhead">
          <div class="section-kicker">Related versions</div>
          <h3>Same family nearby</h3>
        </div>
        <div class="related-list">
          {#if relatedVersions.length === 0}
            <div class="panel-empty">No related versions of this album are on the shelf yet.</div>
          {:else}
            {#each relatedVersions as related}
              <button class="related-card" on:click={() => goto(`/albums/${related.id}`)}>
                <span class="related-title">{related.title}</span>
                <span class="related-meta">{related.year ?? '-'} / {related.track_count} tracks</span>
              </button>
            {/each}
          {/if}
        </div>
      </article>
    </section>
  {/if}
</div>

<style>
  .album-page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 18px;
  }

  .album-loading,
  .album-panel,
  .album-hero {
    padding: 20px;
  }

  .album-loading {
    display: grid;
    justify-items: center;
    gap: 10px;
  }

  .back-link {
    width: fit-content;
    color: var(--text-secondary);
    font-size: 0.8rem;
    letter-spacing: 0.04em;
  }

  .album-hero-grid {
    display: grid;
    grid-template-columns: 260px minmax(0, 1fr);
    gap: 22px;
    margin-top: 10px;
  }

  .album-art-shell {
    width: 100%;
    max-width: 260px;
  }

  .album-art,
  .album-art-ph {
    width: 100%;
    aspect-ratio: 1;
    border-radius: 16px;
    overflow: hidden;
  }

  .album-art {
    object-fit: cover;
    box-shadow: 0 14px 30px rgba(0, 0, 0, 0.35);
  }

  .album-art-ph {
    background: var(--bg-active);
    border: 1px solid var(--border);
    display: grid;
    place-items: center;
    color: var(--text-muted);
  }

  .album-copy {
    display: grid;
    align-content: end;
    gap: 10px;
  }

  .section-kicker {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--accent-bright);
    font-weight: 700;
  }

  .album-copy h1 {
    font-size: clamp(2rem, 4vw, 3.1rem);
    line-height: 0.96;
  }

  .album-subhead,
  .slot-meta,
  .related-meta {
    color: var(--text-secondary);
    font-size: 0.82rem;
  }

  .album-copy p,
  .panel-empty {
    color: var(--text-secondary);
    font-size: 0.84rem;
    line-height: 1.7;
  }

  .album-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .album-chip {
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 5px 10px;
    font-size: 0.7rem;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .album-chip-strong {
    color: var(--text-accent);
    background: color-mix(in srgb, var(--primary) 12%, transparent);
    border-color: color-mix(in srgb, var(--primary) 30%, var(--border));
  }

  .album-actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
    margin-top: 4px;
  }

  .album-stats {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
    margin-top: 16px;
  }

  .album-stat {
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

  .album-stat strong {
    color: var(--text-primary);
    font-size: 1.1rem;
  }

  .album-columns {
    display: grid;
    grid-template-columns: minmax(0, 1.3fr) minmax(300px, 0.7fr);
    gap: 14px;
  }

  .panel-head,
  .panel-subhead {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 12px;
  }

  .slot-list,
  .note-list,
  .related-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .slot-card,
  .related-card,
  .note-line {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-base);
  }

  .slot-main,
  .related-card {
    width: 100%;
    padding: 12px;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    gap: 12px;
    align-items: center;
    text-align: left;
  }

  .slot-index,
  .slot-badge {
    font-size: 0.72rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .slot-copy {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .slot-title,
  .related-title {
    color: var(--text-primary);
    font-size: 0.88rem;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .slot-version-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 0 12px 12px;
  }

  .slot-version {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
    font-size: 0.74rem;
    color: var(--text-secondary);
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--bg-card) 70%, transparent);
  }

  .slot-version.is-best {
    color: var(--text-primary);
    border: 1px solid color-mix(in srgb, var(--primary) 28%, var(--border));
  }

  .archive-health {
    display: grid;
    gap: 4px;
    padding: 12px;
    border-radius: var(--radius);
    margin-bottom: 10px;
    border: 1px solid var(--border);
  }

  .archive-health[data-health='strong'] {
    background: color-mix(in srgb, var(--success) 10%, var(--bg-base));
  }

  .archive-health[data-health='steady'] {
    background: color-mix(in srgb, var(--primary) 10%, var(--bg-base));
  }

  .archive-health[data-health='fragile'] {
    background: color-mix(in srgb, var(--warning) 12%, var(--bg-base));
  }

  .archive-health strong {
    font-size: 0.9rem;
    text-transform: capitalize;
    color: var(--text-primary);
  }

  .note-line {
    padding: 10px 12px;
    font-size: 0.8rem;
    color: var(--text-secondary);
  }

  .related-card {
    grid-template-columns: minmax(0, 1fr);
    gap: 2px;
  }

  @media (max-width: 1040px) {
    .album-hero-grid,
    .album-columns {
      grid-template-columns: 1fr;
    }

    .album-stats {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  @media (max-width: 720px) {
    .album-stats,
    .slot-version {
      grid-template-columns: 1fr;
    }

    .slot-main,
    .related-card {
      grid-template-columns: 1fr;
    }
  }
</style>
