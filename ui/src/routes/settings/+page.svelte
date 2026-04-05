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
  import type { DownloadConfig } from '$lib/api/tauri';

  onMount(async () => {
    await loadDownloadConfig();
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

  let saved = false;
  let persistingEffective = false;
  let lastfmSyncing = false;
  let lastfmSyncMessage: string | null = null;

  async function handleSave() {
    await saveDownloadConfig(cfg);
    saved = true;
    setTimeout(() => {
      saved = false;
    }, 2000);
  }

  async function persistEffectiveSecrets() {
    persistingEffective = true;
    try {
      await persistEffectiveDownloadConfig();
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
</script>

<svelte:head>
  <title>Settings - Cassette</title>
</svelte:head>

<div class="settings-page">
  <div class="page-header">
    <h2>Settings</h2>
  </div>

  <div class="settings-body">
    <section class="settings-section">
      <h3 class="section-title">Provider Readiness</h3>
      <p class="section-desc">
        This rebuild mirrors the provider set surfaced by the recovery pack. Masked secrets will be
        preserved unless you clear or replace them.
      </p>

      <div class="provider-grid">
        {#each $providerStatuses as provider}
          <article class="provider-card" class:provider-card-ready={provider.configured}>
            <div class="provider-card-header">
              <div>
                <div class="provider-name">{provider.label}</div>
                <div class="provider-summary">{provider.summary}</div>
              </div>
              <span class:badge-success={provider.configured} class:badge-muted={!provider.configured} class="badge">
                {provider.configured ? 'Ready' : 'Needs setup'}
              </span>
            </div>

            {#if provider.missing_fields.length > 0}
              <div class="provider-missing">
                Missing: {provider.missing_fields.join(', ')}
              </div>
            {/if}
          </article>
        {/each}
      </div>
    </section>

    <section class="settings-section">
      <h3 class="section-title">Library Folders</h3>
      <p class="section-desc">Cassette will scan these folders for music files.</p>

      <div class="roots-list">
        {#each $libraryRoots as root}
          <div class="root-row">
            <span class="root-icon">Folder</span>
            <span class="root-path">{root.path}</span>
            <button class="remove-btn" on:click={() => removeLibraryRoot(root.path)} title="Remove">
              Remove
            </button>
          </div>
        {/each}
        {#if $libraryRoots.length === 0}
          <div class="roots-empty">No folders added yet.</div>
        {/if}
      </div>

      <div class="roots-actions">
        <button class="btn btn-ghost" on:click={pickFolder}>Add Folder</button>
        <button
          class="btn btn-primary"
          on:click={handleScan}
          disabled={$isScanning || $libraryRoots.length === 0}
        >
          {#if $isScanning}
            <div class="spinner" style="width:14px;height:14px;border-width:2px;"></div>
            Scanning...
          {:else}
            Scan Library
          {/if}
        </button>
      </div>

      {#if $isScanning && $scanProgress}
        <div class="scan-status card" style="margin-top:12px;">
          <div class="scan-row">
            <span>{$scanProgress.scanned.toLocaleString()} tracks found</span>
          </div>
          <div class="scan-bar-track" style="margin-top:6px;">
            <div
              class="scan-bar-fill"
              style:width={$scanProgress.total > 0
                ? `${($scanProgress.scanned / $scanProgress.total) * 100}%`
                : '0%'}
            ></div>
          </div>
          <div class="scan-file" style="margin-top:4px;font-size:0.75rem;color:var(--text-muted);">
            {$scanProgress.current_file.split(/[/\\]/).pop()}
          </div>
        </div>
      {/if}
    </section>

    <section class="settings-section">
      <h3 class="section-title">Download Paths</h3>

      <div class="field-group">
        <label class="field-label" for="library-base">Library Base</label>
        <input id="library-base" class="input" bind:value={cfg.library_base} placeholder="e.g. A:/music" />
      </div>

      <div class="field-group">
        <label class="field-label" for="staging">Staging Folder</label>
        <input
          id="staging"
          class="input"
          bind:value={cfg.staging_folder}
          placeholder="e.g. C:/Users/Admin/staging"
        />
      </div>
    </section>

    <section class="settings-section">
      <h3 class="section-title">Acquisition Providers</h3>

      <div class="field-grid">
        <div class="field-group">
          <label class="field-label" for="slskd-url">slskd URL</label>
          <input id="slskd-url" class="input" bind:value={cfg.slskd_url} placeholder="http://localhost:5030" />
        </div>

        <div class="field-group">
          <label class="field-label" for="slskd-user">slskd Username</label>
          <input id="slskd-user" class="input" bind:value={cfg.slskd_user} placeholder="slskd" />
        </div>

        <div class="field-group">
          <label class="field-label" for="slskd-pass">slskd Password</label>
          <input id="slskd-pass" class="input" type="password" bind:value={cfg.slskd_pass} placeholder="********" />
        </div>

        <div class="field-group">
          <label class="field-label" for="slskd-downloads-dir">slskd Downloads Dir</label>
          <input id="slskd-downloads-dir" class="input" bind:value={cfg.slskd_downloads_dir} placeholder="A:/Staging/slskd" />
        </div>

        <div class="field-group">
          <label class="field-label" for="rd-key">Real-Debrid Key</label>
          <input id="rd-key" class="input" type="password" bind:value={cfg.real_debrid_key} placeholder="********" />
        </div>

        <div class="field-group">
          <label class="field-label" for="jackett-url">Jackett URL</label>
          <input id="jackett-url" class="input" bind:value={cfg.jackett_url} placeholder="http://localhost:9117" />
        </div>

        <div class="field-group">
          <label class="field-label" for="jackett-api-key">Jackett API Key</label>
          <input id="jackett-api-key" class="input" type="password" bind:value={cfg.jackett_api_key} placeholder="********" />
        </div>

        <div class="field-group">
          <label class="field-label" for="deezer-arl">Deezer ARL</label>
          <input id="deezer-arl" class="input" type="password" bind:value={cfg.deezer_arl} placeholder="********" />
        </div>

        <div class="field-group">
          <label class="field-label" for="qobuz-email">Qobuz Email</label>
          <input id="qobuz-email" class="input" bind:value={cfg.qobuz_email} placeholder="your@email.com" />
        </div>

        <div class="field-group">
          <label class="field-label" for="qobuz-password">Qobuz Password</label>
          <input
            id="qobuz-password"
            class="input"
            type="password"
            bind:value={cfg.qobuz_password}
            placeholder="********"
          />
        </div>
      </div>
    </section>

    <section class="settings-section">
      <h3 class="section-title">Usenet</h3>

      <div class="field-grid">
        <div class="field-group">
          <label class="field-label" for="nzbgeek-key">NZBGeek API Key</label>
          <input id="nzbgeek-key" class="input" type="password" bind:value={cfg.nzbgeek_api_key} placeholder="********" />
        </div>

        <div class="field-group">
          <label class="field-label" for="sabnzbd-url">SABnzbd URL</label>
          <input id="sabnzbd-url" class="input" bind:value={cfg.sabnzbd_url} placeholder="http://localhost:8080" />
        </div>

        <div class="field-group">
          <label class="field-label" for="sabnzbd-key">SABnzbd API Key</label>
          <input id="sabnzbd-key" class="input" type="password" bind:value={cfg.sabnzbd_api_key} placeholder="********" />
        </div>
      </div>
    </section>

    <section class="settings-section">
      <h3 class="section-title">Metadata and Discovery Providers</h3>
      <p class="section-desc">
        Spotify and Genius appeared in the recovered credential ledger, so their placeholders live
        here even before their deeper flows are rebuilt.
      </p>

      <div class="field-grid">
        <div class="field-group">
          <label class="field-label" for="spotify-client-id">Spotify Client ID</label>
          <input
            id="spotify-client-id"
            class="input"
            bind:value={cfg.spotify_client_id}
            placeholder="Spotify app client ID"
          />
        </div>

        <div class="field-group">
          <label class="field-label" for="spotify-client-secret">Spotify Client Secret</label>
          <input
            id="spotify-client-secret"
            class="input"
            type="password"
            bind:value={cfg.spotify_client_secret}
            placeholder="********"
          />
        </div>

        <div class="field-group">
          <label class="field-label" for="spotify-access-token">Spotify Access Token</label>
          <input
            id="spotify-access-token"
            class="input"
            type="password"
            bind:value={cfg.spotify_access_token}
            placeholder="********"
          />
        </div>

        <div class="field-group">
          <label class="field-label" for="genius-token">Genius Access Token</label>
          <input
            id="genius-token"
            class="input"
            type="password"
            bind:value={cfg.genius_token}
            placeholder="********"
          />
        </div>

        <div class="field-group">
          <label class="field-label" for="discogs-token">Discogs Token</label>
          <input
            id="discogs-token"
            class="input"
            type="password"
            bind:value={cfg.discogs_token}
            placeholder="********"
          />
        </div>

        <div class="field-group">
          <label class="field-label" for="lastfm-key">Last.fm API Key</label>
          <input
            id="lastfm-key"
            class="input"
            type="password"
            bind:value={cfg.lastfm_api_key}
            placeholder="********"
          />
        </div>

        <div class="field-group">
          <label class="field-label" for="lastfm-username">Last.fm Username</label>
          <input
            id="lastfm-username"
            class="input"
            bind:value={cfg.lastfm_username}
            placeholder="your_lastfm_username"
          />
        </div>
      </div>

      <div class="roots-actions" style="margin-top:12px;">
        <button class="btn btn-secondary" on:click={syncLastfmHistory} disabled={lastfmSyncing}>
          {#if lastfmSyncing}
            <div class="spinner" style="width:14px;height:14px;border-width:2px;"></div>
            Syncing Last.fm...
          {:else}
            Sync Last.fm History
          {/if}
        </button>
        {#if lastfmSyncMessage}
          <span class="saved-confirm">{lastfmSyncMessage}</span>
        {/if}
      </div>
    </section>

    <section class="settings-section">
      <h3 class="section-title">Tools</h3>
      <p class="section-desc">
        Paths to local tool binaries. Leave blank to use system PATH or built-in defaults.
      </p>

      <div class="field-grid">
        <div class="field-group">
          <label class="field-label" for="ytdlp-path">yt-dlp Binary Path</label>
          <input
            id="ytdlp-path"
            class="input"
            bind:value={cfg.ytdlp_path}
            placeholder="C:/tools/yt-dlp.exe"
          />
        </div>

        <div class="field-group">
          <label class="field-label" for="sevenzip-path">7-Zip Binary Path</label>
          <input
            id="sevenzip-path"
            class="input"
            bind:value={cfg.sevenzip_path}
            placeholder="C:/Program Files/7-Zip/7z.exe"
          />
        </div>
      </div>
    </section>

    <div class="save-row">
      <button class="btn btn-ghost" on:click={persistEffectiveSecrets} disabled={persistingEffective}>
        {persistingEffective ? 'Persisting secrets...' : 'Persist Loaded Secrets'}
      </button>
      <button class="btn btn-primary" on:click={handleSave}>Save Settings</button>
      {#if saved}
        <span class="saved-confirm">Saved</span>
      {/if}
    </div>
  </div>
</div>

<style>
  .settings-page {
    display: flex;
    flex-direction: column;
    min-height: 100%;
  }

  .settings-body {
    padding: 0 1.5rem 2rem;
    max-width: 860px;
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .settings-section {
    padding: 20px 0;
    border-bottom: 1px solid var(--border);
  }

  .settings-section:last-child {
    border-bottom: none;
  }

  .section-title {
    font-size: 0.95rem;
    font-weight: 700;
    margin-bottom: 6px;
  }

  .section-desc {
    font-size: 0.8rem;
    color: var(--text-secondary);
    margin-bottom: 14px;
    max-width: 70ch;
  }

  .provider-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 12px;
  }

  .provider-card {
    padding: 14px;
    border-radius: var(--radius);
    background: var(--bg-card);
    border: 1px solid var(--border);
  }

  .provider-card-ready {
    border-color: color-mix(in srgb, var(--success) 40%, var(--border));
  }

  .provider-card-header {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    align-items: flex-start;
  }

  .provider-name {
    font-weight: 700;
    font-size: 0.9rem;
  }

  .provider-summary {
    margin-top: 4px;
    font-size: 0.78rem;
    color: var(--text-secondary);
  }

  .provider-missing {
    margin-top: 10px;
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .roots-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 12px;
  }

  .root-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-radius: var(--radius-sm);
    background: var(--bg-card);
    border: 1px solid var(--border);
  }

  .root-icon {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .root-path {
    flex: 1;
    font-size: 0.85rem;
    font-family: monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .remove-btn {
    font-size: 0.75rem;
    color: var(--text-muted);
    cursor: pointer;
    background: none;
    border: none;
    padding: 2px 6px;
    border-radius: 4px;
    transition: color 0.1s;
  }

  .remove-btn:hover {
    color: var(--error);
  }

  .roots-empty {
    font-size: 0.85rem;
    color: var(--text-muted);
    padding: 8px 0;
  }

  .roots-actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }

  .field-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 0 14px;
  }

  .field-group {
    margin-bottom: 12px;
  }

  .field-label {
    display: block;
    font-size: 0.8rem;
    color: var(--text-secondary);
    margin-bottom: 6px;
    font-weight: 500;
  }

  .save-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 20px 0 0;
  }

  .saved-confirm {
    font-size: 0.85rem;
    color: var(--success);
  }
</style>
