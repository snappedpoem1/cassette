<script lang="ts">
  import { onMount } from 'svelte';
  import { queue, loadQueue, clearQueue } from '$lib/stores/queue';
  import { playbackState } from '$lib/stores/player';
  import { api } from '$lib/api/tauri';
  import { formatDuration } from '$lib/utils';

  onMount(() => loadQueue());

  async function jumpTo(index: number) {
    const items = $queue;
    if (!items[index]) return;
    const trackIds = items.map((i) => i.track_id);
    await api.queueTracks(trackIds, index);
    await loadQueue();
  }
</script>

<div class="queue-panel">
  <div class="queue-header">
    <span class="queue-title">Up Next</span>
    {#if $queue.length > 0}
      <button class="clear-btn" on:click={clearQueue}>Clear</button>
    {/if}
  </div>

  {#if $queue.length === 0}
    <div class="empty-state" style="padding:2rem 1rem;">
      <div class="empty-icon">🎵</div>
      <div class="empty-title">Queue is empty</div>
      <div class="empty-body">Play a track or album to fill the queue.</div>
    </div>
  {:else}
    <ul class="queue-list">
      {#each $queue as item, i}
        {@const track = item.track}
        {@const isCurrent = i === $playbackState.queue_position}
        <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
        <li class="queue-item" class:current={isCurrent} on:dblclick={() => jumpTo(i)}>
          <span class="q-num" class:active={isCurrent}>{isCurrent ? '▶' : i + 1}</span>
          <div class="q-info">
            <div class="q-title">{track?.title ?? 'Unknown'}</div>
            <div class="q-artist">{track?.artist ?? ''}</div>
          </div>
          <span class="q-dur">{formatDuration(track?.duration_secs ?? 0)}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
.queue-panel { display: flex; flex-direction: column; height: 100%; }
.queue-header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 10px 12px 8px; flex-shrink: 0;
}
.queue-title { font-size: 0.62rem; font-weight: 700; letter-spacing: 0.1em; text-transform: uppercase; color: var(--text-muted); }
.clear-btn {
  font-size: 0.68rem; padding: 2px 7px; border-radius: var(--radius-sm);
  color: var(--text-muted); border: 1px solid var(--border-dim); cursor: pointer; background: none;
  transition: background 0.1s, color 0.1s;
}
.clear-btn:hover { background: var(--bg-hover); color: var(--text-secondary); }

.queue-list { list-style: none; margin: 0; padding: 4px 6px; overflow-y: auto; flex: 1; }
.queue-item {
  display: flex; align-items: center; gap: 8px;
  padding: 5px 6px; border-radius: var(--radius-sm);
  cursor: default; transition: background 0.1s;
}
.queue-item:hover { background: var(--bg-hover); }
.queue-item.current { background: var(--bg-active); }
.q-num  { font-size: 0.68rem; color: var(--text-muted); min-width: 18px; text-align: center; }
.q-num.active { color: var(--primary); }
.q-info { flex: 1; overflow: hidden; }
.q-title  { font-size: 0.78rem; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: var(--text-primary); }
.q-artist { font-size: 0.68rem; color: var(--text-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.queue-item.current .q-title { color: var(--primary); }
.q-dur  { font-size: 0.68rem; color: var(--text-muted); white-space: nowrap; }
</style>
