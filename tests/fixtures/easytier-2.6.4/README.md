# EasyTier 2.6.4 captured fixtures

- Release: `v2.6.4` (GitHub `releases/latest` on 2026-10-29; commit `8428a89d2dabc94c97d370ec607c6ca142473626`, tagged 2026-05-12).
- Binaries: `easytier-macos-aarch64-v2.6.4.zip`, sha256 `4be1882d1aa36d31c1d6ba0596f2cf8a097e371f8da124212324b2e0f8df7e4b`
  (matched the GitHub API `digest` field). Captured on macOS 27.2 arm64, unprivileged, `--no-tun`, loopback only.
- `--version` prints `easytier-core 2.6.4-8428a89d` (core, cli, web, web-embed all report `<name> 2.6.4-8428a89d`).

## Commands run (B = unzipped `easytier-macos-aarch64/`)

Help / version: `B/easytier-core --version|--help`, `B/easytier-cli --help`, `B/easytier-web --help`, `B/easytier-web-embed --help`.

Config check (`check-config-ok.txt` = `node-a.toml`, `check-config-bad.txt` = `bad.toml`, `check-config-unknown-key.txt` = `unknown-key.toml`; output is empty on success, last line is the exit code):
`B/easytier-core --check-config -c <file> --disable-env-parsing`

Mesh (two nodes, listeners tcp loopback, B has `[[peer]]` to A):
```
B/easytier-core -c node-a.toml --no-tun --rpc-portal 127.0.0.1:41100 --rpc-portal-whitelist 127.0.0.1/32,::1/128 --disable-env-parsing
B/easytier-core -c node-b.toml --no-tun --rpc-portal 127.0.0.1:41101 --rpc-portal-whitelist 127.0.0.1/32,::1/128 --disable-env-parsing
B/easytier-cli -p 127.0.0.1:41100 -o json connector add tcp://127.0.0.1:41011
B/easytier-cli -p 127.0.0.1:41100 -o json <node|peer|route|connector|port-forward list|peer-center|whitelist show|logger get|credential list|proxy|stats show|mapped-listener|acl|vpn-portal|stun>
```
`cli-<name>.json` = the output of the matching command (`cli-port-forward.json` = `port-forward list`, `cli-whitelist.json` = `whitelist show`,
`cli-logger.json` = `logger get`, `cli-credential-list.json` = `credential list`). A (`node-a.toml`) also has `[[proxy_network]]` and `[[port_forward]]`.
`cli-peer-verbose.json` = `B/easytier-cli -p 127.0.0.1:41100 -v -o json peer` from a later run of the same two nodes (peer ids differ from
`cli-peer.json`): `-v` prints the raw `PeerRoutePair`s, with byte counters to the byte and latency in microseconds, where `-o json peer`
alone prints them humanized to three significant digits (`"1.47 kB"`); the node's `inst_id` is `...0003`.
Note: `rpc_portal` is NOT a TOML key and `no_tun` belongs under `[flags]`; both are passed on the command line.

Config server + REST:
```
B/easytier-web-embed --db <tmp>/et.db --api-server-addr 127.0.0.1 --api-server-port 41211 --config-server-port 41220 --web-server-addr 127.0.0.1 --no-web
B/easytier-core --config-server udp://127.0.0.1:41220/admin --machine-id <uuid> --no-tun --hostname example-host-m --rpc-portal 127.0.0.1:41102 --rpc-portal-whitelist 127.0.0.1/32,::1/128 --disable-env-parsing
curl -c jar -H 'content-type: application/json' -d '{"username":"admin","password":"<md5 hex of admin>"}' http://127.0.0.1:41211/api/v1/auth/login
```
`web-*.json` are response bodies (cookie-jar authenticated; session cookie name `id`); `web-statuses.tsv` lists HTTP status and the request for each
`web-*.json` produced by the REST session (`web-*-request.json` are request bodies; `web-machines.json` was captured right after enrollment, before any network was pushed;
`web-login.json`, `web-login-bad.json`, `web-summary.json`, `web-sessions.json` were captured by hand with the same cookie jar and are reproduced verbatim).
Pushed network: instance `00000000-0000-0000-0000-0000000000aa` to machine `00000000-0000-0000-0000-000000000001`.
`web-help.txt` is `easytier-web --help`; `web-embed-help.txt` is `easytier-web-embed --help` (adds `--no-web`, `--web-server-*`, `--api-host`).

## Redacted / replaced
- Public IPv4/IPv6 from STUN (`203.0.113.10`, `203.0.113.11`, `2001:db8::1`), LAN interface addresses (`192.0.2.1`, `192.0.2.2` / `2001:db8::1|2` encoded as the u32 parts the JSON uses), the LAN source IP seen in logs.
- Machine id -> `00000000-0000-0000-0000-000000000001`; node instance ids -> `...0002` (cli) / `...00aa` (REST pushed network); the web-side heartbeat `inst_id` -> parts `0,0,0,2`.
- Hostnames are `example-host-*`; network secret is `example-secret-do-not-use`; session cookie values are not stored.
- Peer ids (`id`, `node_id`) are random per run and left unredacted; `last_update_time`, ports and latencies are real run values, not stable.
- The passwords in `web-pw-*` requests are MD5 hex of `user` (public seeded default) and of the throwaway `example-new-pass`.

## Peers and credentials through `proxy-rpc` (added by the console client)
A second capture on loopback (api 42211, config server 42220): the web-managed node (`--machine-id 00000000-0000-0000-0000-000000000077`, instance `00000000-0000-0000-0000-0000000000cc`, `no_tun`, listener `tcp://127.0.0.1:42012`) plus one standalone `easytier-core --no-tun -p tcp://127.0.0.1:42012` peer, so a direct peer is present.
- `web-info-peers.json`: `GET /machines/{mid}/networks/info/{inst}` with a direct peer (routes, peers with conns, `peer_route_pairs`); IPs, hostnames and the network name redacted as above.
- `web-list-peer.json`: `PeerManageRpcService/ListPeer`.
- `web-credential-generate.json` / `web-credential-list.json`: `CredentialManageRpcService/GenerateCredential` (secret replaced by `example-secret-do-not-use`) and `ListCredentials`.
- Facts these captures settled: an instance is named in a `proxy-rpc` payload as `{"instance":{"selector":{"Id":{part1..part4}}}}` (protobuf JSON of the oneof). A misspelled selector (`inst_id`, as in `web-proxy-rpc.json`) is accepted and silently falls back to the core's only instance. An unknown instance answers 500 `{"message":"RPC Error: ExecutionError(Instance not found or API service not available)"}`. `GenerateCredential` needs every non-optional field (`groups`, `allow_relay`, `allowed_proxy_cidrs`, `ttl_seconds`, `reusable`), else 500 `missing field`. `RevokeCredential` of an unknown id answers 200 `{"success":false}`. Logout answers `Set-Cookie: id=; Path=/; Max-Age=0; Expires=...`.

## Every other `proxy-rpc` service (added for the full console)
A third capture on loopback (api 43211, config server 43220): the web-managed node (`--machine-id ...0077`, instance
`...00cc`, `no_tun`, listeners `tcp|udp://127.0.0.1:43012`, mapped listener `tcp://127.0.0.1:43013`, proxy network
`10.250.0.0/24`, WireGuard VPN portal on 43030 for `10.14.14.0/24`, port forward `127.0.0.1:43050 -> 10.144.145.2:22`,
KCP and QUIC proxy on), one `example-net` peer (`10.144.145.2`) and one `example-other` node connected to the managed
node's listener (so it relays a foreign network). `web-rpc-<method>.json` = the response body of
`POST /machines/{mid}/proxy-rpc` with `{"instance": <selector>}`; `web-rpc-statuses.tsv` lists status, method and file.
Redacted as above (STUN public IPs, LAN interface addresses and IPv6 parts); the VPN portal's client `PrivateKey` is
`example-private-key-do-not-use`.

- Payload shapes the core demands (else 500 `MalformatRpcPacket("json error: missing field ...")`):
  `ListForeignNetwork` needs `include_trusted_keys`; `PeerCenterManageRpcService/GetGlobalPeerMap` needs `digest` (0 =
  everything) and takes no instance; `LoggerRpcService/*` takes no instance (it sets the core process's level);
  `PatchConfig` needs all six repeated fields (`port_forwards`, `proxy_networks`, `routes`, `exit_nodes`,
  `mapped_listeners`, `connectors`), even when empty.
- `TcpProxyRpcService/ListTcpProxyEntry` cannot be called through `proxy-rpc` in 2.6.4: the core registers it per
  transport (a service domain) and the web's client sends none, so it always answers 500 `InvalidServiceKey`
  (`web-rpc-tcp-proxy-unavailable.json`).
- `ShowNodeInfo`'s `node_info.config` and `GetConfig` carry the network secret in clear (`DumpRoute` too: it is a
  debug dump with the secret, not captured); `GetVpnPortalInfo`'s `client_config` carries a WireGuard private key.
- `PatchConfig` changes the running instance only: `GetConfig` and `GET /networks/config/{inst}` (which ask the
  machine) show it, but easytier-web's saved config (`user_running_network_configs`) does not, so a reconnect or
  restart pushes the saved config again. Requests: `web-rpc-patch-config-request.json` (add one of each kind, a new
  hostname, whitelist ports) and `web-rpc-patch-config-remove-request.json` (`REMOVE` by value, `CLEAR`).
- **A `UrlPatch` whose `url` does not parse panics the whole easytier-core** (`common.rs:274`, `RelativeUrlWithoutBase`;
  seen with `CLEAR` and an empty url): the process exits and every request to the machine then ends in
  `RPC Error: Timeout(Elapsed(()))`. `CLEAR` must carry a parseable url (any, it is ignored), and every url must be
  checked before it is sent.
- A machine that does not answer: 500 `{"message":"RPC Error: Timeout(Elapsed(()))"}` from `proxy-rpc`
  (`web-rpc-proxy-rpc-timeout.json`), 504 with `Timeout(Elapsed(()))` from the REST network routes.

## Embedded running info (added by the mobile bridge)
- `embedded-running-info.json`: EasyTier's `NetworkInstanceRunningInfo` as the library reports it in process (the same
  schema as easytier-web's `networks/info`), captured from two `--no-tun` nodes on loopback through a throwaway
  easytier-web; addresses and hostnames redacted as above. `src/core/embedded/map.rs` maps it to `NodeSnapshot`.

## Every editable network setting (added for the full network editors)
- `web-parse-config-full-request.json`: a TOML config that sets every field the official GUI edits (all `[flags]`
  switches, `ipv6_public_addr_auto`, `mapped_listeners`, `routes`, `[vpn_portal_config]`, `[[proxy_network]]` with
  `mapped_cidr`, `[[port_forward]]`, `instance_recv_bps_limit`, `relay_network_whitelist`, and `[acl.acl_v1]` with a
  chain, a rule and a declared group); `B/easytier-core --check-config -c <file>` accepts it (exit 0). Secrets are
  `example-secret-do-not-use` / `example-group-secret`; addresses are documentation ranges.
- `web-parse-config-full.json`: what `POST /api/v1/parse-config` (no session; easytier-web on loopback, temporary
  `--db`) returned for it: the `NetworkConfig` JSON easytier-web stores. Facts it settled: ACL enums are numbers
  (`chain_type` 1/2/3, `action` 1 allow / 2 drop, `protocol` 1 TCP … 5 any), `enable_ipv6 = false` reads as
  `disable_ipv6: true`, `accept_dns` as `enable_magic_dns`, `aes-gcm` (the default) as `encryption_algorithm: null`, a
  SOCKS5 proxy keeps only its port (`generate-config` writes it back as `socks5://0.0.0.0:<port>`), and the VPN portal
  as `vpn_portal_listen_port` plus `vpn_portal_client_network_addr` / `_len`. `POST /api/v1/generate-config` turns that
  JSON back into the same TOML keys.

## `--check-config` on failure
2.6.4 exits non-zero without printing why: the error is logged before its logger exists, so stdout and stderr stay
empty (`check-config-bad.txt` holds only the exit code). The sidecar parses the TOML itself to name a syntax error.
