<script lang="ts">
  import {
    tracks, albums, artists,
    activeTab, searchResults, isSearching,
    search,
  } from '$lib/stores/library';
  import { queueTracks } from '$lib/stores/queue';
  import ContextActionRail from '$lib/components/ContextActionRail.svelte';
  import { goto } from '$app/navigation';
  import { formatDuration, coverSrc, debounce, tintFromHex } from '$lib/utils';
  import type { Album, Track, TrackIdentityContext } from '$lib/api/tauri';
  import { api } from '$lib/api/tauri';

  let selectedAlbum: Album | null = null;
  let albumTracks: Track[] = [];
  let loadingAlbumTracks = false;
  let selectedTrack: Track | null = null;
  let selectedTrackIdentity: TrackIdentityContext | null = null;

  const debouncedSearch = debounce((query: string) => search(query), 300);

  let searchInput = '';
  $: debouncedSearch(searchInput);

  async function openAlbum(album: Album) {
    selectedAlbum = album;
    loadingAlbumTracks = true;
    albumTracks = await api.getAlbumTracks(album.artist, album.title);
    loadingAlbumTracks = false;
  }

  function closeAlbum() {
    selectedAlbum = null;
    albumTracks = [];
    selectedTrack = null;
  }

  async function playAlbum(album: Album | null) {
    if (!album) {
      return;
    }
    const trackList = await api.getAlbumTracks(album.artist, album.title);
    if (trackList.length) {
      await queueTracks(trackList, 0);
    }
  }

  async function playTrack(trackList: Track[], index: number) {
    await queueTracks(trackList, index);
  }

  async function inspectTrack(track: Track) {
    selectedTrack = track;
    try {
      selectedTrackIdentity = await api.getTrackIdentityContext(track.id);
    } catch {
      selectedTrackIdentity = null;
    }
  }

  function clearTrackInspector() {
    selectedTrack = null;
    selectedTrackIdentity = null;
  }

  const editionBucketLabel: Record<string, string> = {
    standard: 'Standard',
    edition_variant: 'Edition variant',
    remaster: 'Remaster',
    live: 'Live',
    compilation: 'Compilation',
    ep: 'EP',
    single: 'Single',
  };

  function formatQualityLabel(track: Track): string {
    const fmt = track.format?.toUpperCase() ?? 'AUDIO';
    const bits = track.bit_depth ? `${track.bit_depth}-bit` : null;
    const khz = track.sample_rate ? `${(track.sample_rate / 1000).toFixed(1)}kHz` : null;
    const kbps = track.bitrate_kbps ? `${track.bitrate_kbps}kbps` : null;

    if (bits && khz) return `${fmt} · ${bits} / ${khz}`;
    if (kbps) return `${fmt} · ${kbps}`;
    return fmt;
  }

  function qualityTierLabel(tier: string | null): string {
    if (!tier) return '—';
    const map: Record<string, string> = {
      lossless_hires: 'Hi-Res Lossless',
      lossless: 'Lossless',
      lossy_high: 'High Quality',
      lossy_mid: 'Standard Quality',
      lossy_low: 'Low Quality',
    };
    return map[tier] ?? tier;
  }
</script>

<svelte:head><title>Library · Cassette</title></svelte:head>

<div class="library-page">
  <div class="page-header">
    <h2 style="flex:1">Library</h2>
    <div class="search-wrap">
      <span class="search-icon">Search</span>
      <input
        class="input search-input"
        type="text"
        placeholder="Search tracks, artists, albums..."
        bind:value={searchInput}
      />
      {#if $isSearching}
        <span class="search-spinner"><div class="spinner" style="width:14px;height:14px;border-width:2px"></div></span>
      {/if}
    </div>
  </div>

  {#if searchInput.trim() && $searchResults.length > 0}
    <div class="search-results">
      <div class="sr-label">{$searchResults.length} results for "{searchInput}"</div>
      {#each $searchResults as track, i}
        <div class="track-row" role="button" tabindex="0" on:dblclick={() => playTrack($searchResults, i)}>
          <span class="track-num">{i + 1}</span>
          <div class="track-title">{track.title}</div>
          <div class="track-artist">{track.artist} · {track.album}</div>
          <span class="track-duration">{formatDuration(track.duration_secs)}</span>
          <span class="track-format">{track.format.toUpperCase()}</span>
        </div>
      {/each}
    </div>
  {:else if searchInput.trim() && !$isSearching}
    <div class="empty-state" style="padding:2rem;">
      <div class="empty-title">No results</div>
      <div class="empty-body">Nothing matched "{searchInput}"</div>
    </div>
  {:else}
    <div class="tabs">
      <button class="tab" class:active={$activeTab === 'albums'} on:click={() => activeTab.set('albums')}>Albums</button>
      <button class="tab" class:active={$activeTab === 'tracks'} on:click={() => activeTab.set('tracks')}>Tracks</button>
      <button class="tab" class:active={$activeTab === 'artists'} on:click={() => activeTab.set('artists')}>Artists</button>
    </div>

    {#if $activeTab === 'albums'}
      {#if selectedAlbum}
        {@const detailTint = tintFromHex(selectedAlbum.dominant_color_hex)}
        <div class="album-detail">
          {#if selectedAlbum.cover_art_path}
            <div
              class="album-detail-backdrop"
              style="background-image:url({coverSrc(selectedAlbum.cover_art_path)});background-color:{detailTint.bg};"
            ></div>
          {:else}
            <div class="album-detail-backdrop" style="background:{detailTint.bg};"></div>
          {/if}

          <div class="album-detail-header">
            <button class="back-btn" on:click={closeAlbum}>Back to albums</button>
            <div class="album-detail-art">
              {#if selectedAlbum.cover_art_path}
                <img src={coverSrc(selectedAlbum.cover_art_path)} alt="cover" />
              {:else}
                <div class="album-detail-art-ph">Art</div>
              {/if}
            </div>
            <div class="album-detail-info">
              <h1>{selectedAlbum.title}</h1>
              <div class="album-detail-artist">{selectedAlbum.artist}</div>
              <div class="album-detail-meta">
                {#if selectedAlbum.year}{selectedAlbum.year} · {/if}{selectedAlbum.track_count} tracks
              </div>
              <button class="btn btn-primary" style="margin-top:12px;" on:click={() => playAlbum(selectedAlbum)}>
                Play album
              </button>
              <div style="margin-top:10px; max-width:540px;">
                <ContextActionRail
                  compact
                  album={{ artist: selectedAlbum.artist, title: selectedAlbum.title }}
                  artistName={selectedAlbum.artist}
                />
              </div>
            </div>
          </div>

          {#if loadingAlbumTracks}
            <div class="empty-state"><div class="spinner"></div></div>
          {:else}
            <div class="track-list">
              {#each albumTracks as track, i}
                <div
                  class="track-row"
                  role="button"
                  tabindex="0"
                  on:click={() => inspectTrack(track)}
                  on:dblclick={() => playTrack(albumTracks, i)}
                  on:keydown={(event) => {
                    if (event.key === 'Enter' || event.key === ' ') {
                      event.preventDefault();
                      inspectTrack(track);
                    }
                  }}
                >
                  <span class="track-num">{track.track_number ?? i + 1}</span>
                  <div class="track-title">{track.title}</div>
                  <div class="track-artist">{track.artist !== selectedAlbum?.artist ? track.artist : ''}</div>
                  <span class="track-duration">{formatDuration(track.duration_secs)}</span>
                  <span class="track-format">{track.format.toUpperCase()}</span>
                </div>
              {/each}
            </div>
          {/if}

          {#if selectedTrack}
            <div class="track-inspector">
              <div class="track-inspector-header">
                <div>
                  <div class="track-inspector-title">{selectedTrack.title}</div>
                  <div class="track-inspector-sub">{selectedTrack.artist}{selectedTrack.album ? ` · ${selectedTrack.album}` : ''}</div>
                </div>
                <button class="back-btn inspector-close" on:click={clearTrackInspector}>Close</button>
              </div>
              <div class="track-inspector-grid">
                <div class="inspector-summary">
                  <span class="inspector-quality-badge">{formatQualityLabel(selectedTrack)}</span>
                  {#if selectedTrackIdentity?.edition_bucket}
                    <span class="inspector-edition-badge">{editionBucketLabel[selectedTrackIdentity.edition_bucket] ?? selectedTrackIdentity.edition_bucket}</span>
                  {/if}
                  {#if selectedTrack.year}
                    <span class="inspector-year">{selectedTrack.year}</span>
                  {/if}
                </div>
                <div><span>Quality tier</span><code>{qualityTierLabel(selectedTrack.quality_tier)}</code></div>
                <div><span>Edition bucket</span><code>{selectedTrackIdentity?.edition_bucket ? editionBucketLabel[selectedTrackIdentity.edition_bucket] ?? selectedTrackIdentity.edition_bucket : '—'}</code></div>
                <div><span>Format</span><code>{selectedTrack.format?.toUpperCase() ?? '—'}</code></div>
                <div><span>ISRC</span><code>{selectedTrack.isrc ?? '—'}</code></div>
                <div><span>Edition markers</span><code>{selectedTrackIdentity?.edition_markers?.length ? selectedTrackIdentity.edition_markers.join(', ') : '—'}</code></div>
                <details class="inspector-ids">
                  <summary>Identity details</summary>
                  <div class="inspector-ids-grid">
                    <div><span>MB recording</span><code>{selectedTrack.musicbrainz_recording_id ?? '—'}</code></div>
                    <div><span>MB release</span><code>{selectedTrack.musicbrainz_release_id ?? '—'}</code></div>
                    <div><span>MB release group</span><code>{selectedTrackIdentity?.musicbrainz_release_group_id ?? '—'}</code></div>
                    <div><span>Canonical artist</span><code>{selectedTrack.canonical_artist_id ?? '—'}</code></div>
                    <div><span>Canonical release</span><code>{selectedTrack.canonical_release_id ?? '—'}</code></div>
                  </div>
                </details>
                <div class="track-inspector-wide"><span>Path</span><code>{selectedTrack.path}</code></div>
              </div>
              <div style="margin-top:10px;">
                <ContextActionRail
                  compact
                  track={selectedTrack}
                  album={selectedTrack.album ? { artist: selectedTrack.artist, title: selectedTrack.album } : null}
                  artistName={selectedTrack.artist}
                />
              </div>
            </div>
          {/if}
        </div>
      {:else if $albums.length === 0}
        <div class="empty-state">
          <div class="empty-title">No albums yet</div>
          <div class="empty-body">Add a library root in Settings and scan to import your music.</div>
        </div>
      {:else}
        <div class="album-grid">
          {#each $albums as album}
            {@const tint = tintFromHex(album.dominant_color_hex)}
            <div
              class="album-card"
              role="button"
              tabindex="0"
              on:click={() => openAlbum(album)}
              on:dblclick={() => playAlbum(album)}
              on:keydown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  openAlbum(album);
                }
              }}
            >
              {#if album.cover_art_path}
                <img class="album-art" src={coverSrc(album.cover_art_path)} alt="cover" />
              {:else}
                <div class="album-art-placeholder">Art</div>
              {/if}
              <div class="album-info" style="background:{tint.bg};">
                <div class="album-title" style="color:{tint.titleColor};">{album.title}</div>
                <div class="album-artist">{album.artist}</div>
                <div class="album-meta">{album.year ?? ''}{album.year && album.track_count ? ' · ' : ''}{album.track_count} tracks</div>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    {:else if $activeTab === 'tracks'}
      {#if $tracks.length === 0}
        <div class="empty-state">
          <div class="empty-title">No tracks yet</div>
          <div class="empty-body">Scan your library from Settings.</div>
        </div>
      {:else}
        <div class="track-list">
          {#each $tracks as track, i}
            <div
              class="track-row"
              role="button"
              tabindex="0"
              on:click={() => inspectTrack(track)}
              on:dblclick={() => playTrack($tracks, i)}
              on:keydown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  inspectTrack(track);
                }
              }}
            >
              <span class="track-num">{i + 1}</span>
              <div class="track-title">{track.title}</div>
              <div class="track-artist">{track.artist}</div>
              <span class="track-duration">{formatDuration(track.duration_secs)}</span>
              <span class="track-format">{track.format.toUpperCase()}</span>
            </div>
          {/each}
        </div>

        {#if selectedTrack}
          <div class="track-inspector track-inspector-page">
            <div class="track-inspector-header">
              <div>
                <div class="track-inspector-title">{selectedTrack.title}</div>
                <div class="track-inspector-sub">{selectedTrack.artist}{selectedTrack.album ? ` · ${selectedTrack.album}` : ''}</div>
              </div>
              <button class="back-btn inspector-close" on:click={clearTrackInspector}>Close</button>
            </div>
            <div class="track-inspector-grid">
              <div class="inspector-summary">
                <span class="inspector-quality-badge">{formatQualityLabel(selectedTrack)}</span>
                {#if selectedTrackIdentity?.edition_bucket}
                  <span class="inspector-edition-badge">{editionBucketLabel[selectedTrackIdentity.edition_bucket] ?? selectedTrackIdentity.edition_bucket}</span>
                {/if}
                {#if selectedTrack.year}
                  <span class="inspector-year">{selectedTrack.year}</span>
                {/if}
              </div>
              <div><span>Quality tier</span><code>{qualityTierLabel(selectedTrack.quality_tier)}</code></div>
              <div><span>Edition bucket</span><code>{selectedTrackIdentity?.edition_bucket ? editionBucketLabel[selectedTrackIdentity.edition_bucket] ?? selectedTrackIdentity.edition_bucket : '—'}</code></div>
              <div><span>Format</span><code>{selectedTrack.format?.toUpperCase() ?? '—'}</code></div>
              <div><span>ISRC</span><code>{selectedTrack.isrc ?? '—'}</code></div>
              <div><span>Edition markers</span><code>{selectedTrackIdentity?.edition_markers?.length ? selectedTrackIdentity.edition_markers.join(', ') : '—'}</code></div>
              <details class="inspector-ids">
                <summary>Identity details</summary>
                <div class="inspector-ids-grid">
                  <div><span>MB recording</span><code>{selectedTrack.musicbrainz_recording_id ?? '—'}</code></div>
                  <div><span>MB release</span><code>{selectedTrack.musicbrainz_release_id ?? '—'}</code></div>
                  <div><span>MB release group</span><code>{selectedTrackIdentity?.musicbrainz_release_group_id ?? '—'}</code></div>
                  <div><span>Canonical artist</span><code>{selectedTrack.canonical_artist_id ?? '—'}</code></div>
                  <div><span>Content hash</span><code>{selectedTrack.content_hash ?? '—'}</code></div>
                </div>
              </details>
              <div class="track-inspector-wide"><span>Path</span><code>{selectedTrack.path}</code></div>
            </div>
            <div style="margin-top:10px;">
              <ContextActionRail
                compact
                track={selectedTrack}
                album={selectedTrack.album ? { artist: selectedTrack.artist, title: selectedTrack.album } : null}
                artistName={selectedTrack.artist}
              />
            </div>
          </div>
        {/if}
      {/if}
    {:else if $artists.length === 0}
      <div class="empty-state">
        <div class="empty-title">No artists yet</div>
      </div>
    {:else}
      <div class="artist-list">
        {#each $artists as artist}
          <div
            class="artist-row"
            role="button"
            tabindex="0"
            on:click={() => goto('/artists')}
            on:keydown={(event) => {
              if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault();
                goto('/artists');
              }
            }}
          >
            <div class="artist-avatar">{artist.name[0]?.toUpperCase()}</div>
            <div class="artist-info">
              <div class="artist-name">{artist.name}</div>
              <div class="artist-meta">{artist.album_count} albums · {artist.track_count} tracks</div>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
.library-page { display: flex; flex-direction: column; min-height: 100%; }

.page-header { background: linear-gradient(to bottom, var(--bg-base) 70%, transparent); }

.search-wrap {
  position: relative;
  display: flex;
  align-items: center;
  flex: 1;
  max-width: 360px;
}

.search-icon {
  position: absolute;
  left: 10px;
  font-size: 0.66rem;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--text-muted);
  pointer-events: none;
}

.search-input { padding-left: 58px !important; }
.search-spinner { position: absolute; right: 10px; }
.search-results { padding: 0 1rem 1rem; }
.sr-label { font-size: 0.8rem; color: var(--text-muted); padding: 8px 16px 4px; }
.track-list { padding: 8px; }

.album-detail { padding: 1.5rem; position: relative; overflow: hidden; }
.album-detail-backdrop {
  position: absolute;
  inset: 0;
  z-index: 0;
  background-size: cover;
  background-position: center;
  filter: blur(60px) brightness(0.35) saturate(1.4);
  transform: scale(1.1);
  pointer-events: none;
}

.album-detail > *:not(.album-detail-backdrop) { position: relative; z-index: 1; }
.album-detail-header { display: flex; align-items: flex-end; gap: 20px; margin-bottom: 24px; }

.back-btn {
  font-size: 0.85rem;
  color: var(--text-secondary);
  cursor: pointer;
  background: none;
  border: none;
  transition: color 0.1s;
}

.back-btn:hover { color: var(--text-primary); }

.album-detail-art {
  width: 84px;
  height: 84px;
  flex-shrink: 0;
  border-radius: var(--radius);
  overflow: hidden;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
}

.album-detail-art img { width: 100%; height: 100%; object-fit: cover; }

.album-detail-art-ph,
.album-art-placeholder {
  width: 100%;
  aspect-ratio: 1;
  background: var(--bg-card);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
}

.album-detail-info h1 { font-size: 1.3rem; font-weight: 800; color: #deeaf8; }
.album-detail-artist { color: var(--text-secondary); font-size: 1rem; margin-top: 4px; }
.album-detail-meta { color: var(--text-muted); font-size: 0.85rem; margin-top: 4px; }

.track-inspector {
  margin-top: 16px;
  padding: 12px 14px;
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: color-mix(in srgb, var(--bg-card) 82%, var(--bg-base));
}

.track-inspector-page { margin: 8px; }
.track-inspector-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
}

.track-inspector-title { font-size: 0.95rem; font-weight: 700; }
.track-inspector-sub { font-size: 0.76rem; color: var(--text-secondary); }

.track-inspector-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px 12px;
}

.track-inspector-grid div { display: flex; flex-direction: column; gap: 3px; }

.track-inspector-grid span {
  font-size: 0.68rem;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.track-inspector-grid code {
  font-size: 0.72rem;
  color: var(--text-primary);
  word-break: break-all;
}

.inspector-summary {
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 4px;
}

.inspector-quality-badge {
  font-size: 0.76rem;
  font-weight: 600;
  color: var(--text-primary);
  background: color-mix(in srgb, var(--primary) 14%, var(--bg-card));
  border: 1px solid color-mix(in srgb, var(--primary) 28%, var(--border));
  border-radius: 999px;
  padding: 2px 8px;
}

.inspector-edition-badge {
  font-size: 0.72rem;
  color: var(--accent-bright);
  background: color-mix(in srgb, var(--accent) 12%, var(--bg-card));
  border: 1px solid color-mix(in srgb, var(--accent) 24%, var(--border));
  border-radius: 999px;
  padding: 2px 8px;
}

.inspector-year {
  font-size: 0.72rem;
  color: var(--text-muted);
}

.inspector-ids {
  grid-column: 1 / -1;
  margin-top: 4px;
}

.inspector-ids summary {
  font-size: 0.68rem;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  cursor: pointer;
  user-select: none;
  margin-bottom: 6px;
}

.inspector-ids-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px 12px;
  margin-top: 6px;
  padding: 8px;
  background: var(--bg-base);
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-dim);
}

.inspector-ids-grid div { display: flex; flex-direction: column; gap: 3px; }

.inspector-ids-grid span {
  font-size: 0.68rem;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.inspector-ids-grid code {
  font-size: 0.68rem;
  color: var(--text-secondary);
  word-break: break-all;
}

.track-inspector-wide { grid-column: 1 / -1; }
.inspector-close { position: static; }

.artist-list { padding: 8px 1rem; display: flex; flex-direction: column; gap: 4px; }
.artist-row {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 10px 12px;
  border-radius: var(--radius-sm);
  transition: background 0.1s;
  cursor: pointer;
}

.artist-row:hover { background: var(--bg-hover); }

.artist-avatar {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  background: var(--bg-active);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 1rem;
  font-weight: 700;
  color: var(--accent-bright);
  flex-shrink: 0;
}

.artist-name { font-weight: 600; font-size: 0.9rem; }
.artist-meta { font-size: 0.75rem; color: var(--text-secondary); margin-top: 2px; }
</style>
