import type { Album, Artist } from '$lib/api/tauri';

export interface ArtistCluster {
  key: string;
  primaryName: string;
  aliases: string[];
  members: Artist[];
  albumCount: number;
  trackCount: number;
}

const PUNCT_OR_SYMBOL = /[.,'"`()\[\]{}!?/\\+_-]|\u2019/g;
const FEAT_SUFFIX = /\b(feat|featuring|ft|with)\b.*$/i;

export function normalizeArtistKey(name: string): string {
  return name
    .toLowerCase()
    .replace(/&/g, ' and ')
    .replace(FEAT_SUFFIX, '')
    .replace(PUNCT_OR_SYMBOL, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

function pickPrimaryName(names: string[]): string {
  return names
    .slice()
    .sort((a, b) => {
      const len = a.length - b.length;
      return len !== 0 ? len : a.localeCompare(b);
    })[0] ?? names[0] ?? 'Unknown Artist';
}

export function buildArtistClusters(artists: Artist[]): ArtistCluster[] {
  const byKey = new Map<string, Artist[]>();

  for (const artist of artists) {
    const key = normalizeArtistKey(artist.name);
    if (!key) {
      continue;
    }
    if (!byKey.has(key)) {
      byKey.set(key, []);
    }
    byKey.get(key)!.push(artist);
  }

  return Array.from(byKey.entries())
    .map(([key, members]) => {
      const aliases = members.map((member) => member.name).sort((a, b) => a.localeCompare(b));
      const albumCount = members.reduce((sum, member) => sum + member.album_count, 0);
      const trackCount = members.reduce((sum, member) => sum + member.track_count, 0);

      return {
        key,
        primaryName: pickPrimaryName(aliases),
        aliases,
        members,
        albumCount,
        trackCount,
      } satisfies ArtistCluster;
    })
    .sort((a, b) => {
      if (a.primaryName !== b.primaryName) {
        return a.primaryName.localeCompare(b.primaryName);
      }
      return b.trackCount - a.trackCount;
    });
}

export function clusterAlbumsForArtist(albums: Album[], cluster: ArtistCluster): Album[] {
  const memberNames = new Set(cluster.members.map((member) => member.name));

  return albums
    .filter((album) => memberNames.has(album.artist) || normalizeArtistKey(album.artist) === cluster.key)
    .sort((a, b) => {
      const yearA = a.year ?? Number.MAX_SAFE_INTEGER;
      const yearB = b.year ?? Number.MAX_SAFE_INTEGER;
      if (yearA !== yearB) {
        return yearA - yearB;
      }
      return a.title.localeCompare(b.title);
    });
}
