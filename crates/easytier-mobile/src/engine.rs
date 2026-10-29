//! The process-wide table of running instances over EasyTier's `NetworkInstanceManager`.
//!
//! Callers identify an instance by a name they choose (the app uses its per-profile key); EasyTier by UUID. The
//! engine keeps the name table. A name is either a *local* instance (a config the caller supplied, run in the
//! shared manager) or a *managed* session (enrolled to a config server, which pushes the configs; each session
//! owns its manager so its networks never mix with another's). Everything here blocks: call it off the UI thread.

use std::{
    collections::HashMap,
    future::Future,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError, mpsc},
    time::Duration,
};

use async_trait::async_trait;
use easytier::{
    common::{
        MachineIdOptions,
        config::{ConfigFileControl, ConfigLoader as _, TomlConfigLoader},
        constants::EASYTIER_VERSION,
    },
    instance_manager::NetworkInstanceManager,
    web_client::{WebClient, WebClientHooks, run_web_client},
};
use uuid::Uuid;

use crate::{error::MobileError, logs, redact};

/// The longest an instance may take to answer "what is your state": its RPC services are in-process, so this only
/// trips when the instance is wedged.
const INFO_TIMEOUT: Duration = Duration::from_secs(5);

/// The longest the runtime may take to set up a config-server session (it only parses and spawns).
const SESSION_SETUP_TIMEOUT: Duration = Duration::from_secs(10);

/// The longest instance name.
const MAX_NAME_LEN: usize = 64;

/// The longest error text kept from EasyTier's messages.
const MAX_MESSAGE_CHARS: usize = 300;

/// What `running_info` reports about one instance or session.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RunningInfo {
    /// Whether the instance (or the config-server session) is alive. `false` with an `error_msg` means it failed;
    /// without one, it ended.
    pub running: bool,
    /// Why the instance ended or failed to start, redacted.
    pub error_msg: Option<String>,
    /// EasyTier's `NetworkInstanceRunningInfo` as JSON (the schema easytier-web's `networks/info` returns): the
    /// node, peers, routes and recent events. `None` while the instance is still starting, and for a managed
    /// session until the config server has pushed a network that runs.
    pub info_json: Option<String>,
}

/// A config-server session: the client that keeps the enrollment and the manager of the networks it pushes.
struct Managed {
    #[expect(
        dead_code,
        reason = "held only so that dropping the entry ends the session's tasks"
    )]
    client: WebClient,
    manager: Arc<NetworkInstanceManager>,
}

enum Entry {
    /// A config the caller supplied, run in the engine's shared manager.
    Local(Uuid),
    /// Enrolled to a config server.
    Managed(Managed),
}

/// The engine: the shared manager, the name table and the runtime that drives async reads.
struct Engine {
    manager: Arc<NetworkInstanceManager>,
    entries: Mutex<HashMap<String, Entry>>,
    runtime: Result<tokio::runtime::Runtime, String>,
}

static ENGINE: LazyLock<Engine> = LazyLock::new(|| Engine {
    manager: Arc::new(NetworkInstanceManager::new()),
    entries: Mutex::new(HashMap::new()),
    // Two workers: the reads are in-process RPC calls, and the config-server session's tasks run here too.
    runtime: tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("easytier-mobile-rt")
        .enable_all()
        .build()
        .map_err(|error| error.to_string()),
});

impl Engine {
    fn entries(&self) -> MutexGuard<'_, HashMap<String, Entry>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn runtime(&self) -> Result<&tokio::runtime::Runtime, MobileError> {
        self.runtime
            .as_ref()
            .map_err(|message| MobileError::Internal {
                message: format!("the runtime could not start: {message}"),
            })
    }
}

/// Runs `f`, turning a panic inside EasyTier into [`MobileError::Internal`]: the app must survive a bug in a
/// library it cannot patch, and the name table stays consistent because every mutation is after the call.
fn guarded<T>(what: &str, f: impl FnOnce() -> Result<T, MobileError>) -> Result<T, MobileError> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| {
        Err(MobileError::Internal {
            message: format!("EasyTier panicked while {what}"),
        })
    })
}

/// Runs `future` on the engine's runtime and waits for its output here (the caller is not inside a runtime).
fn run_on_runtime<T: Send + 'static>(
    future: impl Future<Output = T> + Send + 'static,
    wait: Duration,
) -> Result<T, MobileError> {
    let runtime = ENGINE.runtime()?;
    let (sender, receiver) = mpsc::channel();
    runtime.spawn(async move {
        // The receiver is gone only when the caller gave up waiting.
        drop(sender.send(future.await));
    });
    receiver
        .recv_timeout(wait)
        .map_err(|_| MobileError::Internal {
            message: "EasyTier did not answer in time".to_owned(),
        })
}

fn validate_name(name: &str) -> Result<(), MobileError> {
    let reason = if name.is_empty() {
        "it is empty"
    } else if name.len() > MAX_NAME_LEN {
        "it is longer than 64 characters"
    } else if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        "only letters, digits, '-' and '_' are allowed"
    } else {
        return Ok(());
    };
    Err(MobileError::InvalidName {
        reason: reason.to_owned(),
    })
}

/// A short, secret-free rendering of a parser error: its headline and its last line (the reason), never the
/// source snippet between them, which quotes the config.
fn summarize(error_text: &str, secrets: &[String]) -> String {
    let mut lines = error_text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let first = lines.next().unwrap_or("unknown error");
    let text = match lines.next_back() {
        Some(last) => format!("{first}: {last}"),
        None => first.to_owned(),
    };
    let clipped: String = text.chars().take(MAX_MESSAGE_CHARS).collect();
    redact::scrub_with(&clipped, secrets)
}

/// A config parsed twice: as plain TOML (to find its secrets and to report syntax errors with a position) and by
/// EasyTier (to get the loader). EasyTier's own error echoes the whole file, hence the pre-parse.
struct Parsed {
    loader: TomlConfigLoader,
    secrets: Vec<String>,
}

fn parse(config_toml: &str) -> Result<Parsed, MobileError> {
    let table = config_toml.parse::<toml::Table>().map_err(|error| {
        let line = error
            .span()
            .and_then(|span| config_toml.get(..span.start))
            .map(|before| {
                before
                    .bytes()
                    .filter(|byte| *byte == b'\n')
                    .count()
                    .saturating_add(1)
            });
        let message = summarize(error.message(), &[]);
        MobileError::ConfigRejected {
            message: match line {
                Some(line) => format!("line {line}: {message}"),
                None => message,
            },
        }
    })?;
    let secrets = redact::secrets_in(&table);
    let loader = guarded("reading the config", || {
        TomlConfigLoader::new_from_str(config_toml).map_err(|error| MobileError::ConfigRejected {
            message: summarize(&error.root_cause().to_string(), &secrets),
        })
    })?;
    Ok(Parsed { loader, secrets })
}

/// Validates a config without starting anything.
pub(crate) fn check(config_toml: &str) -> Result<(), MobileError> {
    parse(config_toml).map(drop)
}

/// Starts the local instance `name`.
pub(crate) fn start(name: &str, config_toml: &str, no_tun: bool) -> Result<(), MobileError> {
    validate_name(name)?;
    let Parsed { loader, secrets } = parse(config_toml)?;
    logs::install();

    guarded("preparing the instance", || {
        if no_tun {
            force_no_tun(&loader);
        }
        loader.set_inst_name(name.to_owned());
        Ok(())
    })?;

    let engine = &*ENGINE;
    let mut entries = engine.entries();
    if entries.contains_key(name) {
        return Err(MobileError::AlreadyRunning {
            name: name.to_owned(),
        });
    }

    // Registered before the instance prints its first line.
    redact::register(name, secrets.clone());
    let started = guarded("starting the instance", || {
        engine
            .manager
            .run_network_instance(loader, false, ConfigFileControl::STATIC_CONFIG)
            .map_err(|error| MobileError::StartFailed {
                message: summarize(&format!("{error:#}"), &secrets),
            })
    });
    match started {
        Ok(id) => {
            entries.insert(name.to_owned(), Entry::Local(id));
            tracing::info!(instance = name, "instance started");
            Ok(())
        }
        Err(error) => {
            redact::forget(name);
            Err(error)
        }
    }
}

fn force_no_tun(loader: &TomlConfigLoader) {
    let mut flags = loader.get_flags();
    flags.no_tun = true;
    loader.set_flags(flags);
}

/// Lets a config server's pushed networks run, minus the TUN they may ask for: the host has not provided one.
struct NoTunHook;

#[async_trait]
impl WebClientHooks for NoTunHook {
    async fn pre_run_network_instance(&self, cfg: &TomlConfigLoader) -> Result<(), String> {
        force_no_tun(cfg);
        Ok(())
    }
}

/// Enrolls to a config server as the session `name`; the server pushes the networks.
///
/// `config_server` is `udp://host:22020/<user>` (the last path segment is the account token). Both it and the
/// token are treated as secrets: registered for redaction and never logged.
pub(crate) fn start_managed(
    name: &str,
    config_server: &str,
    machine_id: &str,
    hostname: Option<String>,
    no_tun: bool,
) -> Result<(), MobileError> {
    validate_name(name)?;
    logs::install();

    let mut secrets = vec![config_server.to_owned()];
    if let Some(token) = config_server.rsplit('/').next().filter(|t| !t.is_empty()) {
        secrets.push(token.to_owned());
    }

    let engine = &*ENGINE;
    let mut entries = engine.entries();
    if entries.contains_key(name) {
        return Err(MobileError::AlreadyRunning {
            name: name.to_owned(),
        });
    }

    redact::register(name, secrets.clone());
    let manager = Arc::new(NetworkInstanceManager::new());
    let hooks: Option<Arc<dyn WebClientHooks>> = no_tun.then(|| Arc::new(NoTunHook) as _);
    let url = config_server.to_owned();
    let options = MachineIdOptions {
        explicit_machine_id: Some(machine_id.to_owned()),
        state_dir: None,
    };
    let session_manager = manager.clone();
    let created = run_on_runtime(
        async move { run_web_client(&url, options, hostname, false, session_manager, hooks).await },
        SESSION_SETUP_TIMEOUT,
    );
    match created {
        Ok(Ok(client)) => {
            entries.insert(name.to_owned(), Entry::Managed(Managed { client, manager }));
            tracing::info!(instance = name, "config-server session started");
            Ok(())
        }
        Ok(Err(error)) => {
            redact::forget(name);
            Err(MobileError::StartFailed {
                message: summarize(&format!("{error:#}"), &secrets),
            })
        }
        Err(error) => {
            redact::forget(name);
            Err(error)
        }
    }
}

/// Ends one entry: managed networks are stopped before the session is dropped; a local instance is deleted (which
/// joins its thread).
fn tear_down(entry: Entry) -> Result<(), MobileError> {
    let engine = &*ENGINE;
    guarded("stopping the instance", || {
        let result = match &entry {
            Entry::Local(id) => engine.manager.delete_network_instance(vec![*id]),
            Entry::Managed(session) => session.manager.retain_network_instance(Vec::new()),
        };
        result.map(drop).map_err(|error| MobileError::Internal {
            message: summarize(&format!("{error:#}"), &[]),
        })
    })
    // `entry` is dropped here: a managed session's client stops its tasks.
}

/// Stops the instance or session `name` and waits for its threads to end.
pub(crate) fn stop(name: &str) -> Result<(), MobileError> {
    let entry = ENGINE
        .entries()
        .remove(name)
        .ok_or_else(|| MobileError::NotFound {
            name: name.to_owned(),
        })?;
    // Outside the table's lock: dropping an instance joins its thread.
    let result = tear_down(entry);
    tracing::info!(instance = name, "instance stopped");
    redact::forget(name);
    result
}

/// Stops everything.
pub(crate) fn stop_all() -> Result<(), MobileError> {
    let drained: Vec<(String, Entry)> = ENGINE.entries().drain().collect();
    let mut first_error = None;
    for (name, entry) in drained {
        if let Err(error) = tear_down(entry) {
            first_error.get_or_insert(error);
        }
        redact::forget(&name);
    }
    first_error.map_or(Ok(()), Err)
}

/// The names of the instances and sessions that were started and not stopped, sorted.
pub(crate) fn list() -> Vec<String> {
    let mut names: Vec<String> = ENGINE.entries().keys().cloned().collect();
    names.sort();
    names
}

/// One read of an instance's running info.
struct Fetched {
    json: String,
    running: bool,
    error_msg: Option<String>,
}

/// Which instance of a manager to read.
#[derive(Clone, Copy)]
enum Query {
    One(Uuid),
    /// The first network the manager runs (a managed session's pushed network).
    First,
}

/// Reads one instance's running info on the engine's runtime.
fn fetch_info(
    manager: Arc<NetworkInstanceManager>,
    query: Query,
) -> Result<Option<Fetched>, MobileError> {
    let outcome = run_on_runtime(
        async move {
            let info = match query {
                Query::One(id) => tokio::time::timeout(INFO_TIMEOUT, manager.get_network_info(&id))
                    .await
                    .map_err(|_| "the instance did not answer in time".to_owned())?,
                Query::First => tokio::time::timeout(INFO_TIMEOUT, manager.collect_network_infos())
                    .await
                    .map_err(|_| "the instance did not answer in time".to_owned())?
                    .map_err(|error| summarize(&format!("{error:#}"), &[]))?
                    .into_values()
                    .next(),
            };
            let Some(info) = info else {
                return Ok(None);
            };
            serde_json::to_string(&info)
                .map(|json| {
                    Some(Fetched {
                        json,
                        running: info.running,
                        error_msg: info.error_msg.clone(),
                    })
                })
                .map_err(|error| format!("the running info could not be encoded: {error}"))
        },
        INFO_TIMEOUT.saturating_add(Duration::from_secs(1)),
    )?;
    outcome.map_err(|message: String| MobileError::Internal { message })
}

/// What the instance or session `name` reports now; `None` when nothing has that name.
pub(crate) fn running_info(name: &str) -> Result<Option<RunningInfo>, MobileError> {
    let engine = &*ENGINE;
    enum Target {
        Local(Uuid),
        Managed(Arc<NetworkInstanceManager>),
    }
    let target = match engine.entries().get(name) {
        Some(Entry::Local(id)) => Target::Local(*id),
        Some(Entry::Managed(session)) => Target::Managed(session.manager.clone()),
        None => return Ok(None),
    };
    match target {
        Target::Local(id) => {
            let Some((alive, error)) =
                engine
                    .manager
                    .iter()
                    .find(|entry| *entry.key() == id)
                    .map(|entry| {
                        (
                            entry.value().is_easytier_running(),
                            entry.value().get_latest_error_msg(),
                        )
                    })
            else {
                return Ok(None);
            };
            let info_json = if alive {
                fetch_info(engine.manager.clone(), Query::One(id))?.map(|fetched| fetched.json)
            } else {
                None
            };
            Ok(Some(RunningInfo {
                running: alive,
                error_msg: error.map(|message| summarize(&message, &[])),
                info_json,
            }))
        }
        Target::Managed(manager) => {
            let fetched = fetch_info(manager, Query::First)?;
            let (info_json, error_msg) = match fetched {
                Some(fetched) if fetched.running => (Some(fetched.json), None),
                Some(fetched) => (None, fetched.error_msg.map(|m| summarize(&m, &[]))),
                None => (None, None),
            };
            Ok(Some(RunningInfo {
                running: true,
                error_msg,
                info_json,
            }))
        }
    }
}

/// Hands a TUN descriptor to the local instance `name`.
pub(crate) fn set_tun_fd(name: &str, fd: i32) -> Result<(), MobileError> {
    let engine = &*ENGINE;
    let id = match engine.entries().get(name) {
        Some(Entry::Local(id)) => *id,
        Some(Entry::Managed(_)) => {
            return Err(MobileError::TunFd {
                message: "a config-server session has no TUN device of its own".to_owned(),
            });
        }
        None => {
            return Err(MobileError::NotFound {
                name: name.to_owned(),
            });
        }
    };
    guarded("handing over the TUN descriptor", || {
        engine
            .manager
            .set_tun_fd(&id, fd)
            .map_err(|error| MobileError::TunFd {
                message: summarize(&format!("{error:#}"), &[]),
            })
    })
}

/// EasyTier's version string (`2.6.4-8428a89d`).
pub(crate) fn easytier_version() -> String {
    EASYTIER_VERSION.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"
instance_name = "example"
hostname = "example-phone"
dhcp = true
listeners = []

[network_identity]
network_name = "example-net"
network_secret = "example-secret-do-not-use"

[flags]
no_tun = true
"#;

    #[test]
    fn names_are_plain_ascii() {
        assert!(validate_name("p-0f3a9c").is_ok());
        assert!(validate_name("").is_err());
        assert!(validate_name("a b").is_err());
        assert!(validate_name(&"a".repeat(65)).is_err());
    }

    #[test]
    fn a_valid_config_passes_the_check() {
        check(EXAMPLE).unwrap();
    }

    #[test]
    fn a_syntax_error_names_the_line_and_never_quotes_the_file() {
        let broken = "[network_identity]\nnetwork_secret = \"example-secret-do-not-use\"\nnetwork_name = = 1\n";
        let Err(MobileError::ConfigRejected { message }) = check(broken) else {
            panic!("expected a rejection");
        };
        assert!(message.starts_with("line 3"), "{message}");
        assert!(!message.contains("example-secret-do-not-use"));
    }

    #[test]
    fn a_type_error_never_echoes_the_secret() {
        let broken =
            "dhcp = \"yes\"\n[network_identity]\nnetwork_secret = \"example-secret-do-not-use\"\n";
        let Err(MobileError::ConfigRejected { message }) = check(broken) else {
            panic!("expected a rejection");
        };
        assert!(!message.contains("example-secret-do-not-use"), "{message}");
    }

    #[test]
    fn unknown_instances_are_reported_not_invented() {
        assert_eq!(running_info("never-started").unwrap(), None);
        assert!(matches!(
            stop("never-started"),
            Err(MobileError::NotFound { .. })
        ));
        assert!(matches!(
            set_tun_fd("never-started", 5),
            Err(MobileError::NotFound { .. })
        ));
    }

    #[test]
    fn a_no_tun_instance_starts_reports_and_stops() {
        let config = EXAMPLE.replace("listeners = []", "listeners = [\"tcp://127.0.0.1:0\"]");
        start("smoke-test", &config, true).unwrap();
        assert!(matches!(
            start("smoke-test", &config, true),
            Err(MobileError::AlreadyRunning { .. })
        ));
        assert_eq!(list(), ["smoke-test"]);
        // The instance publishes its node a moment after it starts.
        let mut info = None;
        for _ in 0..50 {
            info = running_info("smoke-test")
                .unwrap()
                .and_then(|r| r.info_json);
            if info.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let json = info.expect("the instance never published its node");
        assert!(json.contains("example-phone"), "{json}");
        stop("smoke-test").unwrap();
        assert_eq!(running_info("smoke-test").unwrap(), None);
    }
}
