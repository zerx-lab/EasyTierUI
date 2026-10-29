//! The environment file of the server's own node. The enrollment URL (`<scheme>://127.0.0.1:<port>/<owner>`) is
//! the credential that attaches a machine to the owner's account, so it lives in a mode-0600 file and is passed to
//! `easytier-core` as `ET_CONFIG_SERVER`, never on a command line.

use super::{
    plan::DeploymentPlan,
    template::{Artifact, Language, fill},
};

/// Mode of every file that carries an enrollment URL or a secret.
pub const PRIVATE_MODE: u32 = 0o600;

const TEMPLATE: &str = r#"# EasyTierUI deployment: enrollment of this server's own node. Mode 0600; do not share, commit or paste it.
# {{install_hint}}
#
# The URL attaches the node to the easytier-web account "{{owner}}" on this server. That account does not exist
# yet: register it through the SSH tunnel during first-run hardening (README.md), then start the node.
ET_CONFIG_SERVER={{url}}
{{machine_id}}"#;

/// The env file for the node service, or `None` when the plan has no node. `file_name` and `install_hint` differ
/// between compose (`.env` next to the compose file) and systemd (`/etc/easytier/node.env`).
pub(super) fn node_env(
    plan: &DeploymentPlan,
    file_name: &str,
    install_hint: &str,
) -> Option<Artifact> {
    let node = plan.node.as_ref()?;
    let url = format!(
        "{}://127.0.0.1:{}/{}",
        plan.config_protocol.scheme(),
        plan.config_port,
        node.owner
    );
    let machine_id = match &node.machine_id {
        Some(id) => format!("ET_MACHINE_ID={id}\n"),
        None => "# ET_MACHINE_ID=<uuid>   optional: pin the node's identity across reinstalls\n"
            .to_owned(),
    };
    let text = fill(
        TEMPLATE,
        &[
            ("install_hint", install_hint),
            ("owner", node.owner.as_str()),
            ("url", &url),
            ("machine_id", &machine_id),
        ],
    );
    Some(Artifact::new(
        file_name,
        Some(PRIVATE_MODE),
        Language::Ini,
        text,
    ))
}
