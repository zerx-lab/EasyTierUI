---
description: A hosted or deployed easytier-web keeps its REST API on loopback and never auto-creates accounts by default
condition:
  - '--(?:api|web)-server-addr(?:"\s*,\s*"|[ =]|\s*\n\s*-\s*["'']?)(?:0\.0\.0\.0|::|\[::\])'
  - '--allow-auto-create-user'
scope: "tool:edit(*.rs), tool:write(*.rs), tool:edit(*.yml), tool:write(*.yml), tool:edit(*.yaml), tool:write(*.yaml), tool:edit(*.service), tool:write(*.service)"
interruptMode: tool-only
---
This change opens a hosted or deployed easytier-web:

- **`--api-server-addr` / `--web-server-addr` on all interfaces**: a fresh easytier-web database has `admin` / `admin` and `user` / `user` accounts, and the REST API manages every enrolled machine. Bind `127.0.0.1` and publish only through an HTTPS reverse proxy that serves the EasyTierUI web build on the same origin, after the default passwords have been replaced.
- **`--allow-auto-create-user`**: any machine that presents an unknown account name in its config-server URL creates that account. It stays off unless the user turns it on in a setting that explains this.

If this edit is the code path that applies the user's explicit, explained opt-in (with the default passwords already replaced for a public bind), re-issue it unchanged. See `rule://easytier-config-security` "Hosting the control plane".
