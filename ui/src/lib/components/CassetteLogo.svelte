<script lang="ts">
  export let size: number = 40;
  export let spinning: boolean = false;
  export let withWordmark: boolean = false;
  export let color: string = 'var(--accent)';

  // Scale factors relative to a 56×40 base icon
  $: scale = size / 40;
  $: w = Math.round(56 * scale);
  $: h = Math.round(40 * scale);
</script>

<div class="cassette-logo" class:with-wordmark={withWordmark}>
  <!-- Cassette housing SVG -->
  <svg
    width={w}
    height={h}
    viewBox="0 0 56 40"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    aria-hidden="true"
  >
    <!-- Outer housing -->
    <rect x="1.25" y="1.25" width="53.5" height="34.5" rx="5" stroke={color} stroke-width="2.5" fill="rgba(247,180,92,0.05)" />

    <!-- Window cutout background -->
    <rect x="7" y="6" width="42" height="23" rx="3" fill="#060810" stroke={color} stroke-width="1.5" stroke-opacity="0.3" />

    <!-- Left reel -->
    <g transform="translate(18, 17.5)">
      <circle r="7" stroke={color} stroke-width="2" fill="none" />
      <circle r="2.5" fill={color} />
      <line x1="0" y1="-4.5" x2="0" y2="-7" stroke={color} stroke-width="1.5" />
      <line x1="3.9" y1="2.25" x2="6.06" y2="3.5" stroke={color} stroke-width="1.5" />
      <line x1="-3.9" y1="2.25" x2="-6.06" y2="3.5" stroke={color} stroke-width="1.5" />
      {#if spinning}
        <animateTransform
          attributeName="transform"
          type="rotate"
          from="0"
          to="360"
          dur="2.4s"
          repeatCount="indefinite"
        />
      {/if}
    </g>

    <!-- Tape strand -->
    <path d="M 25 17.5 Q 28 20 31 17.5" stroke={color} stroke-width="1.5" stroke-opacity="0.55" fill="none" />

    <!-- Right reel -->
    <g transform="translate(38, 17.5)">
      <circle r="7" stroke={color} stroke-width="2" fill="none" />
      <circle r="2.5" fill={color} />
      <line x1="0" y1="-4.5" x2="0" y2="-7" stroke={color} stroke-width="1.5" />
      <line x1="3.9" y1="2.25" x2="6.06" y2="3.5" stroke={color} stroke-width="1.5" />
      <line x1="-3.9" y1="2.25" x2="-6.06" y2="3.5" stroke={color} stroke-width="1.5" />
      {#if spinning}
        <animateTransform
          attributeName="transform"
          type="rotate"
          from="360"
          to="0"
          dur="2.8s"
          repeatCount="indefinite"
        />
      {/if}
    </g>

    <!-- Bottom notch tab -->
    <rect x="23" y="35.5" width="10" height="4" rx="2" fill={color} />
  </svg>

  {#if withWordmark}
    <div class="wordmark">
      <span class="wm-sub">Cassette</span>
      <span class="wm-name">Cassette</span>
    </div>
  {/if}
</div>

<style>
  .cassette-logo {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }

  .cassette-logo.with-wordmark {
    flex-direction: row;
    align-items: center;
    gap: 10px;
  }

  .wordmark {
    display: flex;
    flex-direction: column;
    gap: 1px;
    line-height: 1;
  }

  .wm-sub {
    font-size: 0.55rem;
    font-weight: 800;
    letter-spacing: 0.22em;
    text-transform: uppercase;
    color: var(--accent-bright);
  }

  .wm-name {
    font-size: 1.4rem;
    font-weight: 800;
    letter-spacing: -0.04em;
    background: linear-gradient(135deg, var(--text-primary) 40%, var(--accent));
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }
</style>
