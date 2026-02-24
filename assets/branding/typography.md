# 🔥 Carnelian OS — Typography Specification

> Cross-references `crates/carnelian-ui/src/theme.rs` (lines 44–85) for the existing font stack.

---

## Type Scale

| Role | Family | Weight | Size | Line Height | Letter Spacing | Usage |
|------|--------|--------|------|-------------|----------------|-------|
| Display | Inter / system-ui | 700 | 28–36px | 1.2 | -0.02em | Logo wordmark, page titles |
| Heading | Inter / system-ui | 600 | 18–24px | 1.3 | -0.01em | Section headers, card titles |
| Body | Inter / system-ui | 400 | 14px | 1.5 | 0 | General UI text |
| Mono | JetBrains Mono / monospace | 400 | 13px | 1.5 | 0 | Code, log output, IDs |
| Label | Inter / system-ui | 500 | 12px | 1.4 | 0.02em | Badges, tags, metadata |

---

## Font Stack

```css
--font-display: 'Inter', system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif;
--font-body:    'Inter', system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif;
--font-mono:    'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace;
```

---

## Colour Pairing by Theme

### Forge (Light Mode)

| Role | Text Colour | Background |
|------|-------------|------------|
| Display | Ink `#10131A` | Bone `#F2EFEA` |
| Heading | Ink `#10131A` | White `#FFFFFF` |
| Body | Ink `#10131A` | Bone `#F2EFEA` / White `#FFFFFF` |
| Mono | Carnelian Ember `#D24B2A` | Warm White `#F7F3EC` |
| Label | Slate `#2A2F3A` | Soft Gold `#FFE7B0` or Sand border `#E6DDCF` |
| Link | Carnelian Ember `#D24B2A` | — |
| Link (hover) | Molten Orange `#E0613E` | — |

### Night Lab (Dark Mode)

| Role | Text Colour | Background |
|------|-------------|------------|
| Display | Bone `#F2EFEA` | Obsidian `#0B0E14` |
| Heading | Bone `#F2EFEA` | Dark Surface `#141824` |
| Body | Bone `#F2EFEA` | Obsidian `#0B0E14` / Dark Surface `#141824` |
| Mono | Jade Glow `#39D98A` | Deep Surface `#1B2030` |
| Label | Violet Mist `#B39DFF` | Slate `#2A2F3A` |
| Link | Jade Glow `#39D98A` | — |
| Link (hover) | Amethyst `#7C4DFF` | — |

---

## Weight Reference

| CSS Weight | Name | Usage |
|------------|------|-------|
| 400 | Regular | Body text, mono code |
| 500 | Medium | Labels, metadata |
| 600 | SemiBold | Section headings |
| 700 | Bold | Display titles, wordmark |

---

## Sizing Scale (px)

| Token | Value | Usage |
|-------|-------|-------|
| `--font-size-xs` | 11px | Fine print, timestamps |
| `--font-size-sm` | 12px | Labels, badges |
| `--font-size-base` | 14px | Body text |
| `--font-size-md` | 16px | Emphasized body |
| `--font-size-lg` | 18px | Sub-headings |
| `--font-size-xl` | 24px | Section headings |
| `--font-size-2xl` | 28px | Page titles |
| `--font-size-3xl` | 36px | Display / hero |

---

## Guidelines

1. **Never mix font families** within a single text block — use Inter for prose, JetBrains Mono for code.
2. **Prefer SemiBold (600) over Bold (700)** for UI headings to avoid visual heaviness on dark backgrounds.
3. **Use negative letter-spacing** only for Display and Heading roles; body text should remain at `0`.
4. **Minimum body size is 14px** — never go below 12px for any readable text.
5. **Line height for code blocks** should be `1.5` to maintain readability in log output and terminal views.
