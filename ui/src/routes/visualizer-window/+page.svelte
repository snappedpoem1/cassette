<script lang="ts">
  import { browser } from '$app/environment';
  import { onDestroy, onMount } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { api, toDesktopRuntimeMessage } from '$lib/api/tauri';
  import {
    currentTrack,
    isPlaying,
    playbackState,
    startPlayerEventListener,
    startPlayerPoll,
    stopPlayerEventListener,
    stopPlayerPoll,
  } from '$lib/stores/player';
  import { coverSrc, formatDuration } from '$lib/utils';

  const WINDOW_GEOMETRY_KEY = 'cassette.shell.visualizerWindowGeometry';

  let PlaybackVisualizer: typeof import('$lib/components/PlaybackVisualizer.svelte').default | null = null;
  let visualizerLowMotion = false;
  let visualizerMode: 'waveform' | 'spectrum' | 'milkdrop' = 'spectrum';
  let visualizerPreset = '';
  let visualizerFpsCap = 30;
  let windowActionError: string | null = null;

  let unlistenMoved: (() => void) | null = null;
  let unlistenResized: (() => void) | null = null;

  async function persistWindowGeometry(): Promise<void> {
    if (!browser) {
      return;
    }

    try {
      const appWindow = getCurrentWindow();
      const [position, size, scaleFactor] = await Promise.all([
        appWindow.outerPosition(),
        appWindow.innerSize(),
        appWindow.scaleFactor(),
      ]);

      const logicalGeometry = {
        x: Math.round(position.x / scaleFactor),
        y: Math.round(position.y / scaleFactor),
        width: Math.round(size.width / scaleFactor),
        height: Math.round(size.height / scaleFactor),
      };

      window.localStorage.setItem(WINDOW_GEOMETRY_KEY, JSON.stringify(logicalGeometry));
    } catch {
      // Geometry persistence is best-effort for this first proof.
    }
  }

  async function loadVisualizerPrefs(): Promise<void> {
    try {
      const lowMotion = await api.getSetting('ui_visualizer_low_motion');
      const mode = await api.getSetting('ui_visualizer_mode');
      const preset = await api.getSetting('ui_visualizer_preset');
      const fpsCap = await api.getSetting('ui_visualizer_fps_cap');
      visualizerLowMotion = lowMotion === 'true';
      visualizerMode = mode === 'milkdrop' || mode === 'waveform' ? mode : 'spectrum';
      visualizerPreset = preset ?? '';
      visualizerFpsCap = Math.min(60, Math.max(15, Number.parseInt(fpsCap ?? '30', 10) || 30));
    } catch {
      visualizerLowMotion = false;
      visualizerMode = 'spectrum';
      visualizerPreset = '';
      visualizerFpsCap = 30;
    }
  }

  onMount(async () => {
    startPlayerPoll();
    void startPlayerEventListener();

    try {
      const initialState = await api.getPlaybackState();
      playbackState.set(initialState);
    } catch {
      // Initial state will fall back to the poll loop.
    }

    await loadVisualizerPrefs();
    const module = await import('$lib/components/PlaybackVisualizer.svelte');
    PlaybackVisualizer = module.default;

    try {
      const appWindow = getCurrentWindow();
      unlistenMoved = await appWindow.onMoved(() => {
        void persistWindowGeometry();
      });
      unlistenResized = await appWindow.onResized(() => {
        void persistWindowGeometry();
      });
      await persistWindowGeometry();
      windowActionError = null;
    } catch (error) {
      windowActionError = toDesktopRuntimeMessage(
        error,
        'Visualizer window opened, but its geometry could not be persisted.'
      );
    }
  });

  onDestroy(() => {
    stopPlayerEventListener();
    stopPlayerPoll();
    unlistenMoved?.();
    unlistenMoved = null;
    unlistenResized?.();
    unlistenResized = null;
  });

  $: track = $currentTrack;
  $: positionSecs = $playbackState.position_secs;
  $: durationSecs = $playbackState.duration_secs;
  $: modeLabel =
    visualizerMode === 'milkdrop'
      ? 'MilkDrop-style preset playback'
      : visualizerMode === 'waveform'
        ? 'Decorative waveform'
        : 'Decorative spectrum';
</script>

<svelte:head>
  <title>Cassette Visualizer</title>
</svelte:head>

<div class="viz-window">
  <div class="viz-window__backdrop" aria-hidden="true"></div>

  <header class="viz-window__header">
    <div class="viz-window__identity">
      <div class="viz-window__kicker">Visualizer breakout proof</div>
      <div class="viz-window__title">{track?.title ?? 'Nothing playing'}</div>
      <div class="viz-window__meta">
        <span>{track?.artist ?? 'Choose something in the main shell to start playback'}</span>
        {#if track?.album}
          <span class="viz-window__dot"></span>
          <span>{track.album}</span>
        {/if}
      </div>
    </div>

    <div class="viz-window__badges">
      <span class="viz-window__badge">{modeLabel}</span>
      <span class="viz-window__badge">{formatDuration(positionSecs)} / {formatDuration(durationSecs)}</span>
    </div>
  </header>

  <section class="viz-window__stage">
    <div class="viz-window__art">
      {#if track?.cover_art_path}
        <img src={coverSrc(track.cover_art_path)} alt="Album artwork" />
      {:else}
        <div class="viz-window__art-placeholder">Cassette</div>
      {/if}
    </div>

    <div class="viz-window__visual">
      {#if PlaybackVisualizer}
        <div class="viz-window__visual-shell">
          <svelte:component
            this={PlaybackVisualizer}
            positionSecs={positionSecs}
            durationSecs={durationSecs}
            isPlaying={$isPlaying}
            lowMotion={visualizerLowMotion}
            mode={visualizerMode}
            presetName={visualizerPreset}
            fpsCap={visualizerFpsCap}
          />
        </div>
      {:else}
        <div class="viz-window__visual-shell viz-window__visual-shell-loading">
          Loading visualizer surface…
        </div>
      {/if}

      <div class="viz-window__note">
        This first detached window proves real breakout behavior and remembered geometry. The current visualizer remains decorative or preset-driven rather than true audio-reactive signal analysis.
      </div>
      {#if windowActionError}
        <div class="viz-window__error" role="status">{windowActionError}</div>
      {/if}
    </div>
  </section>
</div>

<style>
  .viz-window {
    min-height: 100vh;
    padding: 20px 22px;
    background:
      radial-gradient(circle at top left, rgba(var(--mood-accent-rgb), 0.16), transparent 36%),
      radial-gradient(circle at bottom right, rgba(247, 180, 92, 0.1), transparent 34%),
      linear-gradient(180deg, rgba(7, 10, 16, 0.98), rgba(5, 8, 13, 1));
    color: var(--text-primary);
    display: grid;
    gap: 18px;
    overflow: hidden;
  }

  .viz-window__backdrop {
    position: fixed;
    inset: 0;
    pointer-events: none;
    background:
      linear-gradient(90deg, rgba(255, 255, 255, 0.02), transparent 24%, transparent 76%, rgba(255, 255, 255, 0.02)),
      linear-gradient(180deg, rgba(255, 255, 255, 0.015), transparent 35%);
    opacity: 0.55;
  }

  .viz-window__header,
  .viz-window__stage {
    position: relative;
    z-index: 1;
  }

  .viz-window__header {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    align-items: flex-start;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    padding-bottom: 14px;
  }

  .viz-window__kicker {
    font-size: 0.64rem;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--accent-bright);
    font-weight: 700;
  }

  .viz-window__title {
    margin-top: 4px;
    font-size: clamp(1.1rem, 2vw, 1.5rem);
    font-weight: 700;
    line-height: 1.1;
  }

  .viz-window__meta {
    margin-top: 6px;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    color: var(--text-secondary);
    font-size: 0.8rem;
  }

  .viz-window__dot {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.32);
  }

  .viz-window__badges {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
  }

  .viz-window__badge {
    padding: 5px 10px;
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: var(--text-secondary);
    background: rgba(255, 255, 255, 0.04);
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .viz-window__stage {
    display: grid;
    grid-template-columns: minmax(200px, 280px) minmax(0, 1fr);
    gap: 18px;
    min-height: 0;
    align-items: stretch;
  }

  .viz-window__art {
    min-height: 0;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.03);
    overflow: hidden;
    border-radius: 18px;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.06);
    aspect-ratio: 1;
    align-self: start;
  }

  .viz-window__art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .viz-window__art-placeholder {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    color: var(--text-muted);
    letter-spacing: 0.12em;
    text-transform: uppercase;
    font-size: 0.78rem;
  }

  .viz-window__visual {
    min-width: 0;
    display: grid;
    grid-template-rows: minmax(160px, 1fr) auto auto;
    gap: 12px;
  }

  .viz-window__visual-shell {
    min-height: 0;
    border-radius: 18px;
    border: 1px solid rgba(var(--mood-accent-rgb), 0.18);
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 38%),
      rgba(8, 12, 18, 0.98);
    padding: 18px;
    display: grid;
    align-items: end;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.34);
  }

  .viz-window__visual-shell-loading {
    place-items: center;
    color: var(--text-muted);
    font-size: 0.8rem;
  }

  .viz-window__note,
  .viz-window__error {
    border-radius: 12px;
    padding: 10px 12px;
    font-size: 0.78rem;
    line-height: 1.5;
  }

  .viz-window__note {
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: var(--text-secondary);
    background: rgba(255, 255, 255, 0.03);
  }

  .viz-window__error {
    border: 1px solid rgba(255, 143, 143, 0.24);
    color: var(--status-error, #ffb4b4);
    background: rgba(120, 24, 24, 0.18);
  }

  @media (max-width: 860px) {
    .viz-window {
      padding: 16px;
    }

    .viz-window__header,
    .viz-window__stage {
      grid-template-columns: 1fr;
    }

    .viz-window__badges {
      justify-content: flex-start;
    }

    .viz-window__art {
      max-width: 260px;
    }
  }
</style>
