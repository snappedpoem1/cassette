<script lang="ts">
  import { page } from '$app/stores';
  import { trackCount, isScanning, scanProgress } from '$lib/stores/library';

  const coreLinks = [
    { href: '/', label: 'Home',      icon: 'HM' },
    { href: '/artists', label: 'Artists',   icon: 'AR' },
    { href: '/library', label: 'Library',   icon: 'LIB' },
    { href: '/downloads', label: 'Downloads', icon: 'DL' },
    { href: '/playlists', label: 'Playlists', icon: 'PL' },
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
