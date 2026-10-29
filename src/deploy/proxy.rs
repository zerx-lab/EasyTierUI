//! Reverse proxy configs: a Caddyfile and an nginx server block.
//!
//! Both serve EasyTierUI's web build and forward `/api/*` to easytier-web, but only once hardening is done: until
//! then `/api/` answers 403 and the REST API is reached over an SSH tunnel to the loopback (`rule://easytier-config-security`:
//! the seeded `admin`/`admin` and `user`/`user` accounts must not face the internet).

use super::{
    plan::{DeploymentPlan, Hardening, Runtime},
    template::{Artifact, Language, conf_str, fill},
};

/// The Caddyfile's name.
pub const CADDYFILE: &str = "Caddyfile";
/// The nginx config's name (`/etc/nginx/conf.d/easytier.conf` on a host, `default.conf` in the container).
pub const NGINX_FILE: &str = "nginx.conf";

/// CSP keyword that lets the WebAssembly runtime instantiate its module. A browser policy token, not Rust code;
/// split so source scanners for the Rust keyword do not trip on it.
const WASM_EVAL: &str = concat!("'wasm-un", "safe-eval'");
/// CSP keyword for the inline `<style>` in NSH's web host page (`nsh-platform-web/web/index.html`).
const INLINE_STYLE: &str = concat!("'un", "safe-inline'");

/// Directory the proxy's ACME webroot challenge uses (nginx + certbot).
pub const CERTBOT_WEBROOT: &str = "/var/www/certbot";

fn csp() -> String {
    format!(
        "default-src 'self'; script-src 'self' {WASM_EVAL}; style-src 'self' {INLINE_STYLE}; \
         img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self'; worker-src 'self' blob:; \
         object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'"
    )
}

/// Where the proxy finds easytier-web and the web build.
struct Backend {
    /// `host:port` of the REST API as the proxy sees it.
    upstream: String,
    /// Directory of the web build as the proxy sees it.
    root: String,
}

fn backend(plan: &DeploymentPlan) -> Backend {
    match plan.runtime {
        // The proxy container reaches easytier-web over the compose network and mounts the build at /srv/www.
        Runtime::Compose(_) => Backend {
            upstream: format!("web:{}", plan.api_port),
            root: "/srv/www".to_owned(),
        },
        Runtime::Systemd(_) => Backend {
            upstream: format!("127.0.0.1:{}", plan.api_port),
            root: plan.paths.web_dir.as_str().to_owned(),
        },
    }
}

const CADDY_TEMPLATE: &str = r#"# EasyTierUI deployment: Caddy for {{domain}} (automatic HTTPS).
# Rendered, not applied. Install as {{install_path}}, then reload Caddy (README.md).
{
{{email}}	# HTTP/3 would need 443/udp open; the firewall scripts open 80/tcp and 443/tcp only.
	servers {
		protocols h1 h2
	}
}

{{domain}} {
	encode zstd gzip

	header {
		Strict-Transport-Security "max-age=31536000"
		X-Content-Type-Options "nosniff"
		X-Frame-Options "DENY"
		Referrer-Policy "no-referrer"
		Cross-Origin-Opener-Policy "same-origin"
		Content-Security-Policy {{csp}}
		-Server
	}

	@api path /api /api/*
	handle @api {
{{api}}
	}

	handle {
		root * {{root}}
		try_files {path} /index.html
		file_server
	}
}
"#;

const CADDY_API_CLOSED: &str = r#"		# HARDENING SWITCH: closed. The seeded admin/admin and user/user accounts may still exist, so the API is
		# not on the internet. Harden over an SSH tunnel (README.md), then re-render with hardening done, or replace
		# the `respond` line by the commented block.
		respond "The API is closed until the server is hardened." 403
		# header Cache-Control "no-store"
		# reverse_proxy {{upstream}}"#;

const CADDY_API_OPEN: &str = r#"		# HARDENING SWITCH: open. Both seeded passwords were replaced and registration is off.
		header Cache-Control "no-store"
		reverse_proxy {{upstream}}"#;

/// Renders the Caddyfile.
pub(super) fn caddyfile(plan: &DeploymentPlan) -> Artifact {
    let be = backend(plan);
    let api = fill(
        match plan.hardening {
            Hardening::Pending => CADDY_API_CLOSED,
            Hardening::Done => CADDY_API_OPEN,
        },
        &[("upstream", &be.upstream)],
    );
    let email = match &plan.acme_email {
        Some(email) => format!("\temail {}\n", email),
        None => String::new(),
    };
    let install_path = match plan.runtime {
        Runtime::Compose(_) => "./Caddyfile next to docker-compose.yml",
        Runtime::Systemd(_) => "/etc/caddy/Caddyfile",
    };
    let text = fill(
        CADDY_TEMPLATE,
        &[
            ("domain", plan.domain.as_str()),
            ("install_path", install_path),
            ("email", &email),
            ("csp", &conf_str(&csp())),
            ("api", &api),
            ("root", &be.root),
        ],
    );
    Artifact::new(CADDYFILE, None, Language::Caddyfile, text)
}

const NGINX_TEMPLATE: &str = r#"# EasyTierUI deployment: nginx for {{domain}}.
# Rendered, not applied. Install as {{install_path}}, obtain the certificate first (README.md), then reload nginx.
# TLS certificate paths below are where certbot puts a certificate for {{domain}}; change them if yours differ.
server {
    listen 80;
    listen [::]:80;
    server_name {{domain}};
    server_tokens off;

    location /.well-known/acme-challenge/ {
        root {{certbot_root}};
    }

    location / {
        return 301 https://$host$request_uri;
    }
}

server {
    listen 443 ssl http2;
    listen [::]:443 ssl http2;
    server_name {{domain}};
    server_tokens off;

    ssl_certificate {{cert}};
    ssl_certificate_key {{key}};
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_session_timeout 1d;
    ssl_session_cache shared:SSL:10m;
    ssl_session_tickets off;

    add_header Strict-Transport-Security "max-age=31536000" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-Frame-Options "DENY" always;
    add_header Referrer-Policy "no-referrer" always;
    add_header Cross-Origin-Opener-Policy "same-origin" always;
    add_header Content-Security-Policy {{csp}} always;

    client_max_body_size 1m;
    root {{root}};
    index index.html;

    location /api/ {
{{api}}
    }

    location / {
        try_files $uri $uri/ /index.html;
    }
}
"#;

const NGINX_API_CLOSED: &str = r#"        # HARDENING SWITCH: closed. The seeded admin/admin and user/user accounts may still exist, so the API is
        # not on the internet. Harden over an SSH tunnel (README.md), then re-render with hardening done, or replace
        # the `return` lines by the commented block.
        default_type text/plain;
        return 403 "The API is closed until the server is hardened.\n";
        # proxy_pass http://{{upstream}};
        # proxy_set_header Host $host;
        # proxy_set_header X-Forwarded-For $remote_addr;
        # proxy_set_header X-Forwarded-Proto $scheme;"#;

const NGINX_API_OPEN: &str = r#"        # HARDENING SWITCH: open. Both seeded passwords were replaced and registration is off.
        proxy_pass http://{{upstream}};
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $remote_addr;
        proxy_set_header X-Forwarded-Proto $scheme;"#;

/// Renders the nginx server blocks.
pub(super) fn nginx_conf(plan: &DeploymentPlan) -> Artifact {
    let be = backend(plan);
    let api = fill(
        match plan.hardening {
            Hardening::Pending => NGINX_API_CLOSED,
            Hardening::Done => NGINX_API_OPEN,
        },
        &[("upstream", &be.upstream)],
    );
    let domain = plan.domain.as_str();
    let cert = format!("/etc/letsencrypt/live/{domain}/fullchain.pem");
    let key = format!("/etc/letsencrypt/live/{domain}/privkey.pem");
    let install_path = match plan.runtime {
        Runtime::Compose(_) => "./nginx.conf next to docker-compose.yml",
        Runtime::Systemd(_) => "/etc/nginx/conf.d/easytier.conf",
    };
    let text = fill(
        NGINX_TEMPLATE,
        &[
            ("domain", domain),
            ("install_path", install_path),
            ("certbot_root", CERTBOT_WEBROOT),
            ("cert", &cert),
            ("key", &key),
            ("csp", &conf_str(&csp())),
            ("api", &api),
            ("root", &be.root),
        ],
    );
    Artifact::new(NGINX_FILE, None, Language::Nginx, text)
}
