//! The hosted control plane's settings (persisted as JSON) and the argument vector they turn into.

use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use super::error::HostError;
use crate::core::paths;

/// The REST API's default port (easytier-web's own default).
pub const DEFAULT_API_PORT: u16 = 11211;
/// The config-server's default port (easytier-web's own default).
pub const DEFAULT_CONFIG_PORT: u16 = 22020;
/// The lowest port accepted: an unprivileged child cannot bind below it on Unix.
pub const MIN_PORT: u16 = 1024;

/// The protocol machines use to reach the config server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HostProtocol {
    /// UDP, easytier-web's default.
    #[default]
    Udp,
    /// TCP.
    Tcp,
    /// WebSocket.
    Ws,
}

impl HostProtocol {
    /// The value easytier-web's protocol option takes (`udp`, `tcp`, `ws`), also what the UI shows.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Udp => "udp",
            Self::Tcp => "tcp",
            Self::Ws => "ws",
        }
    }

    /// Whether the config server listens on a TCP socket (TCP and WebSocket) rather than a UDP one.
    pub fn is_stream(self) -> bool {
        !matches!(self, Self::Udp)
    }
}

/// How the hosted control plane runs. Persisted in `<data>/web/settings.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HostSettings {
    /// The REST API's port. The API always binds `127.0.0.1`; there is no setting that widens it.
    pub api_port: u16,
    /// The config-server's port. The server binds every interface (it has no address option).
    pub config_port: u16,
    /// The config-server's protocol.
    pub protocol: HostProtocol,
    /// New accounts cannot be registered. True from the first start: the desktop host has no registration UI, and
    /// the process is only ever launched with registration open for the owner-registration window.
    pub registration_disabled: bool,
    /// Any machine that presents an unknown account name in its enrollment URL creates that account. Off unless
    /// the user turns it on in a setting that explains this; hardening never sets it.
    pub allow_auto_create_user: bool,
}

impl Default for HostSettings {
    fn default() -> Self {
        Self {
            api_port: DEFAULT_API_PORT,
            config_port: DEFAULT_CONFIG_PORT,
            protocol: HostProtocol::default(),
            registration_disabled: true,
            allow_auto_create_user: false,
        }
    }
}

/// Where the hosted control plane keeps its files: `<data>/web/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebLayout {
    /// The directory itself (owner-only).
    pub dir: PathBuf,
    /// The SQLite database: password hashes, enrolled machines, network configs.
    pub db: PathBuf,
    /// The log directory.
    pub logs: PathBuf,
    /// The settings file.
    pub settings: PathBuf,
}

impl WebLayout {
    /// The layout under `dir`.
    pub fn under(dir: PathBuf) -> Self {
        Self {
            db: dir.join("et.db"),
            logs: dir.join("logs"),
            settings: dir.join("settings.json"),
            dir,
        }
    }

    /// The layout under the app's data directory (`web/`), creating it owner-only.
    ///
    /// # Errors
    ///
    /// [`HostError::Io`] when the data directory is unavailable.
    pub fn in_data_dir() -> Result<Self, HostError> {
        let dir = paths::data_subdir("web").map_err(|error| HostError::Io(error.to_string()))?;
        Ok(Self::under(dir))
    }

    /// Creates what easytier-web writes into, owner-only: the log directory and an empty database file (so the
    /// SQLite file and the journal files it derives from it start `0600`).
    ///
    /// # Errors
    ///
    /// [`HostError::Io`] naming the path.
    pub fn prepare(&self) -> Result<(), HostError> {
        paths::ensure_private_dir(&self.dir).map_err(|error| io_error(&self.dir, &error))?;
        paths::ensure_private_dir(&self.logs).map_err(|error| io_error(&self.logs, &error))?;
        if !self.db.exists() {
            paths::write_private_file(&self.db, b"").map_err(|error| io_error(&self.db, &error))?;
        }
        Ok(())
    }
}

fn io_error(path: &Path, error: &io::Error) -> HostError {
    HostError::Io(format!("{}: {error}", path.display()))
}

impl HostSettings {
    /// Checks the ports.
    ///
    /// # Errors
    ///
    /// [`HostError::InvalidSettings`] naming the problem.
    pub fn validate(&self) -> Result<(), HostError> {
        if self.api_port < MIN_PORT || self.config_port < MIN_PORT {
            return Err(HostError::InvalidSettings("ports start at 1024"));
        }
        if self.api_port == self.config_port {
            return Err(HostError::InvalidSettings(
                "the API and the config server need different ports",
            ));
        }
        Ok(())
    }

    /// Reads `layout.settings`; a missing file is the defaults.
    ///
    /// # Errors
    ///
    /// [`HostError::SettingsCorrupt`] when the file is not settings (never silently replaced by defaults, which
    /// would reopen registration), [`HostError::Io`] when it cannot be read.
    pub fn load(layout: &WebLayout) -> Result<Self, HostError> {
        let mut settings: Self = match fs::read(&layout.settings) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| HostError::SettingsCorrupt(error.to_string()))?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Self::default(),
            Err(error) => return Err(io_error(&layout.settings, &error)),
        };
        // Files written before registration was off from the first start say `false`: never honoured.
        settings.registration_disabled = true;
        Ok(settings)
    }

    /// Writes `layout.settings` atomically, owner-only.
    ///
    /// # Errors
    ///
    /// [`HostError::Io`] naming the file.
    pub fn save(&self, layout: &WebLayout) -> Result<(), HostError> {
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| HostError::SettingsCorrupt(error.to_string()))?;
        paths::ensure_private_dir(&layout.dir).map_err(|error| io_error(&layout.dir, &error))?;
        paths::write_private_file(&layout.settings, &bytes)
            .map_err(|error| io_error(&layout.settings, &error))
    }

    /// The argument vector that starts easytier-web with these settings: the REST API on `127.0.0.1` only, the
    /// database and logs in `layout`, and `--disable-registration` unless `registration_open` (the owner window).
    /// No secret is ever an argument (easytier-web's own secrets have no UI here).
    pub fn launch_args(&self, layout: &WebLayout, registration_open: bool) -> Vec<OsString> {
        let mut args: Vec<OsString> = Vec::with_capacity(20);
        let mut flag = |name: &str, value: OsString| {
            args.push(name.into());
            args.push(value);
        };
        flag("--db", layout.db.clone().into_os_string());
        flag("--api-server-addr", "127.0.0.1".into());
        flag("--api-server-port", self.api_port.to_string().into());
        flag("--config-server-port", self.config_port.to_string().into());
        flag("--config-server-protocol", self.protocol.as_str().into());
        flag("--file-log-dir", layout.logs.clone().into_os_string());
        flag("--file-log-level", "info".into());
        flag("--console-log-level", "info".into());
        // Registration is off from the very first start. The one exception is the short owner-registration window
        // (`HostedWeb::begin_owner_registration`), the API still bound to loopback.
        if !registration_open {
            args.push("--disable-registration".into());
        }
        if self.allow_auto_create_user {
            args.push("--allow-auto-create-user".into());
        }
        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> WebLayout {
        WebLayout::under(PathBuf::from("/data/web"))
    }

    fn strings(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    fn value_of<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
        args.iter()
            .skip_while(|arg| *arg != flag)
            .nth(1)
            .map(String::as_str)
    }

    #[test]
    fn default_argv_binds_the_api_to_loopback_and_disables_registration_from_the_first_start() {
        let args = strings(&HostSettings::default().launch_args(&layout(), false));
        assert_eq!(value_of(&args, "--api-server-addr"), Some("127.0.0.1"));
        assert_eq!(value_of(&args, "--api-server-port"), Some("11211"));
        assert_eq!(value_of(&args, "--config-server-port"), Some("22020"));
        assert_eq!(value_of(&args, "--config-server-protocol"), Some("udp"));
        assert_eq!(value_of(&args, "--db"), Some("/data/web/et.db"));
        assert_eq!(value_of(&args, "--file-log-dir"), Some("/data/web/logs"));
        assert_eq!(value_of(&args, "--file-log-level"), Some("info"));
        assert_eq!(value_of(&args, "--console-log-level"), Some("info"));
        assert!(args.contains(&"--disable-registration".to_owned()));
        assert!(!args.contains(&"--allow-auto-create-user".to_owned()));
    }

    #[test]
    fn only_the_owner_window_launches_with_registration_open_and_still_on_loopback() {
        let args = strings(&HostSettings::default().launch_args(&layout(), true));
        assert!(!args.contains(&"--disable-registration".to_owned()));
        assert_eq!(value_of(&args, "--api-server-addr"), Some("127.0.0.1"));
        assert!(!args.contains(&"--allow-auto-create-user".to_owned()));
    }

    #[test]
    fn hardened_argv_adds_only_the_registration_flag() {
        let settings = HostSettings {
            registration_disabled: true,
            api_port: 12000,
            config_port: 23000,
            protocol: HostProtocol::Ws,
            ..HostSettings::default()
        };
        let args = strings(&settings.launch_args(&layout(), false));
        assert_eq!(value_of(&args, "--api-server-addr"), Some("127.0.0.1"));
        assert_eq!(value_of(&args, "--api-server-port"), Some("12000"));
        assert_eq!(value_of(&args, "--config-server-port"), Some("23000"));
        assert_eq!(value_of(&args, "--config-server-protocol"), Some("ws"));
        assert_eq!(
            args.iter()
                .filter(|arg| *arg == "--disable-registration")
                .count(),
            1
        );
        assert!(!args.contains(&"--allow-auto-create-user".to_owned()));
    }

    #[test]
    fn auto_create_user_is_passed_only_when_asked_for() {
        let settings = HostSettings {
            allow_auto_create_user: true,
            ..HostSettings::default()
        };
        let args = strings(&settings.launch_args(&layout(), false));
        assert!(args.contains(&"--allow-auto-create-user".to_owned()));
    }

    #[test]
    fn no_argument_can_carry_a_secret() {
        let forbidden = [
            "--internal-auth-token",
            "--webhook-secret",
            "--oidc-client-secret",
            "--webhook-url",
            "--oidc-issuer-url",
            "--web-server-addr",
        ];
        for settings in [
            HostSettings::default(),
            HostSettings {
                registration_disabled: true,
                allow_auto_create_user: true,
                ..HostSettings::default()
            },
        ] {
            let args = strings(&settings.launch_args(&layout(), false));
            for name in forbidden {
                assert!(!args.iter().any(|arg| arg == name), "{name} on argv");
            }
            // Every flag with a value is one of the known non-secret ones.
            let flags: Vec<&String> = args.iter().filter(|arg| arg.starts_with("--")).collect();
            assert!(flags.iter().all(|flag| {
                [
                    "--db",
                    "--api-server-addr",
                    "--api-server-port",
                    "--config-server-port",
                    "--config-server-protocol",
                    "--file-log-dir",
                    "--file-log-level",
                    "--console-log-level",
                    "--disable-registration",
                    "--allow-auto-create-user",
                ]
                .contains(&flag.as_str())
            }));
        }
    }

    #[test]
    fn settings_round_trip_through_json() {
        let settings = HostSettings {
            api_port: 12345,
            config_port: 23456,
            protocol: HostProtocol::Tcp,
            registration_disabled: true,
            allow_auto_create_user: false,
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("\"protocol\":\"tcp\""));
        let back: HostSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, settings);
    }

    #[test]
    fn missing_json_fields_take_the_safe_defaults() {
        let back: HostSettings = serde_json::from_str(r#"{"api_port":12000}"#).unwrap();
        assert_eq!(back.api_port, 12000);
        assert_eq!(back.config_port, DEFAULT_CONFIG_PORT);
        assert!(!back.allow_auto_create_user);
        let unknown: HostSettings = serde_json::from_str(r#"{"future":1}"#).unwrap();
        assert_eq!(unknown, HostSettings::default());
    }

    #[test]
    fn ports_are_validated() {
        assert!(HostSettings::default().validate().is_ok());
        let low = HostSettings {
            api_port: 80,
            ..HostSettings::default()
        };
        assert!(matches!(low.validate(), Err(HostError::InvalidSettings(_))));
        let same = HostSettings {
            config_port: DEFAULT_API_PORT,
            ..HostSettings::default()
        };
        assert!(matches!(
            same.validate(),
            Err(HostError::InvalidSettings(_))
        ));
    }

    #[test]
    fn settings_files_are_saved_loaded_and_corruption_is_an_error() {
        let dir = std::env::temp_dir().join(format!("etui-settings-{}", std::process::id()));
        let layout = WebLayout::under(dir.clone());
        assert_eq!(
            HostSettings::load(&layout).unwrap(),
            HostSettings::default()
        );
        let settings = HostSettings {
            registration_disabled: true,
            ..HostSettings::default()
        };
        settings.save(&layout).unwrap();
        assert_eq!(HostSettings::load(&layout).unwrap(), settings);
        fs::write(&layout.settings, b"{ not json").unwrap();
        assert!(matches!(
            HostSettings::load(&layout),
            Err(HostError::SettingsCorrupt(_))
        ));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn prepare_creates_an_owner_only_database_and_logs() {
        let dir = std::env::temp_dir().join(format!("etui web prepare-{}", std::process::id()));
        let layout = WebLayout::under(dir.clone());
        layout.prepare().unwrap();
        assert!(layout.db.is_file());
        assert!(layout.logs.is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&layout.db), 0o600);
            assert_eq!(mode(&layout.dir), 0o700);
            assert_eq!(mode(&layout.logs), 0o700);
        }
        fs::remove_dir_all(&dir).unwrap();
    }
}
