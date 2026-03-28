<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    downloadJobs, metadataSearchResults, artistDiscography, isSearchingMetadata, providerHealth,
    loadDownloadJobs, searchMetadata, loadDiscography, startDownloadSupervision, stopDownloadSupervision,
  } from '$lib/stores/downloads';
  import { api } from '$lib/api/tauri';
  import { debounce } from '$lib/utils';
  import type { DownloadAlbumResult, CandidateReviewItem } from '$lib/api/tauri';

  let searchInput = '';
  let discogArtist: string | null = null;
  let queueNotice: string | null = null;
  let expandedJob: string | null = null;
  let candidateReview: CandidateReviewItem[] = [];
  let reviewLoading = false;

  async function toggleReview(taskId: string) {
    if (expandedJob === taskId) {
      expandedJob = null;
      candidateReview = [];
      return;
    }
    expandedJob = taskId;
    reviewLoading = true;
    try {
      candidateReview = await api.getCandidateReview(taskId);
    } catch {
      candidateReview = [];
    }
    reviewLoading = false;
  }

  const debouncedSearch = debounce((q: string) => {
    if (q.trim()) searchMetadata(q);
  }, 400);

  $: debouncedSearch(searchInput);

  onMount(() => {
    loadDownloadJobs();
    startDownloadSupervision();
  });
  onDestroy(stopDownloadSupervision);

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

  async function cancelJob(taskId: string) {
    await api.cancelDownload(taskId);
  }

  const statusColors: Record<string, string> = {
    Queued: 'badge-muted',
    Searching: 'badge-warning',
    Downloading: 'badge-accent',
    Verifying: 'badge-warning',
    Done: 'badge-success',
    Cancelled: 'badge-muted',
    Failed: 'badge-error',
  };
</script>

<svelte:head><title>Downloads · Cassette</title></svelte:head>

<div class="downloads-page">
  <div class="page-header">
    <h2 style="flex:1">Downloads</h2>
  </div>

  {#if Object.keys($providerHealth).length > 0}
    <div class="provider-health-strip">
      {#each Object.values($providerHealth) as health}
        <div class="provider-health-chip" class:is-down={health.status === 'Down'}>
          <span class="provider-health-name">{health.provider_id}</span>
          <span class="provider-health-state">{health.status}</span>
          {#if health.message}<span class="provider-health-msg">{health.message}</span>{/if}
        </div>
      {/each}
    </div>
  {/if}

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
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div
            class="job-row"
            class:job-expandable={['Done', 'Failed', 'Cancelled'].includes(job.status)}
            on:click={() => { if (['Done', 'Failed', 'Cancelled'].includes(job.status)) toggleReview(job.id); }}
            on:keydown={(e) => { if ((e.key === 'Enter' || e.key === ' ') && ['Done', 'Failed', 'Cancelled'].includes(job.status)) { e.preventDefault(); toggleReview(job.id); } }}
            role={['Done', 'Failed', 'Cancelled'].includes(job.status) ? 'button' : undefined}
            tabindex={['Done', 'Failed', 'Cancelled'].includes(job.status) ? 0 : undefined}
          >
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
              {#if !['Done', 'Failed', 'Cancelled'].includes(job.status)}
                <button class="btn btn-secondary job-cancel" on:click|stopPropagation={() => cancelJob(job.id)}>
                  Cancel
                </button>
              {/if}
            </div>
          </div>
          {#if job.error}
            <div class="job-error">{job.error}</div>
          {/if}
          {#if expandedJob === job.id}
            <div class="candidate-review-panel">
              {#if reviewLoading}
                <div class="review-loading">Loading candidates...</div>
              {:else if candidateReview.length === 0}
                <div class="review-empty">No candidate data recorded for this task.</div>
              {:else}
                <div class="review-header">Candidates ({candidateReview.length})</div>
                {#each candidateReview as cand}
                  <div class="review-candidate" class:review-selected={cand.is_selected}>
                    <div class="review-cand-header">
                      <span class="review-provider">{cand.provider_display_name}</span>
                      <span class="review-trust">trust {cand.provider_trust_rank}</span>
                      <span class="badge {cand.is_selected ? 'badge-success' : cand.outcome === 'validation_failed' ? 'badge-error' : 'badge-muted'}">
                        {cand.is_selected ? 'SELECTED' : cand.outcome}
                      </span>
                      {#if cand.score_total != null}
                        <span class="review-score">score {cand.score_total}</span>
                      {/if}
                    </div>
                    {#if cand.rejection_reason}
                      <div class="review-rejection">{cand.rejection_reason}</div>
                    {/if}
                  </div>
                {/each}
              {/if}
            </div>
          {/if}
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
.downloads-page { display: flex; flex-direction: column; min-height: 100%; }

.provider-health-strip {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  padding: 0 1.5rem 1rem;
}

.provider-health-chip {
  display: flex;
  gap: 8px;
  align-items: center;
  padding: 6px 10px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: var(--bg-card);
  font-size: 0.75rem;
}

.provider-health-chip.is-down {
  border-color: color-mix(in srgb, var(--error) 50%, var(--border));
}

.provider-health-name {
  font-weight: 600;
  text-transform: uppercase;
}

.provider-health-state {
  color: var(--text-muted);
}

.provider-health-msg {
  color: var(--text-secondary);
}

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
.job-cancel { font-size: 0.72rem; padding: 4px 10px; }
.job-error { font-size: 0.75rem; color: var(--error); padding: 4px 14px 8px; }
.job-expandable { cursor: pointer; }
.job-expandable:hover { border-color: var(--border-active); }

.candidate-review-panel {
  margin: -2px 0 8px;
  padding: 12px 14px;
  border-radius: 0 0 var(--radius-sm) var(--radius-sm);
  background: color-mix(in srgb, var(--bg-card) 80%, var(--bg-primary));
  border: 1px solid var(--border);
  border-top: none;
}
.review-loading, .review-empty { font-size: 0.8rem; color: var(--text-muted); padding: 8px 0; }
.review-header { font-size: 0.72rem; text-transform: uppercase; letter-spacing: 0.06em; color: var(--text-muted); font-weight: 600; margin-bottom: 8px; }
.review-candidate {
  padding: 8px 10px;
  margin-bottom: 4px;
  border-radius: var(--radius-sm);
  background: var(--bg-card);
  border: 1px solid var(--border);
}
.review-candidate.review-selected { border-color: color-mix(in srgb, var(--success) 50%, var(--border)); }
.review-cand-header { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.review-provider { font-weight: 600; font-size: 0.82rem; }
.review-trust { font-size: 0.72rem; color: var(--text-muted); }
.review-score { font-size: 0.75rem; color: var(--text-secondary); font-weight: 600; }
.review-rejection { font-size: 0.75rem; color: var(--error); margin-top: 4px; }
</style>
