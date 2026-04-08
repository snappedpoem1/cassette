<script lang="ts">
  import { onMount } from 'svelte';
  import {
    playlists, activePlaylistItems, activePlaylistId,
    loadPlaylists, loadPlaylistItems, createPlaylist, deletePlaylist, playPlaylist,
  } from '$lib/stores/playlists';
  import { formatDuration } from '$lib/utils';
  import ContextActionRail from '$lib/components/ContextActionRail.svelte';
  import type { Playlist, Track } from '$lib/api/tauri';

  onMount(loadPlaylists);

  let creating = false;
  let newName = '';
  let newDesc = '';
  let confirmDeleteId: number | null = null;
  let selectedTrack: Track | null = null;
  let selectedPlaylistGuard: number | null = null;

  $: if ($activePlaylistId !== selectedPlaylistGuard) {
    selectedPlaylistGuard = $activePlaylistId;
    selectedTrack = null;
  }

  async function handleCreate() {
    if (!newName.trim()) return;
    await createPlaylist(newName.trim(), newDesc.trim() || null);
    newName = '';
    newDesc = '';
    creating = false;
  }

  async function handleDelete(pl: Playlist) {
    if (confirmDeleteId === pl.id) {
      await deletePlaylist(pl.id);
      if ($activePlaylistId === pl.id) activePlaylistId.set(null);
      confirmDeleteId = null;
    } else {
      confirmDeleteId = pl.id;
      setTimeout(() => {
        if (confirmDeleteId === pl.id) confirmDeleteId = null;
      }, 3000);
    }
  }

  function selectTrack(track: Track | null) {
    selectedTrack = track;
  }
</script>

<svelte:head><title>Playlists · Cassette</title></svelte:head>

<div class="playlists-page">
  <div class="page-header">
    <h2 style="flex:1">Playlists</h2>
    <button class="btn btn-primary" on:click={() => (creating = !creating)}>+ New</button>
  </div>

  {#if creating}
    <div class="create-form card" style="margin:0 1.5rem 1rem;">
      <input class="input" placeholder="Playlist name" bind:value={newName} style="margin-bottom:8px;" />
      <input class="input" placeholder="Description (optional)" bind:value={newDesc} style="margin-bottom:12px;" />
      <div style="display:flex;gap:8px;">
        <button class="btn btn-primary" on:click={handleCreate}>Create</button>
        <button class="btn btn-ghost" on:click={() => (creating = false)}>Cancel</button>
      </div>
    </div>
  {/if}

  <div class="playlists-layout">
    <!-- Playlist list -->
    <div class="playlist-list">
      {#if $playlists.length === 0}
        <div class="empty-state" style="padding:2rem 1rem;">
          <div class="empty-icon-text">No playlists yet</div>
          <div class="empty-body">Create a playlist to start building your listening arcs.</div>
        </div>
      {:else}
        {#each $playlists as pl}
          {@const active = $activePlaylistId === pl.id}
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div
            class="playlist-item"
            class:active
            role="button"
            tabindex="0"
            on:click={() => loadPlaylistItems(pl.id)}
            on:keydown={(event) => {
              if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault();
                loadPlaylistItems(pl.id);
              }
            }}
          >
            <div class="pl-icon">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/>
                <line x1="8" y1="18" x2="21" y2="18"/><line x1="3" y1="6" x2="3.01" y2="6"/>
                <line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/>
              </svg>
            </div>
            <div class="pl-info">
              <div class="pl-name">{pl.name}</div>
              <div class="pl-meta">{pl.track_count} tracks</div>
            </div>
            <div class="pl-actions">
              <button class="btn-icon" on:click|stopPropagation={() => playPlaylist(pl.id)} title="Play">▶</button>
              <button
                class="btn-icon"
                class:confirming={confirmDeleteId === pl.id}
                on:click|stopPropagation={() => handleDelete(pl)}
                title={confirmDeleteId === pl.id ? 'Click again to confirm delete' : 'Delete'}
                style={confirmDeleteId === pl.id ? 'color:var(--error)' : ''}
              >
                {confirmDeleteId === pl.id ? '?' : '✕'}
              </button>
            </div>
          </div>
        {/each}
      {/if}
    </div>

    <!-- Track list for selected playlist -->
    <div class="playlist-tracks">
      {#if $activePlaylistId === null}
        <div class="empty-state">
          <div class="empty-icon-text">Select a playlist</div>
        </div>
      {:else if $activePlaylistItems.length === 0}
        <div class="empty-state">
          <div class="empty-icon-text">Empty playlist</div>
          <div class="empty-body">Add tracks from the Library.</div>
        </div>
      {:else}
        {#if selectedTrack}
          <div style="padding: 8px;">
            <ContextActionRail
              compact
              track={selectedTrack}
              album={selectedTrack.album ? { artist: selectedTrack.artist, title: selectedTrack.album } : null}
              artistName={selectedTrack.artist}
            />
          </div>
        {/if}
        {#each $activePlaylistItems as item, i}
          {@const track = item.track}
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div
            class="track-row"
            role="button"
            tabindex="0"
            on:click={() => selectTrack(track)}
            on:dblclick={() => playPlaylist($activePlaylistId!, i)}
            on:keydown={(event) => {
              if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault();
                selectTrack(track);
              }
            }}
          >
            <span class="track-num">{i + 1}</span>
            <div class="track-title">{track?.title ?? 'Unknown'}</div>
            <div class="track-artist">{track?.artist ?? ''}</div>
            <span class="track-duration">{formatDuration(track?.duration_secs ?? 0)}</span>
            <span class="track-format">{track?.format?.toUpperCase() ?? ''}</span>
          </div>
        {/each}
      {/if}
    </div>
  </div>
</div>

<style>
.playlists-page { display: flex; flex-direction: column; min-height: 100%; }

.playlists-layout {
  display: grid;
  grid-template-columns: 260px 1fr;
  flex: 1; overflow: hidden;
}

.playlist-list { border-right: 1px solid var(--border); overflow-y: auto; padding: 8px; }

.playlist-item {
  display: flex; align-items: center; gap: 10px;
  padding: 10px 10px; border-radius: var(--radius-sm);
  cursor: pointer; transition: background 0.1s;
}
.playlist-item:hover { background: var(--bg-hover); }
.playlist-item.active { background: var(--bg-active); }
.pl-icon  { font-size: 1.2rem; flex-shrink: 0; }
.pl-info  { flex: 1; overflow: hidden; }
.pl-name  { font-weight: 600; font-size: 0.9rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.pl-meta  { font-size: 0.75rem; color: var(--text-secondary); }
.pl-actions { display: flex; gap: 2px; opacity: 0; transition: opacity 0.1s; }
.playlist-item:hover .pl-actions { opacity: 1; }
.btn-icon {
  width: 28px; height: 28px; border-radius: 50%; display: flex; align-items: center; justify-content: center;
  font-size: 0.8rem; background: none; border: none; cursor: pointer; color: var(--text-secondary);
  transition: background 0.1s;
}
.btn-icon:hover { background: var(--bg-active); }
.btn-icon.confirming {
  background: color-mix(in srgb, var(--error) 12%, var(--bg-card));
  border: 1px solid color-mix(in srgb, var(--error) 30%, var(--border));
}

.empty-icon-text {
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 6px;
}

.playlist-tracks { overflow-y: auto; padding: 8px; }
</style>
