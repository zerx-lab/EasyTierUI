//! EasyTier's library behind a small, safe API, for the iOS and Android builds of EasyTierUI.
//!
//! The functions below are plain Rust (the app's `embedded` backend calls them directly) and are also exported
//! through UniFFI so a host app can generate Kotlin / Swift bindings for the same surface (see the README). All
//! of them block; call them off the UI thread. Instances are identified by a caller-chosen name.

mod engine;
mod error;
mod logs;
mod redact;

pub use engine::RunningInfo;
pub use error::MobileError;
pub use logs::{LogEntry, LogLevel};

uniffi::setup_scaffolding!();

/// Validates an EasyTier TOML config without starting anything.
///
/// # Errors
///
/// [`MobileError::ConfigRejected`] with the parser's complaint and the line, never the config's content.
#[uniffi::export]
pub fn check_config(config_toml: String) -> Result<(), MobileError> {
    engine::check(&config_toml)
}

/// Starts the network instance `instance_name` from `config_toml`.
///
/// The config's own `instance_name` is replaced by `instance_name`. With `no_tun` the instance never uses a TUN
/// device (`flags.no_tun` is forced on): subnet proxy, SOCKS5 and port forwarding work, the device itself is not
/// routed. Without it the config's flags apply and the host is expected to call [`set_tun_fd`] with the
/// descriptor of its VPN service.
///
/// # Errors
///
/// [`MobileError::InvalidName`], [`MobileError::ConfigRejected`], [`MobileError::AlreadyRunning`] (stop it
/// first), or [`MobileError::StartFailed`] with EasyTier's redacted message.
#[uniffi::export]
pub fn start_network(
    instance_name: String,
    config_toml: String,
    no_tun: bool,
) -> Result<(), MobileError> {
    engine::start(&instance_name, &config_toml, no_tun)
}

/// Enrolls to a config server as the session `instance_name`; the server pushes the networks (which run without
/// a TUN while `no_tun` is set, whatever the pushed config asks for).
///
/// `config_server` is `udp://host:22020/<account>` (`tcp`, `ws`, `wss` work too): an enrollment credential, kept
/// out of logs and error messages. `machine_id` should be a UUID, generated once per installation and kept.
/// [`running_info`] of the session reports the first pushed network.
///
/// # Errors
///
/// [`MobileError::InvalidName`], [`MobileError::AlreadyRunning`], [`MobileError::StartFailed`] (a URL the library
/// cannot use, an unsupported scheme, an empty account) with a redacted message.
#[uniffi::export]
pub fn start_managed_network(
    instance_name: String,
    config_server: String,
    machine_id: String,
    hostname: Option<String>,
    no_tun: bool,
) -> Result<(), MobileError> {
    engine::start_managed(
        &instance_name,
        &config_server,
        &machine_id,
        hostname,
        no_tun,
    )
}

/// Stops the instance and waits until its thread has ended.
///
/// # Errors
///
/// [`MobileError::NotFound`] when no instance has that name.
#[uniffi::export]
pub fn stop_network(instance_name: String) -> Result<(), MobileError> {
    engine::stop(&instance_name)
}

/// Stops every instance (the app is quitting).
///
/// # Errors
///
/// [`MobileError::Internal`] when EasyTier failed while stopping; the table is cleared regardless.
#[uniffi::export]
pub fn stop_all_networks() -> Result<(), MobileError> {
    engine::stop_all()
}

/// The names of the instances that were started and not stopped, sorted.
#[uniffi::export]
pub fn list_networks() -> Vec<String> {
    engine::list()
}

/// What the instance reports now; `None` when no instance has that name.
///
/// `running` with no `info_json` is an instance that is still starting; `running == false` is one that ended,
/// with `error_msg` when it failed.
///
/// # Errors
///
/// [`MobileError::Internal`] when a running instance did not answer.
#[uniffi::export]
pub fn running_info(instance_name: String) -> Result<Option<RunningInfo>, MobileError> {
    engine::running_info(&instance_name)
}

/// Hands the TUN device descriptor of the host's VPN service (Android `VpnService`, iOS packet tunnel) to the
/// instance; EasyTier takes ownership of the descriptor. A value of zero or less detaches the current device.
///
/// # Errors
///
/// [`MobileError::NotFound`] when no instance has that name, [`MobileError::TunFd`] when it is not ready yet.
#[uniffi::export]
pub fn set_tun_fd(instance_name: String, fd: i32) -> Result<(), MobileError> {
    engine::set_tun_fd(&instance_name, fd)
}

/// The newest captured log lines (info and above) of every instance, oldest first, secrets masked.
///
/// The lines of all instances share one buffer: EasyTier's log events carry no instance identity.
#[uniffi::export]
pub fn collect_logs() -> Vec<LogEntry> {
    logs::snapshot()
}

/// EasyTier's version string, for example `2.6.4-8428a89d`.
#[uniffi::export]
pub fn version() -> String {
    engine::easytier_version()
}
