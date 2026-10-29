//! Firewall scripts for ufw and firewalld. They open exactly 80/tcp and 443/tcp for the proxy and, once hardening is
//! done, the config-server port. They never enable the firewall, touch SSH or open the REST API or the RPC portal.

use super::{
    plan::{DeploymentPlan, Hardening, Runtime},
    template::{Artifact, Language, fill},
};

/// The ufw script's name.
pub const UFW_SCRIPT: &str = "firewall-ufw.sh";
/// The firewalld script's name.
pub const FIREWALLD_SCRIPT: &str = "firewall-firewalld.sh";

const UFW_TEMPLATE: &str = r#"#!/bin/sh
# EasyTierUI deployment: ufw rules for {{domain}}.
# Rendered, not applied. Read it, then run it as root. It opens only:
#   80/tcp   HTTP: certificate issuance and the redirect to HTTPS
#   443/tcp  HTTPS: the dashboard{{api_note}}
#   {{config}}  config server (remote nodes enroll here), after hardening
# It does not enable ufw and does not touch SSH: allow your SSH port yourself BEFORE `ufw enable`.
# The REST API ({{api_port}}) and the node's RPC portal ({{rpc_port}}) stay closed.
{{docker_note}}set -eu

ufw allow 80/tcp comment 'EasyTier proxy http'
ufw allow 443/tcp comment 'EasyTier proxy https'
{{config_rule}}"#;

const FIREWALLD_TEMPLATE: &str = r#"#!/bin/sh
# EasyTierUI deployment: firewalld rules for {{domain}}.
# Rendered, not applied. Read it, then run it as root. It opens only:
#   80/tcp   HTTP: certificate issuance and the redirect to HTTPS
#   443/tcp  HTTPS: the dashboard{{api_note}}
#   {{config}}  config server (remote nodes enroll here), after hardening
# It touches only the default zone's permanent rules and does not change SSH.
# The REST API ({{api_port}}) and the node's RPC portal ({{rpc_port}}) stay closed.
{{docker_note}}set -eu

firewall-cmd --permanent --add-port=80/tcp
firewall-cmd --permanent --add-port=443/tcp
{{config_rule}}firewall-cmd --reload
"#;

fn shared_vars(plan: &DeploymentPlan) -> (String, String, String, String, &'static str) {
    let config = format!("{}/{}", plan.config_port, plan.config_protocol.transport());
    let api_note = if plan.hardening == Hardening::Done {
        " and, now that hardening is done, /api/".to_owned()
    } else {
        " (/api/ is closed at the proxy until hardening is done)".to_owned()
    };
    let docker_note = match plan.runtime {
        Runtime::Compose(_) => {
            "# Docker publishes ports through its own iptables rules, which ufw and firewalld do not filter:\n\
             # the compose file therefore publishes the config server on 127.0.0.1 until hardening is done.\n"
        }
        Runtime::Systemd(_) => "",
    };
    (
        config,
        api_note,
        plan.api_port.to_string(),
        super::plan::RPC_PORTAL_PORT.to_string(),
        docker_note,
    )
}

fn config_rule(plan: &DeploymentPlan, config: &str, open: &str, closed_prefix: &str) -> String {
    let line = open.replace("{port}", config);
    match plan.hardening {
        Hardening::Done => format!(
            "# HARDENING SWITCH (done): the config server is reachable from the internet.\n{line}\n"
        ),
        Hardening::Pending => format!(
            "# HARDENING SWITCH: the config server stays closed until hardening is done (README.md); then re-render\n\
             # with hardening done or uncomment the next line. It binds every interface, so this rule is the gate.\n\
             {closed_prefix}{line}\n"
        ),
    }
}

/// Renders the ufw script.
pub(super) fn ufw(plan: &DeploymentPlan) -> Artifact {
    let (config, api_note, api_port, rpc_port, docker_note) = shared_vars(plan);
    let rule = config_rule(
        plan,
        &config,
        "ufw allow {port} comment 'EasyTier config server'",
        "# ",
    );
    let text = fill(
        UFW_TEMPLATE,
        &[
            ("domain", plan.domain.as_str()),
            ("api_note", &api_note),
            ("config", &config),
            ("api_port", &api_port),
            ("rpc_port", &rpc_port),
            ("docker_note", docker_note),
            ("config_rule", &rule),
        ],
    );
    Artifact::new(UFW_SCRIPT, Some(0o755), Language::Sh, text)
}

/// Renders the firewalld script.
pub(super) fn firewalld(plan: &DeploymentPlan) -> Artifact {
    let (config, api_note, api_port, rpc_port, docker_note) = shared_vars(plan);
    let rule = config_rule(
        plan,
        &config,
        "firewall-cmd --permanent --add-port={port}",
        "# ",
    );
    let text = fill(
        FIREWALLD_TEMPLATE,
        &[
            ("domain", plan.domain.as_str()),
            ("api_note", &api_note),
            ("config", &config),
            ("api_port", &api_port),
            ("rpc_port", &rpc_port),
            ("docker_note", docker_note),
            ("config_rule", &rule),
        ],
    );
    Artifact::new(FIREWALLD_SCRIPT, Some(0o755), Language::Sh, text)
}
