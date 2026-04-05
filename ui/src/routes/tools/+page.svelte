<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api/tauri';
  import type { OrganizeReport, DuplicateGroup, TagFix, Artist, Album } from '$lib/api/tauri';

  // ── State ──────────────────────────────────────────────────────────────────
  let activeSection: 'organize' | 'duplicates' | 'metadata' | 'maintenance' = 'organize';

  // Organize
  let organizeReport: OrganizeReport | null = null;
  let organizing = false;
  let organizeError: string | null = null;
  let organizeNotice: string | null = null;
  let organizeSkippedPreview: string[] = [];
  let organizeErrorPreview: string[] = [];

  // Duplicates
  let duplicates: DuplicateGroup[] = [];
  let scanningDupes = false;
  let dupeNotice: string | null = null;
  let resolvingAllDupes = false;

  // Metadata
  let metaArtist = '';
  let metaAlbum = '';
  let allArtists: Artist[] = [];
  let allAlbums: Album[] = [];
  let artistAlbums: Album[] = [];
  let tagFixes: TagFix[] = [];
  let fetchingFixes = false;
  let fixNotice: string | null = null;

  // Maintenance
  let pruneCount: number | null = null;
  let ingestCount: number | null = null;
  let ingestNotice: string | null = null;
  let ingestedPreview: string[] = [];
  let pruning = false;
  let ingesting = false;

  onMount(async () => {
    try {
      [allArtists, allAlbums] = await Promise.all([api.getArtists(), api.getAlbums()]);
    } catch {
      allArtists = [];
      allAlbums = [];
    }
  });

  $: {
    if (!metaArtist.trim()) {
      artistAlbums = [];
      if (metaAlbum) {
        metaAlbum = '';
      }
    } else {
      artistAlbums = allAlbums.filter((album) => album.artist === metaArtist);
      if (metaAlbum && !artistAlbums.some((album) => album.title === metaAlbum)) {
        metaAlbum = '';
      }
    }
  }

  const DUPLICATE_SORT = (a: DuplicateGroup, b: DuplicateGroup) =>
    a.key.localeCompare(b.key);

  // ── Actions ────────────────────────────────────────────────────────────────
  async function previewOrganize() {
    organizing = true;
    organizeError = null;
    organizeNotice = null;
    try {
      organizeReport = await api.organizeLibrary(true);
      organizeSkippedPreview = [];
      organizeErrorPreview = [];
      organizeNotice = `Preview found ${organizeReport.moved.length} move(s) and ${organizeReport.skipped} already in place.`;
    } catch (e) { organizeError = String(e); }
    organizing = false;
  }

  async function executeOrganize() {
    organizing = true;
    organizeError = null;
    organizeNotice = null;
    try {
      organizeReport = await api.organizeLibrary(false);
      organizeSkippedPreview = [];
      organizeErrorPreview = organizeReport.errors.slice(0, 10);
      organizeNotice = `Applied ${organizeReport.moved.length} move(s). ${organizeReport.skipped} skipped.`;
    } catch (e) { organizeError = String(e); }
    organizing = false;
  }

  async function scanDuplicates() {
    scanningDupes = true;
    dupeNotice = null;
    try {
      duplicates = (await api.findDuplicates()).sort(DUPLICATE_SORT);
      dupeNotice = duplicates.length === 0 ? 'No duplicates found.' : null;
    } catch (e) { dupeNotice = String(e); }
    scanningDupes = false;
  }

  async function resolveDupe(group: DuplicateGroup) {
    const best = group.tracks.find(t => t.is_best);
    if (!best) return;
    const removeIds = group.tracks.filter(t => !t.is_best).map(t => t.id);
    try {
      await api.resolveDuplicate(best.id, removeIds, true);
      duplicates = duplicates.filter(g => g.key !== group.key);
      dupeNotice = `Resolved. ${removeIds.length} duplicate(s) removed.`;
    } catch (e) {
      dupeNotice = String(e);
    }
  }

  async function resolveAllDupes() {
    resolvingAllDupes = true;
    dupeNotice = null;
    let removed = 0;
    try {
      for (const group of duplicates) {
        const best = group.tracks.find((track) => track.is_best);
        if (!best) continue;
        const removeIds = group.tracks.filter((track) => !track.is_best).map((track) => track.id);
        await api.resolveDuplicate(best.id, removeIds, true);
        removed += removeIds.length;
      }
      duplicates = [];
      dupeNotice = `Resolved all groups. Removed ${removed} duplicate file(s).`;
    } catch (e) {
      dupeNotice = String(e);
    } finally {
      resolvingAllDupes = false;
    }
  }

  async function fetchTagFixes() {
    if (!metaArtist.trim() || !metaAlbum.trim()) {
      fixNotice = 'Pick an artist and album first.';
      return;
    }
    fetchingFixes = true;
    fixNotice = null;
    try {
      tagFixes = await api.proposeTagFixes(metaArtist.trim(), metaAlbum.trim());
      fixNotice = tagFixes.length === 0 ? 'No corrections needed — tags match MusicBrainz.' : null;
    } catch (e) { fixNotice = String(e); }
    fetchingFixes = false;
  }

  async function applyFixes() {
    fixNotice = null;
    try {
      const count = await api.applyTagFixes(tagFixes);
      fixNotice = `${count} tag fix(es) applied.`;
      tagFixes = [];
    } catch (e) { fixNotice = String(e); }
  }

  async function pruneMissing() {
    pruning = true;
    try {
      pruneCount = await api.pruneMissingTracks();
    } catch (e) { pruneCount = null; }
    pruning = false;
  }

  async function ingestStaging() {
    ingesting = true;
    try {
      const files = await api.ingestStaging();
      ingestCount = files.length;
      ingestedPreview = files.slice(0, 8);
      ingestNotice = files.length === 0
        ? 'No audio files found in staging.'
        : `Ingested ${files.length} file(s) from staging into your library.`;
    } catch (e) { ingestCount = null; }
    ingesting = false;
  }

  function shortPath(p: string): string {
    const parts = p.replace(/\\/g, '/').split('/');
    return parts.slice(-3).join('/');
  }
</script>

<svelte:head><title>Tools · Cassette</title></svelte:head>

<div class="tools-page">
  <div class="page-header">
    <h2 style="flex:1">Library Tools</h2>
  </div>

  <div class="tools-tabs">
    <button class="tool-tab" class:active={activeSection === 'organize'} on:click={() => activeSection = 'organize'}>Organize</button>
    <button class="tool-tab" class:active={activeSection === 'duplicates'} on:click={() => activeSection = 'duplicates'}>Duplicates</button>
    <button class="tool-tab" class:active={activeSection === 'metadata'} on:click={() => activeSection = 'metadata'}>Metadata</button>
    <button class="tool-tab" class:active={activeSection === 'maintenance'} on:click={() => activeSection = 'maintenance'}>Maintenance</button>
  </div>

  <div class="tools-content">
    <!-- ── Organize ─────────────────────────────────────────── -->
    {#if activeSection === 'organize'}
      <div class="tool-section">
        <div class="tool-desc">
          Reorganize your library files into a clean <code>Artist/Album (Year)/Track.ext</code> folder structure.
        </div>
        <div class="tool-actions">
          <button class="btn btn-ghost" on:click={previewOrganize} disabled={organizing}>
            {organizing ? 'Scanning...' : 'Preview Changes'}
          </button>
          {#if organizeReport && organizeReport.moved.length > 0}
            <button class="btn btn-primary" on:click={executeOrganize} disabled={organizing}>
              Apply {organizeReport.moved.length} Move{organizeReport.moved.length === 1 ? '' : 's'}
            </button>
          {/if}
        </div>

        {#if organizeError}
          <div class="tool-error">{organizeError}</div>
        {/if}

        {#if organizeNotice}
          <div class="tool-notice">{organizeNotice}</div>
        {/if}

        {#if organizeReport}
          <div class="tool-stats">
            <span class="stat">{organizeReport.moved.length} to move</span>
            <span class="stat">{organizeReport.skipped} already in place</span>
            {#if organizeReport.errors.length > 0}
              <span class="stat stat-err">{organizeReport.errors.length} errors</span>
            {/if}
          </div>

          {#if organizeReport.moved.length > 0}
            <div class="move-list">
              {#each organizeReport.moved.slice(0, 100) as mv}
                <div class="move-row">
                  <div class="move-from">{shortPath(mv.old_path)}</div>
                  <div class="move-arrow">→</div>
                  <div class="move-to">{shortPath(mv.new_path)}</div>
                </div>
              {/each}
              {#if organizeReport.moved.length > 100}
                <div class="move-more">...and {organizeReport.moved.length - 100} more</div>
              {/if}
            </div>
          {/if}

          {#if organizeErrorPreview.length > 0}
            <div class="tool-mini-list" style="margin-top:12px;">
              {#each organizeErrorPreview as msg}
                <div class="tool-mini-item">{msg}</div>
              {/each}
            </div>
          {/if}
        {/if}
      </div>

    <!-- ── Duplicates ───────────────────────────────────────── -->
    {:else if activeSection === 'duplicates'}
      <div class="tool-section">
        <div class="tool-desc">
          Scan your library for duplicate tracks (same artist, album, track number). Keeps the highest quality version.
        </div>
        <div class="tool-actions">
          <button class="btn btn-primary" on:click={scanDuplicates} disabled={scanningDupes}>
            {scanningDupes ? 'Scanning...' : 'Scan for Duplicates'}
          </button>
          {#if duplicates.length > 0}
            <button class="btn btn-ghost" on:click={resolveAllDupes} disabled={resolvingAllDupes}>
              {resolvingAllDupes ? 'Resolving...' : `Handle All (${duplicates.length} groups)`}
            </button>
          {/if}
        </div>

        {#if dupeNotice}
          <div class="tool-notice">{dupeNotice}</div>
        {/if}

        {#if duplicates.length > 0}
          <div class="dupe-list">
            {#each duplicates as group}
              <div class="dupe-group">
                <div class="dupe-header">
                  <span class="dupe-key">{group.recommendation}</span>
                  <button class="btn btn-ghost" style="font-size:0.78rem;padding:4px 10px;" on:click={() => resolveDupe(group)}>
                    Keep Best & Remove Others
                  </button>
                </div>
                {#each group.tracks as track}
                  <div class="dupe-track" class:is-best={track.is_best}>
                    <span class="dupe-format">{track.format}</span>
                    <span class="dupe-quality">
                      {track.bit_depth ?? '?'}bit / {track.sample_rate ? (track.sample_rate / 1000).toFixed(1) + 'kHz' : '?'}
                      {track.bitrate_kbps ? ` / ${track.bitrate_kbps}kbps` : ''}
                    </span>
                    <span class="dupe-size">{(track.file_size / 1048576).toFixed(1)} MB</span>
                    <span class="dupe-path">{shortPath(track.path)}</span>
                    {#if track.is_best}<span class="badge badge-success">Best</span>{/if}
                  </div>
                {/each}
              </div>
            {/each}
          </div>
        {/if}
      </div>

    <!-- ── Metadata ─────────────────────────────────────────── -->
    {:else if activeSection === 'metadata'}
      <div class="tool-section">
        <div class="tool-desc">
          Look up an album on MusicBrainz and fix tags (title, artist, track numbers, year) to match the canonical release.
        </div>
        <div class="tool-actions" style="gap:8px;">
          <select class="input" bind:value={metaArtist} style="max-width:240px;">
            <option value="">Artist...</option>
            {#each allArtists as artist}
              <option value={artist.name}>{artist.name}</option>
            {/each}
          </select>
          <select class="input" bind:value={metaAlbum} style="max-width:260px;" disabled={!metaArtist}>
            <option value="">Album...</option>
            {#each artistAlbums as album}
              <option value={album.title}>{album.title}</option>
            {/each}
          </select>
          <button class="btn btn-primary" on:click={fetchTagFixes} disabled={fetchingFixes}>
            {fetchingFixes ? 'Looking up...' : 'Check Tags'}
          </button>
        </div>

        {#if fixNotice}
          <div class="tool-notice">{fixNotice}</div>
        {/if}

        {#if tagFixes.length > 0}
          <div class="fix-list">
            <div class="fix-header">
              <div class="fix-col">File</div>
              <div class="fix-col">Field</div>
              <div class="fix-col">Current</div>
              <div class="fix-col">Corrected</div>
            </div>
            {#each tagFixes as fix}
              <div class="fix-row">
                <div class="fix-col fix-path">{fix.path.split(/[/\\]/).pop()}</div>
                <div class="fix-col fix-field">{fix.field}</div>
                <div class="fix-col fix-old">{fix.old_value || '(empty)'}</div>
                <div class="fix-col fix-new">{fix.new_value}</div>
              </div>
            {/each}
          </div>
          <button class="btn btn-primary" style="margin-top:12px;" on:click={applyFixes}>
            Apply {tagFixes.length} Fix{tagFixes.length === 1 ? '' : 'es'}
          </button>
        {/if}
      </div>

    <!-- ── Maintenance ──────────────────────────────────────── -->
    {:else}
      <div class="tool-section">
        <div class="tool-card-grid">
          <div class="tool-card">
            <div class="tool-card-title">Prune Missing Tracks</div>
            <div class="tool-card-desc">Remove tracks from the database whose files no longer exist on disk.</div>
            <button class="btn btn-ghost" on:click={pruneMissing} disabled={pruning}>
              {pruning ? 'Pruning...' : 'Prune'}
            </button>
            {#if pruneCount !== null}
              <div class="tool-card-result">{pruneCount} track{pruneCount === 1 ? '' : 's'} removed.</div>
            {/if}
          </div>

          <div class="tool-card">
            <div class="tool-card-title">Ingest Staging</div>
            <div class="tool-card-desc">Move completed downloads from the staging folder into your library, organized by artist/album.</div>
            <button class="btn btn-ghost" on:click={ingestStaging} disabled={ingesting}>
              {ingesting ? 'Ingesting...' : 'Ingest Now'}
            </button>
            {#if ingestCount !== null}
              <div class="tool-card-result">{ingestCount} file{ingestCount === 1 ? '' : 's'} ingested.</div>
            {/if}
            {#if ingestNotice}
              <div class="tool-card-result">{ingestNotice}</div>
            {/if}
            {#if ingestedPreview.length > 0}
              <div class="tool-mini-list">
                {#each ingestedPreview as filePath}
                  <div class="tool-mini-item">{shortPath(filePath)}</div>
                {/each}
              </div>
            {/if}
          </div>
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
.tools-page { display: flex; flex-direction: column; min-height: 100%; }

.tools-tabs {
  display: flex; gap: 0; padding: 0 1.5rem; border-bottom: 1px solid var(--border);
}
.tool-tab {
  padding: 10px 16px; font-size: 0.85rem; font-weight: 500;
  color: var(--text-secondary); cursor: pointer; background: none;
  border: none; border-bottom: 2px solid transparent; margin-bottom: -1px;
  transition: color 0.15s;
}
.tool-tab:hover { color: var(--text-primary); }
.tool-tab.active { color: var(--accent-bright); border-bottom-color: var(--accent); }

.tools-content { flex: 1; overflow-y: auto; }
.tool-section { padding: 1.25rem 1.5rem; }

.tool-desc {
  font-size: 0.82rem; color: var(--text-secondary); line-height: 1.5; margin-bottom: 12px; max-width: 600px;
}
.tool-desc code { background: var(--bg-active); padding: 2px 5px; border-radius: 3px; font-size: 0.78rem; }

.tool-actions { display: flex; align-items: center; gap: 10px; margin-bottom: 12px; flex-wrap: wrap; }

.tool-error {
  padding: 8px 12px; border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--error) 12%, var(--bg-card)); border: 1px solid var(--error);
  color: var(--error); font-size: 0.8rem; margin-bottom: 10px;
}
.tool-notice {
  padding: 8px 12px; border-radius: var(--radius-sm);
  border: 1px solid var(--border-active); background: color-mix(in srgb, var(--accent) 12%, var(--bg-card));
  color: var(--text-primary); font-size: 0.8rem; margin-bottom: 10px;
}

.tool-stats { display: flex; gap: 16px; font-size: 0.8rem; color: var(--text-secondary); margin-bottom: 12px; }
.stat-err { color: var(--error); }

.move-list { display: flex; flex-direction: column; gap: 4px; max-height: 400px; overflow-y: auto; }
.move-row {
  display: grid; grid-template-columns: 1fr auto 1fr; gap: 8px; align-items: center;
  padding: 6px 10px; border-radius: var(--radius-sm); background: var(--bg-card);
  border: 1px solid var(--border); font-size: 0.78rem;
}
.move-from { color: var(--text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.move-arrow { color: var(--accent); font-weight: 600; }
.move-to { color: var(--text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.move-more { font-size: 0.75rem; color: var(--text-muted); padding: 6px 0; }

.dupe-list { display: flex; flex-direction: column; gap: 12px; }
.dupe-group {
  background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius-sm);
  padding: 10px 14px;
}
.dupe-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; }
.dupe-key { font-size: 0.82rem; font-weight: 600; }
.dupe-track {
  display: flex; align-items: center; gap: 10px; padding: 4px 8px; font-size: 0.78rem;
  border-radius: var(--radius-sm);
}
.dupe-track.is-best { background: color-mix(in srgb, var(--accent) 8%, transparent); }
.dupe-format { font-weight: 600; min-width: 36px; }
.dupe-quality { color: var(--text-secondary); min-width: 140px; }
.dupe-size { color: var(--text-muted); min-width: 60px; }
.dupe-path { color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; }

.fix-list { display: flex; flex-direction: column; gap: 2px; }
.fix-header, .fix-row {
  display: grid; grid-template-columns: 1.5fr 0.7fr 1fr 1fr; gap: 8px; padding: 6px 10px; font-size: 0.78rem;
}
.fix-header { color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.05em; font-size: 0.7rem; font-weight: 600; border-bottom: 1px solid var(--border); }
.fix-row { background: var(--bg-card); border-radius: var(--radius-sm); }
.fix-path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.fix-field { font-weight: 600; color: var(--accent-bright); }
.fix-old { color: var(--text-muted); text-decoration: line-through; }
.fix-new { color: var(--text-primary); font-weight: 500; }

.tool-card-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 12px; }
.tool-card {
  background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius-sm);
  padding: 16px 20px; display: flex; flex-direction: column; gap: 8px;
}
.tool-card-title { font-weight: 600; font-size: 0.95rem; }
.tool-card-desc { font-size: 0.8rem; color: var(--text-secondary); line-height: 1.5; }
.tool-card-result { font-size: 0.8rem; color: var(--accent-bright); }

.tool-mini-list {
  margin-top: 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg-card);
}

.tool-mini-item {
  font-size: 0.76rem;
  color: var(--text-secondary);
  padding: 6px 10px;
  border-top: 1px solid var(--border);
}

.tool-mini-item:first-child {
  border-top: none;
}
</style>
