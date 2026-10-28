---
description: How EasyTierUI Rust code fails without panicking (errors, Option, indexing, text offsets, arithmetic, locks, processes, async, invariants)
globs: ["**/*.rs"]
---
# Failing without panicking

Sources: Google's Rust style ("some codebases avoid panics entirely, and require even broken invariants to be reported via `Result`" — EasyTierUI is one), Microsoft's Pragmatic Rust Guidelines (M-PANIC-IS-STOP, M-LOG-NOT-PRINT), the Rust API Guidelines (C-FAILURE, C-VALIDATE, C-GOOD-ERR, C-DTOR-FAIL), NSH's own rules. Where they conflict with AGENTS.md, AGENTS.md wins.

## Why

EasyTierUI runs on UIKit / AppKit / Win32 / GTK / Android main threads through NSH and in the browser with `panic = "abort"`. It reads untrusted data all day: what users type and paste, imported configs and share links, `easytier-core`'s output, easytier-web's responses, files other versions wrote. A panic closes the user's window onto a running VPN; an ignored error leaves them believing a network is up when it is not.

## Choose, in order

1. **Unrepresentable**: newtypes with fallible constructors (`NetworkName`, `VirtualIpv4`, `PeerUrl`, `Port`, `CoreVersion`: `TryFrom<&str>`), enums over flag combinations, non-empty types. Validate once at the boundary (form submit, import, core output parsing), then pass the validated type.
2. **Propagate**: `Result<T, E>` with a `thiserror` enum per module (one variant per situation, `#[from]` for wrapped sources, `Error + Send + Sync + 'static`). `Option` → `ok_or(E::…)?`. Document `# Errors` on public fallible functions. No `anyhow` in app code: the UI needs to match on what went wrong to say what to do.
3. **Degrade at a boundary**: per event, per row, per poll of the core: `let Some(peer) = peers.get(i) else { log::warn!(…); return; };`. A failure the user is waiting on becomes a visible, recoverable state in the UI (`Failed { reason }` with a retry), not only a log line.
4. **Correct defaults**: `unwrap_or`, `unwrap_or_default`, `map_or` only when the default is a right answer (an absent optional field), never a silencer.
5. **Never drop a `Result` that matters**: no `let _ = …` and no `.ok();` on writes, process spawns, kills, channel sends that carry state. Handle it or log it with context.

## Specific hazards

- **Indexing**: `.get()` / `.get_mut()` / iterators / `first()` / `split_first()`; `Cargo.toml` denies `clippy::indexing_slicing`.
- **Text offsets**: hostnames, peer names and network names are user text, often CJK. `&s[a..b]`, `String::insert_str`, `truncate`, `drain`, `split_off`, `str::split_at` panic off a char boundary (`clippy.toml` disallows them). Use `s.get(a..b)`, `split_at_checked`, `char_indices`, `chars().take(n)`; truncate for display by chars, not bytes.
- **Numbers**: parse with `str::parse::<u16>()` / `u8` and handle the error (ports, prefix lengths, MTU); `checked_*` / `saturating_*` on counters, byte totals, durations and offsets (`clippy::arithmetic_side_effects`); `u32::try_from(x)` instead of a truncating `as`.
- **Network values**: `std::net::Ipv4Addr` / `IpAddr` / an `ipnet`-style CIDR type and `url::Url` from their `FromStr`; never split strings by hand. Validate CIDR prefix ranges and URL schemes against EasyTier's list.
- **Locks / RefCell**: no `lock().unwrap()`; `lock().unwrap_or_else(PoisonError::into_inner)` only when the data stays consistent. Never hold a `RefCell` borrow or a lock across a callback into UI code, an `await`, or a signal write that re-runs bindings; use `try_borrow_mut` where re-entry is possible.
- **Processes** (`easytier-core`, `easytier-cli`): spawn the binary directly with an argument vector, never through a shell; always handle spawn failure, non-zero exit, a signal exit, unparsable output and timeouts; `kill` / `wait` results are checked; a crashed core is a UI state, never a UI crash.
- **Parsing core output**: `serde` with `#[serde(default)]` for fields that may be missing in other core versions and no `deny_unknown_fields`; an unknown enum value maps to an `Unknown` variant (`#[serde(other)]`), not an error that hides the whole list.
- **Async**: a task's error is delivered to the state that started it; a result that arrives after its page, profile or revision changed is dropped. Spawned work under an owner ends with it.
- **Drop** never panics (C-DTOR-FAIL); stopping a sidecar in `Drop` logs failures.
- **Invariants**: return an error or `log::error!` with the values and fall back to a safe state (stop, keep the last good config). No `assert!` / `debug_assert!` outside tests.

## Checks

`cargo clippy --all-targets -- -D warnings` enforces `Cargo.toml`'s `[lints]` (panic, unwrap, indexing, string slicing, arithmetic, printing, dropped results, unexplained `allow`s, `forbid(unsafe_code)`); `clippy.toml` lets `#[test]` functions and `#[cfg(test)]` modules panic. The TTSR rules in `.omp/rules/` stop the same constructs while they are being written.
