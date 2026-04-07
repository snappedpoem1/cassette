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
  import { browser } from '$app/environment';
  import { getMilkdropPresetNames } from '$lib/visualizer/presets';
  import {
    SAFE_EXTENSIONS,
    extensionEnabledKey,
    extensionTelemetryKey,
    type ExtensionHealthReport,
  } from '$lib/extensions/safe-surface';
  import type { DownloadConfig, PolicyProfile, SlskdRuntimeStatus } from '$lib/api/tauri';

  onMount(async () => {
    await loadDownloadConfig();
    await loadSlskdRuntimeStatus();
    await loadPolicyProfile();
    await loadVisualizerPrefs();
    await loadMilkdropPresets();
    await loadExtensionSurface();
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

  let activeSection: 'library' | 'providers' | 'enrichment' | 'tools' | 'lastfm' | 'extensions' = 'library';
  let saved = false;
  let persistingEffective = false;
  let lastfmSyncing = false;
  let lastfmSyncMessage: string | null = null;
  let slskdRuntime: SlskdRuntimeStatus | null = null;
  let slskdRuntimeBusy = false;
  let policyProfile: PolicyProfile = 'balanced_auto';
  let policyProfileSaving = false;
  let policyProfileMessage: string | null = null;
  let visualizerEnabled = true;
  let visualizerLowMotion = false;
  let appreciationLaneEnabled = true;
  let visualizerMode: 'waveform' | 'spectrum' | 'milkdrop' = 'spectrum';
  let visualizerPreset = '';
  let milkdropPresetNames: string[] = [];
  let loadingMilkdropPresets = false;
  let visualizerFpsCap = 30;
  let dynamicGlassEnabled = true;
  let dynamicGlassLowMotion = false;
  let dynamicGlassIntensity = 62;
  let extensionEnabled: Record<string, boolean> = {};
  let extensionHealth: Record<string, ExtensionHealthReport> = {};
  let extensionHealthBusy = false;

  $: if (milkdropPresetNames.length > 0 && !milkdropPresetNames.includes(visualizerPreset)) {
    visualizerPreset = milkdropPresetNames[0];
  }

  $: extensionRows = SAFE_EXTENSIONS.map((extension) => ({
    extension,
    enabled: extensionEnabled[extension.id] ?? true,
    health: extensionHealth[extension.id],
  }));

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
    await saveVisualizerPrefs();
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

  async function loadVisualizerPrefs() {
    try {
      const enabled = await api.getSetting('ui_visualizer_enabled');
      const lowMotion = await api.getSetting('ui_visualizer_low_motion');
      const appreciation = await api.getSetting('ui_appreciation_lane_enabled');
      const mode = await api.getSetting('ui_visualizer_mode');
      const preset = await api.getSetting('ui_visualizer_preset');
      const fpsCap = await api.getSetting('ui_visualizer_fps_cap');
      const glassEnabled = await api.getSetting('ui_dynamic_glass_enabled');
      const glassLowMotion = await api.getSetting('ui_dynamic_glass_low_motion');
      const glassIntensity = await api.getSetting('ui_dynamic_glass_intensity');
      visualizerEnabled = enabled !== 'false';
      visualizerLowMotion = lowMotion === 'true';
      appreciationLaneEnabled = appreciation !== 'false';
      visualizerMode = mode === 'milkdrop' || mode === 'waveform' ? mode : 'spectrum';
      visualizerPreset = preset ?? '';
      visualizerFpsCap = Math.min(60, Math.max(15, Number.parseInt(fpsCap ?? '30', 10) || 30));
      dynamicGlassEnabled = glassEnabled !== 'false';
      dynamicGlassLowMotion = glassLowMotion === 'true';
      dynamicGlassIntensity = Math.min(90, Math.max(15, Number.parseInt(glassIntensity ?? '62', 10) || 62));
    } catch {
      visualizerEnabled = true;
      visualizerLowMotion = false;
      appreciationLaneEnabled = true;
      visualizerMode = 'spectrum';
      visualizerPreset = '';
      visualizerFpsCap = 30;
      dynamicGlassEnabled = true;
      dynamicGlassLowMotion = false;
      dynamicGlassIntensity = 62;
    }
  }

  async function loadMilkdropPresets() {
    loadingMilkdropPresets = true;
    try {
      milkdropPresetNames = await getMilkdropPresetNames();
      if (!visualizerPreset && milkdropPresetNames.length > 0) {
        visualizerPreset = milkdropPresetNames[0];
      }
    } catch {
      milkdropPresetNames = [];
    } finally {
      loadingMilkdropPresets = false;
    }
  }

  async function saveVisualizerPrefs() {
    await api.setSetting('ui_visualizer_enabled', visualizerEnabled ? 'true' : 'false');
    await api.setSetting('ui_visualizer_low_motion', visualizerLowMotion ? 'true' : 'false');
    await api.setSetting('ui_appreciation_lane_enabled', appreciationLaneEnabled ? 'true' : 'false');
    await api.setSetting('ui_visualizer_mode', visualizerMode);
    await api.setSetting('ui_visualizer_preset', visualizerPreset);
    await api.setSetting('ui_visualizer_fps_cap', String(visualizerFpsCap));
    await api.setSetting('ui_dynamic_glass_enabled', dynamicGlassEnabled ? 'true' : 'false');
    await api.setSetting('ui_dynamic_glass_low_motion', dynamicGlassLowMotion ? 'true' : 'false');
    await api.setSetting('ui_dynamic_glass_intensity', String(dynamicGlassIntensity));
  }

  function parseTelemetry(raw: string | null): Record<string, ExtensionHealthReport> {
    if (!raw) {
      return {};
    }
    try {
      const parsed = JSON.parse(raw) as Record<string, ExtensionHealthReport>;
      return typeof parsed === 'object' && parsed ? parsed : {};
    } catch {
      return {};
    }
  }

  async function loadExtensionSurface() {
    const enabledMap: Record<string, boolean> = {};
    for (const extension of SAFE_EXTENSIONS) {
      const raw = await api.getSetting(extensionEnabledKey(extension.id));
      enabledMap[extension.id] = raw !== 'false';
    }
    extensionEnabled = enabledMap;
    extensionHealth = parseTelemetry(await api.getSetting(extensionTelemetryKey()));
    await refreshExtensionHealth();
  }

  async function persistExtensionHealth() {
    await api.setSetting(extensionTelemetryKey(), JSON.stringify(extensionHealth));
  }

  async function setExtensionEnabled(id: string, enabled: boolean) {
    extensionEnabled = { ...extensionEnabled, [id]: enabled };
    await api.setSetting(extensionEnabledKey(id), enabled ? 'true' : 'false');
    await refreshExtensionHealth();
  }

  function fallbackHealth(id: string, status: ExtensionHealthReport['status'], message: string): ExtensionHealthReport {
    const existing = extensionHealth[id];
    return {
      status,
      message,
      checkedAt: new Date().toISOString(),
      successCount: (existing?.successCount ?? 0) + (status === 'healthy' ? 1 : 0),
      failureCount: (existing?.failureCount ?? 0) + (status === 'degraded' ? 1 : 0),
    };
  }

  async function probeExtension(id: string): Promise<Pick<ExtensionHealthReport, 'status' | 'message'>> {
    if (!browser) {
      return { status: 'healthy', message: 'Desktop runtime context available' };
    }

    if (id === 'visual_pack_butterchurn') {
      if (!visualizerEnabled || visualizerMode !== 'milkdrop') {
        return { status: 'healthy', message: 'Installed and idle until MilkDrop mode is selected' };
      }
      const canvas = document.createElement('canvas');
      const gl = canvas.getContext('webgl2') || canvas.getContext('webgl');
      if (!gl) {
        return { status: 'degraded', message: 'WebGL unavailable, bars fallback active' };
      }
      await import('butterchurn');
      return { status: 'healthy', message: 'Renderer initialized with GPU context' };
    }

    if (id === 'enricher_lastfm_context') {
      if (cfg.lastfm_api_key && cfg.lastfm_api_key.trim().length > 0) {
        return { status: 'healthy', message: 'Last.fm key present for context enrichment' };
      }
      return { status: 'degraded', message: 'Missing Last.fm API key' };
    }

    if (id === 'provider_adapter_local_archive') {
      if ($libraryRoots.length === 0) {
        return { status: 'degraded', message: 'No library roots configured' };
      }
      return { status: 'healthy', message: `Bounded to ${$libraryRoots.length} root(s), read-only adapter` };
    }

    return { status: 'degraded', message: 'Unknown extension probe id' };
  }

  async function refreshExtensionHealth() {
    if (extensionHealthBusy) {
      return;
    }
    extensionHealthBusy = true;
    try {
      const next: Record<string, ExtensionHealthReport> = { ...extensionHealth };
      for (const extension of SAFE_EXTENSIONS) {
        const enabled = extensionEnabled[extension.id] ?? true;
        if (!enabled) {
          next[extension.id] = fallbackHealth(extension.id, 'disabled', 'Disabled by policy toggle');
          continue;
        }

        try {
          const report = await probeExtension(extension.id);
          next[extension.id] = fallbackHealth(extension.id, report.status, report.message);
        } catch (error) {
          const message = error instanceof Error ? error.message : 'Extension probe failed';
          next[extension.id] = fallbackHealth(extension.id, 'degraded', message);
        }
      }
      extensionHealth = next;
      await persistExtensionHealth();
    } finally {
      extensionHealthBusy = false;
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
      <!-- svelte-ignore a11y-no-static-element-interactions -->
      <div class="subnav-item" class:active={activeSection === 'extensions'} on:click={() => activeSection = 'extensions'} role="button" tabindex="0" on:keydown={(e) => e.key === 'Enter' && (activeSection = 'extensions')}>Extensions</div>
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

        <div class="settings-section">
          <div class="section-title">Now Playing Visualizer</div>
          <div class="section-sub">Optional player-bar visualizer and appreciation signal lanes with low-motion fallback mode. MilkDrop mode uses imported Butterchurn presets.</div>
          <div class="field-group">
            <div class="field field-full">
              <label class="checkbox-field">
                <input type="checkbox" bind:checked={visualizerEnabled} />
                <span>Enable visualizer in player bar</span>
              </label>
            </div>
            <div class="field field-full">
              <label>
                Visualizer Mode
                <select bind:value={visualizerMode} class="select-field">
                  <option value="waveform">Waveform</option>
                  <option value="spectrum">Spectrum</option>
                  <option value="milkdrop">MilkDrop-style (Butterchurn)</option>
                </select>
              </label>
            </div>
            <div class="field field-full">
              <label>
                Visualizer FPS Cap
                <select bind:value={visualizerFpsCap} class="select-field">
                  <option value={15}>15 fps</option>
                  <option value={24}>24 fps</option>
                  <option value={30}>30 fps</option>
                  <option value={45}>45 fps</option>
                  <option value={60}>60 fps</option>
                </select>
              </label>
            </div>
            <div class="field field-full">
              <label>
                MilkDrop Preset
                <select bind:value={visualizerPreset} class="select-field" disabled={visualizerMode !== 'milkdrop' || loadingMilkdropPresets || milkdropPresetNames.length === 0}>
                  {#if loadingMilkdropPresets}
                    <option value="">Loading presets…</option>
                  {:else if milkdropPresetNames.length === 0}
                    <option value="">No presets found</option>
                  {:else}
                    {#each milkdropPresetNames as name}
                      <option value={name}>{name}</option>
                    {/each}
                  {/if}
                </select>
              </label>
            </div>
            <div class="field field-full">
              <label class="checkbox-field">
                <input type="checkbox" bind:checked={visualizerLowMotion} />
                <span>Use low-motion visualizer mode</span>
              </label>
            </div>
            <div class="field field-full">
              <label class="checkbox-field">
                <input type="checkbox" bind:checked={appreciationLaneEnabled} />
                <span>Show appreciation signal lane (tags, listeners, lyrics source)</span>
              </label>
            </div>
          </div>
        </div>

        <div class="settings-section">
          <div class="section-title">Dynamic Glass and Mood</div>
          <div class="section-sub">Adaptive shell mooding from artwork/track identity with static fallback and bounded effect intensity.</div>
          <div class="field-group">
            <div class="field field-full">
              <label class="checkbox-field">
                <input type="checkbox" bind:checked={dynamicGlassEnabled} />
                <span>Enable dynamic glass mood overlays</span>
              </label>
            </div>
            <div class="field field-full">
              <label class="checkbox-field">
                <input type="checkbox" bind:checked={dynamicGlassLowMotion} />
                <span>Prefer reduced-motion mood transitions</span>
              </label>
            </div>
            <div class="field field-full">
              <label>
                Mood intensity
                <input type="range" min="15" max="90" step="1" bind:value={dynamicGlassIntensity} />
              </label>
              <span class="sync-msg">{dynamicGlassIntensity}%</span>
            </div>
          </div>
        </div>
      {/if}

      {#if activeSection === 'extensions'}
        <div class="settings-section">
          <div class="section-title">Safe Extension Surface</div>
          <div class="section-sub">Capability-scoped extension model. Extensions are isolated from deterministic acquisition/finalization lanes and failures degrade gracefully.</div>
          <div class="sync-row">
            <button class="action-btn" on:click={refreshExtensionHealth} disabled={extensionHealthBusy}>
              {extensionHealthBusy ? 'Checking…' : 'Refresh Extension Health'}
            </button>
          </div>
          <div class="extension-list">
            {#each extensionRows as row}
              <article class="extension-card">
                <div class="extension-head">
                  <div>
                    <div class="provider-name">{row.extension.label}</div>
                    <div class="provider-missing-hint">{row.extension.category} · deterministic core access: blocked</div>
                  </div>
                  <label class="checkbox-field">
                    <input
                      type="checkbox"
                      checked={row.enabled}
                      on:change={(event) => setExtensionEnabled(row.extension.id, (event.currentTarget as HTMLInputElement).checked)}
                    />
                    <span>{row.enabled ? 'Enabled' : 'Disabled'}</span>
                  </label>
                </div>
                <div class="runtime-summary">{row.extension.description}</div>
                <div class="extension-capabilities">
                  {#each row.extension.capabilities as capability}
                    <span class="info-tag">{capability}</span>
                  {/each}
                </div>
                <div class="extension-health-row">
                  <span class="provider-status">
                    <span class="provider-dot" class:dot-ok={row.health?.status === 'healthy'} class:dot-missing={row.health?.status !== 'healthy'}></span>
                    <span class:status-ok={row.health?.status === 'healthy'} class:status-missing={row.health?.status !== 'healthy'}>
                      {row.health?.status ?? 'unknown'}
                    </span>
                  </span>
                  <span class="runtime-note">{row.health?.message ?? 'No health telemetry yet'}</span>
                </div>
                <div class="runtime-meta">
                  <span>checked: {row.health?.checkedAt ?? 'never'}</span>
                  <span>successes: {row.health?.successCount ?? 0}</span>
                  <span>failures: {row.health?.failureCount ?? 0}</span>
                </div>
              </article>
            {/each}
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
.checkbox-field {
  display: flex !important;
  flex-direction: row !important;
  align-items: center;
  gap: 8px;
  font-size: 0.78rem !important;
  letter-spacing: normal !important;
  color: var(--text-secondary) !important;
}

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

.extension-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.extension-card {
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg-card);
  padding: 10px 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.extension-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
}

.extension-capabilities {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}

.extension-health-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
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
