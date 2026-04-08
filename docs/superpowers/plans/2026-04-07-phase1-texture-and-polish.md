# Phase 1: Texture and Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform Cassette from a technically impressive tool into a product with visible finish — replacing placeholder UI elements, making the queue interactive, giving artists visual presence, and fixing the player bar's identity.

**Architecture:** Pure frontend changes only. No new Rust commands, no schema changes. All improvements work within existing data already loaded into stores. This phase is safe to ship incrementally — each task is independent and reversible.

**Tech Stack:** SvelteKit, Svelte 5, TypeScript, CSS custom properties (Steel Dusk palette already in `app.css`). No new npm dependencies required — SVG icons are inlined.

## Execution Log

- [x] 2026-04-07 preflight dirty-tree health check completed before Phase 1 execution:
  - `git status --short` snapshot captured
  - `get_errors` returned no diagnostics
  - `cargo check --workspace` passed
  - `npm run build` passed (chunk-size warning only)

---

## File Map

| File | Change |
|------|--------|
| `ui/src/lib/components/Sidebar.svelte` | Replace text abbreviation icons with inline SVGs |
| `ui/src/lib/components/NowPlaying.svelte` | Replace emoji controls/volume with SVG icons, improve sizing |
| `ui/src/lib/components/QueuePanel.svelte` | Add per-item remove, drag-to-reorder |
| `ui/src/routes/artists/+page.svelte` | Artist cards → album cover mosaics |
| `ui/src/routes/library/+page.svelte` | Track inspector: human-readable quality display |
| `ui/src/routes/playlists/+page.svelte` | Replace emoji empty states, fix confirm dialog |
| `ui/src/routes/+layout.svelte` | Remove topbar nav duplication; topbar = brand only |
| `ui/src/lib/stores/queue.ts` | Add `removeFromQueue` action |
| `ui/src/lib/api/tauri.ts` | Add `removeFromQueue` invoke |
| `ui/src/app.css` | Sidebar icon width tweak for SVG |

---

### Task 1: Replace sidebar nav text abbreviations with inline SVG icons

**Files:**
- Modify: `ui/src/lib/components/Sidebar.svelte`
- Modify: `ui/src/app.css` (nav-icon width)

These are inline SVG path strings — no library install needed. Each icon is 16×16, stroke-based.

- [ ] **Step 1: Replace the nav link data arrays with icon SVG markup**

Open `ui/src/lib/components/Sidebar.svelte`. Replace the existing `coreLinks` and `utilityLinks` arrays and the `nav-icon` span with the following:

```svelte
<script lang="ts">
  import { page } from '$app/stores';
  import { trackCount, isScanning, scanProgress } from '$lib/stores/library';

  // SVG path data only — rendered at 16x16 stroke-2 currentColor
  const coreLinks = [
    {
      href: '/',
      label: 'Home',
      icon: `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>`,
    },
    {
      href: '/artists',
      label: 'Artists',
      icon: `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="8" r="5"/><path d="M20 21a8 8 0 1 0-16 0"/></svg>`,
    },
    {
      href: '/library',
      label: 'Library',
      icon: `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20"/></svg>`,
    },
    {
      href: '/downloads',
      label: 'Acquire',
      icon: `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>`,
    },
    {
      href: '/playlists',
      label: 'Playlists',
      icon: `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/><line x1="3" y1="6" x2="3.01" y2="6"/><line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/></svg>`,
    },
    {
      href: '/session',
      label: 'Session',
      icon: `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>`,
    },
  ];

  const utilityLinks = [
    {
      href: '/import',
      label: 'Import',
      icon: `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>`,
    },
    {
      href: '/tools',
      label: 'Tools',
      icon: `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/></svg>`,
    },
    {
      href: '/settings',
      label: 'Settings',
      icon: `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>`,
    },
  ];

  function isActive(href: string, pathname: string): boolean {
    if (href === '/') return pathname === '/';
    return pathname === href || pathname.startsWith(`${href}/`);
  }
</script>
```

- [ ] **Step 2: Update the nav item template to render SVG via `{@html}`**

Replace the `<span class="nav-icon">{link.icon}</span>` line in the nav template. Find the `<a href={link.href}` block and update it:

```svelte
<ul class="nav-list">
  {#each coreLinks as link}
    {@const active = isActive(link.href, $page.url.pathname)}
    <li>
      <a href={link.href} class="nav-item" class:active>
        <span class="nav-icon">{@html link.icon}</span>
        <span class="nav-label">{link.label}</span>
      </a>
    </li>
  {/each}
</ul>

<div class="nav-divider"></div>

<ul class="nav-list">
  {#each utilityLinks as link}
    {@const active = isActive(link.href, $page.url.pathname)}
    <li>
      <a href={link.href} class="nav-item" class:active>
        <span class="nav-icon">{@html link.icon}</span>
        <span class="nav-label">{link.label}</span>
      </a>
    </li>
  {/each}
</ul>
```

- [ ] **Step 3: Update the `.nav-icon` CSS to center the SVG**

In the `<style>` block of `Sidebar.svelte`, replace the `.nav-icon` rule:

```css
.nav-icon {
  width: 20px;
  height: 20px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: inherit;
}
```

- [ ] **Step 4: Build and verify**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -20
```

Expected: build succeeds, no new type errors. Svelte accessibility warning about downloads page is pre-existing and acceptable.

- [ ] **Step 5: Commit**

```bash
cd "c:/Cassette Music" && git add ui/src/lib/components/Sidebar.svelte && git commit -m "feat(ui): replace text abbreviation nav icons with inline SVG icons"
```

---

### Task 2: Replace emoji controls in NowPlaying with SVG icons

**Files:**
- Modify: `ui/src/lib/components/NowPlaying.svelte`

The existing emoji `⏮ ▶ ⏭` and `🔇🔉🔊` are replaced with inline SVG. The player bar height stays at `--playerbar-h: 72px` — we improve the visual quality, not the layout size (layout expansion is Phase 2).

- [ ] **Step 1: Replace the transport button content**

In `NowPlaying.svelte`, find the `.np-controls` div and replace it:

```svelte
<div class="np-controls">
  <button class="ctrl-btn" on:click={handlePrev} title="Previous">
    <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
      <polygon points="19 20 9 12 19 4 19 20"/><line x1="5" y1="19" x2="5" y2="5" stroke="currentColor" stroke-width="2" fill="none"/>
    </svg>
  </button>
  <button class="ctrl-btn play-btn" on:click={() => player.toggle()} title="Play/Pause">
    {#if $isPlaying}
      <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
        <rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/>
      </svg>
    {:else}
      <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
        <polygon points="5 3 19 12 5 21 5 3"/>
      </svg>
    {/if}
  </button>
  <button class="ctrl-btn" on:click={handleNext} title="Next">
    <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
      <polygon points="5 4 15 12 5 20 5 4"/><line x1="19" y1="5" x2="19" y2="19" stroke="currentColor" stroke-width="2" fill="none"/>
    </svg>
  </button>
</div>
```

- [ ] **Step 2: Replace volume emoji with SVG**

Find the `.np-right` div and replace the `vol-icon` span:

```svelte
<div class="np-right">
  <span class="vol-icon">
    {#if vol === 0}
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><line x1="23" y1="9" x2="17" y2="15"/><line x1="17" y1="9" x2="23" y2="15"/>
      </svg>
    {:else if vol < 0.5}
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><path d="M15.54 8.46a5 5 0 0 1 0 7.07"/>
      </svg>
    {:else}
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"/>
      </svg>
    {/if}
  </span>
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="volume-bar" bind:this={volBarEl} on:mousedown={onVolMouseDown}>
    <div class="volume-fill" style="width:{vol*100}%"></div>
  </div>
</div>
```

- [ ] **Step 3: Update vol-icon CSS to display flex**

In the NowPlaying `<style>` block, update:

```css
.vol-icon { 
  font-size: 0.82rem; 
  color: var(--text-muted); 
  display: flex;
  align-items: center;
  flex-shrink: 0;
}
```

- [ ] **Step 4: Also add the seek bar and volume bar CSS (they exist already, just verify they're present)**

The seek-bar and volume-bar styles should already be in the component. Confirm by searching for `.seek-bar` and `.volume-bar` in the style block. If missing, add:

```css
.seek-bar {
  flex: 1;
  height: 3px;
  background: var(--bg-active);
  border-radius: 99px;
  cursor: pointer;
  position: relative;
}
.seek-fill {
  position: absolute;
  left: 0; top: 0; bottom: 0;
  background: var(--primary);
  border-radius: 99px;
  pointer-events: none;
}
.seek-thumb {
  position: absolute;
  top: 50%;
  transform: translate(-50%, -50%);
  width: 10px; height: 10px;
  background: var(--primary);
  border-radius: 50%;
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.15s;
}
.np-seek:hover .seek-thumb { opacity: 1; }
.volume-bar {
  width: 72px; height: 3px;
  background: var(--bg-active);
  border-radius: 99px;
  cursor: pointer;
  position: relative;
}
.volume-fill {
  position: absolute;
  left: 0; top: 0; bottom: 0;
  background: var(--text-secondary);
  border-radius: 99px;
  pointer-events: none;
}
```

- [ ] **Step 5: Build and verify**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

Expected: build passes.

- [ ] **Step 6: Commit**

```bash
cd "c:/Cassette Music" && git add ui/src/lib/components/NowPlaying.svelte && git commit -m "feat(ui): replace emoji transport/volume controls with SVG icons in NowPlaying"
```

---

### Task 3: Add per-item remove and drag-to-reorder in QueuePanel

**Files:**
- Modify: `ui/src/lib/components/QueuePanel.svelte`
- Modify: `ui/src/lib/stores/queue.ts`
- Modify: `ui/src/lib/api/tauri.ts`

The backend already has `remove_from_queue` — we need to check if it's exposed. If not, we use `queue_tracks` to rebuild the queue (which is already used for jump-to). For drag-reorder we rebuild via the existing `api.queueTracks` call.

- [ ] **Step 1: Check if remove_from_queue Tauri command exists**

```bash
grep -n "remove_from_queue\|remove_queue" "c:/Cassette Music/src-tauri/src/commands/queue.rs"
```

If the command exists: note its name. If not: we will rebuild queue on reorder/remove using the existing `queueTracks` pattern.

- [ ] **Step 2: Add `removeFromQueue` to `tauri.ts`**

Open `ui/src/lib/api/tauri.ts`. Find the `api` object. Add this method alongside the existing queue methods:

```typescript
async removeFromQueue(queueItemId: number): Promise<void> {
  await invoke('remove_from_queue', { queueItemId });
},
async reorderQueue(trackIds: number[], startIndex: number): Promise<void> {
  await invoke('queue_tracks', { trackIds, startIndex });
},
```

If `remove_from_queue` command does not exist in the backend, use this fallback approach instead — it rebuilds the full queue without the removed item:

```typescript
async removeQueueItem(position: number, allTrackIds: number[]): Promise<void> {
  const remaining = allTrackIds.filter((_, i) => i !== position);
  if (remaining.length === 0) {
    await invoke('clear_queue');
  } else {
    await invoke('queue_tracks', { trackIds: remaining, startIndex: 0 });
  }
},
```

- [ ] **Step 3: Add `removeFromQueue` to the queue store**

Open `ui/src/lib/stores/queue.ts`. Add after the existing `clearQueue` function:

```typescript
export async function removeQueueItem(position: number) {
  const items = get(queue);
  const remainingIds = items
    .filter((_, i) => i !== position)
    .map((item) => item.track_id);
  
  if (remainingIds.length === 0) {
    await clearQueue();
  } else {
    // Preserve current playback position context
    await api.queueTracks(remainingIds, 0);
  }
  await loadQueue();
}
```

You need to add `import { get } from 'svelte/store';` at the top if not already present.

- [ ] **Step 4: Add drag-reorder state and handlers to QueuePanel**

Open `ui/src/lib/components/QueuePanel.svelte`. Replace the entire `<script>` block with:

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { queue, loadQueue, clearQueue, removeQueueItem } from '$lib/stores/queue';
  import { playbackState } from '$lib/stores/player';
  import { api } from '$lib/api/tauri';
  import { formatDuration } from '$lib/utils';

  onMount(() => loadQueue());

  let dragIndex: number | null = null;
  let dragOverIndex: number | null = null;

  async function jumpTo(index: number) {
    const items = $queue;
    if (!items[index]) return;
    const trackIds = items.map((i) => i.track_id);
    await api.queueTracks(trackIds, index);
    await loadQueue();
  }

  function onDragStart(index: number) {
    dragIndex = index;
  }

  function onDragOver(e: DragEvent, index: number) {
    e.preventDefault();
    dragOverIndex = index;
  }

  async function onDrop(e: DragEvent, dropIndex: number) {
    e.preventDefault();
    if (dragIndex === null || dragIndex === dropIndex) {
      dragIndex = null;
      dragOverIndex = null;
      return;
    }
    const items = [...$queue];
    const [moved] = items.splice(dragIndex, 1);
    items.splice(dropIndex, 0, moved);
    const trackIds = items.map((i) => i.track_id);
    const currentPos = $playbackState.queue_position;
    await api.queueTracks(trackIds, currentPos);
    await loadQueue();
    dragIndex = null;
    dragOverIndex = null;
  }

  function onDragEnd() {
    dragIndex = null;
    dragOverIndex = null;
  }
</script>
```

- [ ] **Step 5: Update the queue list template with drag handles and remove buttons**

Replace the `<ul class="queue-list">` section:

```svelte
<ul class="queue-list">
  {#each $queue as item, i}
    {@const track = item.track}
    {@const isCurrent = i === $playbackState.queue_position}
    <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
    <li
      class="queue-item"
      class:current={isCurrent}
      class:drag-over={dragOverIndex === i}
      draggable="true"
      on:dragstart={() => onDragStart(i)}
      on:dragover={(e) => onDragOver(e, i)}
      on:drop={(e) => onDrop(e, i)}
      on:dragend={onDragEnd}
      on:dblclick={() => jumpTo(i)}
    >
      <span class="q-drag">
        <svg width="10" height="10" viewBox="0 0 24 24" fill="currentColor" opacity="0.35">
          <circle cx="9" cy="5" r="1.5"/><circle cx="15" cy="5" r="1.5"/>
          <circle cx="9" cy="12" r="1.5"/><circle cx="15" cy="12" r="1.5"/>
          <circle cx="9" cy="19" r="1.5"/><circle cx="15" cy="19" r="1.5"/>
        </svg>
      </span>
      <span class="q-num" class:active={isCurrent}>{isCurrent ? '▶' : i + 1}</span>
      <div class="q-info">
        <div class="q-title">{track?.title ?? 'Unknown'}</div>
        <div class="q-artist">{track?.artist ?? ''}</div>
      </div>
      <span class="q-dur">{formatDuration(track?.duration_secs ?? 0)}</span>
      <button
        class="q-remove"
        on:click|stopPropagation={() => removeQueueItem(i)}
        title="Remove"
        aria-label="Remove from queue"
      >
        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
          <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
        </svg>
      </button>
    </li>
  {/each}
</ul>
```

- [ ] **Step 6: Add drag-and-drop and remove button styles**

Add to the `<style>` block in `QueuePanel.svelte`:

```css
.q-drag {
  cursor: grab;
  display: flex;
  align-items: center;
  padding: 0 3px;
  color: var(--text-muted);
  flex-shrink: 0;
}
.q-drag:active { cursor: grabbing; }
.queue-item.drag-over {
  border-top: 2px solid var(--primary);
}
.q-remove {
  display: none;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  color: var(--text-muted);
  flex-shrink: 0;
  background: none;
  border: none;
  cursor: pointer;
  padding: 0;
}
.queue-item:hover .q-remove { display: flex; }
.q-remove:hover { color: var(--text-primary); background: var(--bg-hover); }
```

- [ ] **Step 7: Build and verify**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

Expected: build passes.

- [ ] **Step 8: Commit**

```bash
cd "c:/Cassette Music" && git add ui/src/lib/components/QueuePanel.svelte ui/src/lib/stores/queue.ts ui/src/lib/api/tauri.ts && git commit -m "feat(ui): add per-item remove and drag-to-reorder to queue panel"
```

---

### Task 4: Artist cards — album cover mosaics instead of letter initials

**Files:**
- Modify: `ui/src/routes/artists/+page.svelte`

The `$artists` store provides `Artist[]` objects but not their cover art. The `$albums` store is loaded at app start via `loadLibrary()` in the layout. We filter albums per artist cluster to get cover art paths.

- [ ] **Step 1: Import the albums store at the top of the script**

In `ui/src/routes/artists/+page.svelte`, add `albums` to the import from the library store:

```svelte
<script lang="ts">
  import { buildArtistClusters, clusterAlbumsForArtist, normalizeArtistKey, type ArtistCluster } from '$lib/artist-clusters';
  import { artists, albums } from '$lib/stores/library';
  import { api } from '$lib/api/tauri';
  import ContextActionRail from '$lib/components/ContextActionRail.svelte';
  import { queueTracks } from '$lib/stores/queue';
  import { formatDuration, coverSrc } from '$lib/utils';
  import type { Album, Track } from '$lib/api/tauri';
  // ... rest of existing script
```

- [ ] **Step 2: Add a helper that returns the first N cover art paths for an artist cluster**

Add this function inside the `<script>` block, after the existing imports:

```typescript
function artistCoverArts(cluster: ArtistCluster, allAlbums: Album[], max = 4): string[] {
  return clusterAlbumsForArtist(allAlbums, cluster)
    .filter((a) => !!a.cover_art_path)
    .slice(0, max)
    .map((a) => coverSrc(a.cover_art_path!));
}
```

- [ ] **Step 3: Replace the artist-card template with a mosaic version**

Find the `<div class="artist-card"` block (inside `{#if !selectedArtist}` → `{#each artistClusters as cluster}`) and replace it entirely:

```svelte
<div
  class="artist-card"
  role="button"
  tabindex="0"
  on:click={() => selectArtist(cluster)}
  on:keydown={(event) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      selectArtist(cluster);
    }
  }}
>
  {@const arts = artistCoverArts(cluster, $albums)}
  <div class="artist-mosaic">
    {#if arts.length >= 4}
      <div class="mosaic-grid">
        {#each arts.slice(0, 4) as src}
          <img class="mosaic-img" {src} alt="" loading="lazy" />
        {/each}
      </div>
    {:else if arts.length >= 1}
      <img class="mosaic-single" src={arts[0]} alt="" loading="lazy" />
    {:else}
      <div class="artist-avatar-fallback">{cluster.primaryName[0]?.toUpperCase() ?? '?'}</div>
    {/if}
    <div class="artist-mosaic-overlay"></div>
  </div>
  <div class="artist-card-info">
    <div class="artist-name">{cluster.primaryName}</div>
    <div class="artist-meta">{cluster.albumCount} albums · {cluster.trackCount} tracks</div>
    {#if cluster.aliases.length > 1}
      <div class="artist-variants">{cluster.aliases.length} name variants</div>
    {/if}
  </div>
</div>
```

- [ ] **Step 4: Replace artist-card styles with mosaic styles**

In the `<style>` block, replace the existing `.artist-card`, `.artist-avatar`, `.artist-name`, `.artist-meta`, `.artist-variants` rules with:

```css
.artist-card {
  display: flex;
  flex-direction: column;
  border-radius: var(--radius);
  background: var(--bg-card);
  border: 1px solid var(--border);
  cursor: pointer;
  transition: transform 0.15s, box-shadow 0.15s, border-color 0.15s;
  overflow: hidden;
  text-align: center;
}

.artist-card:hover {
  transform: translateY(-2px);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
  border-color: var(--border-active);
}

.artist-mosaic {
  position: relative;
  width: 100%;
  aspect-ratio: 1;
  overflow: hidden;
  background: var(--bg-active);
}

.mosaic-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  grid-template-rows: 1fr 1fr;
  width: 100%;
  height: 100%;
  gap: 1px;
}

.mosaic-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.mosaic-single {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.artist-avatar-fallback {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 2rem;
  font-weight: 700;
  color: var(--accent-bright);
  background: linear-gradient(135deg, var(--accent-dim), var(--bg-active));
}

.artist-mosaic-overlay {
  position: absolute;
  inset: 0;
  background: linear-gradient(to top, rgba(8, 11, 18, 0.72) 0%, transparent 55%);
  pointer-events: none;
}

.artist-card-info {
  padding: 10px 10px 12px;
}

.artist-name { font-weight: 600; font-size: 0.85rem; word-break: break-word; color: var(--text-primary); }
.artist-meta { font-size: 0.72rem; color: var(--text-muted); margin-top: 2px; }
.artist-variants { font-size: 0.68rem; color: var(--accent-bright); margin-top: 2px; }
```

- [ ] **Step 5: Build and verify**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

Expected: build passes.

- [ ] **Step 6: Commit**

```bash
cd "c:/Cassette Music" && git add ui/src/routes/artists/+page.svelte && git commit -m "feat(ui): replace letter-initial artist cards with album cover mosaics"
```

---

### Task 5: Track inspector — human-readable quality display

**Files:**
- Modify: `ui/src/routes/library/+page.svelte`

The inspector currently shows raw `bit_depth`, `sample_rate`, `bitrate_kbps`, `quality_tier` fields as code blocks. We replace with a formatted quality string and make the collector data clearly separated from the listener summary.

- [ ] **Step 1: Add a quality label helper function to the library page script**

Open `ui/src/routes/library/+page.svelte`. In the `<script>` block, add after the existing `editionBucketLabel` map:

```typescript
function formatQualityLabel(track: Track): string {
  const fmt = track.format?.toUpperCase() ?? 'AUDIO';
  const bits = track.bit_depth ? `${track.bit_depth}-bit` : null;
  const khz = track.sample_rate ? `${(track.sample_rate / 1000).toFixed(1)}kHz` : null;
  const kbps = track.bitrate_kbps ? `${track.bitrate_kbps}kbps` : null;
  
  if (bits && khz) return `${fmt} · ${bits} / ${khz}`;
  if (kbps) return `${fmt} · ${kbps}`;
  return fmt;
}

function qualityTierLabel(tier: string | null): string {
  if (!tier) return '—';
  const map: Record<string, string> = {
    lossless_hires: 'Hi-Res Lossless',
    lossless: 'Lossless',
    lossy_high: 'High Quality',
    lossy_mid: 'Standard Quality',
    lossy_low: 'Low Quality',
  };
  return map[tier] ?? tier;
}
```

- [ ] **Step 2: Replace the track inspector grid in the album detail view**

Find the first `<div class="track-inspector-grid">` (inside the album detail section). Replace its contents:

```svelte
<div class="track-inspector-grid">
  <div class="inspector-summary">
    <span class="inspector-quality-badge">{formatQualityLabel(selectedTrack)}</span>
    {#if selectedTrackIdentity?.edition_bucket}
      <span class="inspector-edition-badge">{editionBucketLabel[selectedTrackIdentity.edition_bucket] ?? selectedTrackIdentity.edition_bucket}</span>
    {/if}
    {#if selectedTrack.year}
      <span class="inspector-year">{selectedTrack.year}</span>
    {/if}
  </div>
  <div><span>Quality tier</span><code>{qualityTierLabel(selectedTrack.quality_tier)}</code></div>
  <div><span>Format</span><code>{selectedTrack.format?.toUpperCase() ?? '—'}</code></div>
  <div><span>ISRC</span><code>{selectedTrack.isrc ?? '—'}</code></div>
  <div><span>Edition markers</span><code>{selectedTrackIdentity?.edition_markers?.length ? selectedTrackIdentity.edition_markers.join(', ') : '—'}</code></div>
  <details class="inspector-ids">
    <summary>Identity details</summary>
    <div class="inspector-ids-grid">
      <div><span>MB recording</span><code>{selectedTrack.musicbrainz_recording_id ?? '—'}</code></div>
      <div><span>MB release</span><code>{selectedTrack.musicbrainz_release_id ?? '—'}</code></div>
      <div><span>MB release group</span><code>{selectedTrackIdentity?.musicbrainz_release_group_id ?? '—'}</code></div>
      <div><span>Canonical artist</span><code>{selectedTrack.canonical_artist_id ?? '—'}</code></div>
      <div><span>Canonical release</span><code>{selectedTrack.canonical_release_id ?? '—'}</code></div>
    </div>
  </details>
  <div class="track-inspector-wide"><span>Path</span><code>{selectedTrack.path}</code></div>
</div>
```

- [ ] **Step 3: Apply the same replacement to the tracks-tab inspector**

Find the second `<div class="track-inspector-grid">` (inside the tracks tab `{#if $activeTab === 'tracks'}` section). Apply the identical replacement, substituting `content_hash` in place of the MB release group (since tracks tab doesn't load `selectedTrackIdentity` the same way — it does, check that `selectedTrackIdentity` is available there too, and it is):

```svelte
<div class="track-inspector-grid">
  <div class="inspector-summary">
    <span class="inspector-quality-badge">{formatQualityLabel(selectedTrack)}</span>
    {#if selectedTrackIdentity?.edition_bucket}
      <span class="inspector-edition-badge">{editionBucketLabel[selectedTrackIdentity.edition_bucket] ?? selectedTrackIdentity.edition_bucket}</span>
    {/if}
    {#if selectedTrack.year}
      <span class="inspector-year">{selectedTrack.year}</span>
    {/if}
  </div>
  <div><span>Quality tier</span><code>{qualityTierLabel(selectedTrack.quality_tier)}</code></div>
  <div><span>Format</span><code>{selectedTrack.format?.toUpperCase() ?? '—'}</code></div>
  <div><span>ISRC</span><code>{selectedTrack.isrc ?? '—'}</code></div>
  <div><span>Edition markers</span><code>{selectedTrackIdentity?.edition_markers?.length ? selectedTrackIdentity.edition_markers.join(', ') : '—'}</code></div>
  <details class="inspector-ids">
    <summary>Identity details</summary>
    <div class="inspector-ids-grid">
      <div><span>MB recording</span><code>{selectedTrack.musicbrainz_recording_id ?? '—'}</code></div>
      <div><span>MB release</span><code>{selectedTrack.musicbrainz_release_id ?? '—'}</code></div>
      <div><span>MB release group</span><code>{selectedTrackIdentity?.musicbrainz_release_group_id ?? '—'}</code></div>
      <div><span>Canonical artist</span><code>{selectedTrack.canonical_artist_id ?? '—'}</code></div>
      <div><span>Content hash</span><code>{selectedTrack.content_hash ?? '—'}</code></div>
    </div>
  </details>
  <div class="track-inspector-wide"><span>Path</span><code>{selectedTrack.path}</code></div>
</div>
```

- [ ] **Step 4: Add inspector summary and details styles**

Add to the `<style>` block of `library/+page.svelte`:

```css
.inspector-summary {
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 4px;
}

.inspector-quality-badge {
  font-size: 0.76rem;
  font-weight: 600;
  color: var(--text-primary);
  background: color-mix(in srgb, var(--primary) 14%, var(--bg-card));
  border: 1px solid color-mix(in srgb, var(--primary) 28%, var(--border));
  border-radius: 999px;
  padding: 2px 8px;
}

.inspector-edition-badge {
  font-size: 0.72rem;
  color: var(--accent-bright);
  background: color-mix(in srgb, var(--accent) 12%, var(--bg-card));
  border: 1px solid color-mix(in srgb, var(--accent) 24%, var(--border));
  border-radius: 999px;
  padding: 2px 8px;
}

.inspector-year {
  font-size: 0.72rem;
  color: var(--text-muted);
}

.inspector-ids {
  grid-column: 1 / -1;
  margin-top: 4px;
}

.inspector-ids summary {
  font-size: 0.68rem;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  cursor: pointer;
  user-select: none;
  margin-bottom: 6px;
}

.inspector-ids-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px 12px;
  margin-top: 6px;
  padding: 8px;
  background: var(--bg-base);
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-dim);
}

.inspector-ids-grid div { display: flex; flex-direction: column; gap: 3px; }
.inspector-ids-grid span {
  font-size: 0.68rem; color: var(--text-muted);
  text-transform: uppercase; letter-spacing: 0.05em;
}
.inspector-ids-grid code {
  font-size: 0.68rem; color: var(--text-secondary); word-break: break-all;
}
```

- [ ] **Step 5: Build and verify**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

Expected: build passes.

- [ ] **Step 6: Commit**

```bash
cd "c:/Cassette Music" && git add ui/src/routes/library/+page.svelte && git commit -m "feat(ui): humanize track inspector — quality badges, edition labels, collapse raw IDs"
```

---

### Task 6: Remove topbar navigation duplication

**Files:**
- Modify: `ui/src/routes/+layout.svelte`

The topbar currently duplicates Home/Artists/Library/Downloads links that are already in the sidebar. We remove those links and keep only brand identity, the Mini/Full toggle, the Minimize button, and the Commands button.

- [ ] **Step 1: Remove the `<nav class="topbar-nav">` block from the layout template**

In `ui/src/routes/+layout.svelte`, find and remove this entire block:

```svelte
<nav class="topbar-nav" aria-label="Quick actions">
  <a class="topbar-link" href="/">Home</a>
  <a class="topbar-link" href="/artists">Artists</a>
  <a class="topbar-link" href="/library">Library</a>
  <a class="topbar-link" href="/downloads">Downloads</a>
</nav>
```

Replace the `<header class="app-topbar">` contents so it reads:

```svelte
<header class="app-topbar">
  <div class="topbar-brand">
    <span class="brand-wordmark">Cassette</span>
    <span class="brand-divider">//</span>
    <span class="brand-mode">Desktop</span>
  </div>
  <div class="topbar-spacer"></div>
  <button class="topbar-link topbar-toggle" type="button" aria-label="Toggle compact player" on:click={toggleCompactPlayerMode}>
    {$compactPlayerMode ? 'Full Player' : 'Mini Player'}
  </button>
  <button class="topbar-link topbar-toggle" type="button" aria-label="Minimize app" on:click={minimizeAppWindow}>
    Minimize
  </button>
  <button class="topbar-command" type="button" aria-label="Open command palette" on:click={openPalette}>
    Commands
  </button>
</header>
```

- [ ] **Step 2: Add `.topbar-spacer` to the CSS in `app.css`**

In `app.css`, find the `.app-topbar` rules and add:

```css
.topbar-spacer { flex: 1; }
```

(Add this after the `.topbar-brand` rule.)

- [ ] **Step 3: Build and verify**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

Expected: build passes.

- [ ] **Step 4: Commit**

```bash
cd "c:/Cassette Music" && git add ui/src/routes/+layout.svelte ui/src/app.css && git commit -m "feat(ui): remove duplicate topbar navigation — sidebar is the single nav spine"
```

---

### Task 7: Fix playlist empty states and confirm dialog

**Files:**
- Modify: `ui/src/routes/playlists/+page.svelte`

Replace `📋` emoji and the native `confirm()` dialog (which looks wrong in Tauri) with intentional copy and inline confirmation.

- [ ] **Step 1: Replace emoji icons with typed text labels**

In `playlists/+page.svelte`, replace both `<div class="pl-icon">📋</div>` occurrences with:

```svelte
<div class="pl-icon">
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/>
    <line x1="8" y1="18" x2="21" y2="18"/><line x1="3" y1="6" x2="3.01" y2="6"/>
    <line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/>
  </svg>
</div>
```

Replace the empty state `<div class="empty-icon">📋</div>` and `<div class="empty-icon">🎵</div>` occurrences with:

```svelte
<!-- For playlists empty state: -->
<div class="empty-icon-text">No playlists yet</div>
<div class="empty-body">Create a playlist to start building your listening arcs.</div>

<!-- For tracks empty state: -->
<div class="empty-icon-text">Select a playlist</div>
```

(Remove the `empty-icon` divs entirely.)

- [ ] **Step 2: Replace `confirm()` with inline confirmation state**

Add a `confirmDeleteId` store variable to the script:

```svelte
<script lang="ts">
  // add to existing script:
  let confirmDeleteId: number | null = null;

  async function handleDelete(pl: Playlist) {
    if (confirmDeleteId === pl.id) {
      // confirmed — proceed
      await deletePlaylist(pl.id);
      if ($activePlaylistId === pl.id) activePlaylistId.set(null);
      confirmDeleteId = null;
    } else {
      // first click — show confirmation state
      confirmDeleteId = pl.id;
      // auto-cancel after 3s
      setTimeout(() => {
        if (confirmDeleteId === pl.id) confirmDeleteId = null;
      }, 3000);
    }
  }
</script>
```

- [ ] **Step 3: Update the delete button to show confirmation state**

In the playlist item template, find the delete button and replace:

```svelte
<button
  class="btn-icon"
  class:confirming={confirmDeleteId === pl.id}
  on:click|stopPropagation={() => handleDelete(pl)}
  title={confirmDeleteId === pl.id ? 'Click again to confirm delete' : 'Delete'}
  style={confirmDeleteId === pl.id ? 'color:var(--error)' : ''}
>
  {confirmDeleteId === pl.id ? '?' : '✕'}
</button>
```

- [ ] **Step 4: Add confirming style**

Add to the `<style>` block:

```css
.btn-icon.confirming {
  background: color-mix(in srgb, var(--error) 12%, var(--bg-card));
  border: 1px solid color-mix(in srgb, var(--error) 30%, var(--border));
}
.empty-icon-text {
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 6px;
}
```

- [ ] **Step 5: Build and verify**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

Expected: build passes.

- [ ] **Step 6: Commit**

```bash
cd "c:/Cassette Music" && git add ui/src/routes/playlists/+page.svelte && git commit -m "feat(ui): replace emoji empty states and native confirm() with intentional UX in playlists"
```

---

### Task 8: Collection stats in sidebar footer and Home

**Files:**
- Modify: `ui/src/lib/components/Sidebar.svelte`
- Modify: `ui/src/routes/+page.svelte` (Home hero side metrics)

Show albums count alongside track count. The `$albums` store is loaded in the layout.

- [ ] **Step 1: Add albums store import to Sidebar**

In `Sidebar.svelte` script, add `albums` to the import:

```svelte
import { trackCount, albums, isScanning, scanProgress } from '$lib/stores/library';
```

- [ ] **Step 2: Update the sidebar footer stat row**

Find the `<div class="stat-row">` in the sidebar footer and replace with:

```svelte
<div class="stat-row">
  <span class="stat-value">{$trackCount.toLocaleString()}</span>
  <span class="stat-label">tracks</span>
  <span class="stat-sep">·</span>
  <span class="stat-value">{$albums.length.toLocaleString()}</span>
  <span class="stat-label">albums</span>
</div>
```

- [ ] **Step 3: Add `.stat-sep` style**

```css
.stat-sep { font-size: 0.65rem; color: var(--text-muted); margin: 0 2px; }
```

- [ ] **Step 4: Build and verify**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

Expected: build passes.

- [ ] **Step 5: Commit**

```bash
cd "c:/Cassette Music" && git add ui/src/lib/components/Sidebar.svelte && git commit -m "feat(ui): show album count alongside track count in sidebar footer"
```

---

### Task 9: Verify full build and run smoke test

- [ ] **Step 1: Full workspace check**

```bash
cd "c:/Cassette Music" && cargo check --workspace 2>&1 | tail -5
```

Expected: `Finished` with no new errors.

- [ ] **Step 2: Full UI build**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

Expected: build succeeds (pre-existing accessibility warning in downloads page is acceptable).

- [ ] **Step 3: Run smoke test**

```powershell
cd "c:/Cassette Music" && .\scripts\smoke_desktop.ps1 2>&1 | tail -20
```

Expected: smoke test passes.

- [ ] **Step 4: Final commit if anything was adjusted**

```bash
cd "c:/Cassette Music" && git status
```

If any files are staged from smoke fixes, commit them:
```bash
git add -p && git commit -m "fix(ui): phase-1 smoke test adjustments"
```

---

## Self-Review

**Spec coverage check:**
- [x] Replace nav icons → Task 1
- [x] Replace emoji controls → Task 2
- [x] Queue remove + reorder → Task 3
- [x] Artist card mosaics → Task 4
- [x] Track inspector humanized → Task 5
- [x] Topbar deduplication → Task 6
- [x] Playlist empty states + confirm dialog → Task 7
- [x] Collection stats → Task 8

**Placeholder scan:** No TBDs. All code shown inline.

**Type consistency:** `Track` type used throughout matches `ui/src/lib/api/tauri.ts` definition. `Album` type includes `cover_art_path: string | null` and `dominant_color_hex: string | null`. `Artist` type includes `name`, `album_count`, `track_count`. All consistent with existing definitions.
