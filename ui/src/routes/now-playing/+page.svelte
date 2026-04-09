<script lang="ts">
  import NowPlayingShrine from '$lib/components/NowPlayingShrine.svelte';
</script>

<svelte:head><title>Now Playing - Cassette</title></svelte:head>

<div class="now-playing-page">
  <NowPlayingShrine fullPage />
</div>

<style>
  .now-playing-page {
    padding: 18px;
    min-height: 100%;
  }
</style>
