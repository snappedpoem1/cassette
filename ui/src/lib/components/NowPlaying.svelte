<script lang="ts">
  import { playbackState, isPlaying, progressPct, isSeeking, seekPreview, player } from '$lib/stores/player';
  import { loadQueue } from '$lib/stores/queue';
  import { formatDuration, coverSrc, clamp } from '$lib/utils';

  let seekBarEl: HTMLDivElement;
  let volBarEl: HTMLDivElement;

  function getSeekPct(e: MouseEvent): number {
    const rect = seekBarEl.getBoundingClientRect();
    return clamp((e.clientX - rect.left) / rect.width, 0, 1);
  }

  function onSeekMouseDown(e: MouseEvent) {
    isSeeking.set(true);
    seekPreview.set(getSeekPct(e));
    const onMove = (ev: MouseEvent) => seekPreview.set(getSeekPct(ev));
    const onUp = async (ev: MouseEvent) => {
      const pct = getSeekPct(ev);
      isSeeking.set(false);
      await player.seek(pct);
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    };
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  }

  function onVolMouseDown(e: MouseEvent) {
    const update = (ev: MouseEvent) => {
      const rect = volBarEl.getBoundingClientRect();
      player.setVolume(clamp((ev.clientX - rect.left) / rect.width, 0, 1));
    };
    update(e);
    const onUp = () => {
      window.removeEventListener('mousemove', update);
      window.removeEventListener('mouseup', onUp);
    };
    window.addEventListener('mousemove', update);
    window.addEventListener('mouseup', onUp);
  }

  $: track = $playbackState.current_track;
  $: pos   = $playbackState.position_secs;
  $: dur   = $playbackState.duration_secs;
  $: vol   = $playbackState.volume;
  $: pct   = $progressPct;

  async function handleNext() { await player.next(); await loadQueue(); }
  async function handlePrev() { await player.prev(); await loadQueue(); }
</script>

<div class="nowplaying">
  <!-- Left: art + info -->
  <div class="np-left">
    <div class="np-art">
      {#if track?.cover_art_path}
        <img src={coverSrc(track.cover_art_path)} alt="cover" />
      {:else}
        <div class="np-art-ph">🎵</div>
      {/if}
    </div>
    <div class="np-info">
      <div class="np-title">{track?.title ?? '—'}</div>
      <div class="np-artist">{track?.artist ?? 'No track playing'}</div>
    </div>
  </div>

  <!-- Center: controls + seek -->
  <div class="np-center">
    <div class="np-controls">
      <button class="ctrl-btn" on:click={handlePrev} title="Previous">⏮</button>
      <button class="ctrl-btn play-btn" on:click={() => player.toggle()} title="Play/Pause">
        {#if $isPlaying}⏸{:else}▶{/if}
      </button>
      <button class="ctrl-btn" on:click={handleNext} title="Next">⏭</button>
    </div>
    <div class="np-seek">
      <span class="np-time">{formatDuration(pos)}</span>
      <!-- svelte-ignore a11y-no-static-element-interactions -->
      <div class="seek-bar" bind:this={seekBarEl} on:mousedown={onSeekMouseDown}>
        <div class="seek-fill" style="width:{pct*100}%"></div>
        <div class="seek-thumb" style="left:{pct*100}%"></div>
      </div>
      <span class="np-time right">{formatDuration(dur)}</span>
    </div>
  </div>

  <!-- Right: volume -->
  <div class="np-right">
    <span class="vol-icon">{vol === 0 ? '🔇' : vol < 0.5 ? '🔉' : '🔊'}</span>
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div class="volume-bar" bind:this={volBarEl} on:mousedown={onVolMouseDown}>
      <div class="volume-fill" style="width:{vol*100}%"></div>
    </div>
  </div>
</div>

<style>
.nowplaying {
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  align-items: center;
  height: var(--playerbar-h);
  padding: 0 14px;
  gap: 12px;
}

.np-left { display: flex; align-items: center; gap: 10px; overflow: hidden; }
.np-art  { width: 44px; height: 44px; flex-shrink: 0; border-radius: 5px; overflow: hidden; background: var(--bg-card); box-shadow: 0 2px 8px rgba(0,0,0,0.5); }
.np-art img { width: 100%; height: 100%; object-fit: cover; }
.np-art-ph  { width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; font-size: 1.2rem; color: var(--text-muted); }
.np-info    { overflow: hidden; }
.np-title   { font-weight: 600; font-size: 0.82rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: var(--text-primary); }
.np-artist  { font-size: 0.72rem; color: var(--text-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; margin-top: 1px; }

.np-center { display: flex; flex-direction: column; align-items: center; gap: 5px; min-width: 300px; max-width: 460px; width: 100%; }
.np-controls { display: flex; align-items: center; gap: 8px; }
.ctrl-btn {
  display: inline-flex; align-items: center; justify-content: center;
  width: 28px; height: 28px; border-radius: 50%; font-size: 0.9rem;
  color: var(--text-muted); background: none; border: none; cursor: pointer;
  transition: background 0.15s, color 0.15s;
}
.ctrl-btn:hover { background: var(--bg-hover); color: var(--text-primary); }
.play-btn {
  width: 32px; height: 32px;
  background: var(--primary) !important;
  color: var(--bg-deep) !important;
  box-shadow: 0 2px 10px rgba(139,180,212,0.25);
}
.play-btn:hover { background: #a0c8e8 !important; }

.np-seek  { display: flex; align-items: center; gap: 7px; width: 100%; }
.np-time  { font-size: 0.68rem; color: var(--text-muted); white-space: nowrap; min-width: 32px; }
.np-time.right { text-align: right; }

.np-right   { display: flex; align-items: center; gap: 7px; justify-content: flex-end; }
.vol-icon   { font-size: 0.82rem; color: var(--text-muted); }
</style>
