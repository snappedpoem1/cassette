<script lang="ts">
  import '../app.css';
  import { onMount, onDestroy } from 'svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import NowPlaying from '$lib/components/NowPlaying.svelte';
  import RightSidebar from '$lib/components/RightSidebar.svelte';
  import { startPlayerPoll, stopPlayerPoll } from '$lib/stores/player';
  import { loadLibrary } from '$lib/stores/library';

  onMount(() => {
    startPlayerPoll();
    loadLibrary();
  });

  onDestroy(() => {
    stopPlayerPoll();
  });
</script>

<div class="app-shell">
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
</div>
