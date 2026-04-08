<script lang="ts">
  import { goto } from '$app/navigation';
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

  function onVolMouseDown(event: MouseEvent) {
    const update = (moveEvent: MouseEvent) => {
      const rect = volBarEl.getBoundingClientRect();
      player.setVolume(clamp((moveEvent.clientX - rect.left) / rect.width, 0, 1));
    };
    update(event);
    const onUp = () => {
      window.removeEventListener('mousemove', update);
      window.removeEventListener('mouseup', onUp);
    };
    window.addEventListener('mousemove', update);
    window.addEventListener('mouseup', onUp);
  }

  async function onSeekKeyDown(event: KeyboardEvent) {
    if (!dur || dur <= 0) {
      return;
    }
    const step = event.shiftKey ? 0.1 : 0.03;
    let nextPct = pct;
    if (event.key === 'ArrowRight' || event.key === 'ArrowUp') {
      nextPct = clamp(pct + step, 0, 1);
    } else if (event.key === 'ArrowLeft' || event.key === 'ArrowDown') {
      nextPct = clamp(pct - step, 0, 1);
    } else if (event.key === 'Home') {
      nextPct = 0;
    } else if (event.key === 'End') {
      nextPct = 1;
    } else {
      return;
    }
    event.preventDefault();
    await player.seek(nextPct);
  }

  function onVolumeKeyDown(event: KeyboardEvent) {
    const step = event.shiftKey ? 0.12 : 0.05;
    let nextVolume = vol;
    if (event.key === 'ArrowRight' || event.key === 'ArrowUp') {
      nextVolume = clamp(vol + step, 0, 1);
    } else if (event.key === 'ArrowLeft' || event.key === 'ArrowDown') {
      nextVolume = clamp(vol - step, 0, 1);
    } else if (event.key === 'Home') {
      nextVolume = 0;
    } else if (event.key === 'End') {
      nextVolume = 1;
    } else {
      return;
    }
    event.preventDefault();
    player.setVolume(nextVolume);
  }

  $: track = $playbackState.current_track;
  $: pos = $playbackState.position_secs;
  $: dur = $playbackState.duration_secs;
  $: vol = $playbackState.volume;
  $: pct = $progressPct;
  $: ctx = $nowPlayingContext;
  $: signalTags = (ctx?.artist_tags ?? []).slice(0, 2);
  $: listenersLabel = ctx?.listeners ? `${Math.round(ctx.listeners / 1000)}k listeners` : null;
  $: albumLabel = track?.album || ctx?.album_title || null;
  $: qualityChip = track?.quality_tier
    ? track.quality_tier.replace(/_/g, ' ')
    : track?.bit_depth && track?.sample_rate
      ? `${track.bit_depth}-bit / ${(track.sample_rate / 1000).toFixed(1)}kHz`
      : null;
  $: formatChip = track?.format ? track.format.toUpperCase() : null;
  $: lyricsLabel = ctx?.lyrics_source ? `Lyrics ready` : null;

  async function handleNext() {
    await player.next();
    await loadQueue();
  }

  async function handlePrev() {
    await player.prev();
    await loadQueue();
  }

  function openNowPlayingShrine() {
    void goto('/now-playing');
  }
</script>

<div class="nowplaying-wrap">
  <div class="mood-strip" aria-hidden="true"></div>
  <div class="nowplaying">
  <div class="np-left">
    <button class="np-art np-art-btn" type="button" on:click={openNowPlayingShrine} title="Open now playing shrine">
      {#key track?.cover_art_path}
        {#if track?.cover_art_path}
          <img class="np-art-img" src={coverSrc(track.cover_art_path)} alt="Album artwork" />
        {:else}
          <div class="np-art-ph">No Art</div>
        {/if}
      {/key}
    </button>
    <div class="np-info">
      <div class="np-title">{track?.title ?? 'Nothing playing'}</div>
      <div class="np-artist">{track?.artist ?? 'Choose something to start listening'}</div>
      {#if albumLabel}
        <div class="np-album">{albumLabel}</div>
      {/if}
      <div class="np-chips">
        {#if qualityChip}
          <span class="np-chip np-chip-strong">{qualityChip}</span>
        {/if}
        {#if formatChip}
          <span class="np-chip">{formatChip}</span>
        {/if}
        {#if lyricsLabel}
          <span class="np-chip">{lyricsLabel}</span>
        {/if}
      </div>
      {#if appreciationLaneEnabled && track && (signalTags.length || listenersLabel)}
        <div class="np-signals">
          {#each signalTags as tag}
            <span class="np-signal-chip">{tag}</span>
          {/each}
          {#if listenersLabel}
            <span class="np-signal-muted">{listenersLabel}</span>
          {/if}
        </div>
      {/if}
    </div>
  </div>

  <div class="np-center">
    <div class="np-controls">
      <button class="ctrl-btn" on:click={handlePrev} title="Previous">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
          <polygon points="19 20 9 12 19 4 19 20"/><line x1="5" y1="19" x2="5" y2="5" stroke="currentColor" stroke-width="2" fill="none"/>
        </svg>
      </button>
      <button class="ctrl-btn play-btn" on:click={() => player.toggle()} title="Play or pause">
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
      <svelte:component
        this={PlaybackVisualizer}
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
      <div
        class="seek-bar"
        bind:this={seekBarEl}
        role="slider"
        tabindex="0"
        aria-label="Seek playback position"
        aria-valuemin="0"
        aria-valuemax="100"
        aria-valuenow={Math.round(pct * 100)}
        aria-valuetext={`${formatDuration(pos)} of ${formatDuration(dur)}`}
        on:mousedown={onSeekMouseDown}
        on:keydown={onSeekKeyDown}
      >
        <div class="seek-fill" style="width:{pct * 100}%"></div>
        <div class="seek-thumb" style="left:{pct * 100}%"></div>
      </div>
      <span class="np-time right">{formatDuration(dur)}</span>
    </div>
  </div>

  <div class="np-right">
    <button class="focus-btn" type="button" on:click={openNowPlayingShrine}>Shrine</button>
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
    <div
      class="volume-bar"
      bind:this={volBarEl}
      role="slider"
      tabindex="0"
      aria-label="Playback volume"
      aria-valuemin="0"
      aria-valuemax="100"
      aria-valuenow={Math.round(vol * 100)}
      aria-valuetext={`${Math.round(vol * 100)} percent`}
      on:mousedown={onVolMouseDown}
      on:keydown={onVolumeKeyDown}
    >
      <div class="volume-fill" style="width:{vol * 100}%"></div>
    </div>
  </div>
</div>
</div>

<style>
  .nowplaying-wrap {
    display: flex;
    flex-direction: column;
  }

  .mood-strip {
    height: 3px;
    background: linear-gradient(
      90deg,
      rgba(var(--mood-accent-rgb), 0.9) 0%,
      rgba(247, 180, 92, 0.6) 50%,
      rgba(139, 180, 212, 0.4) 100%
    );
    box-shadow: 0 0 10px rgba(var(--mood-accent-rgb), 0.5);
    transition: background var(--mood-shift-ms) ease, box-shadow var(--mood-shift-ms) ease;
    flex-shrink: 0;
  }

  .nowplaying {
    display: grid;
    grid-template-columns: minmax(0, 1.25fr) minmax(320px, 1fr) auto;
    align-items: center;
    height: var(--playerbar-h);
    padding: 0 14px;
    gap: 14px;
  }

  .np-left {
    display: flex;
    align-items: center;
    gap: 12px;
    overflow: hidden;
  }

  .np-art {
    width: 58px;
    height: 58px;
    flex-shrink: 0;
    border-radius: 10px;
    overflow: hidden;
    background: var(--bg-card);
    box-shadow: 0 4px 20px rgba(var(--mood-accent-rgb), 0.2), 0 0 0 1px rgba(255,255,255,0.04), inset 0 1px 0 rgba(255,255,255,0.1);
    border: 1px solid rgba(var(--mood-accent-rgb), 0.25);
    transition: border-color var(--mood-shift-ms) ease, box-shadow var(--mood-shift-ms) ease;
    position: relative;
  }

  .np-art-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    animation: art-fade-in 300ms ease forwards;
  }

  :global(.low-motion) .np-art-img {
    animation: none;
  }

  .np-art-ph {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.72rem;
    color: var(--text-muted);
  }

  .np-art-btn {
    border: none;
    padding: 0;
    cursor: pointer;
  }

  .np-art-btn:hover {
    filter: brightness(1.06);
  }

  .np-info {
    overflow: hidden;
  }

  .np-title {
    font-weight: 700;
    font-size: 0.92rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-primary);
  }

  .np-artist {
    font-size: 0.78rem;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-top: 1px;
  }

  .np-album {
    font-size: 0.74rem;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-top: 1px;
  }

  .np-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-top: 6px;
  }

  .np-chip {
    font-size: 0.62rem;
    color: var(--text-secondary);
    background: rgba(113, 131, 152, 0.16);
    border: 1px solid rgba(113, 131, 152, 0.2);
    border-radius: 999px;
    padding: 2px 7px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .np-chip-strong {
    color: rgba(var(--mood-accent-rgb), 1);
    border-color: rgba(var(--mood-accent-rgb), 0.3);
    background: rgba(var(--mood-accent-rgb), 0.12);
    transition: color var(--mood-shift-ms) ease, border-color var(--mood-shift-ms) ease, background var(--mood-shift-ms) ease;
  }

  .np-signals {
    display: flex;
    align-items: center;
    gap: 5px;
    margin-top: 6px;
    white-space: nowrap;
    overflow: hidden;
  }

  .np-signal-chip {
    font-size: 0.62rem;
    color: var(--accent-bright);
    background: rgba(247, 180, 92, 0.12);
    border: 1px solid rgba(247, 180, 92, 0.2);
    border-radius: 999px;
    padding: 1px 6px;
  }

  .np-signal-muted {
    font-size: 0.64rem;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .np-center {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    min-width: 320px;
    max-width: 560px;
    width: 100%;
  }

  .np-controls {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .ctrl-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    font-size: 0.9rem;
    color: var(--text-secondary);
    background: none;
    border: none;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }

  .ctrl-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .play-btn {
    width: 40px;
    height: 40px;
    background: linear-gradient(135deg, rgba(var(--mood-accent-rgb), 1) 0%, var(--primary) 100%) !important;
    color: var(--bg-deep) !important;
    animation: play-ring-pulse 3s ease-in-out infinite;
    transition: background var(--mood-shift-ms) ease;
  }

  .play-btn:hover {
    filter: brightness(1.12) !important;
  }

  :global(.low-motion) .play-btn {
    animation: none;
    box-shadow: 0 4px 16px rgba(var(--mood-accent-rgb), 0.3);
  }

  .np-seek {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
  }

  .np-time {
    font-size: 0.7rem;
    color: var(--text-secondary);
    white-space: nowrap;
    min-width: 34px;
  }

  .np-time.right {
    text-align: right;
  }

  .seek-bar {
    position: relative;
    flex: 1;
    height: 4px;
    border-radius: 99px;
    background: rgba(255, 255, 255, 0.07);
    cursor: pointer;
    overflow: visible;
  }

  .seek-fill {
    height: 100%;
    border-radius: 99px;
    background: linear-gradient(90deg, rgba(var(--mood-accent-rgb), 0.9), rgba(247, 180, 92, 0.6));
    box-shadow: 0 0 8px rgba(var(--mood-accent-rgb), 0.5);
    transition: background var(--mood-shift-ms) ease, box-shadow var(--mood-shift-ms) ease;
    position: relative;
  }

  .seek-thumb {
    position: absolute;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--text-primary);
    box-shadow: 0 0 6px rgba(var(--mood-accent-rgb), 0.7);
    opacity: 0;
    transition: opacity 0.15s ease, box-shadow var(--mood-shift-ms) ease;
    pointer-events: none;
  }

  .seek-bar:hover .seek-thumb {
    opacity: 1;
  }

  .np-right {
    display: flex;
    align-items: center;
    gap: 8px;
    justify-content: flex-end;
  }

  .focus-btn {
    border: 1px solid var(--border);
    border-radius: 999px;
    background: rgba(139, 180, 212, 0.08);
    color: var(--text-accent);
    padding: 5px 11px;
    font-size: 0.72rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    font-weight: 700;
  }

  .focus-btn:hover {
    background: rgba(139, 180, 212, 0.14);
    border-color: var(--border-active);
  }

  .vol-icon {
    font-size: 0.82rem;
    color: var(--text-secondary);
    display: flex;
    align-items: center;
    flex-shrink: 0;
  }
</style>
