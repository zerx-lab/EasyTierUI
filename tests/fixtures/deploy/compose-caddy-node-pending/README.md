# EasyTier server for vpn.example.com

Rendered by EasyTierUI. Nothing was applied anywhere: read each file, then follow the steps in order.

> These files are rendered with hardening **pending**: the API stays closed at the proxy and the config server stays closed in the firewall until you flip the switch (step "Flip the switch").

## Files

- `docker-compose.yml`: easytier-web, Caddy and an optional node, images pinned by digest
- `.env`: the node's enrollment URL, mode 0600
- `Caddyfile`: the reverse proxy; `/api/` stays closed until hardening is done
- `firewall-ufw.sh`, `firewall-firewalld.sh`: pick the one your server uses

## What is reachable from the internet

| Port | Reachable | Why |
|---|---|---|
| 80/tcp, 443/tcp | yes | the reverse proxy: dashboard and, after hardening, `/api/` |
| 22020/udp | **after hardening** | the config server; it binds every interface and has no bind-address option, so the firewall is its only gate |
| 11211/tcp | no | the REST API: on the compose network and on the host's loopback only |
| 15888/tcp | no | the node's RPC portal, bound to 127.0.0.1 |

## Steps

1. **DNS.** Point an A / AAAA record of `vpn.example.com` at this server. Ports 80 and 443 must be free.

2. **Copy the files.** Put them in one directory on the server, for example `/opt/easytier-deploy`, then `chmod 600 .env`.

3. **Publish the dashboard.** On your machine run `nsh package --target web` in the EasyTierUI project and copy the output directory's contents to `/srv/easytierui` on the server (the proxy serves it read-only; it must be readable by the proxy's user).

4. **Firewall.** Run `sudo sh firewall-ufw.sh` (ufw) or `sudo sh firewall-firewalld.sh` (firewalld). Before hardening they open 80/tcp and 443/tcp only; the config-server rule is commented out. Keep your SSH rule: neither script enables the firewall.

5. **Start the control plane.** `docker compose up -d web proxy` (not the node yet). The REST API is now on the server's loopback (`127.0.0.1:11211`) and on the compose network; `Caddyfile` answers 403 on `/api/`.

6. **Check that the API is closed.** `curl -i https://vpn.example.com/api/v1/summary` must answer `403` from the proxy, and the dashboard must load at `https://vpn.example.com/`. Do not continue if `/api/` answers anything else: the seeded accounts `admin`/`admin` and `user`/`user` are still active.

7. **Harden through an SSH tunnel.** On your machine: `ssh -L 11211:127.0.0.1:11211 <user>@vpn.example.com`. In EasyTierUI choose *Add server*, enter `http://127.0.0.1:11211`, sign in as `admin` / `admin` and change the password, then do the same for `user` / `user` (or leave it unused with a new password). Registration is still open, so also register the account `relay-7f3k2q` (the owner the server's node enrolls under) with a strong, generated password.

8. **Flip the switch.** Only after both seeded passwords are replaced. In EasyTierUI mark hardening as done and render again, or edit by hand: in `Caddyfile` replace the closed `/api/` block by the commented one; in `docker-compose.yml` uncomment `--disable-registration` and drop the `127.0.0.1:` prefix from the config-server port; in `firewall-ufw.sh` / `firewall-firewalld.sh` uncomment the config-server rule. Then `docker compose up -d`, reload the proxy if needed and run the firewall script again. Start the node afterwards.

9. **Start the node.** `docker compose up -d`. It enrolls to `127.0.0.1` as `relay-7f3k2q` with the URL in `.env`; it needs `/dev/net/tun` and `NET_ADMIN`, which the compose file grants to this service only.

10. **Verify.** `curl -i https://vpn.example.com/api/v1/auth/check_login_status` now answers `401` from easytier-web (not `403` from the proxy), registration answers `403`, and in EasyTierUI *Add server* `https://vpn.example.com` shows the machines that enroll. From outside, only 80/tcp, 443/tcp and 22020/udp answer.

## Notes

- The seeded accounts `admin`/`admin` and `user`/`user` exist in every fresh easytier-web database. The API must not be reachable from the internet until both are replaced; that is why the proxy answers 403 on `/api/` first.
- The enrollment URL (`ET_CONFIG_SERVER`) attaches a machine to an account: it is kept in a mode-0600 file, never on a command line.
- The Content-Security-Policy header is a starting point for the web build; if the dashboard stops loading, check the browser console and widen it deliberately.
