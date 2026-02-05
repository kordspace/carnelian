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
