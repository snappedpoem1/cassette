import { writable } from 'svelte/store';
import { api, type Playlist, type PlaylistItem } from '$lib/api/tauri';

export const playlists = writable<Playlist[]>([]);
export const activePlaylistId = writable<number | null>(null);
export const activePlaylistItems = writable<PlaylistItem[]>([]);

export async function loadPlaylists() {
  try {
    playlists.set(await api.getPlaylists());
  } catch {
    playlists.set([]);
  }
}

export async function loadPlaylistItems(id: number) {
  activePlaylistId.set(id);
  try {
    activePlaylistItems.set(await api.getPlaylistItems(id));
  } catch {
    activePlaylistItems.set([]);
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
}
