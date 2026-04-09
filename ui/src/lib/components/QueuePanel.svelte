<script lang="ts">
  import { onMount } from 'svelte';
  import { queue, loadQueue, clearQueue, removeQueueItem } from '$lib/stores/queue';
  import { playbackState } from '$lib/stores/player';
  import { api } from '$lib/api/tauri';
  import { formatDuration } from '$lib/utils';

  onMount(() => loadQueue());

  let dragIndex: number | null = null;
  let dragOverIndex: number | null = null;

  async function jumpTo(index: number) {
    const items = $queue;
    if (!items[index]) return;
    const trackIds = items.map((item) => item.track_id);
    await api.queueTracks(trackIds, index);
    await loadQueue();
  }

  async function handleRemove(position: number) {
    const items = $queue;
    const currentPos = $playbackState.queue_position;
    const remainingLength = Math.max(0, items.length - 1);
    if (remainingLength === 0) {
      await clearQueue();
      return;
    }

    let nextStartIndex = currentPos;
    if (position < currentPos) {
      nextStartIndex = currentPos - 1;
    } else if (position === currentPos) {
      nextStartIndex = Math.min(position, remainingLength - 1);
    }

    await removeQueueItem(position, Math.max(0, nextStartIndex));
  }

  function onDragStart(index: number) {
    dragIndex = index;
  }

  function onDragOver(event: DragEvent, index: number) {
    event.preventDefault();
    dragOverIndex = index;
  }

  async function onDrop(event: DragEvent, dropIndex: number) {
    event.preventDefault();
    if (dragIndex === null || dragIndex === dropIndex) {
      dragIndex = null;
      dragOverIndex = null;
      return;
    }

    const items = [...$queue];
    const [moved] = items.splice(dragIndex, 1);
    items.splice(dropIndex, 0, moved);

    const trackIds = items.map((item) => item.track_id);
    const currentPos = $playbackState.queue_position;
    let nextStartIndex = currentPos;

    if (currentPos === dragIndex) {
      nextStartIndex = dropIndex;
    } else if (dragIndex < currentPos && dropIndex >= currentPos) {
      nextStartIndex = currentPos - 1;
    } else if (dragIndex > currentPos && dropIndex <= currentPos) {
      nextStartIndex = currentPos + 1;
    }

    await api.reorderQueue(trackIds, Math.max(0, Math.min(nextStartIndex, trackIds.length - 1)));
    await loadQueue();
    dragIndex = null;
    dragOverIndex = null;
  }

  function onDragEnd() {
    dragIndex = null;
    dragOverIndex = null;
  }
</script>

<div class="queue-panel">
  <div class="queue-header">
    <div>
      <span class="queue-title">Up Next</span>
      <div class="queue-subtitle">Drag to reshape the run.</div>
    </div>
    {#if $queue.length > 0}
      <button class="clear-btn" on:click={clearQueue}>Clear</button>
    {/if}
  </div>

  {#if $queue.length === 0}
    <div class="empty-state" style="padding:2rem 1rem;">
      <div class="empty-title">Queue is empty</div>
      <div class="empty-body">Start a track, album, playlist, or session to build your next stretch.</div>
    </div>
  {:else}
    <ul class="queue-list">
      {#each $queue as item, i}
        {@const track = item.track}
        {@const isCurrent = i === $playbackState.queue_position}
        <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
        <li
          class="queue-item"
          class:current={isCurrent}
          class:drag-over={dragOverIndex === i}
          draggable="true"
          on:dragstart={() => onDragStart(i)}
          on:dragover={(event) => onDragOver(event, i)}
          on:drop={(event) => onDrop(event, i)}
          on:dragend={onDragEnd}
          on:dblclick={() => jumpTo(i)}
        >
          <span class="q-drag" aria-hidden="true">
            <svg width="10" height="10" viewBox="0 0 24 24" fill="currentColor" opacity="0.45">
              <circle cx="9" cy="5" r="1.5" /><circle cx="15" cy="5" r="1.5" />
              <circle cx="9" cy="12" r="1.5" /><circle cx="15" cy="12" r="1.5" />
              <circle cx="9" cy="19" r="1.5" /><circle cx="15" cy="19" r="1.5" />
            </svg>
          </span>
          <span class="q-num" class:active={isCurrent}>{isCurrent ? 'Now' : i + 1}</span>
          <div class="q-info">
            <div class="q-title">{track?.title ?? 'Unknown track'}</div>
            <div class="q-artist">{track?.artist ?? 'Unknown artist'}</div>
          </div>
          <span class="q-dur">{formatDuration(track?.duration_secs ?? 0)}</span>
          <button
            class="q-remove"
            on:click|stopPropagation={() => handleRemove(i)}
            title="Remove"
            aria-label="Remove from queue"
          >
            <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
              <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .queue-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .queue-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    padding: 12px 12px 8px;
    flex-shrink: 0;
    gap: 8px;
  }

  .queue-title {
    font-size: 0.64rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .queue-subtitle {
    margin-top: 3px;
    font-size: 0.75rem;
    color: var(--text-secondary);
  }

  .clear-btn {
    font-size: 0.72rem;
    padding: 4px 9px;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    border: 1px solid var(--border-dim);
    cursor: pointer;
    background: none;
    transition: background 0.1s, color 0.1s;
  }

  .clear-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .queue-list {
    list-style: none;
    margin: 0;
    padding: 4px 8px 10px;
    overflow-y: auto;
    flex: 1;
  }

  .queue-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 8px;
    border-radius: var(--radius-sm);
    cursor: default;
    transition: background 0.1s, border-color 0.1s;
    border: 1px solid transparent;
  }

  .queue-item:hover {
    background: var(--bg-hover);
    border-color: var(--border);
  }

  .queue-item.current {
    background: rgba(139, 180, 212, 0.1);
    border-color: rgba(139, 180, 212, 0.18);
  }

  .q-num {
    font-size: 0.68rem;
    color: var(--text-muted);
    min-width: 24px;
    text-align: center;
    font-weight: 700;
  }

  .q-num.active {
    color: var(--text-accent);
  }

  .q-drag {
    cursor: grab;
    display: flex;
    align-items: center;
    padding: 0 3px;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .q-drag:active {
    cursor: grabbing;
  }

  .q-info {
    flex: 1;
    overflow: hidden;
  }

  .q-title {
    font-size: 0.8rem;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-primary);
  }

  .q-artist {
    font-size: 0.72rem;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .queue-item.current .q-title {
    color: var(--text-accent);
  }

  .q-dur {
    font-size: 0.7rem;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .queue-item.drag-over {
    border-top: 2px solid var(--primary);
  }

  .q-remove {
    display: none;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    color: var(--text-muted);
    flex-shrink: 0;
    background: none;
    border: none;
    cursor: pointer;
    padding: 0;
  }

  .queue-item:hover .q-remove {
    display: flex;
  }

  .q-remove:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }
</style>
