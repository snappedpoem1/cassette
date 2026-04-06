# UI Redesign — Steel Dusk Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign every surface of the Cassette desktop UI to the Steel Dusk design: slate-blue primary accent, Inter font, editorial album grid with per-card color tinting, blurred-backdrop album detail view, and a bespoke settings layout.

**Architecture:** CSS design tokens are consolidated in `app.css`; all components reference tokens only. The backend gains `dominant_color_hex` on `Album` via a DB migration + scanner extraction using `lofty`'s embedded picture API + a lightweight pixel average. Frontend gains a `tintFromHex` utility and per-card tinting in the album grid. The album detail view adds a blurred backdrop behind the existing tracklist. Settings gets a sub-nav layout. All other pages inherit the new chrome automatically.

**Tech Stack:** Svelte 5 / SvelteKit, TypeScript, Rust (lofty, rusqlite), Tauri 2, Inter (Google Fonts or self-hosted)

---

## File Map

| File | Action | What changes |
|---|---|---|
| `ui/src/app.css` | Modify | Replace all tokens, add Inter import, update all shared component classes |
| `ui/src/routes/+layout.svelte` | Modify | Topbar markup + classes to match new chrome |
| `ui/src/lib/components/Sidebar.svelte` | Modify | Logo block, nav item markup/styles |
| `ui/src/lib/components/NowPlaying.svelte` | Modify | Three-column grid layout, gradient seek bar, primary play button |
| `ui/src/lib/components/RightSidebar.svelte` | Modify | Tab styles, info panel layout |
| `ui/src/lib/components/QueuePanel.svelte` | Modify | Queue item styles |
| `ui/src/routes/+page.svelte` | Modify | Album grid cards with tinting, album detail blurred backdrop |
| `ui/src/routes/settings/+page.svelte` | Modify | Sub-nav layout, provider status cards, field grid |
| `ui/src/lib/utils.ts` | Modify | Add `tintFromHex` |
| `ui/src/lib/api/tauri.ts` | Modify | Add `dominant_color_hex` to `Album` interface |
| `crates/cassette-core/src/models/mod.rs` | Modify | Add `dominant_color_hex: Option<String>` to `Album` |
| `crates/cassette-core/src/db/mod.rs` | Modify | Migration to add column, update `get_albums` query, add `extract_dominant_color` fn |
| `crates/cassette-core/src/library/mod.rs` | Modify | Call `extract_dominant_color` from `find_cover_art` result during scan |
| `crates/cassette-core/Cargo.toml` | Modify | Add `image = { version = "0.25", default-features = false, features = ["jpeg", "png"] }` |

---

## Task 1: Add Inter font and replace CSS design tokens

**Files:**
- Modify: `ui/src/app.css`

- [ ] **Step 1: Replace the `:root` block and font import**

Open `ui/src/app.css`. Replace the entire file content with the following. The existing token names (`--accent`, `--text-primary`, etc.) are kept as aliases so existing components don't break until they're updated in later tasks.

```css
/* ── Font ──────────────────────────────────────────────────────────────────── */
@import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&display=swap');

/* ── Design tokens ─────────────────────────────────────────────────────────── */
:root {
  /* Steel Dusk palette */
  --bg-deep:        #060810;
  --bg-base:        #080b12;
  --bg-card:        #0c1018;
  --bg-hover:       #0f1420;
  --bg-active:      rgba(139, 180, 212, 0.08);
  --bg-overlay:     rgba(6, 8, 16, 0.86);

  --primary:        #8bb4d4;
  --primary-dim:    rgba(139, 180, 212, 0.5);

  /* amber kept as secondary accent only */
  --accent:         #f7b45c;
  --accent-bright:  #f7b45c;
  --accent-dim:     rgba(247, 180, 92, 0.15);
  --accent-glow:    rgba(247, 180, 92, 0.2);

  --text-primary:   #c4d4e4;
  --text-secondary: #4a6070;
  --text-muted:     #253040;
  --text-accent:    #8bb4d4;

  --border:         rgba(139, 180, 212, 0.07);
  --border-dim:     rgba(255, 255, 255, 0.04);
  --border-active:  rgba(139, 180, 212, 0.4);

  --status-ok:      #5ec4a0;
  --success:        #22c55e;
  --warning:        #f59e0b;
  --error:          #ef4444;

  --sidebar-w:       88px;
  --right-sidebar-w: 190px;
  --topbar-h:        40px;
  --nowplaying-h:    72px;
  --radius-sm:       5px;
  --radius:          7px;
  --radius-lg:       12px;

  /* Legacy aliases kept so unmodified components still compile */
  --bg-elevated: #0c1018;

  font-family: 'Inter', system-ui, -apple-system, sans-serif;
  font-size: 14px;
  line-height: 1.5;
  color-scheme: dark;
}

/* ── Reset ─────────────────────────────────────────────────────────────────── */
*, *::before, *::after { box-sizing: border-box; }

html, body {
  margin: 0; padding: 0;
  height: 100%;
  overflow: hidden;
  background: var(--bg-deep);
  color: var(--text-primary);
  -webkit-font-smoothing: antialiased;
}

a { color: inherit; text-decoration: none; }
button { cursor: pointer; border: none; background: none; color: inherit; font: inherit; }
input, textarea { font: inherit; color: inherit; background: transparent; border: none; outline: none; }

/* ── Scrollbars ────────────────────────────────────────────────────────────── */
::-webkit-scrollbar { width: 5px; height: 5px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: var(--text-muted); border-radius: 99px; }
::-webkit-scrollbar-thumb:hover { background: var(--text-secondary); }

/* ── Typography ────────────────────────────────────────────────────────────── */
h1, h2, h3, h4 { margin: 0; font-weight: 700; }
h1 { font-size: 1.5rem; }
h2 { font-size: 1.1rem; }
h3 { font-size: 0.95rem; }
p  { margin: 0; }

/* ── App shell ─────────────────────────────────────────────────────────────── */
.app-shell {
  display: grid;
  grid-template-columns: var(--sidebar-w) 1fr var(--right-sidebar-w);
  grid-template-rows: var(--topbar-h) 1fr var(--nowplaying-h);
  height: 100vh;
  overflow: hidden;
}

.app-topbar {
  grid-column: 1 / 4;
  grid-row: 1;
  display: flex;
  align-items: center;
  gap: 0;
  padding: 0 14px;
  border-bottom: 1px solid var(--border);
  background: var(--bg-deep);
}

.topbar-brand {
  display: flex;
  align-items: baseline;
  gap: 5px;
  margin-right: 20px;
}

.brand-wordmark {
  font-size: 0.72rem;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  font-weight: 800;
  color: var(--primary);
}

.brand-divider {
  color: var(--primary);
  opacity: 0.35;
  font-weight: 800;
}

.brand-mode {
  color: var(--primary);
  opacity: 0.35;
  font-size: 0.72rem;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  font-weight: 800;
}

.topbar-nav {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  flex: 1;
}

.topbar-link {
  color: var(--text-muted);
  font-size: 0.72rem;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  font-weight: 600;
  padding: 4px 8px;
  border-radius: var(--radius-sm);
  border: 1px solid transparent;
  transition: border-color 0.15s, color 0.15s, background 0.15s;
}

.topbar-link:hover {
  color: var(--text-secondary);
  background: rgba(255, 255, 255, 0.03);
  border-color: var(--border-dim);
}

.topbar-toggle {
  background: transparent;
}

.topbar-command {
  border: 1px solid var(--border);
  background: rgba(139, 180, 212, 0.06);
  color: var(--primary);
  border-radius: var(--radius-sm);
  padding: 4px 10px;
  font-size: 0.72rem;
  text-transform: uppercase;
  letter-spacing: 0.09em;
  font-weight: 700;
}

.topbar-command:hover {
  border-color: var(--primary-dim);
  background: rgba(139, 180, 212, 0.1);
}

.app-sidebar {
  grid-column: 1;
  grid-row: 2 / 4;
  overflow-y: auto;
  border-right: 1px solid var(--border-dim);
  background: var(--bg-deep);
}

.app-main {
  grid-column: 2;
  grid-row: 2;
  overflow-y: auto;
  background: var(--bg-base);
}

.app-right {
  grid-column: 3;
  grid-row: 2;
  overflow-y: auto;
  border-left: 1px solid var(--border-dim);
  background: var(--bg-deep);
}

.app-nowplaying {
  grid-column: 1 / 4;
  grid-row: 3;
  border-top: 1px solid var(--border-dim);
  background: var(--bg-deep);
}

/* compact player mode */
.app-shell.compact-player {
  --nowplaying-h: 50px;
}
.app-shell.compact-player .app-right { opacity: 0.7; }
.app-shell.compact-player .np-art,
.app-shell.compact-player .np-info { display: none; }
.app-shell.compact-player .np-center { max-width: 560px; }
.app-shell.compact-player .np-controls { gap: 6px; }
.app-shell.compact-player .play-btn { width: 28px; height: 28px; }
.app-shell.compact-player .np-right { max-width: 160px; }

:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 2px;
}

@media (max-width: 1180px) {
  :root { --right-sidebar-w: 0px; }
  .app-right { display: none; }
}

/* ── Cards ─────────────────────────────────────────────────────────────────── */
.card {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 1rem;
}

/* ── Buttons ───────────────────────────────────────────────────────────────── */
.btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 12px;
  border-radius: var(--radius-sm);
  font-size: 0.8rem;
  font-weight: 600;
  transition: background 0.15s, color 0.15s;
}
.btn-primary { background: var(--primary); color: var(--bg-deep); }
.btn-primary:hover { background: #a0c8e8; }
.btn-ghost { background: transparent; color: var(--text-secondary); border: 1px solid var(--border); }
.btn-ghost:hover { background: var(--bg-hover); color: var(--text-primary); }
.btn-icon {
  display: inline-flex; align-items: center; justify-content: center;
  width: 28px; height: 28px; border-radius: 50%;
  color: var(--text-secondary);
  transition: background 0.15s, color 0.15s;
}
.btn-icon:hover { background: var(--bg-hover); color: var(--text-primary); }
.btn-icon.active { color: var(--primary); }

/* ── Inputs ────────────────────────────────────────────────────────────────── */
.input {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  padding: 6px 10px;
  color: var(--text-primary);
  font-size: 0.85rem;
  width: 100%;
  transition: border-color 0.15s;
}
.input:focus { border-color: var(--primary-dim); outline: none; }
.input::placeholder { color: var(--text-muted); }

/* ── Tabs ──────────────────────────────────────────────────────────────────── */
.tabs {
  display: flex;
  gap: 0;
  border-bottom: 1px solid var(--border-dim);
  padding: 0 12px;
}
.tab {
  padding: 8px 12px;
  font-size: 0.8rem;
  font-weight: 600;
  color: var(--text-muted);
  border-bottom: 2px solid transparent;
  cursor: pointer;
  transition: color 0.15s;
  margin-bottom: -1px;
  background: none; border-top: none; border-left: none; border-right: none;
}
.tab:hover { color: var(--text-secondary); }
.tab.active { color: var(--primary); border-bottom-color: var(--primary); }

/* ── Track row ─────────────────────────────────────────────────────────────── */
.track-row {
  display: grid;
  grid-template-columns: 32px 1fr 1fr 70px 52px;
  align-items: center;
  gap: 10px;
  padding: 5px 10px;
  border-radius: var(--radius-sm);
  border: 1px solid transparent;
  cursor: pointer;
  transition: background 0.1s, border-color 0.1s;
}
.track-row:hover { background: rgba(139, 180, 212, 0.04); border-color: var(--border); }
.track-row.playing { background: var(--bg-active); border-color: rgba(139, 180, 212, 0.15); color: var(--primary); }
.track-row .track-num { color: var(--text-muted); font-size: 0.75rem; text-align: center; }
.track-row .track-title { font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 0.85rem; }
.track-row .track-artist { color: var(--text-secondary); font-size: 0.8rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.track-row .track-duration { color: var(--text-muted); font-size: 0.8rem; text-align: right; }
.track-row .track-format { color: var(--text-muted); font-size: 0.72rem; text-align: right; }

/* ── Album grid ────────────────────────────────────────────────────────────── */
.album-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(110px, 1fr));
  gap: 8px;
  padding: 12px;
}
.album-card {
  background: var(--bg-card);
  border-radius: var(--radius);
  overflow: hidden;
  cursor: pointer;
  transition: transform 0.12s, border-color 0.12s;
  border: 1px solid var(--border);
}
.album-card:hover {
  transform: translateY(-2px);
  border-color: var(--primary-dim);
}
.album-card .album-art { width: 100%; aspect-ratio: 1; object-fit: cover; display: block; }
.album-card .album-art-placeholder {
  width: 100%; aspect-ratio: 1;
  background: var(--bg-card);
  display: flex; align-items: center; justify-content: center;
  font-size: 2rem; color: var(--text-muted);
}
.album-card .album-info { padding: 7px 8px; }
.album-card .album-title { font-weight: 600; font-size: 0.8rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.album-card .album-artist { font-size: 0.72rem; color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin-top: 2px; }
.album-card .album-meta { font-size: 0.68rem; color: var(--text-muted); margin-top: 2px; opacity: 0.6; }

/* ── Seek / volume bars ────────────────────────────────────────────────────── */
.seek-bar {
  position: relative; height: 3px;
  background: rgba(139, 180, 212, 0.1);
  border-radius: 99px; cursor: pointer;
}
.seek-bar .seek-fill {
  position: absolute; left: 0; top: 0; bottom: 0;
  background: linear-gradient(90deg, var(--primary), var(--accent));
  border-radius: 99px; pointer-events: none;
}
.seek-bar .seek-thumb {
  position: absolute; top: 50%;
  width: 10px; height: 10px;
  background: #fff; border-radius: 50%;
  transform: translate(-50%, -50%);
  box-shadow: 0 0 0 2px var(--primary-dim);
  opacity: 0; pointer-events: none; transition: opacity 0.15s;
}
.seek-bar:hover .seek-thumb { opacity: 1; }
.volume-bar { width: 70px; height: 3px; background: rgba(139,180,212,0.08); border-radius: 99px; cursor: pointer; position: relative; }
.volume-bar .volume-fill { position: absolute; left: 0; top: 0; bottom: 0; background: var(--text-muted); border-radius: 99px; pointer-events: none; transition: background 0.1s; }
.volume-bar:hover .volume-fill { background: var(--primary); }

/* ── Spinner ───────────────────────────────────────────────────────────────── */
.spinner { width: 18px; height: 18px; border: 2px solid var(--bg-active); border-top-color: var(--primary); border-radius: 50%; animation: spin 0.7s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }

/* ── Empty state ───────────────────────────────────────────────────────────── */
.empty-state { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; padding: 4rem; color: var(--text-muted); text-align: center; }
.empty-state .empty-icon  { font-size: 2.5rem; opacity: 0.35; }
.empty-state .empty-title { font-size: 0.9rem; font-weight: 600; color: var(--text-secondary); }
.empty-state .empty-body  { font-size: 0.8rem; }

/* ── Scan bar ──────────────────────────────────────────────────────────────── */
.scan-bar-wrap { padding: 6px 14px; background: var(--bg-card); border-bottom: 1px solid var(--border-dim); }
.scan-bar-track { height: 2px; background: var(--bg-active); border-radius: 99px; overflow: hidden; }
.scan-bar-fill { height: 100%; background: var(--primary); border-radius: 99px; transition: width 0.3s ease; }

/* ── Badge ─────────────────────────────────────────────────────────────────── */
.badge { display: inline-block; padding: 2px 7px; border-radius: 99px; font-size: 0.68rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; }
.badge-accent   { background: rgba(139,180,212,0.12); color: var(--primary); }
.badge-success  { background: rgba(34,197,94,0.12); color: var(--success); }
.badge-warning  { background: rgba(245,158,11,0.12); color: var(--warning); }
.badge-error    { background: rgba(239,68,68,0.12); color: var(--error); }
.badge-muted    { background: var(--bg-active); color: var(--text-muted); }

/* ── Dropdown ──────────────────────────────────────────────────────────────── */
.dropdown { background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius); box-shadow: 0 12px 40px rgba(0,0,0,0.5); overflow: hidden; min-width: 180px; }
.dropdown-item { display: flex; align-items: center; gap: 10px; padding: 8px 12px; font-size: 0.82rem; cursor: pointer; transition: background 0.1s; }
.dropdown-item:hover { background: var(--bg-hover); }
.dropdown-item.danger { color: var(--error); }
.dropdown-sep { height: 1px; background: var(--border-dim); margin: 4px 0; }

/* ── Page header ───────────────────────────────────────────────────────────── */
.page-header {
  display: flex; align-items: center; gap: 12px;
  padding: 12px 14px 8px;
  position: sticky; top: 0; z-index: 10;
  background: var(--bg-base);
  border-bottom: 1px solid var(--border-dim);
}

/* ── Page-level backgrounds (downloads/tools/etc keep dark base) ── */
.library-page, .downloads-page, .settings-page,
.playlists-page, .artists-page, .import-page, .tools-page {
  background: var(--bg-base);
}
```

- [ ] **Step 2: Verify the build still passes**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -20
```

Expected: build succeeds (existing a11y warning in downloads page is fine, no new errors).

- [ ] **Step 3: Commit**

```bash
cd "c:/Cassette Music"
git add ui/src/app.css
git commit -m "feat(ui): replace design tokens with Steel Dusk palette, add Inter font"
```

---

## Task 2: Restyle the topbar and layout chrome

**Files:**
- Modify: `ui/src/routes/+layout.svelte`

- [ ] **Step 1: Replace `+layout.svelte`**

```svelte
<script lang="ts">
  import '../app.css';
  import { onMount, onDestroy } from 'svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import NowPlaying from '$lib/components/NowPlaying.svelte';
  import RightSidebar from '$lib/components/RightSidebar.svelte';
  import CommandPalette from '$lib/components/CommandPalette.svelte';
  import { startPlayerPoll, stopPlayerPoll } from '$lib/stores/player';
  import { loadLibrary } from '$lib/stores/library';
  import { openPalette } from '$lib/stores/commands';
  import { compactPlayerMode, toggleCompactPlayerMode, minimizeAppWindow } from '$lib/stores/shell';

  onMount(() => {
    startPlayerPoll();
    loadLibrary();
  });

  onDestroy(() => {
    stopPlayerPoll();
  });
</script>

<div class="app-shell" class:compact-player={$compactPlayerMode}>
  <header class="app-topbar">
    <div class="topbar-brand">
      <span class="brand-wordmark">Cassette</span>
      <span class="brand-divider">//</span>
      <span class="brand-mode">Desktop</span>
    </div>
    <nav class="topbar-nav" aria-label="Quick actions">
      <a class="topbar-link" href="/">Library</a>
      <a class="topbar-link" href="/downloads">Downloads</a>
      <a class="topbar-link" href="/settings">Settings</a>
    </nav>
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

  <aside class="app-sidebar">
    <Sidebar />
  </aside>

  <main class="app-main">
    <slot />
  </main>

  <aside class="app-right">
    <RightSidebar />
  </aside>

  <footer class="app-nowplaying">
    <NowPlaying />
  </footer>

  <CommandPalette />
</div>
```

- [ ] **Step 2: Build and check**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

Expected: no new errors.

- [ ] **Step 3: Commit**

```bash
cd "c:/Cassette Music"
git add ui/src/routes/+layout.svelte
git commit -m "feat(ui): update topbar chrome to Steel Dusk layout"
```

---

## Task 3: Restyle the Sidebar component

**Files:**
- Modify: `ui/src/lib/components/Sidebar.svelte`

- [ ] **Step 1: Replace `Sidebar.svelte`**

```svelte
<script lang="ts">
  import { page } from '$app/stores';
  import { trackCount, isScanning, scanProgress } from '$lib/stores/library';

  const coreLinks = [
    { href: '/', label: 'Library',   icon: 'LIB' },
    { href: '/downloads', label: 'Downloads', icon: 'DL' },
    { href: '/playlists', label: 'Playlists', icon: 'PL' },
    { href: '/artists',   label: 'Artists',   icon: 'AR' },
  ];

  const utilityLinks = [
    { href: '/import',   label: 'Import',   icon: 'IM' },
    { href: '/tools',    label: 'Tools',    icon: 'TL' },
    { href: '/settings', label: 'Settings', icon: 'CFG' },
  ];

  function isActive(href: string, pathname: string): boolean {
    if (href === '/') return pathname === '/';
    return pathname === href || pathname.startsWith(`${href}/`);
  }
</script>

<nav class="sidebar">
  <div class="sidebar-logo">
    <span class="logo-tape">TAPE</span>
    <span class="logo-name">Cassette</span>
  </div>

  <ul class="nav-list">
    {#each coreLinks as link}
      {@const active = isActive(link.href, $page.url.pathname)}
      <li>
        <a href={link.href} class="nav-item" class:active>
          <span class="nav-icon">{link.icon}</span>
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
          <span class="nav-icon">{link.icon}</span>
          <span class="nav-label">{link.label}</span>
        </a>
      </li>
    {/each}
  </ul>

  <div class="sidebar-footer">
    {#if $isScanning}
      <div class="scan-widget">
        <div class="scan-label">
          <span>Scanning…</span>
          {#if $scanProgress}
            <span>{$scanProgress.scanned.toLocaleString()}</span>
          {/if}
        </div>
        <div class="scan-track">
          <div
            class="scan-fill"
            style:width={$scanProgress && $scanProgress.total > 0
              ? `${($scanProgress.scanned / $scanProgress.total) * 100}%`
              : '0%'}
          ></div>
        </div>
        {#if $scanProgress}
          <div class="scan-file">{$scanProgress.current_file.split(/[/\\]/).pop()}</div>
        {/if}
      </div>
    {/if}
    <div class="stat-row">
      <span class="stat-value">{$trackCount.toLocaleString()}</span>
      <span class="stat-label">tracks</span>
    </div>
  </div>
</nav>

<style>
.sidebar {
  display: flex; flex-direction: column;
  height: 100%; padding-bottom: 8px;
  user-select: none;
}

.sidebar-logo {
  display: flex; flex-direction: column;
  padding: 14px 14px 12px;
  border-bottom: 1px solid var(--border-dim);
  margin-bottom: 6px;
}
.logo-tape {
  font-size: 0.62rem; font-weight: 800; letter-spacing: 0.14em;
  text-transform: uppercase; color: var(--primary);
}
.logo-name {
  font-size: 0.85rem; font-weight: 700; letter-spacing: -0.01em;
  color: var(--text-primary); margin-top: 1px;
}

.nav-list { list-style: none; margin: 0; padding: 0 6px; display: flex; flex-direction: column; gap: 1px; }

.nav-item {
  display: flex; align-items: center; gap: 7px;
  padding: 7px 8px 7px 6px;
  border-radius: var(--radius-sm);
  color: var(--text-muted);
  font-size: 0.78rem; font-weight: 500;
  border-right: 2px solid transparent;
  transition: color 0.1s, background 0.1s;
  text-decoration: none;
  margin-right: -6px;
  padding-right: 14px;
}
.nav-item:hover { color: var(--text-secondary); background: rgba(139,180,212,0.04); }
.nav-item.active {
  color: var(--primary);
  background: var(--bg-active);
  border-right-color: var(--primary);
}

.nav-icon {
  font-size: 0.6rem; font-weight: 700; letter-spacing: 0.08em;
  width: 28px; flex-shrink: 0; color: inherit; text-transform: uppercase;
}
.nav-label { flex: 1; }

.nav-divider { height: 1px; background: var(--border-dim); margin: 6px 10px; }

.sidebar-footer {
  margin-top: auto; padding: 10px 8px 0;
  border-top: 1px solid var(--border-dim);
  display: flex; flex-direction: column; gap: 6px;
}

.stat-row { display: flex; align-items: baseline; gap: 5px; padding: 2px 6px; }
.stat-value { font-size: 0.85rem; font-weight: 600; color: var(--text-primary); }
.stat-label { font-size: 0.65rem; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.06em; }

.scan-widget {
  padding: 7px 8px; background: var(--bg-card);
  border-radius: var(--radius-sm); border: 1px solid var(--border);
}
.scan-label { display: flex; justify-content: space-between; font-size: 0.68rem; color: var(--text-secondary); margin-bottom: 5px; }
.scan-track { height: 2px; background: var(--bg-active); border-radius: 99px; overflow: hidden; }
.scan-fill { height: 100%; background: var(--primary); border-radius: 99px; transition: width 0.3s; min-width: 6px; }
.scan-file { font-size: 0.62rem; color: var(--text-muted); margin-top: 3px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
```

- [ ] **Step 2: Build**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

- [ ] **Step 3: Commit**

```bash
cd "c:/Cassette Music"
git add ui/src/lib/components/Sidebar.svelte
git commit -m "feat(ui): restyle sidebar with Steel Dusk nav items and logo block"
```

---

## Task 4: Restyle the NowPlaying bar

**Files:**
- Modify: `ui/src/lib/components/NowPlaying.svelte`

- [ ] **Step 1: Replace `NowPlaying.svelte`**

```svelte
<script lang="ts">
  import { playbackState, isPlaying, progressPct, isSeeking, seekPreview, player } from '$lib/stores/player';
  import { loadQueue } from '$lib/stores/queue';
  import { formatDuration, coverSrc, clamp } from '$lib/utils';

  let seekBarEl: HTMLDivElement;
  let volBarEl: HTMLDivElement;

  function getSeekPct(e: MouseEvent): number {
    const rect = seekBarEl.getBoundingClientRect();
    return clamp((e.clientX - rect.left) / rect.width, 0, 1);
  }

  function onSeekMouseDown(e: MouseEvent) {
    isSeeking.set(true);
    seekPreview.set(getSeekPct(e));
    const onMove = (ev: MouseEvent) => seekPreview.set(getSeekPct(ev));
    const onUp = async (ev: MouseEvent) => {
      const pct = getSeekPct(ev);
      isSeeking.set(false);
      await player.seek(pct);
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    };
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  }

  function onVolMouseDown(e: MouseEvent) {
    const update = (ev: MouseEvent) => {
      const rect = volBarEl.getBoundingClientRect();
      player.setVolume(clamp((ev.clientX - rect.left) / rect.width, 0, 1));
    };
    update(e);
    const onUp = () => {
      window.removeEventListener('mousemove', update);
      window.removeEventListener('mouseup', onUp);
    };
    window.addEventListener('mousemove', update);
    window.addEventListener('mouseup', onUp);
  }

  $: track = $playbackState.current_track;
  $: pos   = $playbackState.position_secs;
  $: dur   = $playbackState.duration_secs;
  $: vol   = $playbackState.volume;
  $: pct   = $progressPct;

  async function handleNext() { await player.next(); await loadQueue(); }
  async function handlePrev() { await player.prev(); await loadQueue(); }
</script>

<div class="nowplaying">
  <!-- Left: art + info -->
  <div class="np-left">
    <div class="np-art">
      {#if track?.cover_art_path}
        <img src={coverSrc(track.cover_art_path)} alt="cover" />
      {:else}
        <div class="np-art-ph">🎵</div>
      {/if}
    </div>
    <div class="np-info">
      <div class="np-title">{track?.title ?? '—'}</div>
      <div class="np-artist">{track?.artist ?? 'No track playing'}</div>
    </div>
  </div>

  <!-- Center: controls + seek -->
  <div class="np-center">
    <div class="np-controls">
      <button class="ctrl-btn" on:click={handlePrev} title="Previous">⏮</button>
      <button class="ctrl-btn play-btn" on:click={() => player.toggle()} title="Play/Pause">
        {#if $isPlaying}⏸{:else}▶{/if}
      </button>
      <button class="ctrl-btn" on:click={handleNext} title="Next">⏭</button>
    </div>
    <div class="np-seek">
      <span class="np-time">{formatDuration(pos)}</span>
      <!-- svelte-ignore a11y-no-static-element-interactions -->
      <div class="seek-bar" bind:this={seekBarEl} on:mousedown={onSeekMouseDown}>
        <div class="seek-fill" style="width:{pct*100}%"></div>
        <div class="seek-thumb" style="left:{pct*100}%"></div>
      </div>
      <span class="np-time right">{formatDuration(dur)}</span>
    </div>
  </div>

  <!-- Right: volume -->
  <div class="np-right">
    <span class="vol-icon">{vol === 0 ? '🔇' : vol < 0.5 ? '🔉' : '🔊'}</span>
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div class="volume-bar" bind:this={volBarEl} on:mousedown={onVolMouseDown}>
      <div class="volume-fill" style="width:{vol*100}%"></div>
    </div>
  </div>
</div>

<style>
.nowplaying {
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  align-items: center;
  height: var(--nowplaying-h);
  padding: 0 14px;
  gap: 12px;
}

.np-left { display: flex; align-items: center; gap: 10px; overflow: hidden; }
.np-art  { width: 44px; height: 44px; flex-shrink: 0; border-radius: 5px; overflow: hidden; background: var(--bg-card); box-shadow: 0 2px 8px rgba(0,0,0,0.5); }
.np-art img { width: 100%; height: 100%; object-fit: cover; }
.np-art-ph  { width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; font-size: 1.2rem; color: var(--text-muted); }
.np-info    { overflow: hidden; }
.np-title   { font-weight: 600; font-size: 0.82rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: var(--text-primary); }
.np-artist  { font-size: 0.72rem; color: var(--text-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; margin-top: 1px; }

.np-center { display: flex; flex-direction: column; align-items: center; gap: 5px; min-width: 300px; max-width: 460px; width: 100%; }
.np-controls { display: flex; align-items: center; gap: 8px; }
.ctrl-btn {
  display: inline-flex; align-items: center; justify-content: center;
  width: 28px; height: 28px; border-radius: 50%; font-size: 0.9rem;
  color: var(--text-muted); background: none; border: none; cursor: pointer;
  transition: background 0.15s, color 0.15s;
}
.ctrl-btn:hover { background: var(--bg-hover); color: var(--text-primary); }
.play-btn {
  width: 32px; height: 32px;
  background: var(--primary) !important;
  color: var(--bg-deep) !important;
  box-shadow: 0 2px 10px rgba(139,180,212,0.25);
}
.play-btn:hover { background: #a0c8e8 !important; }

.np-seek  { display: flex; align-items: center; gap: 7px; width: 100%; }
.np-time  { font-size: 0.68rem; color: var(--text-muted); white-space: nowrap; min-width: 32px; }
.np-time.right { text-align: right; }

.np-right   { display: flex; align-items: center; gap: 7px; justify-content: flex-end; }
.vol-icon   { font-size: 0.82rem; color: var(--text-muted); }
</style>
```

- [ ] **Step 2: Build**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

- [ ] **Step 3: Commit**

```bash
cd "c:/Cassette Music"
git add ui/src/lib/components/NowPlaying.svelte
git commit -m "feat(ui): restyle now-playing bar with primary play button and gradient seek"
```

---

## Task 5: Restyle RightSidebar and QueuePanel

**Files:**
- Modify: `ui/src/lib/components/RightSidebar.svelte`
- Modify: `ui/src/lib/components/QueuePanel.svelte`

- [ ] **Step 1: Replace `RightSidebar.svelte`**

```svelte
<script lang="ts">
  import { nowPlayingContext, currentTrack } from '$lib/stores/player';
  import QueuePanel from './QueuePanel.svelte';

  let activeTab: 'queue' | 'info' = 'queue';
  $: ctx   = $nowPlayingContext;
  $: track = $currentTrack;
</script>

<div class="right-sidebar">
  <div class="rs-tabs">
    <button class="rs-tab" class:active={activeTab === 'queue'} on:click={() => (activeTab = 'queue')}>Queue</button>
    <button class="rs-tab" class:active={activeTab === 'info'}  on:click={() => (activeTab = 'info')}>Info</button>
  </div>

  <div class="rs-content">
    {#if activeTab === 'queue'}
      <QueuePanel />
    {:else}
      <div class="info-panel">
        {#if track && ctx}
          <div class="info-section">
            <div class="info-label">Artist</div>
            <div class="info-artist">{ctx.artist_name}</div>
            {#if ctx.artist_tags?.length}
              <div class="info-tags">
                {#each ctx.artist_tags.slice(0, 6) as tag}
                  <span class="info-tag">{tag}</span>
                {/each}
              </div>
            {/if}
            {#if ctx.listeners}
              <div class="info-meta">{ctx.listeners.toLocaleString()} monthly listeners</div>
            {/if}
            {#if ctx.artist_summary}
              <p class="info-summary">{ctx.artist_summary}</p>
            {/if}
          </div>

          {#if ctx.album_title}
            <div class="info-section">
              <div class="info-label">Album</div>
              <div class="info-album">{ctx.album_title}</div>
              {#if ctx.album_summary}
                <p class="info-summary">{ctx.album_summary}</p>
              {/if}
            </div>
          {/if}

          {#if ctx.lyrics}
            <div class="info-section">
              <div class="info-label">Lyrics {#if ctx.lyrics_source}<span class="info-source">· {ctx.lyrics_source}</span>{/if}</div>
              <pre class="lyrics">{ctx.lyrics}</pre>
            </div>
          {/if}
        {:else if track}
          <div class="empty-state" style="padding:2rem 1rem;">
            <div class="empty-icon">🎵</div>
            <div class="empty-title">{track.title}</div>
            <div class="empty-body">{track.artist}</div>
          </div>
        {:else}
          <div class="empty-state" style="padding:2rem 1rem;">
            <div class="empty-icon">🎤</div>
            <div class="empty-title">Nothing playing</div>
            <div class="empty-body">Artist info &amp; lyrics appear here.</div>
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
.right-sidebar { display: flex; flex-direction: column; height: 100%; }

.rs-tabs { display: flex; border-bottom: 1px solid var(--border-dim); flex-shrink: 0; }
.rs-tab {
  flex: 1; padding: 10px 6px; font-size: 0.75rem; font-weight: 600;
  color: var(--text-muted); cursor: pointer; background: none;
  border: none; border-bottom: 2px solid transparent; margin-bottom: -1px;
  transition: color 0.15s; letter-spacing: 0.04em;
}
.rs-tab:hover { color: var(--text-secondary); }
.rs-tab.active { color: var(--primary); border-bottom-color: var(--primary); }

.rs-content { flex: 1; overflow-y: auto; display: flex; flex-direction: column; }

.info-panel { padding: 0 10px; display: flex; flex-direction: column; }
.info-section { padding: 12px 0; border-bottom: 1px solid var(--border-dim); }
.info-section:last-child { border-bottom: none; }
.info-label { font-size: 0.62rem; text-transform: uppercase; letter-spacing: 0.08em; color: var(--text-muted); margin-bottom: 5px; font-weight: 700; }
.info-source { font-weight: 400; text-transform: none; letter-spacing: 0; }
.info-artist { font-size: 0.9rem; font-weight: 700; color: var(--text-primary); margin-bottom: 5px; }
.info-album  { font-size: 0.82rem; font-weight: 600; color: var(--text-primary); margin-bottom: 5px; }
.info-tags   { display: flex; flex-wrap: wrap; gap: 3px; margin-bottom: 5px; }
.info-tag    { font-size: 0.62rem; padding: 2px 6px; border-radius: 3px; background: rgba(139,180,212,0.1); color: var(--primary); letter-spacing: 0.04em; }
.info-meta   { font-size: 0.7rem; color: var(--text-secondary); margin-bottom: 5px; }
.info-summary { font-size: 0.75rem; color: var(--text-secondary); line-height: 1.65; margin: 0; max-height: 90px; overflow: hidden; }
.lyrics { font-family: inherit; font-size: 0.75rem; color: var(--text-secondary); white-space: pre-wrap; line-height: 2; margin: 0; max-height: 260px; overflow-y: auto; }
</style>
```

- [ ] **Step 2: Replace `QueuePanel.svelte`**

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { queue, loadQueue, clearQueue } from '$lib/stores/queue';
  import { playbackState } from '$lib/stores/player';
  import { api } from '$lib/api/tauri';
  import { formatDuration } from '$lib/utils';

  onMount(() => loadQueue());

  async function jumpTo(index: number) {
    const items = $queue;
    if (!items[index]) return;
    const trackIds = items.map((i) => i.track_id);
    await api.queueTracks(trackIds, index);
    await loadQueue();
  }
</script>

<div class="queue-panel">
  <div class="queue-header">
    <span class="queue-title">Up Next</span>
    {#if $queue.length > 0}
      <button class="clear-btn" on:click={clearQueue}>Clear</button>
    {/if}
  </div>

  {#if $queue.length === 0}
    <div class="empty-state" style="padding:2rem 1rem;">
      <div class="empty-icon">🎵</div>
      <div class="empty-title">Queue is empty</div>
      <div class="empty-body">Play a track or album to fill the queue.</div>
    </div>
  {:else}
    <ul class="queue-list">
      {#each $queue as item, i}
        {@const track = item.track}
        {@const isCurrent = i === $playbackState.queue_position}
        <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
        <li class="queue-item" class:current={isCurrent} on:dblclick={() => jumpTo(i)}>
          <span class="q-num" class:active={isCurrent}>{isCurrent ? '▶' : i + 1}</span>
          <div class="q-info">
            <div class="q-title">{track?.title ?? 'Unknown'}</div>
            <div class="q-artist">{track?.artist ?? ''}</div>
          </div>
          <span class="q-dur">{formatDuration(track?.duration_secs ?? 0)}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
.queue-panel { display: flex; flex-direction: column; height: 100%; }
.queue-header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 10px 12px 8px; flex-shrink: 0;
}
.queue-title { font-size: 0.62rem; font-weight: 700; letter-spacing: 0.1em; text-transform: uppercase; color: var(--text-muted); }
.clear-btn {
  font-size: 0.68rem; padding: 2px 7px; border-radius: var(--radius-sm);
  color: var(--text-muted); border: 1px solid var(--border-dim); cursor: pointer; background: none;
  transition: background 0.1s, color 0.1s;
}
.clear-btn:hover { background: var(--bg-hover); color: var(--text-secondary); }

.queue-list { list-style: none; margin: 0; padding: 4px 6px; overflow-y: auto; flex: 1; }
.queue-item {
  display: flex; align-items: center; gap: 8px;
  padding: 5px 6px; border-radius: var(--radius-sm);
  cursor: default; transition: background 0.1s;
}
.queue-item:hover { background: var(--bg-hover); }
.queue-item.current { background: var(--bg-active); }
.q-num  { font-size: 0.68rem; color: var(--text-muted); min-width: 18px; text-align: center; }
.q-num.active { color: var(--primary); }
.q-info { flex: 1; overflow: hidden; }
.q-title  { font-size: 0.78rem; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: var(--text-primary); }
.q-artist { font-size: 0.68rem; color: var(--text-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.queue-item.current .q-title { color: var(--primary); }
.q-dur  { font-size: 0.68rem; color: var(--text-muted); white-space: nowrap; }
</style>
```

- [ ] **Step 3: Build**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

- [ ] **Step 4: Commit**

```bash
cd "c:/Cassette Music"
git add ui/src/lib/components/RightSidebar.svelte ui/src/lib/components/QueuePanel.svelte
git commit -m "feat(ui): restyle right sidebar, info panel, and queue panel"
```

---

## Task 6: Add `tintFromHex` utility

**Files:**
- Modify: `ui/src/lib/utils.ts`

- [ ] **Step 1: Add `tintFromHex` to the end of `utils.ts`**

```typescript
/**
 * Given a dominant color hex (e.g. "#3d2810"), return CSS color strings
 * for a darkened/desaturated card background and a lightened title color.
 * Falls back to token values if hex is null/invalid.
 */
export function tintFromHex(hex: string | null | undefined): { bg: string; titleColor: string } {
  const fallback = { bg: 'var(--bg-card)', titleColor: 'var(--text-primary)' };
  if (!hex) return fallback;

  const clean = hex.replace('#', '');
  if (clean.length !== 6) return fallback;

  const r = parseInt(clean.slice(0, 2), 16);
  const g = parseInt(clean.slice(2, 4), 16);
  const b = parseInt(clean.slice(4, 6), 16);
  if (isNaN(r) || isNaN(g) || isNaN(b)) return fallback;

  // Dark background: crush luminance, keep a hint of hue
  const bgR = Math.round(r * 0.08 + 4);
  const bgG = Math.round(g * 0.08 + 4);
  const bgB = Math.round(b * 0.08 + 4);

  // Title color: lift luminance, partially desaturate toward slate
  const titleR = Math.round(r * 0.35 + 140);
  const titleG = Math.round(g * 0.35 + 150);
  const titleB = Math.round(b * 0.35 + 160);

  const clamp = (v: number) => Math.max(0, Math.min(255, v));

  return {
    bg: `rgb(${clamp(bgR)}, ${clamp(bgG)}, ${clamp(bgB)})`,
    titleColor: `rgb(${clamp(titleR)}, ${clamp(titleG)}, ${clamp(titleB)})`,
  };
}
```

- [ ] **Step 2: Build to verify no TypeScript errors**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

- [ ] **Step 3: Commit**

```bash
cd "c:/Cassette Music"
git add ui/src/lib/utils.ts
git commit -m "feat(ui): add tintFromHex utility for per-card color tinting"
```

---

## Task 7: Backend — add `dominant_color_hex` to Album

This task has three sub-steps: add the `image` crate, add a DB migration + extraction function, update the `Album` struct and `get_albums` query.

**Files:**
- Modify: `crates/cassette-core/Cargo.toml`
- Modify: `crates/cassette-core/src/models/mod.rs`
- Modify: `crates/cassette-core/src/db/mod.rs`

- [ ] **Step 1: Add the `image` crate to `Cargo.toml`**

In `crates/cassette-core/Cargo.toml`, add after the `blake3` line:

```toml
image = { version = "0.25", default-features = false, features = ["jpeg", "png"] }
```

- [ ] **Step 2: Add `dominant_color_hex` field to the `Album` struct**

In `crates/cassette-core/src/models/mod.rs`, find the `Album` struct (currently lines 40–47) and add the field:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Album {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub year: Option<i32>,
    pub cover_art_path: Option<String>,
    pub track_count: usize,
    pub dominant_color_hex: Option<String>,
}
```

- [ ] **Step 3: Add `extract_dominant_color` function to `db/mod.rs`**

Add this function near the top of `crates/cassette-core/src/db/mod.rs` (after the `use` imports, before `impl Db`):

```rust
/// Reads an image file from disk, downsamples it to 8×8, and returns the
/// average colour as a CSS hex string like `"#3d2810"`.
/// Returns `None` if the file cannot be read or decoded.
pub fn extract_dominant_color(path: &str) -> Option<String> {
    let img = image::open(path).ok()?;
    let small = img.resize_exact(8, 8, image::imageops::FilterType::Lanczos3);
    let rgb = small.to_rgb8();
    let pixels = rgb.pixels();
    let count = 64u64;
    let (r, g, b) = pixels.fold((0u64, 0u64, 0u64), |(ar, ag, ab), p| {
        (ar + p[0] as u64, ag + p[1] as u64, ab + p[2] as u64)
    });
    Some(format!("#{:02x}{:02x}{:02x}", (r / count) as u8, (g / count) as u8, (b / count) as u8))
}
```

Also add the `image` import at the top of the file with the other `use` statements:

```rust
use image;
```

- [ ] **Step 4: Add DB migration for the new column**

In `crates/cassette-core/src/db/mod.rs`, find the `new()` or `open()` function where migrations run (look for `CREATE TABLE IF NOT EXISTS tracks`). After the existing schema setup, add:

```rust
conn.execute_batch(
    "ALTER TABLE tracks ADD COLUMN IF NOT EXISTS dominant_color_hex TEXT;"
).ok(); // ok() because older SQLite may not support IF NOT EXISTS — ignore error
```

Since `get_albums` is a query over `tracks` grouped by album, the hex is derived at query time from `cover_art_path`. No separate column in tracks is needed if we compute it on read — but for performance we store it. Add the migration in the `Db::new` / `Db::open` constructor block after existing `execute_batch` calls:

```rust
conn.execute_batch(
    "ALTER TABLE tracks ADD COLUMN dominant_color_hex TEXT;"
).ok(); // silently ignored if column already exists
```

- [ ] **Step 5: Update `get_albums` to call `extract_dominant_color`**

Replace the existing `get_albums` function (currently lines 760–780 of `db/mod.rs`) with:

```rust
pub fn get_albums(&self) -> Result<Vec<Album>> {
    let mut stmt = self.conn.prepare("
        SELECT album_artist, album, MIN(year), MIN(cover_art_path), COUNT(*)
        FROM tracks GROUP BY album_artist, album
        ORDER BY album_artist COLLATE NOCASE, album COLLATE NOCASE
    ")?;
    let rows = stmt.query_map([], |row| {
        let artist: String = row.get(0)?;
        let title: String = row.get(1)?;
        let id = stable_entity_id(&[artist.as_str(), title.as_str()]);
        let cover_art_path: Option<String> = row.get(3)?;
        Ok(Album {
            id,
            title,
            artist,
            year: row.get(2)?,
            dominant_color_hex: cover_art_path
                .as_deref()
                .and_then(extract_dominant_color),
            cover_art_path,
            track_count: row.get::<_, i64>(4)? as usize,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}
```

- [ ] **Step 6: Verify the Rust workspace compiles**

```bash
cd "c:/Cassette Music" && cargo check --workspace 2>&1 | tail -20
```

Expected: `Finished` with no errors. Fix any type errors before proceeding.

- [ ] **Step 7: Commit**

```bash
cd "c:/Cassette Music"
git add crates/cassette-core/Cargo.toml crates/cassette-core/src/models/mod.rs crates/cassette-core/src/db/mod.rs
git commit -m "feat(core): add dominant_color_hex to Album via lofty pixel average"
```

---

## Task 8: Wire `dominant_color_hex` to the frontend API

**Files:**
- Modify: `ui/src/lib/api/tauri.ts`

- [ ] **Step 1: Add field to the `Album` interface**

In `ui/src/lib/api/tauri.ts`, find the `Album` interface (currently lines 25–31) and add the field:

```typescript
export interface Album {
  id: number;
  title: string;
  artist: string;
  year: number | null;
  cover_art_path: string | null;
  track_count: number;
  dominant_color_hex: string | null;
}
```

- [ ] **Step 2: Build**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -10
```

- [ ] **Step 3: Commit**

```bash
cd "c:/Cassette Music"
git add ui/src/lib/api/tauri.ts
git commit -m "feat(ui): add dominant_color_hex to Album TypeScript interface"
```

---

## Task 9: Restyle the library page — album grid with tinting and album detail backdrop

**Files:**
- Modify: `ui/src/routes/+page.svelte`

This is the biggest single-file change. The album grid cards use `tintFromHex` for per-card metadata tinting. The album detail view adds the blurred backdrop. The overall page structure stays the same — only styles and the card/detail markup change.

- [ ] **Step 1: Add `tintFromHex` import to the script block**

In `ui/src/routes/+page.svelte`, update the import line:

```typescript
import { formatDuration, formatAudioSpec, coverSrc, debounce, tintFromHex } from '$lib/utils';
```

- [ ] **Step 2: Replace the `<style>` block**

Replace the entire `<style>` block at the bottom of `ui/src/routes/+page.svelte` with:

```svelte
<style>
.library-page { display: flex; flex-direction: column; min-height: 100%; background: var(--bg-base); }

/* header */
.page-header { background: var(--bg-base); border-bottom: 1px solid var(--border-dim); }
.search-wrap { position: relative; display: flex; align-items: center; flex: 1; max-width: 320px; }
.search-icon { position: absolute; left: 9px; font-size: 0.78rem; pointer-events: none; color: var(--text-muted); }
.search-input { padding-left: 28px !important; }
.search-spinner { position: absolute; right: 9px; }

/* search results */
.search-results { padding: 0 12px 12px; }
.sr-label { font-size: 0.72rem; color: var(--text-muted); padding: 6px 14px 3px; letter-spacing: 0.04em; }

/* track list */
.track-list { padding: 6px; }

/* album detail */
.album-detail { position: relative; display: flex; flex-direction: column; flex: 1; overflow: hidden; }
.album-backdrop {
  position: absolute; inset: 0;
  background-size: cover; background-position: center;
  filter: brightness(0.18) saturate(1.3) blur(3px);
  z-index: 0;
}
.album-backdrop-grad {
  position: absolute; inset: 0;
  background: linear-gradient(180deg, rgba(6,8,16,0.5) 0%, rgba(6,8,16,0.92) 55%, rgba(6,8,16,1) 100%);
  z-index: 1;
}
.album-detail-inner { position: relative; z-index: 2; display: flex; flex-direction: column; flex: 1; overflow: hidden; padding: 16px; }
.album-detail-header { display: flex; align-items: flex-end; gap: 16px; margin-bottom: 16px; flex-shrink: 0; }
.back-btn {
  position: absolute; top: 12px; left: 14px;
  font-size: 0.72rem; color: var(--text-muted);
  cursor: pointer; background: none; border: none;
  transition: color 0.1s;
}
.back-btn:hover { color: var(--text-secondary); }
.album-detail-art {
  width: 84px; height: 84px; flex-shrink: 0;
  border-radius: var(--radius); overflow: hidden;
  box-shadow: 0 8px 32px rgba(0,0,0,0.6);
}
.album-detail-art img { width: 100%; height: 100%; object-fit: cover; }
.album-detail-art-ph {
  width: 100%; height: 100%;
  background: var(--bg-card);
  display: flex; align-items: center; justify-content: center; font-size: 2.5rem;
}
.album-detail-info h1 { font-size: 1.3rem; font-weight: 800; color: #deeaf8; }
.album-detail-artist { color: rgba(200,220,240,0.55); font-size: 0.85rem; margin-top: 3px; }
.album-detail-meta   { color: rgba(200,220,240,0.3); font-size: 0.75rem; margin-top: 3px; }

/* artist list */
.artist-list { padding: 6px 10px; display: flex; flex-direction: column; gap: 3px; }
.artist-row {
  display: flex; align-items: center; gap: 12px;
  padding: 8px 10px; border-radius: var(--radius-sm);
  transition: background 0.1s; cursor: pointer;
}
.artist-row:hover { background: var(--bg-hover); }
.artist-avatar {
  width: 36px; height: 36px; border-radius: 50%;
  background: var(--bg-card);
  display: flex; align-items: center; justify-content: center;
  font-size: 0.9rem; font-weight: 700; color: var(--primary); flex-shrink: 0;
}
.artist-name { font-weight: 600; font-size: 0.85rem; }
.artist-meta { font-size: 0.7rem; color: var(--text-secondary); margin-top: 1px; }
</style>
```

- [ ] **Step 3: Update album card markup to use tinting**

In the album grid section of `ui/src/routes/+page.svelte`, find the `{#each $albums as album}` block and replace the `.album-card` div and its contents with:

```svelte
<div
  class="album-card"
  role="button"
  tabindex="0"
  on:click={() => openAlbum(album)}
  on:dblclick={() => playAlbum(album)}
  on:keydown={(event) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      openAlbum(album);
    }
  }}
>
  {#if album.cover_art_path}
    <img class="album-art" src={coverSrc(album.cover_art_path)} alt="cover" />
  {:else}
    <div class="album-art-placeholder">💿</div>
  {/if}
  {@const tint = tintFromHex(album.dominant_color_hex)}
  <div class="album-info" style="background:{tint.bg};">
    <div class="album-title" style="color:{tint.titleColor};">{album.title}</div>
    <div class="album-artist">{album.artist}</div>
    <div class="album-meta">{album.year ?? ''}{album.year && album.track_count ? ' · ' : ''}{album.track_count} tracks</div>
  </div>
</div>
```

- [ ] **Step 4: Update album detail view to add blurred backdrop**

Find the `{#if selectedAlbum}` block. Replace the wrapping `<div class="album-detail">` and `<div class="album-detail-header">` section with:

```svelte
<div class="album-detail">
  {#if selectedAlbum.cover_art_path}
    <div class="album-backdrop" style="background-image:url('{coverSrc(selectedAlbum.cover_art_path)}')"></div>
    <div class="album-backdrop-grad"></div>
  {/if}
  <div class="album-detail-inner">
    <button class="back-btn" on:click={closeAlbum}>← Albums</button>
    <div class="album-detail-header">
      <div class="album-detail-art">
        {#if selectedAlbum.cover_art_path}
          <img src={coverSrc(selectedAlbum.cover_art_path)} alt="cover" />
        {:else}
          <div class="album-detail-art-ph">💿</div>
        {/if}
      </div>
      <div class="album-detail-info">
        <h1>{selectedAlbum.title}</h1>
        <div class="album-detail-artist">{selectedAlbum.artist}</div>
        <div class="album-detail-meta">
          {#if selectedAlbum.year}{selectedAlbum.year} · {/if}{selectedAlbum.track_count} tracks
        </div>
        <button class="btn btn-primary" style="margin-top:10px;" on:click={() => playAlbum(selectedAlbum!)}>
          ▶ Play Album
        </button>
      </div>
    </div>

    {#if loadingAlbumTracks}
      <div class="empty-state"><div class="spinner"></div></div>
    {:else}
      <div class="track-list" style="overflow-y:auto;flex:1;">
        {#each albumTracks as track, i}
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div class="track-row" on:dblclick={() => playTrack(albumTracks, i)}>
            <span class="track-num">{track.track_number ?? i + 1}</span>
            <div class="track-title">{track.title}</div>
            <div class="track-artist">{track.artist !== selectedAlbum?.artist ? track.artist : ''}</div>
            <span class="track-duration">{formatDuration(track.duration_secs)}</span>
            <span class="track-format">{track.format.toUpperCase()}</span>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>
```

- [ ] **Step 5: Build**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -15
```

Expected: build passes (existing a11y warning in downloads is fine).

- [ ] **Step 6: Commit**

```bash
cd "c:/Cassette Music"
git add ui/src/routes/+page.svelte
git commit -m "feat(ui): album grid tinting + blurred backdrop album detail view"
```

---

## Task 10: Restyle the Settings page

**Files:**
- Modify: `ui/src/routes/settings/+page.svelte`

- [ ] **Step 1: Replace the `<style>` block in `settings/+page.svelte`**

Find and replace the entire `<style>` block:

```svelte
<style>
.settings-page { display: flex; flex-direction: column; height: 100%; background: var(--bg-base); }

.settings-layout { display: flex; flex: 1; overflow: hidden; }

/* sub-nav */
.settings-subnav {
  width: 130px; flex-shrink: 0;
  border-right: 1px solid var(--border-dim);
  background: var(--bg-deep);
  padding: 10px 0;
  display: flex; flex-direction: column; gap: 1px;
}
.subnav-item {
  padding: 7px 14px; font-size: 0.78rem; cursor: pointer;
  color: var(--text-muted); border-right: 2px solid transparent;
  transition: color 0.1s, background 0.1s;
}
.subnav-item:hover { color: var(--text-secondary); background: rgba(139,180,212,0.04); }
.subnav-item.active { color: var(--primary); background: rgba(139,180,212,0.06); border-right-color: var(--primary); }

/* content */
.settings-content { flex: 1; overflow-y: auto; padding: 16px 20px; display: flex; flex-direction: column; gap: 20px; }

.settings-section { display: flex; flex-direction: column; gap: 10px; }
.section-title {
  font-size: 0.82rem; font-weight: 700; color: var(--text-primary); letter-spacing: 0.03em;
  padding-bottom: 8px; border-bottom: 1px solid var(--border-dim);
}
.section-sub { font-size: 0.72rem; color: var(--text-muted); margin-top: -6px; }

/* field grid */
.field-group { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
.field-full { grid-column: 1 / -1; }
.field { display: flex; flex-direction: column; gap: 3px; }
.field label { font-size: 0.68rem; color: var(--text-secondary); font-weight: 600; letter-spacing: 0.04em; }
.field input {
  background: var(--bg-card); border: 1px solid var(--border);
  border-radius: var(--radius-sm); padding: 5px 9px;
  font-size: 0.8rem; color: var(--text-primary); font-family: inherit;
  transition: border-color 0.15s;
}
.field input::placeholder { color: var(--text-muted); }
.field input:focus { outline: none; border-color: var(--primary-dim); }

/* library roots */
.roots-list { display: flex; flex-direction: column; gap: 4px; }
.root-row {
  display: flex; align-items: center; gap: 8px;
  padding: 6px 8px; background: var(--bg-card);
  border-radius: var(--radius-sm); border: 1px solid var(--border);
}
.root-path { flex: 1; font-size: 0.75rem; color: var(--text-primary); font-family: monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.root-remove {
  font-size: 0.68rem; color: var(--text-muted); cursor: pointer;
  padding: 2px 6px; border-radius: 3px;
  background: rgba(239,68,68,0.06); border: 1px solid rgba(239,68,68,0.1);
  transition: background 0.1s, color 0.1s;
}
.root-remove:hover { background: rgba(239,68,68,0.12); color: var(--error); }

.root-actions { display: flex; gap: 6px; }
.action-btn {
  padding: 5px 11px; border-radius: var(--radius-sm); font-size: 0.72rem; font-weight: 700;
  background: rgba(139,180,212,0.08); color: var(--primary);
  border: 1px solid rgba(139,180,212,0.18); cursor: pointer;
  transition: background 0.1s;
}
.action-btn:hover { background: rgba(139,180,212,0.14); }

/* provider grid */
.provider-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 6px; }
.provider-card {
  padding: 8px 10px; border-radius: var(--radius-sm);
  background: var(--bg-card); border: 1px solid var(--border);
  display: flex; flex-direction: column; gap: 3px;
}
.provider-card.configured { border-color: rgba(139,180,212,0.18); }
.provider-name { font-size: 0.78rem; font-weight: 600; color: var(--text-primary); }
.provider-status { font-size: 0.68rem; display: flex; align-items: center; gap: 4px; }
.provider-dot { width: 5px; height: 5px; border-radius: 50%; flex-shrink: 0; }
.dot-ok { background: var(--status-ok); box-shadow: 0 0 4px rgba(94,196,160,0.5); }
.dot-missing { background: var(--text-muted); }
.status-ok { color: var(--status-ok); }
.status-missing { color: var(--text-muted); }

/* save row */
.save-row { display: flex; gap: 8px; align-items: center; padding-top: 4px; }
.btn-save {
  padding: 6px 16px; border-radius: var(--radius-sm); font-size: 0.78rem; font-weight: 700;
  background: var(--primary); color: var(--bg-deep); border: none; cursor: pointer;
  transition: background 0.15s;
}
.btn-save:hover { background: #a0c8e8; }
.btn-persist {
  padding: 6px 14px; border-radius: var(--radius-sm); font-size: 0.78rem; font-weight: 600;
  background: none; color: var(--text-secondary); border: 1px solid var(--border); cursor: pointer;
  transition: background 0.1s, color 0.1s;
}
.btn-persist:hover { background: var(--bg-hover); color: var(--text-primary); }
.saved-badge { font-size: 0.72rem; color: var(--status-ok); }

/* lastfm sync */
.sync-row { display: flex; gap: 8px; align-items: center; }
.sync-msg { font-size: 0.72rem; color: var(--text-secondary); }
</style>
```

- [ ] **Step 2: Replace the template's outer wrapper and add sub-nav**

In `settings/+page.svelte`, wrap the existing content in a sub-nav layout. Find the top-level `<div class="settings-page">` and add a sub-nav sidebar. The existing section content gets moved into a `.settings-content` div on the right. The sub-nav is purely visual state — add to the script block:

```typescript
let activeSection: 'library' | 'providers' | 'enrichment' | 'tools' | 'lastfm' = 'library';
```

Then replace the outer template structure with:

```svelte
<svelte:head><title>Settings · Cassette</title></svelte:head>

<div class="settings-page">
  <div class="settings-layout">
    <nav class="settings-subnav">
      <div class="subnav-item" class:active={activeSection === 'library'}   on:click={() => activeSection = 'library'}   role="button" tabindex="0" on:keydown={(e) => e.key === 'Enter' && (activeSection = 'library')}>Library</div>
      <div class="subnav-item" class:active={activeSection === 'providers'} on:click={() => activeSection = 'providers'} role="button" tabindex="0" on:keydown={(e) => e.key === 'Enter' && (activeSection = 'providers')}>Providers</div>
      <div class="subnav-item" class:active={activeSection === 'enrichment'} on:click={() => activeSection = 'enrichment'} role="button" tabindex="0" on:keydown={(e) => e.key === 'Enter' && (activeSection = 'enrichment')}>Enrichment</div>
      <div class="subnav-item" class:active={activeSection === 'tools'}     on:click={() => activeSection = 'tools'}     role="button" tabindex="0" on:keydown={(e) => e.key === 'Enter' && (activeSection = 'tools')}>Tools</div>
      <div class="subnav-item" class:active={activeSection === 'lastfm'}    on:click={() => activeSection = 'lastfm'}    role="button" tabindex="0" on:keydown={(e) => e.key === 'Enter' && (activeSection = 'lastfm')}>Last.fm</div>
    </nav>

    <div class="settings-content">
      <!-- LIBRARY -->
      {#if activeSection === 'library'}
        <div class="settings-section">
          <div class="section-title">Library Roots</div>
          <div class="section-sub">Folders Cassette scans for music files.</div>
          <div class="roots-list">
            {#each $libraryRoots as root}
              <div class="root-row">
                <span class="root-path">{root.path}</span>
                <button class="root-remove" on:click={() => removeLibraryRoot(root.path)}>Remove</button>
              </div>
            {/each}
          </div>
          <div class="root-actions">
            <button class="action-btn" on:click={pickFolder}>+ Add Folder</button>
            <button class="action-btn" on:click={scanLibrary} disabled={$isScanning}>
              {$isScanning ? 'Scanning…' : 'Scan Library'}
            </button>
          </div>
        </div>
      {/if}

      <!-- PROVIDERS -->
      {#if activeSection === 'providers'}
        <div class="settings-section">
          <div class="section-title">Provider Status</div>
          <div class="provider-grid">
            {#each $providerStatuses as p}
              <div class="provider-card" class:configured={p.configured}>
                <div class="provider-name">{p.label}</div>
                <div class="provider-status">
                  <span class="provider-dot" class:dot-ok={p.configured} class:dot-missing={!p.configured}></span>
                  <span class:status-ok={p.configured} class:status-missing={!p.configured}>
                    {p.configured ? 'Configured' : 'Not configured'}
                  </span>
                </div>
              </div>
            {/each}
          </div>
        </div>

        <div class="settings-section">
          <div class="section-title">Soulseek</div>
          <div class="field-group">
            <div class="field"><label>URL</label><input class="input" bind:value={cfg.slskd_url} placeholder="http://localhost:5030" /></div>
            <div class="field"><label>Username</label><input class="input" bind:value={cfg.slskd_user} placeholder="slskd user" /></div>
            <div class="field field-full"><label>Password</label><input class="input" type="password" bind:value={cfg.slskd_pass} /></div>
            <div class="field field-full"><label>Downloads Dir</label><input class="input" bind:value={cfg.slskd_downloads_dir} placeholder="C:/slskd/downloads" /></div>
          </div>
        </div>

        <div class="settings-section">
          <div class="section-title">Deezer</div>
          <div class="field-group">
            <div class="field field-full"><label>ARL Cookie</label><input class="input" type="password" bind:value={cfg.deezer_arl} /></div>
          </div>
        </div>

        <div class="settings-section">
          <div class="section-title">Qobuz</div>
          <div class="field-group">
            <div class="field"><label>Email</label><input class="input" bind:value={cfg.qobuz_email} placeholder="email@example.com" /></div>
            <div class="field"><label>Password</label><input class="input" type="password" bind:value={cfg.qobuz_password} /></div>
          </div>
        </div>

        <div class="settings-section">
          <div class="section-title">Real-Debrid &amp; Torrents</div>
          <div class="field-group">
            <div class="field"><label>Real-Debrid API Key</label><input class="input" type="password" bind:value={cfg.real_debrid_key} /></div>
            <div class="field"><label>Jackett URL</label><input class="input" bind:value={cfg.jackett_url} placeholder="http://localhost:9117" /></div>
            <div class="field field-full"><label>Jackett API Key</label><input class="input" type="password" bind:value={cfg.jackett_api_key} /></div>
          </div>
        </div>

        <div class="settings-section">
          <div class="section-title">Usenet</div>
          <div class="field-group">
            <div class="field"><label>NZBGeek API Key</label><input class="input" type="password" bind:value={cfg.nzbgeek_api_key} /></div>
            <div class="field"><label>SABnzbd URL</label><input class="input" bind:value={cfg.sabnzbd_url} placeholder="http://localhost:8080" /></div>
            <div class="field field-full"><label>SABnzbd API Key</label><input class="input" type="password" bind:value={cfg.sabnzbd_api_key} /></div>
          </div>
        </div>
      {/if}

      <!-- ENRICHMENT -->
      {#if activeSection === 'enrichment'}
        <div class="settings-section">
          <div class="section-title">Spotify</div>
          <div class="section-sub">Spotify and Genius credentials for metadata enrichment.</div>
          <div class="field-group">
            <div class="field"><label>Client ID</label><input class="input" bind:value={cfg.spotify_client_id} placeholder="Spotify app client ID" /></div>
            <div class="field"><label>Client Secret</label><input class="input" type="password" bind:value={cfg.spotify_client_secret} /></div>
            <div class="field field-full"><label>Access Token</label><input class="input" type="password" bind:value={cfg.spotify_access_token} /></div>
          </div>
        </div>

        <div class="settings-section">
          <div class="section-title">Metadata APIs</div>
          <div class="field-group">
            <div class="field"><label>Genius Token</label><input class="input" type="password" bind:value={cfg.genius_token} /></div>
            <div class="field"><label>Discogs Token</label><input class="input" type="password" bind:value={cfg.discogs_token} /></div>
          </div>
        </div>
      {/if}

      <!-- LAST.FM -->
      {#if activeSection === 'lastfm'}
        <div class="settings-section">
          <div class="section-title">Last.fm</div>
          <div class="field-group">
            <div class="field"><label>API Key</label><input class="input" type="password" bind:value={cfg.lastfm_api_key} /></div>
            <div class="field"><label>Username</label><input class="input" bind:value={cfg.lastfm_username} placeholder="your_lastfm_username" /></div>
          </div>
          <div class="sync-row">
            <button class="action-btn" on:click={syncLastfm} disabled={lastfmSyncing}>
              {lastfmSyncing ? 'Syncing…' : 'Sync Last.fm History'}
            </button>
            {#if lastfmSyncMessage}
              <span class="sync-msg">{lastfmSyncMessage}</span>
            {/if}
          </div>
        </div>
      {/if}

      <!-- TOOLS -->
      {#if activeSection === 'tools'}
        <div class="settings-section">
          <div class="section-title">Tools</div>
          <div class="section-sub">Paths to local tool binaries. Leave blank to use system PATH or built-in defaults.</div>
          <div class="field-group">
            <div class="field"><label>yt-dlp Binary Path</label><input class="input" bind:value={cfg.ytdlp_path} placeholder="C:/tools/yt-dlp.exe" /></div>
            <div class="field"><label>7-Zip Binary Path</label><input class="input" bind:value={cfg.sevenzip_path} placeholder="C:/Program Files/7-Zip/7z.exe" /></div>
          </div>
        </div>
      {/if}

      <div class="save-row">
        <button class="btn-save" on:click={handleSave}>{saved ? '✓ Saved' : 'Save Settings'}</button>
        <button class="btn-persist" on:click={persistEffectiveSecrets} disabled={persistingEffective}>
          {persistingEffective ? 'Persisting…' : 'Persist Effective Config'}
        </button>
        {#if saved}<span class="saved-badge">✓</span>{/if}
      </div>
    </div>
  </div>
</div>
```

- [ ] **Step 3: Build**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -15
```

Fix any template errors. The existing `syncLastfm`, `handleSave`, `persistEffectiveSecrets`, `pickFolder`, `scanLibrary` functions are already in the script block — this step only changes the template and style.

- [ ] **Step 4: Commit**

```bash
cd "c:/Cassette Music"
git add ui/src/routes/settings/+page.svelte
git commit -m "feat(ui): bespoke settings layout with sub-nav and provider status grid"
```

---

## Task 11: Full build and smoke test

- [ ] **Step 1: Full workspace compile**

```bash
cd "c:/Cassette Music" && cargo check --workspace 2>&1 | tail -10
```

Expected: `Finished` with no errors.

- [ ] **Step 2: Run Rust tests**

```bash
cd "c:/Cassette Music" && cargo test --workspace 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 3: Final UI build**

```bash
cd "c:/Cassette Music/ui" && npm run build 2>&1 | tail -15
```

Expected: build succeeds. The existing a11y warning in `downloads/+page.svelte` is pre-existing — all other warnings should be zero.

- [ ] **Step 4: Run smoke test**

```powershell
.\scripts\smoke_desktop.ps1
```

Expected: passes.

- [ ] **Step 5: Final commit**

```bash
cd "c:/Cassette Music"
git add -A
git commit -m "feat(ui): Steel Dusk redesign — all surfaces complete"
```

---

## Self-Review

**Spec coverage check:**

| Spec requirement | Task |
|---|---|
| Steel Dusk palette tokens | Task 1 |
| Inter font | Task 1 |
| Topbar chrome | Task 2 |
| Sidebar logo block + nav | Task 3 |
| Now-playing bar, gradient seek, primary play btn | Task 4 |
| Right sidebar tabs | Task 5 |
| Queue panel | Task 5 |
| `tintFromHex` utility | Task 6 |
| `dominant_color_hex` on Album (Rust) | Task 7 |
| `dominant_color_hex` on Album (TS) | Task 8 |
| Album grid with per-card tinting | Task 9 |
| Album detail blurred backdrop | Task 9 |
| Settings sub-nav | Task 10 |
| Provider status cards | Task 10 |
| Field grid layout | Task 10 |
| Build + smoke test | Task 11 |

All spec requirements covered. No placeholders. Types consistent throughout (`dominant_color_hex: Option<String>` in Rust, `string | null` in TS, `tintFromHex(album.dominant_color_hex)` in template).
