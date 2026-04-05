import { writable, derived, get } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import { api, type PlaybackState, type NowPlayingContext } from '$lib/api/tauri';
import { browser } from '$app/environment';
import { getCurrentWindow, ProgressBarStatus } from '@tauri-apps/api/window';

// ── State ─────────────────────────────────────────────────────────────────────

const defaultState: PlaybackState = {
  current_track: null,
  queue_position: 0,
  position_secs: 0,
  duration_secs: 0,
  is_playing: false,
  volume: 1.0,
};

export const playbackState = writable<PlaybackState>(defaultState);
export const nowPlayingContext = writable<NowPlayingContext | null>(null);
export const isSeeking = writable(false);
export const seekPreview = writable(0);

// ── Derived ───────────────────────────────────────────────────────────────────

export const currentTrack = derived(playbackState, ($s) => $s.current_track);
export const isPlaying = derived(playbackState, ($s) => $s.is_playing);
export const volume = derived(playbackState, ($s) => $s.volume);

export const progressPct = derived(
  [playbackState, isSeeking, seekPreview],
  ([$s, $seeking, $preview]) => {
    if ($seeking) return $preview;
    if ($s.duration_secs <= 0) return 0;
    return Math.min(1, $s.position_secs / $s.duration_secs);
  }
);

// ── Poll loop ─────────────────────────────────────────────────────────────────

let pollTimer: ReturnType<typeof setInterval> | null = null;
let lastTrackId: number | null = null;
let lastTaskbarSignature: string | null = null;

async function syncTaskbarPlaybackProgress(state: PlaybackState): Promise<void> {
  if (!browser) {
    return;
  }

  const progress =
    state.duration_secs > 0
      ? Math.max(0, Math.min(100, Math.round((state.position_secs / state.duration_secs) * 100)))
      : 0;

  const signature = `${state.current_track?.id ?? 'none'}:${state.is_playing}:${progress}`;
  if (signature === lastTaskbarSignature) {
    return;
  }
  lastTaskbarSignature = signature;

  try {
    const window = getCurrentWindow();
    if (!state.current_track) {
      await window.setProgressBar({ status: ProgressBarStatus.None });
      return;
    }

    await window.setProgressBar({
      status: state.is_playing ? ProgressBarStatus.Normal : ProgressBarStatus.Paused,
      progress,
    });
  } catch {
    // noop when taskbar integration is unavailable
  }
}

export function startPlayerPoll() {
  if (pollTimer) return;
  pollTimer = setInterval(async () => {
    try {
      const state = await api.getPlaybackState();
      playbackState.set(state);
      void syncTaskbarPlaybackProgress(state);

      const track = state.current_track;
      if (track && track.id !== lastTrackId) {
        lastTrackId = track.id;
        try {
          const ctx = await api.getNowPlayingContext(
            track.artist,
            track.title,
            track.album || undefined
          );
          nowPlayingContext.set(ctx);
        } catch {
          nowPlayingContext.set(null);
        }
      } else if (!track) {
        lastTrackId = null;
        nowPlayingContext.set(null);
      }
    } catch {
      // backend not ready
    }
  }, 500);
}

export function stopPlayerPoll() {
  if (pollTimer) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}

// ── Actions ───────────────────────────────────────────────────────────────────

export const player = {
  toggle: () => api.playerToggle(),
  play: () => api.playerPlay(),
  pause: () => api.playerPause(),
  stop: () => api.playerStop(),
  next: () => api.playerNext(),
  prev: () => api.playerPrev(),

  seek: async (pct: number) => {
    const state = get(playbackState);
    const secs = pct * state.duration_secs;
    await api.playerSeek(secs);
    playbackState.update((s) => ({ ...s, position_secs: secs }));
  },

  setVolume: async (v: number) => {
    await api.playerSetVolume(v);
    playbackState.update((s) => ({ ...s, volume: v }));
  },
};
