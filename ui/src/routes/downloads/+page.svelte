<script lang="ts">
  import { onMount } from 'svelte';
  import {
    backlogStatus, debugStats, downloadJobs, isSearchingMetadata, metadataSearchResults,
    artistDiscography, loadDownloadJobs, searchMetadata, loadDiscography,
    refreshBacklogStatus, refreshDebugStats, startBacklogRun, stopBacklogRun, providerHealth,
    providerStatuses, slskdRuntimeStatus, loadDownloadConfig,
  } from '$lib/stores/downloads';
  import { api, type AcquisitionRequestEvent, type AcquisitionRequestListItem, type CandidateReviewItem, type DownloadAlbumResult, type RequestLineage, type SpotifyAlbumHistory, type TaskResultSummary } from '$lib/api/tauri';
  import { debounce } from '$lib/utils';

  let searchInput = '';
  let discogArtist: string | null = null;
  let queueNotice: string | null = null;
  let backlogLimit = 200;
  let showDebug = false;

  type ProviderDiagnostic = {
    id: string;
    label: string;
    configured: boolean;
    status: 'Unknown' | 'Healthy' | 'Down';
    checkedAt: string | null;
    message: string | null;
    hint: string;
    missingFields: string[];
  };

  let missingAlbums: SpotifyAlbumHistory[] = [];
  let recentResults: TaskResultSummary[] = [];
  let recentRequests: AcquisitionRequestListItem[] = [];
  let loadingRequests = false;

  let expandedJob: string | null = null;
  let candidateReview: CandidateReviewItem[] = [];
  let reviewLoading = false;

  let expandedRequestId: number | null = null;
  let requestTimeline: AcquisitionRequestEvent[] = [];
  let requestCandidates: CandidateReviewItem[] = [];
  let requestLineage: RequestLineage | null = null;

  const debouncedSearch = debounce((query: string) => {
    if (query.trim()) {
      searchMetadata(query);
    }
  }, 400);

  $: debouncedSearch(searchInput);

  $: activeJobs = $downloadJobs.filter((job) =>
    ['Queued', 'Searching', 'Downloading', 'Verifying'].includes(job.status)
  );
  $: blockedJobs = $downloadJobs.filter((job) =>
    ['Failed', 'Cancelled'].includes(job.status)
  );

  $: inFlightRequests = recentRequests.filter((request) =>
    ['queued', 'submitted', 'in_progress'].includes(request.status)
  );
  $: reviewRequests = recentRequests.filter((request) => request.status === 'reviewing');
  $: blockedRequests = recentRequests.filter((request) =>
    ['failed', 'cancelled'].includes(request.status)
  );
  $: completedRequests = recentRequests.filter((request) =>
    ['finalized', 'already_present'].includes(request.status)
  );

  const requestStatusColor: Record<string, string> = {
    pending: 'badge-muted',
    queued: 'badge-warning',
    submitted: 'badge-warning',
    in_progress: 'badge-accent',
    reviewing: 'badge-warning',
    finalized: 'badge-success',
    already_present: 'badge-muted',
    failed: 'badge-error',
    cancelled: 'badge-muted',
  };

  const taskDispositionColor: Record<string, string> = {
    Finalized: 'badge-success',
    AlreadyPresent: 'badge-muted',
    MetadataOnly: 'badge-accent',
    Failed: 'badge-error',
    Cancelled: 'badge-muted',
  };

  const statusColors: Record<string, string> = {
    Queued: 'badge-muted',
    Searching: 'badge-warning',
    Downloading: 'badge-accent',
    Verifying: 'badge-warning',
    Done: 'badge-success',
    Cancelled: 'badge-muted',
    Failed: 'badge-error',
  };

  onMount(async () => {
    await Promise.all([
      loadDownloadJobs(),
      refreshBacklogStatus(),
      loadDownloadConfig(),
      loadRecentRequests(),
      loadMissingAlbums(),
      loadRecentResults(),
    ]);
  });

  $: providerStatusById = $providerStatuses.reduce<Record<string, (typeof $providerStatuses)[number]>>((acc, provider) => {
    acc[provider.id] = provider;
    return acc;
  }, {});

  $: providerDiagnostics = Object.values($providerHealth)
    .map((health): ProviderDiagnostic => {
      const status = providerStatusById[health.provider_id];
      return {
        id: health.provider_id,
        label: status?.label ?? health.provider_id,
        configured: status?.configured ?? true,
        status: health.status,
        checkedAt: health.checked_at ?? null,
        message: health.message ?? null,
        hint: providerTroubleshootingHint(health.provider_id, health.status, health.message, status),
        missingFields: status?.missing_fields ?? [],
      };
    })
    .sort((a, b) => {
      const aScore = a.status === 'Down' ? 0 : a.status === 'Unknown' ? 1 : 2;
      const bScore = b.status === 'Down' ? 0 : b.status === 'Unknown' ? 1 : 2;
      return aScore - bScore || a.label.localeCompare(b.label);
    });

  $: providerDownCount = providerDiagnostics.filter((provider) => provider.status === 'Down').length;
  $: providerUnknownCount = providerDiagnostics.filter((provider) => provider.status === 'Unknown').length;

  function providerTroubleshootingHint(
    providerId: string,
    status: 'Unknown' | 'Healthy' | 'Down',
    message: string | null,
    configured: { configured: boolean; missing_fields?: string[] } | undefined,
  ): string {
    if (configured && !configured.configured) {
      const missing = configured.missing_fields?.length
        ? `Missing: ${configured.missing_fields.join(', ')}`
        : 'Missing required provider credentials in Settings.';
      return `Configuration required. ${missing}`;
    }

    if (providerId === 'slskd' && !$slskdRuntimeStatus?.ready) {
      return 'Soulseek runtime is not ready. Use Settings -> Providers -> Soulseek -> Restart, then refresh.';
    }

    if (status === 'Healthy') {
      return 'Provider responded normally. No action needed.';
    }

    const lower = (message ?? '').toLowerCase();
    if (lower.includes('auth') || lower.includes('401') || lower.includes('forbidden')) {
      return 'Authentication failed. Re-save provider credentials in Settings and retry one request.';
    }
    if (lower.includes('timeout') || lower.includes('timed out')) {
      return 'Timeout detected. Check upstream service responsiveness and local network, then retry.';
    }
    if (lower.includes('rate') || lower.includes('429') || lower.includes('cooldown')) {
      return 'Rate-limit pressure detected. Let cooldown expire or switch to another provider profile.';
    }
    if (lower.includes('connection') || lower.includes('dns') || lower.includes('refused')) {
      return 'Connectivity issue. Verify provider endpoint URL and service availability from this machine.';
    }

    return status === 'Unknown'
      ? 'No recent health signal yet. Run one bounded download to populate provider health events.'
      : 'Provider is down. Review message details, verify service/credentials, and retry from the blocked lane.';
  }

  function checkedAtLabel(value: string | null): string {
    if (!value) {
      return 'not yet checked';
    }
    return value.replace('T', ' ').replace('Z', '');
  }

  async function loadMissingAlbums() {
    try {
      missingAlbums = await api.getMissingSpotifyAlbums(16);
    } catch {
      missingAlbums = [];
    }
  }

  async function loadRecentResults() {
    try {
      recentResults = await api.getRecentTaskResults(16);
    } catch {
      recentResults = [];
    }
  }

  async function loadRecentRequests() {
    loadingRequests = true;
    try {
      recentRequests = await api.listAcquisitionRequests(undefined, 40);
    } catch {
      recentRequests = [];
    } finally {
      loadingRequests = false;
    }
  }

  async function downloadAlbum(album: DownloadAlbumResult) {
    await api.startDownload(album.artist, album.title);
    await loadDownloadJobs();
    await loadRecentRequests();
  }

  async function openDiscog(artist: string, mbid?: string | null) {
    discogArtist = artist;
    await loadDiscography(artist, mbid ?? undefined);
  }

  async function queueDiscography() {
    if (!$artistDiscography) {
      return;
    }

    const report = await api.startDiscographyDownloads(
      $artistDiscography.artist.name,
      $artistDiscography.artist.mbid ?? undefined,
      false,
      false,
      false,
      50,
    );

    queueNotice = `${report.queued} queued, ${report.skipped} skipped (${report.scope}).`;
    await loadDownloadJobs();
    await loadRecentRequests();
  }

  async function cancelJob(taskId: string) {
    await api.cancelDownload(taskId);
    await loadDownloadJobs();
  }

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
    } finally {
      reviewLoading = false;
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

  async function approveRequest(requestId: number) {
    await api.approvePlannedRequest(requestId, 'approved from command center');
    await Promise.all([loadRecentRequests(), loadDownloadJobs()]);
  }

  async function rejectRequest(requestId: number) {
    await api.rejectPlannedRequest(requestId, 'rejected from command center');
    await loadRecentRequests();
  }

  async function toggleDebug() {
    showDebug = !showDebug;
    if (showDebug) {
      await refreshDebugStats();
    }
  }

  function requestIdentityMeta(request: AcquisitionRequestListItem): string {
    const parts: string[] = [];
    if (request.musicbrainz_release_group_id) {
      parts.push(`rg:${request.musicbrainz_release_group_id}`);
    }
    if (request.edition_policy) {
      parts.push(`policy:${request.edition_policy}`);
    }
    return parts.join(' · ');
  }
</script>

<svelte:head><title>Downloads · Cassette</title></svelte:head>

<div class="downloads-page">
  <div class="page-header">
    <div>
      <h2>Downloads</h2>
      <div class="page-subtitle">Missing music, active work, blocked work, and finished handoffs in one place.</div>
    </div>
    <button class="btn btn-secondary" style="font-size:0.78rem;padding:5px 12px;" on:click={toggleDebug}>
      {showDebug ? 'Hide debug' : 'Debug'}
    </button>
  </div>

  <div class="command-grid">
    <section class="command-panel">
      <div class="panel-header">
        <div>
          <div class="panel-kicker">Spotify backlog</div>
          <h3>Keep the missing lane moving</h3>
        </div>
        <div class="panel-actions">
          <label class="limit-label">
            Limit
            <input class="input compact-input" type="number" min="10" max="2000" step="10" bind:value={backlogLimit} />
          </label>
          {#if $backlogStatus?.running}
            <button class="btn btn-secondary" on:click={stopBacklogRun}>Stop</button>
          {:else}
            <button class="btn btn-primary" on:click={() => startBacklogRun(10, backlogLimit)}>Run backlog</button>
          {/if}
        </div>
      </div>

      <div class="metric-row">
        <div class="metric-pill"><span>Missing</span><strong>{missingAlbums.length}</strong></div>
        <div class="metric-pill"><span>In progress</span><strong>{activeJobs.length + inFlightRequests.length}</strong></div>
        <div class="metric-pill"><span>Blocked</span><strong>{blockedJobs.length + blockedRequests.length + reviewRequests.length}</strong></div>
        <div class="metric-pill"><span>Completed</span><strong>{completedRequests.length}</strong></div>
      </div>

      {#if $backlogStatus?.current_album}
        <div class="panel-note">Working on {$backlogStatus.current_album}</div>
      {/if}
    </section>

    <section class="command-panel search-panel">
      <div class="panel-header">
        <div>
          <div class="panel-kicker">Manual pull</div>
          <h3>Search an artist or album</h3>
        </div>
      </div>

      <div class="search-row">
        <input
          class="input"
          type="text"
          placeholder="Search for an artist or album to download..."
          bind:value={searchInput}
        />
        {#if $isSearchingMetadata}
          <div class="spinner" style="width:16px;height:16px;"></div>
        {/if}
      </div>

      {#if queueNotice}
        <div class="panel-note">{queueNotice}</div>
      {/if}
    </section>
  </div>

  {#if Object.keys($providerHealth).length > 0}
    <div class="provider-strip">
      {#each Object.values($providerHealth) as health}
        <div class="provider-chip" class:is-down={health.status === 'Down'}>
          <span>{health.provider_id}</span>
          <strong>{health.status}</strong>
          {#if health.message}<em>{health.message}</em>{/if}
        </div>
      {/each}
    </div>

    <section class="lane-section provider-diagnostics">
      <div class="lane-header">
        <div>
          <div class="panel-kicker">Provider Health</div>
          <h3>Troubleshooting snapshot</h3>
        </div>
      </div>

      <div class="metric-row">
        <div class="metric-pill"><span>Down</span><strong>{providerDownCount}</strong></div>
        <div class="metric-pill"><span>Unknown</span><strong>{providerUnknownCount}</strong></div>
        <div class="metric-pill"><span>Total providers</span><strong>{providerDiagnostics.length}</strong></div>
      </div>

      <div class="provider-diagnostic-grid">
        {#each providerDiagnostics as provider}
          <article class="provider-diagnostic-card" class:is-down={provider.status === 'Down'}>
            <div class="provider-diagnostic-header">
              <strong>{provider.label}</strong>
              <span class="badge {provider.status === 'Down' ? 'badge-error' : provider.status === 'Unknown' ? 'badge-muted' : 'badge-success'}">{provider.status}</span>
            </div>
            <div class="provider-diagnostic-meta">
              <span>Config: {provider.configured ? 'configured' : 'missing fields'}</span>
              <span>Checked: {checkedAtLabel(provider.checkedAt)}</span>
            </div>
            {#if provider.message}
              <div class="provider-diagnostic-message">{provider.message}</div>
            {/if}
            <div class="provider-diagnostic-hint">{provider.hint}</div>
            {#if provider.missingFields.length > 0}
              <div class="provider-diagnostic-fields">Missing: {provider.missingFields.join(', ')}</div>
            {/if}
          </article>
        {/each}
      </div>
    </section>
  {/if}

  {#if $metadataSearchResults && searchInput.trim()}
    <section class="results-section">
      {#if $metadataSearchResults.artists.length > 0}
        <div class="lane-section">
          <div class="lane-header">
            <div>
              <div class="panel-kicker">Artists</div>
              <h3>Discography entry points</h3>
            </div>
          </div>
          <div class="simple-stack">
            {#each $metadataSearchResults.artists as artist}
              <button class="simple-row" on:click={() => openDiscog(artist.name, artist.mbid)}>
                <span class="row-title">{artist.name}</span>
                <span class="row-meta">{artist.disambiguation || artist.source}</span>
              </button>
            {/each}
          </div>
        </div>
      {/if}

      {#if $metadataSearchResults.albums.length > 0}
        <div class="lane-section">
          <div class="lane-header">
            <div>
              <div class="panel-kicker">Albums</div>
              <h3>Direct album pulls</h3>
            </div>
          </div>
          <div class="simple-stack">
            {#each $metadataSearchResults.albums as album}
              <div class="simple-row simple-row-static">
                <span class="row-copy">
                  <span class="row-title">{album.title}</span>
                  <span class="row-meta">{album.artist} · {album.year ?? '?'} · {album.release_type ?? 'Album'}</span>
                </span>
                <button class="btn btn-primary" on:click={() => downloadAlbum(album)}>Download</button>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </section>
  {/if}

  {#if $artistDiscography && discogArtist}
    <section class="lane-section">
      <div class="lane-header">
        <div>
          <div class="panel-kicker">Discography</div>
          <h3>{$artistDiscography.artist.name}</h3>
        </div>
        <button class="btn btn-primary" on:click={queueDiscography}>Queue discography</button>
      </div>
      <div class="simple-stack">
        {#each $artistDiscography.albums as album}
          <div class="simple-row simple-row-static">
            <span class="row-copy">
              <span class="row-title">{album.title}</span>
              <span class="row-meta">{album.year ?? '?'} · {album.release_type ?? 'Album'} · {album.track_count ?? '?'} tracks</span>
            </span>
            <button class="btn btn-primary" on:click={() => downloadAlbum(album)}>Download</button>
          </div>
        {/each}
      </div>
    </section>
  {/if}

  <div class="lane-grid">
    <section class="lane-section">
      <div class="lane-header">
        <div>
          <div class="panel-kicker">Missing</div>
          <h3>Albums the system still wants</h3>
        </div>
      </div>
      <div class="simple-stack">
        {#if missingAlbums.length === 0}
          <div class="lane-empty">Nothing is currently flagged missing in the Spotify backlog.</div>
        {:else}
          {#each missingAlbums as album}
            <div class="simple-row simple-row-static">
              <span class="row-copy">
                <span class="row-title">{album.artist} · {album.album}</span>
                <span class="row-meta">{album.play_count} plays · {Math.round(album.total_ms / 60000)} minutes listened</span>
              </span>
              <span class="badge badge-muted">missing</span>
            </div>
          {/each}
        {/if}
      </div>
    </section>

    <section class="lane-section">
      <div class="lane-header">
        <div>
          <div class="panel-kicker">In progress</div>
          <h3>Requests and jobs still moving</h3>
        </div>
      </div>
      <div class="simple-stack">
        {#if activeJobs.length === 0 && inFlightRequests.length === 0}
          <div class="lane-empty">No active search, download, or verification work right now.</div>
        {/if}

        {#each activeJobs as job}
          <div class="job-row">
            <span class="row-copy">
              <span class="row-title">{job.artist} · {job.title}</span>
              <span class="row-meta">{job.album || 'single'}{job.provider ? ` · ${job.provider}` : ''}</span>
            </span>
            <div class="job-meta">
              <span class="badge {statusColors[job.status] ?? 'badge-muted'}">{job.status}</span>
              <button class="btn btn-secondary small-btn" on:click={() => cancelJob(job.id)}>Cancel</button>
            </div>
          </div>
        {/each}

        {#each inFlightRequests as request}
          <button class="request-row" on:click={() => toggleRequest(request)}>
            <span class="row-copy">
              <span class="row-title">{request.artist} · {request.title}</span>
              <span class="row-meta">{request.album || request.scope} · {request.strategy}{requestIdentityMeta(request) ? ` · ${requestIdentityMeta(request)}` : ''}</span>
            </span>
            <span class="badge {requestStatusColor[request.status] ?? 'badge-muted'}">{request.status}</span>
          </button>
          {#if expandedRequestId === request.id}
            <div class="request-detail">
              {#each requestTimeline as event}
                <div class="timeline-row">
                  <span class="badge {requestStatusColor[event.status] ?? 'badge-muted'}">{event.status}</span>
                  <span>{event.event_type}</span>
                  <span class="timeline-time">{event.created_at}</span>
                  {#if event.message}
                    <div class="timeline-message">{event.message}</div>
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
        {/each}
      </div>
    </section>

    <section class="lane-section">
      <div class="lane-header">
        <div>
          <div class="panel-kicker">Blocked</div>
          <h3>Needs a decision or recovery</h3>
        </div>
      </div>
      <div class="simple-stack">
        {#if reviewRequests.length === 0 && blockedRequests.length === 0 && blockedJobs.length === 0}
          <div class="lane-empty">No blocked or review-first work at the moment.</div>
        {/if}

        {#each reviewRequests as request}
          <button class="request-row" on:click={() => toggleRequest(request)}>
            <span class="row-copy">
              <span class="row-title">{request.artist} · {request.title}</span>
              <span class="row-meta">{request.trust_reason_code} · {request.album || request.scope}{requestIdentityMeta(request) ? ` · ${requestIdentityMeta(request)}` : ''}</span>
            </span>
            <span class="badge badge-warning">review</span>
          </button>
          {#if expandedRequestId === request.id}
            <div class="request-detail">
              {#if requestLineage?.trust}
                <div class="trust-card">
                  <div class="trust-eyebrow">{requestLineage.trust.reason_code}</div>
                  <strong>{requestLineage.trust.headline}</strong>
                  <div class="timeline-message">{requestLineage.trust.detail}</div>
                  <div class="trust-evidence">{requestLineage.trust.evidence_count} evidence points recorded</div>
                </div>
              {/if}
              <div class="request-actions">
                <button class="btn btn-primary small-btn" on:click={() => approveRequest(request.id)}>Approve</button>
                <button class="btn btn-secondary small-btn" on:click={() => rejectRequest(request.id)}>Reject</button>
              </div>
              {#if requestCandidates.length === 0}
                <div class="lane-empty">No candidate review recorded yet.</div>
              {:else}
                {#each requestCandidates as candidate}
                  <div class="candidate-row" class:selected={candidate.is_selected}>
                    <strong>{candidate.provider_display_name}</strong>
                    <span>{candidate.is_selected ? 'selected' : candidate.outcome}</span>
                    {#if candidate.rejection_reason}
                      <div class="timeline-message">{candidate.rejection_reason}</div>
                    {/if}
                  </div>
                {/each}
              {/if}
            </div>
          {/if}
        {/each}

        {#each blockedRequests as request}
          <button class="request-row" on:click={() => toggleRequest(request)}>
            <span class="row-copy">
              <span class="row-title">{request.artist} · {request.title}</span>
              <span class="row-meta">{request.trust_reason_code || request.failure_class || request.album || request.scope}{requestIdentityMeta(request) ? ` · ${requestIdentityMeta(request)}` : ''}</span>
            </span>
            <span class="badge {requestStatusColor[request.status] ?? 'badge-muted'}">{request.status}</span>
          </button>
          {#if expandedRequestId === request.id}
            <div class="request-detail">
              {#if requestLineage?.trust}
                <div class="trust-card">
                  <div class="trust-eyebrow">{requestLineage.trust.reason_code}</div>
                  <strong>{requestLineage.trust.headline}</strong>
                  <div class="timeline-message">{requestLineage.trust.detail}</div>
                  <div class="trust-evidence">{requestLineage.trust.evidence_count} evidence points recorded</div>
                </div>
              {/if}
              {#if requestLineage?.execution}
                <div class="timeline-message">
                  {requestLineage.execution.disposition}
                  {#if requestLineage.execution.provider} via {requestLineage.execution.provider}{/if}
                  {#if requestLineage.execution.final_path}<br />{requestLineage.execution.final_path}{/if}
                </div>
              {/if}
              {#each requestTimeline as event}
                <div class="timeline-row">
                  <span class="badge {requestStatusColor[event.status] ?? 'badge-muted'}">{event.status}</span>
                  <span>{event.event_type}</span>
                  <span class="timeline-time">{event.created_at}</span>
                  {#if event.message}
                    <div class="timeline-message">{event.message}</div>
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
        {/each}

        {#each blockedJobs as job}
          <button class="request-row" on:click={() => toggleReview(job.id)}>
            <span class="row-copy">
              <span class="row-title">{job.artist} · {job.title}</span>
              <span class="row-meta">{job.error || job.provider || 'job failed'}</span>
            </span>
            <span class="badge {statusColors[job.status] ?? 'badge-muted'}">{job.status}</span>
          </button>
          {#if expandedJob === job.id}
            <div class="request-detail">
              {#if reviewLoading}
                <div class="lane-empty">Loading candidate review...</div>
              {:else if candidateReview.length === 0}
                <div class="lane-empty">No candidate data recorded for this task.</div>
              {:else}
                {#each candidateReview as candidate}
                  <div class="candidate-row" class:selected={candidate.is_selected}>
                    <strong>{candidate.provider_display_name}</strong>
                    <span>{candidate.is_selected ? 'selected' : candidate.outcome}</span>
                    {#if candidate.rejection_reason}
                      <div class="timeline-message">{candidate.rejection_reason}</div>
                    {/if}
                  </div>
                {/each}
              {/if}
            </div>
          {/if}
        {/each}
      </div>
    </section>

    <section class="lane-section">
      <div class="lane-header">
        <div>
          <div class="panel-kicker">Completed</div>
          <h3>Recent handoffs and skipped duplicates</h3>
        </div>
      </div>
      <div class="simple-stack">
        {#if completedRequests.length === 0 && recentResults.length === 0}
          <div class="lane-empty">No completed request history yet.</div>
        {/if}

        {#each completedRequests as request}
          <button class="request-row" on:click={() => toggleRequest(request)}>
            <span class="row-copy">
              <span class="row-title">{request.artist} · {request.title}</span>
              <span class="row-meta">{request.trust_reason_code} · {request.final_path || request.execution_disposition || request.album || request.scope}{requestIdentityMeta(request) ? ` · ${requestIdentityMeta(request)}` : ''}</span>
            </span>
            <span class="badge {requestStatusColor[request.status] ?? 'badge-muted'}">{request.status}</span>
          </button>
          {#if expandedRequestId === request.id}
            <div class="request-detail">
              {#if requestLineage?.trust}
                <div class="trust-card">
                  <div class="trust-eyebrow">{requestLineage.trust.reason_code}</div>
                  <strong>{requestLineage.trust.headline}</strong>
                  <div class="timeline-message">{requestLineage.trust.detail}</div>
                  <div class="trust-evidence">{requestLineage.trust.evidence_count} evidence points recorded</div>
                </div>
              {/if}
              {#if requestLineage?.execution}
                <div class="timeline-message">
                  {requestLineage.execution.disposition}
                  {#if requestLineage.execution.provider} via {requestLineage.execution.provider}{/if}
                  {#if requestLineage.execution.final_path}<br />{requestLineage.execution.final_path}{/if}
                </div>
              {/if}
            </div>
          {/if}
        {/each}

        {#each recentResults as result}
          <div class="simple-row simple-row-static">
            <span class="row-copy">
              <span class="row-title">{result.task_id}</span>
              <span class="row-meta">{result.provider || 'unknown provider'}{result.error ? ` · ${result.error}` : ''}</span>
            </span>
            <span class="badge {taskDispositionColor[result.disposition] ?? 'badge-muted'}">{result.disposition}</span>
          </div>
        {/each}
      </div>
    </section>
  </div>

  {#if showDebug}
    <section class="lane-section debug-section">
      <div class="lane-header">
        <div>
          <div class="panel-kicker">Debug</div>
          <h3>Director snapshot</h3>
        </div>
        <button class="btn btn-secondary small-btn" on:click={refreshDebugStats}>Refresh</button>
      </div>

      {#if $debugStats}
        <div class="metric-row">
          <div class="metric-pill"><span>Pending</span><strong>{$debugStats.pending_count}</strong></div>
          {#each $debugStats.provider_stats as provider}
            <div class="metric-pill">
              <span>{provider.provider}</span>
              <strong>{provider.success}/{provider.failed}</strong>
            </div>
          {/each}
        </div>
      {:else}
        <div class="lane-empty">Debug snapshot is not loaded yet.</div>
      {/if}
    </section>
  {/if}
</div>

<style>
.downloads-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding-bottom: 18px;
}

.page-subtitle {
  margin-top: 4px;
  color: var(--text-secondary);
  font-size: 0.8rem;
}

.command-grid,
.lane-grid,
.results-section {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
  padding: 0 16px;
}

.lane-grid {
  align-items: start;
}

.command-panel,
.lane-section {
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--bg-card);
  padding: 16px;
}

.debug-section {
  margin: 0 16px;
}

.panel-header,
.lane-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 12px;
  margin-bottom: 12px;
}

.panel-kicker {
  font-size: 0.68rem;
  text-transform: uppercase;
  letter-spacing: 0.1em;
  color: var(--accent);
  font-weight: 700;
}

.panel-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.limit-label {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-secondary);
  font-size: 0.78rem;
}

.compact-input {
  width: 72px;
  padding: 4px 8px;
  font-size: 0.8rem;
}

.search-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.panel-note {
  margin-top: 10px;
  color: var(--text-secondary);
  font-size: 0.78rem;
}

.metric-row {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}

.metric-pill {
  min-width: 110px;
  padding: 10px 12px;
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: var(--bg-base);
}

.metric-pill span {
  display: block;
  font-size: 0.66rem;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--text-muted);
}

.metric-pill strong {
  display: block;
  margin-top: 5px;
  font-size: 1.16rem;
  color: var(--text-primary);
}

.provider-strip {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  padding: 0 16px;
}

.provider-chip {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: var(--bg-card);
  font-size: 0.74rem;
}

.provider-chip.is-down {
  border-color: color-mix(in srgb, var(--error) 40%, var(--border));
}

.provider-chip em {
  font-style: normal;
  color: var(--text-secondary);
}

.provider-diagnostics {
  margin: 0 16px;
}

.provider-diagnostic-grid {
  margin-top: 10px;
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
  gap: 10px;
}

.provider-diagnostic-card {
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg-base);
  padding: 10px;
  display: grid;
  gap: 8px;
}

.provider-diagnostic-card.is-down {
  border-color: color-mix(in srgb, var(--error) 45%, var(--border));
}

.provider-diagnostic-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
}

.provider-diagnostic-meta {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  font-size: 0.72rem;
  color: var(--text-secondary);
}

.provider-diagnostic-message,
.provider-diagnostic-hint,
.provider-diagnostic-fields {
  font-size: 0.74rem;
  color: var(--text-secondary);
  line-height: 1.4;
}

.provider-diagnostic-hint {
  color: var(--text-primary);
}

.simple-stack {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.simple-row,
.request-row,
.job-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  width: 100%;
  padding: 10px 12px;
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: var(--bg-base);
  text-align: left;
}

.request-row:hover,
.simple-row:hover {
  border-color: var(--border-active);
}

.simple-row-static {
  cursor: default;
}

.row-copy {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
}

.row-title {
  font-size: 0.86rem;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.row-meta {
  font-size: 0.74rem;
  color: var(--text-secondary);
}

.job-meta {
  display: flex;
  align-items: center;
  gap: 8px;
}

.small-btn {
  font-size: 0.74rem;
  padding: 4px 10px;
}

.lane-empty {
  color: var(--text-secondary);
  font-size: 0.82rem;
  padding: 12px 0 4px;
}

.request-detail {
  margin-top: -4px;
  padding: 12px;
  border-radius: 0 0 var(--radius) var(--radius);
  border: 1px solid var(--border);
  border-top: none;
  background: color-mix(in srgb, var(--bg-base) 82%, var(--bg-card));
}

.request-actions {
  display: flex;
  gap: 8px;
  margin-bottom: 10px;
}

.candidate-row {
  padding: 8px 10px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: var(--bg-card);
  margin-bottom: 6px;
}

.candidate-row.selected {
  border-color: color-mix(in srgb, var(--success) 42%, var(--border));
}

.timeline-row {
  display: grid;
  grid-template-columns: auto auto 1fr;
  gap: 8px;
  align-items: center;
  padding: 6px 0;
  border-bottom: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
}

.timeline-time {
  font-size: 0.7rem;
  color: var(--text-muted);
}

.timeline-message {
  grid-column: 1 / -1;
  font-size: 0.76rem;
  color: var(--text-secondary);
  white-space: pre-wrap;
}

.trust-card {
  display: grid;
  gap: 4px;
  padding: 10px 12px;
  margin-bottom: 10px;
  border: 1px solid color-mix(in srgb, var(--accent) 30%, var(--border));
  border-radius: var(--radius-md);
  background: color-mix(in srgb, var(--accent) 8%, var(--bg-elevated));
}

.trust-eyebrow,
.trust-evidence {
  font-size: 0.72rem;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--text-secondary);
}

@media (max-width: 1100px) {
  .command-grid,
  .lane-grid,
  .results-section {
    grid-template-columns: 1fr;
  }
}
</style>
