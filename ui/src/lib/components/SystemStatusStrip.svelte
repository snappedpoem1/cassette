<script lang="ts">
  import { backlogStatus, providerHealth, providerStatuses, slskdRuntimeStatus } from '$lib/stores/downloads';
  import { isScanning, scanProgress } from '$lib/stores/library';
  import { queue } from '$lib/stores/queue';

  $: configuredProviders = $providerStatuses.filter((provider) => provider.configured);
  $: healthyProviders = Object.values($providerHealth).filter((provider) => provider.status !== 'Down');
  $: downProviders = Object.values($providerHealth).filter((provider) => provider.status === 'Down');
  $: scanLabel = $isScanning
    ? `Scanning ${$scanProgress?.scanned?.toLocaleString() ?? 0}${$scanProgress?.total ? ` / ${$scanProgress.total.toLocaleString()}` : ''}`
    : 'Scan idle';
  $: queueLabel = $queue.length > 0 ? `${$queue.length} queued` : 'Queue clear';
  $: backlogLabel = $backlogStatus?.running
    ? `Backlog running${$backlogStatus.current_album ? `: ${$backlogStatus.current_album}` : ''}`
    : 'Backlog idle';
  $: serviceLabel = $slskdRuntimeStatus?.ready
    ? 'slskd ready'
    : $slskdRuntimeStatus?.binary_found
      ? 'slskd down'
      : 'slskd missing';
</script>

<div class="status-strip">
  <div class="status-pill" class:is-busy={$isScanning}>
    <span class="status-label">Scan</span>
    <span class="status-value">{scanLabel}</span>
  </div>

  <div class="status-pill" class:is-busy={$queue.length > 0}>
    <span class="status-label">Queue</span>
    <span class="status-value">{queueLabel}</span>
  </div>

  <div class="status-pill" class:is-busy={$backlogStatus?.running}>
    <span class="status-label">Backlog</span>
    <span class="status-value">{backlogLabel}</span>
  </div>

  <div class="status-pill" class:is-down={!$slskdRuntimeStatus?.ready}>
    <span class="status-label">Service</span>
    <span class="status-value">{serviceLabel}</span>
  </div>

  <div class="status-pill" class:is-down={downProviders.length > 0}>
    <span class="status-label">Providers</span>
    <span class="status-value">
      {#if Object.keys($providerHealth).length > 0}
        {healthyProviders.length} healthy / {downProviders.length} down
      {:else}
        {configuredProviders.length} configured
      {/if}
    </span>
  </div>
</div>

<style>
.status-strip {
  display: flex;
  gap: 8px;
  padding: 8px 12px;
  overflow-x: auto;
  border-top: 1px solid var(--border-dim);
  border-bottom: 1px solid var(--border-dim);
  background:
    linear-gradient(180deg, rgba(247, 180, 92, 0.04), transparent 65%),
    linear-gradient(90deg, rgba(139, 180, 212, 0.06), transparent 45%),
    var(--bg-deep);
}

.status-pill {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: max-content;
  padding: 6px 10px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: color-mix(in srgb, var(--bg-card) 78%, transparent);
}

.status-pill.is-busy {
  border-color: color-mix(in srgb, var(--accent) 42%, var(--border));
}

.status-pill.is-down {
  border-color: color-mix(in srgb, var(--error) 44%, var(--border));
}

.status-label {
  font-size: 0.64rem;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--text-muted);
  font-weight: 700;
}

.status-value {
  font-size: 0.74rem;
  color: var(--text-primary);
  white-space: nowrap;
}
</style>
