<script lang="ts">
  import { onMount } from 'svelte';
  import { api, isDesktopRuntimeAvailable, type PlaylistItem, type Track } from '$lib/api/tauri';
  import { createPlaylist, loadPlaylists, playlists } from '$lib/stores/playlists';
  import { currentTrack } from '$lib/stores/player';
  import { loadLibrary, tracks } from '$lib/stores/library';
  import { replaceQueueTrackIds } from '$lib/stores/queue';
  import { formatDuration } from '$lib/utils';
  import {
    branchSessionRecord,
    bumpSessionPlayCount,
    crates,
    deleteSessionRecord,
    loadRituals,
    queueScenes,
    saveSessionRecord,
    sessionLibrary,
    type SessionRecord,
    type SessionModeSnapshot,
    type SessionTransitionReason,
  } from '$lib/stores/rituals';
  import { hydrateTrackIds } from '$lib/ritual-helpers';

  interface SessionMode {
    id: string;
    name: string;
    trackCount: number;
    slope: number;
    era: 'mixed' | 'recent' | 'classic';
    durationTargetSec: number;
  }

  let composerName = 'Evening Flow';
  let noteDraft = '';
  let trackCountTarget = 14;
  let slope = 0.2;
  let era: SessionMode['era'] = 'mixed';
  let durationTargetMin = 5;
  let importLane: 'generated' | 'playlist' | 'crate' | 'queue_scene' = 'generated';
  let importRefId = '';

  let generated: Track[] = [];
  let reasons: SessionTransitionReason[] = [];
  let generating = false;

  let savedModes: SessionMode[] = [];
  let selectedMode = '';
  let feedbackMap: Record<string, number> = {};
  let importedPlaylistItems: PlaylistItem[] = [];
  let selectedSessionId: string | null = null;
  let composerMessage: string | null = null;
  let composerMessageTone: 'info' | 'error' = 'info';

  const SETTINGS_MODES = 'ui_session_composer_modes_json';
  const SETTINGS_FEEDBACK = 'ui_session_composer_feedback_json';

  function toComposerMessage(error: unknown, fallback: string): string {
    const message = error instanceof Error ? error.message : fallback;
    if (message.toLowerCase().includes('tauri runtime unavailable')) {
      return 'This action needs the Cassette desktop runtime. Open the desktop app to use it.';
    }
    return message;
  }

  function setComposerMessage(message: string, tone: 'info' | 'error' = 'info') {
    composerMessage = message;
    composerMessageTone = tone;
  }

  function clamp(v: number, min: number, max: number): number {
    return Math.max(min, Math.min(max, v));
  }

  function pickRandom<T>(arr: T[]): T | null {
    if (arr.length === 0) {
      return null;
    }
    return arr[Math.floor(Math.random() * arr.length)] ?? null;
  }

  function modeSnapshot(): SessionModeSnapshot {
    return {
      trackCount: clamp(trackCountTarget, 6, 30),
      slope: clamp(slope, -1, 1),
      era,
      durationTargetMin: clamp(durationTargetMin, 2, 12),
    };
  }

  function sourceTrackIds(): number[] {
    if (importLane === 'playlist') {
      return importedPlaylistItems.map((item) => item.track_id);
    }
    if (importLane === 'crate') {
      return $crates.find((crate) => crate.id === importRefId)?.trackIds ?? [];
    }
    if (importLane === 'queue_scene') {
      return $queueScenes.find((scene) => scene.id === importRefId)?.trackIds ?? [];
    }
    return [];
  }

  function scoreTransition(from: Track, to: Track, index: number, total: number): { score: number; reason: string } {
    const arc = total <= 1 ? 0 : index / (total - 1);
    const slopeDrift = slope * arc;

    const fromDur = from.duration_secs || 180;
    const toDur = to.duration_secs || 180;
    const targetDur = clamp(fromDur * (1 + slopeDrift * 0.24), 120, 500);

    const durationGap = Math.abs(toDur - targetDur) / targetDur;
    const durationScore = 1 - Math.min(1, durationGap);

    const artistCarry = from.artist === to.artist ? 0.9 : 0;

    const yearA = from.year ?? 0;
    const yearB = to.year ?? 0;
    const yearGap = yearA > 0 && yearB > 0 ? Math.abs(yearA - yearB) : 18;
    const yearScoreBase = 1 - Math.min(1, yearGap / 22);

    let eraScore = yearScoreBase;
    if (era === 'recent' && yearB > 0) {
      eraScore = Math.max(yearScoreBase, yearB >= 2016 ? 0.9 : 0.2);
    } else if (era === 'classic' && yearB > 0) {
      eraScore = Math.max(yearScoreBase, yearB <= 2005 ? 0.88 : 0.2);
    }

    const qualityScore = ['flac', 'wav', 'aiff', 'alac'].includes((to.format || '').toLowerCase()) ? 0.55 : 0.2;
    const pairKey = `${from.id}->${to.id}`;
    const artistKey = `${from.artist}->${to.artist}`;
    const personalHistory = (feedbackMap[pairKey] ?? 0) * 0.35 + (feedbackMap[artistKey] ?? 0) * 0.25;

    const score = durationScore * 0.34 + artistCarry * 0.2 + eraScore * 0.22 + qualityScore * 0.12 + personalHistory;

    const reasonParts: string[] = [];
    if (artistCarry > 0.6) reasonParts.push('artist continuity');
    if (durationScore > 0.72) reasonParts.push('energy slope fit');
    if (eraScore > 0.72) reasonParts.push(era === 'mixed' ? 'era blend' : `${era} era fit`);
    if (personalHistory > 0.22) reasonParts.push('personal replay signal');

    return {
      score,
      reason: reasonParts.length > 0 ? reasonParts.slice(0, 2).join(' + ') : 'balanced transition',
    };
  }

  async function loadComposerState() {
    try {
      const [modesRaw, feedbackRaw] = await Promise.all([
        api.getSetting(SETTINGS_MODES),
        api.getSetting(SETTINGS_FEEDBACK),
      ]);
      savedModes = modesRaw ? (JSON.parse(modesRaw) as SessionMode[]) : [];
      feedbackMap = feedbackRaw ? (JSON.parse(feedbackRaw) as Record<string, number>) : {};
      if (savedModes.length > 0 && !selectedMode) {
        selectedMode = savedModes[0].id;
      }
    } catch {
      savedModes = [];
      feedbackMap = {};
    }
  }

  async function persistModes() {
    try {
      await api.setSetting(SETTINGS_MODES, JSON.stringify(savedModes));
    } catch {
      // no-op outside Tauri runtime
    }
  }

  async function persistFeedback() {
    try {
      await api.setSetting(SETTINGS_FEEDBACK, JSON.stringify(feedbackMap));
    } catch {
      // no-op outside Tauri runtime
    }
  }

  function applyMode(modeId: string) {
    const mode = savedModes.find((item) => item.id === modeId);
    if (!mode) {
      return;
    }
    composerName = mode.name;
    trackCountTarget = mode.trackCount;
    slope = mode.slope;
    era = mode.era;
    durationTargetMin = Math.max(2, Math.round(mode.durationTargetSec / 60));
  }

  async function saveMode() {
    const mode: SessionMode = {
      id: `${Date.now()}`,
      name: composerName.trim() || `Session ${savedModes.length + 1}`,
      trackCount: clamp(trackCountTarget, 6, 30),
      slope: clamp(slope, -1, 1),
      era,
      durationTargetSec: clamp(Math.round(durationTargetMin * 60), 120, 720),
    };
    savedModes = [mode, ...savedModes].slice(0, 12);
    selectedMode = mode.id;
    await persistModes();
  }

  async function refreshImportLane() {
    if (importLane !== 'playlist' || !importRefId) {
      importedPlaylistItems = [];
      return;
    }
    try {
      importedPlaylistItems = await api.getPlaylistItems(Number.parseInt(importRefId, 10));
    } catch {
      importedPlaylistItems = [];
    }
  }

  async function generateSession() {
    const importedIds = sourceTrackIds();
    if (importLane !== 'generated' && importedIds.length > 0) {
      generated = hydrateTrackIds(importedIds, new Map($tracks.map((track) => [track.id, track])));
      reasons = generated.slice(1).map((track) => ({
        toTrackId: track.id,
        reason: `imported from ${importLane.replace('_', ' ')}`,
      }));
      return;
    }

    const pool = $tracks;
    if (pool.length < 10) {
      generated = [];
      reasons = [];
      return;
    }

    generating = true;
    try {
      const used = new Set<number>();
      const picks: Track[] = [];
      const why: SessionTransitionReason[] = [];
      const total = clamp(trackCountTarget, 6, 30);

      const seed = $currentTrack ?? pickRandom(pool);
      if (!seed) {
        generated = [];
        reasons = [];
        return;
      }
      picks.push(seed);
      used.add(seed.id);

      while (picks.length < total) {
        const from = picks[picks.length - 1];
        let best: Track | null = null;
        let bestScore = -1;
        let bestReason = 'balanced transition';

        const sampleSize = Math.min(260, pool.length);
        for (let i = 0; i < sampleSize; i += 1) {
          const candidate = pool[Math.floor(Math.random() * pool.length)];
          if (!candidate || used.has(candidate.id) || (candidate.duration_secs || 0) < 50) {
            continue;
          }
          const { score, reason } = scoreTransition(from, candidate, picks.length, total);
          if (score > bestScore) {
            best = candidate;
            bestScore = score;
            bestReason = reason;
          }
        }

        if (!best) {
          break;
        }

        picks.push(best);
        used.add(best.id);
        why.push({ toTrackId: best.id, reason: bestReason });
      }

      generated = picks;
      reasons = why;
    } finally {
      generating = false;
    }
  }

  async function queueSession() {
    if (generated.length === 0) {
      return;
    }
    try {
      await replaceQueueTrackIds(generated.map((track) => track.id), 0);
      if (selectedSessionId) {
        await bumpSessionPlayCount(selectedSessionId);
      }
    } catch (error) {
      setComposerMessage(toComposerMessage(error, 'Failed to replay session arc.'), 'error');
    }
  }

  async function saveCurrentSession() {
    if (generated.length === 0) {
      return;
    }
    try {
      selectedSessionId = await saveSessionRecord({
        id: selectedSessionId ?? undefined,
        name: composerName.trim() || 'Untitled session',
        note: noteDraft.trim(),
        trackIds: generated.map((track) => track.id),
        reasons,
        source: importLane === 'generated' ? 'generated' : importLane,
        sourceRefId: importRefId || null,
        branchOfId: null,
        modeSnapshot: modeSnapshot(),
      });
      setComposerMessage('Session memory saved.', 'info');
    } catch (error) {
      setComposerMessage(toComposerMessage(error, 'Failed to save session memory.'), 'error');
    }
  }

  async function branchCurrentSession() {
    if (generated.length === 0) {
      return;
    }

    if (selectedSessionId) {
      try {
        const branchedId = await branchSessionRecord(selectedSessionId, {
          name: `${composerName.trim() || 'Session'} / branch`,
          note: noteDraft.trim(),
          trackIds: generated.map((track) => track.id),
          reasons,
          modeSnapshot: modeSnapshot(),
        });
        if (branchedId) {
          selectedSessionId = branchedId;
        }
        setComposerMessage('Session branch saved.', 'info');
      } catch (error) {
        setComposerMessage(toComposerMessage(error, 'Failed to branch session.'), 'error');
      }
      return;
    }

    try {
      selectedSessionId = await saveSessionRecord({
        name: `${composerName.trim() || 'Session'} / branch`,
        note: noteDraft.trim(),
        trackIds: generated.map((track) => track.id),
        reasons,
        source: 'branch',
        sourceRefId: null,
        branchOfId: null,
        modeSnapshot: modeSnapshot(),
      });
      setComposerMessage('Session branch saved.', 'info');
    } catch (error) {
      setComposerMessage(toComposerMessage(error, 'Failed to branch session.'), 'error');
    }
  }

  async function exportToPlaylist() {
    if (generated.length === 0) {
      return;
    }
    try {
      await createPlaylist(composerName.trim() || 'Session export', noteDraft.trim() || null, generated.map((track) => track.id));
      setComposerMessage('Session exported to playlist.', 'info');
    } catch (error) {
      setComposerMessage(toComposerMessage(error, 'Failed to export session to playlist.'), 'error');
    }
  }

  async function rateTransition(index: number, value: number) {
    if (index <= 0 || index >= generated.length) {
      return;
    }
    const from = generated[index - 1];
    const to = generated[index];
    const pairKey = `${from.id}->${to.id}`;
    const artistKey = `${from.artist}->${to.artist}`;
    feedbackMap[pairKey] = clamp((feedbackMap[pairKey] ?? 0) + value, -2, 2);
    feedbackMap[artistKey] = clamp((feedbackMap[artistKey] ?? 0) + value * 0.6, -2, 2);
    await persistFeedback();
  }

  function loadSavedSession(session: SessionRecord) {
    selectedSessionId = session.id;
    composerName = session.name;
    noteDraft = session.note;
    generated = hydrateTrackIds(session.trackIds, new Map($tracks.map((track) => [track.id, track])));
    reasons = session.reasons;
    if (session.modeSnapshot) {
      trackCountTarget = session.modeSnapshot.trackCount;
      slope = session.modeSnapshot.slope;
      era = session.modeSnapshot.era;
      durationTargetMin = session.modeSnapshot.durationTargetMin;
    }
  }

  onMount(async () => {
    await Promise.all([loadComposerState(), loadLibrary(), loadRituals(), loadPlaylists()]);
  });

  $: if (importLane === 'playlist') {
    void refreshImportLane();
  }

  $: if (!isDesktopRuntimeAvailable() && !composerMessage) {
    composerMessage = 'Preview mode detected. Desktop-only actions are unavailable until you run the Cassette desktop app.';
    composerMessageTone = 'info';
  }
</script>

<section class="session-composer card">
  <div class="composer-head">
    <div>
      <div class="section-kicker">Session memory</div>
      <h2>Compose, replay, and branch the arc</h2>
    </div>
    <div class="composer-actions">
      <button class="btn btn-secondary" on:click={saveMode}>Save shape</button>
      <button class="btn btn-primary" on:click={generateSession} disabled={generating}>
        {generating ? 'Composing...' : importLane === 'generated' ? 'Generate arc' : 'Import arc'}
      </button>
      <button class="btn btn-ghost" on:click={queueSession} disabled={generated.length === 0}>Replay arc</button>
    </div>
  </div>

  {#if composerMessage}
    <div class="composer-notice" class:error={composerMessageTone === 'error'}>{composerMessage}</div>
  {/if}

  <div class="composer-layout">
    <aside class="session-memory">
      <div class="memory-head">
        <div class="section-kicker subtle">Memory shelf</div>
        <div class="memory-copy">Saved sessions stay here for replay, branching, and clean export.</div>
      </div>

      {#if $sessionLibrary.length === 0}
        <div class="composer-empty">No saved sessions yet. Save one once an arc deserves a return path.</div>
      {:else}
        <div class="memory-list">
          {#each $sessionLibrary as session}
            <div
              class="memory-item"
              class:active={session.id === selectedSessionId}
              role="button"
              tabindex="0"
              on:click={() => loadSavedSession(session)}
              on:keydown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  loadSavedSession(session);
                }
              }}
            >
              <span class="memory-item-head">
                <span class="memory-item-title">{session.name}</span>
                <span class="memory-item-count">{session.playCount} plays</span>
              </span>
              <span class="memory-item-meta">{session.trackIds.length} tracks / {session.source.replace('_', ' ')}</span>
              <span class="memory-item-actions">
                <button class="memory-link" on:click|stopPropagation={() => replaceQueueTrackIds(session.trackIds, 0)}>Replay</button>
                <button class="memory-link danger" on:click|stopPropagation={() => deleteSessionRecord(session.id)}>Delete</button>
              </span>
            </div>
          {/each}
        </div>
      {/if}
    </aside>

    <div class="composer-main">
      <div class="composer-grid">
        <label>
          Session name
          <input class="input" bind:value={composerName} />
        </label>
        <label>
          Import lane
          <select class="input" bind:value={importLane}>
            <option value="generated">Generate from collection</option>
            <option value="playlist">Import from playlist</option>
            <option value="crate">Import from crate</option>
            <option value="queue_scene">Import from queue scene</option>
          </select>
        </label>
        {#if importLane !== 'generated'}
          <label>
            Source
            <select class="input" bind:value={importRefId}>
              <option value="">Choose source</option>
              {#if importLane === 'playlist'}
                {#each $playlists as playlist}
                  <option value={playlist.id}>{playlist.name}</option>
                {/each}
              {:else if importLane === 'crate'}
                {#each $crates as crate}
                  <option value={crate.id}>{crate.name}</option>
                {/each}
              {:else}
                {#each $queueScenes as scene}
                  <option value={scene.id}>{scene.name}</option>
                {/each}
              {/if}
            </select>
          </label>
        {/if}
        <label>
          Saved shape
          <select class="input" bind:value={selectedMode} on:change={() => applyMode(selectedMode)}>
            <option value="">(none)</option>
            {#each savedModes as mode}
              <option value={mode.id}>{mode.name}</option>
            {/each}
          </select>
        </label>
        <label>
          Run length
          <input class="input" type="number" min="6" max="30" bind:value={trackCountTarget} />
        </label>
        <label>
          Energy slope
          <input class="input" type="range" min="-1" max="1" step="0.1" bind:value={slope} />
          <span class="composer-hint">{slope.toFixed(1)} (negative = cool-down, positive = build-up)</span>
        </label>
        <label>
          Era bias
          <select class="input" bind:value={era}>
            <option value="mixed">Mixed</option>
            <option value="recent">Recent</option>
            <option value="classic">Classic</option>
          </select>
        </label>
        <label>
          Duration target (min)
          <input class="input" type="number" min="2" max="12" bind:value={durationTargetMin} />
        </label>
        <label class="composer-span">
          Session note
          <textarea class="input textarea" bind:value={noteDraft} placeholder="What this arc is for, or what changed when you branched it."></textarea>
        </label>
      </div>

      <div class="session-actions-bar">
        <button class="btn btn-primary" on:click={saveCurrentSession} disabled={generated.length === 0}>Save memory</button>
        <button class="btn btn-secondary" on:click={branchCurrentSession} disabled={generated.length === 0}>Branch arc</button>
        <button class="btn btn-ghost" on:click={exportToPlaylist} disabled={generated.length === 0}>Export to playlist</button>
      </div>

      {#if generated.length > 0}
        <div class="session-list">
          {#each generated as track, idx}
            <div class="session-row">
              <div class="session-main">
                <span class="session-index">{idx + 1}</span>
                <span class="session-track">{track.artist} / {track.title}</span>
                <span class="session-meta">{track.year ?? '-'} / {formatDuration(track.duration_secs)}</span>
              </div>
              {#if idx > 0}
                <div class="session-reason">
                  <span>{reasons[idx - 1]?.reason ?? 'balanced transition'}</span>
                  <button class="feedback-btn" on:click={() => rateTransition(idx, 0.35)}>Replay+</button>
                  <button class="feedback-btn" on:click={() => rateTransition(idx, -0.35)}>Skip-</button>
                </div>
              {/if}
            </div>
          {/each}
        </div>
      {:else}
        <div class="composer-empty">Generate or import an arc to inspect the handoffs, save the memory, or branch it into a new line.</div>
      {/if}
    </div>
  </div>
</section>

<style>
  .session-composer {
    margin-top: 6px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.02), transparent 24%),
      var(--bg-card);
  }

  .section-kicker {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--accent-bright);
    font-weight: 700;
  }

  .section-kicker.subtle {
    color: var(--text-muted);
  }

  .composer-head,
  .memory-item-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 10px;
  }

  .composer-actions,
  .session-actions-bar,
  .memory-item-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .composer-layout {
    display: grid;
    grid-template-columns: 320px minmax(0, 1fr);
    gap: 16px;
  }

  .session-memory,
  .composer-main {
    display: grid;
    gap: 12px;
  }

  .memory-head {
    display: grid;
    gap: 6px;
  }

  .memory-copy {
    color: var(--text-secondary);
    font-size: 0.82rem;
    line-height: 1.6;
  }

  .memory-list,
  .session-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .memory-item,
  .session-row {
    display: grid;
    gap: 6px;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: rgba(255, 255, 255, 0.02);
  }

  .memory-item {
    text-align: left;
    transition: border-color 0.15s, background 0.15s;
  }

  .memory-item:hover,
  .memory-item.active {
    border-color: rgba(247, 180, 92, 0.32);
    background: rgba(247, 180, 92, 0.07);
  }

  .memory-item-title,
  .session-track {
    color: var(--text-primary);
    font-weight: 700;
  }

  .memory-item-count,
  .session-index,
  .session-meta {
    color: var(--text-muted);
    font-size: 0.72rem;
  }

  .memory-item-meta {
    color: var(--text-secondary);
    font-size: 0.76rem;
  }

  .memory-link {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }

  .memory-link.danger {
    color: color-mix(in srgb, var(--error) 80%, white);
  }

  .composer-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
  }

  .composer-grid label {
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: 0.76rem;
    color: var(--text-secondary);
  }

  .composer-span {
    grid-column: 1 / -1;
  }

  .composer-hint {
    font-size: 0.66rem;
    color: var(--text-muted);
  }

  .textarea {
    min-height: 92px;
    resize: vertical;
  }

  .session-main {
    display: grid;
    grid-template-columns: 26px 1fr auto;
    gap: 8px;
    align-items: center;
  }

  .session-track {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .session-reason {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    font-size: 0.72rem;
    color: var(--text-secondary);
  }

  .feedback-btn {
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 2px 8px;
    font-size: 0.64rem;
    color: var(--text-secondary);
    background: var(--bg-hover);
  }

  .feedback-btn:hover {
    color: var(--text-primary);
    border-color: var(--border-active);
  }

  .composer-empty {
    font-size: 0.78rem;
    color: var(--text-muted);
    padding: 14px;
    border: 1px dashed var(--border);
    border-radius: var(--radius-sm);
  }

  .composer-notice {
    border: 1px solid color-mix(in srgb, var(--primary) 45%, transparent);
    border-radius: var(--radius-sm);
    padding: 8px 10px;
    background: color-mix(in srgb, var(--bg-card) 86%, var(--primary) 14%);
    color: var(--text-primary);
    font-size: 0.76rem;
    line-height: 1.5;
  }

  .composer-notice.error {
    border-color: color-mix(in srgb, var(--error) 55%, transparent);
    background: color-mix(in srgb, var(--bg-card) 82%, var(--error) 18%);
  }

  @media (max-width: 1120px) {
    .composer-layout,
    .composer-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
