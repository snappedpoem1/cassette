<script lang="ts">
  import { onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import {
    libraryRoots,
    isScanning,
    scanProgress,
    addLibraryRoot,
    removeLibraryRoot,
    scanLibrary,
  } from '$lib/stores/library';
  import {
    downloadConfig,
    providerStatuses,
    loadDownloadConfig,
    saveDownloadConfig,
    persistEffectiveDownloadConfig,
  } from '$lib/stores/downloads';
  import { api } from '$lib/api/tauri';
  import type { DownloadConfig, PolicyProfile, SlskdRuntimeStatus } from '$lib/api/tauri';

  onMount(async () => {
    await loadDownloadConfig();
    await loadSlskdRuntimeStatus();
    await loadPolicyProfile();
  });

  let cfg: DownloadConfig = {
    library_base: '',
    staging_folder: '',
    slskd_url: null,
    slskd_user: null,
    slskd_pass: null,
    slskd_downloads_dir: null,
    real_debrid_key: null,
    jackett_url: null,
    jackett_api_key: null,
    nzbgeek_api_key: null,
    sabnzbd_url: null,
    sabnzbd_api_key: null,
    qobuz_email: null,
    qobuz_password: null,
    deezer_arl: null,
    spotify_client_id: null,
    spotify_client_secret: null,
    spotify_access_token: null,
    genius_token: null,
    discogs_token: null,
    lastfm_api_key: null,
    lastfm_username: null,
    ytdlp_path: null,
    sevenzip_path: null,
  };

  $: if ($downloadConfig) {
    cfg = { ...$downloadConfig };
  }

  let activeSection: 'library' | 'providers' | 'enrichment' | 'tools' | 'lastfm' = 'library';
  let saved = false;
  let persistingEffective = false;
  let lastfmSyncing = false;
  let lastfmSyncMessage: string | null = null;
  let slskdRuntime: SlskdRuntimeStatus | null = null;
  let slskdRuntimeBusy = false;
  let policyProfile: PolicyProfile = 'balanced_auto';
  let policyProfileSaving = false;
  let policyProfileMessage: string | null = null;

  async function loadSlskdRuntimeStatus() {
    try {
      slskdRuntime = await api.getSlskdRuntimeStatus();
    } catch (error) {
      slskdRuntime = {
        running: false,
        ready: false,
        spawned_by_app: false,
        binary_found: false,
        binary_path: null,
        app_dir: null,
        downloads_dir: null,
        url: cfg.slskd_url ?? 'http://localhost:5030',
        message: error instanceof Error ? error.message : 'slskd status unavailable',
      };
    }
  }

  async function handleSave() {
    await saveDownloadConfig(cfg);
    await loadSlskdRuntimeStatus();
    saved = true;
    setTimeout(() => {
      saved = false;
    }, 2000);
  }

  async function persistEffectiveSecrets() {
    persistingEffective = true;
    try {
      await persistEffectiveDownloadConfig();
      await loadSlskdRuntimeStatus();
      saved = true;
      setTimeout(() => {
        saved = false;
      }, 2000);
    } finally {
      persistingEffective = false;
    }
  }

  async function syncLastfmHistory() {
    lastfmSyncing = true;
    lastfmSyncMessage = null;
    try {
      const inserted = await api.syncLastfmHistory(cfg.lastfm_username ?? undefined, 200);
      lastfmSyncMessage = `Synced ${inserted} new Last.fm plays.`;
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Last.fm sync failed';
      lastfmSyncMessage = message;
    } finally {
      lastfmSyncing = false;
    }
  }

  async function pickFolder() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === 'string') {
      await addLibraryRoot(selected);
    }
  }

  async function handleScan() {
    await scanLibrary();
  }

  async function restartSlskdRuntime() {
    slskdRuntimeBusy = true;
    try {
      slskdRuntime = await api.restartSlskdRuntime();
    } finally {
      slskdRuntimeBusy = false;
    }
  }

  async function stopSlskdRuntime() {
    slskdRuntimeBusy = true;
    try {
      slskdRuntime = await api.stopSlskdRuntime();
    } finally {
      slskdRuntimeBusy = false;
    }
  }

  async function loadPolicyProfile() {
    try {
      policyProfile = await api.getPolicyProfile();
      policyProfileMessage = null;
    } catch (error) {
      policyProfileMessage = error instanceof Error ? error.message : 'Policy profile unavailable';
    }
  }

  async function applyPolicyProfile() {
    policyProfileSaving = true;
    try {
      policyProfile = await api.setPolicyProfile(policyProfile);
      policyProfileMessage = 'Policy profile applied to runtime director behavior.';
    } catch (error) {
      policyProfileMessage = error instanceof Error ? error.message : 'Failed to apply policy profile';
    } finally {
      policyProfileSaving = false;
    }
  }
</script>

<svelte:head><title>Settings · Cassette</title></svelte:head>

<div class="settings-page">
  <div class="settings-layout">
    <nav class="settings-subnav">
      <!-- svelte-ignore a11y-no-static-element-interactions -->
      <div class="subnav-item" class:active={activeSection === 'library'}    on:click={() => activeSection = 'library'}    role="button" tabindex="0" on:keydown={(e) => e.key === 'Enter' && (activeSection = 'library')}>Library</div>
      <!-- svelte-ignore a11y-no-static-element-interactions -->
      <div class="subnav-item" class:active={activeSection === 'providers'}  on:click={() => activeSection = 'providers'}  role="button" tabindex="0" on:keydown={(e) => e.key === 'Enter' && (activeSection = 'providers')}>Providers</div>
      <!-- svelte-ignore a11y-no-static-element-interactions -->
      <div class="subnav-item" class:active={activeSection === 'enrichment'} on:click={() => activeSection = 'enrichment'} role="button" tabindex="0" on:keydown={(e) => e.key === 'Enter' && (activeSection = 'enrichment')}>Enrichment</div>
      <!-- svelte-ignore a11y-no-static-element-interactions -->
      <div class="subnav-item" class:active={activeSection === 'tools'}      on:click={() => activeSection = 'tools'}      role="button" tabindex="0" on:keydown={(e) => e.key === 'Enter' && (activeSection = 'tools')}>Tools</div>
      <!-- svelte-ignore a11y-no-static-element-interactions -->
      <div class="subnav-item" class:active={activeSection === 'lastfm'}     on:click={() => activeSection = 'lastfm'}     role="button" tabindex="0" on:keydown={(e) => e.key === 'Enter' && (activeSection = 'lastfm')}>Last.fm</div>
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
            {#if $libraryRoots.length === 0}
              <div class="roots-empty">No folders added yet.</div>
            {/if}
          </div>
          <div class="root-actions">
            <button class="action-btn" on:click={pickFolder}>+ Add Folder</button>
            <button class="action-btn" on:click={handleScan} disabled={$isScanning || $libraryRoots.length === 0}>
              {$isScanning ? 'Scanning…' : 'Scan Library'}
            </button>
          </div>
          {#if $isScanning && $scanProgress}
            <div class="scan-status">
              <div class="scan-row">
                <span>{$scanProgress.scanned.toLocaleString()} tracks found</span>
              </div>
              <div class="scan-bar-track">
                <div
                  class="scan-bar-fill"
                  style:width={$scanProgress.total > 0
                    ? `${($scanProgress.scanned / $scanProgress.total) * 100}%`
                    : '0%'}
                ></div>
              </div>
              <div class="scan-file">
                {$scanProgress.current_file.split(/[/\\]/).pop()}
              </div>
            </div>
          {/if}
        </div>

        <div class="settings-section">
          <div class="section-title">Download Paths</div>
          <div class="field-group">
            <div class="field field-full">
              <label>Library Base<input bind:value={cfg.library_base} placeholder="e.g. A:/music" /></label>
            </div>
            <div class="field field-full">
              <label>Staging Folder<input bind:value={cfg.staging_folder} placeholder="e.g. C:/Users/Admin/staging" /></label>
            </div>
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
                {#if p.missing_fields && p.missing_fields.length > 0}
                  <div class="provider-missing-hint">Missing: {p.missing_fields.join(', ')}</div>
                {/if}
              </div>
            {/each}
          </div>
        </div>

        <div class="settings-section">
          <div class="section-title">Soulseek</div>
          {#if slskdRuntime}
            <div class="provider-runtime-card" class:runtime-ready={slskdRuntime.ready}>
              <div class="runtime-row">
                <div class="runtime-title">Bundled slskd runtime</div>
                <div class="runtime-actions">
                  <button class="action-btn" on:click={loadSlskdRuntimeStatus} disabled={slskdRuntimeBusy}>Refresh</button>
                  <button class="action-btn" on:click={restartSlskdRuntime} disabled={slskdRuntimeBusy}>
                    {slskdRuntimeBusy ? 'Working...' : 'Restart'}
                  </button>
                  {#if slskdRuntime.spawned_by_app}
                    <button class="action-btn" on:click={stopSlskdRuntime} disabled={slskdRuntimeBusy}>Stop</button>
                  {/if}
                </div>
              </div>
              <div class="runtime-summary">
                {#if slskdRuntime.ready}
                  Cassette sees slskd at {slskdRuntime.url}
                  {#if slskdRuntime.spawned_by_app} and started it itself.{:else} from an existing process.{/if}
                {:else if slskdRuntime.running}
                  slskd is starting, but the endpoint is not reachable yet.
                {:else}
                  slskd is not reachable right now.
                {/if}
              </div>
              {#if slskdRuntime.message}
                <div class="runtime-note">{slskdRuntime.message}</div>
              {/if}
              <div class="runtime-meta">
                <span>binary: {slskdRuntime.binary_path ?? 'not found'}</span>
                <span>app dir: {slskdRuntime.app_dir ?? 'n/a'}</span>
                <span>downloads: {slskdRuntime.downloads_dir ?? 'n/a'}</span>
              </div>
            </div>
          {/if}
          <div class="field-group">
            <div class="field"><label>URL<input bind:value={cfg.slskd_url} placeholder="http://localhost:5030" /></label></div>
            <div class="field"><label>Username<input bind:value={cfg.slskd_user} placeholder="slskd user" /></label></div>
            <div class="field field-full"><label>Password<input type="password" bind:value={cfg.slskd_pass} /></label></div>
            <div class="field field-full"><label>Downloads Dir<input bind:value={cfg.slskd_downloads_dir} placeholder="C:/slskd/downloads" /></label></div>
          </div>
        </div>

        <div class="settings-section">
          <div class="section-title">Deezer</div>
          <div class="field-group">
            <div class="field field-full"><label>ARL Cookie<input type="password" bind:value={cfg.deezer_arl} /></label></div>
          </div>
        </div>

        <div class="settings-section">
          <div class="section-title">Qobuz</div>
          <div class="field-group">
            <div class="field"><label>Email<input bind:value={cfg.qobuz_email} placeholder="email@example.com" /></label></div>
            <div class="field"><label>Password<input type="password" bind:value={cfg.qobuz_password} /></label></div>
          </div>
        </div>

        <div class="settings-section">
          <div class="section-title">Real-Debrid &amp; Torrents</div>
          <div class="field-group">
            <div class="field"><label>Real-Debrid API Key<input type="password" bind:value={cfg.real_debrid_key} /></label></div>
            <div class="field"><label>Jackett URL<input bind:value={cfg.jackett_url} placeholder="http://localhost:9117" /></label></div>
            <div class="field field-full"><label>Jackett API Key<input type="password" bind:value={cfg.jackett_api_key} /></label></div>
          </div>
        </div>

        <div class="settings-section">
          <div class="section-title">Usenet</div>
          <div class="field-group">
            <div class="field"><label>NZBGeek API Key<input type="password" bind:value={cfg.nzbgeek_api_key} /></label></div>
            <div class="field"><label>SABnzbd URL<input bind:value={cfg.sabnzbd_url} placeholder="http://localhost:8080" /></label></div>
            <div class="field field-full"><label>SABnzbd API Key<input type="password" bind:value={cfg.sabnzbd_api_key} /></label></div>
          </div>
        </div>
      {/if}

      <!-- ENRICHMENT -->
      {#if activeSection === 'enrichment'}
        <div class="settings-section">
          <div class="section-title">Spotify</div>
          <div class="section-sub">Spotify credentials for metadata enrichment.</div>
          <div class="field-group">
            <div class="field"><label>Client ID<input bind:value={cfg.spotify_client_id} placeholder="Spotify app client ID" /></label></div>
            <div class="field"><label>Client Secret<input type="password" bind:value={cfg.spotify_client_secret} /></label></div>
            <div class="field field-full"><label>Access Token<input type="password" bind:value={cfg.spotify_access_token} /></label></div>
          </div>
        </div>

        <div class="settings-section">
          <div class="section-title">Metadata APIs</div>
          <div class="field-group">
            <div class="field"><label>Genius Token<input type="password" bind:value={cfg.genius_token} /></label></div>
            <div class="field"><label>Discogs Token<input type="password" bind:value={cfg.discogs_token} /></label></div>
          </div>
        </div>
      {/if}

      <!-- LAST.FM -->
      {#if activeSection === 'lastfm'}
        <div class="settings-section">
          <div class="section-title">Last.fm</div>
          <div class="field-group">
            <div class="field"><label>API Key<input type="password" bind:value={cfg.lastfm_api_key} /></label></div>
            <div class="field"><label>Username<input bind:value={cfg.lastfm_username} placeholder="your_lastfm_username" /></label></div>
          </div>
          <div class="sync-row">
            <button class="action-btn" on:click={syncLastfmHistory} disabled={lastfmSyncing}>
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
          <div class="section-title">Policy Profile</div>
          <div class="section-sub">Choose deterministic runtime behavior for playback pressure, queue throughput, and retry cadence.</div>
          <div class="field-group">
            <div class="field field-full">
              <label>
                Active Profile
                <select bind:value={policyProfile} class="select-field">
                  <option value="playback_first">Playback-First</option>
                  <option value="balanced_auto">Balanced Auto</option>
                  <option value="aggressive_overnight">Aggressive Overnight</option>
                </select>
              </label>
            </div>
          </div>
          <div class="sync-row">
            <button class="action-btn" on:click={applyPolicyProfile} disabled={policyProfileSaving}>
              {policyProfileSaving ? 'Applying…' : 'Apply Profile'}
            </button>
            {#if policyProfileMessage}
              <span class="sync-msg">{policyProfileMessage}</span>
            {/if}
          </div>
        </div>

        <div class="settings-section">
          <div class="section-title">Tools</div>
          <div class="section-sub">Paths to local tool binaries. Leave blank to use system PATH or built-in defaults.</div>
          <div class="field-group">
            <div class="field"><label>yt-dlp Binary Path<input bind:value={cfg.ytdlp_path} placeholder="C:/tools/yt-dlp.exe" /></label></div>
            <div class="field"><label>7-Zip Binary Path<input bind:value={cfg.sevenzip_path} placeholder="C:/Program Files/7-Zip/7z.exe" /></label></div>
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
.field label { display: flex; flex-direction: column; gap: 3px; font-size: 0.68rem; color: var(--text-secondary); font-weight: 600; letter-spacing: 0.04em; }
.field input {
  background: var(--bg-card); border: 1px solid var(--border);
  border-radius: var(--radius-sm); padding: 5px 9px;
  font-size: 0.8rem; color: var(--text-primary); font-family: inherit;
  transition: border-color 0.15s;
}
.field .select-field {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  padding: 5px 9px;
  font-size: 0.8rem;
  color: var(--text-primary);
  font-family: inherit;
}
.field .select-field:focus { outline: none; border-color: var(--primary-dim); }
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
.roots-empty { font-size: 0.8rem; color: var(--text-muted); padding: 4px 0; }

.root-actions { display: flex; gap: 6px; }
.action-btn {
  padding: 5px 11px; border-radius: var(--radius-sm); font-size: 0.72rem; font-weight: 700;
  background: rgba(139,180,212,0.08); color: var(--primary);
  border: 1px solid rgba(139,180,212,0.18); cursor: pointer;
  transition: background 0.1s;
}
.action-btn:hover { background: rgba(139,180,212,0.14); }
.action-btn:disabled { opacity: 0.5; cursor: not-allowed; }

/* scan progress */
.scan-status {
  padding: 8px 10px; background: var(--bg-card); border-radius: var(--radius-sm);
  border: 1px solid var(--border); display: flex; flex-direction: column; gap: 4px;
}
.scan-row { font-size: 0.75rem; color: var(--text-secondary); }
.scan-bar-track { height: 3px; background: var(--border); border-radius: 2px; overflow: hidden; }
.scan-bar-fill { height: 100%; background: var(--primary); border-radius: 2px; transition: width 0.3s; }
.scan-file { font-size: 0.7rem; color: var(--text-muted); font-family: monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

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
.provider-missing-hint { font-size: 0.65rem; color: var(--text-muted); margin-top: 2px; }

.provider-runtime-card {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 12px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: var(--bg-card);
}
.provider-runtime-card.runtime-ready {
  border-color: rgba(94, 196, 160, 0.35);
}
.runtime-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.runtime-title {
  font-size: 0.8rem;
  font-weight: 700;
  color: var(--text-primary);
}
.runtime-actions {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}
.runtime-summary {
  font-size: 0.76rem;
  color: var(--text-secondary);
}
.runtime-note {
  font-size: 0.72rem;
  color: var(--text-muted);
}
.runtime-meta {
  display: grid;
  grid-template-columns: 1fr;
  gap: 2px;
  font-size: 0.68rem;
  color: var(--text-muted);
  font-family: monospace;
}

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
.btn-persist:disabled { opacity: 0.5; cursor: not-allowed; }
.saved-badge { font-size: 0.72rem; color: var(--status-ok); }

/* lastfm sync */
.sync-row { display: flex; gap: 8px; align-items: center; }
.sync-msg { font-size: 0.72rem; color: var(--text-secondary); }
</style>
