//! `docker-compose.yml` and its `.env`.
//!
//! Network design: `web` (easytier-web) and `proxy` share the `edge` network. The REST API listens on the edge
//! network, is reached by the proxy as `web:<api port>`, and is published only on the host's loopback so the
//! operator can open an SSH tunnel for first-run hardening. The optional `node` service joins `web`'s network
//! namespace, so the enrollment URL is the container's own loopback, and its RPC portal stays on `127.0.0.1` there.

use super::{
    images,
    plan::{ComposeImage, DeploymentPlan, Hardening, ProxyKind, RPC_PORTAL_PORT},
    template::{Artifact, Language, fill, yaml_str},
};

/// The compose file's name.
pub const COMPOSE_FILE: &str = "docker-compose.yml";
/// The env file next to it (enrollment URL of the server's node).
pub const ENV_FILE: &str = ".env";

const TEMPLATE: &str = r#"# EasyTierUI deployment: easytier-web {{version}} behind {{proxy_name}} for {{domain}}.
# Rendered, not applied. Review it, then run `docker compose up -d` on the server (README.md has the order).
#
# Network layout
#   edge network  web and proxy. The REST API listens here only: the proxy reaches it as web:{{api_port}},
#                 nothing publishes it to the internet.
#   published     proxy 80/tcp and 443/tcp; the config server {{config_port}}/{{transport}} (see HARDENING SWITCH);
#                 the REST API on the host's loopback only (127.0.0.1), for the SSH tunnel used to harden the server.
#   never         the node's RPC portal: it binds 127.0.0.1 inside web's network namespace and is not published.
# Images are pinned by digest; the tag is only a label. {{checked}}

name: easytier

services:
  web:
    image: {{image}}
    # The image's entrypoint is easytier-core; this service runs easytier-web from the same release.
    entrypoint: ["/sbin/tini", "--", "easytier-web"]
    command:
{{web_command}}
    restart: unless-stopped
    ports:
{{web_ports}}
    volumes:
      - {{data_volume}}:/data
    environment:
      TZ: UTC
    read_only: true
    tmpfs:
      - /tmp
    cap_drop:
      - ALL
    security_opt:
      - no-new-privileges:true
    networks:
      - edge
{{node_service}}{{proxy_service}}
volumes:
  {{data_volume}}:
{{proxy_volumes}}
networks:
  edge: {}
"#;

const NODE_TEMPLATE: &str = r#"
  # The server as a node (relay). It shares web's network namespace so ET_CONFIG_SERVER points at the container's
  # own loopback, and its RPC portal is bound to 127.0.0.1 there. Start it after the owner account exists (README).
  node:
    image: {{image}}
    network_mode: "service:web"
    depends_on:
      - web
    command:
      - "--rpc-portal"
      - "127.0.0.1:{{rpc_port}}"
      - "--rpc-portal-whitelist"
      - "127.0.0.1/32,::1/128"
      - "--disable-env-parsing"
    # ET_CONFIG_SERVER (the enrollment URL) lives in .env, mode 0600; it is never on a command line.
    env_file:
      - .env
    restart: unless-stopped
    cap_drop:
      - ALL
    cap_add:
      - NET_ADMIN
    devices:
      - /dev/net/tun:/dev/net/tun
    security_opt:
      - no-new-privileges:true
"#;

const CADDY_TEMPLATE: &str = r#"
  proxy:
    image: {{proxy_image}}
    restart: unless-stopped
    depends_on:
      - web
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile:ro
      - {{web_mount}}
      - caddy-data:/data
      - caddy-config:/config
    read_only: true
    tmpfs:
      - /tmp
    cap_drop:
      - ALL
    cap_add:
      - NET_BIND_SERVICE
    security_opt:
      - no-new-privileges:true
    networks:
      - edge
"#;

const NGINX_TEMPLATE: &str = r#"
  proxy:
    image: {{proxy_image}}
    restart: unless-stopped
    depends_on:
      - web
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx.conf:/etc/nginx/conf.d/default.conf:ro
      - {{web_mount}}
      # Certificates issued on the host with certbot (README.md).
      - /etc/letsencrypt:/etc/letsencrypt:ro
      - /var/www/certbot:/var/www/certbot:ro
    cap_drop:
      - ALL
    # nginx's master starts as root and drops to the nginx user.
    cap_add:
      - CHOWN
      - SETGID
      - SETUID
      - NET_BIND_SERVICE
    security_opt:
      - no-new-privileges:true
    networks:
      - edge
"#;

/// Renders `docker-compose.yml` for `plan` with `image`.
pub(super) fn compose_file(plan: &DeploymentPlan, image: &ComposeImage) -> Artifact {
    let reference = image.reference();
    let api_port = plan.api_port.to_string();
    let config_port = plan.config_port.to_string();
    let transport = plan.config_protocol.transport();
    let hardened = plan.hardening == Hardening::Done;

    let mut command: Vec<String> = Vec::new();
    for arg in [
        "--db",
        "/data/et.db",
        "--api-server-addr",
        // Inside the container: reachable on the edge network only, see the layout comment.
        "0.0.0.0",
        "--api-server-port",
        api_port.as_str(),
        "--config-server-port",
        config_port.as_str(),
        "--config-server-protocol",
        plan.config_protocol.scheme(),
    ] {
        command.push(format!("      - {}", yaml_str(arg)));
    }
    if hardened {
        command.push("      # HARDENING SWITCH (done): registration is off.".to_owned());
        command.push("      - \"--disable-registration\"".to_owned());
    } else {
        command.push(
            "      # HARDENING SWITCH: registration stays open until the owner account exists. After first-run\n      \
             # hardening, re-render with hardening done (or uncomment the next line) and `docker compose up -d`."
                .to_owned(),
        );
        command.push("      # - \"--disable-registration\"".to_owned());
    }

    let mut ports: Vec<String> = Vec::new();
    ports.push("      # REST API: host loopback only (SSH tunnel). The proxy reaches it over the edge network.".to_owned());
    ports.push(format!(
        "      - {}",
        yaml_str(&format!("127.0.0.1:{api_port}:{api_port}"))
    ));
    if hardened {
        ports.push(
            "      # HARDENING SWITCH (done): the config server is open on every interface."
                .to_owned(),
        );
        ports.push(format!(
            "      - {}",
            yaml_str(&format!("{config_port}:{config_port}/{transport}"))
        ));
    } else {
        ports.push(
            "      # HARDENING SWITCH: the config server stays on the loopback until hardening is done; then drop\n      \
             # the `127.0.0.1:` prefix to let remote nodes enroll. Docker publishes bypass ufw / firewalld."
                .to_owned(),
        );
        ports.push(format!(
            "      - {}",
            yaml_str(&format!(
                "127.0.0.1:{config_port}:{config_port}/{transport}"
            ))
        ));
    }

    let node_service = if plan.node.is_some() {
        fill(
            NODE_TEMPLATE,
            &[
                ("image", reference.as_str()),
                ("rpc_port", &RPC_PORTAL_PORT.to_string()),
            ],
        )
    } else {
        String::new()
    };

    let web_mount = yaml_str(&format!("{}:/srv/www:ro", plan.paths.web_dir));
    let (proxy_name, proxy_service, proxy_volumes) = match plan.proxy {
        ProxyKind::Caddy => {
            let proxy_image = format!(
                "{}:{}@sha256:{}",
                images::CADDY_IMAGE,
                images::CADDY_TAG,
                images::CADDY_DIGEST
            );
            (
                "Caddy",
                fill(
                    CADDY_TEMPLATE,
                    &[("proxy_image", &proxy_image), ("web_mount", &web_mount)],
                ),
                "  caddy-data:\n  caddy-config:\n".to_owned(),
            )
        }
        ProxyKind::Nginx => {
            let proxy_image = format!(
                "{}:{}@sha256:{}",
                images::NGINX_IMAGE,
                images::NGINX_TAG,
                images::NGINX_DIGEST
            );
            (
                "nginx",
                fill(
                    NGINX_TEMPLATE,
                    &[("proxy_image", &proxy_image), ("web_mount", &web_mount)],
                ),
                String::new(),
            )
        }
    };

    let text = fill(
        TEMPLATE,
        &[
            ("version", image.tag.as_str()),
            ("proxy_name", proxy_name),
            ("domain", plan.domain.as_str()),
            ("api_port", &api_port),
            ("config_port", &config_port),
            ("transport", transport),
            (
                "checked",
                &format!("Digests read from the registry on {}.", images::CHECKED_ON),
            ),
            ("image", &reference),
            ("web_command", &command.join("\n")),
            ("web_ports", &ports.join("\n")),
            ("data_volume", plan.paths.data_volume.as_str()),
            ("node_service", &node_service),
            ("proxy_service", &proxy_service),
            ("proxy_volumes", &proxy_volumes),
        ],
    );
    Artifact::new(COMPOSE_FILE, None, Language::Yaml, text)
}
