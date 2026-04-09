<script lang="ts">
  import { onDestroy } from 'svelte';
  import { browser } from '$app/environment';
  import { loadMilkdropPresetMap } from '$lib/visualizer/presets';

  export let positionSecs = 0;
  export let durationSecs = 0;
  export let isPlaying = false;
  export let lowMotion = false;
  export let mode: 'waveform' | 'spectrum' | 'milkdrop' = 'spectrum';
  export let presetName = '';
  export let fpsCap = 30;

  const BAR_COUNT = 16;
  const TRANSITION_SECONDS = 2.2;

  let canvasEl: HTMLCanvasElement | null = null;
  let isMilkdropReady = false;
  let milkdropError: string | null = null;

  let milkdropVisualizer: {
    setRendererSize: (width: number, height: number) => void;
    loadPreset: (preset: unknown, blendSeconds?: number) => void;
    render: () => void;
  } | null = null;
  let milkdropPresets: Record<string, unknown> = {};

  let audioContext: AudioContext | null = null;
  let rafHandle = 0;
  let resizeObserver: ResizeObserver | null = null;
  let loadedPreset = '';
  let visibilityListenerAttached = false;
  let lastRenderTimestamp = 0;

  $: phase = durationSecs > 0 ? positionSecs / durationSecs : 0;
  $: bars = Array.from({ length: BAR_COUNT }, (_, i) => {
    const wave = Math.sin((phase * 8 + i * 0.67) * Math.PI * 2);
    const resting = 0.18 + (i % 4) * 0.02;
    const active = Math.abs(wave) * (isPlaying ? 0.52 : 0.2);
    return Math.min(0.92, resting + active);
  });

  $: useMilkdrop = browser && mode === 'milkdrop' && !lowMotion;
  $: if (useMilkdrop && canvasEl && !isMilkdropReady) {
    void initMilkdrop();
  }
  $: if (!useMilkdrop && isMilkdropReady) {
    stopMilkdropLoop();
  }
  $: if (useMilkdrop && isMilkdropReady && presetName && presetName !== loadedPreset) {
    loadPresetByName(presetName);
  }
  $: sanitizedFpsCap = Math.min(60, Math.max(15, Math.round(fpsCap || 30)));
  $: frameIntervalMs = Math.floor(1000 / sanitizedFpsCap);
  $: waveformPoints = Array.from({ length: 42 }, (_, i) => {
    const x = (i / 41) * 100;
    const base = Math.sin((phase * 4.2 + i * 0.34) * Math.PI * 2);
    const harmonic = Math.sin((phase * 7.8 + i * 0.18) * Math.PI * 2) * 0.35;
    const amplitude = isPlaying ? 16 : 8;
    const y = 50 - (base + harmonic) * amplitude;
    return `${x.toFixed(2)},${y.toFixed(2)}`;
  }).join(' ');

  onDestroy(() => {
    stopMilkdropLoop();
    resizeObserver?.disconnect();
    resizeObserver = null;
    if (visibilityListenerAttached) {
      document.removeEventListener('visibilitychange', onVisibilityChange);
      visibilityListenerAttached = false;
    }
  });

  function onVisibilityChange() {
    if (!useMilkdrop || !isMilkdropReady) {
      return;
    }
    if (document.visibilityState === 'hidden') {
      stopMilkdropLoop();
    } else {
      startMilkdropLoop();
    }
  }

  function stopMilkdropLoop() {
    if (rafHandle) {
      cancelAnimationFrame(rafHandle);
      rafHandle = 0;
    }
  }

  function attachResizeObserver() {
    if (!canvasEl || resizeObserver) {
      return;
    }
    resizeObserver = new ResizeObserver(() => {
      setCanvasRenderSize();
    });
    resizeObserver.observe(canvasEl);
  }

  function setCanvasRenderSize() {
    if (!canvasEl || !milkdropVisualizer) {
      return;
    }

    const dpr = Math.max(1, window.devicePixelRatio || 1);
    const width = Math.max(220, Math.floor(canvasEl.clientWidth * dpr));
    const height = Math.max(20, Math.floor(canvasEl.clientHeight * dpr));
    canvasEl.width = width;
    canvasEl.height = height;
    milkdropVisualizer.setRendererSize(width, height);
  }

  function startMilkdropLoop() {
    stopMilkdropLoop();
    lastRenderTimestamp = 0;
    const renderFrame = (timestamp: number) => {
      if (!milkdropVisualizer || !useMilkdrop) {
        return;
      }
      if (document.visibilityState === 'hidden') {
        rafHandle = requestAnimationFrame(renderFrame);
        return;
      }
      const targetInterval = isPlaying ? frameIntervalMs : frameIntervalMs * 2;
      if (lastRenderTimestamp !== 0 && timestamp - lastRenderTimestamp < targetInterval) {
        rafHandle = requestAnimationFrame(renderFrame);
        return;
      }
      lastRenderTimestamp = timestamp;
      try {
        milkdropVisualizer.render();
      } catch {
        milkdropError = 'MilkDrop render fallback active';
        isMilkdropReady = false;
        return;
      }
      rafHandle = requestAnimationFrame(renderFrame);
    };
    rafHandle = requestAnimationFrame(renderFrame);
  }

  function pickDefaultPresetName(): string | null {
    const names = Object.keys(milkdropPresets);
    if (names.length === 0) {
      return null;
    }
    const requested = presetName.trim();
    if (requested && milkdropPresets[requested]) {
      return requested;
    }
    return names[0];
  }

  function loadPresetByName(name: string) {
    if (!milkdropVisualizer || !milkdropPresets[name]) {
      return;
    }
    milkdropVisualizer.loadPreset(milkdropPresets[name], TRANSITION_SECONDS);
    loadedPreset = name;
  }

  async function initMilkdrop() {
    if (!browser || !canvasEl || isMilkdropReady || !useMilkdrop) {
      return;
    }

    try {
      const butterchurnModule = await import('butterchurn');
      milkdropPresets = await loadMilkdropPresetMap();

      const audioContextCtor = window.AudioContext || (window as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
      if (!audioContextCtor) {
        milkdropError = 'WebAudio unavailable';
        return;
      }
      audioContext = audioContext ?? new audioContextCtor();

      const butterchurnFactory = (butterchurnModule.default ?? butterchurnModule) as {
        createVisualizer: (
          context: AudioContext,
          canvas: HTMLCanvasElement,
          options: Record<string, number>
        ) => typeof milkdropVisualizer;
      };

      milkdropVisualizer = butterchurnFactory.createVisualizer(audioContext, canvasEl, {
        width: Math.max(220, Math.floor(canvasEl.clientWidth)),
        height: Math.max(20, Math.floor(canvasEl.clientHeight)),
        meshWidth: 32,
        meshHeight: 24,
        pixelRatio: Math.max(1, window.devicePixelRatio || 1),
        textureRatio: 1,
      });

      const firstPreset = pickDefaultPresetName();
      if (firstPreset) {
        loadPresetByName(firstPreset);
      }

      setCanvasRenderSize();
      attachResizeObserver();
      if (!visibilityListenerAttached) {
        document.addEventListener('visibilitychange', onVisibilityChange);
        visibilityListenerAttached = true;
      }
      isMilkdropReady = true;
      milkdropError = null;
      startMilkdropLoop();
    } catch {
      milkdropError = 'MilkDrop unavailable, using bars';
      isMilkdropReady = false;
      stopMilkdropLoop();
    }
  }
</script>

{#if useMilkdrop}
  <div class="viz-canvas-wrap" aria-hidden="true">
    <canvas bind:this={canvasEl} class="viz-canvas"></canvas>
    {#if milkdropError}
      <span class="viz-fallback-note">fallback</span>
    {/if}
  </div>
{:else if mode === 'waveform'}
  <div class="viz-wave-wrap" aria-hidden="true">
    <svg viewBox="0 0 100 100" preserveAspectRatio="none" class="viz-wave-svg">
      <polyline class="viz-wave-line" points={waveformPoints}></polyline>
    </svg>
  </div>
{:else}
  <div class="viz" class:low-motion={lowMotion} aria-hidden="true">
    {#each bars as value, i}
      <span
        class="viz-bar"
        class:playing={isPlaying}
        style="height: {Math.max(14, Math.round(value * 100))}%; animation-delay: {-i * 0.07}s;"
      ></span>
    {/each}
  </div>
{/if}

<style>
  .viz {
    width: 100%;
    height: 16px;
    display: grid;
    grid-template-columns: repeat(16, 1fr);
    align-items: end;
    gap: 2px;
    margin-bottom: 2px;
  }

  .viz-canvas-wrap {
    width: 100%;
    height: 20px;
    margin-bottom: 1px;
    border-radius: 3px;
    overflow: hidden;
    border: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
    background: radial-gradient(circle at 18% 50%, rgba(139, 180, 212, 0.12), rgba(0, 0, 0, 0));
    position: relative;
  }

  .viz-wave-wrap {
    width: 100%;
    height: 18px;
    margin-bottom: 2px;
    border-radius: 3px;
    overflow: hidden;
    border: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
    background: linear-gradient(90deg, rgba(139, 180, 212, 0.06), rgba(139, 180, 212, 0.12));
  }

  .viz-wave-svg {
    width: 100%;
    height: 100%;
    display: block;
  }

  .viz-wave-line {
    fill: none;
    stroke: color-mix(in srgb, var(--primary) 82%, white 6%);
    stroke-width: 2.1;
    stroke-linecap: round;
    stroke-linejoin: round;
    opacity: 0.92;
  }

  .viz-canvas {
    width: 100%;
    height: 100%;
    display: block;
  }

  .viz-fallback-note {
    position: absolute;
    right: 6px;
    bottom: 1px;
    font-size: 0.55rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
    opacity: 0.8;
    pointer-events: none;
  }

  .viz-bar {
    border-radius: 2px 2px 0 0;
    background: color-mix(in srgb, var(--primary) 78%, var(--bg-card));
    opacity: 0.7;
    transform-origin: bottom;
  }

  .viz-bar.playing {
    animation: vizPulse 1.3s ease-in-out infinite;
    opacity: 0.95;
  }

  .viz.low-motion .viz-bar.playing {
    animation: none;
  }

  @keyframes vizPulse {
    0%,
    100% {
      transform: scaleY(0.92);
    }
    50% {
      transform: scaleY(1.05);
    }
  }
</style>
