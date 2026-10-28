---
description: UI colors come from theme.json tokens through theme(), never raw color literals
condition: '(?:"#[0-9a-fA-F]{3,8}"|\b(?:rgba?|hsla?)\(\s*(?:0x|\d)|\b(?:Hsla|Rgba)\s*\{|\bbg-(?:slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose)-\d{2,3}\b|\btext-(?:slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose)-\d{2,3}\b)'
scope: "tool:edit(src/**/*.rs), tool:write(src/**/*.rs), tool:edit(**/*.tsx), tool:write(**/*.tsx)"
interruptMode: tool-only
---
This change writes a raw color (hex, `rgb()`, `hsl()`, an `Hsla` / `Rgba` literal, or a Tailwind palette class) into UI code. EasyTierUI follows the system's light and dark appearance, and a literal is right in at most one of them (DESIGN.md "The Token, Not Hex Rule").

Read the color from the theme by role, inside the binding so appearance changes reach it:

- Rust: `.bg(move || theme().card)`, `.text_color(move || theme().muted_foreground)`, the app's own tokens by name: `theme().color("success")`, `theme().color("warning-muted")`.
- TSX: token classes (`bg-card`, `text-muted-foreground`, `bg-success-muted`) or `token("success")`.

If no token has the role you need, add it to `theme.json` with a light and a dark value and document it in DESIGN.md's Colors section in the same change. Raw colors live only in `theme.json`.
