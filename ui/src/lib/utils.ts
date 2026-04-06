import type { Track } from '$lib/api/tauri';

/** Format seconds into M:SS or H:MM:SS */
export function formatDuration(secs: number): string {
  if (!isFinite(secs) || secs < 0) return '0:00';
  const s = Math.floor(secs);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}`;
  return `${m}:${String(sec).padStart(2, '0')}`;
}

/** Format bytes into human-readable size */
export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

/** Format an ISO date string into a readable label */
export function formatDate(iso: string): string {
  try {
    return new Date(iso).toLocaleDateString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    });
  } catch {
    return iso;
  }
}

/** Clamp a value between min and max */
export function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

/** Simple debounce */
export function debounce<T extends (...args: never[]) => unknown>(fn: T, ms: number): T {
  let timer: ReturnType<typeof setTimeout>;
  return ((...args: Parameters<T>) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), ms);
  }) as T;
}

/** Build a cover art src (tauri asset protocol) */
export function coverSrc(path: string | null | undefined): string | null {
  if (!path) return null;
  const normalized = path.replace(/\\/g, '/');
  return `asset://localhost/${normalized}`;
}

/** Format audio spec label e.g. "FLAC · 24bit · 96kHz" */
export function formatAudioSpec(track: Track): string {
  const parts: string[] = [track.format.toUpperCase()];
  if (track.bit_depth) parts.push(`${track.bit_depth}bit`);
  if (track.sample_rate) {
    const khz = track.sample_rate / 1000;
    parts.push(`${khz % 1 === 0 ? khz : khz.toFixed(1)}kHz`);
  } else if (track.bitrate_kbps) {
    parts.push(`${track.bitrate_kbps}kbps`);
  }
  return parts.join(' · ');
}

/** Initials from a string (for avatar placeholders) */
export function initials(name: string): string {
  return name
    .split(/\s+/)
    .slice(0, 2)
    .map((w) => w[0]?.toUpperCase() ?? '')
    .join('');
}

/**
 * Given a dominant color hex (e.g. "#3d2810"), return CSS color strings
 * for a darkened/desaturated card background and a lightened title color.
 * Falls back to token values if hex is null/invalid.
 */
export function tintFromHex(hex: string | null | undefined): { bg: string; titleColor: string } {
  const fallback = { bg: 'var(--bg-card)', titleColor: 'var(--text-primary)' };
  if (!hex) return fallback;

  const clean = hex.replace('#', '');
  if (clean.length !== 6) return fallback;

  const r = parseInt(clean.slice(0, 2), 16);
  const g = parseInt(clean.slice(2, 4), 16);
  const b = parseInt(clean.slice(4, 6), 16);
  if (isNaN(r) || isNaN(g) || isNaN(b)) return fallback;

  // Dark background: crush luminance, keep a hint of hue
  const bgR = Math.round(r * 0.08 + 4);
  const bgG = Math.round(g * 0.08 + 4);
  const bgB = Math.round(b * 0.08 + 4);

  // Title color: lift luminance, partially desaturate toward slate
  const titleR = Math.round(r * 0.35 + 140);
  const titleG = Math.round(g * 0.35 + 150);
  const titleB = Math.round(b * 0.35 + 160);

  const clampVal = (v: number) => Math.max(0, Math.min(255, v));

  return {
    bg: `rgb(${clampVal(bgR)}, ${clampVal(bgG)}, ${clampVal(bgB)})`,
    titleColor: `rgb(${clampVal(titleR)}, ${clampVal(titleG)}, ${clampVal(titleB)})`,
  };
}
