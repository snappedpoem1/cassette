<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    downloadJobs, metadataSearchResults, artistDiscography, isSearchingMetadata, providerHealth,
    backlogStatus, debugStats,
    loadDownloadJobs, searchMetadata, loadDiscography,
    startDownloadSupervision, stopDownloadSupervision,
    startBacklogRun, stopBacklogRun, refreshBacklogStatus, refreshDebugStats,
  } from '$lib/stores/downloads';
  import { api } from '$lib/api/tauri';
  import { debounce } from '$lib/utils';
  import type {
    AcquisitionRequestEvent,
    AcquisitionRequestListItem,
    CandidateReviewItem,
    DownloadAlbumResult
  } from '$lib/api/tauri';

  let searchInput = '';
  let discogArtist: string | null = null;
  let queueNotice: string | null = null;
  let expandedJob: string | null = null;
  let candidateReview: CandidateReviewItem[] = [];
  let reviewLoading = false;
  let showDebug = false;
  let backlogLimit = 200;
  let recentRequests: AcquisitionRequestListItem[] = [];
  let requestLoading = false;
  let expandedRequestId: number | null = null;
  let requestTimeline: AcquisitionRequestEvent[] = [];
  let requestCandidates: CandidateReviewItem[] = [];
  let requestLineage: any = null;

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

  onMount(async () => {
    loadDownloadJobs();
    startDownloadSupervision();
    refreshBacklogStatus();
    await loadRecentRequests();
  });
  onDestroy(stopDownloadSupervision);

  async function loadRecentRequests() {
    requestLoading = true;
    try {
      recentRequests = await api.listAcquisitionRequests(undefined, 25);
    } catch {
      recentRequests = [];
    } finally {
      requestLoading = false;
    }
  }

  async function approveRequest(requestId: number) {
    try {
      await api.approvePlannedRequest(requestId, 'approved from downloads review panel');
      await loadRecentRequests();
      const refreshed = recentRequests.find((request) => request.id === requestId);
      if (refreshed) {
        expandedRequestId = null;
        await toggleRequest(refreshed);
      }
    } catch {
      // no-op; timeline panel already carries request failure signal
    }
  }

  async function rejectRequest(requestId: number) {
    try {
      await api.rejectPlannedRequest(requestId, 'rejected from downloads review panel');
      await loadRecentRequests();
      const refreshed = recentRequests.find((request) => request.id === requestId);
      if (refreshed) {
        expandedRequestId = null;
        await toggleRequest(refreshed);
      }
    } catch {
      // no-op; timeline panel already carries request failure signal
    }
  }

  async function toggleRequest(request: AcquisitionRequestListItem) {
    if (expandedRequestId === request.id) {
      expandedRequestId = null;
      requestTimeline = [];
      requestCandidates = [];
      requestLineage = null;
      return;
    }

    expandedRequestId = request.id;
    requestTimeline = [];
    requestCandidates = [];
    requestLineage = null;

    try {
      const [timeline, candidates, lineage] = await Promise.all([
        api.getAcquisitionRequestTimeline(request.id),
        api.getRequestCandidateReview(request.id),
        api.getRequestLineage(request.id),
      ]);
      requestTimeline = timeline;
      requestCandidates = candidates;
      requestLineage = lineage;
    } catch {
      requestTimeline = [];
      requestCandidates = [];
      requestLineage = null;
    }
  }

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
      false, false, false, 50
    );
    queueNotice = `${report.queued} queued, ${report.skipped} skipped (${report.scope}).`;
    await loadDownloadJobs();
  }

  async function cancelJob(taskId: string) {
    await api.cancelDownload(taskId);
  }

  async function toggleDebug() {
    showDebug = !showDebug;
    if (showDebug) {
      await refreshDebugStats();
      await loadRecentRequests();
    }
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

  const dispositionColor: Record<string, string> = {
    Finalized: 'badge-success',
    AlreadyPresent: 'badge-muted',
    Failed: 'badge-error',
    Cancelled: 'badge-muted',
  };

  const requestStatusColor: Record<string, string> = {
    pending: 'badge-muted',
    queued: 'badge-warning',
    submitted: 'badge-warning',
    in_progress: 'badge-accent',
    finalized: 'badge-success',
    already_present: 'badge-muted',
    failed: 'badge-error',
    cancelled: 'badge-muted',
  };
</script>

<svelte:head><title>Downloads · Cassette</title></svelte:head>

<div class="downloads-page">
  <div class="page-header">
    <h2 style="flex:1">Downloads</h2>
    <button class="btn btn-secondary" style="font-size:0.78rem;padding:5px 12px;" on:click={toggleDebug}>
      {showDebug ? 'Hide Debug' : 'Debug'}
    </button>
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

  <!-- Backlog runner -->
  <div class="backlog-panel">
    <div class="backlog-header">
      <div class="dl-section-label" style="margin:0">Spotify Backlog</div>
      <div class="backlog-controls">
        <label class="backlog-limit-label">
          Limit
          <input class="input" type="number" min="10" max="2000" step="10" bind:value={backlogLimit}
            style="width:72px;padding:4px 8px;font-size:0.8rem;" />
        </label>
        {#if $backlogStatus?.running}
          <button class="btn btn-secondary" on:click={stopBacklogRun}>Stop</button>
        {:else}
          <button class="btn btn-primary" on:click={() => startBacklogRun(10, backlogLimit)}>
            Run Backlog
          </button>
        {/if}
      </div>
    </div>
    {#if $backlogStatus}
      <div class="backlog-stats">
        <span class="backlog-stat">
          <span class="backlog-stat-label">queued</span>
          <span class="backlog-stat-val">{$backlogStatus.albums_queued}</span>
        </span>
        <span class="backlog-stat">
          <span class="backlog-stat-label">skipped</span>
          <span class="backlog-stat-val">{$backlogStatus.albums_skipped}</span>
        </span>
        <span class="backlog-stat">
          <span class="backlog-stat-label">tracks</span>
          <span class="backlog-stat-val">{$backlogStatus.tracks_submitted}</span>
        </span>
        {#if $backlogStatus.running}
          <span class="backlog-stat">
            <span class="backlog-stat-label">status</span>
            <span class="badge badge-accent" style="font-size:0.7rem;">running</span>
          </span>
        {:else if $backlogStatus.finished_at}
          <span class="backlog-stat">
            <span class="backlog-stat-label">done</span>
            <span class="badge badge-muted" style="font-size:0.7rem;">idle</span>
          </span>
        {/if}
      </div>
      {#if $backlogStatus.current_album}
        <div class="backlog-current">Processing: {$backlogStatus.current_album}</div>
      {/if}
      {#if $backlogStatus.errors.length > 0}
        <div class="backlog-errors">
          {#each $backlogStatus.errors.slice(-5) as err}
            <div class="backlog-error-row">{err}</div>
          {/each}
          {#if $backlogStatus.errors.length > 5}
            <div class="backlog-error-row muted">…and {$backlogStatus.errors.length - 5} more</div>
          {/if}
        </div>
      {/if}
    {/if}
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
          {@const expandable = ['Done', 'Failed', 'Cancelled'].includes(job.status)}
          {#if expandable}
            <button class="job-row job-expandable" type="button" on:click={() => toggleReview(job.id)}>
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
            </button>
          {:else}
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
                <button class="btn btn-secondary job-cancel" on:click|stopPropagation={() => cancelJob(job.id)}>
                  Cancel
                </button>
              </div>
            </div>
          {/if}
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

  <div class="requests-panel">
    <div class="requests-header">
      <div class="dl-section-label" style="margin:0">Recent Requests</div>
      <button class="btn btn-secondary" style="font-size:0.75rem;padding:4px 10px;" on:click={loadRecentRequests}>
        Refresh
      </button>
    </div>

    {#if requestLoading}
      <div class="debug-empty">Loading request timeline...</div>
    {:else if recentRequests.length === 0}
      <div class="debug-empty">No control-plane requests recorded yet.</div>
    {:else}
      <div class="debug-results-list">
        {#each recentRequests as request}
          <div
            class="debug-result-row request-row"
            role="button"
            tabindex="0"
            on:click={() => toggleRequest(request)}
            on:keydown={(event) => {
              if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault();
                toggleRequest(request);
              }
            }}
          >
            <span class="badge {requestStatusColor[request.status] ?? 'badge-muted'}" style="font-size:0.68rem;min-width:72px;text-align:center;">
              {request.status}
            </span>
            <span class="debug-result-provider">{request.selected_provider || request.execution_disposition || 'request'}</span>
            <span class="debug-result-task" title={request.task_id || request.request_signature}>
              {request.artist} {request.title ? `- ${request.title}` : ''}{request.album ? ` (${request.album})` : ''}
            </span>
            {#if request.failure_class}
              <span class="debug-result-error" title={request.failure_class}>{request.failure_class}</span>
            {/if}
          </div>

          {#if expandedRequestId === request.id}
            <div class="candidate-review-panel">
              {#if request.status === 'reviewing'}
                <div class="request-review-actions">
                  <button
                    class="btn btn-primary"
                    style="font-size:0.75rem;padding:4px 10px;"
                    on:click|stopPropagation={() => approveRequest(request.id)}>
                    Approve
                  </button>
                  <button
                    class="btn btn-secondary"
                    style="font-size:0.75rem;padding:4px 10px;"
                    on:click|stopPropagation={() => rejectRequest(request.id)}>
                    Reject
                  </button>
                </div>
              {/if}

              <div class="review-header">Timeline ({requestTimeline.length})</div>
              {#if requestTimeline.length === 0}
                <div class="review-empty">No request events recorded.</div>
              {:else}
                {#each requestTimeline as event}
                  <div class="timeline-row">
                    <span class="badge {requestStatusColor[event.status] ?? 'badge-muted'}" style="font-size:0.68rem;">{event.status}</span>
                    <span class="timeline-type">{event.event_type}</span>
                    <span class="timeline-time">{event.created_at}</span>
                    {#if event.message}
                      <div class="timeline-message">{event.message}</div>
                    {/if}
                  </div>
                {/each}
              {/if}

              {#if requestLineage?.execution}
                <div class="review-header" style="margin-top:12px;">Execution</div>
                <div class="timeline-message">
                  {requestLineage.execution.disposition}
                  {#if requestLineage.execution.provider} via {requestLineage.execution.provider}{/if}
                  {#if requestLineage.execution.final_path}<br />{requestLineage.execution.final_path}{/if}
                </div>
              {/if}

              <div class="review-header" style="margin-top:12px;">Candidates ({requestCandidates.length})</div>
              {#if requestCandidates.length === 0}
                <div class="review-empty">No candidate review captured for this request.</div>
              {:else}
                {#each requestCandidates as cand}
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

  <!-- Debug panel -->
  {#if showDebug}
    <div class="debug-panel">
      <div class="debug-header">
        <div class="dl-section-label" style="margin:0">Director Debug</div>
        <button class="btn btn-secondary" style="font-size:0.75rem;padding:4px 10px;"
          on:click={refreshDebugStats}>Refresh</button>
      </div>

      {#if $debugStats}
        <div class="debug-row">
          <span class="debug-label">Pending tasks</span>
          <span class="debug-val">{$debugStats.pending_count}</span>
        </div>

        {#if $debugStats.provider_stats.length > 0}
          <div class="debug-section-title">Provider results (last 100)</div>
          <div class="debug-provider-grid">
            {#each $debugStats.provider_stats.sort((a, b) => (b.success + b.failed) - (a.success + a.failed)) as p}
              <div class="debug-provider-row">
                <span class="debug-provider-name">{p.provider}</span>
                <span class="badge badge-success" style="font-size:0.68rem;">{p.success} ok</span>
                {#if p.failed > 0}
                  <span class="badge badge-error" style="font-size:0.68rem;">{p.failed} fail</span>
                {/if}
              </div>
            {/each}
          </div>
        {/if}

        {#if $debugStats.recent_results.length > 0}
          <div class="debug-section-title">Recent results</div>
          <div class="debug-results-list">
            {#each $debugStats.recent_results.slice(0, 50) as r}
              <div class="debug-result-row">
                <span class="badge {dispositionColor[r.disposition] ?? 'badge-muted'}" style="font-size:0.68rem;min-width:72px;text-align:center;">
                  {r.disposition}
                </span>
                <span class="debug-result-provider">{r.provider || '—'}</span>
                <span class="debug-result-task" title={r.task_id}>{r.task_id}</span>
                {#if r.error}
                  <span class="debug-result-error" title={r.error}>{r.error}</span>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      {:else}
        <div class="debug-empty">Click Refresh to load stats.</div>
      {/if}
    </div>
  {/if}
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
  width: 100%;
  padding: 10px 14px; border-radius: var(--radius-sm);
  background: var(--bg-card); border: 1px solid var(--border);
  text-align: left;
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
.request-review-actions { display: flex; gap: 8px; margin-bottom: 10px; }
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

.requests-panel {
  margin: 0 1.5rem 1.5rem;
  padding: 12px 14px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: color-mix(in srgb, var(--bg-card) 70%, var(--bg-primary));
}
.requests-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }
.request-row { cursor: pointer; }
.request-row:hover { border-color: var(--border-active); }
.timeline-row {
  display: grid;
  grid-template-columns: auto auto 1fr;
  gap: 8px;
  align-items: center;
  padding: 6px 0;
  border-bottom: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
}
.timeline-type { font-size: 0.78rem; font-weight: 600; color: var(--text-primary); }
.timeline-time { font-size: 0.7rem; color: var(--text-muted); }
.timeline-message {
  grid-column: 1 / -1;
  font-size: 0.75rem;
  color: var(--text-secondary);
  white-space: pre-wrap;
}

/* Backlog panel */
.backlog-panel {
  margin: 0 1.5rem 1rem;
  padding: 12px 14px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: var(--bg-card);
}
.backlog-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }
.backlog-controls { display: flex; align-items: center; gap: 10px; }
.backlog-limit-label { display: flex; align-items: center; gap: 6px; font-size: 0.78rem; color: var(--text-secondary); }
.backlog-stats { display: flex; gap: 16px; flex-wrap: wrap; margin-bottom: 6px; }
.backlog-stat { display: flex; flex-direction: column; align-items: center; gap: 2px; }
.backlog-stat-label { font-size: 0.68rem; text-transform: uppercase; letter-spacing: 0.06em; color: var(--text-muted); }
.backlog-stat-val { font-size: 1rem; font-weight: 700; color: var(--text-primary); }
.backlog-current { font-size: 0.78rem; color: var(--text-secondary); margin-top: 6px; font-style: italic; }
.backlog-errors { margin-top: 8px; }
.backlog-error-row { font-size: 0.74rem; color: var(--error); padding: 2px 0; }
.backlog-error-row.muted { color: var(--text-muted); }

/* Debug panel */
.debug-panel {
  margin: 0 1.5rem 1.5rem;
  padding: 12px 14px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: color-mix(in srgb, var(--bg-card) 70%, var(--bg-primary));
}
.debug-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }
.debug-row { display: flex; align-items: center; gap: 10px; margin-bottom: 6px; font-size: 0.8rem; }
.debug-label { color: var(--text-muted); min-width: 100px; }
.debug-val { font-weight: 600; }
.debug-section-title { font-size: 0.7rem; text-transform: uppercase; letter-spacing: 0.07em; color: var(--text-muted); font-weight: 600; margin: 10px 0 6px; }
.debug-provider-grid { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 4px; }
.debug-provider-row { display: flex; align-items: center; gap: 6px; padding: 5px 8px; border-radius: var(--radius-sm); background: var(--bg-card); border: 1px solid var(--border); }
.debug-provider-name { font-size: 0.78rem; font-weight: 600; min-width: 48px; }
.debug-results-list { display: flex; flex-direction: column; gap: 3px; max-height: 320px; overflow-y: auto; }
.debug-result-row { display: flex; align-items: baseline; gap: 8px; padding: 4px 6px; border-radius: var(--radius-sm); background: var(--bg-card); font-size: 0.75rem; border: 1px solid var(--border); }
.debug-result-provider { color: var(--text-secondary); min-width: 56px; font-size: 0.72rem; }
.debug-result-task { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-muted); font-size: 0.7rem; }
.debug-result-error { color: var(--error); font-size: 0.7rem; max-width: 200px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.debug-empty { font-size: 0.8rem; color: var(--text-muted); padding: 8px 0; }
</style>
