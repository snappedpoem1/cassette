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
  height: var(--nowplaying-h);
  padding: 0 16px;
  gap: 12px;
}

/* Left */
.np-left { display: flex; align-items: center; gap: 12px; overflow: hidden; }
.np-art  { width: 52px; height: 52px; flex-shrink: 0; border-radius: 6px; overflow: hidden; background: var(--bg-active); }
.np-art img { width: 100%; height: 100%; object-fit: cover; }
.np-art-ph  { width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; font-size: 1.4rem; color: var(--text-muted); }
.np-info    { overflow: hidden; }
.np-title   { font-weight: 600; font-size: 0.9rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.np-artist  { font-size: 0.8rem; color: var(--text-secondary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

/* Center */
.np-center { display: flex; flex-direction: column; align-items: center; gap: 6px; min-width: 320px; max-width: 480px; width: 100%; }
.np-controls { display: flex; align-items: center; gap: 8px; }
.ctrl-btn {
  display: inline-flex; align-items: center; justify-content: center;
  width: 32px; height: 32px; border-radius: 50%; font-size: 1rem;
  color: var(--text-secondary); background: none; border: none; cursor: pointer;
  transition: background 0.15s, color 0.15s;
}
.ctrl-btn:hover { background: var(--bg-hover); color: var(--text-primary); }
.play-btn { width: 40px; height: 40px; background: var(--accent) !important; color: #fff !important; }
.play-btn:hover { background: var(--accent-bright) !important; }

.np-seek  { display: flex; align-items: center; gap: 8px; width: 100%; }
.np-time  { font-size: 0.75rem; color: var(--text-muted); white-space: nowrap; min-width: 36px; }
.np-time.right { text-align: right; }

.seek-bar  { flex: 1; height: 4px; background: var(--bg-active); border-radius: 99px; cursor: pointer; position: relative; }
.seek-fill { position: absolute; left: 0; top: 0; bottom: 0; background: var(--accent); border-radius: 99px; pointer-events: none; }
.seek-thumb {
  position: absolute; top: 50%; width: 12px; height: 12px; background: #fff;
  border-radius: 50%; transform: translate(-50%,-50%);
  box-shadow: 0 0 0 3px var(--accent-glow);
  opacity: 0; pointer-events: none; transition: opacity 0.15s;
}
.seek-bar:hover .seek-thumb { opacity: 1; }

/* Right */
.np-right   { display: flex; align-items: center; gap: 8px; justify-content: flex-end; }
.vol-icon   { font-size: 0.9rem; }
.volume-bar { width: 80px; height: 4px; background: var(--bg-active); border-radius: 99px; cursor: pointer; position: relative; }
.volume-fill { position: absolute; left: 0; top: 0; bottom: 0; background: var(--text-secondary); border-radius: 99px; pointer-events: none; transition: background 0.1s; }
.volume-bar:hover .volume-fill { background: var(--accent); }
</style>
