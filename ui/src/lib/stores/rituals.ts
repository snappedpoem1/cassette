import { get, writable } from 'svelte/store';
import { api } from '$lib/api/tauri';

export interface PlaylistSection {
  id: string;
  title: string;
  arcLabel: string;
  note: string;
  startIndex: number;
  endIndex: number;
}

export interface PlaylistVariant {
  id: string;
  name: string;
  note: string;
  trackIds: number[];
  createdAt: string;
}

export interface PlaylistAuthorship {
  playlistId: number;
  note: string;
  mood: string;
  sections: PlaylistSection[];
  variants: PlaylistVariant[];
  updatedAt: string;
}

export interface CrateFilter {
  artistQuery: string;
  albumQuery: string;
  format: string;
  qualityTier: string;
  yearFrom: number | null;
  yearTo: number | null;
  playlistId: number | null;
}

export interface CrateRecord {
  id: string;
  name: string;
  note: string;
  kind: 'saved' | 'temporary';
  source: 'manual' | 'artist' | 'album' | 'quality' | 'format' | 'playlist';
  filter: CrateFilter;
  trackIds: number[];
  createdAt: string;
  updatedAt: string;
}

export interface QueueSceneRecord {
  id: string;
  name: string;
  note: string;
  trackIds: number[];
  startIndex: number;
  pinnedPositions: number[];
  heldPositions: number[];
  createdAt: string;
  updatedAt: string;
}

export interface LiveQueueRitual {
  pinnedPositions: number[];
  heldPositions: number[];
  lastPivotLabel: string | null;
}

export interface SessionModeSnapshot {
  trackCount: number;
  slope: number;
  era: 'mixed' | 'recent' | 'classic';
  durationTargetMin: number;
}

export interface SessionTransitionReason {
  toTrackId: number;
  reason: string;
}

export interface SessionRecord {
  id: string;
  name: string;
  note: string;
  trackIds: number[];
  reasons: SessionTransitionReason[];
  createdAt: string;
  updatedAt: string;
  source: 'generated' | 'playlist' | 'crate' | 'queue_scene' | 'branch';
  sourceRefId: string | null;
  branchOfId: string | null;
  modeSnapshot: SessionModeSnapshot | null;
  playCount: number;
}

const PLAYLIST_KEY = 'ui_playlist_authorship_json';
const CRATE_KEY = 'ui_crates_json';
const QUEUE_SCENE_KEY = 'ui_queue_scenes_json';
const LIVE_QUEUE_KEY = 'ui_live_queue_ritual_json';
const SESSION_KEY = 'ui_session_library_json';

const defaultQueueRitual: LiveQueueRitual = {
  pinnedPositions: [],
  heldPositions: [],
  lastPivotLabel: null,
};

export const playlistAuthorship = writable<Record<number, PlaylistAuthorship>>({});
export const crates = writable<CrateRecord[]>([]);
export const queueScenes = writable<QueueSceneRecord[]>([]);
export const liveQueueRitual = writable<LiveQueueRitual>(defaultQueueRitual);
export const sessionLibrary = writable<SessionRecord[]>([]);

let loadPromise: Promise<void> | null = null;

function nowIso(): string {
  return new Date().toISOString();
}

function ritualId(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

async function loadJsonSetting<T>(key: string, fallback: T): Promise<T> {
  try {
    const raw = await api.getSetting(key);
    if (!raw) {
      return fallback;
    }
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

async function saveJsonSetting<T>(key: string, value: T): Promise<void> {
  await api.setSetting(key, JSON.stringify(value));
}

export async function loadRituals(): Promise<void> {
  if (!loadPromise) {
    loadPromise = (async () => {
      const [playlistState, crateState, queueSceneState, liveQueueState, sessionState] = await Promise.all([
        loadJsonSetting<Record<string, PlaylistAuthorship>>(PLAYLIST_KEY, {}),
        loadJsonSetting<CrateRecord[]>(CRATE_KEY, []),
        loadJsonSetting<QueueSceneRecord[]>(QUEUE_SCENE_KEY, []),
        loadJsonSetting<LiveQueueRitual>(LIVE_QUEUE_KEY, defaultQueueRitual),
        loadJsonSetting<SessionRecord[]>(SESSION_KEY, []),
      ]);

      const normalizedPlaylistState: Record<number, PlaylistAuthorship> = {};
      for (const [key, value] of Object.entries(playlistState)) {
        const playlistId = Number.parseInt(key, 10);
        if (!Number.isFinite(playlistId)) {
          continue;
        }
        normalizedPlaylistState[playlistId] = {
          playlistId,
          note: value.note ?? '',
          mood: value.mood ?? '',
          sections: value.sections ?? [],
          variants: value.variants ?? [],
          updatedAt: value.updatedAt ?? nowIso(),
        };
      }

      playlistAuthorship.set(normalizedPlaylistState);
      crates.set(crateState ?? []);
      queueScenes.set(queueSceneState ?? []);
      liveQueueRitual.set({
        pinnedPositions: liveQueueState?.pinnedPositions ?? [],
        heldPositions: liveQueueState?.heldPositions ?? [],
        lastPivotLabel: liveQueueState?.lastPivotLabel ?? null,
      });
      sessionLibrary.set(sessionState ?? []);
    })();
  }

  await loadPromise;
}

async function persistPlaylistAuthorship(): Promise<void> {
  await saveJsonSetting(PLAYLIST_KEY, get(playlistAuthorship));
}

async function persistCrates(): Promise<void> {
  await saveJsonSetting(CRATE_KEY, get(crates));
}

async function persistQueueScenes(): Promise<void> {
  await saveJsonSetting(QUEUE_SCENE_KEY, get(queueScenes));
}

async function persistLiveQueueRitual(): Promise<void> {
  await saveJsonSetting(LIVE_QUEUE_KEY, get(liveQueueRitual));
}

async function persistSessions(): Promise<void> {
  await saveJsonSetting(SESSION_KEY, get(sessionLibrary));
}

export function authorshipForPlaylist(playlistId: number): PlaylistAuthorship {
  const state = get(playlistAuthorship)[playlistId];
  return (
    state ?? {
      playlistId,
      note: '',
      mood: '',
      sections: [],
      variants: [],
      updatedAt: nowIso(),
    }
  );
}

export async function updatePlaylistAuthorship(
  playlistId: number,
  patch: Partial<Omit<PlaylistAuthorship, 'playlistId'>>
): Promise<void> {
  playlistAuthorship.update((state) => ({
    ...state,
    [playlistId]: {
      ...authorshipForPlaylist(playlistId),
      ...patch,
      updatedAt: nowIso(),
    },
  }));
  await persistPlaylistAuthorship();
}

export async function savePlaylistSection(
  playlistId: number,
  section: Omit<PlaylistSection, 'id'> & { id?: string }
): Promise<string> {
  const sectionId = section.id ?? ritualId('section');
  const nextSection: PlaylistSection = {
    id: sectionId,
    title: section.title,
    arcLabel: section.arcLabel,
    note: section.note,
    startIndex: section.startIndex,
    endIndex: section.endIndex,
  };

  const current = authorshipForPlaylist(playlistId);
  const sections = current.sections.filter((item) => item.id !== sectionId);
  sections.push(nextSection);
  sections.sort((left, right) => left.startIndex - right.startIndex || left.endIndex - right.endIndex);
  await updatePlaylistAuthorship(playlistId, { sections });
  return sectionId;
}

export async function removePlaylistSection(playlistId: number, sectionId: string): Promise<void> {
  const current = authorshipForPlaylist(playlistId);
  await updatePlaylistAuthorship(playlistId, {
    sections: current.sections.filter((section) => section.id !== sectionId),
  });
}

export async function savePlaylistVariant(
  playlistId: number,
  variant: Omit<PlaylistVariant, 'id' | 'createdAt'> & { id?: string; createdAt?: string }
): Promise<string> {
  const variantId = variant.id ?? ritualId('variant');
  const nextVariant: PlaylistVariant = {
    id: variantId,
    name: variant.name,
    note: variant.note,
    trackIds: [...variant.trackIds],
    createdAt: variant.createdAt ?? nowIso(),
  };

  const current = authorshipForPlaylist(playlistId);
  const variants = current.variants.filter((item) => item.id !== variantId);
  variants.unshift(nextVariant);
  await updatePlaylistAuthorship(playlistId, { variants: variants.slice(0, 16) });
  return variantId;
}

export async function removePlaylistVariant(playlistId: number, variantId: string): Promise<void> {
  const current = authorshipForPlaylist(playlistId);
  await updatePlaylistAuthorship(playlistId, {
    variants: current.variants.filter((variant) => variant.id !== variantId),
  });
}

export async function saveCrate(
  crate: Omit<CrateRecord, 'id' | 'createdAt' | 'updatedAt'> & { id?: string; createdAt?: string }
): Promise<string> {
  const crateId = crate.id ?? ritualId('crate');
  const nextCrate: CrateRecord = {
    ...crate,
    id: crateId,
    createdAt: crate.createdAt ?? nowIso(),
    updatedAt: nowIso(),
    trackIds: [...crate.trackIds],
  };

  crates.update((current) => {
    const remaining = current.filter((item) => item.id !== crateId);
    return [nextCrate, ...remaining].slice(0, 36);
  });
  await persistCrates();
  return crateId;
}

export async function deleteCrate(crateId: string): Promise<void> {
  crates.update((current) => current.filter((crate) => crate.id !== crateId));
  await persistCrates();
}

export async function saveQueueScene(
  scene: Omit<QueueSceneRecord, 'id' | 'createdAt' | 'updatedAt'> & { id?: string; createdAt?: string }
): Promise<string> {
  const sceneId = scene.id ?? ritualId('scene');
  const nextScene: QueueSceneRecord = {
    ...scene,
    id: sceneId,
    createdAt: scene.createdAt ?? nowIso(),
    updatedAt: nowIso(),
    trackIds: [...scene.trackIds],
    pinnedPositions: [...scene.pinnedPositions],
    heldPositions: [...scene.heldPositions],
  };

  queueScenes.update((current) => {
    const remaining = current.filter((item) => item.id !== sceneId);
    return [nextScene, ...remaining].slice(0, 18);
  });
  await persistQueueScenes();
  return sceneId;
}

export async function deleteQueueScene(sceneId: string): Promise<void> {
  queueScenes.update((current) => current.filter((scene) => scene.id !== sceneId));
  await persistQueueScenes();
}

export async function updateLiveQueueRitual(patch: Partial<LiveQueueRitual>): Promise<void> {
  liveQueueRitual.update((current) => ({
    ...current,
    ...patch,
  }));
  await persistLiveQueueRitual();
}

export async function resetLiveQueueRitual(): Promise<void> {
  liveQueueRitual.set(defaultQueueRitual);
  await persistLiveQueueRitual();
}

export async function saveSessionRecord(
  session: Omit<SessionRecord, 'id' | 'createdAt' | 'updatedAt' | 'playCount'> & {
    id?: string;
    createdAt?: string;
    playCount?: number;
  }
): Promise<string> {
  const sessionId = session.id ?? ritualId('session');
  const nextSession: SessionRecord = {
    ...session,
    id: sessionId,
    createdAt: session.createdAt ?? nowIso(),
    updatedAt: nowIso(),
    playCount: session.playCount ?? 0,
    trackIds: [...session.trackIds],
    reasons: [...session.reasons],
  };

  sessionLibrary.update((current) => {
    const remaining = current.filter((item) => item.id !== sessionId);
    return [nextSession, ...remaining].slice(0, 28);
  });
  await persistSessions();
  return sessionId;
}

export async function deleteSessionRecord(sessionId: string): Promise<void> {
  sessionLibrary.update((current) => current.filter((session) => session.id !== sessionId));
  await persistSessions();
}

export async function branchSessionRecord(
  sessionId: string,
  patch: Partial<Pick<SessionRecord, 'name' | 'note' | 'trackIds' | 'reasons' | 'modeSnapshot'>>
): Promise<string | null> {
  const original = get(sessionLibrary).find((session) => session.id === sessionId);
  if (!original) {
    return null;
  }

  return saveSessionRecord({
    ...original,
    id: undefined,
    name: patch.name ?? `${original.name} / branch`,
    note: patch.note ?? original.note,
    trackIds: patch.trackIds ?? original.trackIds,
    reasons: patch.reasons ?? original.reasons,
    modeSnapshot: patch.modeSnapshot ?? original.modeSnapshot,
    source: 'branch',
    sourceRefId: original.id,
    branchOfId: original.id,
    playCount: 0,
  });
}

export async function bumpSessionPlayCount(sessionId: string): Promise<void> {
  sessionLibrary.update((current) =>
    current.map((session) =>
      session.id === sessionId
        ? {
            ...session,
            playCount: session.playCount + 1,
            updatedAt: nowIso(),
          }
        : session
    )
  );
  await persistSessions();
}

export function emptyCrateFilter(): CrateFilter {
  return {
    artistQuery: '',
    albumQuery: '',
    format: '',
    qualityTier: '',
    yearFrom: null,
    yearTo: null,
    playlistId: null,
  };
}
