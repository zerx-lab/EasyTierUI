//! The console's REST client: [`RemoteClient`] sends the requests of [`api`](super::api) through NSH's `fetch`
//! and reads the replies with [`wire`](super::wire).
//!
//! It runs on the engine thread, inside `spawn(async { .. })`: `fetch` needs the engine, and the session lives in
//! a `RefCell` that is never held across an `await`. Every call races the request against a timer, so a server
//! that accepts the connection and then says nothing ends in [`RemoteError::Timeout`]. It works unchanged on the
//! web build, where `fetch` is the browser's.

use std::{
    cell::RefCell,
    collections::BTreeMap,
    fmt,
    future::{Future, poll_fn},
    pin::pin,
    rc::Rc,
    task::Poll,
    time::Duration,
};

use nsh_core::prelude::{HttpResponse, fetch, timer};
use uuid::Uuid;

use super::{
    address::ServerAddress,
    api::{self, ApiRequest, ApiResponse, CookieUpdate, CredentialRequest},
    error::RemoteError,
    form::NetworkConfigForm,
    model::{
        Credential, Machine, MachineNetworks, NetworkMeta, NetworkRuntime, NewCredential,
        PeerLinks, PeersSummary, ServerNetwork, ServerSession, ServerSummary,
    },
    patch::ConfigPatch,
    rpc_model::{
        AclStats, Connector, ForeignNetwork, ForeignNetworkHost, ForeignNetworkSummary, LogLevel,
        Metric, NodeFacts, PeerMapNode, PortForwardEntry, PublicIpv6Info, RouteEntry,
        VpnPortalInfo, Whitelist,
    },
    wire,
};
use crate::core::{secret::Secret, state::RouteKind};

/// How long a request may take before it is given up.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Longest transport error text kept (a browser's message names the URL and the likely CORS cause).
const MAX_NETWORK_MESSAGE_CHARS: usize = 300;

/// The sign-in state of one client.
#[derive(Default)]
struct Session {
    /// The session cookie where the platform exposes it (native). On the web the browser holds it, and this
    /// stays `None` while the session is valid.
    cookie: Option<Secret>,
    /// The account name of the last successful login, to show which account the app is signed in to.
    account: Option<String>,
}

struct Shared {
    address: ServerAddress,
    session: RefCell<Session>,
    timeout: Duration,
}

/// A client of one easytier-web server. Cloning shares the session.
#[derive(Clone)]
pub struct RemoteClient {
    shared: Rc<Shared>,
}

impl fmt::Debug for RemoteClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemoteClient")
            .field("address", &self.shared.address)
            .field("account", &self.account())
            .field("has_cookie", &self.has_session_cookie())
            .finish()
    }
}

impl RemoteClient {
    /// A client of `address`, not signed in.
    pub fn new(address: ServerAddress) -> Self {
        Self::with_session(address, None, None)
    }

    /// A client that continues a session the store kept: the cookie from the OS keychain and the account name. The
    /// first request that answers 401 shows the session is gone.
    pub fn with_session(
        address: ServerAddress,
        cookie: Option<Secret>,
        account: Option<String>,
    ) -> Self {
        Self {
            shared: Rc::new(Shared {
                address,
                session: RefCell::new(Session { cookie, account }),
                timeout: REQUEST_TIMEOUT,
            }),
        }
    }

    /// The server this client talks to.
    pub fn address(&self) -> &ServerAddress {
        &self.shared.address
    }

    /// The account of the last successful login.
    pub fn account(&self) -> Option<String> {
        self.shared
            .session
            .try_borrow()
            .ok()
            .and_then(|session| session.account.clone())
    }

    /// The session cookie, to keep in the OS keychain; `None` on the web build (the browser holds it) and when
    /// signed out. Never log it.
    pub fn session_cookie(&self) -> Option<Secret> {
        self.shared
            .session
            .try_borrow()
            .ok()
            .and_then(|session| session.cookie.clone())
    }

    /// Whether a session cookie is held here.
    pub fn has_session_cookie(&self) -> bool {
        self.shared
            .session
            .try_borrow()
            .is_ok_and(|session| session.cookie.is_some())
    }

    // --- Session ----------------------------------------------------------------------------------------

    /// Signs in. The password goes out as its MD5 hex (what easytier-web's frontend sends), so the address must be
    /// HTTPS or loopback ([`ServerAddress::parse`] enforces it). On native the reply's `Set-Cookie` becomes the
    /// session; on the web the browser keeps the cookie and nothing is stored here.
    ///
    /// # Errors
    ///
    /// [`RemoteError::BadCredentials`] for a wrong username or password; otherwise the errors of every call.
    pub async fn login(&self, username: &str, password: &Secret) -> Result<(), RemoteError> {
        match self.call(api::login(username, password)).await {
            Ok(_) => {
                self.update_session(|session| session.account = Some(username.to_owned()));
                Ok(())
            }
            Err(RemoteError::Rejected { status: 401, .. }) => Err(RemoteError::BadCredentials),
            Err(error) => Err(error),
        }
    }

    /// Signs out. The local session is dropped whatever the server answers: a failed request must not leave the
    /// app believing it is still signed in.
    ///
    /// # Errors
    ///
    /// The errors of every call, after the local session is already gone. [`RemoteError::Unauthorized`] means
    /// the server had ended it already.
    pub async fn logout(&self) -> Result<(), RemoteError> {
        let result = self.call(api::logout()).await.map(|_| ());
        self.end_session();
        result
    }

    /// Whether the server accepts the current session: `false` for 401 (and the held cookie is dropped).
    ///
    /// # Errors
    ///
    /// The errors of every call except [`RemoteError::Unauthorized`].
    pub async fn check_session(&self) -> Result<bool, RemoteError> {
        match self.call(api::check_login_status()).await {
            Ok(_) => Ok(true),
            Err(RemoteError::Unauthorized) => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Changes the signed-in account's password. The server ends the session afterwards (the old cookie is
    /// rejected), so this client is signed out when it returns: sign in again with the new password.
    ///
    /// # Errors
    ///
    /// The errors of every call. On error the session is unchanged.
    pub async fn change_password(&self, new_password: &Secret) -> Result<(), RemoteError> {
        self.call(api::change_password(new_password)).await?;
        self.end_session();
        Ok(())
    }

    // --- Server -----------------------------------------------------------------------------------------

    /// The server's device count.
    ///
    /// # Errors
    ///
    /// The errors of every call.
    pub async fn summary(&self) -> Result<ServerSummary, RemoteError> {
        wire::parse_summary(&self.call(api::summary()).await?)
    }

    /// The machines connected for this account.
    ///
    /// # Errors
    ///
    /// The errors of every call.
    pub async fn machines(&self) -> Result<Vec<Machine>, RemoteError> {
        wire::parse_machines(&self.call(api::machines()).await?)
    }

    /// Every machine session on the server, of every account (easytier-web does not filter them).
    ///
    /// # Errors
    ///
    /// The errors of every call.
    pub async fn sessions(&self) -> Result<Vec<ServerSession>, RemoteError> {
        wire::parse_sessions(&self.call(api::sessions()).await?)
    }

    // --- Networks of a machine ----------------------------------------------------------------------------

    /// The running and disabled instance ids of a machine.
    ///
    /// # Errors
    ///
    /// The errors of every call; [`RemoteError::NotFound`] when the machine is no longer connected.
    pub async fn machine_networks(&self, machine: Uuid) -> Result<MachineNetworks, RemoteError> {
        wire::parse_machine_networks(&self.call(api::machine_networks(machine)).await?)
    }

    /// The server's metadata (names, source, permission) of instances.
    ///
    /// # Errors
    ///
    /// The errors of every call.
    pub async fn network_metas(
        &self,
        machine: Uuid,
        instances: &[Uuid],
    ) -> Result<Vec<NetworkMeta>, RemoteError> {
        wire::parse_metas(&self.call(api::network_metas(machine, instances)).await?)
    }

    /// One instance as the machine reports it now; `None` when the machine has no such instance. This is the
    /// per-instance GET: the bulk form is a GET with a JSON body, which browsers cannot send.
    ///
    /// # Errors
    ///
    /// The errors of every call.
    pub async fn network_info(
        &self,
        machine: Uuid,
        instance: Uuid,
    ) -> Result<Option<NetworkRuntime>, RemoteError> {
        let body = self.call(api::network_info(machine, instance)).await?;
        wire::parse_network_info(&body, instance, nsh_core::time::unix_ms())
    }

    /// Every network of a machine for the list: running and disabled instances with their names and, for the
    /// running ones, the virtual IP and peer counts. A running instance whose info cannot be read is listed
    /// without them (the read is logged); sign-in and machine failures fail the whole call.
    ///
    /// # Errors
    ///
    /// [`RemoteError::Unauthorized`], [`RemoteError::NotFound`], [`RemoteError::Timeout`] and
    /// [`RemoteError::Network`] on the list or the metas; the same on a per-instance read end the call too, because
    /// the rest would fail the same way.
    pub async fn networks(&self, machine: Uuid) -> Result<Vec<ServerNetwork>, RemoteError> {
        let ids = self.machine_networks(machine).await?;
        let all: Vec<Uuid> = ids
            .running
            .iter()
            .chain(ids.disabled.iter())
            .copied()
            .collect();
        if all.is_empty() {
            return Ok(Vec::new());
        }
        let metas = self.network_metas(machine, &all).await?;
        let mut runtimes = BTreeMap::new();
        for instance in &ids.running {
            match self.network_info(machine, *instance).await {
                Ok(Some(runtime)) => {
                    runtimes.insert(*instance, runtime);
                }
                Ok(None) => {}
                Err(
                    error @ (RemoteError::Unauthorized
                    | RemoteError::NotFound(_)
                    | RemoteError::MachineNoAnswer(_)
                    | RemoteError::Timeout
                    | RemoteError::Network(_)),
                ) => return Err(error),
                Err(error) => {
                    log::warn!("reading network {instance} of machine {machine}: {error}")
                }
            }
        }
        Ok(assemble_networks(&ids, &metas, &runtimes))
    }

    /// An instance's config, editable.
    ///
    /// # Errors
    ///
    /// The errors of every call.
    pub async fn network_config(
        &self,
        machine: Uuid,
        instance: Uuid,
    ) -> Result<NetworkConfigForm, RemoteError> {
        let body = self.call(api::network_config(machine, instance)).await?;
        NetworkConfigForm::from_json(api::decode(&body)?)
    }

    /// Asks the machine's core to check `config` and render it as TOML. The TOML holds the network secret in
    /// clear, so it comes back as a [`Secret`].
    ///
    /// # Errors
    ///
    /// [`RemoteError::Rejected`] with the core's message when it refuses the config; the errors of every call.
    pub async fn validate_config(
        &self,
        machine: Uuid,
        config: &NetworkConfigForm,
    ) -> Result<Secret, RemoteError> {
        let body = self
            .call(api::validate_config(machine, &config.to_json()))
            .await?;
        wire::parse_toml_reply(&body)
    }

    /// Starts a network on a machine; with `save` the server keeps the config and pushes it again when the machine
    /// reconnects.
    ///
    /// # Errors
    ///
    /// The errors of every call.
    pub async fn run_network(
        &self,
        machine: Uuid,
        config: &NetworkConfigForm,
        save: bool,
    ) -> Result<(), RemoteError> {
        self.call(api::run_network(machine, &config.to_json(), save))
            .await
            .map(|_| ())
    }

    /// Replaces a saved instance's config.
    ///
    /// # Errors
    ///
    /// [`RemoteError::Invalid`] without a request when `config.instance_id` is not `instance` (the server
    /// would answer 400 `Instance ID mismatch`); the errors of every call.
    pub async fn update_network_config(
        &self,
        machine: Uuid,
        instance: Uuid,
        config: &NetworkConfigForm,
    ) -> Result<(), RemoteError> {
        if config.instance_id != instance {
            return Err(RemoteError::Invalid(
                "the config belongs to another network instance".into(),
            ));
        }
        self.call(api::update_network_config(
            machine,
            instance,
            &config.to_json(),
        ))
        .await
        .map(|_| ())
    }

    /// Switches an instance off (`true`) or on (`false`) without deleting its config.
    ///
    /// # Errors
    ///
    /// The errors of every call.
    pub async fn set_network_disabled(
        &self,
        machine: Uuid,
        instance: Uuid,
        disabled: bool,
    ) -> Result<(), RemoteError> {
        self.call(api::set_network_disabled(machine, instance, disabled))
            .await
            .map(|_| ())
    }

    /// Stops an instance and deletes its saved config.
    ///
    /// # Errors
    ///
    /// The errors of every call.
    pub async fn delete_network(&self, machine: Uuid, instance: Uuid) -> Result<(), RemoteError> {
        self.call(api::delete_network(machine, instance))
            .await
            .map(|_| ())
    }

    // --- Config conversion (no session needed) ------------------------------------------------------------

    /// Renders a config as the TOML file a node would use. It holds the network secret in clear, so it comes back
    /// as a [`Secret`].
    ///
    /// # Errors
    ///
    /// [`RemoteError::Rejected`] with the server's message when it refuses the config; the errors of every call.
    pub async fn generate_config(&self, config: &NetworkConfigForm) -> Result<Secret, RemoteError> {
        let body = self.call(api::generate_config(&config.to_json())).await?;
        wire::parse_toml_reply(&body)
    }

    /// Reads a TOML config file into a form (an import: show it before applying).
    ///
    /// # Errors
    ///
    /// [`RemoteError::Rejected`] with the parser's message when the TOML is not a config; the errors of every
    /// call.
    pub async fn parse_config(&self, toml: &str) -> Result<NetworkConfigForm, RemoteError> {
        let body = self.call(api::parse_config(toml)).await?;
        NetworkConfigForm::from_json(wire::parse_config_reply(&body)?)
    }

    // --- The machine's core, through proxy-rpc --------------------------------------------------------------

    /// The direct links to the peers of an instance.
    ///
    /// # Errors
    ///
    /// [`RemoteError::Rejected`] (status 500, `RPC Error`) when the core has no such instance; the errors of every
    /// call.
    pub async fn list_peers(
        &self,
        machine: Uuid,
        instance: Uuid,
    ) -> Result<Vec<PeerLinks>, RemoteError> {
        wire::parse_list_peers(&self.call(api::list_peers(machine, instance)).await?)
    }

    /// The active credentials of an instance.
    ///
    /// # Errors
    ///
    /// As [`RemoteClient::list_peers`].
    pub async fn list_credentials(
        &self,
        machine: Uuid,
        instance: Uuid,
    ) -> Result<Vec<Credential>, RemoteError> {
        wire::parse_credentials(&self.call(api::list_credentials(machine, instance)).await?)
    }

    /// Creates a credential: a key a node can join the network with, without the network secret. The secret in the
    /// result is not recoverable later.
    ///
    /// # Errors
    ///
    /// [`RemoteError::Invalid`] without a request when the lifetime is 0 (the core rejects it) or does not fit
    /// the wire's `i64`; as [`RemoteClient::list_peers`].
    pub async fn generate_credential(
        &self,
        machine: Uuid,
        instance: Uuid,
        request: &CredentialRequest,
    ) -> Result<NewCredential, RemoteError> {
        if request.ttl_seconds == 0 || i64::try_from(request.ttl_seconds).is_err() {
            return Err(RemoteError::Invalid(
                "a credential needs a lifetime of at least one second".into(),
            ));
        }
        let body = self
            .call(api::generate_credential(machine, instance, request))
            .await?;
        wire::parse_new_credential(&body)
    }

    /// Revokes a credential; `false` when the core had no such credential.
    ///
    /// # Errors
    ///
    /// As [`RemoteClient::list_peers`].
    pub async fn revoke_credential(
        &self,
        machine: Uuid,
        instance: Uuid,
        credential_id: &str,
    ) -> Result<bool, RemoteError> {
        let body = self
            .call(api::revoke_credential(machine, instance, credential_id))
            .await?;
        wire::parse_revoked(&body)
    }

    // --- More services of the machine's core, through proxy-rpc ---------------------------------------------
    //
    // Every call below fails like `list_peers`: [`RemoteError::Rejected`] (status 500, `RPC Error`) when the core
    // has no such instance, [`RemoteError::MachineNoAnswer`] when the core is silent or gone, plus the errors of
    // every call.

    /// The node's own facts: ids, version, virtual IP, listeners, STUN result, interface addresses, features. The
    /// reply also carries the node's rendered config with the network secret; it is not read.
    ///
    /// # Errors
    ///
    /// As [`RemoteClient::list_peers`].
    pub async fn node_info(&self, machine: Uuid, instance: Uuid) -> Result<NodeFacts, RemoteError> {
        wire::parse_node_facts(&self.call(api::show_node_info(machine, instance)).await?)
    }

    /// The routing table: every node of the network with its hops, latency and next hop.
    ///
    /// # Errors
    ///
    /// As [`RemoteClient::list_peers`].
    pub async fn list_routes(
        &self,
        machine: Uuid,
        instance: Uuid,
    ) -> Result<Vec<RouteEntry>, RemoteError> {
        wire::parse_routes(&self.call(api::list_routes(machine, instance)).await?)
    }

    /// The networks this node relays without being a member, with their peers and links.
    ///
    /// # Errors
    ///
    /// As [`RemoteClient::list_peers`].
    pub async fn list_foreign_networks(
        &self,
        machine: Uuid,
        instance: Uuid,
    ) -> Result<Vec<ForeignNetwork>, RemoteError> {
        wire::parse_foreign_networks(
            &self
                .call(api::list_foreign_networks(machine, instance))
                .await?,
        )
    }

    /// The foreign networks relayed anywhere in the mesh, by relaying node.
    ///
    /// # Errors
    ///
    /// As [`RemoteClient::list_peers`].
    pub async fn list_global_foreign_networks(
        &self,
        machine: Uuid,
        instance: Uuid,
    ) -> Result<Vec<ForeignNetworkHost>, RemoteError> {
        wire::parse_global_foreign_networks(
            &self
                .call(api::list_global_foreign_networks(machine, instance))
                .await?,
        )
    }

    /// How many foreign networks and peers each node of the mesh relays.
    ///
    /// # Errors
    ///
    /// As [`RemoteClient::list_peers`].
    pub async fn foreign_network_summary(
        &self,
        machine: Uuid,
        instance: Uuid,
    ) -> Result<Vec<ForeignNetworkSummary>, RemoteError> {
        wire::parse_foreign_network_summary(
            &self
                .call(api::foreign_network_summary(machine, instance))
                .await?,
        )
    }

    /// The public IPv6 prefix this node leases from and the leases it handed out.
    ///
    /// # Errors
    ///
    /// As [`RemoteClient::list_peers`].
    pub async fn list_public_ipv6(
        &self,
        machine: Uuid,
        instance: Uuid,
    ) -> Result<PublicIpv6Info, RemoteError> {
        wire::parse_public_ipv6(&self.call(api::list_public_ipv6(machine, instance)).await?)
    }

    /// The mesh's direct links with their latency, as the peer center collected them. It names no instance: the
    /// core's only (or first) network answers.
    ///
    /// # Errors
    ///
    /// As [`RemoteClient::list_peers`].
    pub async fn global_peer_map(&self, machine: Uuid) -> Result<Vec<PeerMapNode>, RemoteError> {
        wire::parse_global_peer_map(&self.call(api::global_peer_map(machine)).await?)
    }

    /// The manual connectors with their link status.
    ///
    /// # Errors
    ///
    /// As [`RemoteClient::list_peers`].
    pub async fn list_connectors(
        &self,
        machine: Uuid,
        instance: Uuid,
    ) -> Result<Vec<Connector>, RemoteError> {
        wire::parse_connectors(&self.call(api::list_connectors(machine, instance)).await?)
    }

    /// The mapped listeners' urls.
    ///
    /// # Errors
    ///
    /// As [`RemoteClient::list_peers`].
    pub async fn list_mapped_listeners(
        &self,
        machine: Uuid,
        instance: Uuid,
    ) -> Result<Vec<String>, RemoteError> {
        wire::parse_mapped_listeners(
            &self
                .call(api::list_mapped_listeners(machine, instance))
                .await?,
        )
    }

    /// The VPN portal and its connected clients. Its `client_config` holds a WireGuard private key
    /// ([`Secret`]).
    ///
    /// # Errors
    ///
    /// As [`RemoteClient::list_peers`].
    pub async fn vpn_portal_info(
        &self,
        machine: Uuid,
        instance: Uuid,
    ) -> Result<VpnPortalInfo, RemoteError> {
        wire::parse_vpn_portal(&self.call(api::vpn_portal_info(machine, instance)).await?)
    }

    /// The ACL's rule hit counts, tracked connections and global counters.
    ///
    /// # Errors
    ///
    /// As [`RemoteClient::list_peers`].
    pub async fn acl_stats(&self, machine: Uuid, instance: Uuid) -> Result<AclStats, RemoteError> {
        wire::parse_acl_stats(&self.call(api::acl_stats(machine, instance)).await?)
    }

    /// The TCP and UDP ports open to the mesh.
    ///
    /// # Errors
    ///
    /// As [`RemoteClient::list_peers`].
    pub async fn whitelist(&self, machine: Uuid, instance: Uuid) -> Result<Whitelist, RemoteError> {
        wire::parse_whitelist(&self.call(api::whitelist(machine, instance)).await?)
    }

    /// The port forwards of the running instance.
    ///
    /// # Errors
    ///
    /// As [`RemoteClient::list_peers`].
    pub async fn list_port_forwards(
        &self,
        machine: Uuid,
        instance: Uuid,
    ) -> Result<Vec<PortForwardEntry>, RemoteError> {
        wire::parse_port_forwards(
            &self
                .call(api::list_port_forwards(machine, instance))
                .await?,
        )
    }

    /// The core's metrics (counters with labels).
    ///
    /// # Errors
    ///
    /// As [`RemoteClient::list_peers`].
    pub async fn stats(&self, machine: Uuid, instance: Uuid) -> Result<Vec<Metric>, RemoteError> {
        wire::parse_stats(&self.call(api::stats(machine, instance)).await?)
    }

    /// The same metrics as Prometheus exposition text.
    ///
    /// # Errors
    ///
    /// As [`RemoteClient::list_peers`].
    pub async fn prometheus_stats(
        &self,
        machine: Uuid,
        instance: Uuid,
    ) -> Result<String, RemoteError> {
        wire::parse_prometheus(&self.call(api::prometheus_stats(machine, instance)).await?)
    }

    /// The core process's log level. It is process-wide: every instance of the machine's core shares it.
    ///
    /// # Errors
    ///
    /// [`RemoteError::Decode`] for a level this app does not know; as [`RemoteClient::list_peers`] otherwise.
    pub async fn logger_level(&self, machine: Uuid) -> Result<LogLevel, RemoteError> {
        wire::parse_log_level(&self.call(api::get_logger_config(machine)).await?)
    }

    /// Sets the core process's log level, for every instance of the machine's core, until the core restarts.
    ///
    /// # Errors
    ///
    /// As [`RemoteClient::list_peers`].
    pub async fn set_logger_level(
        &self,
        machine: Uuid,
        level: LogLevel,
    ) -> Result<(), RemoteError> {
        self.call(api::set_logger_config(machine, level))
            .await
            .map(|_| ())
    }

    /// The instance's running config, asked of the core itself: it shows what [`RemoteClient::patch_config`]
    /// changed, which the saved config ([`RemoteClient::network_config`]) does not. It includes the network
    /// secret.
    ///
    /// # Errors
    ///
    /// As [`RemoteClient::list_peers`].
    pub async fn running_config(
        &self,
        machine: Uuid,
        instance: Uuid,
    ) -> Result<NetworkConfigForm, RemoteError> {
        wire::parse_running_config(&self.call(api::get_config(machine, instance)).await?)
    }

    /// Applies a [`ConfigPatch`] to the **running** instance: no restart, but also nothing saved. easytier-web
    /// keeps its own copy of the config and pushes it again when the machine reconnects or restarts, which undoes
    /// the patch; edit the saved config ([`RemoteClient::update_network_config`]) to keep a change.
    ///
    /// # Errors
    ///
    /// [`RemoteError::Invalid`] without a request for an empty patch; as [`RemoteClient::list_peers`] otherwise
    /// (the core answers 500 when it refuses a value, for example a mapped CIDR of another size).
    pub async fn patch_config(
        &self,
        machine: Uuid,
        instance: Uuid,
        patch: &ConfigPatch,
    ) -> Result<(), RemoteError> {
        if patch.is_empty() {
            return Err(RemoteError::Invalid("the change is empty".into()));
        }
        self.call(api::patch_config(machine, instance, patch))
            .await
            .map(|_| ())
    }

    // --- Plumbing ---------------------------------------------------------------------------------------

    /// Sends a request and returns the body of a success.
    async fn call(&self, request: ApiRequest) -> Result<Vec<u8>, RemoteError> {
        let response = self.send(&request).await?;
        match response.into_success() {
            Err(RemoteError::Unauthorized) => {
                // The server no longer knows the cookie: do not keep presenting it.
                self.update_session(|session| session.cookie = None);
                Err(RemoteError::Unauthorized)
            }
            other => other,
        }
    }

    /// Sends a request, races it against the timeout and applies what the reply did to the session cookie.
    async fn send(&self, request: &ApiRequest) -> Result<ApiResponse, RemoteError> {
        let cookie = self.session_cookie();
        let http = api::http_request(&self.shared.address, cookie.as_ref(), request);
        let response: HttpResponse = match with_timeout(fetch(http), self.shared.timeout).await {
            None => return Err(RemoteError::Timeout),
            Some(Err(message)) => {
                return Err(RemoteError::Network(
                    message.chars().take(MAX_NETWORK_MESSAGE_CHARS).collect(),
                ));
            }
            Some(Ok(response)) => response,
        };
        let response = ApiResponse::from_http(response);
        match &response.cookie {
            CookieUpdate::Unchanged => {}
            CookieUpdate::Set(cookie) => {
                let cookie = cookie.clone();
                self.update_session(move |session| session.cookie = Some(cookie));
            }
            CookieUpdate::Cleared => self.update_session(|session| session.cookie = None),
        }
        Ok(response)
    }

    fn update_session(&self, change: impl FnOnce(&mut Session)) {
        match self.shared.session.try_borrow_mut() {
            Ok(mut session) => change(&mut session),
            Err(_) => {
                log::error!("the console session was borrowed while it changed; update dropped")
            }
        }
    }

    fn end_session(&self) {
        self.update_session(|session| *session = Session::default());
    }
}

/// Combines the list, the metas and the running info into the rows of a machine's network list.
///
/// Running ids come first, then disabled ones, each in the server's order. An instance the metas do not name
/// (a race with a delete) gets its id as its name.
pub(crate) fn assemble_networks(
    ids: &MachineNetworks,
    metas: &[NetworkMeta],
    runtimes: &BTreeMap<Uuid, NetworkRuntime>,
) -> Vec<ServerNetwork> {
    let row = |instance: Uuid, running: bool| {
        let meta = metas.iter().find(|meta| meta.instance_id == instance);
        let runtime = runtimes.get(&instance);
        let peers = runtime.map(|runtime| {
            let mut summary = PeersSummary::default();
            for peer in &runtime.snapshot.peers {
                summary.total = summary.total.saturating_add(1);
                match peer.route {
                    RouteKind::Direct => summary.direct = summary.direct.saturating_add(1),
                    RouteKind::Relayed { .. } => {
                        summary.relayed = summary.relayed.saturating_add(1)
                    }
                    RouteKind::Unknown => {}
                }
            }
            summary
        });
        let name = meta.map_or_else(|| instance.to_string(), |meta| meta.network_name.clone());
        ServerNetwork {
            instance_id: instance,
            instance_name: meta.map_or_else(|| name.clone(), |meta| meta.instance_name.clone()),
            name,
            running,
            disabled: !running,
            virtual_ipv4: runtime.and_then(|runtime| runtime.snapshot.node.virtual_ipv4),
            peers,
            error: runtime.and_then(|runtime| runtime.error.clone()),
            source: meta.map(|meta| meta.source).unwrap_or_default(),
            config_permission: meta.map_or(0, |meta| meta.config_permission),
        }
    };
    ids.running
        .iter()
        .map(|instance| row(*instance, true))
        .chain(ids.disabled.iter().map(|instance| row(*instance, false)))
        .collect()
}

/// Waits for `future` at most `limit`; `None` on timeout (the request is dropped, which cancels it).
async fn with_timeout<F: Future>(future: F, limit: Duration) -> Option<F::Output> {
    let mut future = pin!(future);
    let mut delay = pin!(timer(limit));
    poll_fn(|cx| {
        if let Poll::Ready(output) = future.as_mut().poll(cx) {
            return Poll::Ready(Some(output));
        }
        if delay.as_mut().poll(cx).is_ready() {
            return Poll::Ready(None);
        }
        Poll::Pending
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        remote::model::ConfigSource,
        state::{NodeInfo, NodeSnapshot, PeerSummary},
    };

    fn uuid(last: u8) -> Uuid {
        Uuid::from_bytes([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, last])
    }

    fn runtime(peers: Vec<PeerSummary>) -> NetworkRuntime {
        NetworkRuntime {
            running: true,
            error: None,
            snapshot: NodeSnapshot {
                read_at_ms: 0.0,
                node: NodeInfo {
                    virtual_ipv4: "10.0.0.1/24".parse().ok(),
                    ..NodeInfo::default()
                },
                peers,
            },
        }
    }

    #[test]
    fn network_rows_combine_ids_metas_and_runtime() {
        let ids = MachineNetworks {
            running: vec![uuid(1), uuid(2)],
            disabled: vec![uuid(3)],
        };
        let metas = vec![
            NetworkMeta {
                instance_id: uuid(1),
                network_name: "alpha".into(),
                instance_name: "alpha-inst".into(),
                source: ConfigSource::User,
                config_permission: 0,
            },
            NetworkMeta {
                instance_id: uuid(3),
                network_name: "gamma".into(),
                instance_name: "gamma".into(),
                source: ConfigSource::Webhook,
                config_permission: 1,
            },
        ];
        let direct = PeerSummary {
            route: RouteKind::Direct,
            ..PeerSummary::default()
        };
        let relayed = PeerSummary {
            route: RouteKind::Relayed { hops: 2 },
            ..PeerSummary::default()
        };
        let runtimes = BTreeMap::from([(uuid(1), runtime(vec![direct.clone(), direct, relayed]))]);
        let rows = assemble_networks(&ids, &metas, &runtimes);
        assert_eq!(rows.len(), 3);

        assert_eq!(rows[0].name, "alpha");
        assert_eq!(rows[0].instance_name, "alpha-inst");
        assert!(rows[0].running && !rows[0].disabled);
        assert_eq!(rows[0].virtual_ipv4, "10.0.0.1/24".parse().ok());
        assert_eq!(
            rows[0].peers,
            Some(PeersSummary {
                total: 3,
                direct: 2,
                relayed: 1
            })
        );
        assert!(rows[0].is_editable());

        // Running but not named by the metas and not read: its id stands in for the name.
        assert_eq!(rows[1].name, uuid(2).to_string());
        assert_eq!(rows[1].peers, None);

        assert!(rows[2].disabled && !rows[2].running);
        assert_eq!(rows[2].source, ConfigSource::Webhook);
        assert!(!rows[2].is_editable());
    }
}
