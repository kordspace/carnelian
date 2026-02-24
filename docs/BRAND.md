# 🔥 Carnelian OS — Dual Theme Brand Kit

> 💎 *Carnelian is a gemstone — warm, fiery, and grounding. The name reflects both the system's Rust foundation and its role as a precious, reliable core.*

## Theme Concept

- **Light mode ("Forge"):** ember + gold + bone → warm, optimistic, premium
- **Dark mode ("Night Lab"):** obsidian + deep greens + purples → technical, calm, high-contrast

---

## Color Palette

### Core Neutrals (shared)

| Name | Hex | Usage |
|------|-----|-------|
| Obsidian | `#0B0E14` | Background / dark anchor |
| Slate | `#2A2F3A` | Surfaces |
| Bone | `#F2EFEA` | Light background / text |
| Ink | `#10131A` | Light text |

### Light Mode: Forge (ember + gold)

| Name | Hex | Usage |
|------|-----|-------|
| Carnelian Ember | `#D24B2A` | Primary |
| Molten Orange | `#E0613E` | Hover / alt |
| Signal Gold | `#F2C14E` | Accent |
| Soft Gold | `#FFE7B0` | Tint / background |

### Dark Mode: Night Lab (greens + purples)

**Greens** (systems + "safe/healthy state")

| Name | Hex | Usage |
|------|-----|-------|
| Deep Emerald | `#0F6B4C` | Primary accent |
| Jade Glow | `#39D98A` | Interactive / links |
| Moss | `#163C2D` | Muted surfaces |

**Purples** (agent/insight + "mystic compute")

| Name | Hex | Usage |
|------|-----|-------|
| Amethyst | `#7C4DFF` | Secondary accent |
| Violet Mist | `#B39DFF` | Hover / tint |
| Plum Shadow | `#2A1748` | Depth |

### Usage Note

In dark mode, avoid ember/gold as the main accent (it can feel harsh on black). Use jade + amethyst for UI emphasis, and reserve ember only for "🔥 Carnelian OS" identity marks or warnings.

---

## Role-Based Color Tokens

### Brand Tokens

| Token | Light | Dark |
|-------|-------|------|
| `brand.primary` | `#D24B2A` (Carnelian Ember) | `#39D98A` (Jade Glow) |
| `brand.secondary` | `#F2C14E` (Signal Gold) | `#7C4DFF` (Amethyst) |

### Functional Tokens

| Token | Light | Dark |
|-------|-------|------|
| `state.success` | `#0F6B4C` | `#39D98A` |
| `state.warning` | `#F2C14E` | `#D24B2A` (use sparingly) |
| `state.danger` | `#C0392B` | `#FF5C5C` |
| `state.info` | `#2D6CDF` | `#B39DFF` |

### Surface Tokens

| Token | Light | Dark |
|-------|-------|------|
| `bg` | `#F2EFEA` | `#0B0E14` |
| `surface.1` | `#FFFFFF` | `#141824` |
| `surface.2` | `#F7F3EC` | `#1B2030` |
| `border` | `#E6DDCF` | `#2A2F3A` |

---

## Brand Behavior Rules

These rules keep the brand cohesive across all touchpoints:

1. **🔥 is always "Carnelian OS"** — titles, repo badges, product identity
2. **🦎 Lian gets purple-forward accents** — tips, "agent voice", insight callouts
3. **💎 Gemstone references** — premium features, core values, architectural foundations
4. **Greens are "system health / correctness / success"** — checks, "stable", "verified", "ready"
5. **Gold is "highlight / emphasis" in light mode only** — in dark mode use purple for emphasis

---

## Docs Styling Conventions

### Headings

```markdown
# 🔥 Carnelian OS

## 🔥 Runtime Guarantees

## 🦎 Lian Patterns

## ✅ Health & Diagnostics
```

### Callouts (semantic + themed)

| Icon | Type | Usage |
|------|------|-------|
| 🔥 | Carnelian OS Note | Identity / architecture / invariants |
| 🦎 | Lian Tip | Practical usage guidance, patterns |
| 💎 | Gemstone | Premium features, core values, foundations |
| ✅ | Healthy State | Green, success |
| 🟣 | Insight | Purple, design rationale / tradeoffs |

### Example Callouts

> 🟣 **Insight:** Deterministic compression makes debugging and diffs sane.

> ✅ **Healthy State:** Gateway returns structured tool logs with stable schemas.

> 🦎 **Lian Tip:** Keep tool outputs typed; shaping becomes trivial.

> 🔥 **Carnelian OS Note:** All capability grants require explicit approval from the core identity.

> 💎 **Gemstone:** The ledger's hash-chain provides tamper-resistant auditability — a core architectural guarantee.

---

## Taglines

**Light mode (Forge):**
> "🔥 Built in the forge. Shipped with confidence."

**Dark mode (Night Lab):**
> "🟣 Trace the system. ✅ Keep it healthy."

---

## CSS Variables (Reference)

```css
/* Light Mode (Forge) */
:root {
  --brand-primary: #D24B2A;
  --brand-secondary: #F2C14E;
  --bg: #F2EFEA;
  --surface-1: #FFFFFF;
  --surface-2: #F7F3EC;
  --border: #E6DDCF;
  --text: #10131A;
  --state-success: #0F6B4C;
  --state-warning: #F2C14E;
  --state-danger: #C0392B;
  --state-info: #2D6CDF;
}

/* Dark Mode (Night Lab) */
[data-theme="dark"] {
  --brand-primary: #39D98A;
  --brand-secondary: #7C4DFF;
  --bg: #0B0E14;
  --surface-1: #141824;
  --surface-2: #1B2030;
  --border: #2A2F3A;
  --text: #F2EFEA;
  --state-success: #39D98A;
  --state-warning: #D24B2A;
  --state-danger: #FF5C5C;
  --state-info: #B39DFF;
}
```

---

## Logo Usage

- **Primary:** 🔥 emoji + "Carnelian OS" text
- **Agent:** 🦎 emoji + "Lian" text
- **Gemstone:** 💎 emoji for premium/core features
- **Combined:** "🔥 Carnelian OS • 🦎 Lian"

### Badge Examples

```markdown
[![🔥 Carnelian OS](https://img.shields.io/badge/🔥-Carnelian%20OS-D24B2A)](https://github.com/kordspace/carnelian)
[![🦎 Lian](https://img.shields.io/badge/🦎-Lian-7C4DFF)](https://github.com/kordspace/carnelian)
[![💎 Core](https://img.shields.io/badge/💎-Core-0F6B4C)](https://github.com/kordspace/carnelian)
```

---

## Gemstone Symbolism

💎 **Why Carnelian?**

Carnelian is a semi-precious gemstone known for:
- **Warmth** — Its fiery orange-red color reflects the Rust language and the "forge" theme
- **Grounding** — Associated with stability and courage, fitting for a reliable system core
- **Clarity** — Historically used for seals and signatures, aligning with our capability-based security
- **Energy** — Said to boost creativity and motivation, perfect for an AI agent platform

The gemstone metaphor extends throughout the brand:
- 🔥 **Fire** polishes the gem (the forge refines the system)
- 💎 **Gem** is the precious result (reliable, valuable, enduring)
- 🦎 **Lian** is the spirit within (the agent that brings it to life)

---

## Asset Inventory

All brand assets live under the `assets/` directory at the repository root.

### Logos (`assets/logos/`)

| File | Format | Dimensions | Description |
|------|--------|------------|-------------|
| `carnelian-icon.svg` | SVG | 64×64 | Standalone faceted gemstone mark — no text |
| `carnelian-logo-full.svg` | SVG | 280×64 | Icon + "Carnelian OS" wordmark + "🦎 Lian" sub-label (light variant, Ink text) |
| `carnelian-logo-full-dark.svg` | SVG | 280×64 | Icon + wordmark + sub-label (dark variant, Bone text, Jade Glow sub-label) |
| `carnelian-wordmark.svg` | SVG | 240×48 | Text-only: "🔥 Carnelian OS" in Carnelian Ember |

### PNG Exports (`assets/logos/exports/`)

Generated from `carnelian-icon.svg`. Regenerate after any logo revision using the commands below.

| File | Dimensions | Use |
|------|------------|-----|
| `carnelian-icon-512.png` | 512×512 | App store / social avatar |
| `carnelian-icon-256.png` | 256×256 | Desktop app icon |
| `carnelian-icon-128.png` | 128×128 | Taskbar / dock |
| `carnelian-icon-64.png` | 64×64 | Favicon / small badge |

**Rasterisation commands** (requires `rsvg-convert` from librsvg, or Inkscape):

```bash
# Using rsvg-convert (recommended)
rsvg-convert -w 512 -h 512 assets/logos/carnelian-icon.svg -o assets/logos/exports/carnelian-icon-512.png
rsvg-convert -w 256 -h 256 assets/logos/carnelian-icon.svg -o assets/logos/exports/carnelian-icon-256.png
rsvg-convert -w 128 -h 128 assets/logos/carnelian-icon.svg -o assets/logos/exports/carnelian-icon-128.png
rsvg-convert -w 64  -h 64  assets/logos/carnelian-icon.svg -o assets/logos/exports/carnelian-icon-64.png

# Using Inkscape (alternative)
inkscape assets/logos/carnelian-icon.svg --export-type=png --export-width=512 --export-filename=assets/logos/exports/carnelian-icon-512.png
inkscape assets/logos/carnelian-icon.svg --export-type=png --export-width=256 --export-filename=assets/logos/exports/carnelian-icon-256.png
inkscape assets/logos/carnelian-icon.svg --export-type=png --export-width=128 --export-filename=assets/logos/exports/carnelian-icon-128.png
inkscape assets/logos/carnelian-icon.svg --export-type=png --export-width=64  --export-filename=assets/logos/exports/carnelian-icon-64.png
```

### Branding Specs (`assets/branding/`)

| File | Format | Description |
|------|--------|-------------|
| `color-palette.json` | JSON | Machine-readable colour tokens: core, forge, nightLab, and themed token sets |
| `typography.md` | Markdown | Type scale, font stacks, weight reference, colour pairing per theme |
| `brand-tokens.css` | CSS | Canonical CSS custom properties for colours and typography |

### Social Media Templates (`assets/social/`)

| File | Format | Dimensions | Description |
|------|--------|------------|-------------|
| `twitter-card-1200x630.svg` | SVG | 1200×630 | Twitter/X card: gem icon, title, tagline, URL |
| `opengraph-1200x630.svg` | SVG | 1200×630 | OpenGraph card: same composition as Twitter card |
| `github-preview-1280x640.svg` | SVG | 1280×640 | GitHub social preview: card + feature badge pills |

### Screenshots (`assets/screenshots/`)

| File | Format | Resolution | Description |
|------|--------|------------|-------------|
| `dashboard.png` | PNG | ≥1280×800 | Main dashboard: metric cards, system health, XP widget |
| `xp-progression.png` | PNG | ≥1280×800 | XP Progression tab: level history chart, leaderboard, skill levels |
| `task-execution.png` | PNG | ≥1280×800 | Active task: scheduler queue, run logs, worker status |
| `skills-metrics.png` | PNG | ≥1280×800 | Skills metrics: top skills table, usage breakdown |

> All screenshots should use the **Night Lab (dark) theme** for visual consistency in documentation.

---

## Logo Usage Guidelines

### Do

- Use the **full logo** (`carnelian-logo-full.svg`) on white/bone or dark/obsidian backgrounds.
- Maintain **minimum clear space** equal to the gem icon height on all sides.
- Use the **dark variant** (`carnelian-logo-full-dark.svg`) on dark backgrounds.
- Use the **wordmark** (`carnelian-wordmark.svg`) for README badges and documentation headers where the icon is not needed.

### Don't

- **Recolour** the gem facets — the facet colours encode depth and are part of the identity.
- **Stretch or distort** the SVG — always scale proportionally.
- Place the **light logo on a coloured background** without sufficient contrast (minimum 4.5:1 ratio).
- Use the 🔥 emoji as a **substitute for the SVG logo** in formal contexts (marketing, app stores, print).

### Minimum Sizes

| Variant | Minimum Width |
|---------|---------------|
| Full logo (icon + wordmark) | 120px wide |
| Standalone icon | 24×24px |

### File Selection Guide

| Context | Recommended File |
|---------|-----------------|
| README header / badge | `carnelian-wordmark.svg` or shield.io badge |
| App icon (desktop) | `carnelian-icon-256.png` |
| Favicon | `carnelian-icon-64.png` |
| Social card (Twitter/OG) | `twitter-card-1200x630.svg` or `opengraph-1200x630.svg` |
| GitHub repo social preview | `github-preview-1280x640.svg` (rasterise to PNG for upload) |
| Documentation (light bg) | `carnelian-logo-full.svg` |
| Documentation (dark bg) | `carnelian-logo-full-dark.svg` |
| Print / high-res | `carnelian-icon-512.png` or SVG source |
