<script lang="ts">
  import '../app.css';
  import { onMount, onDestroy } from 'svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import NowPlaying from '$lib/components/NowPlaying.svelte';
  import RightSidebar from '$lib/components/RightSidebar.svelte';
  import CommandPalette from '$lib/components/CommandPalette.svelte';
  import SystemStatusStrip from '$lib/components/SystemStatusStrip.svelte';
  import { api, type Track } from '$lib/api/tauri';
  import { startPlayerPoll, stopPlayerPoll } from '$lib/stores/player';
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
  import { compactPlayerMode, toggleCompactPlayerMode, minimizeAppWindow } from '$lib/stores/shell';
  import { loadQueue } from '$lib/stores/queue';

  let dynamicGlassEnabled = true;
  let dynamicGlassLowMotion = false;
  let dynamicGlassIntensity = 62;
  let moodStyle = '--mood-accent-rgb: 139,180,212; --mood-layer-a: rgba(139,180,212,0.08); --mood-layer-b: rgba(247,180,92,0.06); --mood-blur: 24px; --mood-shift-ms: 460ms;';
  let lastMoodCover = '';

  function clamp(value: number, min: number, max: number): number {
    return Math.max(min, Math.min(max, value));
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
    void loadDynamicGlassPrefs();
    startPlayerPoll();
    loadLibrary();
    loadQueue();
    loadDownloadConfig();
    refreshBacklogStatus();
    refreshSlskdRuntimeStatus();
    startDownloadSupervision();
  });

  onDestroy(() => {
    stopPlayerPoll();
    stopDownloadSupervision();
  });
</script>

<div class="app-shell" class:compact-player={$compactPlayerMode} class:mood-enabled={dynamicGlassEnabled} style={moodStyle}>
  <header class="app-topbar">
    <div class="topbar-brand">
      <span class="brand-wordmark">Cassette</span>
      <span class="brand-divider">//</span>
      <span class="brand-mode">Desktop</span>
    </div>
    <div class="topbar-spacer"></div>
    <button class="topbar-link topbar-toggle" type="button" aria-label="Toggle compact player" on:click={toggleCompactPlayerMode}>
      {$compactPlayerMode ? 'Full Player' : 'Mini Player'}
    </button>
    <button class="topbar-link topbar-toggle" type="button" aria-label="Minimize app" on:click={minimizeAppWindow}>
      Minimize
    </button>
    <button class="topbar-command" type="button" aria-label="Open command palette" on:click={openPalette}>
      Commands
    </button>
  </header>

  <aside class="app-sidebar">
    <Sidebar />
  </aside>

  <main class="app-main">
    <slot />
  </main>

  <aside class="app-right">
    <RightSidebar />
  </aside>

  <footer class="app-nowplaying">
    <SystemStatusStrip />
    <NowPlaying />
  </footer>

  <CommandPalette />
</div>
