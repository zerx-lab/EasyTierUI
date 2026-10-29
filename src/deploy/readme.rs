//! `README.md`: the ordered steps for the rendered files, hardening first.

use super::{
    compose::{COMPOSE_FILE, ENV_FILE},
    firewall::{FIREWALLD_SCRIPT, UFW_SCRIPT},
    plan::{DeploymentPlan, Hardening, ProxyKind, RPC_PORTAL_PORT, Runtime},
    proxy::{CADDYFILE, CERTBOT_WEBROOT, NGINX_FILE},
    systemd::{CORE_UNIT, INSTALL_SCRIPT, NODE_ENV_FILE, NODE_ENV_INSTALLED_AS, WEB_UNIT},
    template::{Artifact, Language},
};

/// The README's name.
pub const README_FILE: &str = "README.md";

/// Where the dashboard's API is tunnelled to on the operator's machine.
const TUNNEL_URL: &str = "http://127.0.0.1";

/// Renders the README for `plan`.
pub(super) fn readme(plan: &DeploymentPlan) -> Artifact {
    let domain = plan.domain.as_str();
    let api = plan.api_port;
    let config = format!("{}/{}", plan.config_port, plan.config_protocol.transport());
    let web_dir = plan.paths.web_dir.as_str();
    let proxy_file = match plan.proxy {
        ProxyKind::Caddy => CADDYFILE,
        ProxyKind::Nginx => NGINX_FILE,
    };
    let compose = matches!(plan.runtime, Runtime::Compose(_));
    let owner = plan.node.as_ref().map(|node| node.owner.as_str());

    let mut out = String::new();
    out.push_str(&format!("# EasyTier server for {domain}\n\n"));
    out.push_str(
        "Rendered by EasyTierUI. Nothing was applied anywhere: read each file, then follow the steps in order.\n\n",
    );
    out.push_str(if plan.hardening == Hardening::Done {
        "> These files are rendered with hardening **done**: `/api/` is proxied and the config server is open. Use them only on a server whose seeded passwords are already replaced; the steps below describe the first install, from a fresh server.\n\n"
    } else {
        "> These files are rendered with hardening **pending**: the API stays closed at the proxy and the config server stays closed in the firewall until you flip the switch (step \"Flip the switch\").\n\n"
    });

    out.push_str("## Files\n\n");
    if compose {
        out.push_str(&format!(
            "- `{COMPOSE_FILE}`: easytier-web, {} and an optional node, images pinned by digest\n",
            match plan.proxy {
                ProxyKind::Caddy => "Caddy",
                ProxyKind::Nginx => "nginx",
            }
        ));
        if owner.is_some() {
            out.push_str(&format!(
                "- `{ENV_FILE}`: the node's enrollment URL, mode 0600\n"
            ));
        }
    } else {
        out.push_str(&format!("- `{INSTALL_SCRIPT}`: downloads the release, checks its sha256 before unzipping, installs to `/opt/easytier/<version>`\n"));
        out.push_str(&format!(
            "- `{WEB_UNIT}`: easytier-web, REST API on 127.0.0.1\n"
        ));
        if owner.is_some() {
            out.push_str(&format!(
                "- `{CORE_UNIT}` and `{NODE_ENV_FILE}` (mode 0600): this server as a node\n"
            ));
        }
    }
    out.push_str(&format!(
        "- `{proxy_file}`: the reverse proxy; `/api/` stays closed until hardening is done\n"
    ));
    out.push_str(&format!(
        "- `{UFW_SCRIPT}`, `{FIREWALLD_SCRIPT}`: pick the one your server uses\n\n"
    ));

    out.push_str("## What is reachable from the internet\n\n");
    out.push_str("| Port | Reachable | Why |\n|---|---|---|\n");
    out.push_str(
        "| 80/tcp, 443/tcp | yes | the reverse proxy: dashboard and, after hardening, `/api/` |\n",
    );
    out.push_str(&format!(
        "| {config} | **after hardening** | the config server; it binds every interface and has no bind-address option, so the firewall is its only gate |\n"
    ));
    out.push_str(&format!(
        "| {api}/tcp | no | the REST API: {} |\n",
        if compose {
            "on the compose network and on the host's loopback only"
        } else {
            "bound to 127.0.0.1"
        }
    ));
    out.push_str(&format!(
        "| {RPC_PORTAL_PORT}/tcp | no | the node's RPC portal, bound to 127.0.0.1 |\n\n"
    ));

    let mut steps: Vec<String> = Vec::new();

    steps.push(format!(
        "**DNS.** Point an A / AAAA record of `{domain}` at this server. Ports 80 and 443 must be free."
    ));

    if compose {
        steps.push(format!(
            "**Copy the files.** Put them in one directory on the server, for example `/opt/easytier-deploy`{}.",
            if owner.is_some() {
                format!(", then `chmod 600 {ENV_FILE}`")
            } else {
                String::new()
            }
        ));
    } else {
        steps.push(format!(
            "**Install the binaries.** Copy `{INSTALL_SCRIPT}` to the server, read it, run `sudo bash {INSTALL_SCRIPT}`. \
             It verifies the zip's sha256 before it unzips anything and stops on a mismatch."
        ));
    }

    steps.push(format!(
        "**Publish the dashboard.** On your machine run `nsh package --target web` in the EasyTierUI project and copy the \
         output directory's contents to `{web_dir}` on the server (the proxy serves it read-only; it must be readable by the proxy's user)."
    ));

    if plan.proxy == ProxyKind::Nginx {
        steps.push(format!(
            "**Certificate (nginx).** Before nginx starts, with port 80 free: \
             `sudo certbot certonly --standalone -d {domain}`. Then switch renewal to the webroot that `{NGINX_FILE}` serves \
             (`sudo mkdir -p {CERTBOT_WEBROOT}`; `sudo certbot reconfigure --cert-name {domain} --webroot -w {CERTBOT_WEBROOT}`, certbot 2.3 or newer)."
        ));
    }

    steps.push(format!(
        "**Firewall.** Run `sudo sh {UFW_SCRIPT}` (ufw) or `sudo sh {FIREWALLD_SCRIPT}` (firewalld). Before hardening they open 80/tcp \
         and 443/tcp only; the config-server rule is commented out. Keep your SSH rule: neither script enables the firewall."
    ));

    if compose {
        steps.push(format!(
            "**Start the control plane.** `docker compose up -d web proxy`{}. The REST API is now on the server's loopback \
             (`127.0.0.1:{api}`) and on the compose network; `{proxy_file}` answers 403 on `/api/`.",
            if owner.is_some() {
                " (not the node yet)"
            } else {
                ""
            }
        ));
    } else {
        let proxy_step = match plan.proxy {
            ProxyKind::Caddy => format!(
                "`sudo install -m 0644 {CADDYFILE} /etc/caddy/Caddyfile && sudo systemctl reload caddy`"
            ),
            ProxyKind::Nginx => format!(
                "`sudo install -m 0644 {NGINX_FILE} /etc/nginx/conf.d/easytier.conf && sudo nginx -t && sudo systemctl reload nginx`"
            ),
        };
        steps.push(format!(
            "**Start the control plane.** `sudo install -m 0644 {WEB_UNIT} /etc/systemd/system/ && sudo systemctl daemon-reload && sudo systemctl enable --now easytier-web`, \
             then install the proxy config: {proxy_step}. `/api/` answers 403."
        ));
    }

    steps.push(format!(
        "**Check that the API is closed.** `curl -i https://{domain}/api/v1/summary` must answer `403` from the proxy, and the dashboard must load at `https://{domain}/`. \
         Do not continue if `/api/` answers anything else: the seeded accounts `admin`/`admin` and `user`/`user` are still active."
    ));

    let owner_hint = match owner {
        Some(name) => format!(
            " Registration is still open, so also register the account `{name}` (the owner the server's node enrolls under) \
             with a strong, generated password."
        ),
        None => String::new(),
    };
    steps.push(format!(
        "**Harden through an SSH tunnel.** On your machine: `ssh -L {api}:127.0.0.1:{api} <user>@{domain}`. In EasyTierUI choose *Add server*, \
         enter `{TUNNEL_URL}:{api}`, sign in as `admin` / `admin` and change the password, then do the same for `user` / `user` (or leave it unused with a new password).{owner_hint}"
    ));

    let node_note = if owner.is_some() {
        " Start the node afterwards."
    } else {
        ""
    };
    let flip = if compose {
        format!(
            "In EasyTierUI mark hardening as done and render again, or edit by hand: in `{proxy_file}` replace the closed `/api/` block by the commented one; \
             in `{COMPOSE_FILE}` uncomment `--disable-registration` and drop the `127.0.0.1:` prefix from the config-server port; in `{UFW_SCRIPT}` / `{FIREWALLD_SCRIPT}` uncomment the config-server rule. \
             Then `docker compose up -d`, reload the proxy if needed and run the firewall script again."
        )
    } else {
        format!(
            "In EasyTierUI mark hardening as done and render again, or edit by hand: in `{proxy_file}` replace the closed `/api/` block by the commented one; \
             in `{WEB_UNIT}` append ` --disable-registration` to `ExecStart`; in `{UFW_SCRIPT}` / `{FIREWALLD_SCRIPT}` uncomment the config-server rule. \
             Then `sudo systemctl daemon-reload && sudo systemctl restart easytier-web`, reload the proxy and run the firewall script again."
        )
    };
    steps.push(format!(
        "**Flip the switch.** Only after both seeded passwords are replaced. {flip}{node_note}"
    ));

    if let Some(name) = owner {
        if compose {
            steps.push(format!(
                "**Start the node.** `docker compose up -d`. It enrolls to `127.0.0.1` as `{name}` with the URL in `{ENV_FILE}`; it needs `/dev/net/tun` and `NET_ADMIN`, which the compose file grants to this service only."
            ));
        } else {
            steps.push(format!(
                "**Start the node.** `sudo install -d -m 0755 /etc/easytier && sudo install -m 0600 -o root -g root {NODE_ENV_FILE} {NODE_ENV_INSTALLED_AS}`, \
                 then `sudo install -m 0644 {CORE_UNIT} /etc/systemd/system/ && sudo systemctl daemon-reload && sudo systemctl enable --now easytier-core`. \
                 The account `{name}` must exist (registered in the hardening step) or the config server refuses the enrollment."
            ));
        }
    }

    steps.push(format!(
        "**Verify.** `curl -i https://{domain}/api/v1/auth/check_login_status` now answers `401` from easytier-web (not `403` from the proxy), \
         registration answers `403`, and in EasyTierUI *Add server* `https://{domain}` shows the machines that enroll. \
         From outside, only 80/tcp, 443/tcp and {config} answer."
    ));

    out.push_str("## Steps\n\n");
    for (index, step) in steps.iter().enumerate() {
        out.push_str(&format!("{}. {step}\n\n", index.saturating_add(1)));
    }

    out.push_str("## Notes\n\n");
    out.push_str(
        "- The seeded accounts `admin`/`admin` and `user`/`user` exist in every fresh easytier-web database. The API must not be reachable from the internet until both are replaced; that is why the proxy answers 403 on `/api/` first.\n",
    );
    out.push_str(
        "- The enrollment URL (`ET_CONFIG_SERVER`) attaches a machine to an account: it is kept in a mode-0600 file, never on a command line.\n",
    );
    out.push_str(
        "- The Content-Security-Policy header is a starting point for the web build; if the dashboard stops loading, check the browser console and widen it deliberately.\n",
    );
    if plan.hardening == Hardening::Done {
        out.push_str("- These files were rendered with hardening marked done (`/api/` proxied, registration off, config server open). On a fresh server render them with hardening pending first and follow the steps; use the done files only for an already hardened server.\n");
    }

    Artifact::new(README_FILE, None, Language::Md, out)
}
