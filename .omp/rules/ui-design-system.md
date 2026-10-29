---
description: Building or changing EasyTierUI screens and components — what to read, the named design rules, states, platforms, copy and i18n
globs: ["src/**/*.rs", "**/*.tsx", "theme.json", "nsh.json", "DESIGN.md"]
---
# EasyTierUI's design system in practice

Read before any UI change: `PRODUCT.md` (who, what job), `DESIGN.md` (tokens and named rules; `theme.json` implements its tokens), NSH's Design Guides and Coding Guides (<https://nsh.dev/docs/design-guides>, <https://nsh.dev/docs/coding-guides>), and the nearest existing screen or component. Signatures come from the source and `.nsh/types`, never from React / React Native / shadcn analogies.

## Before writing code, name

1. the user's task on this screen and its primary action;
2. the state owner (store signal, resource), which target it belongs to (this device, a console server, the hosted control plane) and which EasyTier state it reflects;
3. the press, keyboard, focus and screen-reader path;
4. the empty, loading, error, offline, permission-denied (no privileges for TUN), "program missing / unsupported version", "managed by a control plane" (read-only) and "not available on this platform" (Capability Notice with the remote alternative) states;
5. the theme tokens used and any intentional exception;
6. the test that fails if the behavior regresses (`nsh test` with `App.test.tsx`-style steps, or `nsh_testing::Headless`).

## The rules that most often break

- **Token, not hex**: colors through `theme()` / token classes only; new roles go into `theme.json` (light and dark) and DESIGN.md together.
- **One Signal**: `primary` marks the primary action, focus, selection and links — nothing decorative.
- **Red is reserved** for errors and destructive actions; `success` / `warning` only for their states; status always has words or an icon besides its color ("The Lamp Needs Words").
- **Machine voice**: IPs, CIDRs, peer URLs, peer ids, versions, latency, byte counts in the monospace font, tabular figures; human text in the system UI font; never mixed in one string.
- **Quiet sections**: section labels are sentence case, `text_xs`, `muted_foreground`, no uppercase or letter spacing (CJK has no case); no eyebrow above a heading.
- **Flat at rest**: cards and lists have no shadow; one shadow recipe for the popup family.
- **Scale, not literals**: 4-point spacing steps, the type scale, `radius` / `radius-control`; touch targets at least 44 × 44 pt.
- **Native chrome**: the system header, back gesture and alerts on iOS / Android (`NavStack`, `AlertDialog`), a sidebar shell on the desktops and web, the page's `header` slot on desktop and web.
- **Same pages everywhere**: one page per feature for every platform, organized by role (Node, Console, Host) under the Target Switcher; capability comes from `core`, never from `cfg!` in a page; a feature this platform cannot run locally shows a Capability Notice, never disappears. Only the user's work mode (`Store::roles`) decides which roles the shell lists, never the platform.
- **Motion** only to explain a change (150 ms state, 200 ms overlay); `reduce_motion()` turns it into an instant change; the connecting pulse stops too.

## Copy and languages

- Every string the user sees goes through `src/i18n/` with zh-CN and en entries; no literal UI text in components. Write each language natively instead of translating word order; let context carry context (no "Network name of the network").
- Use EasyTier's terms consistently: network (网络), node (节点), peer (对等节点), virtual IP (虚拟 IP), shared node (共享节点), relay (中继), P2P (直连), core (内核).
- Buttons name their action ("Join network" / "加入网络"); errors say what happened and what to do; destructive confirmations name the object ("Delete profile “home”?").
- Layouts hold longer translations and larger text: no fixed-width labels, rows wrap or stack.

## Verifying

`nsh check` and the headless tests prove behavior; looking at the screens (`nsh run`, screenshots, a real device) is the user's unless they ask the agent to do it — then use `nsh test <entry> --target <platform>` with steps on stdin and check both appearances and a large text size. Report what was not looked at.
