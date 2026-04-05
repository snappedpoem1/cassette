import { browser } from '$app/environment';
import { writable } from 'svelte/store';
import { getCurrentWindow } from '@tauri-apps/api/window';

const STORAGE_KEY = 'cassette.shell.compactPlayer';

function readStoredCompactPlayer(): boolean {
  if (!browser) {
    return false;
  }

  return window.localStorage.getItem(STORAGE_KEY) === '1';
}

export const compactPlayerMode = writable<boolean>(readStoredCompactPlayer());

compactPlayerMode.subscribe((enabled) => {
  if (!browser) {
    return;
  }

  window.localStorage.setItem(STORAGE_KEY, enabled ? '1' : '0');
});

export function toggleCompactPlayerMode(): void {
  compactPlayerMode.update((enabled) => !enabled);
}

export async function minimizeAppWindow(): Promise<void> {
  if (!browser) {
    return;
  }

  try {
    await getCurrentWindow().minimize();
  } catch {
    // noop when desktop shell is unavailable
  }
}

export async function restoreAppWindow(): Promise<void> {
  if (!browser) {
    return;
  }

  try {
    await getCurrentWindow().unminimize();
    await getCurrentWindow().setFocus();
  } catch {
    // noop when desktop shell is unavailable
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
  } catch {
    // noop when desktop shell is unavailable
  }
}
