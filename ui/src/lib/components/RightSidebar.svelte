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
                  <span class="badge badge-accent">{tag}</span>
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
            <div class="empty-body">Artist info &amp; lyrics will appear here.</div>
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
.right-sidebar { display: flex; flex-direction: column; height: 100%; }
.rs-tabs { display: flex; border-bottom: 1px solid var(--border); flex-shrink: 0; }
.rs-tab {
  flex: 1; padding: 12px 8px; font-size: 0.85rem; font-weight: 500;
  color: var(--text-secondary); cursor: pointer; background: none;
  border: none; border-bottom: 2px solid transparent; margin-bottom: -1px;
  transition: color 0.15s;
}
.rs-tab:hover { color: var(--text-primary); }
.rs-tab.active { color: var(--accent-bright); border-bottom-color: var(--accent); }

.rs-content { flex: 1; overflow-y: auto; display: flex; flex-direction: column; }

.info-panel { padding: 0 12px; display: flex; flex-direction: column; }
.info-section { padding: 14px 0; border-bottom: 1px solid var(--border); }
.info-section:last-child { border-bottom: none; }
.info-label  { font-size: 0.7rem; text-transform: uppercase; letter-spacing: 0.07em; color: var(--text-muted); margin-bottom: 6px; font-weight: 600; }
.info-source { font-weight: 400; text-transform: none; letter-spacing: 0; }
.info-artist { font-size: 1rem; font-weight: 700; margin-bottom: 6px; }
.info-album  { font-size: 0.9rem; font-weight: 600; margin-bottom: 6px; }
.info-tags   { display: flex; flex-wrap: wrap; gap: 4px; margin-bottom: 6px; }
.info-meta   { font-size: 0.75rem; color: var(--text-secondary); margin-bottom: 6px; }
.info-summary { font-size: 0.8rem; color: var(--text-secondary); line-height: 1.6; margin: 0; max-height: 100px; overflow: hidden; }
.lyrics { font-family: inherit; font-size: 0.8rem; color: var(--text-secondary); white-space: pre-wrap; line-height: 1.9; margin: 0; max-height: 280px; overflow-y: auto; }
</style>
