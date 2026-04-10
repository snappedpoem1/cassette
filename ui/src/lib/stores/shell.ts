import { browser } from '$app/environment';
import { writable } from 'svelte/store';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { toDesktopRuntimeMessage } from '$lib/api/tauri';

export type UtilityWellMode = 'queue' | 'room' | 'context';
export type WorkspacePresetId = 'listen_queue' | 'acquisition';

const STORAGE_KEY = 'cassette.shell.compactPlayer';
const LIBRARY_WIDTH_KEY = 'cassette.shell.libraryWidth';
const UTILITY_WIDTH_KEY = 'cassette.shell.utilityWidth';
const UTILITY_COLLAPSED_KEY = 'cassette.shell.utilityCollapsed';
const UTILITY_MODE_KEY = 'cassette.shell.utilityMode';
const WORKSTATION_OPEN_KEY = 'cassette.shell.workstationOpen';
const ACTIVE_PRESET_KEY = 'cassette.shell.activePreset';
const VISUALIZER_WINDOW_GEOMETRY_KEY = 'cassette.shell.visualizerWindowGeometry';

const VISUALIZER_WINDOW_LABEL = 'visualizer';
const VISUALIZER_WINDOW_PATH = '/visualizer-window';

const MIN_LIBRARY_WIDTH = 260;
const MAX_LIBRARY_WIDTH = 520;
const MIN_UTILITY_WIDTH = 240;
const MAX_UTILITY_WIDTH = 420;

interface StoredWindowGeometry {
  x: number;
  y: number;
  width: number;
  height: number;
}

function readStoredCompactPlayer(): boolean {
  if (!browser) {
    return false;
  }

  return window.localStorage.getItem(STORAGE_KEY) === '1';
}

function readStoredNumber(key: string, fallback: number): number {
  if (!browser) {
    return fallback;
  }

  const raw = window.localStorage.getItem(key);
  if (!raw) {
    return fallback;
  }

  const parsed = Number.parseInt(raw, 10);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function readStoredBoolean(key: string, fallback: boolean): boolean {
  if (!browser) {
    return fallback;
  }

  const raw = window.localStorage.getItem(key);
  if (raw === null) {
    return fallback;
  }

  return raw === '1';
}

function readStoredUtilityMode(): UtilityWellMode {
  if (!browser) {
    return 'queue';
  }

  const raw = window.localStorage.getItem(UTILITY_MODE_KEY);
  if (raw === 'room' || raw === 'context' || raw === 'queue') {
    return raw;
  }

  return 'queue';
}

function readStoredPreset(): WorkspacePresetId {
  if (!browser) {
    return 'listen_queue';
  }

  return window.localStorage.getItem(ACTIVE_PRESET_KEY) === 'acquisition'
    ? 'acquisition'
    : 'listen_queue';
}

function clampWidth(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function readStoredWindowGeometry(): StoredWindowGeometry | null {
  if (!browser) {
    return null;
  }

  const raw = window.localStorage.getItem(VISUALIZER_WINDOW_GEOMETRY_KEY);
  if (!raw) {
    return null;
  }

  try {
    const parsed = JSON.parse(raw) as Partial<StoredWindowGeometry>;
    if (
      typeof parsed.x === 'number' &&
      typeof parsed.y === 'number' &&
      typeof parsed.width === 'number' &&
      typeof parsed.height === 'number'
    ) {
      return {
        x: Math.round(parsed.x),
        y: Math.round(parsed.y),
        width: Math.round(parsed.width),
        height: Math.round(parsed.height),
      };
    }
  } catch {
    // Ignore invalid persisted geometry and fall back to defaults.
  }

  return null;
}

async function focusOwnedWindow(window: WebviewWindow): Promise<void> {
  const minimized = await window.isMinimized();
  if (minimized) {
    await window.unminimize();
  }
  await window.show();
  await window.setFocus();
}

export const compactPlayerMode = writable<boolean>(readStoredCompactPlayer());
export const shellActionError = writable<string | null>(null);
export const libraryRailWidth = writable<number>(readStoredNumber(LIBRARY_WIDTH_KEY, 332));
export const utilityWellWidth = writable<number>(readStoredNumber(UTILITY_WIDTH_KEY, 308));
export const utilityWellCollapsed = writable<boolean>(readStoredBoolean(UTILITY_COLLAPSED_KEY, false));
export const utilityWellMode = writable<UtilityWellMode>(readStoredUtilityMode());
export const workstationDeckOpen = writable<boolean>(readStoredBoolean(WORKSTATION_OPEN_KEY, false));
export const activeWorkspacePreset = writable<WorkspacePresetId>(readStoredPreset());

compactPlayerMode.subscribe((enabled) => {
  if (!browser) {
    return;
  }

  window.localStorage.setItem(STORAGE_KEY, enabled ? '1' : '0');
});

libraryRailWidth.subscribe((width) => {
  if (!browser) {
    return;
  }

  window.localStorage.setItem(
    LIBRARY_WIDTH_KEY,
    String(clampWidth(Math.round(width), MIN_LIBRARY_WIDTH, MAX_LIBRARY_WIDTH))
  );
});

utilityWellWidth.subscribe((width) => {
  if (!browser) {
    return;
  }

  window.localStorage.setItem(
    UTILITY_WIDTH_KEY,
    String(clampWidth(Math.round(width), MIN_UTILITY_WIDTH, MAX_UTILITY_WIDTH))
  );
});

utilityWellCollapsed.subscribe((collapsed) => {
  if (!browser) {
    return;
  }

  window.localStorage.setItem(UTILITY_COLLAPSED_KEY, collapsed ? '1' : '0');
});

utilityWellMode.subscribe((mode) => {
  if (!browser) {
    return;
  }

  window.localStorage.setItem(UTILITY_MODE_KEY, mode);
});

workstationDeckOpen.subscribe((open) => {
  if (!browser) {
    return;
  }

  window.localStorage.setItem(WORKSTATION_OPEN_KEY, open ? '1' : '0');
});

activeWorkspacePreset.subscribe((preset) => {
  if (!browser) {
    return;
  }

  window.localStorage.setItem(ACTIVE_PRESET_KEY, preset);
});

export function toggleCompactPlayerMode(): void {
  compactPlayerMode.update((enabled) => !enabled);
}

export function setLibraryRailWidth(width: number): void {
  libraryRailWidth.set(clampWidth(width, MIN_LIBRARY_WIDTH, MAX_LIBRARY_WIDTH));
}

export function revealLibraryRail(minWidth = 320): void {
  libraryRailWidth.update((width) => clampWidth(Math.max(width, minWidth), MIN_LIBRARY_WIDTH, MAX_LIBRARY_WIDTH));
}

export function setUtilityWellWidth(width: number): void {
  utilityWellWidth.set(clampWidth(width, MIN_UTILITY_WIDTH, MAX_UTILITY_WIDTH));
}

export function setUtilityWellMode(mode: UtilityWellMode): void {
  utilityWellMode.set(mode);
  utilityWellCollapsed.set(false);
}

export function toggleUtilityWellCollapsed(): void {
  utilityWellCollapsed.update((collapsed) => !collapsed);
}

export function openUtilityWell(mode?: UtilityWellMode): void {
  if (mode) {
    utilityWellMode.set(mode);
  }
  utilityWellCollapsed.set(false);
}

export function closeUtilityWell(): void {
  utilityWellCollapsed.set(true);
}

export function ensureQueueWellVisible(): void {
  utilityWellMode.set('queue');
  utilityWellCollapsed.set(false);
}

export function openWorkstationDeck(): void {
  workstationDeckOpen.set(true);
}

export function closeWorkstationDeck(): void {
  workstationDeckOpen.set(false);
}

export function toggleWorkstationDeck(): void {
  workstationDeckOpen.update((open) => !open);
}

export async function openVisualizerWindow(): Promise<void> {
  if (!browser) {
    return;
  }

  try {
    const existing = await WebviewWindow.getByLabel(VISUALIZER_WINDOW_LABEL);
    if (existing) {
      await focusOwnedWindow(existing);
      shellActionError.set(null);
      return;
    }

    const storedGeometry = readStoredWindowGeometry();
    const windowOptions = {
      url: VISUALIZER_WINDOW_PATH,
      title: 'Cassette Visualizer',
      parent: 'main',
      center: storedGeometry ? undefined : true,
      x: storedGeometry?.x,
      y: storedGeometry?.y,
      width: storedGeometry?.width ?? 1040,
      height: storedGeometry?.height ?? 420,
      minWidth: 720,
      minHeight: 240,
      preventOverflow: true,
      resizable: true,
      visible: true,
      focus: true,
      decorations: true,
    };

    const visualizerWindow = new WebviewWindow(VISUALIZER_WINDOW_LABEL, windowOptions);

    void visualizerWindow.once('tauri://created', async () => {
      try {
        await focusOwnedWindow(visualizerWindow);
        shellActionError.set(null);
      } catch (error) {
        shellActionError.set(
          toDesktopRuntimeMessage(error, 'Visualizer window opened but could not be focused.')
        );
      }
    });

    void visualizerWindow.once('tauri://error', (event) => {
      shellActionError.set(
        toDesktopRuntimeMessage(event.payload, 'Failed to open the visualizer window.')
      );
    });
  } catch (error) {
    shellActionError.set(
      toDesktopRuntimeMessage(error, 'Failed to open the visualizer window.')
    );
  }
}

export function applyWorkspacePreset(preset: WorkspacePresetId): void {
  activeWorkspacePreset.set(preset);

  if (preset === 'acquisition') {
    utilityWellMode.set('room');
    utilityWellCollapsed.set(false);
    workstationDeckOpen.set(true);
    return;
  }

  utilityWellMode.set('queue');
  utilityWellCollapsed.set(false);
  workstationDeckOpen.set(false);
}

export async function minimizeAppWindow(): Promise<void> {
  if (!browser) {
    return;
  }

  try {
    await getCurrentWindow().minimize();
    shellActionError.set(null);
  } catch (error) {
    shellActionError.set(toDesktopRuntimeMessage(error, 'Failed to minimize the window.'));
  }
}

export async function restoreAppWindow(): Promise<void> {
  if (!browser) {
    return;
  }

  try {
    await getCurrentWindow().unminimize();
    await getCurrentWindow().setFocus();
    shellActionError.set(null);
  } catch (error) {
    shellActionError.set(toDesktopRuntimeMessage(error, 'Failed to restore the window.'));
  }
}

export async function toggleMinimizedWindowState(): Promise<void> {
  if (!browser) {
    return;
  }

  try {
    const window = getCurrentWindow();
    const minimized = await window.isMinimized();
    if (minimized) {
      await window.unminimize();
      await window.setFocus();
    } else {
      await window.minimize();
    }
    shellActionError.set(null);
  } catch (error) {
    shellActionError.set(toDesktopRuntimeMessage(error, 'Failed to change window state.'));
  }
}
