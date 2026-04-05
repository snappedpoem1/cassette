<script lang="ts">
  import { page } from '$app/stores';
  import { trackCount, isScanning, scanProgress } from '$lib/stores/library';

  const coreLinks = [
    { href: '/', label: 'Library', icon: 'LIB' },
    { href: '/downloads', label: 'Downloads', icon: 'DL' },
    { href: '/playlists', label: 'Playlists', icon: 'PL' },
    { href: '/artists', label: 'Artists', icon: 'AR' },
  ];

  const utilityLinks = [
    { href: '/import', label: 'Import', icon: 'IM' },
    { href: '/tools', label: 'Tools', icon: 'TL' },
    { href: '/settings', label: 'Settings', icon: 'CFG' },
  ];

  function isActive(href: string, pathname: string): boolean {
    if (href === '/') {
      return pathname === '/';
    }
    return pathname === href || pathname.startsWith(`${href}/`);
  }
</script>

<nav class="sidebar">
  <div class="sidebar-logo">
    <span class="logo-icon">Tape</span>
    <span class="logo-text">Cassette</span>
  </div>

  <ul class="nav-list">
    {#each coreLinks as link}
      {@const active = isActive(link.href, $page.url.pathname)}
      <li>
        <a href={link.href} class="nav-link" class:active>
          <span class="nav-icon">{link.icon}</span>
          <span>{link.label}</span>
        </a>
      </li>
    {/each}
  </ul>

  <div class="nav-section-title">Utilities</div>
  <ul class="nav-list nav-list-utilities">
    {#each utilityLinks as link}
      {@const active = isActive(link.href, $page.url.pathname)}
      <li>
        <a href={link.href} class="nav-link" class:active>
          <span class="nav-icon">{link.icon}</span>
          <span>{link.label}</span>
        </a>
      </li>
    {/each}
  </ul>

  <div class="sidebar-footer">
    {#if $isScanning}
      <div class="sidebar-scan">
        <div class="scan-label">
          <span>Scanning...</span>
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

    <div class="sidebar-stats">
      <span class="stat-value">{$trackCount.toLocaleString()}</span>
      <span class="stat-label">tracks</span>
    </div>
  </div>
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding-bottom: 12px;
    user-select: none;
  }

  .sidebar-logo {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 20px 16px 16px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 8px;
  }

  .logo-icon {
    font-size: 0.8rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--accent-bright);
  }

  .logo-text {
    font-size: 1.1rem;
    font-weight: 700;
    letter-spacing: -0.02em;
  }

  .nav-list {
    list-style: none;
    margin: 0;
    padding: 0 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
  }

  .nav-list-utilities {
    flex: 0;
    margin-bottom: 8px;
  }

  .nav-section-title {
    margin: 8px 14px 4px;
    color: var(--text-muted);
    font-size: 0.68rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    font-weight: 700;
  }

  .nav-link {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 10px;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    font-size: 0.9rem;
    font-weight: 500;
    transition: background 0.1s, color 0.1s;
  }

  .nav-link:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .nav-link.active {
    background: var(--bg-active);
    color: var(--accent-bright);
  }

  .nav-icon {
    width: 44px;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: inherit;
  }

  .sidebar-footer {
    margin-top: auto;
    padding: 12px 8px 0;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .sidebar-stats {
    display: flex;
    align-items: baseline;
    gap: 6px;
    padding: 4px 8px;
  }

  .stat-value {
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--text-primary);
  }

  .stat-label {
    font-size: 0.7rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .sidebar-scan {
    padding: 8px 10px;
    background: var(--bg-card);
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
  }

  .scan-label {
    display: flex;
    justify-content: space-between;
    font-size: 0.75rem;
    color: var(--text-secondary);
    margin-bottom: 6px;
  }

  .scan-track {
    height: 3px;
    background: var(--bg-active);
    border-radius: 99px;
    overflow: hidden;
  }

  .scan-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 99px;
    transition: width 0.3s;
    min-width: 8px;
  }

  .scan-file {
    font-size: 0.7rem;
    color: var(--text-muted);
    margin-top: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
