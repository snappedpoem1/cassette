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
