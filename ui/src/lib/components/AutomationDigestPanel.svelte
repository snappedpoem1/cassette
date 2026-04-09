<script lang="ts">
  import { goto } from '$app/navigation';
  import {
    AUTOMATION_THRESHOLD_LABELS,
    type AutomationDigestSummary,
  } from '$lib/automation-digest';

  export let digest: AutomationDigestSummary;
  export let compact = false;
  export let showThresholdLegend = false;
  export let primaryHref = '/workstation';
  export let primaryLabel = 'Open workstation';
  export let secondaryHref: string | null = null;
  export let secondaryLabel: string | null = null;

  const legendOrder = ['silent', 'digest', 'soft_attention', 'explicit_intervention'] as const;
</script>

<section class="digest-panel" class:compact class:tone-watch={digest.tone === 'watch'} class:tone-action={digest.tone === 'action'}>
  <div class="digest-head">
    <div>
      <div class="digest-kicker">Calm automation</div>
      <div class="digest-threshold">{AUTOMATION_THRESHOLD_LABELS[digest.threshold]}</div>
      <h3>{digest.title}</h3>
      <p>{digest.detail}</p>
    </div>

    <div class="digest-actions">
      <button class="btn btn-secondary" on:click={() => goto(primaryHref)}>{primaryLabel}</button>
      {#if secondaryHref && secondaryLabel}
        <button class="btn btn-ghost" on:click={() => goto(secondaryHref)}>{secondaryLabel}</button>
      {/if}
    </div>
  </div>

  <div class="digest-metrics">
    <div class="digest-metric">
      <span>Inbox</span>
      <strong>{digest.counts.inbox}</strong>
    </div>
    <div class="digest-metric">
      <span>Moving</span>
      <strong>{digest.counts.active}</strong>
    </div>
    <div class="digest-metric">
      <span>Review</span>
      <strong>{digest.counts.review}</strong>
    </div>
    <div class="digest-metric">
      <span>Blocked</span>
      <strong>{digest.counts.blocked}</strong>
    </div>
  </div>

  <div class="digest-lines">
    {#each digest.lines as line}
      <div class="digest-line" class:tone-watch={line.tone === 'watch'} class:tone-action={line.tone === 'action'}>
        <span class="digest-line-label">{line.label}</span>
        <span class="digest-line-detail">{line.detail}</span>
      </div>
    {/each}
  </div>

  {#if showThresholdLegend}
    <div class="digest-legend">
      {#each legendOrder as threshold}
        <div class="legend-line" class:active={digest.threshold === threshold}>
          <span>{AUTOMATION_THRESHOLD_LABELS[threshold]}</span>
        </div>
      {/each}
    </div>
  {/if}
</section>

<style>
  .digest-panel {
    display: grid;
    gap: 14px;
    padding: 18px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--border);
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.015), transparent 46%),
      color-mix(in srgb, var(--bg-card) 90%, var(--bg-base));
  }

  .digest-panel.compact {
    padding: 14px;
    border-radius: var(--radius);
  }

  .digest-panel.tone-watch {
    border-color: color-mix(in srgb, var(--warning) 30%, var(--border));
  }

  .digest-panel.tone-action {
    border-color: color-mix(in srgb, var(--error) 38%, var(--border));
  }

  .digest-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 14px;
  }

  .digest-kicker {
    font-size: 0.66rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--accent-bright);
    font-weight: 700;
  }

  .digest-threshold {
    margin-top: 4px;
    font-size: 0.74rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }

  .digest-head h3 {
    margin-top: 6px;
    font-size: 1.08rem;
  }

  .digest-head p {
    margin-top: 6px;
    max-width: 58ch;
    color: var(--text-secondary);
    font-size: 0.84rem;
    line-height: 1.7;
  }

  .digest-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  .digest-metrics {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
  }

  .digest-metric {
    padding: 10px 12px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: color-mix(in srgb, var(--bg-base) 86%, transparent);
  }

  .digest-metric span {
    display: block;
    font-size: 0.66rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
    margin-bottom: 5px;
  }

  .digest-metric strong {
    font-size: 1.08rem;
    color: var(--text-primary);
  }

  .digest-lines {
    display: grid;
    gap: 8px;
  }

  .digest-line {
    display: grid;
    gap: 3px;
    padding: 10px 12px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: color-mix(in srgb, var(--bg-base) 88%, transparent);
  }

  .digest-line.tone-watch {
    border-color: color-mix(in srgb, var(--warning) 22%, var(--border));
  }

  .digest-line.tone-action {
    border-color: color-mix(in srgb, var(--error) 28%, var(--border));
  }

  .digest-line-label {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }

  .digest-line-detail {
    font-size: 0.82rem;
    color: var(--text-secondary);
    line-height: 1.65;
  }

  .digest-legend {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
  }

  .legend-line {
    padding: 9px 10px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    color: var(--text-muted);
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    text-align: center;
    background: color-mix(in srgb, var(--bg-base) 82%, transparent);
  }

  .legend-line.active {
    color: var(--text-primary);
    border-color: var(--border-active);
    background: color-mix(in srgb, var(--primary) 12%, var(--bg-base));
  }

  @media (max-width: 920px) {
    .digest-head,
    .digest-metrics,
    .digest-legend {
      grid-template-columns: 1fr;
      display: grid;
    }
  }
</style>
