<script lang="ts">
  import SessionComposer from '$lib/components/SessionComposer.svelte';
</script>

<svelte:head><title>Session Composer · Cassette</title></svelte:head>

<div class="session-page">
  <section class="session-hero card">
    <div class="section-kicker">Session Workspace</div>
    <h1>Session Composer</h1>
    <p>
      Build explainable listening arcs with duration, era, and transition controls in one dedicated surface.
      Generate a route, inspect why each handoff was chosen, and queue the full run.
    </p>
  </section>

  <SessionComposer />
</div>

<style>
  .session-page {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 18px;
  }

  .session-hero {
    padding: 18px;
    display: grid;
    gap: 8px;
    border-radius: var(--radius-lg);
  }

  .section-kicker {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--accent);
    font-weight: 700;
  }

  .session-hero h1 {
    font-size: clamp(1.55rem, 3vw, 2.15rem);
  }

  .session-hero p {
    max-width: 70ch;
    color: var(--text-secondary);
    font-size: 0.9rem;
    line-height: 1.65;
  }
</style>
