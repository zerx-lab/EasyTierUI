---
description: Layout uses the spacing / type / radius scale, not literal point values
condition: '(?:\bpx\(\s*\d|\b(?:p|px|py|pt|pb|pl|pr|m|mx|my|mt|mb|ml|mr|gap|w|h|min-w|min-h|max-w|max-h|text|rounded|top|left|right|bottom|inset)-\[\d)'
scope: "tool:edit(src/**/*.rs), tool:write(src/**/*.rs), tool:edit(**/*.tsx), tool:write(**/*.tsx)"
interruptMode: never
---
This change writes a literal point value (`px(13.0)`, `p-[13px]`). DESIGN.md's Layout and Shapes sections define the scale: spacing on Tailwind's 4-point step (`.p_2()`, `.gap_3()`, `p-4`), type on `.text_xs()` … `.text_2xl()`, radii from the theme (`theme().radius`, `theme().number("radius-control")`, `rounded-(--radius)`), control heights from the app's components (32 pt desktop, at least 44 pt on touch screens). Text grows with the user's text size; fixed boxes around it clip.

Literal points are right only for physical boundaries: a 1 pt hairline, a platform inset, a bitmap size, a hit-test tolerance. Keep those, with a comment saying which; otherwise move the value onto the scale.
