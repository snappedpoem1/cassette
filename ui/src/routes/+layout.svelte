<script lang="ts">
  import '../app.css';
  import { onMount, onDestroy } from 'svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import NowPlaying from '$lib/components/NowPlaying.svelte';
  import RightSidebar from '$lib/components/RightSidebar.svelte';
  import CommandPalette from '$lib/components/CommandPalette.svelte';
  import { startPlayerPoll, stopPlayerPoll } from '$lib/stores/player';
  import { loadLibrary } from '$lib/stores/library';
  import { openPalette } from '$lib/stores/commands';
  import { compactPlayerMode, toggleCompactPlayerMode, minimizeAppWindow } from '$lib/stores/shell';

  onMount(() => {
    startPlayerPoll();
    loadLibrary();
  });

  onDestroy(() => {
    stopPlayerPoll();
  });
</script>

<div class="app-shell" class:compact-player={$compactPlayerMode}>
  <header class="app-topbar">
    <div class="topbar-brand">
      <span class="brand-wordmark">Cassette</span>
      <span class="brand-divider">//</span>
      <span class="brand-mode">Desktop</span>
    </div>
    <nav class="topbar-nav" aria-label="Quick actions">
      <a class="topbar-link" href="/">Library</a>
      <a class="topbar-link" href="/downloads">Downloads</a>
      <a class="topbar-link" href="/settings">Settings</a>
    </nav>
    <button class="topbar-link topbar-toggle" type="button" aria-label="Toggle compact player" on:click={toggleCompactPlayerMode}>
      {$compactPlayerMode ? 'Full Player' : 'Mini Player'}
    </button>
    <button class="topbar-link topbar-toggle" type="button" aria-label="Minimize app" on:click={minimizeAppWindow}>
      Minimize
    </button>
    <button class="topbar-command" type="button" aria-label="Open command palette" on:click={openPalette}>
      Commands
    </button>
  </header>

  <aside class="app-sidebar">
    <Sidebar />
  </aside>

  <main class="app-main">
    <slot />
  </main>

  <aside class="app-right">
    <RightSidebar />
  </aside>

  <footer class="app-nowplaying">
    <NowPlaying />
  </footer>

  <CommandPalette />
</div>
