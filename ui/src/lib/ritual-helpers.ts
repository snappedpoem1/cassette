import type { PlaylistItem, Track } from '$lib/api/tauri';
import type { CrateFilter, PlaylistSection } from '$lib/stores/rituals';
import { clampSection } from '$lib/queue-ritual';

export function buildTrackMap(tracks: Track[]): Map<number, Track> {
  return new Map(tracks.map((track) => [track.id, track]));
}

export function hydrateTrackIds(trackIds: number[], trackMap: Map<number, Track>): Track[] {
  return trackIds
    .map((trackId) => trackMap.get(trackId))
    .filter((track): track is Track => Boolean(track));
}

export function filterTracksForCrate(
  tracks: Track[],
  filter: CrateFilter,
  playlistTrackIds: Set<number> | null = null
): Track[] {
  const artistNeedle = filter.artistQuery.trim().toLowerCase();
  const albumNeedle = filter.albumQuery.trim().toLowerCase();
  const formatNeedle = filter.format.trim().toLowerCase();
  const qualityNeedle = filter.qualityTier.trim().toLowerCase();

  return tracks.filter((track) => {
    if (artistNeedle && !track.artist.toLowerCase().includes(artistNeedle)) {
      return false;
    }
    if (albumNeedle && !track.album.toLowerCase().includes(albumNeedle)) {
      return false;
    }
    if (formatNeedle && track.format.toLowerCase() !== formatNeedle) {
      return false;
    }
    if (qualityNeedle && (track.quality_tier ?? '').toLowerCase() !== qualityNeedle) {
      return false;
    }
    if (filter.yearFrom !== null && (track.year ?? 0) < filter.yearFrom) {
      return false;
    }
    if (filter.yearTo !== null && (track.year ?? 9999) > filter.yearTo) {
      return false;
    }
    if (playlistTrackIds && !playlistTrackIds.has(track.id)) {
      return false;
    }
    return true;
  });
}

export function sectionSlices(
  sections: PlaylistSection[],
  items: PlaylistItem[]
): Array<{ section: PlaylistSection; items: PlaylistItem[] }> {
  if (items.length === 0) {
    return [];
  }

  return sections
    .map((section) => {
      const bounded = clampSection(section, items.length - 1);
      return {
        section: bounded,
        items: items.slice(bounded.startIndex, bounded.endIndex + 1),
      };
    })
    .filter((slice) => slice.items.length > 0);
}

export function totalDurationSecs(tracks: Track[]): number {
  return tracks.reduce((sum, track) => sum + (track.duration_secs ?? 0), 0);
}
