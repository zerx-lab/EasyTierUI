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
5. Changes to roles and the boundary (`src/core/`, `src/deploy/`, profile rendering): the parsing and rendering tests with captured output from a real release. When release binaries are available: the core with `--no-tun --rpc-portal 127.0.0.1:<port> --rpc-portal-whitelist 127.0.0.1/32,::1/128 --disable-env-parsing -c <rendered file>`, read back with `easytier-cli -p 127.0.0.1:<port> -o json node`; easytier-web with `--api-server-addr 127.0.0.1 --db <temporary file>`, driven through its REST API (first-run hardening replaces the seeded `admin` / `user` passwords). Never with `sudo`, never with a TUN, never against a real server.
6. Capability states: the headless tests of each changed page on the platforms that cannot run its feature locally.
7. `omp ttsr scan` when `.omp/rules/` changed: every match must be intended.
8. UI changes: GUI checks (`nsh run`, `nsh test --target <platform>` with screenshots) only if the user asked for them; otherwise list what the user should look at (both appearances, a large text size, a phone, a desktop window and the web build).

Report exactly what ran, its result, and what could not run on this host (other OSes, devices, a real TUN).
