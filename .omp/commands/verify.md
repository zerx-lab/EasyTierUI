---
description: Run EasyTierUI's checks for the current changes (fmt, strict clippy, tests, nsh check/test)
argument-hint: "[scope]"
---
Verify the current changes in EasyTierUI. Scope: $ARGUMENTS (empty = everything changed; `git status --short` when the directory is a git repository, otherwise the files this session touched).

Run these yourself in the main session, one at a time, and stop at the first failure to fix it:

1. `cargo +nightly fmt --all` (`.rustfmt.toml` uses nightly options).
2. `cargo clippy --all-targets -- -D warnings` — must be clean: it enforces the panic-free, unsafe-free lint table in `Cargo.toml` (`clippy.toml` lets test code panic). Never silence a finding with `allow`; fix the code (`rule://rust-error-handling`).
3. `cargo test`.
4. `nsh check`, then `nsh test` (headless: builds the runner with the crate and runs `*.test.tsx`).
5. Changes to roles and the boundary (`src/core/`, `src/deploy/`, profile rendering): the parsing and rendering tests with captured output from a real release. When release binaries are available (unzipped, sha256-verified, e.g. `/tmp/easytier-2.6.4/easytier-macos-aarch64`): `cargo test --lib -- --ignored` (the sidecar runs a `--no-tun` core on loopback and reads it back; the control plane starts easytier-web on a temporary `--db`, hardens it and restarts it) and `cargo test --test app smoke -- --ignored` (through the UI: adopt the release in Settings and run a network without TUN; add a loopback easytier-web in the console, sign in, list an enrolled core). `ET_RELEASE_DIR` / `EASYTIER_DIR` point elsewhere. Never with `sudo`, never with a TUN, never against a real server; the OS-service path is unit-tested with a recording elevator only.
6. Other targets: `cargo check --target wasm32-unknown-unknown --lib`, `--target aarch64-apple-ios --lib`, `--target aarch64-linux-android --lib` (NDK `CC_aarch64_linux_android` / `AR_…` / linker set; `protoc` on `PATH`), and Windows via `x86_64-pc-windows-gnu` with a `zig cc` wrapper as `CC_x86_64_pc_windows_gnu`; `cargo test -p easytier-mobile` and `cargo clippy -p easytier-mobile --all-targets -- -D warnings` when the bridge changed.
7. Capability states: the headless tests of each changed page on the platforms that cannot run its feature locally.
8. `omp ttsr scan` when `.omp/rules/` changed: every match must be intended.
9. UI changes: GUI checks (`nsh run`, `nsh test --target <platform>` with screenshots) only if the user asked for them; otherwise list what the user should look at (both appearances, a large text size, a phone, a desktop window and the web build).

Report exactly what ran, its result, and what could not run on this host (other OSes, devices, a real TUN).
