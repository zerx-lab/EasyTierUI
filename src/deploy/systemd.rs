//! systemd runtime: the install script (download, verify, install release binaries), `easytier-web.service`
//! and the optional `easytier-core.service`.

use super::{
    env::node_env,
    plan::{DeploymentPlan, Hardening, RPC_PORTAL_PORT, SystemdRelease},
    template::{Artifact, Language, fill, sh_quote},
};

/// The install script's name.
pub const INSTALL_SCRIPT: &str = "install-easytier.sh";
/// The web unit's name.
pub const WEB_UNIT: &str = "easytier-web.service";
/// The node unit's name.
pub const CORE_UNIT: &str = "easytier-core.service";
/// The node's env file name as rendered; installed as [`NODE_ENV_INSTALLED_AS`].
pub const NODE_ENV_FILE: &str = "easytier-node.env";
/// Where the node's env file belongs on the server (root-owned, mode 0600).
pub const NODE_ENV_INSTALLED_AS: &str = "/etc/easytier/node.env";
/// The unprivileged account both services run as; the install script creates it.
pub const SERVICE_USER: &str = "easytier";

const INSTALL_TEMPLATE: &str = r#"#!/usr/bin/env bash
# EasyTierUI deployment: install EasyTier {{version}} (linux-{{arch}}) into /opt/easytier/{{version}}.
# Rendered, not applied. Read it, then run it as root on the server. It downloads one zip, checks its sha256
# BEFORE unzipping, installs three binaries and creates the unprivileged "{{user}}" account. It starts no
# service, writes no unit and opens no port.
set -euo pipefail

VERSION={{version_q}}
ASSET_URL={{url_q}}
ASSET_SHA256={{sha_q}}
ASSET_DIR={{dir_q}}
SERVICE_USER={{user_q}}
DEST="/opt/easytier/${VERSION}"

if [ "$(id -u)" -ne 0 ]; then
  echo "run this script as root" >&2
  exit 1
fi
for tool in curl unzip sha256sum install useradd getent; do
  command -v "$tool" >/dev/null 2>&1 || { echo "missing tool: $tool" >&2; exit 1; }
done

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location \
  --output "$work/release.zip" "$ASSET_URL"

# The checksum comes from the plan, not from the download. A zip that does not match is never opened.
if ! printf '%s  %s\n' "$ASSET_SHA256" "$work/release.zip" | sha256sum --check --status; then
  echo "sha256 mismatch for $ASSET_URL; nothing was installed" >&2
  exit 1
fi

unzip -q "$work/release.zip" -d "$work/unpacked"
for bin in easytier-web easytier-core easytier-cli; do
  if [ ! -f "$work/unpacked/$ASSET_DIR/$bin" ]; then
    echo "release zip has no $ASSET_DIR/$bin; nothing was installed" >&2
    exit 1
  fi
done

install -d -m 0755 -o root -g root /opt/easytier "$DEST"
for bin in easytier-web easytier-core easytier-cli; do
  install -m 0755 -o root -g root "$work/unpacked/$ASSET_DIR/$bin" "$DEST/$bin"
done

if ! getent passwd "$SERVICE_USER" >/dev/null; then
  useradd --system --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin "$SERVICE_USER"
fi
install -d -m 0755 -o root -g root /etc/easytier

echo "installed EasyTier ${VERSION} to ${DEST}"
echo "next: copy the .service files to /etc/systemd/system/ (README.md)"
"#;

const WEB_TEMPLATE: &str = r#"# EasyTierUI deployment: easytier-web (config server + REST API) {{version}}.
# Rendered, not applied. Install to /etc/systemd/system/{{unit}}, then `systemctl daemon-reload`.
#
# The REST API listens on 127.0.0.1 only; the reverse proxy is the one way in. The config server has no bind-address
# option: {{config_port}}/{{transport}} listens on every interface, so the firewall decides who reaches it.
[Unit]
Description=EasyTier web: config server and REST API
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User={{user}}
Group={{user}}
{{switch}}
ExecStart={{exec}}
Restart=on-failure
RestartSec=5

# State: the SQLite database under /var/lib/{{state}}, private to {{user}}.
StateDirectory={{state}}
StateDirectoryMode=0700
WorkingDirectory=/var/lib/{{state}}
ReadWritePaths=/var/lib/{{state}}
UMask=0077

# Hardening.
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
ProtectClock=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
RestrictNamespaces=yes
RestrictRealtime=yes
RestrictSUIDSGID=yes
LockPersonality=yes
SystemCallArchitectures=native
CapabilityBoundingSet=
AmbientCapabilities=

[Install]
WantedBy=multi-user.target
"#;

const CORE_TEMPLATE: &str = r#"# EasyTierUI deployment: this server as an EasyTier node, enrolled to the local easytier-web {{version}}.
# Rendered, not applied. Install to /etc/systemd/system/{{unit}}. Start it only after the owner account
# "{{owner}}" exists (README.md): until then the config server refuses the enrollment.
[Unit]
Description=EasyTier node (enrolled to the local easytier-web)
After=network-online.target {{web_unit}}
Wants=network-online.target

[Service]
Type=simple
User={{user}}
Group={{user}}
# ET_CONFIG_SERVER (the enrollment URL) comes from this root-owned mode-0600 file, never from the command line.
EnvironmentFile={{env_path}}
# The RPC portal is bound to the loopback explicitly: the core's default binds every interface.
ExecStart={{exec}}
Restart=on-failure
RestartSec=5

StateDirectory=easytier-core
StateDirectoryMode=0700
WorkingDirectory=/var/lib/easytier-core
UMask=0077

# A TUN device needs CAP_NET_ADMIN (routes, addresses) and /dev/net/tun, nothing else.
AmbientCapabilities=CAP_NET_ADMIN CAP_NET_RAW
CapabilityBoundingSet=CAP_NET_ADMIN CAP_NET_RAW
DevicePolicy=closed
DeviceAllow=/dev/net/tun rw

# Hardening.
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX AF_NETLINK
RestrictNamespaces=yes
RestrictRealtime=yes
RestrictSUIDSGID=yes
LockPersonality=yes
SystemCallArchitectures=native

[Install]
WantedBy=multi-user.target
"#;

/// The versioned binary directory the units and the script agree on.
fn bin_dir(release: &SystemdRelease) -> String {
    format!("/opt/easytier/{}", release.version)
}

/// Renders the install script.
pub(super) fn install_script(release: &SystemdRelease) -> Artifact {
    let text = fill(
        INSTALL_TEMPLATE,
        &[
            ("version", release.version.as_str()),
            ("arch", release.arch.asset_word()),
            ("user", SERVICE_USER),
            ("version_q", &sh_quote(release.version.as_str())),
            ("url_q", &sh_quote(release.download_url.as_str())),
            ("sha_q", &sh_quote(release.sha256.as_str())),
            ("dir_q", &sh_quote(&release.asset_dir())),
            ("user_q", &sh_quote(SERVICE_USER)),
        ],
    );
    Artifact::new(INSTALL_SCRIPT, Some(0o755), Language::Sh, text)
}

/// Renders `easytier-web.service`.
pub(super) fn web_unit(plan: &DeploymentPlan, release: &SystemdRelease) -> Artifact {
    let state = plan.paths.state_dir.as_str();
    let mut exec = format!(
        "{}/easytier-web --db /var/lib/{state}/et.db --api-server-addr 127.0.0.1 --api-server-port {} \
         --config-server-port {} --config-server-protocol {}",
        bin_dir(release),
        plan.api_port,
        plan.config_port,
        plan.config_protocol.scheme()
    );
    let switch = if plan.hardening == Hardening::Done {
        exec.push_str(" --disable-registration");
        "# HARDENING SWITCH (done): --disable-registration is on; the account list is closed."
    } else {
        "# HARDENING SWITCH: registration stays open until the owner account exists (README.md). After first-run\n\
         # hardening append ` --disable-registration` to ExecStart (or re-render with hardening done), then\n\
         # `systemctl daemon-reload && systemctl restart easytier-web`."
    };
    let text = fill(
        WEB_TEMPLATE,
        &[
            ("version", release.version.as_str()),
            ("unit", WEB_UNIT),
            ("config_port", &plan.config_port.to_string()),
            ("transport", plan.config_protocol.transport()),
            ("user", SERVICE_USER),
            ("switch", switch),
            ("exec", &exec),
            ("state", state),
        ],
    );
    Artifact::new(WEB_UNIT, None, Language::Ini, text)
}

/// Renders `easytier-core.service` and its env file, or `None` when the plan has no node.
pub(super) fn core_unit(
    plan: &DeploymentPlan,
    release: &SystemdRelease,
) -> Option<(Artifact, Artifact)> {
    let node = plan.node.as_ref()?;
    let exec = format!(
        "{}/easytier-core --rpc-portal 127.0.0.1:{RPC_PORTAL_PORT} --rpc-portal-whitelist 127.0.0.1/32,::1/128 --disable-env-parsing",
        bin_dir(release)
    );
    let text = fill(
        CORE_TEMPLATE,
        &[
            ("version", release.version.as_str()),
            ("unit", CORE_UNIT),
            ("owner", node.owner.as_str()),
            ("web_unit", WEB_UNIT),
            ("user", SERVICE_USER),
            ("env_path", NODE_ENV_INSTALLED_AS),
            ("exec", &exec),
        ],
    );
    let env = node_env(
        plan,
        NODE_ENV_FILE,
        &format!("Install as {NODE_ENV_INSTALLED_AS}: root:root, mode 0600."),
    )?;
    Some((Artifact::new(CORE_UNIT, None, Language::Ini, text), env))
}
