<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type AcquisitionRequestListItem, type TrackIdentityContext } from '$lib/api/tauri';
  import ContextActionRail from '$lib/components/ContextActionRail.svelte';
  import { deriveEditionContext, samePath } from '$lib/ownership';
  import { loadQueue, queue } from '$lib/stores/queue';
  import {
    isPlaying,
    isSeeking,
    nowPlayingContext,
    playbackState,
    player,
    progressPct,
    seekPreview,
  } from '$lib/stores/player';
  import { clamp, coverSrc, formatDuration } from '$lib/utils';

  export let fullPage = false;

  let seekBarEl: HTMLDivElement;
  let identityContext: TrackIdentityContext | null = null;
  let provenanceRequest: AcquisitionRequestListItem | null = null;
  let detailTrackGuard: number | null = null;

  onMount(() => {
    void loadQueue();
  });

  function getSeekPct(event: MouseEvent): number {
    const rect = seekBarEl.getBoundingClientRect();
    return clamp((event.clientX - rect.left) / rect.width, 0, 1);
  }

  function onSeekMouseDown(event: MouseEvent) {
    isSeeking.set(true);
    seekPreview.set(getSeekPct(event));

    const onMove = (moveEvent: MouseEvent) => seekPreview.set(getSeekPct(moveEvent));
    const onUp = async (upEvent: MouseEvent) => {
      const pct = getSeekPct(upEvent);
      isSeeking.set(false);
      await player.seek(pct);
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    };

    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  }

  async function onSeekKeyDown(event: KeyboardEvent) {
    if (!durationSecs || durationSecs <= 0) {
      return;
    }
    const step = event.shiftKey ? 0.1 : 0.03;
    let nextPct = seekPct;
    if (event.key === 'ArrowRight' || event.key === 'ArrowUp') {
      nextPct = clamp(seekPct + step, 0, 1);
    } else if (event.key === 'ArrowLeft' || event.key === 'ArrowDown') {
      nextPct = clamp(seekPct - step, 0, 1);
    } else if (event.key === 'Home') {
      nextPct = 0;
    } else if (event.key === 'End') {
      nextPct = 1;
    } else {
      return;
    }
    event.preventDefault();
    await player.seek(nextPct);
  }

  async function handleNext() {
    await player.next();
    await loadQueue();
  }

  async function handlePrev() {
    await player.prev();
    await loadQueue();
  }

  async function loadTrackDetail() {
    if (!$playbackState.current_track) {
      identityContext = null;
      provenanceRequest = null;
      return;
    }

    try {
      const [identity, requests] = await Promise.all([
        api.getTrackIdentityContext($playbackState.current_track.id),
        api.listAcquisitionRequests(undefined, 80),
      ]);
      identityContext = identity;
      provenanceRequest =
        requests.find((request) => request.final_path && samePath(request.final_path, $playbackState.current_track?.path ?? ''))
        ?? null;
    } catch {
      identityContext = null;
      provenanceRequest = null;
    }
  }

  $: track = $playbackState.current_track;
  $: positionSecs = $playbackState.position_secs;
  $: durationSecs = $playbackState.duration_secs;
  $: seekPct = $progressPct;
  $: context = $nowPlayingContext;
  $: queueItems = $queue;
  $: queuePosition = $playbackState.queue_position;
  $: nextItems = queueItems
    .filter((item) => item.position > queuePosition)
    .sort((a, b) => a.position - b.position)
    .slice(0, 6);
  $: edition = track && identityContext ? deriveEditionContext(track.album ?? track.title, [identityContext]) : null;
  $: editionLabel = edition?.bucket
    ? edition.bucket.replace(/_/g, ' ')
    : edition?.markers?.[0] ?? null;
  $: qualityChip = track?.quality_tier
    ? track.quality_tier.replace(/_/g, ' ')
    : track?.bit_depth && track?.sample_rate
      ? `${track.bit_depth}-bit / ${(track.sample_rate / 1000).toFixed(1)}kHz`
      : null;
  $: provenanceLabel = provenanceRequest?.selected_provider
    ? `${provenanceRequest.selected_provider} / ${provenanceRequest.execution_disposition ?? provenanceRequest.status}`
    : provenanceRequest?.trust_detail ?? null;

  $: if (track?.id !== detailTrackGuard) {
    detailTrackGuard = track?.id ?? null;
    void loadTrackDetail();
  }
</script>

<section class="shrine" class:full-page={fullPage}>
  <div class="shrine-main">
    <div class="shrine-art-shell">
      <div class="shrine-art">
        {#if track?.cover_art_path}
          <img src={coverSrc(track.cover_art_path)} alt="Album artwork" />
        {:else}
          <div class="shrine-art-ph">No Art</div>
        {/if}
      </div>
    </div>

    <div class="shrine-meta">
      <div class="section-kicker">Immersion ritual</div>
      <h1>{track?.title ?? 'Nothing playing'}</h1>
      <p>{track?.artist ?? 'Choose something to start listening'}</p>
      {#if track?.album}
        <span>{track.album}</span>
      {/if}
    </div>

    <div class="shrine-chip-row">
      {#if qualityChip}
        <span class="shrine-chip shrine-chip-strong">{qualityChip}</span>
      {/if}
      {#if track?.format}
        <span class="shrine-chip">{track.format.toUpperCase()}</span>
      {/if}
      {#if editionLabel}
        <span class="shrine-chip">{editionLabel}</span>
      {/if}
      {#if provenanceLabel}
        <span class="shrine-chip shrine-chip-accent">{provenanceLabel}</span>
      {/if}
    </div>

    <div class="shrine-controls">
      <button class="shrine-ctrl" type="button" on:click={handlePrev}>Prev</button>
      <button class="shrine-ctrl shrine-ctrl-play" type="button" on:click={() => player.toggle()}>
        {$isPlaying ? 'Pause' : 'Play'}
      </button>
      <button class="shrine-ctrl" type="button" on:click={handleNext}>Next</button>
    </div>

    <div class="shrine-seek">
      <span>{formatDuration(positionSecs)}</span>
      <div
        class="shrine-seek-bar"
        bind:this={seekBarEl}
        role="slider"
        tabindex="0"
        aria-label="Seek playback position"
        aria-valuemin="0"
        aria-valuemax="100"
        aria-valuenow={Math.round(seekPct * 100)}
        aria-valuetext={`${formatDuration(positionSecs)} of ${formatDuration(durationSecs)}`}
        on:mousedown={onSeekMouseDown}
        on:keydown={onSeekKeyDown}
      >
        <div class="shrine-seek-fill" style="width:{seekPct * 100}%"></div>
        <div class="shrine-seek-thumb" style="left:{seekPct * 100}%"></div>
      </div>
      <span>{formatDuration(durationSecs)}</span>
    </div>

    {#if track}
      <ContextActionRail
        compact
        track={track}
        album={track.album ? { artist: track.artist, title: track.album } : null}
        artistName={track.artist}
      />
    {/if}
  </div>

  <aside class="shrine-side">
    <section class="shrine-block">
      <h3>Provenance</h3>
      {#if provenanceRequest}
        <div class="shrine-detail-row">
          <span class="shrine-detail-label">Latest path</span>
          <strong>{provenanceRequest.execution_disposition ?? provenanceRequest.status}</strong>
        </div>
        <div class="shrine-detail-row">
          <span class="shrine-detail-label">Source</span>
          <strong>{provenanceRequest.selected_provider ?? 'Captured in history'}</strong>
        </div>
        <div class="shrine-detail-row">
          <span class="shrine-detail-label">Trust</span>
          <strong>{provenanceRequest.trust_detail}</strong>
        </div>
      {:else}
        <div class="shrine-empty">No saved provenance was matched for the current file yet.</div>
      {/if}
    </section>

    <section class="shrine-block">
      <h3>Context</h3>
      {#if context?.artist_summary || context?.album_summary}
        {#if context.artist_summary}
          <p class="shrine-copy">{context.artist_summary}</p>
        {/if}
        {#if context.album_summary}
          <p class="shrine-copy">{context.album_summary}</p>
        {/if}
      {:else}
        <div class="shrine-empty">No extra context is loaded yet for this track.</div>
      {/if}
    </section>

    <section class="shrine-block">
      <h3>Lyrics</h3>
      {#if context?.synced_lyrics || context?.lyrics}
        <pre>{context.synced_lyrics || context.lyrics}</pre>
      {:else}
        <div class="shrine-empty">Lyrics have not landed for this track yet.</div>
      {/if}
    </section>

    <section class="shrine-block">
      <h3>Up next</h3>
      {#if nextItems.length > 0}
        <ul>
          {#each nextItems as item}
            <li>
              <div class="shrine-queue-title">{item.track?.title ?? 'Unknown track'}</div>
              <div class="shrine-queue-meta">{item.track?.artist ?? 'Unknown artist'}</div>
            </li>
          {/each}
        </ul>
      {:else}
        <div class="shrine-empty">Queue is empty.</div>
      {/if}
    </section>
  </aside>
</section>

<style>
  .shrine {
    display: grid;
    grid-template-columns: minmax(0, 1.7fr) 360px;
    gap: 18px;
    min-height: 0;
  }

  .shrine.full-page {
    min-height: calc(100vh - var(--topbar-h) - var(--nowplaying-h) - 40px);
  }

  .shrine-main,
  .shrine-block {
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 22px;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 26%),
      rgba(10, 14, 22, 0.94);
    box-shadow: 0 24px 70px rgba(0, 0, 0, 0.3);
  }

  .shrine-main {
    padding: 24px;
    display: grid;
    gap: 18px;
  }

  .shrine-art-shell {
    display: flex;
    justify-content: center;
  }

  .shrine-art {
    width: min(60vh, 520px);
    aspect-ratio: 1;
    border-radius: 24px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.06);
    box-shadow: 0 24px 52px rgba(0, 0, 0, 0.38);
  }

  .shrine-art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .shrine-art-ph {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
  }

  .shrine-meta {
    display: grid;
    gap: 4px;
    text-align: center;
  }

  .section-kicker {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--accent-bright);
    font-weight: 700;
  }

  .shrine-meta h1 {
    font-size: clamp(2rem, 4vw, 3.4rem);
    line-height: 0.95;
  }

  .shrine-meta p {
    color: var(--text-secondary);
    font-size: 1rem;
  }

  .shrine-meta span {
    color: var(--text-muted);
    font-size: 0.82rem;
  }

  .shrine-chip-row {
    display: flex;
    justify-content: center;
    flex-wrap: wrap;
    gap: 8px;
  }

  .shrine-chip {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-secondary);
    background: rgba(113, 131, 152, 0.16);
    border: 1px solid rgba(113, 131, 152, 0.22);
    padding: 4px 10px;
    border-radius: 999px;
  }

  .shrine-chip-strong {
    color: var(--text-accent);
    border-color: rgba(139, 180, 212, 0.24);
    background: rgba(139, 180, 212, 0.12);
  }

  .shrine-chip-accent {
    color: var(--accent-bright);
    border-color: rgba(247, 180, 92, 0.22);
    background: rgba(247, 180, 92, 0.1);
  }

  .shrine-controls {
    display: flex;
    gap: 10px;
    align-items: center;
    justify-content: center;
  }

  .shrine-ctrl {
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 999px;
    background: transparent;
    color: var(--text-secondary);
    padding: 9px 16px;
    font-size: 0.82rem;
  }

  .shrine-ctrl-play {
    background: var(--primary);
    border-color: var(--primary);
    color: var(--bg-deep);
    font-weight: 700;
  }

  .shrine-seek {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 10px;
  }

  .shrine-seek span {
    color: var(--text-secondary);
    font-size: 0.76rem;
    min-width: 36px;
  }

  .shrine-seek-bar {
    position: relative;
    height: 6px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.12);
    cursor: pointer;
  }

  .shrine-seek-fill {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--primary);
    border-radius: 999px;
  }

  .shrine-seek-thumb {
    position: absolute;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--primary);
    opacity: 0;
  }

  .shrine-seek:hover .shrine-seek-thumb {
    opacity: 1;
  }

  .shrine-side {
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .shrine-block {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
  }

  .shrine-block h3 {
    margin: 0;
    color: var(--text-primary);
    font-size: 0.84rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .shrine-copy,
  .shrine-empty,
  .shrine-detail-label,
  .shrine-queue-meta {
    color: var(--text-secondary);
    font-size: 0.8rem;
    line-height: 1.65;
  }

  .shrine-detail-row {
    display: flex;
    justify-content: space-between;
    gap: 10px;
  }

  .shrine-block pre {
    margin: 0;
    white-space: pre-wrap;
    line-height: 1.8;
    color: var(--text-secondary);
    font-size: 0.8rem;
    overflow-y: auto;
    max-height: 280px;
  }

  .shrine-block ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .shrine-block li {
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 10px;
    padding: 10px;
    background: rgba(255, 255, 255, 0.02);
  }

  .shrine-queue-title {
    color: var(--text-primary);
    font-size: 0.82rem;
  }

  @media (max-width: 1120px) {
    .shrine {
      grid-template-columns: 1fr;
    }

    .shrine-art {
      width: 100%;
      max-width: 360px;
    }
  }
</style>
