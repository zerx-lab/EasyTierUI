<div align="center">

<img src="brand/logo.svg" width="96" height="96" alt="EasyTierUI">

# EasyTierUI

A native, cross-platform client for the [EasyTier](https://github.com/EasyTier/EasyTier) mesh VPN.

macOS · Windows · Linux · iOS · Android · Web

**English** · [简体中文](README.zh-CN.md)

</div>

## Overview

EasyTierUI deploys, configures and runs EasyTier networks quickly, correctly and safely. One Rust app on
[Native Shell](https://nsh.dev), rendered with each platform's own UI — the same pages and features everywhere,
organized as switchable roles:

|Role|What it does|
|---|---|
|**Node**|Join, create, import (TOML / link / QR) and run networks on this device; peers, traffic and redacted logs|
|**Console**|Manage machines, networks and credentials across one or more easytier-web servers|
|**Control-plane host**|Run a hardened easytier-web locally, or render Docker Compose / systemd deployment files|

Desktops run the official `easytier-core` / `easytier-web` binaries as separate processes, downloaded from GitHub
releases with sha256 verification and switchable / rollback-able versions. Mobile embeds EasyTier's library.

## Security by default

- Encryption on, CSPRNG network secrets, secrets kept in the OS keychain and never in logs or argv
- RPC portal bound to loopback; risky features (exit node, subnet proxy, SOCKS5, …) off until enabled
- Hosted easytier-web replaces default passwords and disables registration before leaving loopback
- Imports are previewed and never auto-started; the UI itself never runs elevated

## Build

Requires Rust (stable) and the `nsh` CLI; `protoc` for iOS / Android.

```sh
nsh run                       # run on this machine (--target ios|android|web|macos|linux|windows)
nsh test                      # cargo test + headless UI tests
nsh package --target macos    # package (web, android, linux, windows, ios)
```

## Contributing

Read [AGENTS.md](AGENTS.md) (architecture and rules), [DESIGN.md](DESIGN.md) and [PRODUCT.md](PRODUCT.md).
Code is panic-free and `unsafe`-free; `cargo clippy --all-targets -- -D warnings` must be clean.
