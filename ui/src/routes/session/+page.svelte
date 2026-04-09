<script lang="ts">
  import SessionComposer from '$lib/components/SessionComposer.svelte';
</script>

<svelte:head><title>Session - Cassette</title></svelte:head>

<div class="session-page">
  <section class="session-hero card mood-card">
    <div class="section-kicker">Arc ritual</div>
    <h1>Build a listening arc</h1>
    <p>
      Shape a replayable run with memory, branching, and clean handoff in one place. Generate or
      import an arc, inspect why each move belongs, then save the line when it earns a return.
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
    background: rgba(6, 8, 16, 0.72);
    border: 1px solid rgba(var(--mood-accent-rgb), 0.12);
    transition: border-color var(--mood-shift-ms) ease;
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
