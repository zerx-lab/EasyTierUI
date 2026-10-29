<img src="brand/logo.svg" width="96" height="96" alt="EasyTierUI">

# EasyTierUI

A client for [EasyTier](https://github.com/EasyTier/EasyTier) on macOS, Windows, Linux, iOS, Android and the web, written
in Rust on Native Shell: its UI is `src/lib.rs`, and `nsh.json` names the root component (`"root": "App"`).

What it does, by role (the same pages on every platform; where a platform cannot do something locally, the page says so
and offers the remote alternative):

- **Node** — this device in a mesh: create / join / import (TOML, share link, QR image) / export / edit / delete
  network profiles; start and stop them; node info, peers (direct or relayed, latency, loss, traffic), redacted logs;
  enroll in a control plane. Desktops run the official `easytier-core` (a TUN network as an OS service after an
  administrator prompt, a no-TUN network as the app's child); iOS / Android run EasyTier's library in process without
  TUN (`crates/easytier-mobile`, UniFFI).
- **Console** — several easytier-web servers: sign in, machines, their networks (create, edit, enable / disable,
  delete, peers), networks across machines, credentials.
- **Control-plane host** — run easytier-web on a desktop with first-run hardening (default passwords replaced, API on
  loopback, registration off); render server deployment files (Docker Compose or systemd, Caddy or nginx, firewall,
  README) on any platform.
- **EasyTier releases** (desktops) — list GitHub releases, download, verify the sha256 digest, install side by side,
  switch, roll back, or use a directory you installed yourself after confirming its version.

Prerequisites beyond NSH's: `protoc` on `PATH` for the iOS / Android builds (EasyTier's library compiles protobufs),
and for Android the NDK that NSH uses.

```sh
nsh run                      # this machine's backend; --target ios, android, web, macos, linux or windows
nsh check                    # cargo check of the crate, then type checks App.test.tsx
nsh test                     # cargo test, then App.test.tsx: the root tested from TypeScript, headless; --target to run it on a platform
nsh test nsh.json --trace    # headless: what the root mounts, and the time of each commit
nsh package --target web     # or android, macos, linux, windows; the icon is nsh.json's "icon" (brand/icon.png)
nsh package --target ios --icon brand/icon-ios.png   # iOS needs the opaque full-bleed icon
```

`nsh run` and `nsh package` mount the root on every platform, the browser included, without QuickJS: the app is Rust
and native code from start to end (in the browser, with NSH's page script). Rust edits rebuild and restart the app.

```sh
cargo run
```

This starts the root directly on the machine's native desktop backend: GTK 4 on Linux, AppKit on macOS, Win32 on
Windows. Install the backend's development libraries before building.

The `nsh` commands build the app's runners from the NSH crates `Cargo.toml` depends on: crates.io's at the version of
the `nsh` you run (keep the `nsh-*` dependencies at that version when you upgrade `nsh`), or an NSH checkout's by path.

## Adding a TSX entry

TSX screens can use the Rust components and modules, with hot reloading for the TSX. The crate's set,
`#[native_module("easytierui/native")]` in `src/lib.rs`, already exports `App`. Add an `App.tsx`:

```tsx
import { mount } from "nsh";
import { App } from "easytierui/native";

mount(App);
```

then run it with `nsh run App.tsx` (`tsconfig.json` is there for it and the test): a named TSX entry
runs on QuickJS, while `nsh run` alone keeps running the Rust root. The first `nsh build App.tsx` writes the TypeScript
declarations of `easytierui/native` into `.nsh/types`. Register more with `registry.register::<…>()` (components),
`registry.module::<…>()` and `registry.class::<…>()`.

NSH-generated files stay in `.nsh/`: runner sources in `.nsh/rust/` and default packages in `.nsh/out/`.
Cargo keeps its normal `target/` directory and respects your existing `CARGO_TARGET_DIR`.

## Project rules

- `AGENTS.md`: architecture (one client for every platform with switchable roles — node, console, control-plane host —
  and EasyTier's programs as separate, freely upgradable components), security, Rust safety (no panics, no `unsafe`),
  conventions and checks. `CLAUDE.md` imports it.
- `PRODUCT.md` and `DESIGN.md` (with `.impeccable/design.json`): who the app is for and its design system;
  `theme.json` holds the design tokens (`nsh.json` `"theme"`).
- `Cargo.toml` `[lints]` and `clippy.toml`: the panic-free, unsafe-free lint policy;
  `cargo clippy --all-targets -- -D warnings` must be clean.
- `.omp/`: agent configuration — `RULES.md`, rules that stop unsafe patterns as they are written (`omp ttsr list`),
  `/verify`, a guard hook and LSP settings.
