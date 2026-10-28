---
description: EasyTierUI logs through the log facade, never print!/eprintln!
condition: '^(?![\s\S]*#\[\s*(?:cfg\(\s*(?:all\(\s*)?test\b|(?:\w+::)*test\s*\]))[\s\S]*?(?:^|\n)(?![ \t]*//)[^\n]*?\be?print(?:ln)?!\s*\('
scope: "tool:edit(src/**/*.rs), tool:write(src/**/*.rs), tool:edit(crates/**/src/**/*.rs), tool:write(crates/**/src/**/*.rs)"
interruptMode: tool-only
---
EasyTierUI only logs through the `log` facade (`log::info!`, `log::warn!`, `log::debug!`); `src/main.rs` installs the logger. `Cargo.toml` denies `clippy::print_stdout` / `print_stderr`. A desktop app has no terminal to print to, and the mobile and web builds drop stdout.

Logs never contain secrets: network secrets, easytier-web passwords and session tokens, private keys, credential secrets, full config-server URLs. Log the profile or instance name instead.

If this edit is test code, keep it; otherwise switch to `log`.
