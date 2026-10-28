---
description: No new .unwrap()/.expect() in non-test EasyTierUI Rust code
condition: '^(?![\s\S]*#\[\s*(?:cfg\(\s*(?:all\(\s*)?test\b|(?:\w+::)*test\s*\]))[\s\S]*?(?:^|\n)(?![ \t]*//)[^\n]*?\.(?:unwrap\(\s*\)|expect\(|unwrap_err\(\s*\)|expect_err\()'
scope: "tool:edit(src/**/*.rs), tool:write(src/**/*.rs), tool:edit(crates/**/src/**/*.rs), tool:write(crates/**/src/**/*.rs)"
interruptMode: tool-only
---
This change adds `.unwrap()` / `.expect(..)` to production Rust code. EasyTierUI runs on the main thread of desktop, mobile and web hosts and parses input from users, config files, the EasyTier core's output and the network: a panic there crashes the client, and the user loses the window onto their running network. `Cargo.toml` denies `clippy::unwrap_used` / `expect_used`, so this would not pass `cargo clippy` either.

Replace it, in this order of preference:

1. Make the state unrepresentable: typed ids and newtypes with fallible constructors (`TryFrom`), `let … else` on the value you already matched.
2. Propagate: `?` with the module's `thiserror` error enum, `ok_or(Error::…)`, `ok_or_else`.
3. Degrade visibly at a boundary: `let Some(x) = … else { log::warn!(…); return; };` — skip one event, one row, one poll of the core instead of aborting the app, and show a recoverable error in the UI when the user is waiting on it.
4. Defaults only when they are correct values: `unwrap_or`, `unwrap_or_default`, `unwrap_or_else`.
5. Locks: `lock().unwrap_or_else(PoisonError::into_inner)` when the guarded data stays consistent; otherwise propagate.

If this code is test-only (inside `#[cfg(test)]` / `#[test]`, or under `tests/`), re-issue the same change unchanged. An invariant that truly cannot fail still needs proof in the type system, not a comment; when that is impossible, ask the user before keeping it.
