---
description: Roles (node, console, control-plane host) and the UI ↔ EasyTier boundary on every platform — sidecar core / web processes, embedded library where forced, easytier-web REST, server deployment, version detection and upgrades
globs: ["src/**/*.rs", "crates/**/*.rs", "Cargo.toml", "nsh.json"]
---
# Roles and the core boundary

Two user decisions shape everything here:

1. **The EasyTier core is upgraded freely, independently of the UI; UI and core stay decoupled.**
2. **One client, every platform (desktop, mobile, web), all features, switchable roles.** The UI and its feature set are the same everywhere; what differs is which backend carries a feature on a platform.

## Roles

A role is something this installation does; roles combine, they are not one exclusive mode.

| Role | What it is | EasyTier program |
|---|---|---|
| **Node** (data plane) | a device inside the mesh: TUN, peers, routes | `easytier-core` |
| **Console** (control-plane client) | signs in to an easytier-web server and manages its machines, networks, credentials | easytier-web's REST API |
| **Control-plane host** | runs easytier-web: config server (UDP 22020 by default) + REST API (11211) + SQLite | `easytier-web` / `easytier-web-embed` |

Each node instance has a **config source**: *local* (a profile the user edits here, rendered to TOML) or *managed* (`--config-server <url> --machine-id <id>`: the control plane pushes configs and reads state). Managed instances are read-only in the local UI except for "leave management"; the machine id is generated once, persisted, and never reused across installations.

## Platform matrix

Physical limits, not product choices: a browser cannot create a TUN or listen on ports; iOS cannot spawn processes and upstream's iOS library has no TUN (loopback port forwarding only); phones cannot host a server reliably. Where a platform cannot do something locally, the same page acts on a remote target instead, and says so.

| | Node | Console | Control-plane host |
|---|---|---|---|
| macOS / Windows / Linux desktop | `sidecar`: official `easytier-core` process | `remote` | `controlplane`: official `easytier-web` process, opt-in |
| Android | `embedded` (no TUN until a host app supplies a VpnService fd: NSH 0.3 packaging cannot declare one) | `remote` | remote host only (console) + `deploy` artifacts |
| iOS | `embedded`, no TUN until a packet tunnel exists (open decision) | `remote` | remote host only (console) + `deploy` artifacts |
| Web (served on the server) | the server's own `easytier-core`, enrolled as a managed machine of the co-located easytier-web | `remote`, same origin | it *is* the dashboard of that easytier-web; process settings change through `deploy` artifacts |

## Backends

All of them implement traits in `src/core/` and speak the app's own domain types; nothing outside `src/core/` (and `src/deploy/` for artifact text) names an EasyTier crate, binary, flag, RPC method or JSON field.

| Backend | How it reaches EasyTier |
|---|---|
| `sidecar` | runs the official `easytier-core` release binary as a separate process (or an OS service it installed), configured by a TOML file the app renders, read through the same release's `easytier-cli -o json` on the loopback RPC portal |
| `controlplane` | runs the official `easytier-web` binary of the same release as a separate process with app-chosen flags, database and logs in the app's data directory; administered through its REST API on loopback |
| `embedded` | EasyTier's library through `crates/easytier-mobile` (UniFFI; git-pinned to v2.6.4, mobile targets only), because iOS cannot spawn processes and Android hands the VpnService TUN fd to in-process Rust; upgraded with app releases; `set_tun_fd` is exported for a host app's VPN service |
| `remote` | easytier-web's REST API (`/api/v1/…`) with its session cookie; on the web build, same origin |
| `deploy` | renders server deployment artifacts (below); runs nothing itself |

## Server deployment (`src/deploy/`)

On a Linux server the product is headless processes plus EasyTierUI's web build as the dashboard:

- **Processes**: `easytier-web` (control plane) and, when the server is also a node or shared relay, `easytier-core` enrolled to it with `--config-server udp://127.0.0.1:22020/<owner>`.
- **Dashboard**: `nsh package --target web` output served by a reverse proxy on the same origin as easytier-web's REST API (`/api/v1/…` proxied to `127.0.0.1:11211`), with HTTPS. easytier-web's own dashboard is off (`easytier-web`, or `easytier-web-embed --no-web`).
- **Artifacts**: a Docker Compose file using the official image `easytier/easytier` pinned by digest (the image holds every Linux binary in `/usr/local/bin`; the entrypoint is `easytier-core`, so the web service overrides it), or systemd units running a release binary verified as below. Generated from typed templates with the security defaults of `rule://easytier-config-security`; secrets go into an env file with mode `0600` or Docker secrets, never inline.

## Contracts, most stable first

1. **The TOML config file** (documented at <https://easytier.cn/en/guide/network/config-file.html>): rendered with a TOML serializer from typed structs, never by string formatting.
2. **Process flags**: `easytier-core` `-c <file>`, `--check-config`, `--rpc-portal 127.0.0.1:<port>`, `--rpc-portal-whitelist 127.0.0.1/32,::1/128`, `--disable-env-parsing`, `--no-tun`, `--config-server`, `--machine-id`, `--file-log-*`; `easytier-web` `--db`, `--config-server-port`, `--config-server-protocol`, `--api-server-addr`, `--api-server-port`, `--no-web`, `--disable-registration`, `--file-log-*`. Pass only what the detected version supports.
3. **`easytier-cli -p 127.0.0.1:<port> -o json <command>`** from the same release directory as the running core: versions always match.
4. **easytier-web's REST API**, versioned by the server's reported version; its login sends the password as the official frontend does (lower-case hex MD5 of the password, `frontend/src/modules/api.ts`), so it is only ever sent over HTTPS or loopback.
5. **The RPC portal directly** only if (3) proves too slow, behind the same adapter.

## Versions and upgrades

- Detect versions from `easytier-core --version` / `easytier-web --version` (`easytier-core 2.6.4-8428a89d`) at startup and after every swap, and from the server for `remote`; parse into `CoreVersion`, keep `MIN_CORE_VERSION` in one place, and gate features on the detected version — never on the version the UI was built or tested with.
- Below the minimum or unparsable: a clear UI state with the detected version and the way out, never a crash and never a silent fallback. Node and control plane on one machine come from the same release.
- Releases come from <https://github.com/EasyTier/EasyTier/releases> (`easytier-<os>-<arch>-v<version>.zip`: `easytier-core`, `easytier-cli`, `easytier-web`, `easytier-web-embed`). The GitHub API lists each asset's `digest` (`sha256:…`): download over HTTPS, verify before unpacking, unpack into `cores/<version>/` under the app's data directory, never into `PATH` or a world-writable location. Docker images are pinned by their registry digest.
- Install side by side; switch by pointing the active version at another directory; keep the previous one for rollback; stop old processes cleanly first. Upgrades are the user's decision; a user-chosen binary needs explicit confirmation with its detected version shown.

## Process management (`sidecar`, `controlplane`)

- Spawn binaries directly with an argument vector, never through a shell, never with user text interpolated into a command string. Strip inherited `ET_*` variables from every child (each flag also reads `ET_<NAME>`).
- Validate a rendered config with `easytier-core --check-config -c <file>` before starting or replacing an instance. 2.6.4 exits non-zero without printing why (the error is logged before its logger exists), so the app also parses the TOML itself to name a syntax error.
- Own each child: capture stdout/stderr into the log ring (redacted — the core prints its whole config, secrets included, at startup), observe exit status, restart with backoff only while the user wants it up, leave no orphan after the app quits unless it was installed as a service.
- One core instance per local profile; instance names derive from profile ids, not user text.
- Privileges: TUN needs root / Administrator / `CAP_NET_ADMIN` (Windows also `wintun.dll`). A TUN profile runs as the OS service `easytierui-<key>`, installed with `easytier-cli service --name <svc> install --core-path … --service-work-dir … --disable-autostart true -- -c <config> --rpc-portal 127.0.0.1:<port> --rpc-portal-whitelist … --disable-env-parsing --file-log-dir …`, then `service start` / `stop` / `uninstall`, each through the OS's elevation (`pkexec`; `osascript` with `quoted form of` argv; PowerShell `Start-Process -Verb RunAs`) after the user agreed. **A service never runs files the user can write**: the elevated step is the app's own executable re-run with a typed request (`--etui-privileged`, `core::sidecar::privileged`; no shell, values only as arguments), which copies `easytier-core`, `easytier-cli` and the rendered config into an administrator-only directory (macOS `/Library/Application Support/EasyTierUI/services/<key>/`, Linux `/var/lib/easytierui/<key>/`, Windows `%ProgramData%\EasyTierUI\<key>\` with an `icacls` ACL: Administrators and SYSTEM only, Users may only list and read `logs\`), refuses any directory on the way that is not owned by root / the administrator, writable by others or a symlink / reparse point, checks the copies' sha256 against the digests the app computed just before the prompt, and runs the copied CLI. Every service argument (core path, `-c`, working and log directories) points at those copies; later `service start` / `stop` run the copied CLI; a changed config or release is copied again through the same step; uninstall removes the directory. On Linux `easytier-cli` writes the unit's arguments unquoted; the fixed directory names contain no space, quote, `%` or `$`. The UI never runs elevated; `no_tun` profiles, managed instances and easytier-web need no privileges.

## Reading state

- Poll or subscribe off the main thread; one coherent snapshot per tick into the store (`NetworkState`: `Stopped`, `Starting`, `Running(snapshot)`, `Stopping`, `Failed { reason }`), per target (this device, each console server, the hosted control plane).
- Never show "connected" before the core reports it; lists show when they were read.
- Parse JSON tolerantly (`#[serde(default)]`, no `deny_unknown_fields`, `#[serde(other)]` for enums).

## When you change this boundary

Name the role, platform, contract and oldest version you rely on; add a parsing test with captured output from a real release (redacted) under `tests/`; if you can, run the real binaries locally — core with `--no-tun` on loopback, easytier-web with `--api-server-addr 127.0.0.1` and a temporary `--db` — and say which version you ran.
