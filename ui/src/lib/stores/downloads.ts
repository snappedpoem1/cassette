import { writable } from 'svelte/store';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import {
  api,
  type DirectorEvent,
  type DirectorTaskResult,
  type DownloadJob,
  type DownloadConfig,
  type ProviderHealthEvent,
  type ProviderStatus,
  type DownloadMetadataSearchResult,
  type DownloadArtistDiscography,
  type BacklogRunStatus,
  type DirectorDebugStats,
  type SlskdRuntimeStatus,
} from '$lib/api/tauri';

export const downloadJobs = writable<DownloadJob[]>([]);
export const downloadConfig = writable<DownloadConfig | null>(null);
export const providerStatuses = writable<ProviderStatus[]>([]);
export const providerHealth = writable<Record<string, ProviderHealthEvent>>({});
export const slskdRuntimeStatus = writable<SlskdRuntimeStatus | null>(null);
export const metadataSearchResults = writable<DownloadMetadataSearchResult | null>(null);
export const artistDiscography = writable<DownloadArtistDiscography | null>(null);
export const isSearchingMetadata = writable(false);
export const backlogStatus = writable<BacklogRunStatus | null>(null);
export const debugStats = writable<DirectorDebugStats | null>(null);

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
    slskdRuntimeStatus.set(await api.getSlskdRuntimeStatus());
  } catch {
    downloadConfig.set(null);
    providerStatuses.set([]);
    slskdRuntimeStatus.set(null);
  }
}

export async function saveDownloadConfig(config: DownloadConfig) {
  await api.saveConfig(config);
  await loadDownloadConfig();
}

export async function persistEffectiveDownloadConfig() {
  await api.persistEffectiveConfig();
  await loadDownloadConfig();
}

export async function refreshSlskdRuntimeStatus() {
  try {
    slskdRuntimeStatus.set(await api.getSlskdRuntimeStatus());
  } catch {
    slskdRuntimeStatus.set(null);
  }
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

let unlisteners: UnlistenFn[] = [];
let visibilityHandler: (() => void) | null = null;

function upsertJob(taskId: string, mutate: (job: DownloadJob) => DownloadJob) {
  downloadJobs.update((jobs) => {
    const index = jobs.findIndex((job) => job.id === taskId);
    if (index === -1) {
      return jobs;
    }
    const next = [...jobs];
    next[index] = mutate(next[index]);
    return next;
  });
}

function applyDirectorEvent(event: DirectorEvent) {
  upsertJob(event.task_id, (job) => {
    const next = { ...job, provider: event.provider_id, error: event.message ?? null };
    switch (event.progress) {
      case 'Queued':
        return { ...next, status: 'Queued', progress: 0 };
      case 'InProgress':
      case 'ProviderAttempt':
        return { ...next, status: 'Searching', progress: 0.15 };
      case 'Validating':
        return { ...next, status: 'Verifying', progress: 0.65 };
      case 'Tagging':
      case 'Finalizing':
        return { ...next, status: 'Verifying', progress: 0.85 };
      case 'Finalized':
      case 'Skipped':
        return { ...next, status: 'Done', progress: 1, error: null };
      case 'Cancelled':
        return { ...next, status: 'Cancelled', progress: 0, error: event.message };
      case 'Failed':
      case 'Exhausted':
        return { ...next, status: 'Failed', progress: 0 };
      default:
        return next;
    }
  });
}

function applyDirectorResult(result: DirectorTaskResult) {
  upsertJob(result.task_id, (job) => {
    switch (result.disposition) {
      case 'Finalized':
      case 'AlreadyPresent':
      case 'MetadataOnly':
        return { ...job, status: 'Done', progress: 1, error: null };
      case 'Cancelled':
        return { ...job, status: 'Cancelled', progress: 0, error: result.error ?? 'Cancelled by user' };
      case 'Failed':
      default:
        return { ...job, status: 'Failed', progress: 0, error: result.error };
    }
  });
}

export async function startDownloadSupervision() {
  if (unlisteners.length > 0) {
    return;
  }

  unlisteners = await Promise.all([
    listen<DirectorEvent>('director-event', ({ payload }) => applyDirectorEvent(payload)),
    listen<DirectorTaskResult>('director-result', ({ payload }) => applyDirectorResult(payload)),
    listen<ProviderHealthEvent>('director-provider-health', ({ payload }) => {
      providerHealth.update((current) => ({ ...current, [payload.provider_id]: payload }));
    }),
    listen<BacklogRunStatus>('director-backlog-progress', ({ payload }) => {
      backlogStatus.set(payload);
    }),
  ]);

  visibilityHandler = () => {
    if (document.visibilityState === 'visible') {
      void loadDownloadJobs();
      void refreshBacklogStatus();
      void refreshSlskdRuntimeStatus();
    }
  };
  document.addEventListener('visibilitychange', visibilityHandler);
}

export function stopDownloadSupervision() {
  for (const unlisten of unlisteners) {
    unlisten();
  }
  unlisteners = [];
  if (visibilityHandler) {
    document.removeEventListener('visibilitychange', visibilityHandler);
    visibilityHandler = null;
  }
}

export async function refreshBacklogStatus() {
  try {
    backlogStatus.set(await api.getBacklogStatus());
  } catch {
    // ignore
  }
}

export async function startBacklogRun(batchSize?: number, limit?: number) {
  try {
    const status = await api.startBacklogRun(batchSize, limit);
    backlogStatus.set(status);
  } catch (e) {
    console.error('startBacklogRun failed:', e);
  }
}

export async function stopBacklogRun() {
  try {
    await api.stopBacklogRun();
    await refreshBacklogStatus();
  } catch {
    // ignore
  }
}

export async function refreshDebugStats() {
  try {
    debugStats.set(await api.getDirectorDebugStats(100));
  } catch {
    debugStats.set(null);
  }
}
