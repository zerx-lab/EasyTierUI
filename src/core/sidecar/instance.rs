//! One instance's directory under `<data dir>/instances/<key>/`: `config.toml` (owner-only), `meta.json` (RPC port,
//! privilege, last config digest, service bookkeeping) and `logs/`.
//!
//! The RPC port is chosen once (a free loopback port, skipping the ports other instances persisted) and kept, so a
//! service that outlives the app is found again at the same address.

use std::{
    fs,
    io::{self, Read as _, Seek as _, SeekFrom},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::redact::Redactor;
use crate::core::{
    node::{InstanceKey, NodeError, Privilege},
    paths,
    state::{LogLine, LogStream},
};

/// The most log lines `logs` returns.
pub const MAX_LOG_LINES: usize = 500;

/// How much of the end of a log file is read.
const LOG_TAIL_BYTES: u64 = 256 * 1024;

/// How many free ports are tried before giving up.
const PORT_ATTEMPTS: usize = 32;

/// The files of one instance directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstancePaths {
    /// `<root>/<key>`.
    pub dir: PathBuf,
    /// `config.toml` (owner-only).
    pub config: PathBuf,
    /// `meta.json`.
    pub meta: PathBuf,
    /// `logs/`.
    pub logs: PathBuf,
}

impl InstancePaths {
    /// The layout of `key` under `root`.
    pub fn new(root: &Path, key: &InstanceKey) -> Self {
        let dir = root.join(key.as_str());
        Self {
            config: dir.join("config.toml"),
            meta: dir.join("meta.json"),
            logs: dir.join("logs"),
            dir,
        }
    }

    /// Creates the directory and `logs/` owner-only.
    ///
    /// # Errors
    ///
    /// [`NodeError::Io`] when either cannot be created.
    pub fn create(&self) -> Result<(), NodeError> {
        paths::ensure_private_dir(&self.dir).map_err(io_error("the instance directory"))?;
        paths::ensure_private_dir(&self.logs).map_err(io_error("the log directory"))
    }
}

/// An [`io::Error`] as the UI-facing [`NodeError::Io`], saying which file.
pub fn io_error(what: &'static str) -> impl Fn(io::Error) -> NodeError {
    move |error| NodeError::Io(format!("{what}: {error}"))
}

/// What the instance's `meta.json` remembers.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Meta {
    /// The loopback RPC portal port; `0` until chosen.
    pub rpc_port: u16,
    /// How the instance was last started.
    pub privilege: Option<MetaPrivilege>,
    /// SHA-256 of the config that was last started.
    pub config_digest: Option<String>,
    /// Whether an OS service for this instance was installed by a successful install.
    pub service_installed: bool,
    /// Identifies what the installed service was registered with (core path, port, directory, autostart): a
    /// different value means `install` must run again.
    pub service_fingerprint: Option<String>,
    /// Identifies the files last copied into the service's administrator-only directory (the core's, the CLI's and
    /// the config's sha256): a different value means they are copied again.
    pub service_files_digest: Option<String>,
}

/// [`Privilege`] as stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetaPrivilege {
    /// A child of the app.
    User,
    /// An OS service.
    System,
}

impl From<Privilege> for MetaPrivilege {
    fn from(privilege: Privilege) -> Self {
        match privilege {
            Privilege::User => Self::User,
            Privilege::System => Self::System,
        }
    }
}

/// Reads `meta.json`; a missing or damaged file is the default (the port is chosen again).
pub fn read_meta(paths: &InstancePaths) -> Meta {
    match fs::read_to_string(&paths.meta) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|error| {
            log::warn!(
                "{} is damaged, starting over: {error}",
                paths.meta.display()
            );
            Meta::default()
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Meta::default(),
        Err(error) => {
            log::warn!("could not read {}: {error}", paths.meta.display());
            Meta::default()
        }
    }
}

/// Writes `meta.json` atomically, owner-only.
///
/// # Errors
///
/// [`NodeError::Io`] when it cannot be written.
pub fn write_meta(paths: &InstancePaths, meta: &Meta) -> Result<(), NodeError> {
    let bytes = serde_json::to_vec_pretty(meta)
        .map_err(|error| NodeError::Io(format!("meta.json: {error}")))?;
    paths::write_private_file(&paths.meta, &bytes).map_err(io_error("meta.json"))
}

/// Every instance's persisted port, but `own`'s.
fn ports_of_others(root: &Path, own: &InstanceKey) -> Vec<u16> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy() != own.as_str())
        .filter_map(|entry| fs::read_to_string(entry.path().join("meta.json")).ok())
        .filter_map(|text| serde_json::from_str::<Meta>(&text).ok())
        .map(|meta| meta.rpc_port)
        .filter(|port| *port != 0)
        .collect()
}

/// A free loopback port nobody else persisted: bind `127.0.0.1:0`, read the port, release it. Another process can
/// still take it before the core binds; the core then fails to start and the user sees why.
///
/// # Errors
///
/// [`NodeError::Io`] when no loopback port can be bound.
pub fn pick_free_port(taken: &[u16]) -> Result<u16, NodeError> {
    let mut last = None;
    for _ in 0..PORT_ATTEMPTS {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .map_err(io_error("a loopback port"))?;
        match listener.local_addr() {
            Ok(SocketAddr::V4(addr)) if !taken.contains(&addr.port()) => return Ok(addr.port()),
            Ok(other) => last = Some(other.port()),
            Err(error) => return Err(io_error("a loopback port")(error)),
        }
    }
    Err(NodeError::Io(format!(
        "no free loopback port found (last tried {})",
        last.unwrap_or_default()
    )))
}

/// The instance's RPC port: the persisted one, or a newly picked one that is persisted now.
///
/// # Errors
///
/// [`NodeError::Io`] when a port cannot be picked or `meta.json` cannot be written.
pub fn ensure_port(
    root: &Path,
    paths: &InstancePaths,
    key: &InstanceKey,
) -> Result<(u16, Meta), NodeError> {
    let mut meta = read_meta(paths);
    if meta.rpc_port == 0 {
        meta.rpc_port = pick_free_port(&ports_of_others(root, key))?;
        write_meta(paths, &meta)?;
    }
    Ok((meta.rpc_port, meta))
}

/// Lower-case hex SHA-256 of `bytes`.
pub fn digest_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut out, byte| {
            // Writing to a String cannot fail.
            if write!(out, "{byte:02x}").is_err() {
                log::debug!("formatting a digest failed");
            }
            out
        })
}

/// The newest regular file in `logs`, if any.
fn newest_log(logs: &Path) -> Option<(PathBuf, f64)> {
    let entries = fs::read_dir(logs).ok()?;
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            metadata.is_file().then_some(())?;
            let modified = metadata
                .modified()
                .ok()?
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_secs_f64()
                * 1000.0;
            Some((entry.path(), modified))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)))
}

/// The last [`MAX_LOG_LINES`] lines of the newest file in `logs`, redacted. Empty when there is none.
pub fn tail_log_files(logs: &Path, redactor: &Redactor) -> Vec<LogLine> {
    let Some((path, modified_ms)) = newest_log(logs) else {
        return Vec::new();
    };
    match read_tail(&path) {
        Ok(text) => tail_lines(&text, redactor, modified_ms),
        Err(error) => {
            log::warn!("could not read {}: {error}", path.display());
            Vec::new()
        }
    }
}

fn read_tail(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let length = file.metadata()?.len();
    let start = length.saturating_sub(LOG_TAIL_BYTES);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(LOG_TAIL_BYTES).read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if start == 0 {
        return Ok(text);
    }
    // The cut is mid-line: drop the partial first line.
    Ok(text
        .split_once('\n')
        .map_or_else(String::new, |(_, rest)| rest.to_owned()))
}

/// The last [`MAX_LOG_LINES`] non-empty lines of `text`, redacted, stamped `at_ms`.
pub fn tail_lines(text: &str, redactor: &Redactor, at_ms: f64) -> Vec<LogLine> {
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let skip = lines.len().saturating_sub(MAX_LOG_LINES);
    lines
        .into_iter()
        .skip(skip)
        .map(|line| LogLine {
            at_ms,
            stream: LogStream::Stdout,
            text: redactor.redact(line),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("etui-sidecar-{name}-{}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).unwrap();
        }
        paths::ensure_private_dir(&dir).unwrap();
        dir
    }

    fn key(id: &str) -> InstanceKey {
        InstanceKey::for_profile(id).unwrap()
    }

    #[test]
    fn layout_follows_the_key() {
        let paths = InstancePaths::new(Path::new("/d/instances"), &key("a1"));
        assert_eq!(paths.dir, Path::new("/d/instances/p-a1"));
        assert_eq!(paths.config, Path::new("/d/instances/p-a1/config.toml"));
        assert_eq!(paths.meta, Path::new("/d/instances/p-a1/meta.json"));
        assert_eq!(paths.logs, Path::new("/d/instances/p-a1/logs"));
    }

    #[test]
    fn the_port_is_picked_once_and_kept() {
        let root = scratch("port");
        let paths = InstancePaths::new(&root, &key("a1"));
        paths.create().unwrap();
        let (first, _) = ensure_port(&root, &paths, &key("a1")).unwrap();
        assert_ne!(first, 0);
        let (again, meta) = ensure_port(&root, &paths, &key("a1")).unwrap();
        assert_eq!(first, again);
        assert_eq!(meta.rpc_port, first);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn instances_never_share_a_persisted_port() {
        let root = scratch("ports");
        let a = InstancePaths::new(&root, &key("a1"));
        let b = InstancePaths::new(&root, &key("b2"));
        a.create().unwrap();
        b.create().unwrap();
        let (port_a, _) = ensure_port(&root, &a, &key("a1")).unwrap();
        let (port_b, _) = ensure_port(&root, &b, &key("b2")).unwrap();
        assert_ne!(port_a, port_b);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn meta_round_trips_and_damage_falls_back_to_default() {
        let root = scratch("meta");
        let paths = InstancePaths::new(&root, &key("a1"));
        paths.create().unwrap();
        let meta = Meta {
            rpc_port: 41500,
            privilege: Some(MetaPrivilege::System),
            config_digest: Some(digest_hex(b"x")),
            service_installed: true,
            service_fingerprint: Some("fp".to_owned()),
            service_files_digest: Some(digest_hex(b"files")),
        };
        write_meta(&paths, &meta).unwrap();
        assert_eq!(read_meta(&paths), meta);
        fs::write(&paths.meta, "{not json").unwrap();
        assert_eq!(read_meta(&paths), Meta::default());
        fs::write(&paths.meta, r#"{"rpc_port":7,"future_field":1}"#).unwrap();
        assert_eq!(read_meta(&paths).rpc_port, 7);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn digests_are_sha256_hex() {
        assert_eq!(
            digest_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn the_newest_log_file_is_tailed_and_redacted() {
        let root = scratch("logs");
        let logs = root.join("logs");
        paths::ensure_private_dir(&logs).unwrap();
        fs::write(logs.join("old.log"), "old line\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(30));
        let mut body = String::new();
        for n in 0..600 {
            body.push_str(&format!(
                "line {n} network_secret = \"example-secret-do-not-use\"\n"
            ));
        }
        fs::write(logs.join("new.log"), body).unwrap();
        let lines = tail_log_files(
            &logs,
            &Redactor::new().with_secret("example-secret-do-not-use"),
        );
        assert_eq!(lines.len(), MAX_LOG_LINES);
        assert!(lines.last().unwrap().text.starts_with("line 599"));
        assert!(lines.iter().all(|l| !l.text.contains("example-secret")));
        assert!(tail_log_files(&root.join("missing"), &Redactor::new()).is_empty());
        fs::remove_dir_all(&root).unwrap();
    }
}
