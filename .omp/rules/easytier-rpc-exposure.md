---
description: The EasyTier RPC portal binds to loopback only, with a loopback whitelist
condition:
  - '--rpc-portal"\s*,\s*(?:"0\.0\.0\.0|"\[::\]|"::|"\d{1,5}")'
  - '(?:--rpc-portal-whitelist|rpc_portal_whitelist|ET_RPC_PORTAL_WHITELIST)[^\n]*?(?:0\.0\.0\.0/0|::/0)'
scope: "tool:edit(*.rs), tool:write(*.rs), tool:edit(*.toml), tool:write(*.toml)"
interruptMode: always
---
This change exposes easytier-core's management RPC portal. The portal has no authentication: whoever reaches it can read the network, change peers, credentials, ACLs and logging. Current EasyTier binds a bare port (`--rpc-portal 15888`) to every interface, despite its help text.

Always pass an explicit loopback address and a loopback whitelist:

```rust
.args(["--rpc-portal", &format!("127.0.0.1:{port}")])
.args(["--rpc-portal-whitelist", "127.0.0.1/32,::1/128"])
```

Never bind it to `0.0.0.0` / `::`, never widen the whitelist, never forward the port. Remote management goes through easytier-web (`remote` backend), which has accounts. Re-issue the change with a loopback binding.
