<script lang="ts">
  import { loadQueue, queue } from '$lib/stores/queue';
  import {
    isPlaying,
    isSeeking,
    nowPlayingContext,
    playbackState,
    player,
    progressPct,
    seekPreview,
  } from '$lib/stores/player';
  import { clamp, coverSrc, formatDuration } from '$lib/utils';
  import { onMount } from 'svelte';

  export let open = false;
  export let onClose: () => void = () => {};

  let seekBarEl: HTMLDivElement;

  onMount(() => {
    void loadQueue();
  });

  function closeOverlay() {
    onClose();
  }

  function handleBackdropClick(event: MouseEvent) {
    if (event.target === event.currentTarget) {
      closeOverlay();
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (open && event.key === 'Escape') {
      event.preventDefault();
      closeOverlay();
    }
  }

  function getSeekPct(event: MouseEvent): number {
    const rect = seekBarEl.getBoundingClientRect();
    return clamp((event.clientX - rect.left) / rect.width, 0, 1);
  }

  function onSeekMouseDown(event: MouseEvent) {
    isSeeking.set(true);
    seekPreview.set(getSeekPct(event));

    const onMove = (moveEvent: MouseEvent) => seekPreview.set(getSeekPct(moveEvent));
    const onUp = async (upEvent: MouseEvent) => {
      const pct = getSeekPct(upEvent);
      isSeeking.set(false);
      await player.seek(pct);
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    };

    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  }

  async function handleNext() {
    await player.next();
    await loadQueue();
  }

  async function handlePrev() {
    await player.prev();
    await loadQueue();
  }

  $: track = $playbackState.current_track;
  $: positionSecs = $playbackState.position_secs;
  $: durationSecs = $playbackState.duration_secs;
  $: seekPct = $progressPct;
  $: context = $nowPlayingContext;
  $: queueItems = $queue;
  $: queuePosition = $playbackState.queue_position;
  $: nextItems = queueItems
    .filter((item) => item.position > queuePosition)
    .sort((a, b) => a.position - b.position)
    .slice(0, 5);
  $: lyricsText = context?.synced_lyrics || context?.lyrics || null;
</script>

<svelte:window on:keydown={handleKeydown} />

{#if open}
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="npx-backdrop" on:click={handleBackdropClick}>
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div
      class="npx-panel"
      role="dialog"
      aria-modal="true"
      aria-label="Expanded now playing"
      tabindex="-1"
      on:click|stopPropagation
    >
      <header class="npx-header">
        <div class="npx-heading">Now Playing</div>
        <button class="npx-close" type="button" on:click={closeOverlay} aria-label="Close expanded now playing">Close</button>
      </header>

      <div class="npx-grid">
        <div class="npx-main">
          <div class="npx-track">
            <div class="npx-art">
              {#if track?.cover_art_path}
                <img src={coverSrc(track.cover_art_path)} alt="cover" />
              {:else}
                <div class="npx-art-ph">No Art</div>
              {/if}
            </div>
            <div class="npx-meta">
              <h2>{track?.title ?? 'Nothing playing'}</h2>
              <p>{track?.artist ?? 'Select a track to start listening'}</p>
              {#if track?.album}
                <span>{track.album}</span>
              {/if}
            </div>
          </div>

          <div class="npx-controls">
            <button class="npx-ctrl" type="button" on:click={handlePrev}>Prev</button>
            <button class="npx-ctrl npx-ctrl-play" type="button" on:click={() => player.toggle()}>
              {$isPlaying ? 'Pause' : 'Play'}
            </button>
            <button class="npx-ctrl" type="button" on:click={handleNext}>Next</button>
          </div>

          <div class="npx-seek">
            <span>{formatDuration(positionSecs)}</span>
            <!-- svelte-ignore a11y-no-static-element-interactions -->
            <div class="npx-seek-bar" bind:this={seekBarEl} on:mousedown={onSeekMouseDown}>
              <div class="npx-seek-fill" style="width:{seekPct * 100}%"></div>
              <div class="npx-seek-thumb" style="left:{seekPct * 100}%"></div>
            </div>
            <span>{formatDuration(durationSecs)}</span>
          </div>
        </div>

        <aside class="npx-side">
          <section class="npx-block">
            <h3>Lyrics</h3>
            {#if lyricsText}
              <pre>{lyricsText}</pre>
              {#if context?.lyrics_source}
                <div class="npx-note">Source: {context.lyrics_source}</div>
              {/if}
            {:else}
              <div class="npx-empty">No lyrics available for this track yet.</div>
            {/if}
          </section>

          <section class="npx-block">
            <h3>Up Next</h3>
            {#if nextItems.length > 0}
              <ul>
                {#each nextItems as item}
                  <li>
                    <div class="npx-queue-title">{item.track?.title ?? 'Unknown track'}</div>
                    <div class="npx-queue-meta">{item.track?.artist ?? 'Unknown artist'}</div>
                  </li>
                {/each}
              </ul>
            {:else}
              <div class="npx-empty">Queue is empty.</div>
            {/if}
          </section>
        </aside>
      </div>
    </div>
  </div>
{/if}

<style>
  .npx-backdrop {
    position: fixed;
    inset: 0;
    z-index: 1100;
    background: rgba(6, 9, 15, 0.8);
    backdrop-filter: blur(6px);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
  }

  .npx-panel {
    width: min(1100px, 100%);
    max-height: min(88vh, 900px);
    border-radius: 16px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: linear-gradient(165deg, rgba(23, 31, 44, 0.97), rgba(14, 18, 28, 0.98));
    box-shadow: 0 20px 64px rgba(0, 0, 0, 0.45);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .npx-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 18px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  }

  .npx-heading {
    font-size: 0.78rem;
    letter-spacing: 0.1em;
    font-weight: 700;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .npx-close {
    border: 1px solid rgba(255, 255, 255, 0.18);
    border-radius: 999px;
    background: transparent;
    color: var(--text-secondary);
    padding: 6px 12px;
    font-size: 0.74rem;
    cursor: pointer;
  }

  .npx-close:hover {
    color: var(--text-primary);
    border-color: rgba(255, 255, 255, 0.28);
  }

  .npx-grid {
    display: grid;
    grid-template-columns: minmax(0, 1.6fr) minmax(280px, 1fr);
    gap: 16px;
    padding: 16px;
    min-height: 0;
    flex: 1;
  }

  .npx-main {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .npx-track {
    display: flex;
    gap: 14px;
    align-items: center;
    background: rgba(255, 255, 255, 0.04);
    border-radius: 12px;
    padding: 12px;
  }

  .npx-art {
    width: 140px;
    height: 140px;
    flex-shrink: 0;
    border-radius: 10px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.08);
  }

  .npx-art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .npx-art-ph {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    font-size: 0.8rem;
  }

  .npx-meta h2 {
    margin: 0;
    font-size: 1.15rem;
    color: var(--text-primary);
    line-height: 1.35;
  }

  .npx-meta p {
    margin: 4px 0;
    color: var(--text-secondary);
  }

  .npx-meta span {
    color: var(--text-muted);
    font-size: 0.82rem;
  }

  .npx-controls {
    display: flex;
    gap: 10px;
    align-items: center;
  }

  .npx-ctrl {
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 999px;
    background: transparent;
    color: var(--text-secondary);
    padding: 8px 14px;
    font-size: 0.82rem;
    cursor: pointer;
  }

  .npx-ctrl:hover {
    color: var(--text-primary);
    border-color: rgba(255, 255, 255, 0.28);
  }

  .npx-ctrl-play {
    background: var(--primary);
    border-color: var(--primary);
    color: var(--bg-deep);
    font-weight: 700;
  }

  .npx-ctrl-play:hover {
    color: var(--bg-deep);
    filter: brightness(1.05);
  }

  .npx-seek {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 10px;
  }

  .npx-seek span {
    color: var(--text-muted);
    font-size: 0.76rem;
    min-width: 36px;
  }

  .npx-seek-bar {
    position: relative;
    height: 6px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.12);
    cursor: pointer;
  }

  .npx-seek-fill {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--primary);
    border-radius: 999px;
    pointer-events: none;
  }

  .npx-seek-thumb {
    position: absolute;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--primary);
    pointer-events: none;
    opacity: 0;
    transition: opacity 0.15s;
  }

  .npx-seek:hover .npx-seek-thumb {
    opacity: 1;
  }

  .npx-side {
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .npx-block {
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    padding: 12px;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .npx-block h3 {
    margin: 0;
    color: var(--text-primary);
    font-size: 0.84rem;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .npx-block pre {
    margin: 0;
    white-space: pre-wrap;
    line-height: 1.7;
    color: var(--text-secondary);
    font-size: 0.78rem;
    overflow-y: auto;
    max-height: 240px;
  }

  .npx-note {
    color: var(--text-muted);
    font-size: 0.72rem;
  }

  .npx-empty {
    color: var(--text-muted);
    font-size: 0.78rem;
  }

  .npx-block ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    overflow-y: auto;
    max-height: 210px;
  }

  .npx-block li {
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 8px;
    background: rgba(255, 255, 255, 0.02);
  }

  .npx-queue-title {
    color: var(--text-primary);
    font-size: 0.8rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .npx-queue-meta {
    margin-top: 2px;
    color: var(--text-muted);
    font-size: 0.72rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  @media (max-width: 900px) {
    .npx-backdrop {
      padding: 12px;
      align-items: stretch;
    }

    .npx-panel {
      max-height: 100%;
    }

    .npx-grid {
      grid-template-columns: 1fr;
    }

    .npx-art {
      width: 96px;
      height: 96px;
    }
  }
</style>
