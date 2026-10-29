//! The node backend of iOS and Android: EasyTier's library in this process, through the `easytier-mobile` bridge.
//!
//! Instances are `no_tun`: NSH 0.3's Rust-app packaging cannot add an Android `VpnService` or an iOS network
//! extension, so there is no TUN device to give the library (the bridge's `set_tun_fd` is for a host app that has
//! one). Subnet proxies, SOCKS5 and port forwarding work; routing the whole device does not.

use std::{
    collections::HashMap,
    sync::{Mutex, PoisonError},
};

use easytier_mobile as bridge;
use ipnet::Ipv4Net;

use super::map::{self, MapError};
use crate::core::{
    node::{InstanceKey, LaunchSource, NodeBackend, NodeError, NodeLaunch, Privilege},
    state::{FailReason, LogLine, LogStream, NetworkState},
};

/// Why a TUN node cannot run here, shown to the user as the reason of the refusal.
const NO_TUN_REASON: &str =
    "a TUN network interface needs a VPN service, which this build does not include";

/// The node backend over the in-process library.
#[derive(Debug, Default)]
pub struct EmbeddedBackend {
    /// The subnets each started local instance proxies: the running info does not carry them, the config does.
    proxies: Mutex<HashMap<InstanceKey, Vec<Ipv4Net>>>,
}

impl EmbeddedBackend {
    /// A backend with no instance started.
    pub fn new() -> Self {
        Self::default()
    }

    fn proxies(&self) -> std::sync::MutexGuard<'_, HashMap<InstanceKey, Vec<Ipv4Net>>> {
        self.proxies.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The bridge's errors in the app's terms. The bridge redacts its messages already.
fn node_error(error: bridge::MobileError) -> NodeError {
    use bridge::MobileError as E;
    match error {
        E::ConfigRejected { message } => NodeError::ConfigRejected(message),
        E::StartFailed { message } => NodeError::Spawn(message),
        E::InvalidName { reason } => NodeError::Other(format!("invalid instance name: {reason}")),
        E::AlreadyRunning { name } => NodeError::Other(format!("{name} is already running")),
        E::NotFound { name } => NodeError::Other(format!("{name} is not running")),
        E::TunFd { message } | E::Internal { message } => NodeError::Other(message),
    }
}

/// Stopping something that is not running is not a failure.
fn ignore_not_found(result: Result<(), bridge::MobileError>) -> Result<(), NodeError> {
    match result {
        Err(bridge::MobileError::NotFound { .. }) => Ok(()),
        other => other.map_err(node_error),
    }
}

fn refuse_tun(launch: &NodeLaunch) -> Result<(), NodeError> {
    match launch.privilege {
        Privilege::User => Ok(()),
        Privilege::System => Err(NodeError::Unsupported(NO_TUN_REASON)),
    }
}

impl NodeBackend for EmbeddedBackend {
    fn check(&self, launch: &NodeLaunch) -> Result<(), NodeError> {
        refuse_tun(launch)?;
        match &launch.source {
            LaunchSource::Local { config_toml } => {
                bridge::check_config(config_toml.expose().to_owned()).map_err(node_error)
            }
            // There is no config to check: the server pushes it once the session is enrolled.
            LaunchSource::Managed { .. } => Ok(()),
        }
    }

    fn start(&self, launch: &NodeLaunch) -> Result<(), NodeError> {
        refuse_tun(launch)?;
        let name = launch.key.as_str();
        // "Starting a running instance restarts it with the new config".
        ignore_not_found(bridge::stop_network(name.to_owned()))?;
        match &launch.source {
            LaunchSource::Local { config_toml } => {
                self.proxies().insert(
                    launch.key.clone(),
                    map::proxy_cidrs_from_config(config_toml.expose()),
                );
                bridge::start_network(name.to_owned(), config_toml.expose().to_owned(), true)
                    .map_err(|error| {
                        self.proxies().remove(&launch.key);
                        node_error(error)
                    })
            }
            LaunchSource::Managed {
                config_server,
                machine_id,
                hostname,
            } => {
                self.proxies().remove(&launch.key);
                bridge::start_managed_network(
                    name.to_owned(),
                    config_server.expose().to_owned(),
                    machine_id.clone(),
                    hostname.clone(),
                    true,
                )
                .map_err(node_error)
            }
        }
    }

    fn stop(&self, key: &InstanceKey) -> Result<(), NodeError> {
        ignore_not_found(bridge::stop_network(key.as_str().to_owned()))
    }

    fn remove(&self, key: &InstanceKey) -> Result<(), NodeError> {
        // An embedded instance leaves nothing on the device: no service, no config file, no log directory.
        let result = self.stop(key);
        self.proxies().remove(key);
        result
    }

    fn poll(&self, key: &InstanceKey) -> NetworkState {
        let info = match bridge::running_info(key.as_str().to_owned()) {
            Ok(Some(info)) => info,
            Ok(None) => return NetworkState::Stopped,
            Err(error) => {
                return NetworkState::Failed {
                    reason: FailReason::Unreachable(error.to_string()),
                };
            }
        };
        if !info.running {
            return match info.error_msg {
                Some(last_log) => NetworkState::Failed {
                    reason: FailReason::Exited {
                        code: None,
                        last_log,
                    },
                },
                None => NetworkState::Stopped,
            };
        }
        let Some(json) = info.info_json else {
            return NetworkState::Starting;
        };
        let proxy_cidrs = self.proxies().get(key).cloned().unwrap_or_default();
        match map::snapshot_from_json(&json, nsh_core::time::unix_ms(), proxy_cidrs) {
            Ok(snapshot) => NetworkState::Running(Box::new(snapshot)),
            Err(MapError::NoNode) => NetworkState::Starting,
            Err(error @ MapError::Parse(_)) => NetworkState::Failed {
                reason: FailReason::Other(error.to_string()),
            },
        }
    }

    fn logs(&self, _key: &InstanceKey) -> Vec<LogLine> {
        // The library's log events carry no instance identity: every instance sees the shared, redacted buffer.
        bridge::collect_logs()
            .into_iter()
            .map(|entry| LogLine {
                // Unix milliseconds are far below 2^53: the conversion is exact.
                at_ms: entry.at_ms as f64,
                stream: LogStream::App,
                text: format!("{:?} {}: {}", entry.level, entry.target, entry.message),
            })
            .collect()
    }

    fn service_installed(&self, _key: &InstanceKey) -> bool {
        false
    }

    fn shutdown(&self) {
        if let Err(error) = bridge::stop_all_networks() {
            log::warn!("stopping the embedded instances failed: {error}");
        }
        self.proxies().clear();
    }
}
