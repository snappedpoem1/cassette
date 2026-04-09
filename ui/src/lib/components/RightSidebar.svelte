<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type AcquisitionRequestListItem, type SpotifyAlbumHistory } from '$lib/api/tauri';
  import { buildAutomationDigest } from '$lib/automation-digest';
  import AutomationDigestPanel from './AutomationDigestPanel.svelte';
  import { nowPlayingContext, currentTrack } from '$lib/stores/player';
  import { backlogStatus, providerHealth, providerStatuses, slskdRuntimeStatus } from '$lib/stores/downloads';
  import { isScanning } from '$lib/stores/library';
  import { queue } from '$lib/stores/queue';
  import QueuePanel from './QueuePanel.svelte';

  let activeTab: 'queue' | 'room' | 'context' = 'queue';
  let requests: AcquisitionRequestListItem[] = [];
  let missingAlbums: SpotifyAlbumHistory[] = [];
  $: ctx = $nowPlayingContext;
  $: track = $currentTrack;
  $: digest = buildAutomationDigest({
    requests,
    missingAlbums,
    providerHealth: $providerHealth,
    providerStatuses: $providerStatuses,
    slskdReady: $slskdRuntimeStatus?.ready ?? false,
    isScanning: $isScanning,
    backlogRunning: $backlogStatus?.running ?? false,
    queueCount: $queue.length,
  });

  onMount(() => {
    void loadDigest();
  });

  async function loadDigest() {
    try {
      const [nextRequests, missing] = await Promise.all([
        api.listAcquisitionRequests(undefined, 24),
        api.getMissingSpotifyAlbums(6),
      ]);
      requests = nextRequests;
      missingAlbums = missing;
    } catch {
      requests = [];
      missingAlbums = [];
    }
  }
</script>

<div class="right-sidebar">
  <div class="rs-tabs" role="tablist" aria-label="Right sidebar panels">
    <button
      class="rs-tab"
      class:active={activeTab === 'queue'}
      role="tab"
      aria-selected={activeTab === 'queue'}
      aria-controls="rs-panel-queue"
      id="rs-tab-queue"
      on:click={() => (activeTab = 'queue')}
    >
      Up Next
    </button>
    <button
      class="rs-tab"
      class:active={activeTab === 'room'}
      role="tab"
      aria-selected={activeTab === 'room'}
      aria-controls="rs-panel-room"
      id="rs-tab-room"
      on:click={() => {
        activeTab = 'room';
        void loadDigest();
      }}
    >
      Room
    </button>
    <button
      class="rs-tab"
      class:active={activeTab === 'context'}
      role="tab"
      aria-selected={activeTab === 'context'}
      aria-controls="rs-panel-context"
      id="rs-tab-context"
      on:click={() => (activeTab = 'context')}
    >
      Context
    </button>
  </div>

  <div class="rs-content" role="tabpanel" id={`rs-panel-${activeTab}`} aria-labelledby={`rs-tab-${activeTab}`}>
    {#if activeTab === 'queue'}
      <QueuePanel />
    {:else if activeTab === 'room'}
      <div class="room-panel">
        <AutomationDigestPanel
          digest={digest}
          compact={true}
          primaryHref="/workstation"
          primaryLabel="Open workstation"
          secondaryHref="/downloads"
          secondaryLabel="Downloads"
        />
      </div>
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
              <div class="info-meta">{ctx.listeners.toLocaleString()} listeners</div>
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
              <div class="info-label">Lyrics {#if ctx.lyrics_source}<span class="info-source">/ {ctx.lyrics_source}</span>{/if}</div>
              <pre class="lyrics">{ctx.lyrics}</pre>
            </div>
          {/if}
        {:else if track}
          <div class="empty-state" style="padding:2rem 1rem;">
            <div class="empty-title">{track.title}</div>
            <div class="empty-body">{track.artist}</div>
          </div>
        {:else}
          <div class="empty-state" style="padding:2rem 1rem;">
            <div class="empty-title">Nothing playing</div>
            <div class="empty-body">Artist notes, album context, and lyrics appear here.</div>
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .right-sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .rs-tabs {
    display: flex;
    border-bottom: 1px solid var(--border-dim);
    flex-shrink: 0;
  }

  .rs-tab {
    flex: 1;
    padding: 11px 6px;
    font-size: 0.75rem;
    font-weight: 700;
    color: var(--text-muted);
    cursor: pointer;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    transition: color 0.15s;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .rs-tab:hover {
    color: var(--text-secondary);
  }

  .rs-tab.active {
    color: var(--text-accent);
    border-bottom-color: var(--primary);
  }

  .rs-content {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }

  .room-panel {
    padding: 12px;
  }

  .info-panel {
    padding: 0 12px 12px;
    display: flex;
    flex-direction: column;
  }

  .info-section {
    padding: 14px 0;
    border-bottom: 1px solid var(--border-dim);
  }

  .info-section:last-child {
    border-bottom: none;
  }

  .info-label {
    font-size: 0.64rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
    margin-bottom: 6px;
    font-weight: 700;
  }

  .info-source {
    font-weight: 500;
    text-transform: none;
    letter-spacing: 0;
  }

  .info-artist {
    font-size: 0.95rem;
    font-weight: 700;
    color: var(--text-primary);
    margin-bottom: 6px;
  }

  .info-album {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-primary);
    margin-bottom: 5px;
  }

  .info-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-bottom: 6px;
  }

  .info-tag {
    font-size: 0.64rem;
    padding: 2px 6px;
    border-radius: 999px;
    background: rgba(139, 180, 212, 0.1);
    color: var(--text-accent);
    letter-spacing: 0.03em;
  }

  .info-meta {
    font-size: 0.74rem;
    color: var(--text-secondary);
    margin-bottom: 6px;
  }

  .info-summary {
    font-size: 0.78rem;
    color: var(--text-secondary);
    line-height: 1.7;
    margin: 0;
    max-height: 120px;
    overflow: hidden;
  }

  .lyrics {
    font-family: inherit;
    font-size: 0.77rem;
    color: var(--text-secondary);
    white-space: pre-wrap;
    line-height: 1.95;
    margin: 0;
    max-height: 280px;
    overflow-y: auto;
  }
</style>
