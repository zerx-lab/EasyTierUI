---
description: Network profiles, EasyTier config files, secrets, imports, safe defaults, hosting easytier-web and server deployment artifacts — what may be stored, logged, passed, exposed or applied
globs: ["src/**/*.rs", "crates/**/*.rs", "tests/**", "**/*.toml", "**/*.yml", "**/*.yaml", "**/*.service"]
---
# Configuring EasyTier safely

## Secrets

Secrets: `network_secret`, easytier-web passwords and session tokens, `secure_mode.local_private_key`, VPN-portal (WireGuard) private keys, ACL group secrets, credential secrets, and the config-server URL (its path names the easytier-web user).

- Hold them in a `Secret` newtype whose `Debug` / `Display` print `***` and that never derives `Serialize` into anything but the core's config file; compare in constant time where it matters.
- Never in logs, error messages shown to users, panics, analytics (there is none), URLs other than the config-server one, clipboard without a user action, test fixtures with real values, or committed files. Fixtures use obvious placeholders (`example-secret-do-not-use`).
- Never on a command line: `--network-secret` and friends are visible in every process list. Write them into the config file instead.
- At rest: the OS keychain where the platform has one (Keychain, Credential Manager, Secret Service, Android Keystore); a local profile keeps its network secret under `profile/<id>/network-secret` and its ACL group secrets as one entry under `profile/<id>/acl-group-secrets` (the stored profile holds group names only), and deleting the profile deletes both; the rendered config file that must contain them is written atomically (temp file + rename) with owner-only permissions (`0o600` on Unix, a user-only ACL on Windows) inside the app's data directory and removed when the profile is deleted.
- Generate network secrets with a CSPRNG, at least 128 bits, URL-safe. EasyTier derives the network digest with SipHash, not a password KDF: warn when a user types a short or common secret.

## Defaults

- Encryption on, `aes-gcm`. `xor` is obfuscation, never labelled as encryption. Disabling encryption or changing the algorithm is an advanced setting with a warning that every peer must match.
- Listeners only where the user asked; DHCP on by default for joiners; `no_tun` offered when privileges are missing.
- `enable_exit_node`, `exit_nodes`, `proxy_network` (subnet proxy), `port_forward`, `socks5_proxy`, `vpn_portal_config` (WireGuard on every interface), `proxy_forward_by_system`, `relay_all_peer_rpc`, `private_mode=false` and wide `relay_network_whitelist` change what other peers can reach: each is off by default and explained where it is turned on.
- No public shared-node addresses are hard-coded; the user enters or picks one, and the UI says that a shared node relays traffic.

## The management surfaces

- RPC portal: always `--rpc-portal 127.0.0.1:<port>` with an explicit loopback address (a bare port binds every interface in current EasyTier) and `--rpc-portal-whitelist 127.0.0.1/32,::1/128`. It has no authentication: never bind it elsewhere, never widen the whitelist, never forward it.
- easytier-web (`remote`): HTTPS / WSS for anything but loopback (the login sends an unsalted MD5 of the password, which protects nothing in transit); never store the password, keep the session cookie as a secret; show which server and account the app is signed in to.
- Managed nodes: the config-server URL path names the easytier-web account the machine enrolls into, so it is an enrollment credential — keep it with the secrets and choose owner names that are not guessable (never `admin` / `user`).
- Launch the core with `--disable-env-parsing`: configs the app renders are final, and an imported config must not expand `${VAR}` from the user's environment.

## Hosting the control plane (`controlplane` on the desktops, `deploy` on servers)

easytier-web's defaults are open, so every hosted or deployed control plane starts from these, and each departure is a visible, explained choice:

- **Default accounts**: easytier-web's first database migration seeds `admin` / `admin` and `user` / `user`, and no later migration removes them (checked on 2.6.4: both sign in on a fresh database). Before the REST API is reachable from anything but loopback, sign in to both on loopback and replace their passwords (`PUT /api/v1/auth/password`) with CSPRNG passwords or ones the user sets and confirms; keep them as secrets. A deployment artifact that cannot do this itself starts the API on loopback only and says how to finish setup.
- **REST API and dashboard**: `--api-server-addr 127.0.0.1` (the default is `0.0.0.0`); public access only through an HTTPS reverse proxy that also serves the EasyTierUI web build on the same origin. Never `--web-server-addr 0.0.0.0`.
- **Accounts**: the hosted easytier-web starts with `--disable-registration` from its very first start and only the loopback-only owner-registration window (the owner account, below) runs with it open; deployments pass it once the owner exists. `--allow-auto-create-user` stays off unless the user turns it on knowing that any machine presenting an unknown name then creates an account.
- **Owner account**: machines enroll under a CSPRNG owner account (`owner-` and 10 base32 characters, CSPRNG password), never under `admin` / `user`. The desktop host registers it through `GET /api/v1/auth/captcha` and `POST /api/v1/auth/register` on loopback with a person typing the captcha, keeps the name and password as secrets (`host/owner-name`, `host/owner-password`) and restarts with registration disabled again; the enrollment URL built from it is a secret. A default password that is refused while no stored password signs in is an error (someone else got there first), not "replaced"; hardening is complete only when the keychain holds both passwords.
- **Config server**: easytier-web binds its config-server listener on every interface with no address option; tell the user which port and protocol become reachable, and generate the matching firewall rule in deployment artifacts.
- **Database and logs**: `--db` and `--file-log-dir` inside the app's data directory (or a named volume on servers), owner-only permissions; the database holds password hashes, device inventories and network configs.
- **Secrets of easytier-web itself** (`--internal-auth-token`, `--webhook-secret`, `--oidc-client-secret`): never on the command line. Pass a secret through the environment variable the detected version reads (2.6.4 reads `OIDC_CLIENT_SECRET`); a feature whose secret the detected version only takes as a flag is unavailable on that version. `--internal-auth-token` bypasses session authentication entirely: off unless the user configures an integration that needs it.
- **Deployment artifacts**: images pinned by digest, release binaries verified by sha256; secrets in a `0600` env file or Docker secrets; only the config-server port and HTTPS published; the RPC portal never published; the generated text is shown to the user before it is copied or written.

## Imports and share links

TOML files, URLs, QR codes and pasted text are untrusted input:

1. Parse into the typed profile; reject unknown schemes, malformed URLs and CIDRs, out-of-range ports and prefixes, over-long names — with a message naming the field.
2. Show what applying it will do before it runs: network name, peers, listeners (especially on `0.0.0.0` / `::`), exit node use, subnets proxied, port forwards, encryption state.
3. Never auto-start an imported profile.

## Peer and listener URLs

Schemes: `tcp`, `udp`, `ws`, `wss`, `quic`, `wg`, `faketcp` for listeners and peers; `http`, `https`, `txt`, `srv` are connector-only (peer discovery). Ports are 11010 plus the scheme's offset: tcp / udp 11010, ws / wg 11011, quic / wss 11012, faketcp 11013 (a bare listener port `11010` expands to that set); a peer URL without a port uses the same, except ws (80) and wss (443). Parse with `url::Url`, keep the scheme list in one place, and show the effective port when the user omits it.
