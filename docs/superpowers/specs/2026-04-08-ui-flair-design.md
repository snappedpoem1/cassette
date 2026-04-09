# Cassette UI — Visual Identity & Flair Design

**Date:** 2026-04-08  
**Status:** Approved  
**Approach:** CSS variable expansion (layered system), no new JS complexity

---

## Vision

Cinematic depth sets the stage. Analog texture makes it feel worn and loved. Club energy makes it feel alive while music plays. Together: a room that looks premium when still and crackles when something's spinning.

The UI morphs with the music — not as a gimmick, but as the natural expression of what's playing. When a track changes, the whole room shifts.

---

## 1. Logo & Identity

**Chosen: Option A — Cassette housing with spinning reels**

An inline SVG icon depicting a cassette tape housing:
- Rounded rectangular outer shell, amber (`#f7b45c`) stroke
- Two reels inside a window cutout — each a circle with a center dot, animated to spin when playback is active (`animation: spin` paused when not playing)
- A thin strand connecting the two reels
- A bottom notch tab on the housing

**Wordmark treatment:**
- Supertitle: `CASSETTE` in small-caps uppercase, 0.55rem, letter-spacing 0.22em, amber color — sits above
- Main name: `Cassette` in 1.4rem, weight 800, letter-spacing -0.04em, gradient fill from `--text-primary` → amber

**Placement:**
- Full logo (icon + wordmark) in the sidebar logo area
- Icon-only (housing without wordmark) used as the app favicon/window icon
- Reels animate (CSS `@keyframes spin`) tied to `$isPlaying` store — spinning when playing, paused when not

---

## 2. Living Backdrop System

**Goal:** The app background feels like the music is ambient-lighting the room.

### Implementation

Three blob layers as `::before` pseudo-elements on `.app-shell` (or a dedicated `.backdrop` child div for Svelte scoping):

```css
.backdrop-blob {
  position: fixed; inset: 0; z-index: 0;
  pointer-events: none; overflow: hidden;
}
.blob { position: absolute; border-radius: 50%; filter: blur(90px); opacity: 0.14; }
.blob-a { /* top-left, mood accent color, 22s drift */ }
.blob-b { /* bottom-right, amber offset, 17s drift */ }
.blob-c { /* center, blue offset, 26s drift */ }
```

Blob colors are set as inline CSS variables from the layout's mood sampler:
- Blob A: `rgba(var(--mood-accent-rgb), 0.18)` — primary track color
- Blob B: `rgba(247, 180, 92, 0.12)` — fixed amber warmth anchor
- Blob C: `rgba(139, 180, 212, 0.08)` — fixed steel blue anchor

The `--mood-accent-rgb` variable already exists and is computed per-track in `+layout.svelte`. No new sampling logic needed — just wider application.

**Motion:** Each blob drifts on a slow `@keyframes drift` (translate + scale, 17–26s cycles, alternate). When `low_motion` is true: `animation: none`, blobs static at fixed positions.

**Layering:** All page content sits above blobs via `z-index`. Glass panels use `backdrop-filter: blur(16–24px)` to diffuse blob colors up through surfaces.

---

## 3. Analog Grain Layer

A persistent noise texture applied as a `body::after` pseudo-element:

```css
body::after {
  content: '';
  position: fixed; inset: 0; z-index: 9999;
  pointer-events: none;
  opacity: 0.035;
  background-image: url("data:image/svg+xml,...feTurbulence baseFrequency='0.9'...");
  background-size: 180px 180px;
}
```

- Inline SVG `feTurbulence` filter — no external image, no network request
- Opacity `0.035` — present but never distracting, like tape hiss at a low level
- `pointer-events: none` — completely non-interactive
- When `low_motion` true: opacity reduced to `0.02` (grain stays, just quieter)
- Does not animate — static texture, always present

---

## 4. Player Bar Redesign

The player bar is the heartbeat of the UI. Every element reacts to the current track.

### Mood strip
A 3px decorative line at the very top of the player bar area (above the `<NowPlaying>` component):
```css
background: linear-gradient(90deg,
  rgba(var(--mood-accent-rgb), 0.9),
  rgba(247,180,92,0.6),
  rgba(139,180,212,0.4)
);
box-shadow: 0 0 10px rgba(var(--mood-accent-rgb), 0.5);
```
Transitions on `--mood-shift-ms` (already a CSS variable).

### Seek bar
- Track height: 4px (up from current thin line)
- Fill: `linear-gradient(90deg, rgba(var(--mood-accent-rgb), 0.9), rgba(247,180,92,0.6))`
- Fill glow: `box-shadow: 0 0 8px rgba(var(--mood-accent-rgb), 0.5)`
- Thumb: white circle with mood-color glow, appears on hover
- All colors transition on `--mood-shift-ms`

### Play button
- Size: 40px (up from 36px)
- Background: `linear-gradient(135deg, rgba(var(--mood-accent-rgb), 1), #8bb4d4)` — mood-tinted gradient
- Breathing ring animation: `box-shadow` pulse at 3s cycle, `rgba(var(--mood-accent-rgb), 0.4)` expanding to transparent
- When `low_motion`: animation paused, static glow only

### Album art thumbnail
- Border: `1px solid rgba(var(--mood-accent-rgb), 0.25)`
- Box shadow: `0 4px 20px rgba(var(--mood-accent-rgb), 0.2)`
- Subtle `inset 0 1px 0 rgba(255,255,255,0.1)` highlight

### Quality/format chips
- Active/lossless chip: border and color derived from `--mood-accent-rgb` instead of hardcoded `--primary`

### Visualizer bars (spectrum mode)
- Bar fill: `linear-gradient(to top, rgba(var(--mood-accent-rgb), 0.9), rgba(247,180,92,0.5))`
- Bar glow: `box-shadow: 0 0 4px rgba(var(--mood-accent-rgb), 0.35)`
- Bars animate independently with staggered `animation-delay`

---

## 5. Sidebar Redesign

### Logo area
Replace the current text-only logo with the cassette icon + wordmark (see §1).

### Active nav item
- Left border accent (on collapsed) / right border accent (on expanded): `rgba(var(--mood-accent-rgb), 0.9)` — shifts with track
- Background: `linear-gradient(90deg, rgba(var(--mood-accent-rgb), 0.1), transparent)`
- Text color: derived from mood accent (lightened for contrast)

### Hover state
- Subtle glow: `background: rgba(var(--mood-accent-rgb), 0.05)`

---

## 6. Home Hero Redesign

### Backdrop
The hero section's existing radial gradients are extended to use `--mood-layer-a` and `--mood-layer-b` (already computed per-track). The hero becomes a window into the current mood.

### Cover art backdrop
The existing blurred cover art backdrop (`opacity: 0.18`) stays — increase opacity slightly to `0.22` and add `mix-blend-mode: luminosity` for a more ethereal bleed.

### Typography
- H1: gradient fill from `--text-primary` → `rgba(var(--mood-accent-rgb), 0.9)` — track-tinted headline
- Kicker label: `--accent-bright` stays amber (brand anchor)

### Primary CTA button
- Background: `linear-gradient(135deg, rgba(var(--mood-accent-rgb), 1), #8bb4d4)`
- Glow: `box-shadow: 0 4px 14px rgba(var(--mood-accent-rgb), 0.3)`

### Metric cards
- Glass effect: `backdrop-filter: blur(10px)`, `background: rgba(6,8,16,0.55)`
- Border: `rgba(var(--mood-accent-rgb), 0.12)`

---

## 7. Global Card & Surface Refinements

Applied across all `--bg-card` surfaces:

### Card hover states
All interactive cards get:
```css
transition: border-color 0.2s, box-shadow 0.2s, transform 0.15s;
&:hover {
  border-color: rgba(var(--mood-accent-rgb), 0.28);
  box-shadow: 0 8px 28px rgba(var(--mood-accent-rgb), 0.1);
  transform: translateY(-1px);
}
```

### Glass panels
Panels that sit over the backdrop (hero metrics, right sidebar, status strip):
- `backdrop-filter: blur(16px)`
- `background: rgba(6,8,16,0.6)` (slightly more transparent to let blobs bleed through)
- Border: `rgba(255,255,255,0.07)` base + `rgba(var(--mood-accent-rgb), 0.08)` mood tint

### Tone indicator cards (summary-card)
The `tone-steady` / `tone-watch` / `tone-action` cards get left-border accents instead of full border-color — more refined, less noisy.

---

## 8. Typography Refinements

- Kicker labels (`section-kicker`, `hero-kicker`): keep amber — brand anchor, never morphs
- H1/H2 on hero surfaces: gradient fill using mood accent
- Artist names in artist-line rows: weight 600 → 700, slight tracking tweak
- Body copy: no changes — `--text-secondary` stays neutral

---

## 9. CSS Variable Additions

New variables added to `:root` / extended from existing mood system:

```css
/* Already exists — widen application */
--mood-accent-rgb: 139, 180, 212;   /* per-track, computed in layout.svelte */
--mood-layer-a: ...;                 /* per-track */
--mood-layer-b: ...;                 /* per-track */
--mood-shift-ms: 460ms;              /* per-track transition speed */

/* New */
--mood-glow: rgba(var(--mood-accent-rgb), 0.22);
--mood-border: rgba(var(--mood-accent-rgb), 0.18);
--mood-fill-a: rgba(var(--mood-accent-rgb), 0.9);
--mood-fill-b: rgba(var(--mood-accent-rgb), 0.12);
```

All new variables transition automatically because they reference `--mood-accent-rgb` which is updated via inline style with `transition` already set.

---

## 10. Low Motion Respect

Every new animation checks `low_motion`:
- Blob drift: `animation: none` when low_motion
- Play button ring: `animation: none` when low_motion  
- Grain: opacity `0.035` → `0.02`
- Seek bar glow: present but no shimmer animation
- Track change transitions: `--mood-shift-ms` already collapses to `120ms` in low_motion mode

The existing `ui_visualizer_low_motion` setting drives all of this — no new setting needed.

---

## 11. Cassette Logo SVG (reusable component)

A `CassetteLogo.svelte` component:

**Props:**
- `size: number` (default 40) — scales the whole icon
- `spinning: boolean` — tied to `$isPlaying`, animates reels
- `withWordmark: boolean` — shows/hides the text wordmark below icon
- `color: string` (default `var(--accent)`) — stroke/fill color

**Usage:**
- Sidebar logo area: `<CassetteLogo size={28} spinning={$isPlaying} withWordmark />`
- Future favicon/window icon: SVG export of icon-only variant

---

## Implementation Order

1. **`app.css`** — new CSS variables, grain `body::after`, blob keyframes
2. **`+layout.svelte`** — add backdrop blob div, wire blob colors to mood vars
3. **`CassetteLogo.svelte`** — new component, spinning reels
4. **`Sidebar.svelte`** — swap logo area, wire active state to mood accent
5. **`NowPlaying.svelte`** — mood strip, seek bar, play button, art border, chip colors, visualizer bars
6. **`+page.svelte` (home)** — hero gradient, button, metric card glass, typography gradient
7. **Global card hover pass** — all route pages get updated card/surface hover styles

---

## 12. System Status Strip — Glass + Mood

The `SystemStatusStrip` sits directly above the player bar and is seen constantly. It gets:

- `backdrop-filter: blur(12px)` + `background: rgba(6,8,16,0.55)` — glass instead of opaque
- `border-top: 1px solid rgba(var(--mood-accent-rgb), 0.1)` — whisper-thin mood border
- `.is-busy` pills: left border `2px solid rgba(var(--mood-accent-rgb), 0.6)` + background `rgba(var(--mood-accent-rgb), 0.07)`
- `.is-down` pills: keep existing error red, but sharper
- All transitions on `--mood-shift-ms`

---

## 13. Cover Art Cross-Fade on Track Change

When `$currentTrack` changes, the album art thumbnail in `NowPlaying` cross-fades from old to new:

- Two `<img>` slots, absolutely positioned in the art wrapper
- When track changes: new image fades in (`opacity 0 → 1`) over `300ms` while old fades out
- Implemented as a Svelte `{#key track?.cover_art_path}` keyed block with a CSS `@keyframes fade-in` entry animation
- No JS timers needed — Svelte's keyed block handles the swap, CSS handles the transition
- When `low_motion`: instant swap, no fade

---

## Out of Scope

- Beat-sync animations (requires audio analysis pipeline not yet in place)
- Milkdrop preset picker UI (separate feature)
- Per-page unique layouts (this is a global pass)
- Mobile/responsive changes beyond what the existing breakpoints handle
