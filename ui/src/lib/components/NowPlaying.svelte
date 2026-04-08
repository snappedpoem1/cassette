<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api/tauri';
  import { playbackState, isPlaying, progressPct, isSeeking, seekPreview, player, nowPlayingContext } from '$lib/stores/player';
  import { loadQueue } from '$lib/stores/queue';
  import { formatDuration, coverSrc, clamp } from '$lib/utils';

  let seekBarEl: HTMLDivElement;
  let volBarEl: HTMLDivElement;
  let visualizerEnabled = true;
  let visualizerLowMotion = false;
  let appreciationLaneEnabled = true;
  let visualizerMode: 'waveform' | 'spectrum' | 'milkdrop' = 'spectrum';
  let visualizerPreset = '';
  let visualizerFpsCap = 30;
  let PlaybackVisualizer: typeof import('$lib/components/PlaybackVisualizer.svelte').default | null = null;
  let NowPlayingExpanded: typeof import('$lib/components/NowPlayingExpanded.svelte').default | null = null;
  let isExpandedOpen = false;

  onMount(async () => {
    try {
      const enabled = await api.getSetting('ui_visualizer_enabled');
      const lowMotion = await api.getSetting('ui_visualizer_low_motion');
      const appreciation = await api.getSetting('ui_appreciation_lane_enabled');
      const mode = await api.getSetting('ui_visualizer_mode');
      const preset = await api.getSetting('ui_visualizer_preset');
      const fpsCap = await api.getSetting('ui_visualizer_fps_cap');
      visualizerEnabled = enabled !== 'false';
      visualizerLowMotion = lowMotion === 'true';
      appreciationLaneEnabled = appreciation !== 'false';
      visualizerMode = mode === 'milkdrop' || mode === 'waveform' ? mode : 'spectrum';
      visualizerPreset = preset ?? '';
      visualizerFpsCap = Math.min(60, Math.max(15, Number.parseInt(fpsCap ?? '30', 10) || 30));
    } catch {
      visualizerEnabled = true;
      visualizerLowMotion = false;
      appreciationLaneEnabled = true;
      visualizerMode = 'spectrum';
      visualizerPreset = '';
      visualizerFpsCap = 30;
    }

    const module = await import('$lib/components/PlaybackVisualizer.svelte');
    PlaybackVisualizer = module.default;
  });

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
  $: ctx = $nowPlayingContext;
  $: signalTags = (ctx?.artist_tags ?? []).slice(0, 2);
  $: listenersLabel = ctx?.listeners ? `${Math.round(ctx.listeners / 1000)}k listeners` : null;
  $: lyricsLabel = ctx?.lyrics_source ? `Lyrics: ${ctx.lyrics_source}` : null;

  async function handleNext() { await player.next(); await loadQueue(); }
  async function handlePrev() { await player.prev(); await loadQueue(); }

  async function openExpandedNowPlaying() {
    if (!NowPlayingExpanded) {
      const module = await import('$lib/components/NowPlayingExpanded.svelte');
      NowPlayingExpanded = module.default;
    }
    isExpandedOpen = true;
  }

  function closeExpandedNowPlaying() {
    isExpandedOpen = false;
  }
</script>

<div class="nowplaying">
  <!-- Left: art + info -->
  <div class="np-left">
    <button class="np-art np-art-btn" type="button" on:click={openExpandedNowPlaying} title="Open expanded now playing">
      {#if track?.cover_art_path}
        <img src={coverSrc(track.cover_art_path)} alt="cover" />
      {:else}
        <div class="np-art-ph">🎵</div>
      {/if}
    </button>
    <div class="np-info">
      <div class="np-title">{track?.title ?? '—'}</div>
      <div class="np-artist">{track?.artist ?? 'No track playing'}</div>
      {#if appreciationLaneEnabled && track && (signalTags.length || listenersLabel || lyricsLabel)}
        <div class="np-signals">
          {#each signalTags as tag}
            <span class="np-signal-chip">{tag}</span>
          {/each}
          {#if listenersLabel}
            <span class="np-signal-muted">{listenersLabel}</span>
          {/if}
          {#if lyricsLabel}
            <span class="np-signal-muted">{lyricsLabel}</span>
          {/if}
        </div>
      {/if}
    </div>
  </div>

  <!-- Center: controls + seek -->
  <div class="np-center">
    <div class="np-controls">
      <button class="ctrl-btn" on:click={handlePrev} title="Previous">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
          <polygon points="19 20 9 12 19 4 19 20"/><line x1="5" y1="19" x2="5" y2="5" stroke="currentColor" stroke-width="2" fill="none"/>
        </svg>
      </button>
      <button class="ctrl-btn play-btn" on:click={() => player.toggle()} title="Play/Pause">
        {#if $isPlaying}
          <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
            <rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/>
          </svg>
        {:else}
          <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
            <polygon points="5 3 19 12 5 21 5 3"/>
          </svg>
        {/if}
      </button>
      <button class="ctrl-btn" on:click={handleNext} title="Next">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
          <polygon points="5 4 15 12 5 20 5 4"/><line x1="19" y1="5" x2="19" y2="19" stroke="currentColor" stroke-width="2" fill="none"/>
        </svg>
      </button>
    </div>
    {#if visualizerEnabled && PlaybackVisualizer}
      <svelte:component this={PlaybackVisualizer}
        positionSecs={pos}
        durationSecs={dur}
        isPlaying={$isPlaying}
        lowMotion={visualizerLowMotion}
        mode={visualizerMode}
        presetName={visualizerPreset}
        fpsCap={visualizerFpsCap}
      />
    {/if}
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
    <span class="vol-icon">
      {#if vol === 0}
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><line x1="23" y1="9" x2="17" y2="15"/><line x1="17" y1="9" x2="23" y2="15"/>
        </svg>
      {:else if vol < 0.5}
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><path d="M15.54 8.46a5 5 0 0 1 0 7.07"/>
        </svg>
      {:else}
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"/>
        </svg>
      {/if}
    </span>
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div class="volume-bar" bind:this={volBarEl} on:mousedown={onVolMouseDown}>
      <div class="volume-fill" style="width:{vol*100}%"></div>
    </div>
  </div>
</div>

{#if NowPlayingExpanded}
  <svelte:component this={NowPlayingExpanded} open={isExpandedOpen} onClose={closeExpandedNowPlaying} />
{/if}

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
.np-art-btn { border: none; padding: 0; cursor: pointer; }
.np-art-btn:hover { filter: brightness(1.08); }
.np-info    { overflow: hidden; }
.np-title   { font-weight: 600; font-size: 0.82rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: var(--text-primary); }
.np-artist  { font-size: 0.72rem; color: var(--text-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; margin-top: 1px; }
.np-signals {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-top: 3px;
  white-space: nowrap;
  overflow: hidden;
}
.np-signal-chip {
  font-size: 0.62rem;
  color: var(--primary);
  background: rgba(139,180,212,0.12);
  border: 1px solid rgba(139,180,212,0.2);
  border-radius: 999px;
  padding: 1px 6px;
}
.np-signal-muted {
  font-size: 0.62rem;
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
}

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
.seek-bar {
  flex: 1;
  height: 3px;
  background: var(--bg-active);
  border-radius: 99px;
  cursor: pointer;
  position: relative;
}
.seek-fill {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  background: var(--primary);
  border-radius: 99px;
  pointer-events: none;
}
.seek-thumb {
  position: absolute;
  top: 50%;
  transform: translate(-50%, -50%);
  width: 10px;
  height: 10px;
  background: var(--primary);
  border-radius: 50%;
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.15s;
}
.np-seek:hover .seek-thumb { opacity: 1; }

.np-right   { display: flex; align-items: center; gap: 7px; justify-content: flex-end; }
.vol-icon {
  font-size: 0.82rem;
  color: var(--text-muted);
  display: flex;
  align-items: center;
  flex-shrink: 0;
}
.volume-bar {
  width: 72px;
  height: 3px;
  background: var(--bg-active);
  border-radius: 99px;
  cursor: pointer;
  position: relative;
}
.volume-fill {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  background: var(--text-secondary);
  border-radius: 99px;
  pointer-events: none;
}
</style>
