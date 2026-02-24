# 🔥 Carnelian OS Color Palette

## Brand Colors

| Token | Hex | Usage |
|-------|-----|-------|
| **Carnelian Red** | `#B7410E` | Primary brand color, headings, primary buttons |
| **Deep Orange** | `#FF6F00` | Accents, highlights, hover states |
| **Electric Blue** | `#00B0FF` | Links, interactive elements, info states |

## Neutral & Background Tokens

| Token | Hex | Usage |
|-------|-----|-------|
| **Forge Black** | `#1A1A1A` | Dark mode background |
| **Night Lab Gray** | `#2D2D2D` | Dark mode surface, cards |
| **Steel Gray** | `#6B6B6B` | Secondary text, borders |
| **Warm Gray** | `#E8E4E0` | Light mode background |
| **Paper White** | `#F5F3F1` | Light mode surface, cards |
| **Pure White** | `#FFFFFF` | Text on dark, backgrounds |

## Semantic Colors

| Token | Hex | Usage |
|-------|-----|-------|
| **Success Green** | `#22C55E` | Success states, confirmations |
| **Warning Amber** | `#F59E0B` | Warnings, cautions |
| **Error Crimson** | `#DC2626` | Errors, destructive actions |
| **Info Blue** | `#3B82F6` | Information, tips |

## Gemstone Variations

| Token | Hex | Usage |
|-------|-----|-------|
| **Carnelian Light** | `#E0613E` | Lighter accent, gradients |
| **Carnelian Dark** | `#8B2E0A` | Darker accent, shadows |
| **Carnelian Accent** | `#D24B2A` | Mid-tone for UI elements |
| **Gold Highlight** | `#F2C14E` | Premium features, achievements |

## CSS Variables

```css
:root {
  /* Brand */
  --carnelian-red: #B7410E;
  --deep-orange: #FF6F00;
  --electric-blue: #00B0FF;
  
  /* Neutral */
  --forge-black: #1A1A1A;
  --night-lab-gray: #2D2D2D;
  --steel-gray: #6B6B6B;
  --warm-gray: #E8E4E0;
  --paper-white: #F5F3F1;
  --pure-white: #FFFFFF;
  
  /* Semantic */
  --success: #22C55E;
  --warning: #F59E0B;
  --error: #DC2626;
  --info: #3B82F6;
  
  /* Gemstone variations */
  --carnelian-light: #E0613E;
  --carnelian-dark: #8B2E0A;
  --carnelian-accent: #D24B2A;
  --gold-highlight: #F2C14E;
}
```

## Usage Guidelines

- **Forge Theme** (Dark): Use `forge-black` background with `carnelian-red` and `deep-orange` accents
- **Night Lab Theme** (Dark): Use `night-lab-gray` surfaces with `electric-blue` for interactive elements
- **Paper Theme** (Light): Use `paper-white` background with `carnelian-red` for brand elements

See `brand-tokens.css` for complete token definitions.
