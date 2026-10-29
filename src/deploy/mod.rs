//! Server deployment artifacts rendered from a typed [`DeploymentPlan`]: Docker Compose or systemd, a Caddy or nginx
//! reverse proxy, firewall scripts and a README with the steps in order.
//!
//! Pure rendering. Nothing is applied, downloaded or written here; the page shows the [`Artifact`]s and the user
//! copies or saves them. Compiles on every target, the web build included.
//!
//! Security stance baked into the output (`rule://easytier-config-security`):
//! - images are pinned by digest ([`images`]); release zips are checked against a sha256 before they are unzipped;
//! - the REST API is never published beyond the host's loopback / the compose network, the RPC portal never;
//! - until [`Hardening::Done`] the proxy answers 403 on `/api/`, registration is open only so the owner account
//!   can be created through an SSH tunnel, and the config server is not opened in the firewall;
//! - the enrollment URL lives in a mode-0600 env file, never on a command line.

mod compose;
mod env;
pub mod error;
mod firewall;
pub mod images;
pub mod plan;
mod proxy;
mod readme;
mod systemd;
mod template;

pub use env::PRIVATE_MODE;
pub use error::{DeployError, Field, Reason};
pub use plan::{
    AbsPath, AssetUrl, ComposeImage, ConfigProtocol, DEFAULT_API_PORT, DEFAULT_CONFIG_PORT,
    DeploymentPlan, Email, Hardening, Hostname, ImageTag, LinuxArch, NodeService, OwnerName, Paths,
    ProxyKind, RPC_PORTAL_PORT, ReleaseVersion, ResourceName, Runtime, Sha256Hex, SystemdRelease,
};
pub use template::{Artifact, Language};

/// Renders every artifact for `plan`, in the order the README applies them: runtime files, node env, proxy config,
/// firewall scripts, README.
///
/// # Errors
///
/// [`DeployError`] naming the plan field at fault; no artifact is produced then.
pub fn render(plan: &DeploymentPlan) -> Result<Vec<Artifact>, DeployError> {
    plan.validate()?;
    let mut artifacts = Vec::new();
    match &plan.runtime {
        Runtime::Compose(image) => {
            artifacts.push(compose::compose_file(plan, image));
            artifacts.extend(env::node_env(
                plan,
                compose::ENV_FILE,
                "Next to docker-compose.yml, chmod 600.",
            ));
        }
        Runtime::Systemd(release) => {
            artifacts.push(systemd::install_script(release));
            artifacts.push(systemd::web_unit(plan, release));
            if let Some((unit, env)) = systemd::core_unit(plan, release) {
                artifacts.push(unit);
                artifacts.push(env);
            }
        }
    }
    artifacts.push(match plan.proxy {
        ProxyKind::Caddy => proxy::caddyfile(plan),
        ProxyKind::Nginx => proxy::nginx_conf(plan),
    });
    artifacts.push(firewall::ufw(plan));
    artifacts.push(firewall::firewalld(plan));
    artifacts.push(readme::readme(plan));
    Ok(artifacts)
}

#[cfg(test)]
mod tests;
