//! The owner account of the hosted control plane: a non-guessable easytier-web account that machines enroll under.
//!
//! The seeded `admin` / `user` accounts have well-known names, and the account name is the path of the enrollment
//! URL (`udp://host:22020/<name>`), so an enrollment URL ending in one of them lets anyone who can reach the config
//! port attach machines to that account. The owner is registered once, with a CSPRNG name (`owner-` and ten base32
//! characters) and password, kept in the keychain (`host/owner-name`, `host/owner-password`).
//!
//! easytier-web registers an account only after a captcha (a PNG a person reads; the answer lives in the session the
//! captcha request opened), and the hosted server starts with registration disabled. So the flow is two steps with a
//! person in between ([`HostedWeb::begin_owner_registration`](super::HostedWeb::begin_owner_registration) opens a
//! registration window on loopback and returns the PNG; [`HostedWeb::complete_owner_registration`](super::HostedWeb::complete_owner_registration)
//! submits the typed text and closes the window again). The account logic is written against [`OwnerApi`] so it is
//! tested without a process.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

use zeroize::Zeroize as _;

use super::{
    error::HostError,
    harden::{md5_hex, status_error},
    http::Loopback,
    settings::HostSettings,
};
use crate::core::{keychain::SecretStore, secret::Secret};

/// The keychain account of the owner account's name (an enrollment credential: it is the enrollment URL's path).
pub const OWNER_NAME_ACCOUNT: &str = "host/owner-name";
/// The keychain account of the owner account's password.
pub const OWNER_PASSWORD_ACCOUNT: &str = "host/owner-password";
/// What stands for this computer's address in an enrollment URL when no LAN address could be found: the person
/// replaces it with the address the other machines reach this one by.
pub const ENROLLMENT_HOST_PLACEHOLDER: &str = "<address-of-this-computer>";

/// The owner name's fixed start.
const OWNER_PREFIX: &str = "owner-";
/// Random base32 characters after the prefix: 50 bits.
const SUFFIX_CHARS: usize = 10;
/// The base32 alphabet (lower case, RFC 4648): 32 symbols, so a random byte masked to 5 bits has no bias.
const BASE32: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
/// The longest captcha text sent (the server's captchas are four characters).
const MAX_CAPTCHA_CHARS: usize = 16;
/// A documentation address never routed: connecting a UDP socket to it sends nothing, and only asks the OS which
/// local address it would use.
const ROUTE_PROBE: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), 9);

/// Why the owner account could not be registered.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OwnerError {
    /// The control plane, its API or the keychain failed.
    #[error(transparent)]
    Host(#[from] HostError),
    /// The typed captcha text was not accepted. The registration is still waiting: type it again, or start over.
    #[error("the captcha text was not accepted")]
    CaptchaRefused,
    /// There is no registration waiting for a captcha (it was never begun, or it ended).
    #[error("no owner registration is waiting for a captcha")]
    NotStarted,
    /// The keychain already holds an owner account that signs in.
    #[error("the owner account is already registered")]
    AlreadyRegistered,
}

/// A registration that has its captcha and waits for the typed answer.
pub(super) struct Pending {
    /// The generated account name.
    pub name: String,
    /// The generated password (kept in the keychain only once the person answers).
    pub password: Secret,
    /// The session cookie the captcha was issued in: the answer is checked against this session.
    pub cookie: Secret,
}

/// What `register` answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Registered {
    /// The account exists now.
    Yes,
    /// The captcha text was wrong.
    CaptchaRefused,
}

/// What the owner step needs from easytier-web's REST API.
pub(super) trait OwnerApi {
    /// `GET /api/v1/auth/captcha`: the PNG and the session cookie it was issued in.
    fn captcha(&self) -> Result<(Vec<u8>, Secret), HostError>;

    /// `POST /api/v1/auth/register` in the captcha's session.
    fn register(
        &self,
        cookie: &Secret,
        name: &str,
        password: &Secret,
        captcha: &str,
    ) -> Result<Registered, HostError>;

    /// Whether `name` / `password` signs in.
    fn can_sign_in(&self, name: &str, password: &Secret) -> Result<bool, HostError>;
}

/// A fresh owner name: `owner-` and ten random base32 characters.
///
/// # Errors
///
/// [`HostError::Random`] when the OS random source fails.
pub(super) fn generate_name() -> Result<String, HostError> {
    let mut bytes = [0_u8; SUFFIX_CHARS];
    getrandom::fill(&mut bytes).map_err(|error| HostError::Random(error.to_string()))?;
    let mut name = String::with_capacity(OWNER_PREFIX.len().saturating_add(SUFFIX_CHARS));
    name.push_str(OWNER_PREFIX);
    for byte in bytes {
        let symbol = BASE32.get(usize::from(byte & 31)).copied().unwrap_or(b'a');
        name.push(char::from(symbol));
    }
    bytes.zeroize();
    Ok(name)
}

/// The owner credentials in the keychain, if both are there.
pub(super) fn stored(secrets: &dyn SecretStore) -> Result<Option<(String, Secret)>, HostError> {
    let keychain =
        |error: crate::core::keychain::KeychainError| HostError::Keychain(error.to_string());
    let name = secrets.get(OWNER_NAME_ACCOUNT).map_err(keychain)?;
    let password = secrets.get(OWNER_PASSWORD_ACCOUNT).map_err(keychain)?;
    Ok(match (name, password) {
        (Some(name), Some(password)) if !name.is_empty() && !password.is_empty() => {
            Some((name.expose().to_owned(), password))
        }
        _ => None,
    })
}

/// Whether the keychain holds an owner account that signs in.
pub(super) fn already_registered<A: OwnerApi>(
    api: &A,
    secrets: &dyn SecretStore,
) -> Result<bool, HostError> {
    match stored(secrets)? {
        Some((name, password)) => api.can_sign_in(&name, &password),
        None => Ok(false),
    }
}

/// Generates the account and asks for a captcha; nothing is kept yet.
pub(super) fn begin<A: OwnerApi>(api: &A) -> Result<(Pending, Vec<u8>), HostError> {
    let name = generate_name()?;
    let password = Secret::generate().map_err(|error| HostError::Random(error.to_string()))?;
    let (png, cookie) = api.captcha()?;
    Ok((
        Pending {
            name,
            password,
            cookie,
        },
        png,
    ))
}

/// Registers the account with the typed captcha. The credentials are kept in the keychain before the server sees
/// them (a crash in between must not lose the only copy of the password) and put back as they were when the
/// captcha is refused or the registration fails.
pub(super) fn complete<A: OwnerApi>(
    api: &A,
    secrets: &dyn SecretStore,
    pending: &Pending,
    captcha: &str,
) -> Result<(), OwnerError> {
    let captcha = captcha.trim();
    if captcha.is_empty() || captcha.chars().count() > MAX_CAPTCHA_CHARS {
        return Err(OwnerError::CaptchaRefused);
    }
    let keychain =
        |error: crate::core::keychain::KeychainError| HostError::Keychain(error.to_string());
    let previous_name = secrets.get(OWNER_NAME_ACCOUNT).map_err(keychain)?;
    let previous_password = secrets.get(OWNER_PASSWORD_ACCOUNT).map_err(keychain)?;
    secrets
        .set(OWNER_NAME_ACCOUNT, &Secret::new(pending.name.as_str()))
        .map_err(keychain)?;
    secrets
        .set(OWNER_PASSWORD_ACCOUNT, &pending.password)
        .map_err(keychain)?;
    let undo = || {
        restore(secrets, OWNER_NAME_ACCOUNT, previous_name.as_ref());
        restore(secrets, OWNER_PASSWORD_ACCOUNT, previous_password.as_ref());
    };

    match api.register(&pending.cookie, &pending.name, &pending.password, captcha) {
        Ok(Registered::Yes) => {}
        Ok(Registered::CaptchaRefused) => {
            undo();
            return Err(OwnerError::CaptchaRefused);
        }
        Err(error) => {
            // The request may have been applied even though the answer was lost: ask the server before undoing.
            match api.can_sign_in(&pending.name, &pending.password) {
                Ok(true) => {}
                Ok(false) => {
                    undo();
                    return Err(error.into());
                }
                Err(probe) => {
                    // Unknown outcome: keep the new credentials, they may be the only ones that work.
                    log::warn!("could not tell whether the owner account was created: {probe}");
                    return Err(error.into());
                }
            }
        }
    }
    if !api.can_sign_in(&pending.name, &pending.password)? {
        return Err(HostError::ChangeUnconfirmed { account: "owner" }.into());
    }
    Ok(())
}

fn restore(secrets: &dyn SecretStore, key: &str, previous: Option<&Secret>) {
    let result = match previous {
        Some(secret) => secrets.set(key, secret),
        None => secrets.delete(key),
    };
    if let Err(error) = result {
        log::warn!("could not restore the keychain entry {key}: {error}");
    }
}

/// The address other machines on the local network would reach this computer at, if the OS can say (the source
/// address of the route to a documentation address; nothing is sent).
pub(super) fn lan_address() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect(ROUTE_PROBE).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_unspecified() && !ip.is_loopback() => Some(ip),
        IpAddr::V4(_) | IpAddr::V6(_) => None,
    }
}

/// `<protocol>://<host>:<config port>/<owner>`: what a machine is given to enroll under the owner account. `host`
/// is the LAN address, or [`ENROLLMENT_HOST_PLACEHOLDER`] when there is none.
pub(super) fn enrollment_url(
    settings: &HostSettings,
    owner: &str,
    host: Option<Ipv4Addr>,
) -> Secret {
    let host = host.map_or_else(
        || ENROLLMENT_HOST_PLACEHOLDER.to_owned(),
        |ip| ip.to_string(),
    );
    Secret::new(format!(
        "{}://{host}:{}/{owner}",
        settings.protocol.as_str(),
        settings.config_port
    ))
}

/// [`OwnerApi`] over the hosted server's loopback REST API.
pub(super) struct LoopbackOwner(pub(super) Loopback);

impl OwnerApi for LoopbackOwner {
    fn captcha(&self) -> Result<(Vec<u8>, Secret), HostError> {
        let response = self.0.send("GET", "/api/v1/auth/captcha", None, None)?;
        if response.status != 200 {
            return Err(status_error("captcha", &response));
        }
        if !response.body.starts_with(b"\x89PNG") {
            return Err(HostError::Api("the captcha is not a PNG image".to_owned()));
        }
        let cookie = response
            .session_cookie()
            .ok_or_else(|| HostError::Api("the captcha opened no session".to_owned()))?;
        Ok((response.body, cookie))
    }

    fn register(
        &self,
        cookie: &Secret,
        name: &str,
        password: &Secret,
        captcha: &str,
    ) -> Result<Registered, HostError> {
        let body = Secret::new(
            serde_json::json!({
                "credentials": { "username": name, "password": md5_hex(password).expose() },
                "captcha": captcha,
            })
            .to_string(),
        );
        let response = self
            .0
            .send("POST", "/api/v1/auth/register", Some(cookie), Some(&body))?;
        match response.status {
            200 => Ok(Registered::Yes),
            400 if response.message().to_lowercase().contains("captcha") => {
                Ok(Registered::CaptchaRefused)
            }
            _ => Err(status_error("registration", &response)),
        }
    }

    fn can_sign_in(&self, name: &str, password: &Secret) -> Result<bool, HostError> {
        let body = Secret::new(
            serde_json::json!({
                "username": name,
                "password": md5_hex(password).expose(),
            })
            .to_string(),
        );
        let response = self
            .0
            .send("POST", "/api/v1/auth/login", None, Some(&body))?;
        match response.status {
            200 => Ok(true),
            401 => Ok(false),
            _ => Err(status_error("sign-in", &response)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::core::keychain::MemoryStore;

    /// An in-memory easytier-web: registered accounts, a fixed captcha answer, optional failures.
    struct FakeApi {
        accounts: RefCell<Vec<(String, String)>>,
        answer: &'static str,
        /// `register` applies but reports a failure.
        lose_answer: bool,
        /// `register` fails before applying.
        refuse: bool,
    }

    impl FakeApi {
        fn new() -> Self {
            Self {
                accounts: RefCell::new(Vec::new()),
                answer: "ab3d",
                lose_answer: false,
                refuse: false,
            }
        }
    }

    impl OwnerApi for FakeApi {
        fn captcha(&self) -> Result<(Vec<u8>, Secret), HostError> {
            Ok((b"\x89PNG fake".to_vec(), Secret::new("cookie-for-captcha")))
        }

        fn register(
            &self,
            cookie: &Secret,
            name: &str,
            password: &Secret,
            captcha: &str,
        ) -> Result<Registered, HostError> {
            if cookie.expose() != "cookie-for-captcha" {
                return Err(HostError::Api(
                    "the captcha cookie was not sent back".to_owned(),
                ));
            }
            if self.refuse {
                return Err(HostError::Api("registration answered 500".to_owned()));
            }
            if !captcha.eq_ignore_ascii_case(self.answer) {
                return Ok(Registered::CaptchaRefused);
            }
            self.accounts
                .borrow_mut()
                .push((name.to_owned(), password.expose().to_owned()));
            if self.lose_answer {
                return Err(HostError::Api("connection reset".to_owned()));
            }
            Ok(Registered::Yes)
        }

        fn can_sign_in(&self, name: &str, password: &Secret) -> Result<bool, HostError> {
            Ok(self
                .accounts
                .borrow()
                .iter()
                .any(|(n, p)| n == name && p == password.expose()))
        }
    }

    #[test]
    fn owner_names_are_random_base32_after_the_prefix() {
        let a = generate_name().unwrap();
        let b = generate_name().unwrap();
        assert_ne!(a, b);
        for name in [&a, &b] {
            let suffix = name.strip_prefix("owner-").unwrap();
            assert_eq!(suffix.len(), 10);
            assert!(
                suffix
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || (b'2'..=b'7').contains(&c)),
                "{name}"
            );
        }
        // Never one of the seeded names.
        assert!(!["admin", "user"].contains(&a.as_str()));
    }

    #[test]
    fn a_typed_captcha_registers_the_account_and_keeps_the_credentials() {
        let api = FakeApi::new();
        let secrets = MemoryStore::default();
        assert!(!already_registered(&api, &secrets).unwrap());
        let (pending, png) = begin(&api).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
        // Nothing is kept until the person answers.
        assert_eq!(secrets.get(OWNER_NAME_ACCOUNT).unwrap(), None);

        complete(&api, &secrets, &pending, " AB3D ").unwrap();
        let (name, password) = stored(&secrets).unwrap().unwrap();
        assert_eq!(name, pending.name);
        assert_eq!(password, pending.password);
        assert!(api.can_sign_in(&name, &password).unwrap());
        assert!(already_registered(&api, &secrets).unwrap());
    }

    #[test]
    fn a_wrong_captcha_keeps_nothing_and_can_be_retried() {
        let api = FakeApi::new();
        let secrets = MemoryStore::default();
        let (pending, _) = begin(&api).unwrap();
        for wrong in ["zzzz", "", "   ", "a-very-long-wrong-captcha-text"] {
            assert_eq!(
                complete(&api, &secrets, &pending, wrong),
                Err(OwnerError::CaptchaRefused),
                "{wrong:?}"
            );
            assert_eq!(secrets.get(OWNER_NAME_ACCOUNT).unwrap(), None);
            assert_eq!(secrets.get(OWNER_PASSWORD_ACCOUNT).unwrap(), None);
        }
        assert!(api.accounts.borrow().is_empty());
        complete(&api, &secrets, &pending, "ab3d").unwrap();
        assert!(stored(&secrets).unwrap().is_some());
    }

    #[test]
    fn a_failed_registration_puts_the_previous_credentials_back() {
        let mut api = FakeApi::new();
        api.refuse = true;
        let secrets = MemoryStore::default();
        secrets
            .set(OWNER_NAME_ACCOUNT, &Secret::new("owner-oldoldold2"))
            .unwrap();
        secrets
            .set(
                OWNER_PASSWORD_ACCOUNT,
                &Secret::new("example-secret-do-not-use"),
            )
            .unwrap();
        let (pending, _) = begin(&api).unwrap();
        let error = complete(&api, &secrets, &pending, "ab3d").unwrap_err();
        assert!(matches!(error, OwnerError::Host(HostError::Api(_))));
        assert_eq!(
            secrets.get(OWNER_NAME_ACCOUNT).unwrap().unwrap().expose(),
            "owner-oldoldold2"
        );
        assert_eq!(
            secrets
                .get(OWNER_PASSWORD_ACCOUNT)
                .unwrap()
                .unwrap()
                .expose(),
            "example-secret-do-not-use"
        );
    }

    #[test]
    fn a_lost_answer_after_the_registration_keeps_the_new_credentials() {
        let mut api = FakeApi::new();
        api.lose_answer = true;
        let secrets = MemoryStore::default();
        let (pending, _) = begin(&api).unwrap();
        complete(&api, &secrets, &pending, "ab3d").unwrap();
        let (name, _) = stored(&secrets).unwrap().unwrap();
        assert_eq!(name, pending.name);
    }

    #[test]
    fn an_account_the_keychain_knows_is_already_registered() {
        let api = FakeApi::new();
        let secrets = MemoryStore::default();
        let (pending, _) = begin(&api).unwrap();
        complete(&api, &secrets, &pending, "ab3d").unwrap();
        assert!(already_registered(&api, &secrets).unwrap());
        // A keychain entry the server does not know (a reset database) is not "registered".
        let fresh_server = FakeApi::new();
        assert!(!already_registered(&fresh_server, &secrets).unwrap());
    }

    #[test]
    fn the_enrollment_url_names_the_owner_and_the_config_server() {
        let settings = HostSettings {
            config_port: 23456,
            protocol: super::super::settings::HostProtocol::Tcp,
            ..HostSettings::default()
        };
        assert_eq!(
            enrollment_url(
                &settings,
                "owner-abcde23456",
                Some(Ipv4Addr::new(192, 168, 1, 20))
            )
            .expose(),
            "tcp://192.168.1.20:23456/owner-abcde23456"
        );
        assert_eq!(
            enrollment_url(&HostSettings::default(), "owner-abcde23456", None).expose(),
            "udp://<address-of-this-computer>:22020/owner-abcde23456"
        );
    }

    #[test]
    fn the_route_probe_is_a_documentation_address() {
        assert_eq!(ROUTE_PROBE.ip(), IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)));
        // Whatever the machine's network is, the answer is never loopback or unspecified.
        if let Some(ip) = lan_address() {
            assert!(!ip.is_loopback() && !ip.is_unspecified());
        }
    }
}
