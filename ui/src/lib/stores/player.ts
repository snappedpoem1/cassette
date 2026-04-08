import { writable, derived, get } from 'svelte/store';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
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
let unlistenPlayerEvent: UnlistenFn | null = null;
let lastTrackId: number | null = null;
let lastTaskbarSignature: string | null = null;
const scrobbledTrackIds = new Set<number>();
const failedScrobbleTrackIds = new Set<number>();

async function syncStateSideEffects(state: PlaybackState): Promise<void> {
  void syncTaskbarPlaybackProgress(state);
  void maybeScrobbleToLastfm(state);

  const track = state.current_track;
  if (track && track.id !== lastTrackId) {
    lastTrackId = track.id;
    failedScrobbleTrackIds.delete(track.id);
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
}

function lastfmScrobbleThresholdSecs(durationSecs: number): number {
  const halfTrack = durationSecs > 0 ? durationSecs * 0.5 : 120;
  return Math.max(30, Math.min(240, halfTrack));
}

async function maybeScrobbleToLastfm(state: PlaybackState): Promise<void> {
  const track = state.current_track;
  if (!track || !state.is_playing || state.position_secs <= 0 || state.duration_secs <= 0) {
    return;
  }
  if (scrobbledTrackIds.has(track.id) || failedScrobbleTrackIds.has(track.id)) {
    return;
  }

  const threshold = lastfmScrobbleThresholdSecs(state.duration_secs);
  if (state.position_secs < threshold) {
    return;
  }

  try {
    const scrobbled = await api.submitLastfmScrobble(
      track.id,
      track.artist,
      track.title,
      track.album || undefined,
      state.duration_secs,
      state.position_secs,
    );
    if (scrobbled) {
      scrobbledTrackIds.add(track.id);
    } else {
      failedScrobbleTrackIds.add(track.id);
    }
  } catch {
    failedScrobbleTrackIds.add(track.id);
  }
}

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
      void syncStateSideEffects(state);
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

export async function startPlayerEventListener() {
  if (!browser || unlistenPlayerEvent) {
    return;
  }

  unlistenPlayerEvent = await listen<PlaybackState>('playback_state_changed', (event) => {
    const incoming = event.payload;
    const seeking = get(isSeeking);

    playbackState.update((current) =>
      seeking ? { ...incoming, position_secs: current.position_secs } : incoming
    );

    void syncStateSideEffects(incoming);
  });
}

export function stopPlayerEventListener() {
  if (unlistenPlayerEvent) {
    unlistenPlayerEvent();
    unlistenPlayerEvent = null;
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
