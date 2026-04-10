import { writable } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import {
  api,
  DESKTOP_RUNTIME_REQUIRED_MESSAGE,
  toDesktopRuntimeMessage,
  type Track,
  type Album,
  type Artist,
  type LibraryRoot,
  type ScanProgress,
} from '$lib/api/tauri';

// ── State ─────────────────────────────────────────────────────────────────────

export const tracks = writable<Track[]>([]);
export const albums = writable<Album[]>([]);
export const artists = writable<Artist[]>([]);
export const libraryRoots = writable<LibraryRoot[]>([]);
export const trackCount = writable(0);
export const libraryLoadError = writable<string | null>(null);

export const searchQuery = writable('');
export const searchResults = writable<Track[]>([]);
export const isSearching = writable(false);

export const scanProgress = writable<ScanProgress | null>(null);
export const isScanning = writable(false);

export type LibraryTab = 'albums' | 'tracks' | 'artists';
export const activeTab = writable<LibraryTab>('albums');

function tauriRuntimeAvailable(): boolean {
  return (
    typeof window !== 'undefined' &&
    typeof (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !== 'undefined'
  );
}

// ── Actions ───────────────────────────────────────────────────────────────────

export async function loadLibrary() {
  if (!tauriRuntimeAvailable()) {
    libraryLoadError.set(DESKTOP_RUNTIME_REQUIRED_MESSAGE);
    return;
  }

  try {
    const [t, a, ar, roots, count] = await Promise.all([
      api.getTracks(50000, 0),
      api.getAlbums(),
      api.getArtists(),
      api.getLibraryRoots(),
      api.getTrackCount(),
    ]);
    tracks.set(t);
    albums.set(a);
    artists.set(ar);
    libraryRoots.set(roots);
    trackCount.set(count);
    libraryLoadError.set(null);
  } catch (e) {
    console.error('loadLibrary failed', e);
    libraryLoadError.set(toDesktopRuntimeMessage(e, 'Failed to load the library.'));
  }
}

export async function search(query: string) {
  searchQuery.set(query);
  if (!query.trim()) {
    searchResults.set([]);
    isSearching.set(false);
    return;
  }
  isSearching.set(true);
  try {
    const results = await api.searchTracks(query);
    searchResults.set(results);
  } catch {
    searchResults.set([]);
  } finally {
    isSearching.set(false);
  }
}

export async function scanLibrary() {
  if (!tauriRuntimeAvailable()) {
    isScanning.set(false);
    scanProgress.set(null);
    return;
  }

  isScanning.set(true);
  scanProgress.set(null);

  const unlisten = await listen<ScanProgress>('scan-progress', (event) => {
    scanProgress.set(event.payload);
    if (event.payload.done) {
      isScanning.set(false);
      loadLibrary();
    }
  });

  try {
    await api.scanLibrary();
  } catch (e) {
    console.error('Scan failed', e);
    isScanning.set(false);
  } finally {
    unlisten();
  }
}

export async function addLibraryRoot(path: string) {
  if (!tauriRuntimeAvailable()) {
    return;
  }
  await api.addLibraryRoot(path);
  libraryRoots.set(await api.getLibraryRoots());
}

export async function removeLibraryRoot(path: string) {
  if (!tauriRuntimeAvailable()) {
    return;
  }
  await api.removeLibraryRoot(path);
  libraryRoots.set(await api.getLibraryRoots());
}
