<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type CollectionStats } from '$lib/api/tauri';

  let stats: CollectionStats | null = null;
  let loading = true;
  let errorMessage = '';

  $: totalTracks = stats?.total_tracks ?? 0;
  $: totalAlbums = stats?.total_albums ?? 0;
  $: totalDurationSecs = stats?.total_duration_secs ?? 0;
  $: losslessCount = stats?.lossless_count ?? 0;
  $: hiresCount = stats?.hires_count ?? 0;

  $: decadeRows = buildRows(stats?.by_decade ?? {});
  $: formatRows = buildRows(stats?.by_format ?? {});

  $: maxDecadeCount = decadeRows.length ? decadeRows[0].count : 0;
  $: maxFormatCount = formatRows.length ? formatRows[0].count : 0;

  onMount(async () => {
    await loadStats();
  });

  async function loadStats() {
    loading = true;
    errorMessage = '';
    try {
      stats = await api.getCollectionStats();
    } catch {
      stats = null;
      errorMessage = 'Unable to load collection stats right now.';
    } finally {
      loading = false;
    }
  }

  function buildRows(record: Record<string, number>) {
    return Object.entries(record)
      .map(([label, count]) => ({ label, count }))
      .filter((row) => row.count > 0)
      .sort((a, b) => b.count - a.count || a.label.localeCompare(b.label));
  }

  function formatWhole(n: number): string {
    return Math.round(n).toLocaleString();
  }

  function formatHours(seconds: number): string {
    if (!isFinite(seconds) || seconds <= 0) {
      return '0h';
    }
    const hours = Math.floor(seconds / 3600);
    return `${hours.toLocaleString()}h`;
  }

  function percentOfTotal(count: number): string {
    if (!totalTracks) {
      return '0%';
    }
    return `${Math.round((count / totalTracks) * 100)}%`;
  }

  function widthPercent(count: number, maxCount: number): string {
    if (!maxCount) {
      return '0%';
    }
    return `${Math.max(8, Math.round((count / maxCount) * 100))}%`;
  }
</script>

<svelte:head><title>Collection · Cassette</title></svelte:head>

<div class="collection-page">
  <div class="page-header">
    <h2>Collection</h2>
    <button class="btn btn-ghost" on:click={loadStats} disabled={loading}>Refresh</button>
  </div>

  {#if loading}
    <div class="empty-state">
      <div class="spinner"></div>
      <div class="empty-body">Loading collection statistics...</div>
    </div>
  {:else if errorMessage}
    <div class="empty-state">
      <div class="empty-title">Collection stats unavailable</div>
      <div class="empty-body">{errorMessage}</div>
    </div>
  {:else}
    <section class="totals-grid">
      <article class="card total-card">
        <div class="total-label">Total Tracks</div>
        <div class="total-value">{formatWhole(totalTracks)}</div>
      </article>
      <article class="card total-card">
        <div class="total-label">Total Albums</div>
        <div class="total-value">{formatWhole(totalAlbums)}</div>
      </article>
      <article class="card total-card">
        <div class="total-label">Total Duration</div>
        <div class="total-value">{formatHours(totalDurationSecs)}</div>
      </article>
      <article class="card total-card">
        <div class="total-label">Lossless / Hi-Res</div>
        <div class="total-value">{formatWhole(losslessCount)} / {formatWhole(hiresCount)}</div>
      </article>
    </section>

    <section class="stats-grid">
      <article class="card stat-panel">
        <div class="panel-head">
          <h3>Decade Distribution</h3>
          <span class="panel-hint">tracks by release decade</span>
        </div>

        {#if decadeRows.length === 0}
          <p class="panel-empty">No decade metadata available yet.</p>
        {:else}
          <div class="bar-list">
            {#each decadeRows as row}
              <div class="bar-row">
                <div class="bar-meta">
                  <span class="bar-label">{row.label}</span>
                  <span class="bar-value">{row.count.toLocaleString()} · {percentOfTotal(row.count)}</span>
                </div>
                <div class="bar-track">
                  <div class="bar-fill" style:width={widthPercent(row.count, maxDecadeCount)}></div>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </article>

      <article class="card stat-panel">
        <div class="panel-head">
          <h3>Format Breakdown</h3>
          <span class="panel-hint">tracks by audio format</span>
        </div>

        {#if formatRows.length === 0}
          <p class="panel-empty">No format data available yet.</p>
        {:else}
          <div class="bar-list">
            {#each formatRows as row}
              <div class="bar-row">
                <div class="bar-meta">
                  <span class="bar-label">{row.label.toUpperCase()}</span>
                  <span class="bar-value">{row.count.toLocaleString()} · {percentOfTotal(row.count)}</span>
                </div>
                <div class="bar-track">
                  <div class="bar-fill format" style:width={widthPercent(row.count, maxFormatCount)}></div>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </article>
    </section>
  {/if}
</div>

<style>
.collection-page {
  display: flex;
  flex-direction: column;
  min-height: 100%;
  background: var(--bg-base);
}

.totals-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 10px;
  padding: 12px;
}

.total-card {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-height: 94px;
  justify-content: center;
}

.total-label {
  font-size: 0.72rem;
  letter-spacing: 0.07em;
  text-transform: uppercase;
  color: var(--text-muted);
}

.total-value {
  font-size: 1.25rem;
  font-weight: 700;
  color: var(--text-primary);
}

.stats-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
  padding: 0 12px 12px;
}

.stat-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.panel-head {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  align-items: baseline;
}

.panel-head h3 {
  font-size: 0.95rem;
  color: var(--text-primary);
}

.panel-hint {
  font-size: 0.72rem;
  color: var(--text-muted);
}

.panel-empty {
  margin: 8px 0 4px;
  color: var(--text-muted);
  font-size: 0.8rem;
}

.bar-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
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
  font-size: 0.78rem;
  color: var(--text-secondary);
}

.bar-value {
  font-size: 0.74rem;
  color: var(--text-muted);
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
  min-width: 8px;
  border-radius: 999px;
  background: linear-gradient(90deg, var(--primary), rgba(139, 180, 212, 0.5));
}

.bar-fill.format {
  background: linear-gradient(90deg, var(--accent), rgba(247, 180, 92, 0.45));
}

@media (max-width: 1100px) {
  .totals-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .stats-grid {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 640px) {
  .total-value {
    font-size: 1.05rem;
  }

  .panel-head {
    flex-direction: column;
    align-items: flex-start;
  }
}
</style>
