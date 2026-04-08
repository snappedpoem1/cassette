<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import { buildArtistClusters } from '$lib/artist-clusters';
  import { api, type AcquisitionRequestListItem, type SpotifyAlbumHistory, type TaskResultSummary, type TrustReasonDistributionEntry } from '$lib/api/tauri';
  import { artists, trackCount } from '$lib/stores/library';
  import { backlogStatus, slskdRuntimeStatus } from '$lib/stores/downloads';
  import { currentTrack, nowPlayingContext, playbackState, player } from '$lib/stores/player';
  import { formatDuration, coverSrc } from '$lib/utils';

  interface WhileAwayMessage {
    tone: 'steady' | 'watch' | 'action';
    title: string;
    detail: string;
  }

  let missingAlbums: SpotifyAlbumHistory[] = [];
  let recentResults: TaskResultSummary[] = [];
  let recentRequests: AcquisitionRequestListItem[] = [];
  let trustDistribution: TrustReasonDistributionEntry[] = [];
  let loading = true;
  let SessionComposer: typeof import('$lib/components/SessionComposer.svelte').default | null = null;

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

  $: whileAway = buildWhileAwayMessages({
    recentResults,
    missingAlbums,
    inProgressCount,
    blockedCount,
  });

  onMount(async () => {
    try {
      const [missing, results, requests, trust] = await Promise.all([
        api.getMissingSpotifyAlbums(10),
        api.getRecentTaskResults(12),
        api.listAcquisitionRequests(undefined, 32),
        api.getTrustReasonDistribution(6),
      ]);
      missingAlbums = missing;
      recentResults = results;
      recentRequests = requests;
      trustDistribution = trust;
    } finally {
      loading = false;
    }

    const module = await import('$lib/components/SessionComposer.svelte');
    SessionComposer = module.default;
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
        title: `${finalized} library handoff${finalized === 1 ? '' : 's'} completed`,
        detail: 'Cassette finished recent acquisition or repair work without needing you in the loop.',
      });
    }

    if (input.inProgressCount > 0) {
      messages.push({
        tone: 'steady',
        title: `${input.inProgressCount} request${input.inProgressCount === 1 ? '' : 's'} still moving`,
        detail: 'Search, download, and verification work is still active in the background.',
      });
    }

    if (input.blockedCount > 0 || failed > 0) {
      const total = input.blockedCount + failed;
      messages.push({
        tone: total > 3 ? 'action' : 'watch',
        title: `${total} lane${total === 1 ? '' : 's'} need attention`,
        detail: 'Open Downloads to review blocked requests, failed runs, or items waiting on a decision.',
      });
    }

    if (input.missingAlbums.length > 0) {
      messages.push({
        tone: 'watch',
        title: `${input.missingAlbums.length} Spotify backlog album${input.missingAlbums.length === 1 ? '' : 's'} still missing`,
        detail: 'The command center knows what is still absent and can keep draining it in the background.',
      });
    }

    if (messages.length === 0) {
      messages.push({
        tone: 'steady',
        title: 'Quiet desk',
        detail: 'No fresh download noise, no blocked work, and the collection is sitting still for now.',
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

<svelte:head><title>Home · Cassette</title></svelte:head>

<div class="home-page">
  <section class="home-hero">
    <div class="hero-backdrop">
      {#if current?.cover_art_path}
        <img src={coverSrc(current.cover_art_path)} alt="Current artwork" />
      {/if}
    </div>

    <div class="hero-copy">
      <div class="hero-kicker">Music-first desktop</div>
      <h1>{activePlayback ? current?.title : 'Back in the chair'}</h1>
      <p class="hero-summary">
        {#if activePlayback && context}
          {current?.artist}{context.album_title ? ` · ${context.album_title}` : ''}. Playback is live and the background lanes are still keeping score.
        {:else if current}
          {current.artist}{current.album ? ` · ${current.album}` : ''}. Cassette is ready to pick the thread back up.
        {:else}
          Playback, missing music, and service health stay in one place so the system can keep moving without becoming noise.
        {/if}
      </p>

      <div class="hero-actions">
        <button class="btn btn-primary" on:click={() => goto('/artists')}>Open artists</button>
        <button class="btn btn-ghost" on:click={() => goto('/downloads')}>Open downloads</button>
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
        <span class="hero-metric-label">Backlog</span>
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
        <span class="hero-metric-value">{$slskdRuntimeStatus?.ready ? 'slskd ready' : 'slskd idle'}</span>
      </div>
    </div>
  </section>

  <section class="home-band">
    <div class="band-heading">
      <div>
        <div class="section-kicker">While you were away</div>
        <h2>Plain-language system recap</h2>
      </div>
      <button class="band-link" on:click={() => goto('/downloads')}>Open command center</button>
    </div>

    {#if loading}
      <div class="summary-grid loading-grid">
        <div class="summary-card">Loading background summary...</div>
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

  <section class="home-columns">
    <div class="column-block">
      <div class="band-heading">
        <div>
          <div class="section-kicker">Artist-first collection</div>
          <h2>Where the library actually starts</h2>
        </div>
        <button class="band-link" on:click={() => goto('/artists')}>See all artists</button>
      </div>

      <div class="artist-stack">
        {#if topArtists.length === 0}
          <div class="stack-empty">Scan a library root and your artist clusters will show up here.</div>
        {:else}
          {#each topArtists as artist}
            <button class="artist-line" on:click={() => goto('/artists')}>
              <span class="artist-mark">{artist.primaryName[0]?.toUpperCase() ?? '?'}</span>
              <span class="artist-copy">
                <span class="artist-name">{artist.primaryName}</span>
                <span class="artist-meta">{artist.albumCount} albums · {artist.trackCount} tracks</span>
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
          <div class="section-kicker">Collection intelligence</div>
          <h2>Missing work, live work, blocked work</h2>
        </div>
        <button class="band-link" on:click={() => goto('/downloads')}>Manage lanes</button>
      </div>

      <div class="lane-metrics">
        <div class="lane-metric">
          <span class="lane-label">Missing</span>
          <strong>{missingAlbums.length}</strong>
        </div>
        <div class="lane-metric">
          <span class="lane-label">In progress</span>
          <strong>{inProgressCount}</strong>
        </div>
        <div class="lane-metric">
          <span class="lane-label">Blocked</span>
          <strong>{blockedCount}</strong>
        </div>
        <div class="lane-metric">
          <span class="lane-label">Completed</span>
          <strong>{completedCount}</strong>
        </div>
      </div>

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
              <span class="trust-copy">{entry.label} across {entry.count} recent request{entry.count === 1 ? '' : 's'}.</span>
            </div>
          {/each}
        {:else}
          <div class="stack-empty">No trust-ledger issues are crowding the desk right now.</div>
        {/if}
      </div>

      <div class="missing-stack">
        {#if missingAlbums.length === 0}
          <div class="stack-empty">No Spotify backlog albums are currently flagged missing.</div>
        {:else}
          {#each missingAlbums.slice(0, 5) as album}
            <div class="missing-line">
              <span class="missing-copy">
                <span class="missing-title">{album.artist} · {album.album}</span>
                <span class="missing-meta">{album.play_count} plays · {formatDuration(album.total_ms / 1000)}</span>
              </span>
              <span class="missing-badge">missing</span>
            </div>
          {/each}
        {/if}
      </div>
    </div>
  </section>

  {#if SessionComposer}
    <svelte:component this={SessionComposer} />
  {/if}
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
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  overflow: hidden;
  background:
    radial-gradient(circle at top left, rgba(247, 180, 92, 0.16), transparent 40%),
    linear-gradient(135deg, rgba(139, 180, 212, 0.14), transparent 55%),
    var(--bg-card);
}

.hero-backdrop {
  position: absolute;
  inset: 0;
  opacity: 0.18;
  pointer-events: none;
}

.hero-backdrop img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  filter: blur(22px) saturate(1.2);
  transform: scale(1.06);
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
  color: var(--accent);
  font-weight: 700;
}

.home-hero h1 {
  font-size: clamp(2rem, 4vw, 3.4rem);
  line-height: 0.96;
  max-width: 10ch;
}

.hero-summary {
  max-width: 54ch;
  font-size: 0.92rem;
  line-height: 1.7;
  color: var(--text-secondary);
}

.hero-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin-top: 4px;
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
  border: 1px solid var(--border);
  background: rgba(6, 8, 16, 0.52);
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
  font-size: 0.88rem;
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
  color: var(--primary);
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
  font-size: 0.8rem;
  line-height: 1.6;
  color: var(--text-secondary);
}

.summary-tone {
  font-size: 0.68rem;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--text-muted);
}

.tone-steady { border-color: color-mix(in srgb, var(--primary) 28%, var(--border)); }
.tone-watch { border-color: color-mix(in srgb, var(--warning) 30%, var(--border)); }
.tone-action { border-color: color-mix(in srgb, var(--error) 36%, var(--border)); }

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
  color: var(--accent);
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

.lane-metrics {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 10px;
  margin-bottom: 12px;
}

.lane-metric {
  padding: 12px;
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: var(--bg-base);
}

.lane-metric strong {
  font-size: 1.3rem;
  color: var(--text-primary);
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
