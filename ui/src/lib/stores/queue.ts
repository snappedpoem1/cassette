import { get, writable } from 'svelte/store';
import { api, type QueueItem, type Track } from '$lib/api/tauri';
import { playbackState } from '$lib/stores/player';
import type { QueueRitualResult } from '$lib/queue-ritual';
import { cutQueueAfter, holdQueueItem, pinQueueItem, playAfterCurrent } from '$lib/queue-ritual';
import { liveQueueRitual, resetLiveQueueRitual, updateLiveQueueRitual } from '$lib/stores/rituals';

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
  await resetLiveQueueRitual();
  await loadQueue();
}

export async function addToQueue(track: Track) {
  await api.addToQueue(track.id);
  await loadQueue();
}

export async function clearQueue() {
  await api.clearQueue();
  await resetLiveQueueRitual();
  queue.set([]);
}

export async function removeQueueItem(position: number, startIndex = 0) {
  const items = get(queue);
  const trackIds = items.map((item) => item.track_id);
  await api.removeQueueItem(position, trackIds, startIndex);
  await loadQueue();
}

export async function replaceQueueTrackIds(trackIds: number[], startIndex = 0) {
  if (trackIds.length === 0) {
    await clearQueue();
    return;
  }
  await api.queueTracks(trackIds, startIndex);
  await resetLiveQueueRitual();
  await loadQueue();
}

async function applyQueueRitualResult(result: QueueRitualResult) {
  if (result.trackIds.length === 0) {
    await clearQueue();
    return;
  }

  await api.queueTracks(result.trackIds, result.startIndex);
  await updateLiveQueueRitual({
    pinnedPositions: result.pinnedPositions,
    heldPositions: result.heldPositions,
  });
  await loadQueue();
}

export async function playItemAfterCurrent(position: number) {
  const items = get(queue);
  const currentPosition = get(playbackState).queue_position;
  const ritual = get(liveQueueRitual);
  await applyQueueRitualResult(playAfterCurrent(items, currentPosition, position, ritual));
}

export async function pinQueuePosition(position: number) {
  const items = get(queue);
  const currentPosition = get(playbackState).queue_position;
  const ritual = get(liveQueueRitual);
  await applyQueueRitualResult(pinQueueItem(items, currentPosition, position, ritual));
}

export async function holdQueuePosition(position: number) {
  const items = get(queue);
  const currentPosition = get(playbackState).queue_position;
  const ritual = get(liveQueueRitual);
  await applyQueueRitualResult(holdQueueItem(items, currentPosition, position, ritual));
}

export async function cutQueueAfterPosition(position: number) {
  const items = get(queue);
  const currentPosition = get(playbackState).queue_position;
  const ritual = get(liveQueueRitual);
  await applyQueueRitualResult(cutQueueAfter(items, currentPosition, position, ritual));
}
