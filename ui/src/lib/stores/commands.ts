import { derived, writable } from 'svelte/store';
import { goto } from '$app/navigation';
import { player } from '$lib/stores/player';
import { activeTab } from '$lib/stores/library';
import {
  toggleCompactPlayerMode,
  minimizeAppWindow,
  openVisualizerWindow,
  openWorkstationDeck,
  restoreAppWindow,
  revealLibraryRail,
} from '$lib/stores/shell';

export type CommandId =
  | 'nav.home'
  | 'nav.collection'
  | 'nav.now_playing'
  | 'nav.artists'
  | 'nav.playlists'
  | 'nav.crates'
  | 'nav.queue'
  | 'nav.session'
  | 'nav.workstation'
  | 'nav.library_browser'
  | 'player.toggle'
  | 'player.next'
  | 'player.previous'
  | 'shell.toggle_compact_player'
  | 'window.visualizer'
  | 'window.minimize'
  | 'window.restore';

export type CommandCategory = 'Navigation' | 'Playback' | 'Window';

export interface AppCommand {
  id: CommandId;
  label: string;
  category: CommandCategory;
  shortcut?: string;
  aliases?: string[];
  run: () => Promise<void> | void;
}

const commandList: AppCommand[] = [
  {
    id: 'nav.home',
    label: 'Open Home',
    category: 'Navigation',
    shortcut: 'Alt+1',
    aliases: ['return', 'front door'],
    run: () => goto('/'),
  },
  {
    id: 'nav.collection',
    label: 'Open Collection',
    category: 'Navigation',
    shortcut: 'Alt+2',
    aliases: ['ownership', 'shelves'],
    run: () => goto('/collection'),
  },
  {
    id: 'nav.now_playing',
    label: 'Open Now Playing',
    category: 'Navigation',
    aliases: ['shrine', 'immersion', 'focus'],
    run: () => goto('/now-playing'),
  },
  {
    id: 'nav.artists',
    label: 'Open Artists',
    category: 'Navigation',
    shortcut: 'Alt+3',
    aliases: ['performers', 'rediscovery'],
    run: () => goto('/artists'),
  },
  {
    id: 'nav.playlists',
    label: 'Open Playlists',
    category: 'Navigation',
    shortcut: 'Alt+4',
    aliases: ['lists', 'authorship'],
    run: () => goto('/playlists'),
  },
  {
    id: 'nav.crates',
    label: 'Open Crates',
    category: 'Navigation',
    aliases: ['slices', 'shelves', 'saved slice'],
    run: () => goto('/crates'),
  },
  {
    id: 'nav.queue',
    label: 'Open Queue',
    category: 'Navigation',
    shortcut: 'Alt+5',
    aliases: ['up next', 'scene'],
    run: () => goto('/queue'),
  },
  {
    id: 'nav.session',
    label: 'Open Session',
    category: 'Navigation',
    shortcut: 'Alt+6',
    aliases: ['arc', 'memory'],
    run: () => goto('/session'),
  },
  {
    id: 'nav.workstation',
    label: 'Open Workstation Deck',
    category: 'Navigation',
    shortcut: 'Alt+7',
    aliases: ['inbox', 'repairs', 'settings', 'downloads'],
    run: () => openWorkstationDeck(),
  },
  {
    id: 'nav.library_browser',
    label: 'Focus Library Rail',
    category: 'Navigation',
    aliases: ['library', 'albums', 'tracks'],
    run: () => {
      activeTab.set('albums');
      revealLibraryRail();
    },
  },
  {
    id: 'player.toggle',
    label: 'Toggle Play / Pause',
    category: 'Playback',
    shortcut: 'Space',
    aliases: ['play', 'pause'],
    run: () => player.toggle(),
  },
  {
    id: 'player.next',
    label: 'Next Track',
    category: 'Playback',
    shortcut: 'Ctrl+Right',
    aliases: ['skip'],
    run: () => player.next(),
  },
  {
    id: 'player.previous',
    label: 'Previous Track',
    category: 'Playback',
    shortcut: 'Ctrl+Left',
    aliases: ['back'],
    run: () => player.prev(),
  },
  {
    id: 'shell.toggle_compact_player',
    label: 'Toggle Compact Player',
    category: 'Playback',
    shortcut: 'Ctrl+M',
    aliases: ['mini player', 'compact mode'],
    run: () => toggleCompactPlayerMode(),
  },
  {
    id: 'window.visualizer',
    label: 'Open Visualizer Window',
    category: 'Window',
    aliases: ['detached visualizer', 'breakout visualizer', 'visualizer'],
    run: () => openVisualizerWindow(),
  },
  {
    id: 'window.minimize',
    label: 'Minimize Window',
    category: 'Window',
    shortcut: 'Ctrl+Down',
    aliases: ['taskbar', 'hide app'],
    run: () => minimizeAppWindow(),
  },
  {
    id: 'window.restore',
    label: 'Restore Window',
    category: 'Window',
    shortcut: 'Ctrl+Up',
    aliases: ['show app'],
    run: () => restoreAppWindow(),
  },
];

const paletteOpen = writable(false);
const paletteQuery = writable('');

export const commands = writable<AppCommand[]>(commandList);
export const isPaletteOpen = derived(paletteOpen, ($open) => $open);
export const paletteSearchQuery = derived(paletteQuery, ($query) => $query);

export const filteredCommands = derived([commands, paletteQuery], ([$commands, $query]) => {
  const query = $query.trim().toLowerCase();
  if (!query) {
    return $commands;
  }

  return $commands.filter((command) => {
    const haystack = [command.label, command.category, ...(command.aliases ?? [])].join(' ').toLowerCase();
    return haystack.includes(query);
  });
});

export function openPalette(): void {
  paletteOpen.set(true);
}

export function closePalette(): void {
  paletteOpen.set(false);
  paletteQuery.set('');
}

export function togglePalette(): void {
  paletteOpen.update((open) => !open);
  paletteQuery.set('');
}

export function setPaletteQuery(query: string): void {
  paletteQuery.set(query);
}

export async function executeCommand(command: AppCommand): Promise<void> {
  await command.run();
  closePalette();
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  if (target.isContentEditable) {
    return true;
  }

  const tag = target.tagName.toLowerCase();
  return tag === 'input' || tag === 'textarea' || tag === 'select';
}

function normalizeShortcutPart(part: string): string {
  return part.trim().toLowerCase();
}

function normalizeKey(key: string): string {
  if (key === ' ') {
    return 'space';
  }
  return key.toLowerCase();
}

function matchesShortcut(event: KeyboardEvent, shortcut: string): boolean {
  const parts = shortcut.split('+').map(normalizeShortcutPart);
  const keyPart = parts[parts.length - 1];

  const requiresCtrl = parts.includes('ctrl');
  const requiresAlt = parts.includes('alt');
  const requiresShift = parts.includes('shift');
  const requiresMeta = parts.includes('meta') || parts.includes('cmd');

  if (event.ctrlKey !== requiresCtrl) return false;
  if (event.altKey !== requiresAlt) return false;
  if (event.shiftKey !== requiresShift) return false;
  if (event.metaKey !== requiresMeta) return false;

  return normalizeKey(event.key) === keyPart;
}

export async function handleGlobalShortcut(event: KeyboardEvent): Promise<boolean> {
  if (isEditableTarget(event.target)) {
    return false;
  }

  const commandEntries = commandList.filter((command) => command.shortcut);
  for (const command of commandEntries) {
    const shortcut = command.shortcut;
    if (!shortcut) continue;
    if (!matchesShortcut(event, shortcut)) continue;

    event.preventDefault();
    await command.run();
    return true;
  }

  return false;
}
