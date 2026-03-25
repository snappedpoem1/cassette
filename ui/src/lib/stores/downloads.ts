import { writable } from 'svelte/store';
import {
  api,
  type DownloadJob,
  type DownloadConfig,
  type ProviderStatus,
  type DownloadMetadataSearchResult,
  type DownloadArtistDiscography,
} from '$lib/api/tauri';

export const downloadJobs = writable<DownloadJob[]>([]);
export const downloadConfig = writable<DownloadConfig | null>(null);
export const providerStatuses = writable<ProviderStatus[]>([]);
export const metadataSearchResults = writable<DownloadMetadataSearchResult | null>(null);
export const artistDiscography = writable<DownloadArtistDiscography | null>(null);
export const isSearchingMetadata = writable(false);

export async function loadDownloadJobs() {
  try {
    downloadJobs.set(await api.getDownloadJobs());
  } catch {
    downloadJobs.set([]);
  }
}

export async function loadDownloadConfig() {
  try {
    downloadConfig.set(await api.getConfig());
    providerStatuses.set(await api.getProviderStatuses());
  } catch {
    downloadConfig.set(null);
    providerStatuses.set([]);
  }
}

export async function saveDownloadConfig(config: DownloadConfig) {
  await api.saveConfig(config);
  await loadDownloadConfig();
}

export async function searchMetadata(query: string) {
  isSearchingMetadata.set(true);
  try {
    metadataSearchResults.set(await api.searchDownloadMetadata(query));
  } catch {
    metadataSearchResults.set(null);
  } finally {
    isSearchingMetadata.set(false);
  }
}

export async function loadDiscography(artist: string, mbid?: string) {
  try {
    artistDiscography.set(await api.getArtistDiscography(artist, mbid));
  } catch {
    artistDiscography.set(null);
  }
}

let pollTimer: ReturnType<typeof setInterval> | null = null;

export function startJobsPoll() {
  if (pollTimer) return;
  pollTimer = setInterval(loadDownloadJobs, 2000);
}

export function stopJobsPoll() {
  if (pollTimer) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}
