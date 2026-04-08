<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type Track } from '$lib/api/tauri';
  import { queueTracks } from '$lib/stores/queue';
  import { formatDuration, coverSrc } from '$lib/utils';

  type HistoryTab = 'recent' | 'most-played';

  let activeTab: HistoryTab = 'recent';
  let recentTracks: Track[] = [];
  let mostPlayedTracks: Track[] = [];
  let loading = true;
  let errorMessage = '';

  $: activeList = activeTab === 'recent' ? recentTracks : mostPlayedTracks;

  onMount(async () => {
    await loadHistory();
  });

  async function loadHistory() {
    loading = true;
    errorMessage = '';
    try {
      const [recent, mostPlayed] = await Promise.all([
        api.getRecentlyFinalizedTracks(30),
        api.getMostPlayedTracks(50),
      ]);
      recentTracks = recent;
      mostPlayedTracks = mostPlayed;
    } catch {
      errorMessage = 'Unable to load history right now.';
      recentTracks = [];
      mostPlayedTracks = [];
    } finally {
      loading = false;
    }
  }

  async function playFromActive(index: number) {
    if (!activeList.length) {
      return;
    }
    await queueTracks(activeList, index);
  }

</script>

<svelte:head><title>History · Cassette</title></svelte:head>

<div class="history-page">
  <div class="page-header">
    <h2>History</h2>
    <button class="btn btn-ghost" on:click={loadHistory} disabled={loading}>Refresh</button>
  </div>

  <div class="tabs">
    <button class="tab" class:active={activeTab === 'recent'} on:click={() => (activeTab = 'recent')}>
      Recent
      {#if recentTracks.length}
        <span class="tab-count">{recentTracks.length}</span>
      {/if}
    </button>
    <button
      class="tab"
      class:active={activeTab === 'most-played'}
      on:click={() => (activeTab = 'most-played')}
    >
      Most Played
      {#if mostPlayedTracks.length}
        <span class="tab-count">{mostPlayedTracks.length}</span>
      {/if}
    </button>
  </div>

  {#if loading}
    <div class="empty-state">
      <div class="spinner"></div>
      <div class="empty-body">Loading playback history...</div>
    </div>
  {:else if errorMessage}
    <div class="empty-state">
      <div class="empty-title">History unavailable</div>
      <div class="empty-body">{errorMessage}</div>
    </div>
  {:else if activeList.length === 0}
    <div class="empty-state">
      <div class="empty-title">No entries yet</div>
      <div class="empty-body">
        {activeTab === 'recent'
          ? 'Finalized arrivals will appear here as they land.'
          : 'Play tracks from Library or Artists to build your top list.'}
      </div>
    </div>
  {:else}
    <div class="history-list">
      {#each activeList as track, i}
        <div
          class="track-row"
          role="button"
          tabindex="0"
          on:dblclick={() => playFromActive(i)}
          on:keydown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              void playFromActive(i);
            }
          }}
        >
          <span class="track-num">{i + 1}</span>
          {#if track.cover_art_path}
            <img class="track-art" src={coverSrc(track.cover_art_path)} alt="" loading="lazy" />
          {:else}
            <span class="track-art-ph"></span>
          {/if}
          <div class="track-title">{track.title}</div>
          <div class="track-artist">{track.artist}{track.album ? ` · ${track.album}` : ''}</div>
          <span class="track-duration">{formatDuration(track.duration_secs)}</span>
          <span class="track-meta">{activeTab === 'recent' ? track.added_at.slice(0, 10) : `Top ${i + 1}`}</span>
          <span class="track-format">{track.format.toUpperCase()}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
.history-page {
  display: flex;
  flex-direction: column;
  min-height: 100%;
}

.tab-count {
  margin-left: 6px;
  font-size: 0.68rem;
  color: var(--text-muted);
}

.history-list {
  padding: 8px;
}

.track-art,
.track-art-ph {
  width: 26px;
  height: 26px;
  border-radius: 6px;
  flex-shrink: 0;
}

.track-art {
  object-fit: cover;
}

.track-art-ph {
  background: var(--bg-active);
  border: 1px solid var(--border);
}

.track-meta {
  width: 120px;
  text-align: right;
  font-size: 0.7rem;
  color: var(--text-muted);
  flex-shrink: 0;
}

@media (max-width: 920px) {
  .track-meta {
    display: none;
  }
}
</style>