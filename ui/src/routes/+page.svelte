<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import { buildArtistClusters } from '$lib/artist-clusters';
  import { buildAutomationDigest } from '$lib/automation-digest';
  import AutomationDigestPanel from '$lib/components/AutomationDigestPanel.svelte';
  import {
    api,
    type AcquisitionRequestListItem,
    type SpotifyAlbumHistory,
    type TaskResultSummary,
    type Track,
    type TrustReasonDistributionEntry,
  } from '$lib/api/tauri';
  import { artists, isScanning, trackCount } from '$lib/stores/library';
  import { backlogStatus, providerHealth, providerStatuses, slskdRuntimeStatus } from '$lib/stores/downloads';
  import { currentTrack, nowPlayingContext, playbackState, player } from '$lib/stores/player';
  import { queue } from '$lib/stores/queue';
  import { formatDuration, coverSrc } from '$lib/utils';

  interface WhileAwayMessage {
    tone: 'steady' | 'watch' | 'action';
    title: string;
    detail: string;
  }

  let missingAlbums: SpotifyAlbumHistory[] = [];
  let recentResults: TaskResultSummary[] = [];
  let recentRequests: AcquisitionRequestListItem[] = [];
  let recentTracks: Track[] = [];
  let trustDistribution: TrustReasonDistributionEntry[] = [];
  let loading = true;

  $: artistClusters = buildArtistClusters($artists);
  $: topArtists = artistClusters.slice(0, 8);
  $: current = $currentTrack;
  $: context = $nowPlayingContext;
  $: activePlayback = $playbackState.is_playing && current;

  $: inProgressCount = recentRequests.filter((request) =>
    ['queued', 'submitted', 'in_progress'].includes(request.status)
  ).length;
  $: blockedCount = recentRequests.filter((request) =>
    ['reviewing', 'failed', 'cancelled'].includes(request.status)
  ).length;
  $: completedCount = recentRequests.filter((request) =>
    ['finalized', 'already_present'].includes(request.status)
  ).length;
  $: trustWatch = recentRequests
    .filter((request) => ['review', 'blocked'].includes(request.trust_stage))
    .slice(0, 3);
  $: automationDigest = buildAutomationDigest({
    requests: recentRequests,
    missingAlbums,
    providerHealth: $providerHealth,
    providerStatuses: $providerStatuses,
    slskdReady: $slskdRuntimeStatus?.ready ?? false,
    isScanning: $isScanning,
    backlogRunning: $backlogStatus?.running ?? false,
    queueCount: $queue.length,
  });

  $: whileAway = buildWhileAwayMessages({
    recentResults,
    missingAlbums,
    inProgressCount,
    blockedCount,
  });

  onMount(async () => {
    try {
      const [missing, results, requests, trust, recent] = await Promise.all([
        api.getMissingSpotifyAlbums(10),
        api.getRecentTaskResults(12),
        api.listAcquisitionRequests(undefined, 32),
        api.getTrustReasonDistribution(6),
        api.getRecentlyFinalizedTracks(7),
      ]);
      missingAlbums = missing;
      recentResults = results;
      recentRequests = requests;
      trustDistribution = trust;
      recentTracks = recent;
    } finally {
      loading = false;
    }
  });

  function buildWhileAwayMessages(input: {
    recentResults: TaskResultSummary[];
    missingAlbums: SpotifyAlbumHistory[];
    inProgressCount: number;
    blockedCount: number;
  }): WhileAwayMessage[] {
    const finalized = input.recentResults.filter((result) =>
      ['Finalized', 'AlreadyPresent', 'MetadataOnly'].includes(result.disposition)
    ).length;
    const failed = input.recentResults.filter((result) =>
      ['Failed', 'Cancelled'].includes(result.disposition)
    ).length;
    const messages: WhileAwayMessage[] = [];

    if (finalized > 0) {
      messages.push({
        tone: 'steady',
        title: `${finalized} handoff${finalized === 1 ? '' : 's'} landed`,
        detail: 'Cassette finished recent library work without pulling focus away from listening.',
      });
    }

    if (input.inProgressCount > 0) {
      messages.push({
        tone: 'steady',
        title: `${input.inProgressCount} item${input.inProgressCount === 1 ? '' : 's'} still moving`,
        detail: 'Search, transfer, and verification work is still running quietly in the background.',
      });
    }

    if (input.blockedCount > 0 || failed > 0) {
      const total = input.blockedCount + failed;
      messages.push({
        tone: total > 3 ? 'action' : 'watch',
        title: `${total} lane${total === 1 ? '' : 's'} need a look`,
        detail: 'Open Workstation when you are ready to review blocked items or stalled runs.',
      });
    }

    if (input.missingAlbums.length > 0) {
      messages.push({
        tone: 'watch',
        title: `${input.missingAlbums.length} album${input.missingAlbums.length === 1 ? '' : 's'} still missing`,
        detail: 'Your inbox still knows what is absent and can keep draining it in the background.',
      });
    }

    if (messages.length === 0) {
      messages.push({
        tone: 'steady',
        title: 'Quiet room',
        detail: 'No fresh noise, no blocked work, and the collection is sitting still for now.',
      });
    }

    return messages.slice(0, 4);
  }

  function resumePlayback() {
    if ($playbackState.current_track) {
      void player.toggle();
    }
  }
</script>

<svelte:head><title>Home - Cassette</title></svelte:head>

<div class="home-page">
  <section class="home-hero">
    <div class="hero-backdrop">
      {#if current?.cover_art_path}
        <img src={coverSrc(current.cover_art_path)} alt="Current artwork" />
      {/if}
    </div>

    <div class="hero-copy">
      <div class="hero-kicker">Return ritual</div>
      <h1>{activePlayback ? current?.title : 'Back in the chair'}</h1>
      <p class="hero-summary">
        {#if activePlayback && context}
          {current?.artist}{context.album_title ? ` / ${context.album_title}` : ''}. Playback is live, the room is settled, and the inbox can wait its turn.
        {:else if current}
          {current.artist}{current.album ? ` / ${current.album}` : ''}. Cassette is ready to pick the thread back up.
        {:else}
          Playback, unfinished collection work, and service status stay in one place so the system can stay useful without getting loud.
        {/if}
      </p>

      <div class="hero-actions">
        <button class="btn btn-primary" on:click={() => goto('/collection')}>Open collection</button>
        <button class="btn btn-secondary" on:click={() => goto('/queue')}>Open queue</button>
        <button class="btn btn-ghost" on:click={() => goto('/session')}>Open session</button>
        <button class="btn btn-ghost" on:click={() => goto('/workstation')}>Open workstation</button>
        {#if current}
          <button class="btn btn-ghost" on:click={resumePlayback}>
            {$playbackState.is_playing ? 'Pause' : 'Resume'}
          </button>
        {/if}
      </div>
    </div>

    <div class="hero-side">
      <div class="hero-metric">
        <span class="hero-metric-label">Collection</span>
        <span class="hero-metric-value">{$trackCount.toLocaleString()} tracks</span>
      </div>
      <div class="hero-metric">
        <span class="hero-metric-label">Artists</span>
        <span class="hero-metric-value">{artistClusters.length.toLocaleString()} clustered</span>
      </div>
      <div class="hero-metric">
        <span class="hero-metric-label">Inbox</span>
        <span class="hero-metric-value">
          {#if $backlogStatus?.running}
            Running
          {:else}
            {missingAlbums.length} missing
          {/if}
        </span>
      </div>
      <div class="hero-metric">
        <span class="hero-metric-label">Service</span>
        <span class="hero-metric-value">{$slskdRuntimeStatus?.ready ? 'Ready' : 'Waiting'}</span>
      </div>
    </div>
  </section>

  <section class="home-band">
    <div class="band-heading">
      <div>
        <div class="section-kicker">While you were away</div>
        <h2>Plain-language recap</h2>
      </div>
      <button class="band-link" on:click={() => goto('/workstation')}>Open workstation</button>
    </div>

    {#if loading}
      <div class="summary-grid loading-grid">
        <div class="summary-card">Loading background recap...</div>
      </div>
    {:else}
      <div class="summary-grid">
        {#each whileAway as message}
          <article class="summary-card tone-{message.tone}">
            <div class="summary-tone">{message.tone}</div>
            <h3>{message.title}</h3>
            <p>{message.detail}</p>
          </article>
        {/each}
      </div>
    {/if}
  </section>

  {#if recentTracks.length > 0}
    <section class="home-band">
      <div class="band-heading">
        <div>
          <div class="section-kicker">New on the shelf</div>
          <h2>Recently arrived</h2>
        </div>
        <button class="band-link" on:click={() => goto('/collection')}>Browse collection</button>
      </div>

      <div class="arrivals-grid">
        {#each recentTracks.slice(0, 8) as track}
          <div class="arrival-card mood-card">
            {#if track.cover_art_path}
              <img class="arrival-art" src={coverSrc(track.cover_art_path)} alt="" loading="lazy" />
            {:else}
              <div class="arrival-art-ph"></div>
            {/if}

            <div class="arrival-info">
              <div class="arrival-title">{track.title}</div>
              <div class="arrival-meta">{track.artist}</div>
              {#if track.quality_tier === 'lossless_hires' || track.quality_tier === 'lossless'}
                <span class="arrival-badge">Lossless</span>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    </section>
  {/if}

  <section class="home-columns">
    <div class="column-block">
      <div class="band-heading">
        <div>
          <div class="section-kicker">Artist-first</div>
          <h2>Artist-first collection</h2>
        </div>
        <button class="band-link" on:click={() => goto('/artists')}>See all artists</button>
      </div>

      <div class="artist-stack">
        {#if topArtists.length === 0}
          <div class="stack-empty">Scan a library root and your artist clusters will show up here.</div>
        {:else}
          {#each topArtists as artist}
            <button class="artist-line mood-card" on:click={() => goto('/artists')}>
              <span class="artist-mark">{artist.primaryName[0]?.toUpperCase() ?? '?'}</span>
              <span class="artist-copy">
                <span class="artist-name">{artist.primaryName}</span>
                <span class="artist-meta">{artist.albumCount} albums / {artist.trackCount} tracks</span>
              </span>
              {#if artist.aliases.length > 1}
                <span class="artist-variants">{artist.aliases.length} variants</span>
              {/if}
            </button>
          {/each}
        {/if}
      </div>
    </div>

    <div class="column-block intelligence-block">
      <div class="band-heading">
        <div>
          <div class="section-kicker">Calm automation</div>
          <h2>What the room is carrying for you</h2>
        </div>
        <button class="band-link" on:click={() => goto('/workstation')}>Review details</button>
      </div>

      <AutomationDigestPanel
        digest={automationDigest}
        compact={true}
        showThresholdLegend={true}
        primaryHref="/workstation"
        primaryLabel="Open workstation"
        secondaryHref="/downloads"
        secondaryLabel="Open downloads"
      />

      <div class="trust-strip">
        {#if trustWatch.length > 0}
          {#each trustWatch as request}
            <div class="trust-line">
              <span class="trust-code">{request.trust_reason_code}</span>
              <span class="trust-copy">{request.trust_detail}</span>
            </div>
          {/each}
        {:else if trustDistribution.length > 0}
          {#each trustDistribution.slice(0, 3) as entry}
            <div class="trust-line">
              <span class="trust-code">{entry.reason_code}</span>
              <span class="trust-copy">{entry.label} across {entry.count} recent item{entry.count === 1 ? '' : 's'}.</span>
            </div>
          {/each}
        {:else}
          <div class="stack-empty">No trust issues are crowding the room right now.</div>
        {/if}
      </div>

      <div class="missing-stack">
        {#if missingAlbums.length === 0}
          <div class="stack-empty">No Spotify backlog albums are currently marked missing.</div>
        {:else}
          {#each missingAlbums.slice(0, 5) as album}
            <div class="missing-line">
              <span class="missing-copy">
                <span class="missing-title">{album.artist} / {album.album}</span>
                <span class="missing-meta">{album.play_count} plays / {formatDuration(album.total_ms / 1000)}</span>
              </span>
              <span class="missing-badge">missing</span>
            </div>
          {/each}
        {/if}
      </div>
    </div>
  </section>
</div>

<style>
  .home-page {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 18px;
  }

  .home-hero {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1.5fr) minmax(220px, 0.65fr);
    gap: 18px;
    min-height: 320px;
    padding: 24px;
    border: 1px solid rgba(var(--mood-accent-rgb), 0.14);
    border-radius: var(--radius-lg);
    overflow: hidden;
    background:
      radial-gradient(circle at top left, var(--mood-layer-b), transparent 45%),
      radial-gradient(circle at bottom right, var(--mood-layer-a), transparent 50%),
      linear-gradient(135deg, rgba(var(--mood-accent-rgb), 0.08), transparent 55%),
      var(--bg-card);
    transition: border-color var(--mood-shift-ms) ease;
  }

  .hero-backdrop {
    position: absolute;
    inset: 0;
    opacity: 0.22;
    pointer-events: none;
  }

  .hero-backdrop img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    filter: blur(22px) saturate(1.3);
    transform: scale(1.06);
    mix-blend-mode: luminosity;
  }

  .hero-copy,
  .hero-side {
    position: relative;
    z-index: 1;
  }

  .hero-copy {
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    gap: 12px;
    max-width: 620px;
  }

  .hero-kicker,
  .section-kicker {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--accent-bright);
    font-weight: 700;
  }

  .home-hero h1 {
    font-size: clamp(2rem, 4vw, 3.4rem);
    line-height: 0.96;
    max-width: 10ch;
    background: linear-gradient(135deg, var(--text-primary) 50%, rgba(var(--mood-accent-rgb), 0.9));
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
    transition: background var(--mood-shift-ms) ease;
  }

  .hero-summary {
    max-width: 56ch;
    font-size: 0.95rem;
    line-height: 1.75;
    color: var(--text-secondary);
  }

  .hero-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin-top: 4px;
  }

  .btn-primary {
    background: linear-gradient(135deg, rgba(var(--mood-accent-rgb), 1) 0%, var(--primary) 100%);
    color: var(--bg-deep);
    border: none;
    box-shadow: 0 4px 14px rgba(var(--mood-accent-rgb), 0.3);
    transition: background var(--mood-shift-ms) ease, box-shadow var(--mood-shift-ms) ease, filter 0.15s ease;
  }

  .btn-primary:hover {
    filter: brightness(1.1);
  }

  .hero-side {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    align-content: end;
    gap: 10px;
  }

  .hero-metric {
    padding: 12px 14px;
    border-radius: var(--radius);
    border: 1px solid rgba(var(--mood-accent-rgb), 0.12);
    background: rgba(6, 8, 16, 0.55);
    backdrop-filter: blur(10px);
    transition: border-color var(--mood-shift-ms) ease;
  }

  .hero-metric-label,
  .lane-label {
    display: block;
    font-size: 0.66rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
    margin-bottom: 5px;
  }

  .hero-metric-value {
    font-size: 0.9rem;
    color: var(--text-primary);
  }

  .home-band,
  .column-block {
    padding: 18px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-card);
  }

  .band-heading {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 14px;
  }

  .band-heading h2 {
    margin-top: 4px;
    font-size: 1.18rem;
  }

  .band-link {
    color: var(--text-accent);
    font-size: 0.78rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .summary-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(190px, 1fr));
    gap: 12px;
  }

  .summary-card {
    padding: 14px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.02), transparent), var(--bg-base);
  }

  .summary-card h3 {
    margin: 8px 0 6px;
    font-size: 0.95rem;
  }

  .summary-card p {
    font-size: 0.82rem;
    line-height: 1.7;
    color: var(--text-secondary);
  }

  .summary-tone {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }

  .tone-steady {
    border-color: color-mix(in srgb, var(--primary) 28%, var(--border));
  }

  .tone-watch {
    border-color: color-mix(in srgb, var(--warning) 30%, var(--border));
  }

  .tone-action {
    border-color: color-mix(in srgb, var(--error) 36%, var(--border));
  }

  .arrivals-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 10px;
  }

  .arrival-card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    border-radius: var(--radius);
    overflow: hidden;
    background: var(--bg-card);
    border: 1px solid var(--border);
    cursor: pointer;
    transition: border-color 0.15s, transform 0.15s;
  }

  .arrival-card:hover {
    border-color: var(--border-active);
    transform: translateY(-1px);
  }

  .arrival-art {
    width: 100%;
    aspect-ratio: 1;
    object-fit: cover;
    display: block;
  }

  .arrival-art-ph {
    width: 100%;
    aspect-ratio: 1;
    background: var(--bg-active);
  }

  .arrival-info {
    padding: 0 10px 10px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .arrival-title {
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .arrival-meta {
    font-size: 0.72rem;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .arrival-badge {
    font-size: 0.62rem;
    color: var(--status-ok);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    margin-top: 2px;
  }

  .home-columns {
    display: grid;
    grid-template-columns: minmax(0, 1.1fr) minmax(300px, 0.9fr);
    gap: 18px;
  }

  .artist-stack,
  .missing-stack {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .artist-line,
  .missing-line {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 12px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: var(--bg-base);
  }

  .artist-line:hover {
    border-color: var(--border-active);
  }

  .artist-mark {
    width: 38px;
    height: 38px;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(135deg, rgba(247, 180, 92, 0.16), rgba(139, 180, 212, 0.2));
    color: var(--accent-bright);
    flex-shrink: 0;
  }

  .artist-copy,
  .missing-copy {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    min-width: 0;
  }

  .artist-name,
  .missing-title {
    font-size: 0.88rem;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .artist-meta,
  .missing-meta {
    font-size: 0.74rem;
    color: var(--text-secondary);
  }

  .artist-variants,
  .missing-badge {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }

  .trust-strip {
    display: grid;
    gap: 8px;
    margin-bottom: 14px;
  }

  .trust-line {
    display: grid;
    gap: 4px;
    padding: 10px 12px;
    border: 1px solid color-mix(in srgb, var(--accent) 26%, var(--border));
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--accent) 7%, var(--bg-base));
  }

  .trust-code {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }

  .trust-copy {
    font-size: 0.82rem;
    color: var(--text-secondary);
  }

  .stack-empty {
    padding: 16px 0 4px;
    color: var(--text-secondary);
    font-size: 0.84rem;
  }

  .loading-grid {
    grid-template-columns: 1fr;
  }

  @media (max-width: 1100px) {
    .home-hero,
    .home-columns {
      grid-template-columns: 1fr;
    }

    .hero-side,
    .lane-metrics {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
