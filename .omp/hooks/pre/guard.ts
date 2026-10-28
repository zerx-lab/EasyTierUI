/**
 * Project guard for EasyTierUI agents.
 *
 * Blocks tool calls that would bypass the commit checks, rewrite shared history,
 * break AGENTS.md's "Parallel agents" rule (subagents never run builds, tests or
 * the app), change the user's machine with elevated privileges (sudo, system
 * services, the network stack), or deploy to a server (SSH, containers) without
 * the user's explicit consent.
 */
import type { ExtensionAPI } from "@oh-my-pi/pi-coding-agent";

const HEAVY_CARGO = /\bcargo\s+(?:\+\S+\s+)?(?:build|b|test|t|clippy|run|r|bench|nextest|doc|xtask)\b/;
const HEAVY_NSH = /(?:^|[\s/;&|(])nsh\s+(?:run|test|package|build|dev)\b/;
const PRIVILEGED = /^(?:\w+=\S*\s+)*(?:\S*\/)?(?:sudo|doas|pkexec|runas|gsudo)(?:\s|$)/;
const SYSTEM_CHANGE =
	/\beasytier-cli\b[^|;&]*\bservice\s+(?:install|uninstall|start|stop|restart)\b|\b(?:systemctl|launchctl)\s+(?:enable|disable|start|stop|restart|load|unload|bootstrap|bootout|kickstart)\b|\bsc(?:\.exe)?\s+(?:create|delete|start|stop|config)\b|\bnetsh\b|\broute\s+(?:add|delete|change|flush)\b|\bip\s+(?:route|link|addr|address|rule)\s+(?:add|del|delete|change|replace|set|flush)\b|\bifconfig\s+\S+\s+(?:up|down|inet|alias|-alias|destroy|create)\b/;
const DEPLOY =
	/^(?:\w+=\S*\s+)*(?:\S*\/)?(?:ssh|scp|sftp)\s|\bdocker\b[^|;&]*\s(?:run|up|start|create)\b|\bdocker\s+stack\s+deploy\b|\bpodman\b[^|;&]*\s(?:run|up|start|create)\b|\bkubectl\s+(?:apply|create|run|replace|patch)\b/;

export type Verdict = { block: true; reason: string } | { confirm: string; reason: string } | undefined;

/** Splits a shell line into simple commands (good enough for policy checks, not a parser). */
function commands(line: string): string[] {
	return line
		.split(/&&|\|\||;|\n|\|/)
		.map(part => part.trim())
		.filter(Boolean);
}

function gitArgs(cmd: string): string[] | undefined {
	const words = cmd.split(/\s+/);
	const at = words.findIndex(w => w === "git" || w.endsWith("/git"));
	if (at < 0) return undefined;
	// Skip global options such as `-C dir` or `-c key=value`.
	let i = at + 1;
	while (i < words.length && words[i]!.startsWith("-")) {
		i += words[i] === "-C" || words[i] === "-c" ? 2 : 1;
	}
	return words.slice(i);
}

export function checkBash(line: string, isSubagent: boolean): Verdict {
	for (const cmd of commands(line)) {
		const args = gitArgs(cmd);
		if (args) {
			const [sub, ...rest] = args;
			if (sub === "commit" && rest.some(a => a === "--no-verify" || a === "-n" || /^-[a-zA-Z]*n[a-zA-Z]*$/.test(a))) {
				return { block: true, reason: "Do not bypass the commit checks (--no-verify). Fix what they report, or ask the user to commit manually." };
			}
			if (sub === "push" && rest.some(a => a === "--force" || a === "-f") && !rest.some(a => a.startsWith("--force-with-lease"))) {
				return { block: true, reason: "Use --force-with-lease, never a bare --force." };
			}
			if (sub === "update-index" && rest.some(a => a === "--assume-unchanged" || a === "--skip-worktree")) {
				return { block: true, reason: "Do not hide tracked files from git." };
			}
		}
		if (isSubagent && (HEAVY_CARGO.test(cmd) || HEAVY_NSH.test(cmd))) {
			return {
				block: true,
				reason:
					'AGENTS.md "Parallel agents": subagents never run cargo build/test/clippy/run or nsh run/test/package/build. ' +
					"Use at most `cargo check`, and report which checks the main session should run.",
			};
		}
		if (PRIVILEGED.test(cmd) || SYSTEM_CHANGE.test(cmd) || DEPLOY.test(cmd)) {
			const reason =
				'AGENTS.md "Parallel agents": agents do not elevate privileges, install or control system services, change routes, ' +
				"interfaces or the firewall, or deploy to a server or container on the user's behalf. Test the core with --no-tun " +
				"on loopback and easytier-web on loopback with a temporary --db; deployment artifacts are generated and shown.";
			if (isSubagent) return { block: true, reason };
			return { confirm: `Allow a privileged, system-changing or deploying command?\n\n${cmd}`, reason };
		}
	}
	return undefined;
}

export default function guard(pi: ExtensionAPI): void {
	pi.on("tool_call", async (event, ctx) => {
		if (event.toolName !== "bash") return;
		const command = event.input.command;
		if (typeof command !== "string") return;
		const verdict = checkBash(command, ctx.agent?.kind === "sub");
		if (!verdict) return;
		if ("block" in verdict) return verdict;
		if (ctx.hasUI && (await ctx.ui.confirm("EasyTierUI guard", verdict.confirm))) return;
		return { block: true, reason: verdict.reason };
	});
}
