//! The argument vectors the sidecar hands to the official programs. Pure functions: spawning and elevation live
//! elsewhere, so tests assert exactly what a process list would show.
//!
//! Rules (`rule://easytier-config-security`): the RPC portal is always `127.0.0.1:<port>` with a loopback whitelist,
//! config files are passed by path, `--disable-env-parsing` is always present, and no secret is ever an argument. The
//! one enrollment credential a managed instance needs, the config-server URL, travels in [`ChildLaunch::config_server`]
//! and is set as the environment variable `ET_CONFIG_SERVER` on that one child.

use std::{ffi::OsString, path::Path};

use crate::core::{node::NodeError, secret::Secret};

/// Who may reach the RPC portal: this machine only, both families.
pub const RPC_WHITELIST: &str = "127.0.0.1/32,::1/128";

/// The environment variable `easytier-core` reads the config-server URL from.
pub const CONFIG_SERVER_ENV: &str = "ET_CONFIG_SERVER";

/// The prefix of every variable `easytier-core` and `easytier-cli` read their options from.
pub const CORE_ENV_PREFIX: &str = "ET_";

/// The console log level of a child process.
const CONSOLE_LOG_LEVEL: &str = "info";

/// The file log level of an OS service.
const FILE_LOG_LEVEL: &str = "info";

/// What a child process is started with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildLaunch {
    /// The program's arguments (never a secret).
    pub args: Vec<OsString>,
    /// The enrollment URL of a managed instance, to be set as [`CONFIG_SERVER_ENV`] on the child only.
    pub config_server: Option<Secret>,
}

/// `127.0.0.1:<port>`: the only address the portal binds and the CLI dials.
pub fn rpc_portal(port: u16) -> String {
    format!("127.0.0.1:{port}")
}

fn os(text: &str) -> OsString {
    OsString::from(text)
}

/// The flags every launch shares: the loopback portal with its whitelist and no environment expansion in configs.
fn portal_args(port: u16) -> Vec<OsString> {
    vec![
        os("--rpc-portal"),
        OsString::from(rpc_portal(port)),
        os("--rpc-portal-whitelist"),
        os(RPC_WHITELIST),
        os("--disable-env-parsing"),
    ]
}

/// Arguments of a local instance running as the user's child: `-c <config>` plus the portal flags.
pub fn local_child(config: &Path, port: u16) -> ChildLaunch {
    let mut args = vec![os("-c"), config.as_os_str().to_owned()];
    args.extend(portal_args(port));
    args.extend([os("--console-log-level"), os(CONSOLE_LOG_LEVEL)]);
    ChildLaunch {
        args,
        config_server: None,
    }
}

/// Arguments of a managed instance (always the user's child, so always without a TUN): the persisted machine id,
/// the optional hostname, and the config-server URL in `config_server` only.
///
/// # Errors
///
/// [`NodeError::Other`] when the machine id or hostname could not be a command-line value.
pub fn managed_child(
    config_server: &Secret,
    machine_id: &str,
    hostname: Option<&str>,
    port: u16,
) -> Result<ChildLaunch, NodeError> {
    let id_ok = !machine_id.is_empty()
        && machine_id.len() <= 64
        && machine_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    if !id_ok {
        return Err(NodeError::Other("the machine id is not valid".to_owned()));
    }
    // `--flag=value` so a value that starts with `-` is never read as another flag.
    let mut args = vec![OsString::from(format!("--machine-id={machine_id}"))];
    if let Some(hostname) = hostname.filter(|h| !h.is_empty()) {
        if hostname.chars().any(char::is_control) {
            return Err(NodeError::Other("the hostname is not valid".to_owned()));
        }
        args.push(OsString::from(format!("--hostname={hostname}")));
    }
    // A managed child never has the privileges for a TUN; the server's pushed config may not ask for one.
    args.push(os("--no-tun"));
    args.extend(portal_args(port));
    args.extend([os("--console-log-level"), os(CONSOLE_LOG_LEVEL)]);
    Ok(ChildLaunch {
        args,
        config_server: Some(config_server.clone()),
    })
}

/// `--check-config -c <file>`: validate a rendered config without starting a network.
pub fn check_config(config: &Path) -> Vec<OsString> {
    vec![
        os("--check-config"),
        os("-c"),
        config.as_os_str().to_owned(),
        os("--disable-env-parsing"),
    ]
}

/// The OS service name of an instance: `easytierui-<key>`.
pub fn service_name(key: &str) -> String {
    format!("easytierui-{key}")
}

/// The service-manager subcommands that take no further arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceVerb {
    /// `service start`.
    Start,
    /// `service stop`.
    Stop,
    /// `service uninstall`.
    Uninstall,
    /// `service status`.
    Status,
}

impl ServiceVerb {
    fn word(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Uninstall => "uninstall",
            Self::Status => "status",
        }
    }
}

/// `service --name <svc> <verb>`: the `--name` option belongs to `service`, before the subcommand.
pub fn service_verb(service: &str, verb: ServiceVerb) -> Vec<OsString> {
    vec![os("service"), os("--name"), os(service), os(verb.word())]
}

/// What `service install` registers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceInstall<'a> {
    /// The OS service name.
    pub service: &'a str,
    /// The shown name.
    pub display_name: &'a str,
    /// The core binary the service runs.
    pub core: &'a Path,
    /// The instance directory (the service's working directory).
    pub work_dir: &'a Path,
    /// The rendered config.
    pub config: &'a Path,
    /// The log directory the service writes to.
    pub log_dir: &'a Path,
    /// The loopback portal port.
    pub port: u16,
    /// Whether the OS starts the service at boot.
    pub autostart: bool,
}

/// `service --name <svc> install … -- <core args>`: the arguments of the elevated `easytier-cli`.
pub fn service_install(install: &ServiceInstall<'_>) -> Vec<OsString> {
    let mut args = vec![
        os("service"),
        os("--name"),
        os(install.service),
        os("install"),
        os("--description"),
        os("EasyTier network node managed by EasyTierUI"),
        os("--display-name"),
        os(install.display_name),
        os("--disable-autostart"),
        os(if install.autostart { "false" } else { "true" }),
        os("--core-path"),
        install.core.as_os_str().to_owned(),
        os("--service-work-dir"),
        install.work_dir.as_os_str().to_owned(),
        os("--"),
        os("-c"),
        install.config.as_os_str().to_owned(),
    ];
    args.extend(portal_args(install.port));
    args.extend([
        os("--file-log-dir"),
        install.log_dir.as_os_str().to_owned(),
        os("--file-log-level"),
        os(FILE_LOG_LEVEL),
    ]);
    args
}

/// Characters a systemd `ExecStart=` line (which `easytier-cli service install` joins with plain spaces, unquoted)
/// would split or expand.
const UNSAFE_IN_UNIT_LINE: [char; 9] = [' ', '\t', '"', '\'', '\\', '%', '$', ';', '`'];

/// Whether `install`'s paths survive the service manager. Where the manager writes the arguments into a unit file
/// unquoted (`strict`: Linux), a path with a space, quote, `%` or `$` would split or expand there; refuse it before
/// anything is installed. Launchd and the Windows SCM keep each argument whole.
///
/// # Errors
///
/// [`NodeError::Service`] naming the problem.
pub fn check_service_paths(install: &ServiceInstall<'_>, strict: bool) -> Result<(), NodeError> {
    if !strict {
        return Ok(());
    }
    let plain = [
        install.core,
        install.work_dir,
        install.config,
        install.log_dir,
    ]
    .iter()
    .all(|path| {
        path.to_str().is_some_and(|text| {
            !text
                .chars()
                .any(|c| UNSAFE_IN_UNIT_LINE.contains(&c) || c.is_control())
        })
    });
    if plain {
        Ok(())
    } else {
        Err(NodeError::Service(
            "the app's data directory has a space or special character in its path; the system service manager \
             cannot pass such a path safely"
                .to_owned(),
        ))
    }
}

/// What the sidecar reads through `easytier-cli`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliRead {
    /// `node`: this node.
    Node,
    /// `-v peer`: the raw peer-route pairs. Without `-v` the CLI rounds byte counters to three significant digits
    /// (`"1.47 kB"`), which turns a rate over a two-second step into zeros and spikes.
    Peers,
}

impl CliRead {
    /// Its name in logs.
    pub fn name(self) -> &'static str {
        match self {
            CliRead::Node => "node",
            CliRead::Peers => "peer",
        }
    }
}

/// `-p 127.0.0.1:<port> -o json [-v] <subcommand>`: a read of the running node through the CLI.
pub fn cli_read(port: u16, read: CliRead) -> Vec<OsString> {
    let mut args = vec![
        os("-p"),
        OsString::from(rpc_portal(port)),
        os("-o"),
        os("json"),
    ];
    if read == CliRead::Peers {
        args.push(os("-v"));
    }
    args.push(os(read.name()));
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    fn has_pair(args: &[String], flag: &str, value: &str) -> bool {
        args.windows(2).any(|w| {
            w.first().map(String::as_str) == Some(flag)
                && w.get(1).map(String::as_str) == Some(value)
        })
    }

    fn assert_loopback_only(args: &[String]) {
        assert!(
            has_pair(args, "--rpc-portal", "127.0.0.1:41500"),
            "{args:?}"
        );
        assert!(has_pair(
            args,
            "--rpc-portal-whitelist",
            "127.0.0.1/32,::1/128"
        ));
        assert!(args.iter().any(|a| a == "--disable-env-parsing"));
        assert!(!args.iter().any(|a| a.contains("0.0.0.0")));
    }

    #[test]
    fn local_child_passes_the_config_by_path_and_the_portal_on_loopback() {
        let launch = local_child(Path::new("/data/instances/p-a1/config.toml"), 41500);
        let args = text(&launch.args);
        assert!(has_pair(&args, "-c", "/data/instances/p-a1/config.toml"));
        assert!(has_pair(&args, "--console-log-level", "info"));
        assert_loopback_only(&args);
        assert!(launch.config_server.is_none());
    }

    #[test]
    fn managed_child_keeps_the_config_server_out_of_argv() {
        let url = Secret::new("udp://config.example:22020/example-user-do-not-use");
        let launch = managed_child(
            &url,
            "00000000-0000-0000-0000-000000000001",
            Some("example-host-m"),
            41500,
        )
        .unwrap();
        let args = text(&launch.args);
        assert_loopback_only(&args);
        assert!(
            args.iter()
                .any(|a| a == "--machine-id=00000000-0000-0000-0000-000000000001")
        );
        assert!(args.iter().any(|a| a == "--hostname=example-host-m"));
        assert!(args.iter().any(|a| a == "--no-tun"));
        assert!(!args.iter().any(|a| a == "-c"));
        for arg in &args {
            assert!(!arg.contains("config.example"), "{arg}");
            assert!(!arg.contains("example-user"), "{arg}");
            assert!(!arg.to_ascii_lowercase().contains("config-server"), "{arg}");
        }
        assert_eq!(launch.config_server.as_ref(), Some(&url));
        assert_eq!(CONFIG_SERVER_ENV, "ET_CONFIG_SERVER");
        assert!(!format!("{launch:?}").contains("config.example"));
    }

    #[test]
    fn managed_child_rejects_values_that_are_not_ids_or_names() {
        let url = Secret::new("udp://x:1/y");
        assert!(managed_child(&url, "", None, 1).is_err());
        assert!(managed_child(&url, "a b", None, 1).is_err());
        assert!(managed_child(&url, "id", Some("bad\nname"), 1).is_err());
        let launch = managed_child(&url, "id", Some("--evil"), 1).unwrap();
        assert!(text(&launch.args).iter().any(|a| a == "--hostname=--evil"));
    }

    #[test]
    fn check_config_never_starts_a_network() {
        let args = text(&check_config(Path::new("/tmp/c.toml")));
        assert_eq!(
            args,
            [
                "--check-config",
                "-c",
                "/tmp/c.toml",
                "--disable-env-parsing"
            ]
        );
    }

    #[test]
    fn service_install_puts_name_before_the_subcommand_and_core_args_after_the_separator() {
        let install = ServiceInstall {
            service: "easytierui-p-a1",
            display_name: "EasyTierUI p-a1",
            core: Path::new("/data/cores/2.6.4/easytier-core"),
            work_dir: Path::new("/data/instances/p-a1"),
            config: Path::new("/data/instances/p-a1/config.toml"),
            log_dir: Path::new("/data/instances/p-a1/logs"),
            port: 41500,
            autostart: false,
        };
        let args = text(&service_install(&install));
        assert_eq!(
            args.get(..4).map(<[String]>::to_vec),
            Some(vec![
                "service".to_owned(),
                "--name".to_owned(),
                "easytierui-p-a1".to_owned(),
                "install".to_owned()
            ])
        );
        assert!(has_pair(&args, "--disable-autostart", "true"));
        assert!(has_pair(
            &args,
            "--core-path",
            "/data/cores/2.6.4/easytier-core"
        ));
        assert!(has_pair(
            &args,
            "--service-work-dir",
            "/data/instances/p-a1"
        ));
        let separator = args.iter().position(|a| a == "--").unwrap();
        let (cli_part, core_part) = args.split_at(separator);
        assert!(!cli_part.iter().any(|a| a == "-c"));
        assert!(has_pair(
            core_part,
            "-c",
            "/data/instances/p-a1/config.toml"
        ));
        assert!(has_pair(
            core_part,
            "--file-log-dir",
            "/data/instances/p-a1/logs"
        ));
        assert!(has_pair(core_part, "--file-log-level", "info"));
        assert_loopback_only(core_part);
        let enabled = text(&service_install(&ServiceInstall {
            autostart: true,
            ..install
        }));
        assert!(has_pair(&enabled, "--disable-autostart", "false"));
    }

    #[test]
    fn paths_a_unit_file_would_split_are_refused_where_it_matters() {
        let good = ServiceInstall {
            service: "s",
            display_name: "d",
            core: Path::new("/home/jo/.local/share/EasyTierUI/cores/2.6.4/easytier-core"),
            work_dir: Path::new("/home/jo/.local/share/EasyTierUI/instances/p-a1"),
            config: Path::new("/home/jo/.local/share/EasyTierUI/instances/p-a1/config.toml"),
            log_dir: Path::new("/home/jo/.local/share/EasyTierUI/instances/p-a1/logs"),
            port: 1,
            autostart: false,
        };
        assert!(check_service_paths(&good, true).is_ok());
        let spaced = ServiceInstall {
            config: Path::new("/home/jo doe/.local/share/EasyTierUI/instances/p-a1/config.toml"),
            ..good.clone()
        };
        assert!(check_service_paths(&spaced, true).is_err());
        assert!(check_service_paths(&spaced, false).is_ok());
        let percent = ServiceInstall {
            work_dir: Path::new("/home/jo/100%/p-a1"),
            ..good
        };
        assert!(check_service_paths(&percent, true).is_err());
    }

    #[test]
    fn service_verbs_and_cli_reads() {
        assert_eq!(
            text(&service_verb("easytierui-p-a1", ServiceVerb::Stop)),
            ["service", "--name", "easytierui-p-a1", "stop"]
        );
        assert_eq!(
            text(&cli_read(41500, CliRead::Peers)),
            ["-p", "127.0.0.1:41500", "-o", "json", "-v", "peer"]
        );
        assert_eq!(
            text(&cli_read(41500, CliRead::Node)),
            ["-p", "127.0.0.1:41500", "-o", "json", "node"]
        );
        assert_eq!(service_name("p-a1"), "easytierui-p-a1");
    }
}
