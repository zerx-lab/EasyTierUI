---
description: No panic!/unreachable!/assert! family in non-test EasyTierUI Rust code
condition: '^(?![\s\S]*#\[\s*(?:cfg\(\s*(?:all\(\s*)?test\b|(?:\w+::)*test\s*\]))[\s\S]*?(?:^|\n)(?![ \t]*//)[^\n]*?\b(?:panic|unreachable|(?:debug_)?assert(?:_eq|_ne)?)!\s*[(\[{]'
scope: "tool:edit(src/**/*.rs), tool:write(src/**/*.rs), tool:edit(crates/**/src/**/*.rs), tool:write(crates/**/src/**/*.rs)"
interruptMode: tool-only
---
This change adds a panicking macro (`panic!`, `unreachable!`, `assert!`, `assert_eq!`, `assert_ne!` or a `debug_assert*!`) to production Rust code. EasyTierUI has no panic paths outside tests: a panic on the UI thread crashes the client, and the web build aborts.

- An "impossible" match arm: restructure so the compiler proves it (split the enum, match on the narrower type, return the value from the arm that produced it). If you cannot, return an error or log and skip.
- Invalid input from the user, a config file, an import link, easytier-core's output or easytier-web is never a panic: reject it with a typed error the UI can show.
- An internal invariant you want checked: return an error, or `log::error!` with the values involved and fall back to a safe state (stop the network, keep the last good config). `debug_assert!` panics in the debug builds `nsh run` uses, so it is not a substitute.

If this code is test-only (`#[cfg(test)]`, `#[test]`, `tests/`), re-issue the same change unchanged.
