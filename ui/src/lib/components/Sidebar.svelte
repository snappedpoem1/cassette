<script lang="ts">
  import { page } from '$app/stores';
  import { trackCount, albums, isScanning, scanProgress } from '$lib/stores/library';
  import CassetteLogo from '$lib/components/CassetteLogo.svelte';
  import { isPlaying } from '$lib/stores/player';

  const listeningLinks = [
    {
      href: '/',
      label: 'Home',
      icon: `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>`,
    },
    {
      href: '/collection',
      label: 'Collection',
      icon: `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="4" y="3" width="6" height="18" rx="2"/><rect x="14" y="3" width="6" height="18" rx="2"/></svg>`,
    },
    {
      href: '/now-playing',
      label: 'Now Playing',
      icon: `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M10 9.5v5l5-2.5-5-2.5z"/></svg>`,
    },
    {
      href: '/artists',
      label: 'Artists',
      icon: `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="8" r="5"/><path d="M20 21a8 8 0 1 0-16 0"/></svg>`,
    },
    {
      href: '/playlists',
      label: 'Playlists',
      icon: `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/><line x1="3" y1="6" x2="3.01" y2="6"/><line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/></svg>`,
    },
    {
      href: '/queue',
      label: 'Queue',
      icon: `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 6h12"/><path d="M4 12h16"/><path d="M4 18h10"/><path d="m18 17 3 3 3-3"/></svg>`,
    },
    {
      href: '/crates',
      label: 'Crates',
      icon: `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7h18"/><path d="M5 7l2 12h10l2-12"/><path d="M9 11h6"/><path d="M10 15h4"/></svg>`,
    },
    {
      href: '/session',
      label: 'Session',
      icon: `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>`,
    },
  ];

  const utilityLinks = [
    {
      href: '/workstation',
      label: 'Workstation',
      icon: `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="14" rx="2"/><path d="M8 20h8"/><path d="M12 18v2"/></svg>`,
    },
  ];

  function isActive(href: string, pathname: string): boolean {
    if (href === '/') return pathname === '/';
    return pathname === href || pathname.startsWith(`${href}/`);
  }
</script>

<nav class="sidebar">
  <div class="sidebar-logo">
    <CassetteLogo size={32} spinning={$isPlaying} withWordmark />
  </div>

  <ul class="nav-list">
    <li class="nav-heading">Listen</li>
    {#each listeningLinks as link}
      {@const active = isActive(link.href, $page.url.pathname)}
      <li>
        <a href={link.href} class="nav-item" class:active aria-current={active ? 'page' : undefined}>
          <span class="nav-icon">{@html link.icon}</span>
          <span class="nav-label">{link.label}</span>
        </a>
      </li>
    {/each}
  </ul>

  <div class="nav-divider"></div>

  <ul class="nav-list">
    <li class="nav-heading">Control</li>
    {#each utilityLinks as link}
      {@const active = isActive(link.href, $page.url.pathname)}
      <li>
        <a href={link.href} class="nav-item" class:active aria-current={active ? 'page' : undefined}>
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
    <div class="stat-row">
      <span class="stat-value">{$trackCount.toLocaleString()}</span>
      <span class="stat-label">tracks</span>
      <span class="stat-sep">/</span>
      <span class="stat-value">{$albums.length.toLocaleString()}</span>
      <span class="stat-label">albums</span>
    </div>
  </div>
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding-bottom: 8px;
    user-select: none;
  }

  .sidebar-logo {
    display: flex;
    align-items: center;
    padding: 14px 14px 12px;
    border-bottom: 1px solid var(--border-dim);
    margin-bottom: 8px;
  }

  .nav-list {
    list-style: none;
    margin: 0;
    padding: 0 8px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .nav-heading {
    padding: 8px 8px 4px;
    font-size: 0.62rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--text-muted);
    font-weight: 700;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px 8px 8px;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    font-size: 0.78rem;
    font-weight: 600;
    border-right: 2px solid transparent;
    transition: color 0.1s, background 0.1s, border-color 0.1s;
    text-decoration: none;
    margin-right: -8px;
    padding-right: 16px;
  }

  .nav-item:hover {
    color: var(--text-primary);
    background: rgba(var(--mood-accent-rgb), 0.05);
  }

  .nav-item.active {
    color: rgba(var(--mood-accent-rgb), 1);
    background: linear-gradient(90deg, rgba(var(--mood-accent-rgb), 0.1), transparent 85%);
    border-right-color: rgba(var(--mood-accent-rgb), 0.9);
    transition: color var(--mood-shift-ms) ease, background var(--mood-shift-ms) ease, border-color var(--mood-shift-ms) ease;
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

  .nav-label {
    flex: 1;
  }

  .nav-divider {
    height: 1px;
    background: var(--border-dim);
    margin: 8px 12px;
  }

  .sidebar-footer {
    margin-top: auto;
    padding: 12px 10px 0;
    border-top: 1px solid var(--border-dim);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .stat-row {
    display: flex;
    align-items: baseline;
    gap: 5px;
    padding: 2px 6px;
  }

  .stat-value {
    font-size: 0.85rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  .stat-label {
    font-size: 0.65rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .stat-sep {
    font-size: 0.7rem;
    color: var(--text-muted);
    margin: 0 2px;
  }

  .scan-widget {
    padding: 8px 9px;
    background: var(--bg-card);
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
  }

  .scan-label {
    display: flex;
    justify-content: space-between;
    font-size: 0.68rem;
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
    background: var(--primary);
    border-radius: 99px;
    transition: width 0.3s;
    min-width: 6px;
  }

  .scan-file {
    font-size: 0.66rem;
    color: var(--text-muted);
    margin-top: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  @media (max-width: 960px) {
    .sidebar-logo {
      padding-left: 10px;
      padding-right: 10px;
    }

    .nav-list {
      padding-left: 6px;
      padding-right: 6px;
    }

    .nav-item {
      justify-content: center;
      margin-right: 0;
      padding: 8px;
      border-right-width: 0;
      border-left: 2px solid transparent;
    }

    .nav-item.active {
      border-left-color: rgba(var(--mood-accent-rgb), 0.9);
      background: linear-gradient(90deg, rgba(var(--mood-accent-rgb), 0.12), transparent 78%);
    }

    .nav-label,
    .nav-heading,
    .stat-label,
    .stat-sep {
      display: none;
    }

    .scan-widget {
      padding: 6px;
    }

    .scan-label {
      justify-content: center;
    }

    .sidebar-footer {
      padding-left: 6px;
      padding-right: 6px;
    }

    .stat-row {
      justify-content: center;
      gap: 7px;
    }
  }
</style>
