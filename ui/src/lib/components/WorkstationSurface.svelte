<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import { buildAutomationDigest } from '$lib/automation-digest';
  import AutomationDigestPanel from '$lib/components/AutomationDigestPanel.svelte';
  import {
    api,
    type AcquisitionRequestListItem,
    type ProviderStatus,
    type SpotifyAlbumHistory,
    type TaskResultSummary,
  } from '$lib/api/tauri';
  import { providerHealth, slskdRuntimeStatus } from '$lib/stores/downloads';
  import { isScanning } from '$lib/stores/library';
  import { queue } from '$lib/stores/queue';

  export let compact = false;

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
    } catch {
      missingAlbums = [];
      requests = [];
      recentResults = [];
      providerStatuses = [];
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
      body: 'Bring Spotify history and desired-track files into Cassette intake without breaking the listening room.',
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

<div class="workstation-surface" class:compact>
  <section class="ws-hero">
    <div class="section-kicker">Control deck</div>
    <div class="ws-hero-row">
      <div class="ws-copy">
        <h2>Search, progress, review, and repair stay here</h2>
        <p>
          This is the machine room: clean, deliberate, and close at hand without taking over the
          listening shell.
        </p>
      </div>
      <div class="ws-actions">
        <button class="btn btn-primary" on:click={() => goto('/downloads')}>Downloads</button>
        <button class="btn btn-secondary" on:click={() => goto('/import')}>Import</button>
      </div>
    </div>

    <div class="ws-stats">
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

  <section class="ws-grid">
    {#each surfaceCards as card}
      <button class="ws-card" on:click={() => goto(card.route)}>
        <span class="ws-card-label">{card.label}</span>
        <h3>{card.title}</h3>
        <p>{card.body}</p>
      </button>
    {/each}
  </section>

  <section class="ws-columns">
    <article class="ws-panel">
      <div class="ws-panel-head">
        <div>
          <div class="section-kicker">Digest boundary</div>
          <h3>What deserves your attention</h3>
        </div>
        <button class="ws-link" on:click={() => goto('/downloads')}>Downloads</button>
      </div>

      <AutomationDigestPanel
        digest={digest}
        compact={true}
        showThresholdLegend={true}
        primaryHref="/downloads"
        primaryLabel="Open downloads"
        secondaryHref="/settings"
        secondaryLabel="Settings"
      />
    </article>

    <article class="ws-panel">
      <div class="ws-panel-head">
        <div>
          <div class="section-kicker">Recent handoffs</div>
          <h3>Finished in the background</h3>
        </div>
        <button class="ws-link" on:click={() => goto('/history')}>History</button>
      </div>

      {#if loading}
        <div class="ws-empty">Loading recent handoffs...</div>
      {:else if recentResults.length === 0}
        <div class="ws-empty">No finished handoffs recorded yet.</div>
      {:else}
        <div class="ws-result-stack">
          {#each recentResults.slice(0, 6) as result}
            <div class="ws-result-line">
              <span class="ws-result-id">{result.task_id}</span>
              <span class="ws-result-meta">
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
</div>

<style>
  .workstation-surface {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-height: 100%;
  }

  .workstation-surface.compact {
    gap: 12px;
  }

  .section-kicker {
    font-size: 0.66rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--accent-bright);
    font-weight: 700;
  }

  .ws-hero,
  .ws-card,
  .ws-panel {
    border: 1px solid var(--border);
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.014), transparent 46%),
      color-mix(in srgb, var(--bg-card) 92%, var(--bg-base));
  }

  .ws-hero,
  .ws-panel {
    padding: 16px;
    border-radius: var(--radius);
  }

  .ws-hero {
    display: grid;
    gap: 14px;
  }

  .ws-hero-row {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
  }

  .ws-copy {
    display: grid;
    gap: 8px;
    max-width: 52ch;
  }

  .ws-copy h2 {
    font-size: clamp(1.3rem, 3vw, 2rem);
    line-height: 1.02;
  }

  .ws-copy p {
    color: var(--text-secondary);
    font-size: 0.84rem;
    line-height: 1.7;
  }

  .ws-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .ws-stats {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
  }

  .ws-stat {
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: rgba(6, 8, 16, 0.55);
  }

  .ws-label {
    display: block;
    margin-bottom: 5px;
    font-size: 0.64rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }

  .ws-stat strong {
    font-size: 1rem;
    color: var(--text-primary);
  }

  .ws-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
    gap: 10px;
  }

  .ws-card {
    padding: 14px;
    border-radius: var(--radius);
    text-align: left;
    display: grid;
    gap: 6px;
    transition: border-color 0.15s, transform 0.15s;
  }

  .ws-card:hover {
    border-color: var(--border-active);
    transform: translateY(-1px);
  }

  .ws-card-label {
    font-size: 0.66rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }

  .ws-card h3 {
    font-size: 0.96rem;
  }

  .ws-card p {
    color: var(--text-secondary);
    font-size: 0.78rem;
    line-height: 1.6;
  }

  .ws-columns {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }

  .ws-panel {
    display: grid;
    gap: 12px;
  }

  .ws-panel-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }

  .ws-link {
    color: var(--text-accent);
    font-size: 0.74rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .ws-empty {
    color: var(--text-secondary);
    font-size: 0.82rem;
  }

  .ws-result-stack {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .ws-result-line {
    display: grid;
    gap: 3px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: rgba(6, 8, 16, 0.5);
  }

  .ws-result-id {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }

  .ws-result-meta {
    color: var(--text-secondary);
    font-size: 0.78rem;
  }

  @media (max-width: 1040px) {
    .ws-columns,
    .ws-hero-row {
      display: grid;
      grid-template-columns: 1fr;
    }

    .ws-stats {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  @media (max-width: 680px) {
    .ws-stats {
      grid-template-columns: 1fr;
    }
  }
</style>
