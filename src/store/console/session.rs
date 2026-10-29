//! Servers: adding, restoring, signing in and out, changing the password, and what is kept of them.
//!
//! The servers the user added are kept in `Storage` as `console.servers.v1` (id, base URL, name: no credentials).
//! The session cookie and the account name rest in the keychain (`server/<sha256 of the base URL>/session`, `…/account`)
//! where the platform has one. A password is held only from the moment it is typed until the request that uses it
//! is sent.

use std::{fmt::Write as _, time::Duration};

use nsh_core::{prelude::*, storage::Storage, time::unix_ms};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::{ConsoleStore, Session};
use crate::{
    core::{
        keychain::KeychainError,
        remote::{RemoteClient, RemoteError, ServerAddress},
        secret::Secret,
        target::{Target, TargetId, TargetKind, TargetOrigin, TargetStatus},
        work::off_thread,
    },
    store::{Page, Store},
};

/// The storage key of the servers the user added.
const SERVERS_KEY: &str = "console.servers.v1";
/// How often the selected server's lists are read again while the app is open.
const POLL_EVERY: Duration = Duration::from_secs(30);
/// Two automatic sign-ins to the hosted control plane are at least this far apart (ms): a server that keeps
/// answering 401 right after a sign-in must not be signed in to in a loop.
const AUTO_SIGN_IN_GAP_MS: f64 = 10_000.0;
/// The longest name a server is given, in characters.
const MAX_NAME_CHARS: usize = 64;

/// A server as kept in `Storage`.
#[derive(Debug, Serialize, Deserialize)]
struct StoredServer {
    id: u32,
    url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// A digest of a server's normalized base URL: what its keychain entries are filed under, so a session cookie can
/// only be read back for the address it was issued by, never for another server that ends up with the same number.
fn address_digest(base: &str) -> String {
    let mut out = String::with_capacity(64);
    for byte in Sha256::digest(base.as_bytes()) {
        if write!(out, "{byte:02x}").is_err() {
            log::debug!("formatting a digest failed");
        }
    }
    out
}

/// The keychain account of the session cookie issued by `address`; none for the serving origin (the browser keeps
/// its cookie).
fn cookie_account(address: &ServerAddress) -> Option<String> {
    address
        .base()
        .map(|base| format!("server/{}/session", address_digest(base)))
}

/// The keychain account of the account name signed in at `address`.
fn account_account(address: &ServerAddress) -> Option<String> {
    address
        .base()
        .map(|base| format!("server/{}/account", address_digest(base)))
}

/// The accounts earlier versions filed a server's session under, by target number: deleted, never read.
fn legacy_accounts(id: TargetId) -> [String; 2] {
    [
        format!("server/{}/session", id.number()),
        format!("server/{}/account", id.number()),
    ]
}

/// Why a sign-in did not start or complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignInError {
    /// The username or the password is empty.
    Missing,
    /// The server refused, or could not be asked.
    Remote(RemoteError),
}

/// The sign-in form's fields (one form: the password is wiped whenever the selected server changes).
#[derive(Clone, Copy)]
pub struct SignInFields {
    /// The account name.
    pub username: Signal<String>,
    /// The password as typed; cleared the moment it is sent.
    pub password: Signal<String>,
    /// Why the last attempt failed.
    pub error: Signal<Option<SignInError>>,
}

impl SignInFields {
    pub(super) fn new() -> Self {
        Self {
            username: signal(String::new()),
            password: signal(String::new()),
            error: signal(None),
        }
    }
}

/// Why a server was not added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddError {
    /// The address is not usable (not a URL, plain http to another machine, ...).
    Address(RemoteError),
    /// A server with this address is already listed, under this name.
    Duplicate(String),
    /// The app ran out of ids for servers.
    NoRoom,
}

/// The add-server dialog's fields.
#[derive(Clone, Copy)]
pub struct AddFields {
    /// The dialog shows.
    pub open: Signal<bool>,
    /// The server's base URL as typed.
    pub url: Signal<String>,
    /// The name the user gives it; empty for its host.
    pub name: Signal<String>,
    /// Why it was not added.
    pub error: Signal<Option<AddError>>,
}

impl AddFields {
    pub(super) fn new() -> Self {
        Self {
            open: signal(false),
            url: signal(String::new()),
            name: signal(String::new()),
            error: signal(None),
        }
    }
}

/// Why a password was not changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasswordError {
    /// The new password is empty.
    Empty,
    /// The two entries differ.
    Mismatch,
    /// The server refused, or could not be asked.
    Remote(RemoteError),
}

/// The change-password dialog's fields.
#[derive(Clone, Copy)]
pub struct PasswordFields {
    /// The server whose account password is being changed; `None` while the dialog is closed.
    pub open: Signal<Option<TargetId>>,
    /// The new password as typed.
    pub new_password: Signal<String>,
    /// The new password again.
    pub confirm: Signal<String>,
    /// The request is in flight.
    pub busy: Signal<bool>,
    /// Why it failed.
    pub error: Signal<Option<PasswordError>>,
}

impl PasswordFields {
    pub(super) fn new() -> Self {
        Self {
            open: signal(None),
            new_password: signal(String::new()),
            confirm: signal(String::new()),
            busy: signal(false),
            error: signal(None),
        }
    }
}

impl ConsoleStore {
    // --- Restoring and watching --------------------------------------------------------------------------

    /// Puts the servers kept in `Storage` back as targets.
    pub(super) fn restore_servers(&self, store: Store) {
        let Some(text) = Storage::get_item(SERVERS_KEY) else {
            return;
        };
        let kept: Vec<StoredServer> = match serde_json::from_str(&text) {
            Ok(kept) => kept,
            Err(error) => {
                log::warn!("the kept servers could not be read: {error}");
                return;
            }
        };
        for server in kept {
            let Some(id) = TargetId::from_number(server.id) else {
                log::warn!("a kept server has the reserved id {}; skipped", server.id);
                continue;
            };
            if let Err(error) = ServerAddress::parse(&server.url) {
                log::warn!("kept server {} skipped: {error}", server.id);
                continue;
            }
            store.put_target(Target {
                id,
                kind: TargetKind::Server,
                origin: TargetOrigin::Url(server.url),
                name: server.name.map(SharedString::from),
                status: TargetStatus::NotConnected,
            });
        }
    }

    /// Connects the selected server when it is first looked at (or its address changed), keeps the sign-in form
    /// from carrying a password across servers, and reads the selected server's lists now and then.
    pub(super) fn watch(&self, store: Store) {
        let this = *self;
        effect(move || {
            let Some(target) = store.current_target() else {
                return;
            };
            if target.is_server() {
                untracked(|| this.sync(&target));
            }
        });
        effect(move || {
            let _selected = store.current_target_id();
            untracked(|| this.wipe_passwords());
        });
        let _poll = set_interval(POLL_EVERY, move || this.poll());
    }

    /// Clears every password field and the sign-in error, so nothing typed or refused for one server shows on
    /// another.
    fn wipe_passwords(&self) {
        batch(|| {
            self.sign_in.password.set(String::new());
            self.sign_in.error.set(None);
            self.password.new_password.set(String::new());
            self.password.confirm.set(String::new());
        });
    }

    /// Reads the selected signed-in server's machines (and the machine in detail) again.
    fn poll(&self) {
        let Some(store) = self.root() else {
            return;
        };
        let Some(target) = untracked(|| store.current_target()) else {
            return;
        };
        if !target.is_server() {
            return;
        }
        let id = target.id;
        if !matches!(self.session_now(id), Session::SignedIn { .. }) {
            return;
        }
        self.refresh_machines(id);
        if let Some(machine) = self.selected.with_untracked(|all| all.get(&id).copied()) {
            self.refresh_networks(id, machine);
        }
    }

    /// Makes sure `target` has a client for its current address and has started connecting.
    fn sync(&self, target: &Target) {
        let id = target.id;
        let address = match ServerAddress::from_origin(&target.origin) {
            Ok(address) => address,
            Err(error) => {
                let first = self.started.with(|started| started.insert(id));
                if first {
                    self.unreachable(id, &error);
                }
                return;
            }
        };
        let known = self
            .clients
            .with(|clients| clients.get(&id).map(|client| client.address().clone()));
        match known {
            Some(known) if known == address => {
                if self.started.with(|started| started.contains(&id)) {
                    // The hosted control plane came back up while the app waited for it: connect again.
                    let came_back = target.kind == TargetKind::Hosted
                        && target.status == TargetStatus::Online
                        && matches!(self.session_now(id), Session::Unreachable { .. });
                    if came_back {
                        self.connect(id);
                    }
                    return;
                }
            }
            Some(_) => self.reset(id),
            None => {}
        }
        self.clients
            .with(|clients| clients.insert(id, RemoteClient::new(address)));
        self.started.with(|started| started.insert(id));
        self.connect(id);
    }

    /// Forgets everything about `id`'s session and data: its address changed or it is gone.
    fn reset(&self, id: TargetId) {
        self.bump_epoch(id);
        self.clients.with(|clients| clients.remove(&id));
        self.started.with(|started| started.remove(&id));
        self.clear_data(id);
        self.set_session(id, Session::NotSignedIn);
    }

    // --- Small helpers -------------------------------------------------------------------------------------

    /// `id`'s client.
    pub(super) fn client(&self, id: TargetId) -> Option<RemoteClient> {
        self.clients.with(|clients| clients.get(&id).cloned())
    }

    /// `id`'s session, without subscribing.
    pub(super) fn session_now(&self, id: TargetId) -> Session {
        self.sessions
            .with_untracked(|all| all.get(&id).cloned())
            .unwrap_or_default()
    }

    /// The epoch of `id`'s session.
    pub(super) fn epoch(&self, id: TargetId) -> u64 {
        self.epochs
            .with(|epochs| epochs.get(&id).copied().unwrap_or(0))
    }

    /// Whether `epoch` is still the epoch of `id`'s session (an answer that fails this is dropped).
    pub(super) fn current_epoch(&self, id: TargetId, epoch: u64) -> bool {
        self.epoch(id) == epoch
    }

    /// Starts a new epoch for `id`: what is in flight no longer counts.
    fn bump_epoch(&self, id: TargetId) {
        self.epochs.with(|epochs| {
            let next = epochs.get(&id).copied().unwrap_or(0).wrapping_add(1);
            epochs.insert(id, next);
        });
    }

    pub(super) fn set_session(&self, id: TargetId, session: Session) {
        self.sessions.update(|all| {
            all.insert(id, session);
        });
    }

    /// Sets `id`'s lamp, except for the hosted control plane: its lamp is the process's state, which the host role
    /// keeps.
    pub(super) fn set_status(&self, id: TargetId, status: TargetStatus) {
        if self.is_hosted(id) {
            return;
        }
        if let Some(store) = self.root() {
            store.set_target_status(id, status);
        }
    }

    /// Whether `id` is the control plane this desktop hosts.
    fn is_hosted(&self, id: TargetId) -> bool {
        self.root()
            .and_then(|store| untracked(|| store.target(id)))
            .is_some_and(|target| target.kind == TargetKind::Hosted)
    }

    /// The server did not answer.
    pub(super) fn unreachable(&self, id: TargetId, error: &RemoteError) {
        self.set_session(
            id,
            Session::Unreachable {
                message: error.to_string(),
            },
        );
        self.set_status(id, TargetStatus::Failed);
    }

    /// A read or write of `id` failed with `error`: a 401 ends the session, no answer marks the server
    /// unreachable; anything else is the caller's to show.
    pub(super) fn react_to(&self, id: TargetId, error: &RemoteError) {
        match error {
            RemoteError::Unauthorized => self.expire(id),
            RemoteError::Network(_) | RemoteError::Timeout => self.unreachable(id, error),
            _ => {}
        }
    }

    /// The server ended the session: sign in again. The hosted control plane signs in by itself.
    pub(super) fn expire(&self, id: TargetId) {
        if !matches!(self.session_now(id), Session::SignedIn { .. }) {
            return;
        }
        self.bump_epoch(id);
        self.stop_loading(id);
        self.set_session(id, Session::Expired);
        self.set_status(id, TargetStatus::Failed);
        self.close_overlays_of(id);
        let this = *self;
        let address = self.kept_address(id);
        self.go(async move { this.forget_kept(id, address).await });
        if self.is_hosted(id) {
            self.connect(id);
        }
    }

    // --- The keychain ---------------------------------------------------------------------------------------

    /// The session cookie and account name kept for `id` at `address`; nothing for another address.
    async fn load_kept(
        &self,
        id: TargetId,
        address: &ServerAddress,
    ) -> Option<(Secret, Option<String>)> {
        let (cookie_key, account_key) = (cookie_account(address)?, account_account(address)?);
        let secrets = self.secrets.get();
        let read = off_thread(move || (secrets.get(&cookie_key), secrets.get(&account_key))).await;
        match read {
            Ok((Ok(Some(cookie)), account)) => {
                let account = account
                    .ok()
                    .flatten()
                    .map(|account| account.expose().to_owned())
                    .filter(|account| !account.is_empty());
                Some((cookie, account))
            }
            Ok((Ok(None), _)) => None,
            Ok((Err(error), _)) => {
                log::warn!(
                    "the kept session of server {} could not be read: {error}",
                    id.number()
                );
                None
            }
            Err(error) => {
                log::warn!(
                    "the kept session of server {} could not be read: {error}",
                    id.number()
                );
                None
            }
        }
    }

    /// Keeps `client`'s session cookie and account name for `id` (where the keychain keeps secrets), under its
    /// address.
    async fn remember_kept(&self, id: TargetId, client: &RemoteClient) {
        if !self.persistent {
            return;
        }
        let Some(cookie) = client.session_cookie() else {
            return;
        };
        let (Some(cookie_key), Some(account_key)) = (
            cookie_account(client.address()),
            account_account(client.address()),
        ) else {
            return;
        };
        let account = Secret::new(client.account().unwrap_or_default());
        let secrets = self.secrets.get();
        let written = off_thread(move || -> Result<(), KeychainError> {
            secrets.set(&cookie_key, &cookie)?;
            if !account.is_empty() {
                secrets.set(&account_key, &account)?;
            }
            Ok(())
        })
        .await;
        match written {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                log::warn!(
                    "the session of server {} was not kept: {error}",
                    id.number()
                );
            }
            Err(error) => {
                log::warn!(
                    "the session of server {} was not kept: {error}",
                    id.number()
                );
            }
        }
    }

    /// The address `id`'s kept entries are filed under: its client's, else its target's.
    pub(super) fn kept_address(&self, id: TargetId) -> Option<ServerAddress> {
        if let Some(client) = self.client(id) {
            return Some(client.address().clone());
        }
        let store = self.root()?;
        let target = untracked(|| store.target(id))?;
        ServerAddress::from_origin(&target.origin).ok()
    }

    /// Deletes what is kept for `id`: the entries under `address`, and the ones earlier versions filed under the
    /// target's number.
    pub(super) async fn forget_kept(&self, id: TargetId, address: Option<ServerAddress>) {
        if !self.persistent {
            return;
        }
        let secrets = self.secrets.get();
        let mut keys: Vec<String> = legacy_accounts(id).into();
        if let Some(address) = &address {
            keys.extend(cookie_account(address));
            keys.extend(account_account(address));
        }
        let deleted = off_thread(move || -> Result<(), KeychainError> {
            // Every entry is tried; the first failure is the one reported.
            let mut failure = None;
            for key in &keys {
                if let Err(error) = secrets.delete(key)
                    && failure.is_none()
                {
                    failure = Some(error);
                }
            }
            failure.map_or(Ok(()), Err)
        })
        .await;
        match deleted {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                log::warn!(
                    "the kept session of server {} was not deleted: {error}",
                    id.number()
                );
            }
            Err(error) => {
                log::warn!(
                    "the kept session of server {} was not deleted: {error}",
                    id.number()
                );
            }
        }
    }

    // --- Connecting -----------------------------------------------------------------------------------------

    /// Connects to `id` again: restores a kept session if there is one, signs in to the hosted control plane by
    /// itself, otherwise waits for the person to sign in. Also the "Retry" of an unreachable server.
    pub fn connect(&self, id: TargetId) {
        let Some(client) = self.client(id) else {
            return;
        };
        let epoch = self.epoch(id);
        self.set_session(id, Session::SigningIn);
        self.set_status(id, TargetStatus::Connecting);
        let this = *self;
        self.go(async move { this.restore_session(id, epoch, client).await });
    }

    /// The first steps of a connection: a kept or browser-held session is checked before it is trusted.
    async fn restore_session(&self, id: TargetId, epoch: u64, client: RemoteClient) {
        let mut client = client;
        let mut restored = false;
        if !client.has_session_cookie()
            && self.persistent
            && let Some((cookie, account)) = self.load_kept(id, client.address()).await
        {
            if !self.current_epoch(id, epoch) {
                return;
            }
            client = RemoteClient::with_session(client.address().clone(), Some(cookie), account);
            self.clients
                .with(|clients| clients.insert(id, client.clone()));
            restored = true;
        }
        let hosted = self.is_hosted(id);
        // A browser keeps the session cookie itself, out of the app's reach: ask the server.
        let held_by_browser = matches!(client.address(), ServerAddress::SameOrigin);
        if client.has_session_cookie() || held_by_browser {
            let checked = client.check_session().await;
            if !self.current_epoch(id, epoch) {
                return;
            }
            match checked {
                Ok(true) => self.signed_in(id, &client).await,
                Ok(false) => {
                    self.forget_kept(id, Some(client.address().clone())).await;
                    if hosted {
                        self.auto_sign_in(id, epoch, client).await;
                    } else if restored {
                        self.set_session(id, Session::Expired);
                        self.set_status(id, TargetStatus::Failed);
                    } else {
                        self.set_session(id, Session::NotSignedIn);
                        self.set_status(id, TargetStatus::NotConnected);
                    }
                }
                Err(error) => self.unreachable(id, &error),
            }
        } else if hosted {
            self.auto_sign_in(id, epoch, client).await;
        } else {
            self.set_session(id, Session::NotSignedIn);
            self.set_status(id, TargetStatus::NotConnected);
        }
    }

    /// Signs in to the hosted control plane with the account the host role keeps for it.
    async fn auto_sign_in(&self, id: TargetId, epoch: u64, client: RemoteClient) {
        let Some(store) = self.root() else {
            return;
        };
        let now = unix_ms();
        let too_soon = self.auto_at.with(|at| {
            let soon = at
                .get(&id)
                .is_some_and(|last| now - *last < AUTO_SIGN_IN_GAP_MS);
            if !soon {
                at.insert(id, now);
            }
            soon
        });
        if too_soon {
            self.set_session(id, Session::Expired);
            self.set_status(id, TargetStatus::Failed);
            return;
        }
        let Some((username, password)) = store.host().hosted_sign_in().await else {
            if self.current_epoch(id, epoch) {
                self.set_session(id, Session::NotSignedIn);
                self.set_status(id, TargetStatus::NotConnected);
            }
            return;
        };
        let result = client.login(&username, &password).await;
        if !self.current_epoch(id, epoch) {
            return;
        }
        self.finish_login(id, &client, result).await;
    }

    /// Signed in: shows the machines and keeps the session.
    async fn signed_in(&self, id: TargetId, client: &RemoteClient) {
        self.set_session(
            id,
            Session::SignedIn {
                account: client.account().unwrap_or_default(),
            },
        );
        self.set_status(id, TargetStatus::Online);
        self.refresh_machines(id);
        self.remember_kept(id, client).await;
    }

    /// The outcome of a sign-in request.
    async fn finish_login(
        &self,
        id: TargetId,
        client: &RemoteClient,
        result: Result<(), RemoteError>,
    ) {
        match result {
            Ok(()) => self.signed_in(id, client).await,
            Err(RemoteError::Network(message)) => {
                self.unreachable(id, &RemoteError::Network(message));
            }
            Err(RemoteError::Timeout) => self.unreachable(id, &RemoteError::Timeout),
            Err(error) => {
                self.set_session(id, Session::NotSignedIn);
                self.set_status(id, TargetStatus::NotConnected);
                let shown = match error {
                    RemoteError::Unauthorized => RemoteError::BadCredentials,
                    other => other,
                };
                self.sign_in.error.set(Some(SignInError::Remote(shown)));
            }
        }
    }

    // --- Signing in and out ---------------------------------------------------------------------------------

    /// Signs in to `id` with the form's username and password.
    pub fn sign_in(&self, id: TargetId) {
        if matches!(self.session_now(id), Session::SigningIn) {
            return;
        }
        let username = self.sign_in.username.get_untracked().trim().to_owned();
        let password = Secret::new(self.sign_in.password.get_untracked());
        if username.is_empty() || password.is_empty() {
            self.sign_in.error.set(Some(SignInError::Missing));
            return;
        }
        let Some(client) = self.client(id) else {
            return;
        };
        batch(|| {
            self.sign_in.password.set(String::new());
            self.sign_in.error.set(None);
            self.password_changed.set(None);
        });
        let epoch = self.epoch(id);
        self.set_session(id, Session::SigningIn);
        self.set_status(id, TargetStatus::Connecting);
        let this = *self;
        self.go(async move {
            let result = client.login(&username, &password).await;
            if !this.current_epoch(id, epoch) {
                return;
            }
            this.finish_login(id, &client, result).await;
        });
    }

    /// Signs out of `id`: the server is told, what is kept is deleted, and what was read is dropped.
    pub fn sign_out(&self, id: TargetId) {
        let client = self.client(id);
        let address = self.kept_address(id);
        self.bump_epoch(id);
        self.clear_data(id);
        self.password_changed.set(None);
        self.set_session(id, Session::NotSignedIn);
        self.set_status(id, TargetStatus::NotConnected);
        let this = *self;
        self.go(async move {
            // What is kept goes first: the logout request can take its whole timeout, and the app may be closed
            // before it ends. The client still holds the cookie the logout needs.
            this.forget_kept(id, address).await;
            if let Some(client) = client
                && let Err(error) = client.logout().await
            {
                log::debug!("signing out of server {}: {error}", id.number());
            }
        });
    }

    // --- Adding and removing servers -----------------------------------------------------------------------

    /// Shows the add-server dialog (the target switcher's "Add server…", wherever the user is).
    pub fn request_add_server(&self) {
        batch(|| {
            self.add.url.set(String::new());
            self.add.name.set(String::new());
            self.add.error.set(None);
            self.add.open.set(true);
        });
    }

    /// Closes the add-server dialog.
    pub fn close_add_server(&self) {
        self.add.open.set(false);
    }

    /// Adds the server the dialog describes, keeps it, selects it and opens its machines (where the sign-in form
    /// waits).
    pub fn add_server(&self) {
        let Some(store) = self.root() else {
            return;
        };
        let address = match ServerAddress::parse(&self.add.url.get_untracked()) {
            Ok(address) => address,
            Err(error) => {
                self.add.error.set(Some(AddError::Address(error)));
                return;
            }
        };
        let Some(base) = address.base().map(str::to_owned) else {
            self.add
                .error
                .set(Some(AddError::Address(RemoteError::InvalidAddress(
                    "no base URL".into(),
                ))));
            return;
        };
        let listed = untracked(|| store.targets());
        let duplicate = listed
            .iter()
            .find(|target| matches!(&target.origin, TargetOrigin::Url(url) if *url == base));
        if let Some(existing) = duplicate {
            let name = existing
                .name
                .as_ref()
                .map_or_else(|| base.clone(), ToString::to_string);
            self.add.error.set(Some(AddError::Duplicate(name)));
            return;
        }
        let Some(id) = store.next_target_id() else {
            self.add.error.set(Some(AddError::NoRoom));
            return;
        };
        let name: String = self
            .add
            .name
            .get_untracked()
            .trim()
            .chars()
            .take(MAX_NAME_CHARS)
            .collect();
        store.put_target(Target {
            id,
            kind: TargetKind::Server,
            origin: TargetOrigin::Url(base),
            name: (!name.is_empty()).then(|| SharedString::from(name)),
            status: TargetStatus::NotConnected,
        });
        // The id is never handed out again, even after this server is removed.
        store.record_target_id(id);
        self.persist_servers();
        batch(|| {
            self.sign_in.username.set(String::new());
            self.sign_in.error.set(None);
            self.add.open.set(false);
            store.select_target(id);
            store.open(Page::Machines);
        });
    }

    /// Removes server `id`: signs out, deletes what is kept for it and forgets it. Not the serving origin or the
    /// hosted control plane.
    pub fn remove_server(&self, id: TargetId) {
        let Some(store) = self.root() else {
            return;
        };
        let removable = untracked(|| store.target(id))
            .is_some_and(|target| matches!(target.origin, TargetOrigin::Url(_)));
        if !removable {
            log::warn!(
                "target {} is not a server the user added; not removed",
                id.number()
            );
            return;
        }
        self.sign_out(id);
        self.clients.with(|clients| clients.remove(&id));
        self.started.with(|started| started.remove(&id));
        self.sessions.update(|all| {
            all.remove(&id);
        });
        store.remove_target(id);
        self.persist_servers();
    }

    /// Keeps the servers the user added.
    fn persist_servers(&self) {
        let Some(store) = self.root() else {
            return;
        };
        let kept: Vec<StoredServer> = untracked(|| store.targets())
            .iter()
            .filter_map(|target| match (&target.kind, &target.origin) {
                (TargetKind::Server, TargetOrigin::Url(url)) => Some(StoredServer {
                    id: target.id.number(),
                    url: url.clone(),
                    name: target.name.as_ref().map(ToString::to_string),
                }),
                _ => None,
            })
            .collect();
        let written = serde_json::to_string(&kept)
            .map_err(|error| error.to_string())
            .and_then(|json| {
                Storage::set_item(SERVERS_KEY, json)
                    .map_err(|full| format!("storage full at {} bytes", full.bytes))
            });
        if let Err(error) = written {
            log::warn!("the servers were not kept: {error}");
        }
    }

    // --- Changing the password ------------------------------------------------------------------------------

    /// Opens the change-password dialog for `id`'s account.
    pub fn open_change_password(&self, id: TargetId) {
        batch(|| {
            self.password.new_password.set(String::new());
            self.password.confirm.set(String::new());
            self.password.error.set(None);
            self.password.busy.set(false);
            self.password.open.set(Some(id));
        });
    }

    /// Closes the change-password dialog and wipes what was typed.
    pub fn close_change_password(&self) {
        batch(|| {
            self.password.open.set(None);
            self.password.new_password.set(String::new());
            self.password.confirm.set(String::new());
        });
    }

    /// Changes the signed-in account's password. The server ends the session afterwards: the person signs in
    /// again with the new one.
    pub fn change_password(&self) {
        let Some(id) = self.password.open.get_untracked() else {
            return;
        };
        if self.password.busy.get_untracked() {
            return;
        }
        let new_password = self.password.new_password.get_untracked();
        if new_password.is_empty() {
            self.password.error.set(Some(PasswordError::Empty));
            return;
        }
        if new_password != self.password.confirm.get_untracked() {
            self.password.error.set(Some(PasswordError::Mismatch));
            return;
        }
        let Some(client) = self.client(id) else {
            return;
        };
        let address = client.address().clone();
        let secret = Secret::new(new_password);
        batch(|| {
            self.password.error.set(None);
            self.password.busy.set(true);
        });
        let epoch = self.epoch(id);
        let this = *self;
        self.go(async move {
            let result = client.change_password(&secret).await;
            this.password.busy.set(false);
            if !this.current_epoch(id, epoch) {
                return;
            }
            match result {
                Ok(()) => {
                    this.bump_epoch(id);
                    this.clear_data(id);
                    this.set_session(id, Session::NotSignedIn);
                    this.set_status(id, TargetStatus::NotConnected);
                    this.password_changed.set(Some(id));
                    this.close_change_password();
                    this.forget_kept(id, Some(address)).await;
                }
                Err(RemoteError::Unauthorized) => {
                    this.close_change_password();
                    this.expire(id);
                }
                Err(error) => this.password.error.set(Some(PasswordError::Remote(error))),
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(url: &str) -> ServerAddress {
        ServerAddress::parse(url).unwrap()
    }

    #[test]
    fn kept_entries_are_filed_under_the_address_not_the_number() {
        let a = base("https://et.example.com");
        let b = base("https://other.example.com");
        let cookie_a = cookie_account(&a).unwrap();
        assert_ne!(cookie_a, cookie_account(&b).unwrap());
        assert_ne!(cookie_a, account_account(&a).unwrap());
        // The same address always finds its own entry (normalized by the address parser).
        assert_eq!(
            cookie_a,
            cookie_account(&base("https://et.example.com/")).unwrap()
        );
        // 64 hex characters of sha256, no URL text in the account name.
        let digest = cookie_a
            .strip_prefix("server/")
            .and_then(|rest| rest.strip_suffix("/session"))
            .unwrap();
        assert_eq!(digest.len(), 64);
        assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(!cookie_a.contains("example"));
        // The browser keeps its own cookie: nothing to file.
        assert_eq!(cookie_account(&ServerAddress::SameOrigin), None);
        assert_eq!(account_account(&ServerAddress::SameOrigin), None);
    }

    #[test]
    fn the_number_keyed_entries_of_earlier_versions_are_only_ever_deleted() {
        let id = TargetId::from_number(16).unwrap();
        assert_eq!(
            legacy_accounts(id),
            [
                "server/16/session".to_owned(),
                "server/16/account".to_owned()
            ]
        );
        let address = base("https://et.example.com");
        assert!(!legacy_accounts(id).contains(&cookie_account(&address).unwrap()));
    }
}
