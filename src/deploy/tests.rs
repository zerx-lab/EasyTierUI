//! Security properties of every runtime / proxy / node / hardening combination, and a snapshot of one plan.

use std::path::PathBuf;

use uuid::Uuid;

use super::*;

const OWNER: &str = "relay-7f3k2q";
const MACHINE_ID: &str = "0b7d1d5e-3f55-4c1a-9d2e-6a1f0c3b7e44";

fn plan(
    runtime_compose: bool,
    proxy: ProxyKind,
    node: bool,
    hardening: Hardening,
) -> DeploymentPlan {
    let domain = Hostname::parse("vpn.example.com").unwrap();
    let mut plan = if runtime_compose {
        DeploymentPlan::compose(domain, proxy).unwrap()
    } else {
        DeploymentPlan::systemd(domain, proxy, LinuxArch::X86_64).unwrap()
    };
    plan.acme_email = Some(Email::parse("ops@example.com").unwrap());
    plan.hardening = hardening;
    if node {
        plan.node = Some(NodeService {
            owner: OwnerName::parse(OWNER).unwrap(),
            machine_id: Some(Uuid::parse_str(MACHINE_ID).unwrap()),
        });
    }
    plan
}

fn all_plans() -> Vec<DeploymentPlan> {
    let mut plans = Vec::new();
    for compose in [true, false] {
        for proxy in [ProxyKind::Caddy, ProxyKind::Nginx] {
            for node in [false, true] {
                for hardening in [Hardening::Pending, Hardening::Done] {
                    plans.push(plan(compose, proxy, node, hardening));
                }
            }
        }
    }
    plans
}

/// The text without comment lines: what a tool would actually act on.
fn active(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

/// The entries under every `ports:` key of a compose file (active lines only).
fn published_ports<'a>(lines: &[&'a str]) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut in_ports = false;
    for line in lines {
        if *line == "ports:" {
            in_ports = true;
        } else if in_ports && line.starts_with("- ") {
            out.push(*line);
        } else {
            in_ports = false;
        }
    }
    out
}

fn artifact<'a>(artifacts: &'a [Artifact], name: &str) -> &'a Artifact {
    artifacts
        .iter()
        .find(|a| a.file_name == name)
        .unwrap_or_else(|| panic!("no artifact {name}"))
}

fn find<'a>(artifacts: &'a [Artifact], name: &str) -> Option<&'a Artifact> {
    artifacts.iter().find(|a| a.file_name == name)
}

#[test]
fn every_combination_renders_cleanly() {
    for plan in all_plans() {
        let artifacts = render(&plan).unwrap();
        for a in &artifacts {
            assert!(
                !a.text.contains("{{"),
                "{} has an unfilled marker",
                a.file_name
            );
            assert!(
                !a.text.contains("}}"),
                "{} has a stray marker end",
                a.file_name
            );
            assert!(!a.text.contains("example-secret"), "{}", a.file_name);
            assert!(
                a.text.ends_with('\n'),
                "{} lacks a final newline",
                a.file_name
            );
        }
        // Registration may be left open only while hardening is pending; auto-created accounts are never on.
        for a in &artifacts {
            for line in active(&a.text) {
                assert!(
                    !line.contains("--allow-auto-create-user"),
                    "{}",
                    a.file_name
                );
                assert!(!line.contains("--network-secret"), "{}", a.file_name);
                assert!(!line.contains("--webhook-secret"), "{}", a.file_name);
                assert!(!line.contains("--internal-auth-token"), "{}", a.file_name);
            }
        }
    }
}

#[test]
fn artifact_set_follows_the_plan() {
    for plan in all_plans() {
        let artifacts = render(&plan).unwrap();
        let has_node = plan.node.is_some();
        match plan.runtime {
            Runtime::Compose(_) => {
                assert!(find(&artifacts, "docker-compose.yml").is_some());
                assert_eq!(find(&artifacts, ".env").is_some(), has_node);
                assert!(find(&artifacts, "easytier-web.service").is_none());
            }
            Runtime::Systemd(_) => {
                assert!(find(&artifacts, "install-easytier.sh").is_some());
                assert!(find(&artifacts, "easytier-web.service").is_some());
                assert_eq!(
                    find(&artifacts, "easytier-core.service").is_some(),
                    has_node
                );
                assert_eq!(find(&artifacts, "easytier-node.env").is_some(), has_node);
            }
        }
        let proxy_file = if plan.proxy == ProxyKind::Caddy {
            "Caddyfile"
        } else {
            "nginx.conf"
        };
        assert!(find(&artifacts, proxy_file).is_some());
        assert!(find(&artifacts, "firewall-ufw.sh").is_some());
        assert!(find(&artifacts, "firewall-firewalld.sh").is_some());
        assert!(find(&artifacts, "README.md").is_some());
    }
}

#[test]
fn env_files_are_private_and_hold_the_only_enrollment_url() {
    for plan in all_plans() {
        let Some(node) = &plan.node else { continue };
        let artifacts = render(&plan).unwrap();
        let url = format!("udp://127.0.0.1:22020/{}", node.owner);
        let mut holders = 0;
        for a in &artifacts {
            let is_env = a.file_name == ".env" || a.file_name == "easytier-node.env";
            if is_env {
                assert_eq!(a.mode, Some(0o600), "{}", a.file_name);
                assert!(a.text.contains(&format!("ET_CONFIG_SERVER={url}\n")));
                assert!(a.text.contains(&format!("ET_MACHINE_ID={MACHINE_ID}\n")));
                holders += 1;
            } else if a.file_name != "README.md" {
                // No other file carries the enrollment URL in a place a tool would act on.
                for line in active(&a.text) {
                    assert!(!line.contains(&url), "{}: {line}", a.file_name);
                    assert!(
                        !line.contains("ET_CONFIG_SERVER"),
                        "{}: {line}",
                        a.file_name
                    );
                }
            }
            if a.file_name.ends_with(".sh") {
                assert_eq!(a.mode, Some(0o755));
            }
        }
        assert_eq!(holders, 1);
    }
}

#[test]
fn compose_pins_digests_and_never_publishes_the_api_or_rpc_portal() {
    for plan in all_plans()
        .into_iter()
        .filter(|p| matches!(p.runtime, Runtime::Compose(_)))
    {
        let artifacts = render(&plan).unwrap();
        let compose = artifact(&artifacts, "docker-compose.yml");
        let lines = active(&compose.text);

        let images: Vec<&&str> = lines.iter().filter(|l| l.starts_with("image:")).collect();
        let expected = if plan.node.is_some() { 3 } else { 2 };
        assert_eq!(images.len(), expected);
        for line in &images {
            assert!(line.contains("@sha256:"), "unpinned image: {line}");
        }
        assert!(compose.text.contains(&format!(
            "easytier/easytier:v2.6.4@sha256:{}",
            images::EASYTIER_DIGEST
        )));

        // The REST API is published on the host loopback only; the RPC portal never.
        let ports = published_ports(&lines);
        assert!(ports.len() >= 3);
        for line in &ports {
            if line.contains("11211") {
                assert!(line.starts_with("- \"127.0.0.1:11211:11211\""), "{line}");
            }
            assert!(!line.contains("15888"), "RPC portal published: {line}");
        }
        // Only the proxy's 80/443 and the config server are published to every interface.
        for line in ports.iter().filter(|l| !l.contains("127.0.0.1")) {
            let ok = ["- \"80:80\"", "- \"443:443\"", "- \"22020:22020/udp\""]
                .iter()
                .any(|p| line.starts_with(p));
            assert!(ok, "unexpected public publish: {line}");
            if line.contains("22020") {
                assert_eq!(
                    plan.hardening,
                    Hardening::Done,
                    "config server public before hardening"
                );
            }
        }
        // The portal is bound to the loopback explicitly, with a loopback whitelist.
        if plan.node.is_some() {
            assert!(lines.contains(&"- \"127.0.0.1:15888\""));
            assert!(lines.contains(&"- \"127.0.0.1/32,::1/128\""));
            assert!(lines.contains(&"- \"--disable-env-parsing\""));
            assert!(lines.contains(&"network_mode: \"service:web\""));
            assert!(lines.contains(&"- .env"));
        } else {
            assert!(!compose.text.contains("15888"));
        }
        assert!(lines.contains(&"- no-new-privileges:true"));
        assert_eq!(
            lines.contains(&"- \"--disable-registration\""),
            plan.hardening == Hardening::Done
        );
        // No secret or enrollment URL on any command line.
        assert!(
            !lines
                .iter()
                .any(|l| l.contains("udp://") || l.contains(OWNER))
        );
    }
}

#[test]
fn systemd_units_are_hardened_and_loopback_only() {
    for plan in all_plans()
        .into_iter()
        .filter(|p| matches!(p.runtime, Runtime::Systemd(_)))
    {
        let artifacts = render(&plan).unwrap();
        let web = artifact(&artifacts, "easytier-web.service");
        let lines = active(&web.text);
        let exec = lines.iter().find(|l| l.starts_with("ExecStart=")).unwrap();
        assert!(exec.starts_with("ExecStart=/opt/easytier/2.6.4/easytier-web "));
        assert!(exec.contains("--api-server-addr 127.0.0.1"));
        assert!(!exec.contains("0.0.0.0"));
        assert_eq!(
            exec.contains("--disable-registration"),
            plan.hardening == Hardening::Done
        );
        for must in [
            "NoNewPrivileges=yes",
            "ProtectSystem=strict",
            "StateDirectory=easytier-web",
            "ReadWritePaths=/var/lib/easytier-web",
            "User=easytier",
            "CapabilityBoundingSet=",
        ] {
            assert!(lines.contains(&must), "web unit lacks {must}");
        }
        assert!(
            web.text.contains("--disable-registration"),
            "the hardening step is explained in a comment"
        );

        let core = find(&artifacts, "easytier-core.service");
        assert_eq!(core.is_some(), plan.node.is_some());
        if let Some(core) = core {
            let lines = active(&core.text);
            let exec = lines.iter().find(|l| l.starts_with("ExecStart=")).unwrap();
            assert!(exec.contains("--rpc-portal 127.0.0.1:15888"));
            assert!(exec.contains("--rpc-portal-whitelist 127.0.0.1/32,::1/128"));
            assert!(exec.contains("--disable-env-parsing"));
            assert!(!exec.contains("udp://") && !exec.contains(OWNER));
            assert!(lines.contains(&"EnvironmentFile=/etc/easytier/node.env"));
            assert!(lines.contains(&"AmbientCapabilities=CAP_NET_ADMIN CAP_NET_RAW"));
            assert!(lines.contains(&"NoNewPrivileges=yes"));
        }
    }
}

#[test]
fn install_script_verifies_before_unzipping() {
    let plan = plan(false, ProxyKind::Caddy, false, Hardening::Pending);
    let artifacts = render(&plan).unwrap();
    let script = artifact(&artifacts, "install-easytier.sh");
    let text = &script.text;
    let check = text.find("sha256sum --check").unwrap();
    let unzip = text.find("unzip -q").unwrap();
    assert!(check < unzip, "checksum must be verified before unzip");
    assert!(text.contains(
        "ASSET_SHA256='61b659eaedba658fa66fe47d17e1426cdd77e5d02fa15fed447bb4357c09dfd6'"
    ));
    assert!(text.contains(
        "ASSET_URL='https://github.com/EasyTier/EasyTier/releases/download/v2.6.4/easytier-linux-x86_64-v2.6.4.zip'"
    ));
    assert!(text.contains("--proto '=https'"));
    assert!(text.contains("set -euo pipefail"));
    assert!(
        !text.contains("systemctl"),
        "the script starts no service (comments aside)"
    );
}

#[test]
fn api_is_closed_until_hardening_is_done() {
    for plan in all_plans() {
        let artifacts = render(&plan).unwrap();
        let proxy = match plan.proxy {
            ProxyKind::Caddy => artifact(&artifacts, "Caddyfile"),
            ProxyKind::Nginx => artifact(&artifacts, "nginx.conf"),
        };
        let lines = active(&proxy.text);
        let forwards = lines
            .iter()
            .any(|l| l.starts_with("reverse_proxy") || l.starts_with("proxy_pass"));
        let closed = lines.iter().any(|l| l.contains("403"));
        match plan.hardening {
            Hardening::Pending => {
                assert!(
                    !forwards,
                    "{}: API proxied before hardening",
                    proxy.file_name
                );
                assert!(closed);
                // The commented-out alternative is the documented switch.
                assert!(proxy.text.contains("HARDENING SWITCH"));
            }
            Hardening::Done => {
                assert!(forwards);
                assert!(!closed);
            }
        }
        let upstream = match plan.runtime {
            Runtime::Compose(_) => "web:11211",
            Runtime::Systemd(_) => "127.0.0.1:11211",
        };
        assert!(proxy.text.contains(upstream));
        for header in [
            "Strict-Transport-Security",
            "X-Content-Type-Options",
            "Content-Security-Policy",
            "frame-ancestors 'none'",
        ] {
            assert!(proxy.text.contains(header), "{}: {header}", proxy.file_name);
        }
        // The API port is never the proxy's own listener.
        assert!(
            !lines
                .iter()
                .any(|l| l.contains("listen 11211") || l.contains(":11211 {"))
        );
    }
}

#[test]
fn registration_is_open_only_while_hardening_is_pending() {
    for plan in all_plans() {
        let artifacts = render(&plan).unwrap();
        let (name, flag) = match plan.runtime {
            Runtime::Compose(_) => ("docker-compose.yml", "--disable-registration"),
            Runtime::Systemd(_) => ("easytier-web.service", "--disable-registration"),
        };
        let a = artifact(&artifacts, name);
        let on = active(&a.text).iter().any(|l| l.contains(flag));
        assert_eq!(on, plan.hardening == Hardening::Done);
    }
}

fn open_ports(script: &str, prefix: &str) -> Vec<String> {
    active(script)
        .iter()
        .filter_map(|l| l.strip_prefix(prefix))
        .map(|rest| rest.split_whitespace().next().unwrap_or("").to_owned())
        .collect()
}

#[test]
fn firewall_opens_only_the_expected_ports() {
    for plan in all_plans() {
        let artifacts = render(&plan).unwrap();
        let ufw = artifact(&artifacts, "firewall-ufw.sh");
        let fwd = artifact(&artifacts, "firewall-firewalld.sh");
        let mut expected = vec!["80/tcp".to_owned(), "443/tcp".to_owned()];
        if plan.hardening == Hardening::Done {
            expected.push("22020/udp".to_owned());
        }
        assert_eq!(open_ports(&ufw.text, "ufw allow "), expected);
        let fw: Vec<String> = open_ports(&fwd.text, "firewall-cmd --permanent --add-port=");
        assert_eq!(fw, expected);
        for script in [ufw, fwd] {
            for line in active(&script.text) {
                assert!(!line.contains("11211") && !line.contains("15888"), "{line}");
                assert!(!line.contains("enable"), "{line}");
            }
        }
    }
}

#[test]
fn config_protocol_selects_transport() {
    let mut p = plan(false, ProxyKind::Caddy, true, Hardening::Done);
    p.config_protocol = ConfigProtocol::Ws;
    p.config_port = 22021;
    let artifacts = render(&p).unwrap();
    assert!(
        artifact(&artifacts, "easytier-node.env")
            .text
            .contains("ET_CONFIG_SERVER=ws://127.0.0.1:22021/relay-7f3k2q")
    );
    assert_eq!(
        open_ports(&artifact(&artifacts, "firewall-ufw.sh").text, "ufw allow "),
        ["80/tcp", "443/tcp", "22021/tcp"]
    );
    assert!(
        artifact(&artifacts, "easytier-web.service")
            .text
            .contains("--config-server-protocol ws")
    );
}

#[test]
fn readme_orders_hardening_before_exposure() {
    let artifacts = render(&plan(true, ProxyKind::Caddy, true, Hardening::Pending)).unwrap();
    let text = &artifact(&artifacts, "README.md").text;
    let tunnel = text.find("ssh -L 11211:127.0.0.1:11211").unwrap();
    let add_server = text.find("*Add server*").unwrap();
    let flip = text.find("**Flip the switch.**").unwrap();
    let node = text.find("**Start the node.**").unwrap();
    assert!(tunnel < flip && add_server < flip && flip < node);
    assert!(text.contains("http://127.0.0.1:11211"));
    assert!(text.contains("22020/udp"));
    assert!(text.contains("binds every interface"));
    assert!(text.contains("admin") && text.contains("user"));
    assert!(text.contains(OWNER));
}

#[test]
fn invalid_input_never_reaches_a_template() {
    assert!(Hostname::parse("evil.example.com\nreverse_proxy 6.6.6.6").is_err());
    assert!(Hostname::parse("a.com {").is_err());
    assert!(OwnerName::parse("admin").is_err());
    assert!(OwnerName::parse("user").is_err());
    assert!(OwnerName::parse("ab").is_err());
    assert!(OwnerName::parse("a;b;c").is_err());
    assert!(AbsPath::parse("/srv/x; rm -rf /", Field::WebDir).is_err());
}

// --- snapshot -------------------------------------------------------------------------------------------------

fn snapshot_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/deploy")
        .join(name)
}

/// Compares every artifact of `plan` with `tests/fixtures/deploy/<name>/<file>`. Run once with
/// `UPDATE_DEPLOY_SNAPSHOTS=1` to (re)write them after an intended template change, and review the diff.
fn check_snapshot(name: &str, plan: &DeploymentPlan) {
    let dir = snapshot_dir(name);
    let update = std::env::var_os("UPDATE_DEPLOY_SNAPSHOTS").is_some();
    if update {
        std::fs::create_dir_all(&dir).unwrap();
    }
    let artifacts = render(plan).unwrap();
    for a in &artifacts {
        let path = dir.join(&a.file_name);
        if update {
            std::fs::write(&path, &a.text).unwrap();
        }
        let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "missing snapshot {}: {e}; run with UPDATE_DEPLOY_SNAPSHOTS=1",
                path.display()
            )
        });
        assert_eq!(a.text, expected, "snapshot {} differs", path.display());
    }
}

#[test]
fn snapshot_compose_caddy_node_pending() {
    check_snapshot(
        "compose-caddy-node-pending",
        &plan(true, ProxyKind::Caddy, true, Hardening::Pending),
    );
}
