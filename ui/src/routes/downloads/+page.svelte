<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    downloadJobs, metadataSearchResults, artistDiscography, isSearchingMetadata,
    loadDownloadJobs, searchMetadata, loadDiscography, startJobsPoll, stopJobsPoll,
  } from '$lib/stores/downloads';
  import { api } from '$lib/api/tauri';
  import { debounce } from '$lib/utils';
  import type { DownloadAlbumResult } from '$lib/api/tauri';

  let searchInput = '';
  let discogArtist: string | null = null;
  let queueNotice: string | null = null;

  const debouncedSearch = debounce((q: string) => {
    if (q.trim()) searchMetadata(q);
  }, 400);

  $: debouncedSearch(searchInput);

  onMount(() => {
    loadDownloadJobs();
    startJobsPoll();
  });
  onDestroy(stopJobsPoll);

  async function downloadAlbum(album: DownloadAlbumResult) {
    await api.startDownload(album.artist, album.title);
    await loadDownloadJobs();
  }

  async function openDiscog(artist: string, mbid?: string | null) {
    discogArtist = artist;
    await loadDiscography(artist, mbid ?? undefined);
  }

  async function queueDiscography() {
    if (!$artistDiscography) return;
    const report = await api.startDiscographyDownloads(
      $artistDiscography.artist.name,
      $artistDiscography.artist.mbid ?? undefined,
      false,
      false,
      false,
      50
    );
    queueNotice = `${report.queued} queued, ${report.skipped} skipped (${report.scope}).`;
    await loadDownloadJobs();
  }

  const statusColors: Record<string, string> = {
    Queued: 'badge-muted',
    Searching: 'badge-warning',
    Downloading: 'badge-accent',
    Verifying: 'badge-warning',
    Done: 'badge-success',
    Failed: 'badge-error',
  };
</script>

<svelte:head><title>Downloads · Cassette</title></svelte:head>

<div class="downloads-page">
  <div class="page-header">
    <h2 style="flex:1">Downloads</h2>
  </div>

  <!-- Search -->
  <div class="dl-search-section">
    <input
      class="input"
      type="text"
      placeholder="Search for an artist or album to download…"
      bind:value={searchInput}
      style="max-width:480px;"
    />
    {#if $isSearchingMetadata}
      <div class="spinner" style="width:16px;height:16px;margin-left:10px;"></div>
    {/if}
  </div>

  {#if queueNotice}
    <div class="dl-notice">{queueNotice}</div>
  {/if}

  <!-- Search results -->
  {#if $metadataSearchResults && searchInput.trim()}
    <div class="dl-results">
      {#if $metadataSearchResults.artists.length > 0}
        <div class="dl-section-label">Artists</div>
        <div class="dl-artist-list">
          {#each $metadataSearchResults.artists as artist}
            <!-- svelte-ignore a11y-no-static-element-interactions -->
            <div
              class="dl-artist-row"
              role="button"
              tabindex="0"
              on:click={() => openDiscog(artist.name, artist.mbid)}
              on:keydown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  openDiscog(artist.name, artist.mbid);
                }
              }}
            >
              <div class="dl-artist-name">{artist.name}</div>
              {#if artist.disambiguation}<div class="dl-artist-meta">{artist.disambiguation}</div>{/if}
            </div>
          {/each}
        </div>
      {/if}

      {#if $metadataSearchResults.albums.length > 0}
        <div class="dl-section-label">Albums</div>
        <div class="dl-album-list">
          {#each $metadataSearchResults.albums as album}
            <div class="dl-album-row">
              <div class="dl-album-info">
                <div class="dl-album-title">{album.title}</div>
                <div class="dl-album-meta">{album.artist} · {album.year ?? '?'} · {album.release_type ?? 'Album'}</div>
              </div>
              <button class="btn btn-primary" style="font-size:0.8rem;padding:5px 12px;" on:click={() => downloadAlbum(album)}>
                ⬇ Download
              </button>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}

  <!-- Discography drilldown -->
  {#if $artistDiscography && discogArtist}
    <div class="dl-discog">
      <div class="dl-discog-header">
        <div class="dl-section-label" style="margin:0">{$artistDiscography.artist.name} — Discography</div>
        <button class="btn btn-primary" style="font-size:0.78rem;padding:5px 12px;" on:click={queueDiscography}>
          Queue Discography
        </button>
      </div>
      {#each $artistDiscography.albums as album}
        <div class="dl-album-row">
          <div class="dl-album-info">
            <div class="dl-album-title">{album.title}</div>
            <div class="dl-album-meta">{album.year ?? '?'} · {album.release_type ?? 'Album'} · {album.track_count ?? '?'} tracks</div>
          </div>
          <button class="btn btn-primary" style="font-size:0.8rem;padding:5px 12px;" on:click={() => downloadAlbum(album)}>
            ⬇ Download
          </button>
        </div>
      {/each}
    </div>
  {/if}

  <!-- Active jobs -->
  <div class="dl-jobs-section">
    <div class="dl-section-label" style="padding: 0 1.5rem;">
      Active Jobs {#if $downloadJobs.length > 0}({$downloadJobs.length}){/if}
    </div>

    {#if $downloadJobs.length === 0}
      <div class="empty-state" style="padding:2rem;">
        <div class="empty-icon">⬇️</div>
        <div class="empty-title">No active downloads</div>
        <div class="empty-body">Search for music above to start downloading.</div>
      </div>
    {:else}
      <div class="job-list">
        {#each $downloadJobs as job}
          <div class="job-row">
            <div class="job-info">
              <div class="job-title">{job.title}</div>
              <div class="job-artist">{job.artist}{job.album ? ` · ${job.album}` : ''}</div>
            </div>
            <div class="job-right">
              {#if job.status === 'Downloading'}
                <div class="job-progress">
                  <div class="seek-bar" style="width:120px;">
                    <div class="seek-fill" style="width:{job.progress * 100}%"></div>
                  </div>
                  <span class="job-pct">{Math.round(job.progress * 100)}%</span>
                </div>
              {/if}
              <span class="badge {statusColors[job.status] ?? 'badge-muted'}">{job.status}</span>
              {#if job.provider}<span class="job-provider">{job.provider}</span>{/if}
            </div>
          </div>
          {#if job.error}
            <div class="job-error">{job.error}</div>
          {/if}
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
.downloads-page { display: flex; flex-direction: column; min-height: 100%; }

.dl-search-section { display: flex; align-items: center; padding: 0 1.5rem 1rem; }
.dl-notice {
  margin: 0 1.5rem 0.75rem;
  padding: 8px 12px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-active);
  background: color-mix(in srgb, var(--accent) 12%, var(--bg-card));
  color: var(--text-primary);
  font-size: 0.8rem;
}

.dl-results, .dl-discog { padding: 0 1.5rem 1rem; }
.dl-section-label { font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.07em; color: var(--text-muted); font-weight: 600; margin-bottom: 8px; margin-top: 12px; }
.dl-discog-header { display: flex; align-items: center; justify-content: space-between; margin: 12px 0 8px; }

.dl-artist-list { display: flex; flex-direction: column; gap: 4px; margin-bottom: 8px; }
.dl-artist-row {
  display: flex; align-items: baseline; gap: 10px;
  padding: 8px 12px; border-radius: var(--radius-sm); cursor: pointer;
  background: var(--bg-card); border: 1px solid var(--border);
  transition: border-color 0.1s;
}
.dl-artist-row:hover { border-color: var(--border-active); }
.dl-artist-name { font-weight: 600; }
.dl-artist-meta { font-size: 0.8rem; color: var(--text-secondary); }

.dl-album-list { display: flex; flex-direction: column; gap: 6px; }
.dl-album-row {
  display: flex; align-items: center; gap: 12px;
  padding: 10px 12px; border-radius: var(--radius-sm);
  background: var(--bg-card); border: 1px solid var(--border);
}
.dl-album-info { flex: 1; overflow: hidden; }
.dl-album-title { font-weight: 600; font-size: 0.9rem; }
.dl-album-meta  { font-size: 0.75rem; color: var(--text-secondary); margin-top: 2px; }

.dl-jobs-section { flex: 1; }
.job-list { padding: 8px 1.5rem; display: flex; flex-direction: column; gap: 6px; }
.job-row {
  display: flex; align-items: center; gap: 12px;
  padding: 10px 14px; border-radius: var(--radius-sm);
  background: var(--bg-card); border: 1px solid var(--border);
}
.job-info  { flex: 1; overflow: hidden; }
.job-title  { font-weight: 600; font-size: 0.9rem; }
.job-artist { font-size: 0.75rem; color: var(--text-secondary); }
.job-right  { display: flex; align-items: center; gap: 8px; }
.job-progress { display: flex; align-items: center; gap: 6px; }
.job-pct    { font-size: 0.75rem; color: var(--text-muted); min-width: 32px; }
.job-provider { font-size: 0.72rem; color: var(--text-muted); }
.job-error { font-size: 0.75rem; color: var(--error); padding: 4px 14px 8px; }
</style>
