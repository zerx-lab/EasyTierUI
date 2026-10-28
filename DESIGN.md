---
name: EasyTierUI
description: Native client for EasyTier mesh networks — a quiet switchboard of networks and peers whose lamps light only when something changes.
colors:
  cobalt: "#3156d3"
  cobalt-glow: "#7896ff"
  on-cobalt: "#ffffff"
  on-cobalt-glow: "#0b1020"
  lamp-green: "#146f46"
  lamp-green-glow: "#3fd394"
  relay-amber: "#9a5b06"
  relay-amber-glow: "#f0b546"
  fault-red: "#c1343a"
  fault-red-glow: "#f0595e"
  on-fault-glow: "#14070a"
  series-teal: "#0f8f7a"
  series-teal-glow: "#2fd1b2"
  series-violet: "#7a4fd0"
  series-violet-glow: "#ab8cff"
  series-sky: "#0a7fb5"
  series-sky-glow: "#4cc0f0"
  series-slate: "#6b7280"
  series-slate-glow: "#9aa1af"
  paper-floor: "#f6f7f9"
  paper-panel: "#ffffff"
  paper-sidebar: "#eef0f3"
  paper-well: "#eceef2"
  paper-hover: "#e4e7ec"
  paper-rule: "#dfe2e8"
  paper-field-edge: "#878d99"
  ink: "#16181d"
  pencil: "#5b616e"
  night-floor: "#0c0e12"
  night-sidebar: "#111318"
  night-panel: "#14161b"
  night-raised: "#181b21"
  night-well: "#20232b"
  night-hover: "#272b34"
  night-rule: "#262a33"
  night-field-edge: "#646a78"
  readout: "#eceef2"
  dim-steel: "#9aa1af"
typography:
  headline:
    fontFamily: "system-ui, sans-serif"
    fontSize: "20px"
    fontWeight: 600
    lineHeight: "28px"
  title:
    fontFamily: "system-ui, sans-serif"
    fontSize: "16px"
    fontWeight: 600
    lineHeight: "24px"
  body:
    fontFamily: "system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: "20px"
  body-touch:
    fontFamily: "system-ui, sans-serif"
    fontSize: "16px"
    fontWeight: 400
    lineHeight: "24px"
  control:
    fontFamily: "system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 500
    lineHeight: "20px"
  label:
    fontFamily: "system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 500
    lineHeight: "16px"
  meta:
    fontFamily: "system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: "16px"
  mono:
    fontFamily: "ui-monospace, monospace"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: "20px"
    fontFeature: "tnum"
  stat:
    fontFamily: "ui-monospace, monospace"
    fontSize: "24px"
    fontWeight: 500
    lineHeight: "32px"
    fontFeature: "tnum"
rounded:
  control: "6px"
  container: "8px"
  pill: "9999px"
spacing:
  "0.5": "2px"
  "1": "4px"
  "2": "8px"
  "3": "12px"
  "4": "16px"
  "6": "24px"
  "8": "32px"
components:
  button-primary:
    backgroundColor: "{colors.cobalt}"
    textColor: "{colors.on-cobalt}"
    typography: "{typography.control}"
    rounded: "{rounded.control}"
    padding: "6px 12px"
    height: "32px"
  button-primary-touch:
    height: "44px"
    padding: "10px 16px"
  button-secondary:
    backgroundColor: "{colors.paper-well}"
    textColor: "{colors.ink}"
    typography: "{typography.control}"
    rounded: "{rounded.control}"
    padding: "6px 12px"
    height: "32px"
  button-secondary-hover:
    backgroundColor: "{colors.paper-hover}"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    padding: "6px 12px"
    height: "32px"
  button-destructive:
    backgroundColor: "{colors.fault-red}"
    textColor: "{colors.on-cobalt}"
    typography: "{typography.control}"
    rounded: "{rounded.control}"
    padding: "6px 12px"
    height: "32px"
  input:
    backgroundColor: "{colors.paper-panel}"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.control}"
    padding: "6px 10px"
    height: "32px"
  input-touch:
    typography: "{typography.body-touch}"
    height: "44px"
  status-chip:
    backgroundColor: "color-mix(in srgb, #146f46 10%, transparent)"
    textColor: "{colors.ink}"
    typography: "{typography.label}"
    rounded: "{rounded.pill}"
    padding: "0 8px"
    height: "20px"
  status-lamp:
    backgroundColor: "{colors.lamp-green}"
    rounded: "{rounded.pill}"
    size: "8px"
  card:
    backgroundColor: "{colors.paper-panel}"
    textColor: "{colors.ink}"
    rounded: "{rounded.container}"
    padding: "16px"
  peer-row:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    padding: "8px 12px"
    height: "44px"
  sidebar-item:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.control}"
    padding: "6px 10px"
    height: "32px"
  sidebar-item-active:
    backgroundColor: "{colors.paper-hover}"
  target-switcher:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    typography: "{typography.title}"
    rounded: "{rounded.control}"
    padding: "6px 10px"
    height: "36px"
  capability-notice:
    backgroundColor: "color-mix(in srgb, #3156d3 10%, transparent)"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.container}"
    padding: "12px 16px"
  dialog:
    backgroundColor: "{colors.paper-panel}"
    textColor: "{colors.ink}"
    rounded: "{rounded.container}"
    padding: "24px"
    width: "480px"
  callout:
    backgroundColor: "color-mix(in srgb, #3156d3 10%, transparent)"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.container}"
    padding: "12px 16px"
---

# Design System: EasyTierUI

Status: **seed**. The project has no screens yet; these values are the direction the first screens implement. Once they
ship, re-run `$impeccable document` in scan mode and let the code correct this file. `theme.json` implements the color
and radius tokens below; the two always change together.

## Overview

**Creative North Star: "The Quiet Switchboard"**

EasyTierUI is a switchboard for a private mesh: each network is a board, each peer a line with a lamp. At rest the board
is quiet — neutral layers, hairlines, readable text — so when a lamp changes (a peer falls back to a relay, the core
stops, a node joins) it is the only thing that moves. Color means state, or the one action that matters on the screen;
it is never decoration. This is SigNoz's night-shift discipline (layered neutrals, one accent, reserved hues, machine
data in monospace, flat surfaces) carried to a native, cross-platform client that also runs on phones.

Two scenes share one system, on every platform. Someone at a laptop or phone joins their own devices and wants a yes
or no at a glance: comfortable density, one decision per screen, touch-sized targets. An operator manages dozens of
nodes, or stands up a control plane, from a desktop, a phone or the server's web dashboard: medium density, compact only
in admin tables. Both meet the same layers, tokens and lamps, and the same pages whichever role or target is selected.

The app follows the system appearance. Light and dark are equal themes on one layer model; neither is derived from the
other. Platform chrome stays the platform's: iOS and Android headers, back gestures and alerts; a sidebar shell on the
desktops. The brand lives where the platforms leave room: the cobalt accent, the status lamps and the precision of the
machine-data typography.

**Key Characteristics:**
- Three neutral layers (floor, panel, well) with 1 pt hairlines, identical in structure in light and dark.
- One chrome accent (Cobalt); green, amber and red only for link state and errors, always with words.
- System UI font for people, system monospace with tabular figures for machine data.
- Two radii: 6 pt for controls, 8 pt for containers; pills only for chips, switches and lamps.
- Flat at rest; one shadow recipe for popups; no glass.
- Quick, quiet motion (150 ms state, 200 ms overlay) that stops under Reduce Motion.

## Colors

Near-neutral cool grays carry the structure; one cobalt accent marks action and focus; three lamp hues report link
state. Every value has a token in `theme.json` (shadcn names, read in Rust through `theme()`; the app's own names
through `theme().color("…")`). Hex values here are for reference; code never contains them.

### Primary
- **Cobalt Line** (#3156d3 light, `primary`) / **Cobalt Glow** (#7896ff dark): the primary action of a screen (Join,
  Start, Save), the focus ring (`ring`), the selected sidebar item's marker, checked switches and checkboxes, links to
  external resources. Text on it is **On Cobalt** (#ffffff light, #0b1020 dark; `primary-foreground`), ≥ 6:1.
- **Cobalt tint** (`primary-muted`, 10 %): informational callouts and the selected row's fill.

### Secondary (lamp hues)
- **Lamp Green** (#146f46 light / #3fd394 dark, `success`, tint `success-muted`): running, peer reachable directly
  (P2P), check passed.
- **Relay Amber** (#9a5b06 light / #f0b546 dark, `warning`, tint `warning-muted`): degraded or attention — peer reached
  through a relay, high latency or loss, a setting that widens exposure, a core update available.
- **Fault Red** (#c1343a light / #f0595e dark, `destructive`, tint `destructive-muted`; text on it
  `destructive-foreground`: #ffffff light, **On Fault Glow** #14070a dark): failed, unreachable, destructive actions.
  Nothing else.

### Tertiary (data series)
Traffic and latency charts use `chart-1` … `chart-5`: Cobalt, **Series Teal** (#0f8f7a / #2fd1b2), **Series Violet**
(#7a4fd0 / #ab8cff), **Series Sky** (#0a7fb5 / #4cc0f0), **Series Slate** (#6b7280 / #9aa1af). Received traffic is
`chart-1`, sent is `chart-2`. No red or amber series.

### Neutral

| Role | Token | Light | Dark |
|---|---|---|---|
| Floor: the window / screen background | `background` | **Paper Floor** #f6f7f9 | **Night Floor** #0c0e12 |
| Sidebar (desktop shell) | `sidebar` | **Paper Sidebar** #eef0f3 | **Night Sidebar** #111318 |
| Panel: cards, grouped lists, dialogs, inputs | `card` | **Paper Panel** #ffffff | **Night Panel** #14161b |
| Raised: popovers, menus, tooltips | `popover` | **Paper Panel** #ffffff | **Night Raised** #181b21 |
| Well: secondary buttons, chips, code blocks | `secondary`, `muted` | **Paper Well** #eceef2 | **Night Well** #20232b |
| Hover / selected row | `accent`, `sidebar-accent` | **Paper Hover** #e4e7ec | **Night Hover** #272b34 |
| Hairline | `border`, `sidebar-border` | **Paper Rule** #dfe2e8 | **Night Rule** #262a33 |
| Input edge, checkbox and radio outlines | `input` | **Paper Field Edge** #878d99 | **Night Field Edge** #646a78 |
| Primary text | `foreground` | **Ink** #16181d | **Readout** #eceef2 |
| Secondary text, placeholders, icons | `muted-foreground` | **Pencil** #5b616e | **Dim Steel** #9aa1af |

Contrast is checked (WCAG): primary and secondary text ≥ 5:1 on every layer, hover fill included; lamp hues and Cobalt
≥ 4.5:1 on the floor, panels, the sidebar and their own 10 % tints; input edges ≥ 3:1 on the floor and panels (fields
stay identifiable without color or fill); the focus ring ≥ 5.7:1 against the floor. Hairlines (`border`) only separate
regions and carry no meaning on their own.

### Named Rules
**The One Signal Rule.** Cobalt is the only accent the chrome uses: the primary action, focus, selection, checked state
and links. Cobalt anywhere else is a bug.

**The Red Is Reserved Rule.** Fault Red means failure or destruction. No chart series, badge or illustration is red;
amber is not a softer red for "important".

**The Lamp Needs Words Rule.** A state is never color alone: every lamp sits next to a label ("Direct", "Relayed",
"Unreachable" / "直连"、"中继"、"不可达") or an icon with an accessible name.

**The Token, Not Hex Rule.** Colors come from `theme.json` through `theme()`; a raw color breaks the other appearance and
a host's theme. A missing role is added to `theme.json` (light and dark) and to this section in the same change.

## Typography

**Display Font:** none. The product has no display tier; screens lead with a title.
**Body Font:** the platform's UI font (`font-sans` unset: SF Pro, Segoe UI, Roboto, Cantarell; PingFang SC, Microsoft
YaHei, Noto Sans CJK for Chinese).
**Label/Mono Font:** the platform's monospace (`font-mono` unset: SF Mono, Cascadia Mono / Consolas, Roboto Mono,
DejaVu Sans Mono), always with tabular figures.

**Character:** the system face makes the app feel native on each platform and handles CJK well; the monospace marks
everything a machine produced, so an address can be read, compared and copied exactly.

### Hierarchy
Sizes are points and grow with the user's text size; Rust uses NSH's presets (`.text_xs()` 12/16 … `.text_2xl()`
24/32) and `.font_weight(FontWeight::…)`.
- **Headline** (600, 20/28, `.text_xl()`): page titles on the desktops and the web; iOS / Android use the system
  header's title instead.
- **Title** (600, 16/24, `.text_base()`): network names in cards, dialog titles, section heads inside a page.
- **Body** (400, 14/20, `.text_sm()`): desktop working size — rows, forms, dialogs, callouts.
- **Body Touch** (400, 16/24, `.text_base()`): the working size on phones and tablets.
- **Control** (500, 14/20): button and tab labels; 16/24 on touch.
- **Label** (500, 12/16, `.text_xs()`, sentence case, `muted-foreground`): section labels, field captions, chip text.
- **Meta** (400, 12/16): timestamps ("updated 3 s ago"), helper text, versions in footers.
- **Mono** (400, 13/20, tabular): virtual IPs, CIDRs, peer URLs, peer and instance ids, versions, latency, loss, byte
  counts, config snippets, logs.
- **Stat** (mono 500, 24/32, tabular): the one headline number of a network card (peers online, latency).

### Named Rules
**The Machine Voice Rule.** What the core or the network produced (`10.144.144.2/24`, `tcp://1.2.3.4:11010`,
`2.6.4-8428a89d`, `23 ms`, `1.2 GiB`) is set in the monospace with tabular figures; what a person wrote for a person is
in the UI font. Never mix the two in one text run; put them in adjacent texts.

**The Quiet Section Rule.** Sections are introduced by a Label in sentence case and `muted-foreground` — no uppercase,
no letter spacing (Chinese has no case, and tracking hurts CJK), no eyebrow above a heading. Hierarchy comes from
grouping and space, not from louder labels.

## Layout

Spacing follows Tailwind's 4-point step, in classes and typed Rust methods alike: 4 (`.p_1()`) between an icon and its
label, 8 (`_2`) between related controls, 12 (`_3`) inside rows and compact cards, 16 (`_4`) inside cards and as the
page gutter, 24 (`_6`) between sections, 32 (`_8`) around empty states.

- **Shell on every platform:** a target switcher (this device, each easytier-web server, the hosted control plane) sits
  above the role's sections — Node (networks, peers), Console (machines, networks, credentials), Host (status,
  hardening, deployment). Switching keeps each target's state; nothing is hidden by platform, unavailable actions show a
  Capability Notice.
- **Desktops and web:** a sidebar (`sidebar` tokens, about 240 pt) with the target switcher on top and the sections
  below; the main pane shows the selection. Windows narrower than about 640 pt collapse to a single column with a
  `NavStack`, as on phones. Each pane owns its scrolling; the sidebar and the page header stay put. On a server the web
  build opens on that server's control plane as its target.
- **Desktop title bar (the app's own):** the system title bar is hidden and the app draws a 44 pt bar that moves the
  window (a double click does what the user set). No app name in it. In the wide layout its two halves are the
  sidebar's top and the page header (the page title in Headline); the sidebar toggle (32 pt ghost button, a sidebar
  glyph) sits 12 pt after the window controls and does not move. The toggle slides the sidebar shut or open (its
  width to and from 240 pt, a critically damped 280 ms spring, instant under Reduce Motion) while the page widens and
  its title moves past the toggle; the choice is kept. In the compact layout the bar is one empty full-width row above
  the stack. The native window controls stay the platform's (traffic lights, Windows' caption buttons, GTK's window
  buttons); the bar keeps 12 pt clear of the edges `title_bar()` reports for them. Same fills as the panes below
  (`sidebar`, `background`), a hairline under the page header, no shadow. The web keeps the browser's chrome: the same
  rows are plain headers there.
- **Phones and tablets:** `NavStack` with the system header and back gesture; the target switcher is the title control
  of the top-level screen; a tab bar (iOS) / navigation bar (Android) for the 2–4 sections of the current role; one
  column; safe areas respected; primary actions within thumb reach.
- **Density tiers:** comfortable on first run and on touch screens (44 pt targets), medium for desktop chrome (32 pt
  controls, 36–44 pt rows), compact (28 pt) only for admin tables and toolbars.
- **Text size:** frames grow with their labels (`min_h` plus padding, never a fixed height around text); rows wrap or
  stack when text gets large; layouts leave room for the longer of English and Chinese.

## Elevation & Depth

Depth is tone, not light. The floor, panel and well layers step in brightness and are outlined by 1 pt hairlines;
cards, lists, the sidebar and headers have no shadow. Only surfaces that float over the page lift, all with one recipe:
`popover` fill, 1 pt `border`, `radius`, and NSH's `shadow_md` (`shadow_lg` for dialogs and sheets on the desktops).
Mobile sheets and alerts are the platform's own.

### Shadow Vocabulary
- **Popup lift** (`.shadow_md()`): popovers, menus, tooltips, select lists.
- **Dialog lift** (`.shadow_lg()`): desktop and web dialogs over a scrim of `foreground` at 40 %.

### Named Rules
**The Flat Resting State Rule.** A surface that stays on screen never casts a shadow; it separates by one layer step and
a hairline.

**The Solid Overlay Rule.** Floating layers are solid `popover` with a hairline. No translucency or blur as decoration.

## Shapes

- **Control (6 pt, theme `radius-control`):** buttons, inputs, selects, segmented controls, sidebar items, row highlights.
- **Container (8 pt, theme `radius`):** cards, grouped lists, callouts, popovers, menus, dialogs.
- **Pill:** status chips, switches, lamps, avatars only.
- **Nesting:** inner radius = max(0, outer − gap), so a control inside a 16 pt-padded card keeps 6 pt; children and
  highlights follow their container's corners (`.overflow_hidden()` where they would poke out).
- **Borders:** 1 pt solid. Dashed only for the drop zone of a config import.

**The Two-Tier Radius Rule.** Controls are 6 pt and containers 8 pt; anything else that is not a pill is a mistake.

## Components

Native in behavior, quiet in look: NSH's base components (`Button`, `Switch`, `Tabs`, `Dialog`, `AlertDialog`,
`Popover`, `Menu`, `Sheet`, `NavStack`) bring focus, keyboard and accessibility; the app's components in
`src/components/` give them these looks. State changes take 150 ms.

### Buttons
- **Shape:** control radius (6 pt). Desktop 32 pt high, 6×12 pt padding, Control type; touch at least 44 pt.
- **Primary:** Cobalt fill, On Cobalt label. At most one per screen region.
- **Secondary:** well fill (`secondary`), `foreground` label; hover / pressed move to `accent`.
- **Ghost:** transparent, `foreground` label, `accent` on hover / press; for toolbars and dialog dismissals.
- **Destructive:** Fault Red fill, `destructive-foreground` label; only in a confirmation that names the object.
- **Focus / disabled / busy:** a 2 pt `ring` outline with 2 pt offset; disabled at 50 % opacity with the reason nearby;
  busy keeps the label width and shows a spinner, never a second click.

### Status Lamp (signature)
An 8 pt pill in the lamp hue, followed by its word, in one row: green "Running" / "Direct", amber "Relayed" /
"Degraded", red "Failed" / "Unreachable", a hollow `muted-foreground` ring for "Stopped". "Starting" / "Connecting" use
the hollow ring with a slow opacity pulse that stops under Reduce Motion. The lamp is the only colored element of a row.

### Chips
- **Status chip:** 20 pt pill, 10 % hue tint, `foreground` label in Label type, optional lamp. For route type (P2P /
  Relay), NAT type, protocol.
- **Neutral chip:** well fill, `muted-foreground` label: tags, versions, counts. Most chips are neutral.

### Network Card (signature)
The home of one network: Title (network name) with the Status Lamp on the first line; the node's virtual IP in Mono
with a copy button; a Stat (peers online) and a Meta line (core version, last update); the primary action (Start / Stop)
at the trailing edge. Panel fill, container radius, 16 pt padding, hairline border, no shadow.

### Peer Row (signature)
Hostname in Body; virtual IP, latency, loss and traffic in Mono with tabular figures, right-aligned in columns on the
desktops and stacked under the hostname on phones; a Status Chip for P2P / Relay. Rows are 44 pt (36 pt in compact
admin tables), separated by hairlines, `accent` on hover and selection. Latency turns amber only past a threshold, with
the number still readable.

### Inputs / Fields
- **Style:** panel fill, 1 pt `input` edge, control radius, 32 pt (44 pt touch), visible label above, helper or error
  text below.
- **Focus:** the edge becomes `ring` plus a 2 pt `ring` outline.
- **Error:** `destructive` edge and a message that names the field and the fix; the field keeps its value.
- **Secret field (signature):** masked by default, with Show, Copy and Generate actions; never prefilled in screenshots
  or tests with a real value; a strength hint when typed by hand.
- **Address fields:** Mono, with the effective default port shown as placeholder text.

### Navigation
- **Desktop / web sidebar:** 32 pt items in Body with 16 pt icons; hover and selection share `sidebar-accent`; the
  selected item adds a 2 pt Cobalt marker at its leading edge; section Labels in sentence case.
- **Mobile:** the system header and back gesture; tab bar / navigation bar per platform for the current role's sections.

### Target Switcher (signature)
One control at the top of the shell: the current target's name, its kind as a neutral chip ("This device", "Server",
"Hosted" / "本机"、"服务器"、"托管") and its Status Lamp; opening it lists every target with lamp and kind, plus "Add
server" and, on the desktops, "Host a control plane". On the desktops and web it is a 36 pt sidebar row with a `Menu`;
on phones the header's title opens a sheet. Signing out or removing a target asks first and names it.

### Capability Notice
Where this platform cannot run a feature locally, the page still exists: a `primary-muted` callout states what this
device cannot do and why in one sentence ("This browser can't join a network." / "浏览器无法加入组网。"), followed by
the remote alternative as a button ("Manage this server's node", "Deploy a control plane"). Never a disabled page
without a way forward, never a feature silently missing.

### Dialogs and Sheets
Desktop and web dialogs: panel fill, container radius, 24 pt padding, 480 pt wide (640 for config editors), Dialog lift.
Phones: the platform's sheet and `AlertDialog`. Confirmations name the object and the consequence ("Delete profile
“home”? Its config and saved secret are removed from this device.").

### Callouts
10 % tint of the hue (`primary-muted`, `warning-muted`, `destructive-muted`), container radius, 12×16 pt padding, a hue
icon and `foreground` text. For exposure warnings (exit node, subnet proxy, encryption off), core problems and tips.

## Do's and Don'ts

### Do:
- **Do** take every color from `theme()` / token classes, and add missing roles to `theme.json` in light and dark.
- **Do** build on the floor / panel / well layers with 1 pt hairlines.
- **Do** set machine data in the monospace with tabular figures, and people's text in the system UI font.
- **Do** pair every lamp and status color with a word or a named icon.
- **Do** use 6 pt radii for controls and 8 pt for containers, and the 4-point spacing steps.
- **Do** keep touch targets at least 44 × 44 pt and let frames grow with text.
- **Do** keep state transitions at 150 ms and overlays at 200 ms, and honor `reduce_motion()`.
- **Do** keep iOS and Android headers, back gestures and alerts native.

### Don't:
- **Don't** write hex, `rgb()`, `hsl()` or Tailwind palette colors in UI code.
- **Don't** use Cobalt for decoration, or a second accent in the chrome.
- **Don't** use red for anything but failure and destruction, or amber as "important".
- **Don't** put shadows on cards, lists, the sidebar or headers, or use glass and blur.
- **Don't** use uppercase or letter-spaced labels, or an eyebrow above a heading.
- **Don't** use a radius other than 6 pt, 8 pt or a pill.
- **Don't** fix the height of anything that holds text.
- **Don't** show a peer or network as connected before the core reports it.
