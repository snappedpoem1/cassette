<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type Track } from '$lib/api/tauri';
  import { tracks } from '$lib/stores/library';
  import { currentTrack } from '$lib/stores/player';
  import { queueTracks } from '$lib/stores/queue';
  import { formatDuration } from '$lib/utils';

  interface SessionMode {
    id: string;
    name: string;
    trackCount: number;
    slope: number;
    era: 'mixed' | 'recent' | 'classic';
    durationTargetSec: number;
  }

  interface TransitionReason {
    toTrackId: number;
    reason: string;
  }

  let composerName = 'Evening Flow';
  let trackCountTarget = 14;
  let slope = 0.2;
  let era: SessionMode['era'] = 'mixed';
  let durationTargetMin = 5;

  let generated: Track[] = [];
  let reasons: TransitionReason[] = [];
  let generating = false;

  let savedModes: SessionMode[] = [];
  let selectedMode = '';

  let feedbackMap: Record<string, number> = {};

  const SETTINGS_MODES = 'ui_session_composer_modes_json';
  const SETTINGS_FEEDBACK = 'ui_session_composer_feedback_json';

  function clamp(v: number, min: number, max: number): number {
    return Math.max(min, Math.min(max, v));
  }

  function pickRandom<T>(arr: T[]): T | null {
    if (arr.length === 0) {
      return null;
    }
    return arr[Math.floor(Math.random() * arr.length)] ?? null;
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
    if (artistCarry > 0.6) {
      reasonParts.push('artist continuity');
    }
    if (durationScore > 0.72) {
      reasonParts.push('energy slope fit');
    }
    if (eraScore > 0.72) {
      reasonParts.push(era === 'mixed' ? 'era blend' : `${era} era fit`);
    }
    if (personalHistory > 0.22) {
      reasonParts.push('personal replay signal');
    }

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
    await api.setSetting(SETTINGS_MODES, JSON.stringify(savedModes));
  }

  async function persistFeedback() {
    await api.setSetting(SETTINGS_FEEDBACK, JSON.stringify(feedbackMap));
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

  async function generateSession() {
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
      const why: TransitionReason[] = [];
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
          if (!candidate || used.has(candidate.id)) {
            continue;
          }
          if ((candidate.duration_secs || 0) < 50) {
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
    await queueTracks(generated, 0);
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

  onMount(() => {
    void loadComposerState();
  });
</script>

<section class="session-composer card">
  <div class="composer-head">
    <div>
      <div class="section-kicker">Session Composer</div>
      <h2>Build explainable listening arcs</h2>
    </div>
    <div class="composer-actions">
      <button class="btn btn-secondary" on:click={saveMode}>Save mode</button>
      <button class="btn btn-primary" on:click={generateSession} disabled={generating}>
        {generating ? 'Composing…' : 'Generate arc'}
      </button>
      <button class="btn btn-ghost" on:click={queueSession} disabled={generated.length === 0}>Queue session</button>
    </div>
  </div>

  <div class="composer-grid">
    <label>
      Mode name
      <input class="input" bind:value={composerName} />
    </label>
    <label>
      Saved mode
      <select class="input" bind:value={selectedMode} on:change={() => applyMode(selectedMode)}>
        <option value="">(none)</option>
        {#each savedModes as mode}
          <option value={mode.id}>{mode.name}</option>
        {/each}
      </select>
    </label>
    <label>
      Track count
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
  </div>

  {#if generated.length > 0}
    <div class="session-list">
      {#each generated as track, idx}
        <div class="session-row">
          <div class="session-main">
            <span class="session-index">{idx + 1}</span>
            <span class="session-track">{track.artist} · {track.title}</span>
            <span class="session-meta">{track.year ?? '—'} · {formatDuration(track.duration_secs)}</span>
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
    <div class="composer-empty">Generate a session to see explainable transitions and collect replay/skip feedback.</div>
  {/if}
</section>

<style>
  .session-composer {
    margin-top: 6px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .composer-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 10px;
  }

  .composer-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
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

  .composer-hint {
    font-size: 0.66rem;
    color: var(--text-muted);
  }

  .session-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .session-row {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 7px 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-card);
  }

  .session-main {
    display: grid;
    grid-template-columns: 26px 1fr auto;
    gap: 8px;
    align-items: center;
  }

  .session-index {
    color: var(--text-muted);
    font-size: 0.72rem;
    text-align: center;
  }

  .session-track {
    font-size: 0.82rem;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .session-meta {
    font-size: 0.72rem;
    color: var(--text-muted);
  }

  .session-reason {
    display: flex;
    align-items: center;
    gap: 6px;
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
    padding: 12px;
    border: 1px dashed var(--border);
    border-radius: var(--radius-sm);
  }

  @media (max-width: 1120px) {
    .composer-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
