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
    <span class="queue-title">Queue</span>
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
          <span class="q-num">{isCurrent ? '▶' : i + 1}</span>
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
  padding: 14px 14px 10px; border-bottom: 1px solid var(--border); flex-shrink: 0;
}
.queue-title { font-weight: 600; font-size: 0.9rem; }
.clear-btn {
  font-size: 0.75rem; padding: 3px 8px; border-radius: var(--radius-sm);
  color: var(--text-secondary); border: 1px solid var(--border); cursor: pointer; background: none;
  transition: background 0.1s, color 0.1s;
}
.clear-btn:hover { background: var(--bg-hover); color: var(--text-primary); }

.queue-list { list-style: none; margin: 0; padding: 8px; overflow-y: auto; flex: 1; }
.queue-item {
  display: flex; align-items: center; gap: 10px;
  padding: 7px 8px; border-radius: var(--radius-sm);
  cursor: default; transition: background 0.1s;
}
.queue-item:hover { background: var(--bg-hover); }
.queue-item.current { background: var(--bg-active); color: var(--accent-bright); }
.q-num  { font-size: 0.75rem; color: var(--text-muted); min-width: 20px; text-align: center; }
.queue-item.current .q-num { color: var(--accent-bright); }
.q-info { flex: 1; overflow: hidden; }
.q-title  { font-size: 0.85rem; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.q-artist { font-size: 0.75rem; color: var(--text-secondary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.q-dur  { font-size: 0.75rem; color: var(--text-muted); white-space: nowrap; }
</style>
