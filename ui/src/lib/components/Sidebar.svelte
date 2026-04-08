<script lang="ts">
  import { page } from '$app/stores';
  import { trackCount, albums, isScanning, scanProgress } from '$lib/stores/library';

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
      <span class="stat-sep">·</span>
      <span class="stat-value">{$albums.length.toLocaleString()}</span>
      <span class="stat-label">albums</span>
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
  width: 20px;
  height: 20px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: inherit;
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
.stat-sep { font-size: 0.65rem; color: var(--text-muted); margin: 0 2px; }

.scan-widget {
  padding: 7px 8px; background: var(--bg-card);
  border-radius: var(--radius-sm); border: 1px solid var(--border);
}
.scan-label { display: flex; justify-content: space-between; font-size: 0.68rem; color: var(--text-secondary); margin-bottom: 5px; }
.scan-track { height: 2px; background: var(--bg-active); border-radius: 99px; overflow: hidden; }
.scan-fill { height: 100%; background: var(--primary); border-radius: 99px; transition: width 0.3s; min-width: 6px; }
.scan-file { font-size: 0.62rem; color: var(--text-muted); margin-top: 3px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
