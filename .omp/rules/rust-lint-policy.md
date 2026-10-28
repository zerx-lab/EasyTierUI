---
description: The panic-free / unsafe-free lint table in Cargo.toml and clippy.toml is never loosened
condition: '(?:^|\n)[ \t]*(?:unsafe_code\s*=\s*"(?:allow|warn|deny)"|(?:unused_must_use|unwrap_used|expect_used|panic|todo|unimplemented|unreachable|dbg_macro|get_unwrap|indexing_slicing|string_slice|unwrap_in_result|panic_in_result_fn|fallible_impl_from|exit|mem_forget|arithmetic_side_effects|print_stdout|print_stderr|let_underscore_must_use|allow_attributes_without_reason)\s*=\s*"allow"|allow-dbg-in-tests\s*=\s*true)'
scope: "tool:edit(Cargo.toml), tool:write(Cargo.toml), tool:edit(**/Cargo.toml), tool:write(**/Cargo.toml), tool:edit(clippy.toml), tool:write(clippy.toml)"
interruptMode: always
---
This change loosens EasyTierUI's safety lint policy (`Cargo.toml` `[lints]`, `clippy.toml`). The policy is the compile-time half of "no panics, no unsafe" (AGENTS.md "Rust safety"); the TTSR rules are the other half. `unsafe_code` stays `forbid`; panic, print and dropped-result lints stay `deny` / `warn` (`warn` is an error under `cargo clippy -- -D warnings`); test code is already exempt through `clippy.toml`.

Fix the code that trips the lint instead. Changing the policy is the user's decision: stop and ask, naming the lint and the reason. A new crate in this repository copies the same `[lints]` table.
