import { writable } from 'svelte/store';
import { api, toDesktopRuntimeMessage, type Playlist, type PlaylistItem } from '$lib/api/tauri';
import { loadQueue } from '$lib/stores/queue';

export const playlists = writable<Playlist[]>([]);
export const activePlaylistId = writable<number | null>(null);
export const activePlaylistItems = writable<PlaylistItem[]>([]);
export const playlistsLoadError = writable<string | null>(null);

export async function loadPlaylists() {
  try {
    playlists.set(await api.getPlaylists());
    playlistsLoadError.set(null);
  } catch (error) {
    playlistsLoadError.set(toDesktopRuntimeMessage(error, 'Failed to load playlists.'));
  }
}

export async function loadPlaylistItems(id: number) {
  activePlaylistId.set(id);
  try {
    activePlaylistItems.set(await api.getPlaylistItems(id));
    playlistsLoadError.set(null);
  } catch (error) {
    playlistsLoadError.set(toDesktopRuntimeMessage(error, 'Failed to load playlist items.'));
  }
}

export async function createPlaylist(
  name: string,
  description: string | null = null,
  trackIds: number[] = []
) {
  const id = await api.createPlaylist(name, description, trackIds);
  await loadPlaylists();
  return id;
}

export async function deletePlaylist(id: number) {
  await api.deletePlaylist(id);
  await loadPlaylists();
}

export async function playPlaylist(id: number, startIndex = 0) {
  await api.playPlaylist(id, startIndex);
  await loadQueue();
}
