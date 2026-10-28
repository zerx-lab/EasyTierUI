---
description: Only src/core/embedded/ may name EasyTier's crates; the desktop UI never links them
condition:
  - '\b(?:use\s+)?(?:easytier|easytier_core|easytier_proto)::'
  - '(?:^|\n)[ \t]*easytier(?:-core|-proto|-ffi|-android-jni|-ios)?\s*='
scope: "tool:edit(*.rs), tool:write(*.rs), tool:edit(Cargo.toml), tool:write(Cargo.toml), tool:edit(**/Cargo.toml), tool:write(**/Cargo.toml)"
interruptMode: tool-only
---
This change names or adds EasyTier's crates. The user chose a decoupled, freely upgradable core (`rule://easytier-core-boundary`):

- **Desktops** never link EasyTier: they run the official `easytier-core` release as a separate process and read it through `easytier-cli -o json`, so the user can upgrade the core without a new app.
- **Mobile** links EasyTier's library only in `src/core/embedded/` (or its own crate), because iOS cannot spawn processes and Android passes the VpnService TUN fd in process. The dependency is target-gated (`[target.'cfg(any(target_os = "android", target_os = "ios"))'.dependencies]`), from git pinned to a rev (crates.io's `easytier` is stale at 2.0.3; `easytier`, `easytier-core`, `easytier-proto` from the same rev), and reviewed for LGPL-3.0 obligations. Adding it is the user's decision: ask first.
- **Everything else** (pages, components, store, profile) uses the app's domain types from `src/core/`, so a renamed EasyTier field is fixed in one adapter.

If this edit is inside `src/core/embedded/` (or the embedded crate's own manifest) and the user approved the dependency, re-issue it unchanged.
