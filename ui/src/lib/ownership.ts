import type { AcquisitionRequestListItem, Album, Track, TrackIdentityContext } from '$lib/api/tauri';

export interface AlbumEditionContext {
  bucket: string | null;
  markers: string[];
}

export interface TrackSlot {
  key: string;
  discNumber: number;
  trackNumber: number;
  title: string;
  tracks: Track[];
  bestTrack: Track;
}

export interface AlbumOwnershipSummary {
  album: Album;
  tracks: Track[];
  slots: TrackSlot[];
  bestTrackCount: number;
  duplicateSlotCount: number;
  missingMetadataCount: number;
  lossyTrackCount: number;
  losslessTrackCount: number;
  hiResTrackCount: number;
  sourceProviders: string[];
  edition: AlbumEditionContext;
  archiveHealth: 'strong' | 'steady' | 'fragile';
  archiveNotes: string[];
  qualityLabel: string;
}

const EDITION_MARKERS = [
  'deluxe',
  'expanded',
  'remaster',
  'live',
  'anniversary',
  'special edition',
  'limited edition',
  'tour edition',
  'collector',
  'bonus',
];

export function qualityRank(track: Track): number {
  const tier = (track.quality_tier ?? '').toLowerCase();
  if (tier === 'lossless_hires') return 5;
  if (tier === 'lossless') return 4;
  if (tier === 'lossy_high') return 3;
  if (tier === 'lossy_mid') return 2;
  if (tier === 'lossy_low') return 1;

  const format = (track.format ?? '').toLowerCase();
  if (['flac', 'wav', 'aiff', 'alac'].includes(format)) {
    return track.sample_rate && track.sample_rate >= 88200 ? 5 : 4;
  }
  if (track.bitrate_kbps && track.bitrate_kbps >= 320) return 3;
  if (track.bitrate_kbps && track.bitrate_kbps >= 192) return 2;
  return 1;
}

export function qualityLabel(tracks: Track[]): string {
  const best = Math.max(0, ...tracks.map((track) => qualityRank(track)));
  if (best >= 5) return 'Hi-Res in hand';
  if (best >= 4) return 'Lossless in hand';
  if (best >= 3) return 'Strong lossy copy';
  if (best >= 2) return 'Standard copy';
  return 'Needs a better copy';
}

export function detectEditionMarkers(title: string): string[] {
  const normalized = title.toLowerCase();
  return EDITION_MARKERS.filter((marker) => normalized.includes(marker));
}

export function deriveEditionContext(
  albumTitle: string,
  identities: Array<TrackIdentityContext | null | undefined>,
): AlbumEditionContext {
  const markers = new Set<string>(detectEditionMarkers(albumTitle));
  const buckets = new Map<string, number>();

  for (const identity of identities) {
    if (!identity) continue;
    for (const marker of identity.edition_markers ?? []) {
      markers.add(marker.replace(/_/g, ' '));
    }
    if (identity.edition_bucket) {
      buckets.set(identity.edition_bucket, (buckets.get(identity.edition_bucket) ?? 0) + 1);
    }
  }

  const bucket = [...buckets.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))[0]?.[0] ?? null;

  return {
    bucket,
    markers: [...markers].sort((a, b) => a.localeCompare(b)),
  };
}

export function buildTrackSlots(tracks: Track[]): TrackSlot[] {
  const slots = new Map<string, Track[]>();

  for (const track of tracks) {
    const discNumber = track.disc_number ?? 1;
    const trackNumber = track.track_number ?? 0;
    const key = `${discNumber}:${trackNumber}:${track.title.toLowerCase()}`;
    const existing = slots.get(key) ?? [];
    existing.push(track);
    slots.set(key, existing);
  }

  return [...slots.entries()]
    .map(([key, grouped]) => {
      const bestTrack = [...grouped].sort(compareTracksForBestCopy)[0];
      return {
        key,
        discNumber: bestTrack.disc_number ?? 1,
        trackNumber: bestTrack.track_number ?? 0,
        title: bestTrack.title,
        tracks: grouped.sort(compareTracksForBestCopy),
        bestTrack,
      };
    })
    .sort((a, b) => {
      return a.discNumber - b.discNumber ||
        a.trackNumber - b.trackNumber ||
        a.title.localeCompare(b.title);
    });
}

export function compareTracksForBestCopy(left: Track, right: Track): number {
  return qualityRank(right) - qualityRank(left) ||
    (right.bit_depth ?? 0) - (left.bit_depth ?? 0) ||
    (right.sample_rate ?? 0) - (left.sample_rate ?? 0) ||
    (right.bitrate_kbps ?? 0) - (left.bitrate_kbps ?? 0) ||
    Number(right.file_size) - Number(left.file_size) ||
    left.path.localeCompare(right.path);
}

export function buildAlbumOwnershipSummary(
  album: Album,
  tracks: Track[],
  identities: Array<TrackIdentityContext | null | undefined>,
  requests: AcquisitionRequestListItem[],
): AlbumOwnershipSummary {
  const slots = buildTrackSlots(tracks);
  const missingMetadataCount = tracks.filter((track) =>
    !track.year || !track.isrc || !track.musicbrainz_release_id || !track.quality_tier
  ).length;
  const lossyTrackCount = tracks.filter((track) => qualityRank(track) <= 3).length;
  const losslessTrackCount = tracks.filter((track) => qualityRank(track) >= 4).length;
  const hiResTrackCount = tracks.filter((track) => qualityRank(track) >= 5).length;
  const duplicateSlotCount = slots.filter((slot) => slot.tracks.length > 1).length;
  const matchedRequests = requests.filter((request) =>
    tracks.some((track) => request.final_path && samePath(request.final_path, track.path))
  );
  const sourceProviders = [...new Set(
    matchedRequests
      .map((request) => request.selected_provider)
      .filter((provider): provider is string => Boolean(provider))
  )].sort((a, b) => a.localeCompare(b));
  const edition = deriveEditionContext(album.title, identities);
  const archiveNotes: string[] = [];

  if (duplicateSlotCount > 0) {
    archiveNotes.push(`${duplicateSlotCount} track slot${duplicateSlotCount === 1 ? '' : 's'} have multiple copies`);
  }
  if (missingMetadataCount > 0) {
    archiveNotes.push(`${missingMetadataCount} track${missingMetadataCount === 1 ? '' : 's'} still have thin metadata`);
  }
  if (!album.cover_art_path) {
    archiveNotes.push('cover art is missing');
  }
  if (edition.markers.length > 0) {
    archiveNotes.push(`edition cues: ${edition.markers.slice(0, 3).join(', ')}`);
  }
  if (sourceProviders.length > 0) {
    archiveNotes.push(`provenance recorded from ${sourceProviders.join(', ')}`);
  }

  const archiveHealth = duplicateSlotCount > 1 || missingMetadataCount > Math.max(2, tracks.length / 2)
    ? 'fragile'
    : archiveNotes.length > 2 || lossyTrackCount > 0
      ? 'steady'
      : 'strong';

  return {
    album,
    tracks,
    slots,
    bestTrackCount: slots.length,
    duplicateSlotCount,
    missingMetadataCount,
    lossyTrackCount,
    losslessTrackCount,
    hiResTrackCount,
    sourceProviders,
    edition,
    archiveHealth,
    archiveNotes,
    qualityLabel: qualityLabel(tracks),
  };
}

export function relatedVersionsForArtist(
  albums: Album[],
  artistName: string,
  anchorTitle?: string,
  excludeId?: number,
): Album[] {
  const artistAlbums = albums.filter((album) => album.artist === artistName);
  if (!anchorTitle) {
    return artistAlbums
      .filter((album) => detectEditionMarkers(album.title).length > 0)
      .sort((a, b) => (a.year ?? 0) - (b.year ?? 0) || a.title.localeCompare(b.title));
  }

  const baseTitle = normalizeAlbumFamily(anchorTitle);
  return artistAlbums
    .filter((album) => album.id !== excludeId)
    .filter((album) => normalizeAlbumFamily(album.title) === baseTitle)
    .sort((a, b) => (a.year ?? 0) - (b.year ?? 0) || a.title.localeCompare(b.title));
}

export function missingAlbumsForArtist(requests: { artist: string; album: string; play_count: number }[], artistName: string) {
  const target = normalizeArtistKey(artistName);
  return requests
    .filter((entry) => normalizeArtistKey(entry.artist) === target)
    .sort((a, b) => b.play_count - a.play_count || a.album.localeCompare(b.album));
}

export function summarizeArtistMissing(
  entries: { artist: string; album: string; play_count: number }[],
): Array<{ artist: string; missingAlbums: number; playCount: number }> {
  const buckets = new Map<string, { artist: string; missingAlbums: number; playCount: number }>();
  for (const entry of entries) {
    const key = normalizeArtistKey(entry.artist);
    const current = buckets.get(key) ?? {
      artist: entry.artist,
      missingAlbums: 0,
      playCount: 0,
    };
    current.missingAlbums += 1;
    current.playCount += entry.play_count;
    buckets.set(key, current);
  }

  return [...buckets.values()].sort((a, b) =>
    b.missingAlbums - a.missingAlbums ||
    b.playCount - a.playCount ||
    a.artist.localeCompare(b.artist)
  );
}

export function normalizeAlbumFamily(title: string): string {
  return title
    .toLowerCase()
    .replace(/\(([^)]+)\)/g, '')
    .replace(/\b(deluxe|expanded|remaster(?:ed)?|live|anniversary|special|limited|collector'?s|edition|bonus)\b/g, '')
    .replace(/\s+/g, ' ')
    .trim();
}

export function normalizeArtistKey(value: string): string {
  return value.toLowerCase().replace(/&/g, 'and').replace(/\s+/g, ' ').trim();
}

export function samePath(left: string, right: string): boolean {
  return left.replace(/\\/g, '/').toLowerCase() === right.replace(/\\/g, '/').toLowerCase();
}
