---
description: No unsafe code in EasyTierUI (the crate forbids unsafe_code)
condition: '(?:^|\n)(?![ \t]*//)[^\n]*?(?:\bunsafe\b|\ballow\(\s*unsafe_code|\bstatic\s+mut\b)'
scope: "tool:edit(src/**/*.rs), tool:write(src/**/*.rs), tool:edit(tests/**/*.rs), tool:write(tests/**/*.rs), tool:edit(crates/easytier-mobile/**/*.rs), tool:write(crates/easytier-mobile/**/*.rs), tool:edit(crates/desktop-shell/src/lib.rs), tool:write(crates/desktop-shell/src/lib.rs), tool:edit(crates/desktop-shell/src/error.rs), tool:write(crates/desktop-shell/src/error.rs), tool:edit(crates/desktop-shell/src/shell.rs), tool:write(crates/desktop-shell/src/shell.rs), tool:edit(crates/desktop-shell/src/tray.rs), tool:write(crates/desktop-shell/src/tray.rs), tool:edit(crates/desktop-shell/src/instance.rs), tool:write(crates/desktop-shell/src/instance.rs), tool:edit(crates/desktop-shell/src/login.rs), tool:write(crates/desktop-shell/src/login.rs), tool:edit(crates/desktop-shell/src/linux.rs), tool:write(crates/desktop-shell/src/linux.rs)"
interruptMode: always
---
EasyTierUI's crate sets `unsafe_code = "forbid"` (`Cargo.toml` `[lints.rust]`): no `unsafe` blocks, `unsafe fn` / `unsafe impl`, `#[unsafe(no_mangle)]` / `#[unsafe(export_name)]`, `static mut`, and no loosening of that lint. NSH does every FFI call for the app (UIKit, AppKit, JNI, Win32, GTK, the web), and the EasyTier core runs as its own process on the desktops, so the app has no reason to touch raw memory.

- Global or shared state: per-instance state passed down (`Copy` signal handles, `Rc` / `Arc`), `std::sync::LazyLock` for process-wide immutable data, atomics for counters, `thread_local!` with `Cell` / `RefCell` for main-thread-only state.
- An NSH API that needs `unsafe` (`#[class]` expands to `unsafe impl Trace`) or a platform bridge (Android VpnService / JNI, an iOS Network Extension, a C ABI to EasyTier's library) is a design decision for the user: stop and ask. If approved, it lives in a separate crate with its own lint policy, every block carries a `// SAFETY:` comment, and the main crate keeps `forbid`. The one approved so far: `crates/desktop-shell/src/macos.rs` and `windows.rs` (AppKit's window delegate, Win32's window subclass), outside this rule's scope; that crate's `deny` keeps its other modules safe.

Re-issue the change without `unsafe`.
