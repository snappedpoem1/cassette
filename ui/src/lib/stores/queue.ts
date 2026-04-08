import { get, writable } from 'svelte/store';
import { api, type QueueItem, type Track } from '$lib/api/tauri';

export const queue = writable<QueueItem[]>([]);

export async function loadQueue() {
  try {
    queue.set(await api.getQueue());
  } catch {
    queue.set([]);
  }
}

export async function queueTracks(tracks: Track[], startIndex = 0) {
  const ids = tracks.map((t) => t.id);
  await api.queueTracks(ids, startIndex);
  await loadQueue();
}

export async function addToQueue(track: Track) {
  await api.addToQueue(track.id);
  await loadQueue();
}

export async function clearQueue() {
  await api.clearQueue();
  queue.set([]);
}

export async function removeQueueItem(position: number, startIndex = 0) {
  const items = get(queue);
  const trackIds = items.map((item) => item.track_id);
  await api.removeQueueItem(position, trackIds, startIndex);
  await loadQueue();
}
