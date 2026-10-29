# easytier-mobile

EasyTier's library behind a small, safe Rust API and a [UniFFI](https://mozilla.github.io/uniffi-rs/) surface. It is
what EasyTierUI links on **iOS and Android**, where the app cannot start the official `easytier-core` binary as a
separate process (iOS cannot spawn processes at all). The desktops never compile this crate or EasyTier: they run the
official release binaries (see `AGENTS.md`, "Roles and the core boundary").

EasyTier is pinned to one commit, the v2.6.4 release (`8428a89d2dabc94c97d370ec607c6ca142473626`), from git (crates.io's
`easytier` is stale at 2.0.3). The mobile app is upgraded with the app, not independently.

## API

Plain Rust functions (the app's `src/core/embedded/` calls them directly), each also exported through UniFFI. All
block; call them off the UI thread. Instances are named by the caller; the name is the instance's identity here.

| Function | Does |
|---|---|
| `check_config(config_toml)` | Validates an EasyTier TOML config. The error names the line, never quotes the file. |
| `start_network(name, config_toml, no_tun)` | Runs the config as instance `name`. `no_tun` forces `flags.no_tun`. |
| `start_managed_network(name, config_server, machine_id, hostname, no_tun)` | Enrolls to a config server (`udp://host:22020/<account>`); the server pushes the networks. |
| `stop_network(name)` / `stop_all_networks()` | Stops and waits for the instance's thread. |
| `list_networks()` | The names that were started and not stopped. |
| `running_info(name)` | `RunningInfo { running, error_msg, info_json }`; `info_json` is EasyTier's `NetworkInstanceRunningInfo` (node, peers, routes, events) in the schema easytier-web's `networks/info` returns. |
| `set_tun_fd(name, fd)` | Hands the host's VPN-service descriptor to an instance started without `no_tun`. Zero or less detaches. |
| `collect_logs()` | The newest captured log lines (info and above), oldest first, secrets masked. |
| `version()` | EasyTier's version string. |

Errors are `MobileError` (`InvalidName`, `ConfigRejected`, `AlreadyRunning`, `NotFound`, `StartFailed`, `TunFd`,
`Internal`). Messages never contain the config or the config-server URL: secret values found in the config
(`network_secret`, keys, tokens) and the enrollment URL are registered for redaction and masked as `***` in errors and
logs. A panic inside EasyTier is caught and returned as `Internal`.

`info_json` is JSON, not typed Records, on purpose: the proto is large and changes between releases; the app parses
the few fields it shows in one adapter (`src/core/embedded/map.rs`).

## Platform limit: no TUN in this build

NSH 0.3 packages a Rust app as an APK / IPA and cannot add an Android `VpnService` or an iOS Network Extension to it.
Without one there is no TUN descriptor, so **instances in the app run `no_tun`**: subnet proxy, SOCKS5 and port
forwarding work, but the device's own traffic is not routed through the mesh. A host app that does have a VPN service
(or packet tunnel) starts the instance without `no_tun` and passes the descriptor to `set_tun_fd`; the library takes
ownership of it. `set_tun_fd` is wired through EasyTier's mobile code path but **has not been exercised on a device**.

## Licence: LGPL-3.0

EasyTier is licensed under the **GNU LGPL v3.0**. This crate links it statically, so a binary that contains it (the
app's APK / IPA) is a combined work. Distributing such a build carries LGPL obligations — among them offering the
corresponding source of EasyTier and of the application's object files / a way to relink with a modified EasyTier,
which is difficult to satisfy on app stores that forbid user-replaceable libraries. **This needs a legal review before
any distribution; it has not had one.** Adding this dependency was the project owner's decision (`AGENTS.md`).

## Features chosen

`easytier` is built with `websocket`, `smoltcp`, `socks5`, `quic`, `wireguard`, `tun`, `magic-dns` (default features
off). `tun` and `magic-dns` are required by EasyTier's own mobile code (`setup_nic_ctx_for_mobile` calls both; without
`magic-dns` the library does not compile for iOS / Android) and create no device by themselves. `wireguard` brings
`ring`, which provides the `aes-gcm` peer encryption that is EasyTier's default. Left out: `kcp`, `faketcp` and
`zstd` (KCP proxy, raw-socket fake TCP and compression: not needed for a first mobile build, and not tried here),
`aes-gcm` / `openssl-crypto` (a second AES-GCM implementation / a vendored OpenSSL build).

## Building

* Needs `protoc` on `PATH` (EasyTier's build script compiles its protobufs).
* Android: the NDK, with `CC_<target>`, `AR_<target>` and `CARGO_TARGET_<TARGET>_LINKER` pointing at its
  `toolchains/llvm/prebuilt/<host>/bin/<triple><api>-clang` and `llvm-ar`.
* iOS: Xcode's toolchain.
* `Cargo.toml` caps `libc` below 0.2.191 on iOS: 0.2.191 limited `RTAX_MAX` to macOS, which breaks `netdev` 0.31.0
  (EasyTier's `natpmp` dependency) on iOS. Drop the cap when `netdev` is fixed.

## Kotlin / Swift bindings for a host app

The crate builds as `staticlib` and `cdylib` as well as `lib`. The bindings come from UniFFI's library mode (commands
below are the documented UniFFI flow; they were not run in this repository's checks):

```sh
# 1. Build the library for a target
cargo build -p easytier-mobile --release --target aarch64-linux-android     # -> libeasytier_mobile.so
cargo build -p easytier-mobile --release --target aarch64-apple-ios         # -> libeasytier_mobile.a

# 2. Generate the bindings from the built library
cargo run -p easytier-mobile --features bindgen --bin uniffi-bindgen -- \
  generate --library target/aarch64-linux-android/release/libeasytier_mobile.so \
  --language kotlin --out-dir out/kotlin
cargo run -p easytier-mobile --features bindgen --bin uniffi-bindgen -- \
  generate --library target/aarch64-apple-ios/release/libeasytier_mobile.a \
  --language swift --out-dir out/swift
```

Ship the generated sources with the matching library in the host app (jniLibs for Android, an XCFramework for iOS).

## Lints

This crate has its own `[lints]` table: the app's panic-free clippy policy, with `unsafe_code = "deny"` instead of
`forbid` because UniFFI's generated scaffolding is `unsafe extern "C"` code. Hand-written `unsafe` is denied without a
`// SAFETY:` comment; the crate has none.
