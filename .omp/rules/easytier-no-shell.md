---
description: easytier-core / easytier-cli are spawned directly, never through a shell
condition: 'Command::new\(\s*"(?:/bin/)?(?:sh|bash|zsh|fish|dash|cmd|cmd\.exe|powershell|powershell\.exe|pwsh)"'
scope: "tool:edit(*.rs), tool:write(*.rs)"
interruptMode: always
---
This change runs a command through a shell. EasyTierUI passes user-controlled values to the core (profile names, peer URLs, file paths, hostnames); through a shell, any of them can become a command, run with the core's privileges.

Spawn the binary itself with an argument vector: `Command::new(&core_path).arg("-c").arg(&config_path)`. Resolve the binary to an absolute path inside the app's managed core directory (or the user-confirmed path), never via `PATH` lookup of a bare name. Platform service managers (`launchctl`, `systemctl`, `sc.exe`) are spawned the same way, with fixed arguments.

Re-issue the change without the shell.
