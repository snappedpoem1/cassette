<script lang="ts">
  import { page } from '$app/stores';
  import '../app.css';
  import { onMount, onDestroy } from 'svelte';
  import { browser } from '$app/environment';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import LibraryRail from '$lib/components/LibraryRail.svelte';
  import NowPlaying from '$lib/components/NowPlaying.svelte';
  import RightSidebar from '$lib/components/RightSidebar.svelte';
  import CommandPalette from '$lib/components/CommandPalette.svelte';
  import SystemStatusStrip from '$lib/components/SystemStatusStrip.svelte';
  import WorkstationDeck from '$lib/components/WorkstationDeck.svelte';
  import { api, type Track } from '$lib/api/tauri';
  import {
    startPlayerEventListener,
    startPlayerPoll,
    stopPlayerEventListener,
    stopPlayerPoll,
  } from '$lib/stores/player';
  import { currentTrack } from '$lib/stores/player';
  import { loadLibrary } from '$lib/stores/library';
  import {
    loadDownloadConfig,
    refreshBacklogStatus,
    refreshSlskdRuntimeStatus,
    startDownloadSupervision,
    stopDownloadSupervision,
  } from '$lib/stores/downloads';
  import { openPalette } from '$lib/stores/commands';
  import {
    activeWorkspacePreset,
    applyWorkspacePreset,
    compactPlayerMode,
    libraryRailWidth,
    minimizeAppWindow,
    openUtilityWell,
    shellActionError,
    setLibraryRailWidth,
    setUtilityWellWidth,
    toggleCompactPlayerMode,
    toggleUtilityWellCollapsed,
    toggleWorkstationDeck,
    utilityWellCollapsed,
    utilityWellMode,
    utilityWellWidth,
    workstationDeckOpen,
  } from '$lib/stores/shell';
  import { loadQueue, queue } from '$lib/stores/queue';

  let dynamicGlassEnabled = true;
  let dynamicGlassLowMotion = false;
  let dynamicGlassIntensity = 62;
  let moodStyle = '--mood-accent-rgb: 139,180,212; --mood-layer-a: rgba(139,180,212,0.08); --mood-layer-b: rgba(247,180,92,0.06); --mood-blur: 24px; --mood-shift-ms: 460ms;';
  let lastMoodCover = '';
  let resizeCleanup: (() => void) | null = null;
  let lastQueueCount = 0;
  $: isImmersionRoute = $page.url.pathname === '/now-playing';
  $: isVisualizerWindowRoute = $page.url.pathname === '/visualizer-window';
  $: shellStyle = `${moodStyle}; --library-rail-w: ${$libraryRailWidth}px; --utility-well-w: ${
    isImmersionRoute || $utilityWellCollapsed ? 0 : $utilityWellWidth
  }px;`;

  $: if (browser) {
    document.documentElement.classList.toggle('low-motion', dynamicGlassLowMotion);
  }

  $: if (browser) {
    const queueCount = $queue.length;
    if (queueCount > 0 && lastQueueCount === 0 && !isImmersionRoute) {
      openUtilityWell('queue');
    }
    lastQueueCount = queueCount;
  }

  function clamp(value: number, min: number, max: number): number {
    return Math.max(min, Math.min(max, value));
  }

  function beginHorizontalResize(target: 'library' | 'utility', event: PointerEvent) {
    if (!browser) {
      return;
    }

    const startX = event.clientX;
    const startWidth = target === 'library' ? $libraryRailWidth : $utilityWellWidth;

    const onPointerMove = (moveEvent: PointerEvent) => {
      const delta = moveEvent.clientX - startX;
      if (target === 'library') {
        setLibraryRailWidth(startWidth + delta);
      } else {
        setUtilityWellWidth(startWidth - delta);
      }
    };

    const stop = () => {
      window.removeEventListener('pointermove', onPointerMove);
      window.removeEventListener('pointerup', stop);
      resizeCleanup = null;
    };

    resizeCleanup?.();
    resizeCleanup = stop;
    window.addEventListener('pointermove', onPointerMove);
    window.addEventListener('pointerup', stop);
  }

  function hashHue(input: string): number {
    let hash = 0;
    for (let i = 0; i < input.length; i += 1) {
      hash = (hash * 31 + input.charCodeAt(i)) | 0;
    }
    return Math.abs(hash) % 360;
  }

  function hueToRgb(hue: number, sat = 0.58, light = 0.52): { r: number; g: number; b: number } {
    const c = (1 - Math.abs(2 * light - 1)) * sat;
    const x = c * (1 - Math.abs(((hue / 60) % 2) - 1));
    const m = light - c / 2;
    let r = 0;
    let g = 0;
    let b = 0;
    if (hue < 60) {
      r = c;
      g = x;
    } else if (hue < 120) {
      r = x;
      g = c;
    } else if (hue < 180) {
      g = c;
      b = x;
    } else if (hue < 240) {
      g = x;
      b = c;
    } else if (hue < 300) {
      r = x;
      b = c;
    } else {
      r = c;
      b = x;
    }
    return {
      r: Math.round((r + m) * 255),
      g: Math.round((g + m) * 255),
      b: Math.round((b + m) * 255),
    };
  }

  function luminance(r: number, g: number, b: number): number {
    const norm = [r, g, b].map((v) => {
      const c = v / 255;
      return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
    });
    return norm[0] * 0.2126 + norm[1] * 0.7152 + norm[2] * 0.0722;
  }

  async function loadDynamicGlassPrefs() {
    try {
      const enabled = await api.getSetting('ui_dynamic_glass_enabled');
      const lowMotion = await api.getSetting('ui_dynamic_glass_low_motion');
      const intensity = await api.getSetting('ui_dynamic_glass_intensity');
      dynamicGlassEnabled = enabled !== 'false';
      dynamicGlassLowMotion = lowMotion === 'true';
      dynamicGlassIntensity = clamp(Number.parseInt(intensity ?? '62', 10) || 62, 15, 90);
    } catch {
      dynamicGlassEnabled = true;
      dynamicGlassLowMotion = false;
      dynamicGlassIntensity = 62;
    }
  }

  async function sampleCoverMood(track: Track): Promise<{ r: number; g: number; b: number } | null> {
    if (!track.cover_art_path) {
      return null;
    }
    try {
      const img = new Image();
      img.src = `asset://localhost/${track.cover_art_path.replace(/\\/g, '/')}`;
      await img.decode();
      const canvas = document.createElement('canvas');
      canvas.width = 24;
      canvas.height = 24;
      const ctx = canvas.getContext('2d', { willReadFrequently: true });
      if (!ctx) {
        return null;
      }
      ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
      const data = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
      let r = 0;
      let g = 0;
      let b = 0;
      let count = 0;
      for (let i = 0; i < data.length; i += 4) {
        const alpha = data[i + 3];
        if (alpha < 40) {
          continue;
        }
        r += data[i];
        g += data[i + 1];
        b += data[i + 2];
        count += 1;
      }
      if (count === 0) {
        return null;
      }
      return {
        r: Math.round(r / count),
        g: Math.round(g / count),
        b: Math.round(b / count),
      };
    } catch {
      return null;
    }
  }

  async function refreshMoodFromTrack(track: Track | null) {
    if (!dynamicGlassEnabled || !track) {
      moodStyle = '--mood-accent-rgb: 139,180,212; --mood-layer-a: rgba(139,180,212,0.08); --mood-layer-b: rgba(247,180,92,0.06); --mood-blur: 24px; --mood-shift-ms: 460ms;';
      return;
    }

    if (track.cover_art_path && track.cover_art_path === lastMoodCover) {
      return;
    }

    const byCover = await sampleCoverMood(track);
    const byHash = hueToRgb(hashHue(`${track.artist}|${track.album}|${track.title}`));
    const picked = byCover ?? byHash;
    const lum = luminance(picked.r, picked.g, picked.b);

    const lift = lum < 0.35 ? 1.3 : 1.05;
    const r = clamp(Math.round(picked.r * lift), 0, 255);
    const g = clamp(Math.round(picked.g * lift), 0, 255);
    const b = clamp(Math.round(picked.b * lift), 0, 255);

    const fx = dynamicGlassIntensity / 100;
    const alphaA = (0.04 + fx * 0.15).toFixed(3);
    const alphaB = (0.03 + fx * 0.12).toFixed(3);
    const blur = dynamicGlassLowMotion ? 10 : Math.round(14 + fx * 20);
    const transitionMs = dynamicGlassLowMotion ? 120 : 520;

    moodStyle = `--mood-accent-rgb: ${r},${g},${b}; --mood-layer-a: rgba(${r},${g},${b},${alphaA}); --mood-layer-b: rgba(${Math.max(50, b)},${Math.max(40, r)},${Math.max(30, g)},${alphaB}); --mood-blur: ${blur}px; --mood-shift-ms: ${transitionMs}ms;`;
    lastMoodCover = track.cover_art_path ?? '';
  }

  $: void refreshMoodFromTrack($currentTrack);

  onMount(() => {
    if (isVisualizerWindowRoute) {
      return;
    }

    void loadDynamicGlassPrefs();
    startPlayerPoll();
    void startPlayerEventListener();
    loadLibrary();
    loadQueue();
    loadDownloadConfig();
    refreshBacklogStatus();
    refreshSlskdRuntimeStatus();
    startDownloadSupervision();
  });

  onDestroy(() => {
    if (isVisualizerWindowRoute) {
      return;
    }

    stopPlayerPoll();
    stopPlayerEventListener();
    stopDownloadSupervision();
    resizeCleanup?.();
  });
</script>

{#if isVisualizerWindowRoute}
  <slot />
{:else}
  <div
    class="app-shell"
    class:compact-player={$compactPlayerMode}
    class:mood-enabled={dynamicGlassEnabled}
    class:immersive-route={isImmersionRoute}
    class:utility-collapsed={$utilityWellCollapsed && !isImmersionRoute}
    class:workstation-open={$workstationDeckOpen}
    style={shellStyle}
  >
    <div class="app-backdrop" aria-hidden="true">
      <div class="backdrop-blob blob-a"></div>
      <div class="backdrop-blob blob-b"></div>
      <div class="backdrop-blob blob-c"></div>
    </div>
    <header class="app-topbar">
      <div class="topbar-brand">
        <span class="brand-wordmark">Cassette</span>
        <span class="brand-divider">//</span>
        <span class="brand-mode">Listening Room</span>
      </div>
      <div class="topbar-spacer"></div>
      <button class="topbar-link topbar-toggle" type="button" aria-label="Toggle compact player" on:click={toggleCompactPlayerMode}>
        {$compactPlayerMode ? 'Full player' : 'Compact player'}
      </button>
      {#if !isImmersionRoute}
        <button class="topbar-link topbar-toggle" type="button" aria-label="Toggle utility well" on:click={toggleUtilityWellCollapsed}>
          {$utilityWellCollapsed ? 'Open well' : 'Hide well'}
        </button>
      {/if}
      <button class="topbar-link topbar-toggle" type="button" aria-label="Toggle workstation deck" on:click={toggleWorkstationDeck}>
        {$workstationDeckOpen ? 'Close workstation' : 'Workstation'}
      </button>
      <button class="topbar-link topbar-toggle" type="button" aria-label="Minimize app" on:click={minimizeAppWindow}>
        Minimize
      </button>
      <button
        class="topbar-link topbar-preset"
        class:topbar-preset-active={$activeWorkspacePreset === 'listen_queue'}
        type="button"
        aria-label="Apply listen preset"
        on:click={() => applyWorkspacePreset('listen_queue')}
      >
        Listen
      </button>
      <button
        class="topbar-link topbar-preset"
        class:topbar-preset-active={$activeWorkspacePreset === 'acquisition'}
        type="button"
        aria-label="Apply acquisition preset"
        on:click={() => applyWorkspacePreset('acquisition')}
      >
        Acquire
      </button>
      <button class="topbar-command" type="button" aria-label="Open command palette" on:click={openPalette}>
        Commands
      </button>
    </header>

    {#if $shellActionError}
      <div class="shell-action-banner" role="status">{$shellActionError}</div>
    {/if}

    <aside class="app-sidebar">
      <Sidebar />
    </aside>

    <aside class="app-library">
      <LibraryRail />
    </aside>

    <div
      class="app-resize-handle handle-library"
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize library rail"
      on:pointerdown={(event) => beginHorizontalResize('library', event)}
    ></div>

    <main class="app-main">
      <slot />
    </main>

    {#if !isImmersionRoute && !$utilityWellCollapsed}
      <div
        class="app-resize-handle handle-utility"
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize utility well"
        on:pointerdown={(event) => beginHorizontalResize('utility', event)}
      ></div>

      <aside class="app-right">
        <RightSidebar />
      </aside>
    {:else if !isImmersionRoute}
      <button class="utility-reopen" type="button" on:click={() => openUtilityWell($utilityWellMode)}>
        {$utilityWellMode === 'queue' ? 'Queue' : $utilityWellMode === 'room' ? 'Room' : 'Context'}
      </button>
    {/if}

    <footer class="app-nowplaying">
      <SystemStatusStrip />
      <NowPlaying />
    </footer>

    <CommandPalette />
    <WorkstationDeck />
  </div>
{/if}

<style>
  .app-backdrop {
    position: fixed;
    inset: 0;
    z-index: 0;
    pointer-events: none;
    overflow: hidden;
  }

  .backdrop-blob {
    position: absolute;
    border-radius: 50%;
    opacity: 0.12;
    /* Keep blur local to each blob so perf tuning stays explicit. */
  }

  .blob-a {
    width: 560px;
    height: 560px;
    top: -200px;
    left: -100px;
    background: radial-gradient(circle, rgba(var(--mood-accent-rgb), 1), transparent 70%);
    filter: blur(48px);
    will-change: transform;
    animation: blob-drift-a 22s ease-in-out infinite alternate;
    transition: background var(--mood-shift-ms) ease;
  }

  .blob-b {
    width: 460px;
    height: 460px;
    bottom: -140px;
    right: -100px;
    background: radial-gradient(circle, rgba(247, 180, 92, 1), transparent 70%);
    filter: blur(48px);
    will-change: transform;
    animation: blob-drift-b 17s ease-in-out infinite alternate;
    animation-delay: -8s;
  }

  .blob-c {
    width: 360px;
    height: 360px;
    top: 35%;
    left: 38%;
    background: radial-gradient(circle, rgba(139, 180, 212, 1), transparent 70%);
    filter: blur(36px);
    animation: blob-drift-c 26s ease-in-out infinite alternate;
    animation-delay: -14s;
  }

  :global(html.low-motion) .backdrop-blob { animation: none; }

  .app-sidebar,
  .app-library,
  .app-main,
  .app-right,
  .app-nowplaying,
  .app-topbar,
  .shell-action-banner {
    position: relative;
    z-index: 1;
  }

  .shell-action-banner {
    margin: 0 14px;
    padding: 8px 12px;
    border: 1px solid rgba(255, 143, 143, 0.25);
    border-radius: var(--radius-sm);
    background: rgba(120, 24, 24, 0.18);
    color: var(--status-error, #ffb4b4);
    font-size: 0.76rem;
  }

  .topbar-preset.topbar-preset-active {
    border-color: rgba(var(--mood-accent-rgb), 0.36);
    color: var(--text-accent);
    background: rgba(var(--mood-accent-rgb), 0.08);
  }

  .app-resize-handle {
    position: relative;
    z-index: 2;
    cursor: col-resize;
  }

  .app-resize-handle::before {
    content: '';
    position: absolute;
    inset: 0 auto 0 50%;
    width: 1px;
    transform: translateX(-50%);
    background: rgba(var(--mood-accent-rgb), 0.14);
  }

  .app-resize-handle:hover::before {
    background: rgba(var(--mood-accent-rgb), 0.36);
  }

  .utility-reopen {
    position: relative;
    z-index: 2;
    writing-mode: vertical-rl;
    transform: rotate(180deg);
    padding: 16px 6px;
    border-left: 1px solid var(--border-dim);
    background: rgba(7, 10, 16, 0.95);
    color: var(--text-muted);
    font-size: 0.66rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    font-weight: 700;
  }

  .utility-reopen:hover {
    color: var(--text-primary);
  }
</style>
