import type { QueueItem, Track } from '$lib/api/tauri';
import type { LiveQueueRitual, PlaylistSection } from '$lib/stores/rituals';

interface QueueEntry {
  trackId: number;
  pinned: boolean;
  held: boolean;
}

export interface QueueRitualResult {
  trackIds: number[];
  startIndex: number;
  pinnedPositions: number[];
  heldPositions: number[];
}

function clampIndex(value: number, max: number): number {
  return Math.max(0, Math.min(value, max));
}

function moveEntry(entries: QueueEntry[], fromIndex: number, toIndex: number): QueueEntry[] {
  const next = [...entries];
  const [moved] = next.splice(fromIndex, 1);
  next.splice(clampIndex(toIndex, next.length), 0, moved);
  return next;
}

function snapshot(entries: QueueEntry[], startIndex: number): QueueRitualResult {
  return {
    trackIds: entries.map((entry) => entry.trackId),
    startIndex: clampIndex(startIndex, Math.max(0, entries.length - 1)),
    pinnedPositions: entries.flatMap((entry, index) => (entry.pinned ? [index] : [])),
    heldPositions: entries.flatMap((entry, index) => (entry.held ? [index] : [])),
  };
}

export function buildQueueEntries(items: QueueItem[], ritual: LiveQueueRitual): QueueEntry[] {
  return items.map((item, index) => ({
    trackId: item.track_id,
    pinned: ritual.pinnedPositions.includes(index),
    held: ritual.heldPositions.includes(index),
  }));
}

export function playAfterCurrent(
  items: QueueItem[],
  currentIndex: number,
  targetIndex: number,
  ritual: LiveQueueRitual
): QueueRitualResult {
  const entries = buildQueueEntries(items, ritual);
  if (entries.length === 0) {
    return snapshot(entries, 0);
  }
  const next = moveEntry(entries, targetIndex, currentIndex + 1);
  return snapshot(next, currentIndex);
}

export function pinQueueItem(
  items: QueueItem[],
  currentIndex: number,
  targetIndex: number,
  ritual: LiveQueueRitual
): QueueRitualResult {
  const entries = buildQueueEntries(items, ritual);
  const moving = entries[targetIndex];
  if (!moving) {
    return snapshot(entries, currentIndex);
  }

  moving.pinned = true;
  moving.held = false;
  let lastPinnedAfterCurrent = -1;
  for (let index = currentIndex + 1; index < entries.length; index += 1) {
    if (entries[index]?.pinned) {
      lastPinnedAfterCurrent = index;
    }
  }
  const pinDestination = Math.max(currentIndex + 1, lastPinnedAfterCurrent + 1);
  const next = moveEntry(entries, targetIndex, pinDestination);
  return snapshot(next, currentIndex);
}

export function holdQueueItem(
  items: QueueItem[],
  currentIndex: number,
  targetIndex: number,
  ritual: LiveQueueRitual
): QueueRitualResult {
  const entries = buildQueueEntries(items, ritual);
  const moving = entries[targetIndex];
  if (!moving) {
    return snapshot(entries, currentIndex);
  }

  moving.held = true;
  moving.pinned = false;
  const next = moveEntry(entries, targetIndex, entries.length - 1);
  return snapshot(next, currentIndex);
}

export function cutQueueAfter(
  items: QueueItem[],
  currentIndex: number,
  keepThroughIndex: number,
  ritual: LiveQueueRitual
): QueueRitualResult {
  const entries = buildQueueEntries(items, ritual).slice(0, keepThroughIndex + 1);
  return snapshot(entries, Math.min(currentIndex, keepThroughIndex));
}

export function queueDuration(items: Array<QueueItem | { track: Track | null }>): number {
  return items.reduce((sum, item) => sum + (item.track?.duration_secs ?? 0), 0);
}

export function projectQueueTrackIds(items: QueueItem[]): number[] {
  return items.map((item) => item.track_id);
}

export function clampSection(section: PlaylistSection, maxIndex: number): PlaylistSection {
  const startIndex = clampIndex(section.startIndex, maxIndex);
  const endIndex = clampIndex(section.endIndex, maxIndex);
  return {
    ...section,
    startIndex: Math.min(startIndex, endIndex),
    endIndex: Math.max(startIndex, endIndex),
  };
}
