//! First-run hardening of the hosted easytier-web: replace the seeded `admin` / `admin` and `user` / `user`
//! passwords before anything but loopback can reach the API (`rule://easytier-config-security`, "Hosting the control
//! plane").
//!
//! The account logic is written against [`AdminApi`] so it is tested without a process; [`LoopbackAdmin`] is the real
//! implementation over the hosted server's loopback REST API.

use md5::{Digest as _, Md5};

use super::{
    error::{HostError, PasswordProblem},
    http::{Loopback, Response},
    settings::HostProtocol,
};
use crate::core::{
    keychain::SecretStore,
    secret::{Secret, SecretError},
};

/// The shortest password a person may supply for a default account.
pub const MIN_PASSWORD_CHARS: usize = 8;

/// The keychain account of the hosted `admin` password.
pub const ADMIN_PASSWORD_ACCOUNT: &str = "host/admin-password";
/// The keychain account of the hosted `user` password.
pub const USER_PASSWORD_ACCOUNT: &str = "host/user-password";

/// One of the two accounts easytier-web seeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostAccount {
    /// `admin`, in the admins group: the account the console signs in with.
    Admin,
    /// `user`, the second seeded account.
    User,
}

impl HostAccount {
    /// The account's name in easytier-web.
    pub fn username(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::User => "user",
        }
    }

    /// The well-known password the first migration seeds.
    fn default_password(self) -> &'static str {
        self.username()
    }

    /// Where the password is kept.
    pub fn keychain_account(self) -> &'static str {
        match self {
            Self::Admin => ADMIN_PASSWORD_ACCOUNT,
            Self::User => USER_PASSWORD_ACCOUNT,
        }
    }
}

/// What the user chose for the new passwords; `None` means "generate one".
#[derive(Default)]
pub struct HardenRequest {
    /// The new `admin` password.
    pub admin_password: Option<Secret>,
    /// The new `user` password.
    pub user_password: Option<Secret>,
}

impl HardenRequest {
    /// Checks the supplied passwords before anything is changed.
    ///
    /// # Errors
    ///
    /// [`HostError::Password`] for an empty, short or well-known one.
    pub fn validate(&self) -> Result<(), HostError> {
        for (account, supplied) in [
            (HostAccount::Admin, &self.admin_password),
            (HostAccount::User, &self.user_password),
        ] {
            if let Some(password) = supplied {
                check_password(account, password)?;
            }
        }
        Ok(())
    }

    fn supplied(&self, account: HostAccount) -> Option<&Secret> {
        match account {
            HostAccount::Admin => self.admin_password.as_ref(),
            HostAccount::User => self.user_password.as_ref(),
        }
    }
}

fn check_password(account: HostAccount, password: &Secret) -> Result<(), HostError> {
    let problem = if password.is_empty() {
        Some(PasswordProblem::Empty)
    } else if password.expose().chars().count() < MIN_PASSWORD_CHARS {
        Some(PasswordProblem::TooShort)
    } else {
        None
    };
    match problem {
        Some(problem) => Err(HostError::Password {
            account: account.username(),
            problem,
        }),
        None => Ok(()),
    }
}

/// What hardening did and what is still open. Shown to the user whole: the config server's exposure is part of it.
#[derive(Debug, Clone, PartialEq)]
pub struct HardeningReport {
    /// `admin` no longer signs in with its seeded password (replaced now, or found already replaced).
    pub admin_default_replaced: bool,
    /// `user` no longer signs in with its seeded password.
    pub user_default_replaced: bool,
    /// Registration is closed, confirmed against the running server.
    pub registration_disabled: bool,
    /// Every account already refused its default, so this run changed nothing about accounts.
    pub already_hardened: bool,
    /// The keychain holds the `admin` password and it signs in (the console can use it).
    pub admin_password_stored: bool,
    /// The keychain holds the `user` password and it signs in.
    pub user_password_stored: bool,
    /// The REST API listens on `127.0.0.1` only. Always true: nothing here widens it.
    pub api_loopback_only: bool,
    /// The config server's port.
    pub config_server_port: u16,
    /// The config server's protocol.
    pub protocol: HostProtocol,
    /// The config server listens on every network interface; easytier-web has no address option for it. The UI
    /// says so and names the port and protocol.
    pub exposed_on_all_interfaces: bool,
    /// Why hardening stopped early; the fields above say how far it got. The API stays on loopback either way.
    pub error: Option<HostError>,
}

impl HardeningReport {
    pub(super) fn new(config_server_port: u16, protocol: HostProtocol) -> Self {
        Self {
            admin_default_replaced: false,
            user_default_replaced: false,
            registration_disabled: false,
            already_hardened: false,
            admin_password_stored: false,
            user_password_stored: false,
            api_loopback_only: true,
            config_server_port,
            protocol,
            exposed_on_all_interfaces: true,
            error: None,
        }
    }

    /// Every step finished: both defaults gone, registration closed, and the keychain holds the password of both
    /// accounts (an account someone else changed cannot be managed from here, so it is not "done").
    pub fn is_complete(&self) -> bool {
        self.error.is_none()
            && self.admin_default_replaced
            && self.user_default_replaced
            && self.admin_password_stored
            && self.user_password_stored
            && self.registration_disabled
    }

    fn set_account(&mut self, account: HostAccount, outcome: &AccountOutcome) {
        let (replaced, stored) = match account {
            HostAccount::Admin => (
                &mut self.admin_default_replaced,
                &mut self.admin_password_stored,
            ),
            HostAccount::User => (
                &mut self.user_default_replaced,
                &mut self.user_password_stored,
            ),
        };
        *replaced = true;
        *stored = outcome.password_stored;
    }
}

/// One signed-in session: the cookie easytier-web set.
pub(super) struct Session(pub(super) Secret);

/// What hardening needs from easytier-web's REST API.
pub(super) trait AdminApi {
    /// Signs in. `Ok(None)` is "the server refused these credentials" (401).
    fn login(&self, account: HostAccount, password: &Secret) -> Result<Option<Session>, HostError>;

    /// Replaces the signed-in account's password. The session is over afterwards.
    fn change_password(&self, session: &Session, new_password: &Secret) -> Result<(), HostError>;

    /// Whether registration is closed (the server refuses to register before even checking the captcha).
    fn registration_closed(&self) -> Result<bool, HostError>;
}

/// What happened to one account.
pub(super) struct AccountOutcome {
    /// This run changed the password (false: it was already changed).
    pub replaced_now: bool,
    /// The keychain holds a password that signs in.
    pub password_stored: bool,
}

/// Hardens both accounts, admin first, stopping at the first failure. Returns the report so far.
pub(super) fn harden_accounts<A: AdminApi>(
    api: &A,
    secrets: &dyn SecretStore,
    request: &HardenRequest,
    report: &mut HardeningReport,
) -> Result<(), HostError> {
    let mut changed_any = false;
    for account in [HostAccount::Admin, HostAccount::User] {
        let outcome = harden_account(api, secrets, account, request.supplied(account))?;
        report.set_account(account, &outcome);
        changed_any |= outcome.replaced_now;
    }
    report.already_hardened = !changed_any;
    Ok(())
}

fn harden_account<A: AdminApi>(
    api: &A,
    secrets: &dyn SecretStore,
    account: HostAccount,
    supplied: Option<&Secret>,
) -> Result<AccountOutcome, HostError> {
    let default = Secret::new(account.default_password());
    let Some(session) = api.login(account, &default)? else {
        // The default is refused: replaced earlier by this app, or by whoever made the database. Only an account
        // whose password the keychain holds (and that signs in) is ours to manage.
        if !stored_password_signs_in(api, secrets, account)? {
            return Err(HostError::PasswordUnknown {
                account: account.username(),
            });
        }
        return Ok(AccountOutcome {
            replaced_now: false,
            password_stored: true,
        });
    };

    let new_password = match supplied {
        Some(password) => password.clone(),
        None => {
            Secret::generate().map_err(|error: SecretError| HostError::Random(error.to_string()))?
        }
    };
    let key = account.keychain_account();
    // Keep it before the server knows it: a crash between the two must not lose the only copy of the password.
    let previous = secrets
        .get(key)
        .map_err(|error| HostError::Keychain(error.to_string()))?;
    secrets
        .set(key, &new_password)
        .map_err(|error| HostError::Keychain(error.to_string()))?;

    if let Err(error) = api.change_password(&session, &new_password) {
        // The request may have been applied even though the answer was lost: ask the server before undoing.
        match api.login(account, &default) {
            Ok(None) => {}
            Ok(Some(_)) => {
                restore(secrets, key, previous.as_ref());
                return Err(error);
            }
            Err(probe) => {
                // Unknown outcome: keep the new password in the keychain, it may be the only one that works.
                log::warn!(
                    "could not tell whether the {} password changed: {probe}",
                    account.username()
                );
                return Err(error);
            }
        }
    }

    // Confirm both directions: the seeded password is dead and the new one works.
    if api.login(account, &default)?.is_some() {
        return Err(HostError::ChangeUnconfirmed {
            account: account.username(),
        });
    }
    if api.login(account, &new_password)?.is_none() {
        return Err(HostError::ChangeUnconfirmed {
            account: account.username(),
        });
    }
    Ok(AccountOutcome {
        replaced_now: true,
        password_stored: true,
    })
}

/// Whether the keychain's password for `account` signs in.
fn stored_password_signs_in<A: AdminApi>(
    api: &A,
    secrets: &dyn SecretStore,
    account: HostAccount,
) -> Result<bool, HostError> {
    let stored = match secrets.get(account.keychain_account()) {
        Ok(stored) => stored,
        Err(error) => {
            log::warn!(
                "the keychain could not be read for the {} account: {error}",
                account.username()
            );
            None
        }
    };
    match stored {
        Some(password) => Ok(api.login(account, &password)?.is_some()),
        None => Ok(false),
    }
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

/// easytier-web's login pre-hash: lower-case hex MD5 of the password (what its own frontend sends; the server
/// stores an argon2 hash of that string). Not a protection in transit, which is why this client is loopback only.
pub(super) fn md5_hex(password: &Secret) -> Secret {
    let digest = Md5::digest(password.expose().as_bytes());
    let mut hex = String::with_capacity(32);
    for byte in digest.iter() {
        hex.push_str(&format!("{byte:02x}"));
    }
    Secret::new(hex)
}

fn json_body(template: impl FnOnce(&str) -> String, password: &Secret) -> Secret {
    Secret::new(template(md5_hex(password).expose()))
}

pub(super) fn status_error(what: &str, response: &Response) -> HostError {
    // easytier-web's error bodies are fixed sentences (`{"message":"Invalid credentials"}`), never echoes of input.
    let message = response.message();
    if message.is_empty() {
        HostError::Api(format!("{what} answered {}", response.status))
    } else {
        HostError::Api(format!("{what} answered {}: {message}", response.status))
    }
}

/// [`AdminApi`] over the hosted server's loopback REST API.
pub(super) struct LoopbackAdmin(pub(super) Loopback);

impl AdminApi for LoopbackAdmin {
    fn login(&self, account: HostAccount, password: &Secret) -> Result<Option<Session>, HostError> {
        let username = account.username();
        let body = json_body(
            |hash| format!(r#"{{"username":"{username}","password":"{hash}"}}"#),
            password,
        );
        let response = self
            .0
            .send("POST", "/api/v1/auth/login", None, Some(&body))?;
        match response.status {
            200 => response
                .session_cookie()
                .map(|cookie| Some(Session(cookie)))
                .ok_or_else(|| HostError::Api("the sign-in set no session".to_owned())),
            401 => Ok(None),
            _ => Err(status_error("sign-in", &response)),
        }
    }

    fn change_password(&self, session: &Session, new_password: &Secret) -> Result<(), HostError> {
        let body = json_body(
            |hash| format!(r#"{{"new_password":"{hash}"}}"#),
            new_password,
        );
        let response = self.0.send(
            "PUT",
            "/api/v1/auth/password",
            Some(&session.0),
            Some(&body),
        )?;
        if response.status == 200 {
            Ok(())
        } else {
            Err(status_error("password change", &response))
        }
    }

    fn registration_closed(&self) -> Result<bool, HostError> {
        // A closed server answers 403 before it looks at the captcha; an open one answers 400 (wrong captcha) and
        // creates nothing.
        let body = Secret::new(
            r#"{"credentials":{"username":"registration-probe","password":"x"},"captcha":"0000"}"#,
        );
        let response = self
            .0
            .send("POST", "/api/v1/auth/register", None, Some(&body))?;
        match response.status {
            403 => Ok(true),
            400 => Ok(false),
            _ => Err(status_error("registration probe", &response)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::BTreeMap};

    use super::*;
    use crate::core::keychain::MemoryStore;

    /// An in-memory easytier-web: plain passwords per account, optional failures.
    struct FakeApi {
        passwords: RefCell<BTreeMap<&'static str, String>>,
        /// `change_password` fails before applying.
        refuse_change: bool,
        /// `change_password` applies but reports a failure.
        lose_answer: bool,
    }

    impl FakeApi {
        fn fresh() -> Self {
            let passwords =
                BTreeMap::from([("admin", "admin".to_owned()), ("user", "user".to_owned())]);
            Self {
                passwords: RefCell::new(passwords),
                refuse_change: false,
                lose_answer: false,
            }
        }

        fn password_of(&self, name: &str) -> String {
            self.passwords.borrow().get(name).cloned().unwrap()
        }
    }

    impl AdminApi for FakeApi {
        fn login(
            &self,
            account: HostAccount,
            password: &Secret,
        ) -> Result<Option<Session>, HostError> {
            let current = self.password_of(account.username());
            Ok((current == password.expose())
                .then(|| Session(Secret::new(format!("cookie-for-{}", account.username())))))
        }

        fn change_password(
            &self,
            session: &Session,
            new_password: &Secret,
        ) -> Result<(), HostError> {
            if self.refuse_change {
                return Err(HostError::Api("password change answered 500".to_owned()));
            }
            let name = session
                .0
                .expose()
                .trim_start_matches("cookie-for-")
                .to_owned();
            let key = if name == "admin" { "admin" } else { "user" };
            self.passwords
                .borrow_mut()
                .insert(key, new_password.expose().to_owned());
            if self.lose_answer {
                return Err(HostError::Api("connection reset".to_owned()));
            }
            Ok(())
        }

        fn registration_closed(&self) -> Result<bool, HostError> {
            Ok(true)
        }
    }

    fn report() -> HardeningReport {
        HardeningReport::new(22020, HostProtocol::Udp)
    }

    #[test]
    fn a_fresh_server_gets_generated_passwords_kept_in_the_keychain() {
        let api = FakeApi::fresh();
        let store = MemoryStore::default();
        let mut report = report();
        harden_accounts(&api, &store, &HardenRequest::default(), &mut report).unwrap();
        assert!(report.admin_default_replaced && report.user_default_replaced);
        assert!(report.admin_password_stored && report.user_password_stored);
        assert!(!report.already_hardened);
        let admin = store.get(ADMIN_PASSWORD_ACCOUNT).unwrap().unwrap();
        let user = store.get(USER_PASSWORD_ACCOUNT).unwrap().unwrap();
        assert_ne!(admin, user);
        assert_eq!(admin.expose().len(), 32);
        assert_eq!(api.password_of("admin"), admin.expose());
        assert_eq!(api.password_of("user"), user.expose());
        assert!(report.api_loopback_only && report.exposed_on_all_interfaces);
    }

    #[test]
    fn supplied_passwords_are_used() {
        let api = FakeApi::fresh();
        let store = MemoryStore::default();
        let request = HardenRequest {
            admin_password: Some(Secret::new("example-secret-do-not-use")),
            user_password: None,
        };
        request.validate().unwrap();
        harden_accounts(&api, &store, &request, &mut report()).unwrap();
        assert_eq!(api.password_of("admin"), "example-secret-do-not-use");
        assert_eq!(
            store.get(ADMIN_PASSWORD_ACCOUNT).unwrap().unwrap().expose(),
            "example-secret-do-not-use"
        );
    }

    #[test]
    fn running_twice_detects_the_hardened_database() {
        let api = FakeApi::fresh();
        let store = MemoryStore::default();
        harden_accounts(&api, &store, &HardenRequest::default(), &mut report()).unwrap();
        let admin_before = api.password_of("admin");
        let mut second = report();
        harden_accounts(&api, &store, &HardenRequest::default(), &mut second).unwrap();
        assert!(second.already_hardened);
        assert!(second.admin_default_replaced && second.user_default_replaced);
        assert!(second.admin_password_stored);
        assert_eq!(api.password_of("admin"), admin_before);
    }

    #[test]
    fn a_refused_default_without_a_working_stored_password_is_an_error() {
        let api = FakeApi::fresh();
        api.passwords
            .borrow_mut()
            .insert("admin", "someone-elses-password".to_owned());
        api.passwords
            .borrow_mut()
            .insert("user", "another-password".to_owned());
        let store = MemoryStore::default();
        let mut report = report();
        let error =
            harden_accounts(&api, &store, &HardenRequest::default(), &mut report).unwrap_err();
        assert_eq!(error, HostError::PasswordUnknown { account: "admin" });
        assert!(!report.admin_default_replaced && !report.admin_password_stored);
        assert_eq!(store.get(ADMIN_PASSWORD_ACCOUNT).unwrap(), None);

        // A stored password that does not sign in is no better.
        store
            .set(
                ADMIN_PASSWORD_ACCOUNT,
                &Secret::new("stale-example-do-not-use"),
            )
            .unwrap();
        let error = harden_accounts(&api, &store, &HardenRequest::default(), &mut report.clone())
            .unwrap_err();
        assert_eq!(error, HostError::PasswordUnknown { account: "admin" });
    }

    #[test]
    fn a_refused_change_restores_the_keychain_and_reports_how_far_it_got() {
        let mut api = FakeApi::fresh();
        api.refuse_change = true;
        let store = MemoryStore::default();
        let mut report = report();
        let error =
            harden_accounts(&api, &store, &HardenRequest::default(), &mut report).unwrap_err();
        assert!(matches!(error, HostError::Api(_)));
        assert!(!report.admin_default_replaced);
        assert_eq!(store.get(ADMIN_PASSWORD_ACCOUNT).unwrap(), None);
        assert_eq!(api.password_of("admin"), "admin");
    }

    #[test]
    fn a_lost_answer_after_the_change_keeps_the_new_password() {
        let mut api = FakeApi::fresh();
        api.lose_answer = true;
        let store = MemoryStore::default();
        let mut report = report();
        harden_accounts(&api, &store, &HardenRequest::default(), &mut report).unwrap();
        let admin = store.get(ADMIN_PASSWORD_ACCOUNT).unwrap().unwrap();
        assert_eq!(api.password_of("admin"), admin.expose());
        assert!(report.admin_default_replaced);
    }

    #[test]
    fn weak_supplied_passwords_are_refused_before_anything_changes() {
        for (password, problem) in [
            ("", PasswordProblem::Empty),
            ("short", PasswordProblem::TooShort),
            ("admin", PasswordProblem::TooShort),
        ] {
            let request = HardenRequest {
                admin_password: Some(Secret::new(password)),
                user_password: None,
            };
            assert_eq!(
                request.validate().unwrap_err(),
                HostError::Password {
                    account: "admin",
                    problem
                }
            );
        }
        assert!(check_password(HostAccount::User, &Secret::new("12345678")).is_ok());
    }

    #[test]
    fn md5_hex_matches_what_the_web_frontend_sends() {
        assert_eq!(
            md5_hex(&Secret::new("admin")).expose(),
            "21232f297a57a5a743894a0e4a801fc3"
        );
        assert_eq!(
            md5_hex(&Secret::new("user")).expose(),
            "ee11cbb19052e40b07aac0ca060c23ee"
        );
    }

    #[test]
    fn the_report_is_complete_only_when_nothing_failed_and_both_passwords_are_ours() {
        let mut report = report();
        assert!(!report.is_complete());
        report.admin_default_replaced = true;
        report.user_default_replaced = true;
        report.registration_disabled = true;
        // Replaced by someone else: nothing in the keychain signs in, so the console cannot manage the server.
        assert!(!report.is_complete());
        report.admin_password_stored = true;
        assert!(!report.is_complete());
        report.user_password_stored = true;
        assert!(report.is_complete());
        report.error = Some(HostError::NotRunning);
        assert!(!report.is_complete());
    }
}
