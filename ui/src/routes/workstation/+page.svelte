<script lang="ts">
  import WorkstationSurface from '$lib/components/WorkstationSurface.svelte';
</script>

<svelte:head><title>Workstation - Cassette</title></svelte:head>

<div class="workstation-route">
  <div class="workstation-note">
    <span class="workstation-note-kicker">Compatibility surface</span>
    <p>
      Workstation now lives in the shell as a sliding deck. This route remains as a focused fallback
      while the workspace conversion settles.
    </p>
  </div>

  <WorkstationSurface />
</div>

<style>
  .workstation-route {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 18px;
  }

  .workstation-note {
    display: grid;
    gap: 6px;
    padding: 14px 16px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: rgba(7, 10, 16, 0.72);
  }

  .workstation-note-kicker {
    font-size: 0.66rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--accent-bright);
    font-weight: 700;
  }

  .workstation-note p {
    color: var(--text-secondary);
    font-size: 0.82rem;
    line-height: 1.7;
  }
</style>
