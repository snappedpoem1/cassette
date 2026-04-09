<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import { buildAutomationDigest } from '$lib/automation-digest';
  import AutomationDigestPanel from '$lib/components/AutomationDigestPanel.svelte';
  import { api, type AcquisitionRequestListItem, type ProviderStatus, type SpotifyAlbumHistory, type TaskResultSummary } from '$lib/api/tauri';
  import { providerHealth, slskdRuntimeStatus } from '$lib/stores/downloads';
  import { isScanning } from '$lib/stores/library';
  import { queue } from '$lib/stores/queue';

  let missingAlbums: SpotifyAlbumHistory[] = [];
  let requests: AcquisitionRequestListItem[] = [];
  let recentResults: TaskResultSummary[] = [];
  let providerStatuses: ProviderStatus[] = [];
  let loading = true;

  $: runningCount = requests.filter((request) =>
    ['queued', 'submitted', 'in_progress'].includes(request.status)
  ).length;
  $: reviewCount = requests.filter((request) => request.status === 'reviewing').length;
  $: blockedCount = requests.filter((request) =>
    ['failed', 'cancelled'].includes(request.status)
  ).length;
  $: readyProviders = providerStatuses.filter((provider) => provider.configured).length;
  $: digest = buildAutomationDigest({
    requests,
    missingAlbums,
    providerHealth: $providerHealth,
    providerStatuses,
    slskdReady: $slskdRuntimeStatus?.ready ?? false,
    isScanning: $isScanning,
    backlogRunning: runningCount > 0,
    queueCount: $queue.length,
  });

  onMount(async () => {
    try {
      const [missing, nextRequests, results, providers] = await Promise.all([
        api.getMissingSpotifyAlbums(8),
        api.listAcquisitionRequests(undefined, 32),
        api.getRecentTaskResults(10),
        api.getProviderStatuses(),
      ]);
      missingAlbums = missing;
      requests = nextRequests;
      recentResults = results;
      providerStatuses = providers;
    } finally {
      loading = false;
    }
  });

  const surfaceCards = [
    {
      title: 'Downloads',
      route: '/downloads',
      label: 'Background work',
      body: 'Review missing items, live runs, blocked lanes, and recent handoffs without dragging that detail into the listening shell.',
    },
    {
      title: 'Import',
      route: '/import',
      label: 'Intake',
      body: 'Bring Spotify history and desired-track files into Cassette’s intake lane.',
    },
    {
      title: 'History',
      route: '/history',
      label: 'Memory',
      body: 'Look back at recent arrivals and most-played material.',
    },
    {
      title: 'Library Tools',
      route: '/tools',
      label: 'Repair',
      body: 'Handle organize, duplicate cleanup, metadata fixes, and staging work when you mean to operate on the collection.',
    },
    {
      title: 'Settings',
      route: '/settings',
      label: 'System',
      body: 'Manage roots, providers, enrichment, visuals, and safe extension boundaries.',
    },
  ];
</script>

<svelte:head><title>Workstation - Cassette</title></svelte:head>

<div class="workstation-page">
  <section class="workstation-hero card">
    <div class="section-kicker">Control ritual</div>
    <div class="workstation-hero-row">
      <div class="workstation-copy">
        <h1>Review the machinery without letting it become the room</h1>
        <p>
          Workstation holds the background detail: intake, blocked work, health checks, and repair
          tools. Listening stays out front. This is where you come when you want to intervene on
          purpose.
        </p>
      </div>
      <div class="workstation-actions">
        <button class="btn btn-primary" on:click={() => goto('/downloads')}>Open downloads</button>
        <button class="btn btn-secondary" on:click={() => goto('/')}>Back to home</button>
      </div>
    </div>

    <div class="workstation-stats">
      <div class="ws-stat">
        <span class="ws-label">Missing albums</span>
        <strong>{missingAlbums.length}</strong>
      </div>
      <div class="ws-stat">
        <span class="ws-label">Live work</span>
        <strong>{runningCount}</strong>
      </div>
      <div class="ws-stat">
        <span class="ws-label">Needs review</span>
        <strong>{reviewCount + blockedCount}</strong>
      </div>
      <div class="ws-stat">
        <span class="ws-label">Configured providers</span>
        <strong>{readyProviders}</strong>
      </div>
    </div>
  </section>

  <section class="surface-grid">
    {#each surfaceCards as card}
      <button class="surface-card card" on:click={() => goto(card.route)}>
        <span class="surface-label">{card.label}</span>
        <h2>{card.title}</h2>
        <p>{card.body}</p>
      </button>
    {/each}
  </section>

  <section class="workstation-columns">
    <article class="card workstation-panel">
      <div class="panel-head">
        <div>
          <div class="section-kicker">Digest boundary</div>
          <h2>How the system decides to surface itself</h2>
        </div>
        <button class="panel-link" on:click={() => goto('/downloads')}>Open downloads</button>
      </div>

      <AutomationDigestPanel
        digest={digest}
        compact={true}
        showThresholdLegend={true}
        primaryHref="/downloads"
        primaryLabel="Open downloads"
        secondaryHref="/settings"
        secondaryLabel="Open settings"
      />
    </article>

    <article class="card workstation-panel">
      <div class="panel-head">
        <div>
          <div class="section-kicker">Recent handoffs</div>
          <h2>Finished without pulling focus</h2>
        </div>
        <button class="panel-link" on:click={() => goto('/history')}>Open history</button>
      </div>

      {#if recentResults.length === 0}
        <div class="panel-empty">No finished handoffs recorded yet.</div>
      {:else}
        <div class="result-stack">
          {#each recentResults.slice(0, 6) as result}
            <div class="result-line">
              <span class="result-id">{result.task_id}</span>
              <span class="result-meta">
                {result.provider || 'unknown provider'} / {result.disposition}
                {#if result.error}
                  {' / '}{result.error}
                {/if}
              </span>
            </div>
          {/each}
        </div>
      {/if}
    </article>
  </section>

  <section class="surface-grid">
    <button class="surface-card card" on:click={() => goto('/downloads')}>
      <span class="surface-label">Review</span>
      <h2>Inbox and blocked lanes</h2>
      <p>Use Downloads for approvals, stalled requests, candidate review, and recent completions.</p>
    </button>
    <button class="surface-card card" on:click={() => goto('/import')}>
      <span class="surface-label">Intake</span>
      <h2>Bring music in on purpose</h2>
      <p>Spotify history and desired-track intake live here, away from the listening shell.</p>
    </button>
    <button class="surface-card card" on:click={() => goto('/tools')}>
      <span class="surface-label">Repair</span>
      <h2>Keep the collection clean</h2>
      <p>Organization, duplicate cleanup, metadata correction, and staging all stay in this room.</p>
    </button>
    <button class="surface-card card" on:click={() => goto('/settings')}>
      <span class="surface-label">System</span>
      <h2>Adjust boundaries and services</h2>
      <p>Provider keys, visual preferences, policy profiles, and extension boundaries live here.</p>
    </button>
  </section>
</div>

<style>
  .workstation-page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 18px;
  }

  .workstation-hero,
  .workstation-panel {
    padding: 20px;
  }

  .section-kicker {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--accent-bright);
    font-weight: 700;
  }

  .workstation-hero {
    display: grid;
    gap: 16px;
  }

  .workstation-hero-row {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
  }

  .workstation-copy {
    display: grid;
    gap: 8px;
    max-width: 64ch;
  }

  .workstation-copy h1 {
    font-size: clamp(1.9rem, 4vw, 3rem);
    line-height: 0.98;
  }

  .workstation-copy p {
    color: var(--text-secondary);
    line-height: 1.75;
    font-size: 0.9rem;
  }

  .workstation-actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  .workstation-stats {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
  }

  .ws-stat {
    padding: 12px 14px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: var(--bg-base);
  }

  .ws-label {
    display: block;
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
    margin-bottom: 6px;
  }

  .ws-stat strong {
    font-size: 1.2rem;
    color: var(--text-primary);
  }

  .surface-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 12px;
  }

  .surface-card {
    padding: 18px;
    text-align: left;
    display: grid;
    gap: 8px;
    transition: border-color 0.15s, transform 0.15s;
  }

  .surface-card:hover {
    border-color: var(--border-active);
    transform: translateY(-1px);
  }

  .surface-label {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }

  .surface-card h2 {
    font-size: 1rem;
    color: var(--text-primary);
  }

  .surface-card p {
    color: var(--text-secondary);
    font-size: 0.82rem;
    line-height: 1.65;
  }

  .workstation-columns {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
  }

  .panel-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 14px;
  }

  .panel-head h2 {
    margin-top: 4px;
  }

  .panel-link {
    color: var(--text-accent);
    font-size: 0.78rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .panel-empty {
    color: var(--text-secondary);
    font-size: 0.84rem;
  }

  .result-stack {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .result-line {
    display: grid;
    gap: 2px;
    padding: 10px 12px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: var(--bg-base);
  }

  .result-meta {
    font-size: 0.8rem;
    color: var(--text-secondary);
  }

  .result-id {
    font-size: 0.74rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  @media (max-width: 1100px) {
    .workstation-hero-row,
    .workstation-columns {
      grid-template-columns: 1fr;
      display: grid;
    }

    .workstation-stats {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  @media (max-width: 680px) {
    .workstation-stats {
      grid-template-columns: 1fr;
    }
  }
</style>
