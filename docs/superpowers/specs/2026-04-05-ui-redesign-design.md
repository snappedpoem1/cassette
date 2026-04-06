# Cassette UI Redesign — Design Spec
**Date:** 2026-04-05  
**Status:** Approved for implementation

---

## Overview

Full visual redesign of the Cassette desktop UI. Every surface gets bespoke treatment. The goal is to move from a sparse utility aesthetic to a rich, editorial music app feel — serious and tool-like, not flashy.

Reference: the Cassette // Lyra mockup's third panel (album detail with blurred atmospheric backdrop), minus all Lyra-specific AI components.

---

## Palette — Steel Dusk

| Token | Value | Usage |
|---|---|---|
| `--bg-deep` | `#060810` | Topbar, sidebar, now-playing bar |
| `--bg-base` | `#080b12` | Main content area base |
| `--bg-card` | `#0c1018` | Album cards, form inputs |
| `--bg-hover` | `#0f1420` | Hover states |
| `--bg-active` | `rgba(139,180,212,0.08)` | Active nav item fill |
| `--border` | `rgba(139,180,212,0.07)` | Card borders, dividers |
| `--border-dim` | `rgba(255,255,255,0.04)` | Structural dividers |
| `--primary` | `#8bb4d4` | Slate-blue — nav active, play button, seek fill start |
| `--primary-dim` | `rgba(139,180,212,0.5)` | Hover borders, muted primary |
| `--accent` | `#f7b45c` | Amber — seek fill end, secondary highlights only |
| `--text-1` | `#c4d4e4` | Primary text |
| `--text-2` | `#4a6070` | Secondary text |
| `--text-3` | `#253040` | Muted / disabled |
| `--status-ok` | `#5ec4a0` | Provider healthy indicator |

**Seek bar gradient:** `linear-gradient(90deg, var(--primary), var(--accent))` — the one place amber and blue meet visually.

---

## Typography

**Font:** Inter (Google Fonts), loaded via `@import` in `app.css`.  
**Fallback:** `system-ui, -apple-system, sans-serif`

Weights used: 400 (body), 500 (track titles), 600 (album titles, nav), 700 (section headers, brand), 800 (album detail hero title).

No size changes to existing scale — same rem values, just Inter instead of system-ui.

---

## Layout Chrome (shared across all views)

### Topbar
- Background: `var(--bg-deep)`
- Border-bottom: `1px solid var(--border)`
- Brand: `CASSETTE // DESKTOP` in `var(--primary)`, 9.5px, weight 800, letter-spacing 0.12em. The `//` at 35% opacity.
- Nav links: 8px, `var(--text-3)`, active link `var(--text-2)`
- Right: "Mini Player" and "Commands" as ghost buttons (`border: 1px solid var(--border)`)

### Left Sidebar
- Width: 88px
- Background: `var(--bg-deep)`
- Border-right: `1px solid var(--border-dim)`
- Logo block at top: "TAPE" label in `var(--primary)` 7px 800 weight, "Cassette" wordmark below in `var(--text-1)` 9px
- Nav items: icon abbreviation (LIB, DL, PL, AR, IM, TL, CFG) + label, 8px
- Active item: `var(--primary)` color, `var(--bg-active)` fill, `2px solid var(--primary)` right border
- Divider between core and utility links
- Footer: track count `{n} tracks` in `var(--text-1)` bold + `var(--text-2)` label. Scan progress widget when scanning.

### Now Playing Bar
- Background: `var(--bg-deep)`
- Border-top: `1px solid var(--border-dim)`
- Three-column grid: `1fr auto 1fr`
- **Left:** 38×38px art thumbnail (border-radius 5px, box-shadow) + title/artist stacked
- **Center:** Prev/Play/Next controls + seek bar below. Play button: 28×28px circle, `var(--primary)` fill, `var(--bg-deep)` icon, subtle blue glow shadow. Seek bar 3px tall, gradient fill.
- **Right:** Volume icon + 70px volume bar, right-aligned

### Right Sidebar
- Width: 190px
- Background: `var(--bg-deep)`
- Border-left: `1px solid var(--border-dim)`
- Two tabs: Queue / Info. Active tab: `var(--primary)` color + bottom border.

---

## View 1 — Library (Album Grid)

**Main area layout:**
- Header row: "Library" h3 + search input (right-aligned, 160px wide, icon inside)
- Tab row: Albums / Tracks / Artists. Active tab: `var(--primary)` bottom border.
- Album grid: `grid-template-columns: repeat(auto-fill, minmax(100px, 1fr))`, gap 8px, padding 12px, scrollable

**Album card:**
- Border-radius: 7px
- Border: `1px solid var(--border)`
- Background: `var(--bg-card)` — **overridden per card** with a tinted background derived from `dominant_color_hex` (see Color Extraction below)
- Hover: `border-color: var(--primary-dim)`, `translateY(-1px)`
- Art: square aspect-ratio, `object-fit: cover`
- Metadata strip: title (8px, weight 600, color tinted from dominant color), artist (7px, `var(--text-3)`), year (6.5px, very muted)

**Color tinting on metadata strip:**  
Each card reads `album.dominant_color_hex` from the store. A small utility function `tintFromHex(hex)` desaturates and darkens the color for the background (e.g. `oklch(12% 0.03 <hue>)`) and lightens/shifts it for the title text (e.g. `oklch(72% 0.08 <hue>)`). Falls back to `var(--bg-card)` / `var(--text-1)` if no color stored.

**Queue tab (right sidebar):**
- Section label "Up next" in `var(--text-3)` uppercase 6.5px
- Track rows: number / title+artist / duration. Current track shows `▶` in `var(--primary)` instead of number, row gets `var(--bg-active)` fill.
- Clear button in header when queue non-empty.

---

## View 2 — Album Detail

Triggered when user clicks an album card. Replaces the album grid within the main area.

**Backdrop:**
- The album's cover art is placed as an absolutely-positioned `<div>` with `background-image`, `background-size: cover`, `filter: brightness(0.18) saturate(1.3) blur(3px)` filling the entire main panel.
- Gradient overlay on top: `linear-gradient(180deg, rgba(6,8,16,0.5) 0%, rgba(6,8,16,0.92) 55%, rgba(6,8,16,1) 100%)`

**Hero section:**
- 84×84px art (border-radius 8px, heavy drop shadow) + info block side-by-side, bottom-aligned
- Back link above title: `← Albums` in `var(--text-3)` 7px
- Title: 16px, weight 800, `#deeaf8` (slightly cooler white)
- Artist: 9px, `rgba(200,220,240,0.55)`
- Meta line: format + bit depth + sample rate, very muted
- Play button: pill shape, `var(--primary)` fill, `var(--bg-deep)` text

**Tracklist:**
- Rows: track number / title / format badge / duration
- Currently playing row: `var(--bg-active)` fill, `rgba(139,180,212,0.15)` border, title in `var(--text-1)` weight 600, number replaced with `▶` in `var(--primary)`
- Hover: subtle `var(--border)` border + `rgba(139,180,212,0.04)` fill

**Info tab (right sidebar) when album is open:**
- Artist section: name (9px bold), genre tags as small pills (`var(--primary)` tint), listener count, bio paragraph
- Lyrics section: monospace-ish text, `line-height: 2`, scrollable, source attribution
- Both sections populated from existing `nowPlayingContext` store — no new backend work

---

## View 3 — Settings

**Layout:** Left sub-nav (130px) + scrollable content area. No right sidebar on Settings.

**Sub-nav sections:** Library · Providers · Enrichment · Tools · Last.fm  
Active item: `var(--primary)` color, `rgba(139,180,212,0.06)` fill, `2px solid var(--primary)` right border.

**Provider Status section:**
- 3-column grid of provider cards
- Each card: provider name (8px weight 600) + status line
- Configured: green dot (`var(--status-ok)`) + glow, "Configured" label
- Not configured: muted grey dot, "Not configured"
- Cards have subtle border: `rgba(139,180,212,0.2)` when configured, `rgba(255,255,255,0.03)` otherwise

**Form fields:**
- Label: 7px, `var(--text-2)`, weight 600, letter-spacing 0.04em
- Input: `var(--bg-card)` background, `var(--border)` border, border-radius 5px, 8px text, `var(--text-1)` value, `var(--text-3)` placeholder
- Focus: `border-color: var(--primary-dim)`, no outline
- 2-column field grid within each section. Full-width fields use `grid-column: 1 / -1`.
- Password fields: `type="password"` (browser handles masking)

**Library Roots section:**
- Each root in a row: monospace path + "Remove" danger button (red-tinted ghost)
- "+ Add Folder" + "Scan Library" action buttons as ghost-primary buttons

**Save row:** Primary "Save Settings" button + ghost "Persist Effective Config" button

---

## Color Extraction — Backend Change

**Where:** Librarian scanner, during track/album import.

**What:** After reading file tags and cover art path, extract the dominant color from the cover image and store it as a hex string (e.g. `"#3d2810"`).

**How:** Use the `image` crate (already available or add as dep) to decode the cover art thumbnail, sample pixels (e.g. 8×8 resize to average), and pick the most saturated/dominant hue via a simple median-cut or average. Store in the `albums` table as `dominant_color_hex TEXT`.

**API surface:**
- `Album` struct gains `dominant_color_hex: Option<String>`
- `get_albums` command returns this field
- `Album` TypeScript interface in `tauri.ts` gains `dominant_color_hex: string | null`

**Frontend utility — `tintFromHex(hex: string | null)`** in `utils.ts`:
- Returns `{ bg: string, titleColor: string }` CSS color strings
- Darkens + desaturates hex for card background
- Lightens + partially desaturates for title text
- Returns fallback `{ bg: 'var(--bg-card)', titleColor: 'var(--text-1)' }` if null

---

## CSS Architecture

All token definitions move to a single `:root` block in `app.css`. Component-level `<style>` blocks reference tokens only — no hardcoded color values in components.

Inter font import added to top of `app.css`:
```css
@import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&display=swap');
```
Or self-hosted via `$lib/fonts/` if offline use is needed (decide at implementation time — prefer self-hosted given local-first nature of the app).

---

## What Does NOT Change

- Route structure (`+page.svelte` files, layout hierarchy)
- Store logic, API surface (except `dominant_color_hex` addition)
- Tauri command surface (except `Album` struct field)
- Component file structure
- Accessibility suppression comments (existing `svelte-ignore` annotations stay)

---

## Scope Boundaries

This spec covers visual redesign only. The following are explicitly out of scope:

- Any new pipeline, acquisition, or playback features
- Downloads page redesign (complex enough for its own spec — same palette/chrome applies but layout is unchanged for now)
- Artists page redesign (same)
- Playlists page redesign (same)
- Import page redesign (same)
- Tools page redesign (same)

Downloads, Artists, Playlists, Import, and Tools pages will inherit the new chrome (topbar, sidebar, now-playing bar, CSS tokens, Inter font) automatically but their internal layouts are not redesigned in this pass.
