<script lang="ts">
  import { onMount } from 'svelte';
  import { tick } from 'svelte';
  import {
    isPaletteOpen,
    filteredCommands,
    paletteSearchQuery,
    openPalette,
    closePalette,
    setPaletteQuery,
    executeCommand,
    handleGlobalShortcut,
  } from '$lib/stores/commands';

  let selectedIndex = 0;
  let inputEl: HTMLInputElement;

  $: visibleCommands = $filteredCommands;
  $: if ($isPaletteOpen) {
    selectedIndex = 0;
    tick().then(() => inputEl?.focus());
  }

  onMount(() => {
    const onKeyDown = async (event: KeyboardEvent) => {
      const usesCommandKey = event.ctrlKey || event.metaKey;
      const isPaletteShortcut = usesCommandKey && event.key.toLowerCase() === 'k';

      if (isPaletteShortcut) {
        event.preventDefault();
        if ($isPaletteOpen) {
          closePalette();
        } else {
          openPalette();
        }
        return;
      }

      if (!$isPaletteOpen) {
        const handled = await handleGlobalShortcut(event);
        if (handled) {
          return;
        }
      }

      if (!$isPaletteOpen) {
        return;
      }

      if (event.key === 'Escape') {
        event.preventDefault();
        closePalette();
        return;
      }

      if (event.key === 'ArrowDown') {
        event.preventDefault();
        if (visibleCommands.length > 0) {
          selectedIndex = Math.min(selectedIndex + 1, visibleCommands.length - 1);
        }
        return;
      }

      if (event.key === 'ArrowUp') {
        event.preventDefault();
        if (visibleCommands.length > 0) {
          selectedIndex = Math.max(selectedIndex - 1, 0);
        }
        return;
      }

      if (event.key === 'Enter') {
        event.preventDefault();
        const selected = visibleCommands[selectedIndex];
        if (selected) {
          await executeCommand(selected);
        }
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  });
</script>

{#if $isPaletteOpen}
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="command-overlay" on:click={closePalette}>
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div
      class="command-panel"
      role="dialog"
      aria-modal="true"
      aria-label="Command Palette"
      tabindex="-1"
      on:click|stopPropagation
    >
      <input
        class="command-input"
        type="text"
        placeholder="Type a command or route..."
        bind:this={inputEl}
        value={$paletteSearchQuery}
        on:input={(event) => setPaletteQuery((event.currentTarget as HTMLInputElement).value)}
      />

      {#if visibleCommands.length === 0}
        <div class="command-empty">No commands found.</div>
      {:else}
        <div class="command-list" role="listbox" aria-label="Commands">
          {#each visibleCommands as command, index}
            <button
              class="command-item"
              class:active={index === selectedIndex}
              on:click={() => executeCommand(command)}
              role="option"
              aria-selected={index === selectedIndex}
            >
              <span class="command-label">{command.label}</span>
              <span class="command-meta">
                <span class="command-category">{command.category}</span>
                {#if command.shortcut}
                  <span class="command-shortcut">{command.shortcut}</span>
                {/if}
              </span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .command-overlay {
    position: fixed;
    inset: 0;
    z-index: 1000;
    background: rgba(4, 6, 10, 0.72);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 92px;
  }

  .command-panel {
    width: min(760px, calc(100vw - 40px));
    max-height: min(70vh, 620px);
    border-radius: 12px;
    border: 1px solid rgba(247, 180, 92, 0.25);
    background: linear-gradient(180deg, rgba(27, 34, 46, 0.98), rgba(17, 22, 31, 0.98));
    box-shadow: 0 18px 44px rgba(0, 0, 0, 0.46);
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .command-input {
    width: 100%;
    padding: 14px 16px;
    border: none;
    border-bottom: 1px solid var(--border);
    color: var(--text-primary);
    background: transparent;
    font-size: 0.95rem;
  }

  .command-input::placeholder {
    color: var(--text-muted);
  }

  .command-list {
    overflow-y: auto;
    padding: 8px;
  }

  .command-item {
    width: 100%;
    padding: 10px 12px;
    border-radius: 8px;
    border: 1px solid transparent;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    color: var(--text-primary);
    background: transparent;
    text-align: left;
  }

  .command-item:hover,
  .command-item.active {
    background: rgba(247, 180, 92, 0.12);
    border-color: rgba(247, 180, 92, 0.24);
  }

  .command-label {
    font-size: 0.9rem;
    font-weight: 600;
  }

  .command-meta {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    color: var(--text-secondary);
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .command-shortcut {
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 2px 6px;
    color: var(--text-primary);
    letter-spacing: 0.06em;
  }

  .command-empty {
    padding: 22px 16px;
    color: var(--text-secondary);
  }
</style>
