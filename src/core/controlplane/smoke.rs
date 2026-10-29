//! Smoke test against the real easytier-web binary (ignored by default).
//!
//! `cargo test -p nsh-app-easytierui controlplane::smoke -- --ignored --nocapture`
//!
//! `ET_RELEASE_DIR` names the directory with `easytier-core`, `easytier-cli` and `easytier-web` (default: the
//! 2.6.4 macOS arm64 release unzipped at `/tmp/easytier-2.6.4/easytier-macos-aarch64`). Everything runs on loopback
//! with a temporary data directory whose name contains a space, `--no-tun` style (the web server has no TUN).

use std::{
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use super::{
    ADMIN_PASSWORD_ACCOUNT, HardenRequest, HostError, HostSettings, HostState, HostedWeb,
    USER_PASSWORD_ACCOUNT, WebLayout,
    harden::{AdminApi as _, HostAccount, LoopbackAdmin},
    http::Loopback,
};
use crate::core::{
    keychain::{MemoryStore, SecretStore as _},
    node::{NodeError, ProgramSource, Programs},
    secret::Secret,
    version::CoreVersion,
};

struct Release(PathBuf);

impl ProgramSource for Release {
    fn active(&self) -> Result<Programs, NodeError> {
        let exe = |name: &str| {
            self.0
                .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
        };
        Ok(Programs {
            version: "2.6.4-8428a89d"
                .parse::<CoreVersion>()
                .map_err(|error| NodeError::Other(error.to_string()))?,
            dir: self.0.clone(),
            core: exe("easytier-core"),
            cli: exe("easytier-cli"),
            web: exe("easytier-web"),
        })
    }
}

fn free_port() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.local_addr().unwrap().port()
}

#[test]
#[ignore = "runs the real easytier-web binary on loopback"]
fn hosted_web_starts_hardens_restarts_and_stops() {
    let release = PathBuf::from(
        std::env::var("ET_RELEASE_DIR")
            .unwrap_or_else(|_| "/tmp/easytier-2.6.4/easytier-macos-aarch64".to_owned()),
    );
    let dir = std::env::temp_dir().join(format!("etui web smoke {}", std::process::id()));
    let layout = WebLayout::under(dir.clone());
    let host = HostedWeb::with_layout(Arc::new(Release(release)), Ok(layout.clone()));

    let settings = HostSettings {
        api_port: free_port(),
        config_port: free_port(),
        ..HostSettings::default()
    };
    let api_addr: SocketAddr = format!("127.0.0.1:{}", settings.api_port).parse().unwrap();
    let api = LoopbackAdmin(Loopback::new(api_addr).unwrap());

    // Start: Starting, then Running once the API answers.
    host.start(&settings).unwrap();
    assert!(matches!(
        host.state(),
        HostState::Starting | HostState::Running { .. }
    ));
    assert_eq!(host.start(&settings), Err(HostError::AlreadyRunning));
    let running = host.wait_running(Duration::from_secs(40)).unwrap();
    println!("running: {running:?}");
    assert!(matches!(running, HostState::Running { api, .. } if api == api_addr));

    // The seeded accounts sign in; registration is closed from the very first start.
    assert!(
        api.login(HostAccount::Admin, &Secret::new("admin"))
            .unwrap()
            .is_some()
    );
    assert!(
        api.login(HostAccount::User, &Secret::new("user"))
            .unwrap()
            .is_some()
    );
    assert!(api.registration_closed().unwrap());

    // Harden.
    let secrets = MemoryStore::default();
    let report = host.harden(&HardenRequest::default(), &secrets);
    println!("report: {report:?}");
    assert_eq!(report.error, None);
    assert!(report.is_complete());
    assert!(report.admin_password_stored && report.user_password_stored);
    assert!(!report.already_hardened);
    assert!(report.api_loopback_only && report.exposed_on_all_interfaces);
    assert_eq!(report.config_server_port, settings.config_port);
    assert!(host.settings().registration_disabled);
    assert!(matches!(host.state(), HostState::Running { .. }));

    let admin = secrets.get(ADMIN_PASSWORD_ACCOUNT).unwrap().unwrap();
    let user = secrets.get(USER_PASSWORD_ACCOUNT).unwrap().unwrap();
    assert!(
        api.login(HostAccount::Admin, &Secret::new("admin"))
            .unwrap()
            .is_none()
    );
    assert!(
        api.login(HostAccount::User, &Secret::new("user"))
            .unwrap()
            .is_none()
    );
    assert!(api.login(HostAccount::Admin, &admin).unwrap().is_some());
    assert!(api.login(HostAccount::User, &user).unwrap().is_some());
    assert!(api.registration_closed().unwrap());

    // The console's loopback access.
    let access = host.api_access(&secrets).unwrap();
    assert_eq!(
        access.base_url,
        format!("http://127.0.0.1:{}", settings.api_port)
    );

    // Hardening again is a no-op that recognizes the hardened database.
    let again = host.harden(&HardenRequest::default(), &secrets);
    assert_eq!(again.error, None);
    assert!(again.is_complete() && again.already_hardened);
    assert_eq!(secrets.get(ADMIN_PASSWORD_ACCOUNT).unwrap().unwrap(), admin);

    // A hardened database stays hardened across a restart (the switch-version path).
    host.restart().unwrap();
    host.wait_running(Duration::from_secs(40)).unwrap();
    assert!(api.login(HostAccount::Admin, &admin).unwrap().is_some());
    assert!(api.registration_closed().unwrap());

    // Logs were captured and carry no password.
    let logs = host.logs();
    assert!(!logs.is_empty());
    assert!(logs.iter().all(|line| !line.text.contains(admin.expose())));

    // Files are owner-only.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode =
            |path: &std::path::Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&layout.dir), 0o700);
        assert_eq!(mode(&layout.logs), 0o700);
        assert_eq!(mode(&layout.db), 0o600);
        assert_eq!(mode(&layout.settings), 0o600);
    }

    // Stop kills and waits: the API port is closed afterwards and the state is Stopped.
    host.stop().unwrap();
    assert_eq!(host.state(), HostState::Stopped);
    assert!(TcpStream::connect_timeout(&api_addr, Duration::from_millis(500)).is_err());

    // Dropping a running host leaves nothing behind either.
    host.start(&host.settings()).unwrap();
    host.wait_running(Duration::from_secs(40)).unwrap();
    drop(host);
    assert!(TcpStream::connect_timeout(&api_addr, Duration::from_millis(500)).is_err());

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
#[ignore = "runs the real easytier-web binary on loopback"]
fn owner_registration_opens_a_loopback_window_and_a_wrong_captcha_is_refused() {
    use super::{OWNER_NAME_ACCOUNT, OWNER_PASSWORD_ACCOUNT, OwnerError};

    let release = PathBuf::from(
        std::env::var("ET_RELEASE_DIR")
            .unwrap_or_else(|_| "/tmp/easytier-2.6.4/easytier-macos-aarch64".to_owned()),
    );
    let dir = std::env::temp_dir().join(format!("etui web owner {}", std::process::id()));
    let host = HostedWeb::with_layout(
        Arc::new(Release(release)),
        Ok(WebLayout::under(dir.clone())),
    );
    let settings = HostSettings {
        api_port: free_port(),
        config_port: free_port(),
        ..HostSettings::default()
    };
    let api_addr: SocketAddr = format!("127.0.0.1:{}", settings.api_port).parse().unwrap();
    let api = LoopbackAdmin(Loopback::new(api_addr).unwrap());
    host.set_settings(&settings).unwrap();
    let secrets = MemoryStore::default();

    assert!(!host.owner_registered(&secrets));
    assert!(host.owner_enrollment_url(&secrets).is_none());

    // Begin: the server runs, registration is open for this step, and a captcha PNG comes back.
    let png = host.begin_owner_registration(&secrets).unwrap();
    assert!(
        png.starts_with(b"\x89PNG"),
        "not a PNG: {} bytes",
        png.len()
    );
    assert!(!api.registration_closed().unwrap());
    // Nothing is kept before the person answers.
    assert_eq!(secrets.get(OWNER_NAME_ACCOUNT).unwrap(), None);

    // A wrong captcha is refused; the window stays open for another try and nothing is kept.
    assert_eq!(
        host.complete_owner_registration("zzzz", &secrets),
        Err(OwnerError::CaptchaRefused)
    );
    assert_eq!(secrets.get(OWNER_NAME_ACCOUNT).unwrap(), None);
    assert_eq!(secrets.get(OWNER_PASSWORD_ACCOUNT).unwrap(), None);
    assert!(!api.registration_closed().unwrap());
    assert!(!host.owner_registered(&secrets));

    // Cancelling closes the window again; nothing is pending afterwards.
    host.cancel_owner_registration().unwrap();
    host.wait_running(Duration::from_secs(40)).unwrap();
    assert!(api.registration_closed().unwrap());
    assert_eq!(
        host.complete_owner_registration("zzzz", &secrets),
        Err(OwnerError::NotStarted)
    );
    assert!(host.settings().registration_disabled);

    host.stop().unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
#[ignore = "runs the real easytier-web binary on loopback"]
fn a_taken_port_fails_the_start_with_the_port_named() {
    let release = PathBuf::from(
        std::env::var("ET_RELEASE_DIR")
            .unwrap_or_else(|_| "/tmp/easytier-2.6.4/easytier-macos-aarch64".to_owned()),
    );
    let dir = std::env::temp_dir().join(format!("etui web port {}", std::process::id()));
    let host = HostedWeb::with_layout(
        Arc::new(Release(release)),
        Ok(WebLayout::under(dir.clone())),
    );
    let held = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = held.local_addr().unwrap().port();
    let settings = HostSettings {
        api_port: port,
        config_port: free_port(),
        ..HostSettings::default()
    };
    assert!(matches!(
        host.start(&settings),
        Err(HostError::PortInUse { port: taken, .. }) if taken == port
    ));
    assert_eq!(
        host.state(),
        HostState::Failed {
            reason: HostError::PortInUse {
                which: super::HostPort::Api,
                port
            }
        }
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
