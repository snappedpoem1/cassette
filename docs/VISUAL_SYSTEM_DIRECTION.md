# Visual System Direction

Last updated: 2026-04-08
Status: active direction

Supersession note:

- This document governs visual direction and anti-generic UI rules.
- It does not by itself define Cassette's shell architecture.
- For workspace behavior, surface model, and modular desktop priorities, read `docs/MODULAR_DESKTOP_DIRECTION_RESET.md` first.

## Direction

Cassette should look authored, warm, and lived-in. It should not look like a generic dark admin app with album art dropped on top.

The current repo has promising pieces:

- dynamic shell mood
- strong cover-art usage
- compact playback shell

But the current visual system still fails on the fundamentals:

- visible mojibake in CSS and UI copy
- low-contrast secondary text
- remote font dependency in a desktop app
- too many panels that read like utility cards
- primary surfaces that are more tidy than memorable

## Design Principles

1. Art leads.
2. Text stays readable for hours.
3. Chrome gets quieter during playback.
4. Surfaces feel arranged, not card-generated.
5. State chips communicate quality and provenance without taking over.
6. Motion is optional and never required for comprehension.

## Wave 0 Quality Floor

This is mandatory before larger rebuilds.

### Exact Files

- `ui/src/app.css`
- `ui/src/routes/+page.svelte`
- `ui/src/routes/collection/+page.svelte`
- `ui/src/routes/artists/+page.svelte`
- `ui/src/routes/playlists/+page.svelte`
- `ui/src/routes/session/+page.svelte`
- `ui/src/routes/downloads/+page.svelte`
- `ui/src/lib/components/Sidebar.svelte`
- `ui/src/lib/components/NowPlaying.svelte`
- `ui/src/lib/components/NowPlayingExpanded.svelte`
- `ui/src/lib/components/QueuePanel.svelte`
- `ui/src/lib/components/RightSidebar.svelte`
- `ui/src/lib/api/tauri.ts`

### What Changes

- remove mojibake from comments, labels, titles, placeholders, icons, and separators
- replace `@import` Google Fonts usage in `ui/src/app.css` with bundled local assets or a desktop-safe system stack
- raise the contrast floor for `--text-secondary`, `--text-muted`, and low-emphasis metadata
- improve focus outlines and hover clarity on primary interactive elements
- convert obvious clickable non-buttons on primary surfaces into semantic controls
- increase line-height and spacing where long-session reading is dense or cramped

### What Does Not Change

- core palette direction
- dynamic glass concept
- low-motion support

### Acceptance

- no mojibake remains in `ui/src`
- primary metadata is readable at a glance without leaning on hover state
- Now Playing, Home, Collection, Artists, Playlists, Queue, and Session are keyboard legible
- desktop shell does not rely on network font availability

## Core Visual Language

### Color

Keep the dark base, but tune it toward legibility and warmth.

- background stays dark and deep
- primary text must read clearly against the base
- secondary text must be readable, not decorative
- amber stays a supporting accent, not the dominant state color
- quality and provenance chips should use restrained color coding

### Typography

Typography should feel deliberate, not default.

- use one main sans family with a stable fallback stack
- reserve all-caps for labels and chips only
- large headers should feel album-sleeve confident, not dashboard-bold
- metadata should be quiet but still readable

### Spacing

Current surfaces jump between compact utility density and large hero spacing. Unify this.

- one tight rhythm for lists and rails
- one relaxed rhythm for immersive surfaces
- avoid unrelated one-off paddings in route-local CSS

### Art Treatment

Cover art is not garnish. It is a structural element.

- Home uses art as mood and return anchor
- Album and Artist use art as shelf material
- Now Playing uses art as the center of gravity
- Queue uses art sparingly so the list stays sculptable

### Motion

Motion stays calm.

- subtle shell transitions are fine
- playback-active emphasis is fine
- no decorative drift or nonfunctional shimmer
- reduced-motion must preserve parity

## File-Level Direction

### `ui/src/app.css`

What changes:

- own the base visual tokens here
- define stronger text colors, spacing tokens, motion tokens, and surface elevations
- remove corrupted comments and text

What does not change:

- overall dark-shell direction

### `ui/src/routes/+layout.svelte`

What changes:

- top bar and shell framing become quieter and more place-like
- reduce utility-console wording

What does not change:

- shell structure and player placement

### `ui/src/lib/components/Sidebar.svelte`

What changes:

- listening-first grouping
- clearer hierarchy between primary surfaces and Workstation
- stronger visual identity than plain nav list

What does not change:

- simple keyboardable nav behavior

### `ui/src/lib/components/NowPlaying.svelte` and `ui/src/lib/components/NowPlayingExpanded.svelte`

What changes:

- art weight increases
- chrome weight decreases
- provenance, quality, and focus mode become first-class visual states

What does not change:

- playback controls
- lyrics access

### `ui/src/lib/components/QueuePanel.svelte` and `ui/src/lib/components/RightSidebar.svelte`

What changes:

- legibility and density improve
- queue gains scene and sculpt affordances

What does not change:

- queue remains fast to scan

## Explicit Non-Goals

- no light theme in this cycle
- no neon skin pass
- no nostalgia cosplay UI
- no animation-heavy shell reskin
