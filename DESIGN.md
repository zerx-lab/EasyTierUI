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
  figure:
    fontFamily: "ui-monospace, monospace"
    fontSize: "20px"
    fontWeight: 500
    lineHeight: "28px"
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
  toolbar:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    padding: "8px 24px"
    height: "48px"
  search-field:
    backgroundColor: "{colors.paper-panel}"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.control}"
    padding: "6px 32px"
    height: "32px"
    width: "260px"
  summary-strip:
    backgroundColor: "{colors.paper-panel}"
    textColor: "{colors.ink}"
    typography: "{typography.figure}"
    rounded: "{rounded.container}"
    padding: "12px 16px"
  data-table-header:
    backgroundColor: "{colors.paper-well}"
    textColor: "{colors.pencil}"
    typography: "{typography.label}"
    padding: "0 16px"
    height: "36px"
  data-table-row:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    padding: "4px 16px"
    height: "40px"
  data-table-row-touch:
    height: "44px"
  data-table-row-hover:
    backgroundColor: "{colors.paper-hover}"
  drawer:
    backgroundColor: "{colors.paper-panel}"
    textColor: "{colors.ink}"
    rounded: "{rounded.container}"
    padding: "24px"
    width: "760px"
  icon-button:
    backgroundColor: "transparent"
    textColor: "{colors.pencil}"
    rounded: "{rounded.control}"
    size: "32px"
  icon-button-touch:
    size: "44px"
  kind-chip:
    backgroundColor: "{colors.paper-well}"
    textColor: "{colors.pencil}"
    typography: "{typography.label}"
    rounded: "{rounded.pill}"
    padding: "0 8px"
    height: "20px"
  threshold-bar:
    backgroundColor: "{colors.paper-well}"
    rounded: "{rounded.pill}"
    height: "4px"
    width: "64px"
---

# Design System: EasyTierUI

Status: **redesign in progress**. Every role's pages are built from these values and the components in
`src/components/`: the icon set, toolbar, search field, summary strip, data table, drawer, key-value grid, charts,
threshold bar, icon buttons and menus, fields, secret field, switch rows with exposure warnings, select, dialogs,
callouts, chips, cards, code blocks, QR view, busy buttons, age text. Nobody has looked at them on a screen yet: re-run
`$impeccable document` in scan mode after the first visual review and let the code correct this file. `theme.json`
implements the color and radius tokens below; the two always change together.

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
- **Stat** (mono 500, 24/32, tabular): a headline number (peers online, latency) where a panel has room for one.
- **Figure** (mono 500, 20/28, tabular): the value of a Summary Strip cell, with a muted unit in Body beside it.

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
  Capability Notice. The user's work mode decides which roles are listed: a client lists the node alone (its pages
  without a section label, the target switcher a plain row without a menu), control lists every role, custom the
  roles the user turned on. A hidden role that still runs something here (this device's networks, the hosted control
  plane) keeps one sidebar row ("Control plane still running" / "控制平面仍在运行") that opens its pages.
- **First run (desktops and phones):** before the shell, one centered question (at most 560 pt wide, comfortable
  density): "How will you use EasyTierUI?" / "选择工作模式", two radio cards — Client / 客户端 (preselected) and
  Control / 控制端, each an icon, its name and one sentence; a checked card has a Cobalt edge, icon and radio dot — and
  one primary Continue. Under the app's title bar on the desktops, the system header on phones. Settings → Work mode
  changes it later (Client, Control, Custom with a switch per role, the last one on disabled) and says that hiding
  stops nothing.
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

### Page anatomy
Under the title bar's row every page starts with a **toolbar**: the page's context on the left, its actions on the
right, a hairline below. Pages may also put their primary actions in the title-bar row itself, at its trailing edge
(`pages::header_actions`), which the shell renders on wide layouts and in the compact header. Under the toolbar the
page body has 24 pt gutters and 24 pt between sections. Order within a page: toolbar, summary strip, charts, data
table; the detail of a row opens in a drawer, not on another page.

### Density
SigNoz-grade density, kept legible: 12 pt Label and Meta type for captions, 13 pt Mono for figures, 40 pt rows, 36 pt
headers and a 36 pt toolbar control row. Touch screens keep 44 pt targets and 16 pt body type. Panels and tables
separate by hairlines and layer steps, never by shadows or nested cards.

## Elevation & Depth

Depth is tone, not light. The floor, panel and well layers step in brightness and are outlined by 1 pt hairlines;
cards, lists, the sidebar and headers have no shadow. Only surfaces that float over the page lift, all with one recipe:
`popover` fill, 1 pt `border`, `radius`, and NSH's `shadow_md` (`shadow_lg` for dialogs and sheets on the desktops).
Mobile sheets and alerts are the platform's own.

### Shadow Vocabulary
- **Popup lift** (`.shadow_md()`): popovers, menus, tooltips, select lists.
- **Dialog lift** (`.shadow_lg()`): desktop and web dialogs over a scrim of `foreground` at 40 %, and the Drawer over a
  backdrop of `foreground` at 24 %.

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

### Network Row (signature)
The home of one network is a row of the Data Table (above), not a card: its name over the EasyTier network name, the
Status Lamp with its word, the virtual IP in Mono without its prefix, the peers online with direct / relayed bars,
the received and sent rates with their sparklines (only from real samples), a neutral chip for how it runs (TUN / No
TUN / Managed), and at the trailing edge Start / Stop and a more menu (Settings, Logs, Export, Delete). The whole row
opens the Network drawer; a failed network says why in a full-width line under its row, with the action that fixes it.

### Peer Row (signature)
A peer is a row of the Data Table (above): hostname in Body; virtual IP, latency, loss and traffic in Mono with
tabular figures, right-aligned and never wrapping; a Status Chip for P2P / Relay; latency and loss as Threshold
Bars. Rows are 40 pt (44 pt on touch), separated by hairlines, `accent` on hover, `primary-muted` when selected, and
open the Peer drawer. On phones the figures stack in a wrapping line under the hostname.

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
"Hosted" / "本机"、"服务器"、"托管") and its Status Lamp; opening it lists every target the work mode serves with lamp
and kind, plus "Add server" while the console is listed and, on the desktops, "Host a control plane" while the host
is. In client mode it has one target and is a plain row: no chevron, no menu. On the desktops and web it is a 36 pt
sidebar row with a `Menu`; on phones the header's title opens a sheet. Signing out or removing a target asks first and
names it.

### Capability Notice
Where this platform cannot run a feature locally, the page still exists: a `primary-muted` callout states what this
device cannot do and why in one sentence ("This browser can't join a network." / "浏览器无法加入组网。"), followed by
the remote alternative as a button ("Manage this server's node", "Deploy a control plane"). Never a disabled page
without a way forward, never a feature silently missing.

### Dialogs and Sheets
Confirmations are the platform's own alert on every platform. A short or multi-step task that needs protected focus
(create, join, import, enroll, add server) stays a centered dialog on the desktops and the web: panel fill, container
radius, 24 pt padding, 480 pt wide (640 for config editors), Dialog lift; its body scrolls inside the panel with room
for focus rings, and on phones it is a full-height page sheet. Anything that is an entity's detail or editing is a
Drawer (above). Confirmations name the object and the consequence ("Delete profile “home”? Its config and saved
secret are removed from this device.").

### Callouts
10 % tint of the hue (`primary-muted`, `warning-muted`, `destructive-muted`), container radius, 12×16 pt padding, a hue
icon and `foreground` text. For exposure warnings (exit node, subnet proxy, encryption off), core problems and tips.

### Icons
One authored set, drawn natively (`src/components/icon.rs`): SVG path data on a 24-unit grid, stroked on a canvas with
round caps and joins at 1.5 pt whatever the size, 16 pt in buttons, rows and menus, 14 pt in dense table cells and
32 pt in empty states. The tint is a theme token read in the canvas's binding, so a change of appearance redraws it:
`muted-foreground` at rest, `foreground` under the pointer, `primary` for a selected toggle, a lamp hue only for state.
The set: network, peers, server, key, activity, shield, rocket, settings, plus, import, export, download, link, play,
stop, restart, refresh, more, copy, check, close, search, chevron-down, chevron-right, info, alert-triangle,
alert-circle, external, terminal, qr, eye, eye-off, trash, edit, globe, lock, arrow-down (received), arrow-up (sent),
filter. An icon is decoration: the control that holds it has the accessible name.

**The Drawn Icon Rule.** Icons are drawn, never typed. A glyph (`i`, `!`, a triangle or tick character, an emoji) is a
font-dependent stand-in; a missing icon is added to the set with the same stroke, not approximated with text.

### Toolbar and Search Field
The row under the page title: context on the left (search, segmented filters, group-by), a flexible spacer, the page's
actions on the right with the primary one last, and a hairline below. It spans the page edge to edge (it cancels the
24 pt page gutter) and wraps on narrow windows. The search field is a 32 pt input (44 pt touch) with a search icon at
its leading edge, a clear button while it holds text, and the usual focus ring; it filters as you type.

### Summary Strip
One panel of 3-5 cells divided by hairlines, never separate cards. A cell is a Label over a value in Figure type
(mono 20/28, tabular) with a muted unit, an optional Meta caption and optional status-count bars: a 3 x 14 pt bar in
the lamp hue, a tabular count and its word ("2 direct", "0 relayed"). The strip states only what the core reported: a
cell with no data is left out, never shown as zero. Cells share the width and wrap to a second row when narrow.

### Data Table (signature)
A panel (panel fill, hairline, container radius, clipped to its corners) with a 36 pt header row on the well fill in
Label type and 40 pt rows (44 pt on touch) separated by hairlines. A row has no zebra and no resting fill; `accent`
shows under the pointer and `primary-muted` marks the selected row; the whole row is one button that opens the
drawer. Columns have fixed widths, except one flexible text column. A cell is one line, cut with an ellipsis; numbers,
addresses, versions and rates are Mono with tabular figures and right-aligned, in a column headed right-aligned; a
missing value is a muted dash. A column whose meaning needs saying (thresholds, units) gets an info icon with a
tooltip in its header. Columns that do not fit a narrow window can be hidden below a width. On phones and narrow
windows rows stack: the first cell is the title and the other cells follow in a wrapping line of facts, each still one
line, with the action cells at the row's end. A failed row shows its reason and its action under the row. An empty
table keeps its header and shows an empty state inside the panel.

**The No-Wrap Number Rule.** A number, address, version or id is never allowed to wrap. Give its column the width of
its longest value, set it in Mono with `number_of_lines(1)`, and let a text that still does not fit end in an ellipsis.
A cell that breaks `10.126.126.2/24` into two lines is a bug in the column, not in the data.

### Drawer (signature)
The place for an entity's detail and editing: a network's, a machine's, a peer's, a server network's, a credential's.
On the desktops and the web a right-side sheet `min(760 pt, 72 %)` of the window wide (the whole window below 640 pt),
panel fill, a hairline on its leading edge, the Dialog lift and a backdrop of `foreground` at 24 %; on phones a
full-height page sheet whose close is back. Anatomy, top to bottom: a header (the entity's name in Title, a neutral
kind chip such as "Network", the status lamp with its word, the header's own actions such as Start / Stop, a close
icon button; a hairline below), an optional key-value grid, optional segmented tabs, a scrolling body padded 24 pt on
every side, and a sticky footer of actions under a hairline. Escape, the backdrop and the close button close it.

**The Drawer Rule.** Detail and editing of an entity that lives in a list open in a drawer, not in a centered modal.
Modals stay for what interrupts or needs protected focus: short confirmations (the platform's alert) and multi-step
creation flows (create, join, import, enroll, add server). The body is padded 24 pt on every side, inside the scroll
view, so a 2 pt focus ring offset by 2 pt is never clipped at the scroll edge; the same padding applies inside modals.

### Key-Value Grid
Facts as a Label over a value, in columns of at least 180 pt that fit the width and a row gap of 16 pt, so a drawer
never shows a near-empty second row. Machine data is Mono, one line, selectable, optionally with a copy button; a
missing value is a muted dash.

### Charts
`metric_chart` and `sparkline` draw in-memory samples (three minutes, read every second) on a canvas the app records:
area and line series, a hairline grid (`border` at low opacity) at the top, middle and bottom of the y axis, a y axis of
Mono labels in Meta size and `muted-foreground`, formatted with the unit (B/s, KiB/s, MiB/s, GiB/s; ms, s; %), and no
legend box: a row of 8 pt swatches and words above the plot. A byte axis tops out at a round number of the binary unit
it shows (2 KiB/s, not 2000 B/s). The x axis is time: each sample sits at the moment it was read, the axis ends at the
newest one and reaches back three minutes, so a fresh network fills it from the right and a missing reading is a
break in the line. Received is `chart-1`, sent `chart-2`, further series `chart-3` ... `chart-5`; a peer's color is
hashed from its id over that red-free palette, so it is the same in every chart. Before two samples the chart says it
is collecting them. A sparkline is 64 x 20 pt, a line only (evenly spaced), and sits in a table cell. The Peers table
shows each peer's rate now, received and sent; its byte totals since the link came up are in the peer's drawer.

**The Real Samples Rule.** A chart or sparkline shows only samples the core reported. No samples, no line: the empty
state says so. Never a decorative trend, a smoothed guess or a placeholder curve.

### Threshold Bar
A 4 pt bar (at least 64 pt wide) beside its number for latency and loss: `success` below the first threshold,
`warning` at or above it, `destructive` only for loss over 10 % or an unreachable peer. The number stays Mono and
readable; the color only repeats it. The column header's tooltip states the thresholds.

### Icon Button and More Menu
A ghost button holding one icon: 32 pt on the desktops, 44 pt on touch, the control radius, `accent` on hover and
press. It is named for screen readers and, with a pointer, by a tooltip. The more menu is an icon button that opens a
menu of labelled actions, each with its icon; a destructive action is drawn in `destructive` and asks its own
confirmation naming the object.

### Live Indicator
An 8 pt `success` dot with a slow pulse (stopped under Reduce Motion) while the page polls a running instance, beside
"Updated 3 s ago". Not polling, it is a hollow `muted-foreground` ring. Never alone: the words carry the meaning.

### Empty States
Inside the table or panel area: a 32 pt icon, a Title, one sentence and the primary action. No illustrations.

## Logo and App Icon

**The mark: "Lit Mesh".** Four nodes in a diamond, every pair linked (a full mesh: no node is the center), and one
node lit in Cobalt: the network, and the one lamp that tells you it is up. It is the app's own mark, not EasyTier's
logo (PRODUCT.md, Brand Commitments). Sources live in `brand/`; the PNGs are rendered from the SVGs, never edited.

| File | What | Used by |
|---|---|---|
| `brand/logo.svg` | The mark alone, transparent; Ink / Cobalt Line, Readout / Cobalt Glow under `prefers-color-scheme: dark` | README, documentation, a future in-app mark |
| `brand/icon.svg` → `brand/icon.png` | 824 pt Night Panel tile (radius 185) with a Night Rule hairline, centered in a transparent 1024 canvas; mark in Readout with a Cobalt Glow lamp | `nsh.json` `"icon"`: macOS (`.icns`), Windows (`.exe`), Linux (`.desktop`), Android launcher, web favicon / manifest / touch icon |
| `brand/icon-ios.svg` → `brand/icon-ios.png` | Full-bleed opaque Night Panel square, larger mark; iOS masks the corners itself and rejects alpha | `nsh package --target ios… --icon brand/icon-ios.png` |

Render after changing an SVG (resvg 0.48):

```sh
resvg -w 1024 -h 1024 brand/icon.svg brand/icon.png
resvg -w 1024 -h 1024 brand/icon-ios.svg brand/icon-ios.png
magick brand/icon-ios.png -background '#14161b' -alpha remove -alpha off PNG24:brand/icon-ios.png
```

**The Lit Lamp Rule.** The mark has exactly one lit node, in Cobalt; the other nodes and the links are the ink of the
appearance. The icon tile is dark in both appearances (it sits on docks, launchers and tabs, not on the app's floor).
The mark's Cobalt is the brand's, outside the chrome, so it does not count against the One Signal Rule. Keep the mark
at 16 pt or larger, with clear space of a quarter of its width; don't recolor the lamp to a state hue, add a second lit
node, outline it or set it on a photograph.

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
- **Do** draw icons from the one set (`icon(Icon, size)`), 1.5 pt strokes, tinted by a token.
- **Do** open a row's detail in a drawer, with its focus rings clear of the scroll edge.
- **Do** set numbers and addresses in Mono, one line, right-aligned in tables, with a muted dash when missing.

### Don't:
- **Don't** write hex, `rgb()`, `hsl()` or Tailwind palette colors in UI code.
- **Don't** use Cobalt for decoration, or a second accent in the chrome.
- **Don't** use red for anything but failure and destruction, or amber as "important".
- **Don't** put shadows on cards, lists, the sidebar or headers, or use glass and blur.
- **Don't** use uppercase or letter-spaced labels, or an eyebrow above a heading.
- **Don't** put a colored border thicker than 1 pt on a card, row or callout; a selected row is a fill, not a stripe.
- **Don't** use a radius other than 6 pt, 8 pt or a pill.
- **Don't** fix the height of anything that holds text.
- **Don't** show a peer or network as connected before the core reports it.
- **Don't** let a number, address, version or id wrap: cut it with an ellipsis or widen its column.
- **Don't** use a Unicode glyph or emoji as an icon, a bullet, a chevron or a check: draw it from the icon set.
- **Don't** put an entity's detail or editing in a centered modal when a drawer fits; keep modals for short
  confirmations and multi-step creation.
- **Don't** draw a sparkline or chart from anything but samples the core reported.
