<script lang="ts">
  import { closeWorkstationDeck, workstationDeckOpen } from '$lib/stores/shell';
  import WorkstationSurface from '$lib/components/WorkstationSurface.svelte';

  function handleBackdropClick(event: MouseEvent) {
    if (event.target === event.currentTarget) {
      closeWorkstationDeck();
    }
  }
</script>

<svelte:window
  on:keydown={(event) => {
    if ($workstationDeckOpen && event.key === 'Escape') {
      event.preventDefault();
      closeWorkstationDeck();
    }
  }}
/>

{#if $workstationDeckOpen}
  <div
    class="workstation-deck-backdrop"
    role="button"
    tabindex="0"
    aria-label="Close workstation deck backdrop"
    on:click={handleBackdropClick}
    on:keydown={(event) => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        closeWorkstationDeck();
      }
    }}
  >
    <div
      class="workstation-deck"
      role="dialog"
      aria-modal="true"
      aria-label="Workstation deck"
      tabindex="-1"
    >
      <header class="deck-header">
        <div>
          <div class="deck-kicker">Workstation</div>
          <h2>Acquisition and repair deck</h2>
        </div>
        <button class="deck-close" type="button" on:click={closeWorkstationDeck}>Close</button>
      </header>

      <div class="deck-body">
        <WorkstationSurface compact />
      </div>
    </div>
  </div>
{/if}

<style>
  .workstation-deck-backdrop {
    position: fixed;
    inset: 0;
    z-index: 950;
    background: rgba(3, 5, 9, 0.54);
    backdrop-filter: blur(4px);
  }

  .workstation-deck {
    position: absolute;
    inset: 0 auto 0 0;
    width: min(540px, calc(100vw - 120px));
    display: grid;
    grid-template-rows: auto 1fr;
    border-right: 1px solid rgba(var(--mood-accent-rgb), 0.18);
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.012), transparent 42%),
      rgba(7, 10, 16, 0.98);
    box-shadow: 24px 0 60px rgba(0, 0, 0, 0.38);
    animation: deck-slide-in 180ms ease-out;
  }

  .deck-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding: 16px 16px 14px;
    border-bottom: 1px solid var(--border-dim);
    background:
      linear-gradient(90deg, rgba(var(--mood-accent-rgb), 0.08), transparent 64%),
      rgba(8, 11, 18, 0.96);
  }

  .deck-kicker {
    font-size: 0.66rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--accent-bright);
    font-weight: 700;
  }

  .deck-header h2 {
    margin-top: 4px;
    font-size: 1rem;
  }

  .deck-close {
    padding: 7px 10px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    color: var(--text-secondary);
    font-size: 0.74rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .deck-close:hover {
    border-color: var(--border-active);
    color: var(--text-primary);
    background: rgba(255, 255, 255, 0.04);
  }

  .deck-body {
    overflow: auto;
    padding: 14px;
  }

  @keyframes deck-slide-in {
    from {
      opacity: 0;
      transform: translateX(-18px);
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
  }

  @media (max-width: 840px) {
    .workstation-deck {
      width: calc(100vw - 32px);
    }
  }
</style>
