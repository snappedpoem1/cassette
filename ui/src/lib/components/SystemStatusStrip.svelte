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
  $: queueLabel = $queue.length > 0 ? `${$queue.length} waiting` : 'Queue clear';
  $: backlogLabel = $backlogStatus?.running
    ? `Catching up${$backlogStatus.current_album ? `: ${$backlogStatus.current_album}` : ''}`
    : 'Inbox resting';
  $: serviceLabel = $slskdRuntimeStatus?.ready
    ? 'Soulseek ready'
    : $slskdRuntimeStatus?.binary_found
      ? 'Soulseek waiting'
      : 'Soulseek missing';
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
    <span class="status-label">Inbox</span>
    <span class="status-value">{backlogLabel}</span>
  </div>

  <div class="status-pill" class:is-down={!$slskdRuntimeStatus?.ready}>
    <span class="status-label">Soulseek</span>
    <span class="status-value">{serviceLabel}</span>
  </div>

  <div class="status-pill" class:is-down={downProviders.length > 0}>
    <span class="status-label">Sources</span>
    <span class="status-value">
      {#if Object.keys($providerHealth).length > 0}
        {healthyProviders.length} steady / {downProviders.length} down
      {:else}
        {configuredProviders.length} configured
      {/if}
    </span>
  </div>
</div>

<style>
.status-strip {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 0 14px;
  height: var(--statusstrip-h);
  overflow-x: auto;
  scrollbar-width: none;
  background: rgba(6, 8, 16, 0.78);
  border-top: 1px solid rgba(var(--mood-accent-rgb), 0.1);
  transition: border-color var(--mood-shift-ms) ease;
}

.status-strip::-webkit-scrollbar {
  display: none;
}

.status-pill {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-radius: var(--radius-sm);
  border-left: 2px solid transparent;
  white-space: nowrap;
  transition: border-color var(--mood-shift-ms) ease, background var(--mood-shift-ms) ease;
}

.status-pill.is-busy {
  border-left-color: rgba(var(--mood-accent-rgb), 0.7);
  background: rgba(var(--mood-accent-rgb), 0.07);
}

.status-pill.is-down {
  border-left-color: rgba(239, 68, 68, 0.8);
  background: rgba(239, 68, 68, 0.06);
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
