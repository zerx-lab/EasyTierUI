---
description: EasyTier secrets never reach argv, environment variables, logs or derived Debug output
condition:
  - '--network-secret|ET_NETWORK_SECRET|--vpn-portal-private-key|ET_VPN_PORTAL_PRIVATE_KEY|--internal-auth-token|--webhook-secret|--oidc-client-secret'
  - '\b(?:trace|debug|info|warn|error|format|println|eprintln|write|writeln)!\([^;]*[,{(]\s*[\w.&*]*(?:network_secret|password|passwd|private_key|session_token|credential_secret|group_secret|config_server(?:_url)?)\b'
  - '#\[derive\([^)]*\bDebug\b[^)]*\)\][^{]*\bstruct\s+\w+[^{]*\{[^}]*\b(?:network_secret|password|private_key|session_token|secret)\s*:\s*(?:String|Option<String>|Vec<u8>|Box<str>)'
scope: "tool:edit(*.rs), tool:write(*.rs), tool:edit(*.yml), tool:write(*.yml), tool:edit(*.yaml), tool:write(*.yaml), tool:edit(*.service), tool:write(*.service)"
interruptMode: tool-only
---
This change may expose an EasyTier secret (network secret, easytier-web password, session cookie or internal / webhook / OIDC secret, private key, credential or group secret, config-server URL):

- **On a command line** (`--network-secret`, `--internal-auth-token`, `--webhook-secret`, `--oidc-client-secret`): every local user and process list sees argv. easytier-core secrets go into the profile's config file (atomic write, `0o600` / user-only ACL) with only `-c <file>` on the command line.
- **In the environment** (`ET_NETWORK_SECRET`): child processes inherit it. Only easytier-web secrets that the detected version reads from an environment variable (2.6.4: `OIDC_CLIENT_SECRET`) travel this way, set on that one child process (or in a `0600` env file / Docker secret on servers); otherwise the feature is unavailable on that version.
- **In a log line or formatted string**: logs are kept, attached to bug reports and shown in the diagnostics page. Log the profile name or instance id; if a value is needed, log `Secret`'s redacted `Display` (`***`).
- **In a derived `Debug`**: `{:?}` of the struct prints the secret wherever the struct is logged or shown in an error. Hold the field as the `Secret` newtype (redacted `Debug` / `Display`) or implement `Debug` by hand without it.

If the value is already the redacting `Secret` wrapper, or the identifier is not a secret, re-issue the change unchanged. See `rule://easytier-config-security`.
