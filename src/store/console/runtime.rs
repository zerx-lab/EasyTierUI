//! What the console reads from a machine's running network through easytier-web's `proxy-rpc`, and the runtime
//! changes it sends there: connectors, mapped listeners, port forwards, subnet proxies, routes, exit nodes, the
//! whitelist and the node's own settings (`PatchConfig`), plus the core's log level.
//!
//! A patch changes the **running** instance only. easytier-web keeps its own copy of the config and pushes it
//! again when the machine reconnects or restarts, which undoes the patch; the pages say so next to every change.
//! Every value is checked by [`ConfigPatch`] before anything is sent: a rejected value shows on its form and no
//! request leaves. Reads keep the last good value like every other [`Feed`]; a change that succeeded reads the
//! lists it touched again.
//!
//! Two answers carry secrets and neither is kept: the running config is reduced to the fields the pages show (the
//! network secret never leaves `core::remote`), and the VPN portal's client config (a WireGuard private key) is
//! read only by [`ConsoleStore::reveal_vpn_config`] and forgotten by [`ConsoleStore::hide_vpn_config`].

use std::collections::{BTreeMap, BTreeSet};

use nsh_core::{prelude::*, time::unix_ms};
use uuid::Uuid;

use super::{ConsoleStore, Feed};
use crate::core::{
    remote::{
        AclStats, ConfigPatch, Connector, LogLevel, NetworkConfigForm, PortForwardEntry,
        RemoteClient, RemoteError, TransportProtocol, Whitelist,
    },
    secret::Secret,
    target::TargetId,
};

/// What a runtime read or change is about: one network instance of one machine, or (with a nil instance) the
/// machine's core itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Scope {
    /// The server.
    pub target: TargetId,
    /// The machine.
    pub machine: Uuid,
    /// The network instance; nil for the machine's core.
    pub instance: Uuid,
}

impl Scope {
    /// Network `instance` of `machine` on `target`.
    pub fn new(target: TargetId, machine: Uuid, instance: Uuid) -> Self {
        Self {
            target,
            machine,
            instance,
        }
    }

    /// `machine`'s core as a whole (the log level is its only setting).
    pub fn machine(target: TargetId, machine: Uuid) -> Self {
        Self::new(target, machine, Uuid::nil())
    }
}

/// A subnet the node proxies into the network, as the running config lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyNet {
    /// The subnet (`192.168.50.0/24`).
    pub cidr: String,
    /// The range it is announced as, when it differs.
    pub mapped: Option<String>,
}

/// The parts of a network's running config the runtime pages show. The network secret is not among them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunningFacts {
    /// The hostname the node announces; `None` for the machine's own.
    pub hostname: Option<String>,
    /// Whether the address comes from DHCP.
    pub dhcp: bool,
    /// The static virtual IPv4 with its prefix, when one is set.
    pub virtual_ipv4: Option<String>,
    /// Subnets proxied into the network.
    pub proxy_networks: Vec<ProxyNet>,
    /// Manual routes.
    pub routes: Vec<String>,
    /// Whether the config has manual routing on: routes listed there do nothing otherwise.
    pub manual_routes_on: bool,
    /// Exit nodes.
    pub exit_nodes: Vec<String>,
    /// Whether the node refuses to relay other nodes' data.
    pub relay_data_disabled: bool,
}

impl RunningFacts {
    /// The shown fields of `form`.
    fn of(form: &NetworkConfigForm) -> Self {
        Self {
            hostname: form.hostname.clone(),
            dhcp: form.dhcp,
            virtual_ipv4: form.virtual_ipv4.map(|net| net.to_string()),
            proxy_networks: form
                .proxy_cidrs
                .iter()
                .map(|entry| match entry.split_once("->") {
                    Some((cidr, mapped)) => ProxyNet {
                        cidr: cidr.trim().to_owned(),
                        mapped: Some(mapped.trim().to_owned()),
                    },
                    None => ProxyNet {
                        cidr: entry.trim().to_owned(),
                        mapped: None,
                    },
                })
                .collect(),
            routes: form.manual_routes.clone().unwrap_or_default(),
            manual_routes_on: form.manual_routes.is_some(),
            exit_nodes: form.exit_nodes.clone(),
            relay_data_disabled: form.relay_data_disabled(),
        }
    }
}

/// The VPN portal as the pages list it: its kind and who is connected. The client config is not part of it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VpnSummary {
    /// The portal's kind (`wireguard`); empty when the network has none.
    pub vpn_type: String,
    /// The clients connected right now.
    pub connected_clients: Vec<String>,
}

impl VpnSummary {
    /// Whether the network runs a portal.
    pub fn is_enabled(&self) -> bool {
        !self.vpn_type.trim().is_empty()
    }
}

/// A form of the runtime pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Form {
    /// Add a connector.
    Connector,
    /// Add a mapped listener.
    MappedListener,
    /// Add a port forward.
    PortForward,
    /// Add a subnet proxy.
    SubnetProxy,
    /// Add a manual route.
    Route,
    /// Add an exit node.
    ExitNode,
    /// Open a whitelist port.
    Whitelist,
    /// Change the hostname.
    Hostname,
    /// Change the virtual IPv4.
    VirtualIp,
}

/// The text the runtime forms hold, and what each rejected.
#[derive(Clone, Copy)]
pub struct RuntimeFields {
    /// The connector URL.
    pub connector: Signal<String>,
    /// The mapped listener URL.
    pub mapped_listener: Signal<String>,
    /// The port forward's protocol.
    pub forward_protocol: Signal<TransportProtocol>,
    /// The port forward's bind address (`ip:port`).
    pub forward_bind: Signal<String>,
    /// The port forward's destination (`ip:port`).
    pub forward_dst: Signal<String>,
    /// The subnet to proxy.
    pub subnet: Signal<String>,
    /// The range the subnet is announced as; empty for none.
    pub subnet_mapped: Signal<String>,
    /// The manual route.
    pub route: Signal<String>,
    /// The exit node's address.
    pub exit_node: Signal<String>,
    /// The whitelist entry's protocol.
    pub whitelist_protocol: Signal<TransportProtocol>,
    /// The whitelist port or range.
    pub whitelist_port: Signal<String>,
    /// The new hostname.
    pub hostname: Signal<String>,
    /// The new virtual IPv4 with its prefix.
    pub virtual_ip: Signal<String>,
    errors: Signal<BTreeMap<Form, RemoteError>>,
}

impl RuntimeFields {
    fn new() -> Self {
        Self {
            connector: signal(String::new()),
            mapped_listener: signal(String::new()),
            forward_protocol: signal(TransportProtocol::Tcp),
            forward_bind: signal(String::new()),
            forward_dst: signal(String::new()),
            subnet: signal(String::new()),
            subnet_mapped: signal(String::new()),
            route: signal(String::new()),
            exit_node: signal(String::new()),
            whitelist_protocol: signal(TransportProtocol::Tcp),
            whitelist_port: signal(String::new()),
            hostname: signal(String::new()),
            virtual_ip: signal(String::new()),
            errors: signal(BTreeMap::new()),
        }
    }

    /// Empties `form`'s fields (its protocol choice stays).
    fn clear(&self, form: Form) {
        batch(|| {
            match form {
                Form::Connector => self.connector.set(String::new()),
                Form::MappedListener => self.mapped_listener.set(String::new()),
                Form::PortForward => {
                    self.forward_bind.set(String::new());
                    self.forward_dst.set(String::new());
                }
                Form::SubnetProxy => {
                    self.subnet.set(String::new());
                    self.subnet_mapped.set(String::new());
                }
                Form::Route => self.route.set(String::new()),
                Form::ExitNode => self.exit_node.set(String::new()),
                Form::Whitelist => self.whitelist_port.set(String::new()),
                Form::Hostname => self.hostname.set(String::new()),
                Form::VirtualIp => self.virtual_ip.set(String::new()),
            }
            self.errors.update(|errors| {
                errors.remove(&form);
            });
        });
    }

    /// Empties every form and forgets what they rejected.
    fn reset(&self) {
        batch(|| {
            for form in [
                Form::Connector,
                Form::MappedListener,
                Form::PortForward,
                Form::SubnetProxy,
                Form::Route,
                Form::ExitNode,
                Form::Whitelist,
                Form::Hostname,
                Form::VirtualIp,
            ] {
                self.clear(form);
            }
        });
    }
}

/// A runtime change the machine refused or could not be asked, for the banner of its tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFailure {
    /// What it was about.
    pub scope: Scope,
    /// Why it failed.
    pub error: RemoteError,
}

/// A VPN client config shown on request.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Revealed {
    scope: Scope,
    config: Secret,
}

/// One read per key.
type FeedMap<T> = Signal<BTreeMap<Scope, Feed<T>>>;

/// The runtime pages' state: a feed per kind of read, the forms, and what is in flight.
#[derive(Clone, Copy)]
pub(super) struct RuntimeState {
    connectors: FeedMap<Vec<Connector>>,
    mapped_listeners: FeedMap<Vec<String>>,
    port_forwards: FeedMap<Vec<PortForwardEntry>>,
    running: FeedMap<RunningFacts>,
    vpn: FeedMap<VpnSummary>,
    whitelist: FeedMap<Whitelist>,
    acl: FeedMap<AclStats>,
    log_level: FeedMap<LogLevel>,
    /// Scopes with a change in flight.
    applying: Signal<BTreeSet<Scope>>,
    failure: Signal<Option<RuntimeFailure>>,
    revealed: Signal<Option<Revealed>>,
    revealing: Signal<bool>,
    fields: RuntimeFields,
}

impl RuntimeState {
    pub(super) fn new() -> Self {
        Self {
            connectors: signal(BTreeMap::new()),
            mapped_listeners: signal(BTreeMap::new()),
            port_forwards: signal(BTreeMap::new()),
            running: signal(BTreeMap::new()),
            vpn: signal(BTreeMap::new()),
            whitelist: signal(BTreeMap::new()),
            acl: signal(BTreeMap::new()),
            log_level: signal(BTreeMap::new()),
            applying: signal(BTreeSet::new()),
            failure: signal(None),
            revealed: signal(None),
            revealing: signal(false),
            fields: RuntimeFields::new(),
        }
    }
}

/// The list a patch changes, so the pages read it again afterwards.
#[derive(Debug, Clone, Copy)]
enum Change {
    Connectors,
    MappedListeners,
    PortForwards,
    Running,
    Whitelist,
}

/// `map`'s feed for `scope` (tracked).
fn feed_of<T: Clone + 'static>(map: FeedMap<T>, scope: Scope) -> Feed<T> {
    map.with(|all| all.get(&scope).cloned()).unwrap_or_default()
}

/// Ends the reads of `target` that show as loading (their answers are dropped with the epoch).
fn stop<T: Clone + 'static>(map: FeedMap<T>, target: TargetId) {
    map.update(|all| {
        for (scope, feed) in all.iter_mut() {
            if scope.target == target {
                feed.loading = false;
            }
        }
    });
}

impl ConsoleStore {
    // --- Reads -------------------------------------------------------------------------------------------------

    /// The manual connectors of `scope` with their status (tracked).
    pub fn connectors(&self, scope: Scope) -> Feed<Vec<Connector>> {
        feed_of(self.runtime.connectors, scope)
    }

    /// The mapped listeners' URLs of `scope` (tracked).
    pub fn mapped_listeners(&self, scope: Scope) -> Feed<Vec<String>> {
        feed_of(self.runtime.mapped_listeners, scope)
    }

    /// The port forwards of `scope` (tracked).
    pub fn port_forwards(&self, scope: Scope) -> Feed<Vec<PortForwardEntry>> {
        feed_of(self.runtime.port_forwards, scope)
    }

    /// The running config's shown fields for `scope` (tracked).
    pub fn running_facts(&self, scope: Scope) -> Feed<RunningFacts> {
        feed_of(self.runtime.running, scope)
    }

    /// The VPN portal of `scope` (tracked).
    pub fn vpn_portal(&self, scope: Scope) -> Feed<VpnSummary> {
        feed_of(self.runtime.vpn, scope)
    }

    /// The open ports of `scope` (tracked).
    pub fn whitelist(&self, scope: Scope) -> Feed<Whitelist> {
        feed_of(self.runtime.whitelist, scope)
    }

    /// The ACL's counters of `scope` (tracked).
    pub fn acl_stats(&self, scope: Scope) -> Feed<AclStats> {
        feed_of(self.runtime.acl, scope)
    }

    /// The core's log level on the machine of `scope` (tracked).
    pub fn log_level(&self, scope: Scope) -> Feed<LogLevel> {
        feed_of(self.runtime.log_level, scope)
    }

    /// Reads `map`'s feed for `scope` with `call`, unless a read is already in flight. The read starts in the task,
    /// so asking for it while a tab is being built never writes a signal there.
    fn read_into<T, Fut>(
        &self,
        scope: Scope,
        map: FeedMap<T>,
        call: impl FnOnce(RemoteClient) -> Fut + 'static,
    ) where
        T: Clone + 'static,
        Fut: Future<Output = Result<T, RemoteError>> + 'static,
    {
        let Some(client) = self.client(scope.target) else {
            return;
        };
        let epoch = self.epoch(scope.target);
        let this = *self;
        self.go(async move {
            let mut started = false;
            map.update(|all| {
                let feed = all.entry(scope).or_default();
                if !feed.loading {
                    feed.begin();
                    started = true;
                }
            });
            if !started {
                return;
            }
            let result = call(client).await;
            if !this.current_epoch(scope.target, epoch) {
                return;
            }
            match result {
                Ok(value) => {
                    map.update(|all| all.entry(scope).or_default().succeed(value, unix_ms()))
                }
                Err(error) => {
                    map.update(|all| all.entry(scope).or_default().fail(error.clone()));
                    this.react_to(scope.target, &error);
                }
            }
        });
    }

    /// Reads the connectors again.
    pub fn refresh_connectors(&self, scope: Scope) {
        let Scope {
            machine, instance, ..
        } = scope;
        self.read_into(scope, self.runtime.connectors, move |client| async move {
            client.list_connectors(machine, instance).await
        });
    }

    /// Reads the mapped listeners again.
    pub fn refresh_mapped_listeners(&self, scope: Scope) {
        let Scope {
            machine, instance, ..
        } = scope;
        self.read_into(
            scope,
            self.runtime.mapped_listeners,
            move |client| async move { client.list_mapped_listeners(machine, instance).await },
        );
    }

    /// Reads the port forwards again.
    pub fn refresh_port_forwards(&self, scope: Scope) {
        let Scope {
            machine, instance, ..
        } = scope;
        self.read_into(
            scope,
            self.runtime.port_forwards,
            move |client| async move { client.list_port_forwards(machine, instance).await },
        );
    }

    /// Reads the running config again, keeping only the fields the pages show.
    pub fn refresh_running_facts(&self, scope: Scope) {
        let Scope {
            machine, instance, ..
        } = scope;
        self.read_into(scope, self.runtime.running, move |client| async move {
            let form = client.running_config(machine, instance).await?;
            Ok(RunningFacts::of(&form))
        });
    }

    /// Reads the VPN portal again, without its client config.
    pub fn refresh_vpn_portal(&self, scope: Scope) {
        let Scope {
            machine, instance, ..
        } = scope;
        self.read_into(scope, self.runtime.vpn, move |client| async move {
            let info = client.vpn_portal_info(machine, instance).await?;
            Ok(VpnSummary {
                vpn_type: info.vpn_type.clone(),
                connected_clients: info.connected_clients.clone(),
            })
        });
    }

    /// Reads the open ports again.
    pub fn refresh_whitelist(&self, scope: Scope) {
        let Scope {
            machine, instance, ..
        } = scope;
        self.read_into(scope, self.runtime.whitelist, move |client| async move {
            client.whitelist(machine, instance).await
        });
    }

    /// Reads the ACL's counters again.
    pub fn refresh_acl_stats(&self, scope: Scope) {
        let Scope {
            machine, instance, ..
        } = scope;
        self.read_into(scope, self.runtime.acl, move |client| async move {
            client.acl_stats(machine, instance).await
        });
    }

    /// Reads the core's log level again.
    pub fn refresh_log_level(&self, scope: Scope) {
        let machine = scope.machine;
        self.read_into(scope, self.runtime.log_level, move |client| async move {
            client.logger_level(machine).await
        });
    }

    /// Reads what the Connections tab shows.
    pub fn refresh_connections(&self, scope: Scope) {
        self.refresh_connectors(scope);
        self.refresh_mapped_listeners(scope);
    }

    /// Reads what the Forwarding tab shows.
    pub fn refresh_forwarding(&self, scope: Scope) {
        self.refresh_port_forwards(scope);
        self.refresh_running_facts(scope);
        self.refresh_vpn_portal(scope);
    }

    /// Reads what the Access tab shows.
    pub fn refresh_access(&self, scope: Scope) {
        self.refresh_whitelist(scope);
        self.refresh_acl_stats(scope);
    }

    /// Reads the list `change` touched.
    fn refresh_change(&self, scope: Scope, change: Change) {
        match change {
            Change::Connectors => self.refresh_connectors(scope),
            Change::MappedListeners => self.refresh_mapped_listeners(scope),
            Change::PortForwards => self.refresh_port_forwards(scope),
            Change::Running => self.refresh_running_facts(scope),
            Change::Whitelist => self.refresh_whitelist(scope),
        }
    }

    // --- State -------------------------------------------------------------------------------------------------

    /// The text the runtime forms hold.
    pub fn runtime_fields(&self) -> RuntimeFields {
        self.runtime.fields
    }

    /// Why `form` refused its values (tracked); the request was not sent.
    pub fn form_error(&self, form: Form) -> Option<RemoteError> {
        self.runtime
            .fields
            .errors
            .with(|errors| errors.get(&form).cloned())
    }

    /// Whether a change on `scope` is in flight (tracked).
    pub fn is_applying(&self, scope: Scope) -> bool {
        self.runtime
            .applying
            .with(|applying| applying.contains(&scope))
    }

    /// Why the last change on `scope` failed (tracked).
    pub fn runtime_failure(&self, scope: Scope) -> Option<RemoteError> {
        self.runtime.failure.with(|failure| {
            failure
                .as_ref()
                .filter(|failure| failure.scope == scope)
                .map(|failure| failure.error.clone())
        })
    }

    /// Hides the failure banner.
    pub fn dismiss_runtime_failure(&self) {
        self.runtime.failure.set(None);
    }

    /// Starts a runtime tab with empty forms, no banner and no revealed secret: what an earlier view left in them
    /// belongs to another network or tab. The work runs in a task, so building the tab never writes a signal.
    pub fn begin_runtime_view(&self) {
        let this = *self;
        self.go(async move {
            batch(|| {
                this.runtime.fields.reset();
                this.runtime.failure.set(None);
                this.runtime.revealed.set(None);
                this.runtime.revealing.set(false);
            });
        });
    }

    /// Forgets what was read from `id` (its session ended or it was removed).
    pub(super) fn clear_runtime(&self, id: TargetId) {
        fn drop_of<T: Clone + 'static>(map: FeedMap<T>, id: TargetId) {
            map.update(|all| all.retain(|scope, _| scope.target != id));
        }
        batch(|| {
            drop_of(self.runtime.connectors, id);
            drop_of(self.runtime.mapped_listeners, id);
            drop_of(self.runtime.port_forwards, id);
            drop_of(self.runtime.running, id);
            drop_of(self.runtime.vpn, id);
            drop_of(self.runtime.whitelist, id);
            drop_of(self.runtime.acl, id);
            drop_of(self.runtime.log_level, id);
            self.runtime
                .applying
                .update(|set| set.retain(|scope| scope.target != id));
            self.runtime.failure.set(None);
            self.runtime.revealed.set(None);
            self.runtime.revealing.set(false);
        });
    }

    /// Ends every runtime read of `id` that is shown as loading.
    pub(super) fn stop_runtime_loading(&self, id: TargetId) {
        batch(|| {
            stop(self.runtime.connectors, id);
            stop(self.runtime.mapped_listeners, id);
            stop(self.runtime.port_forwards, id);
            stop(self.runtime.running, id);
            stop(self.runtime.vpn, id);
            stop(self.runtime.whitelist, id);
            stop(self.runtime.acl, id);
            stop(self.runtime.log_level, id);
            self.runtime
                .applying
                .update(|set| set.retain(|scope| scope.target != id));
            self.runtime.revealing.set(false);
        });
    }

    // --- Changes -----------------------------------------------------------------------------------------------

    /// Sends the patch `build` makes to `scope`'s running network and reads the list it touched afterwards. A
    /// value the builder refuses shows on `form` (or as the tab's failure when there is no form) and nothing is
    /// sent.
    fn apply(
        &self,
        scope: Scope,
        form: Option<Form>,
        change: Change,
        build: impl FnOnce(&mut ConfigPatch) -> Result<(), RemoteError>,
    ) {
        let mut patch = ConfigPatch::new();
        if let Err(error) = build(&mut patch) {
            match form {
                Some(form) => self.runtime.fields.errors.update(|errors| {
                    errors.insert(form, error);
                }),
                None => self
                    .runtime
                    .failure
                    .set(Some(RuntimeFailure { scope, error })),
            }
            return;
        }
        let Some(client) = self.client(scope.target) else {
            return;
        };
        if self
            .runtime
            .applying
            .with_untracked(|applying| applying.contains(&scope))
        {
            return;
        }
        batch(|| {
            self.runtime.failure.set(None);
            if let Some(form) = form {
                self.runtime.fields.errors.update(|errors| {
                    errors.remove(&form);
                });
            }
            self.runtime.applying.update(|applying| {
                applying.insert(scope);
            });
        });
        let epoch = self.epoch(scope.target);
        let this = *self;
        self.go(async move {
            let result = client
                .patch_config(scope.machine, scope.instance, &patch)
                .await;
            this.runtime.applying.update(|applying| {
                applying.remove(&scope);
            });
            if !this.current_epoch(scope.target, epoch) {
                return;
            }
            match result {
                Ok(()) => {
                    if let Some(form) = form {
                        this.runtime.fields.clear(form);
                    }
                    this.refresh_change(scope, change);
                }
                Err(error) => {
                    this.react_to(scope.target, &error);
                    this.runtime
                        .failure
                        .set(Some(RuntimeFailure { scope, error }));
                }
            }
        });
    }

    /// Adds the connector the form holds.
    pub fn add_connector(&self, scope: Scope) {
        let url = self.runtime.fields.connector.get_untracked();
        self.apply(
            scope,
            Some(Form::Connector),
            Change::Connectors,
            move |patch| patch.add_connector(&url).map(|_| ()),
        );
    }

    /// Removes the connector `url`.
    pub fn remove_connector(&self, scope: Scope, url: String) {
        self.apply(scope, None, Change::Connectors, move |patch| {
            patch.remove_connector(&url).map(|_| ())
        });
    }

    /// Removes every connector.
    pub fn clear_connectors(&self, scope: Scope) {
        self.apply(scope, None, Change::Connectors, |patch| {
            patch.clear_connectors();
            Ok(())
        });
    }

    /// Adds the mapped listener the form holds.
    pub fn add_mapped_listener(&self, scope: Scope) {
        let url = self.runtime.fields.mapped_listener.get_untracked();
        self.apply(
            scope,
            Some(Form::MappedListener),
            Change::MappedListeners,
            move |patch| patch.add_mapped_listener(&url).map(|_| ()),
        );
    }

    /// Removes the mapped listener `url`.
    pub fn remove_mapped_listener(&self, scope: Scope, url: String) {
        self.apply(scope, None, Change::MappedListeners, move |patch| {
            patch.remove_mapped_listener(&url).map(|_| ())
        });
    }

    /// Removes every mapped listener.
    pub fn clear_mapped_listeners(&self, scope: Scope) {
        self.apply(scope, None, Change::MappedListeners, |patch| {
            patch.clear_mapped_listeners();
            Ok(())
        });
    }

    /// Adds the port forward the form holds.
    pub fn add_port_forward(&self, scope: Scope) {
        let fields = self.runtime.fields;
        let protocol = fields.forward_protocol.get_untracked();
        let bind = fields.forward_bind.get_untracked();
        let dst = fields.forward_dst.get_untracked();
        self.apply(
            scope,
            Some(Form::PortForward),
            Change::PortForwards,
            move |patch| patch.add_port_forward(protocol, &bind, &dst).map(|_| ()),
        );
    }

    /// Removes the port forward `entry`.
    pub fn remove_port_forward(&self, scope: Scope, entry: PortForwardEntry) {
        self.apply(scope, None, Change::PortForwards, move |patch| {
            patch
                .remove_port_forward(
                    entry.protocol,
                    &entry.bind.to_string(),
                    &entry.dst.to_string(),
                )
                .map(|_| ())
        });
    }

    /// Adds the subnet proxy the form holds.
    pub fn add_subnet_proxy(&self, scope: Scope) {
        let fields = self.runtime.fields;
        let cidr = fields.subnet.get_untracked();
        let mapped = fields.subnet_mapped.get_untracked();
        self.apply(
            scope,
            Some(Form::SubnetProxy),
            Change::Running,
            move |patch| {
                let mapped = Some(mapped.trim()).filter(|text| !text.is_empty());
                patch.add_proxy_network(&cidr, mapped).map(|_| ())
            },
        );
    }

    /// Stops proxying `cidr`.
    pub fn remove_subnet_proxy(&self, scope: Scope, cidr: String) {
        self.apply(scope, None, Change::Running, move |patch| {
            patch.remove_proxy_network(&cidr).map(|_| ())
        });
    }

    /// Adds the manual route the form holds.
    pub fn add_route(&self, scope: Scope) {
        let cidr = self.runtime.fields.route.get_untracked();
        self.apply(scope, Some(Form::Route), Change::Running, move |patch| {
            patch.add_route(&cidr).map(|_| ())
        });
    }

    /// Removes the manual route `cidr`.
    pub fn remove_route(&self, scope: Scope, cidr: String) {
        self.apply(scope, None, Change::Running, move |patch| {
            patch.remove_route(&cidr).map(|_| ())
        });
    }

    /// Adds the exit node the form holds.
    pub fn add_exit_node(&self, scope: Scope) {
        let ip = self.runtime.fields.exit_node.get_untracked();
        self.apply(scope, Some(Form::ExitNode), Change::Running, move |patch| {
            patch.add_exit_node(&ip).map(|_| ())
        });
    }

    /// Removes the exit node `ip`.
    pub fn remove_exit_node(&self, scope: Scope, ip: String) {
        self.apply(scope, None, Change::Running, move |patch| {
            patch.remove_exit_node(&ip).map(|_| ())
        });
    }

    /// Sets the hostname the form holds.
    pub fn set_hostname(&self, scope: Scope) {
        let name = self.runtime.fields.hostname.get_untracked();
        self.apply(scope, Some(Form::Hostname), Change::Running, move |patch| {
            patch.hostname(&name).map(|_| ())
        });
    }

    /// Sets the virtual IPv4 the form holds.
    pub fn set_virtual_ip(&self, scope: Scope) {
        let cidr = self.runtime.fields.virtual_ip.get_untracked();
        self.apply(
            scope,
            Some(Form::VirtualIp),
            Change::Running,
            move |patch| patch.virtual_ipv4(&cidr).map(|_| ()),
        );
    }

    /// Turns the refusal to relay other nodes' data on or off.
    pub fn set_relay_data_disabled(&self, scope: Scope, disabled: bool) {
        self.apply(scope, None, Change::Running, move |patch| {
            patch.disable_relay_data(disabled);
            Ok(())
        });
    }

    /// Opens the whitelist entry the form holds.
    pub fn add_whitelist_port(&self, scope: Scope) {
        let fields = self.runtime.fields;
        let protocol = fields.whitelist_protocol.get_untracked();
        let port = fields.whitelist_port.get_untracked();
        self.apply(
            scope,
            Some(Form::Whitelist),
            Change::Whitelist,
            move |patch| patch.add_whitelist_port(protocol, &port).map(|_| ()),
        );
    }

    /// Closes the whitelist entry `port` of `protocol`.
    pub fn remove_whitelist_port(&self, scope: Scope, protocol: TransportProtocol, port: String) {
        self.apply(scope, None, Change::Whitelist, move |patch| {
            patch.remove_whitelist_port(protocol, &port).map(|_| ())
        });
    }

    /// Closes every whitelist entry of `protocol`.
    pub fn clear_whitelist(&self, scope: Scope, protocol: TransportProtocol) {
        self.apply(scope, None, Change::Whitelist, move |patch| {
            patch.clear_whitelist(protocol);
            Ok(())
        });
    }

    // --- The core's log level ----------------------------------------------------------------------------------

    /// Sets the log level of the whole core on `machine`, then reads it back. `scope` is
    /// [`Scope::machine`]'s.
    pub fn set_log_level(&self, scope: Scope, level: LogLevel) {
        let Some(client) = self.client(scope.target) else {
            return;
        };
        if self
            .runtime
            .applying
            .with_untracked(|applying| applying.contains(&scope))
        {
            return;
        }
        batch(|| {
            self.runtime.failure.set(None);
            self.runtime.applying.update(|applying| {
                applying.insert(scope);
            });
        });
        let epoch = self.epoch(scope.target);
        let this = *self;
        self.go(async move {
            let result = client.set_logger_level(scope.machine, level).await;
            this.runtime.applying.update(|applying| {
                applying.remove(&scope);
            });
            if !this.current_epoch(scope.target, epoch) {
                return;
            }
            match result {
                Ok(()) => this.refresh_log_level(scope),
                Err(error) => {
                    this.react_to(scope.target, &error);
                    this.runtime
                        .failure
                        .set(Some(RuntimeFailure { scope, error }));
                }
            }
        });
    }

    // --- The VPN portal's client config ------------------------------------------------------------------------

    /// Reads the VPN portal's client config for display. It holds the client's WireGuard private key: this is the
    /// only way it reaches the screen, and [`Self::hide_vpn_config`] forgets it.
    pub fn reveal_vpn_config(&self, scope: Scope) {
        let Some(client) = self.client(scope.target) else {
            return;
        };
        if self.runtime.revealing.get_untracked() {
            return;
        }
        batch(|| {
            self.runtime.failure.set(None);
            self.runtime.revealing.set(true);
        });
        let epoch = self.epoch(scope.target);
        let this = *self;
        self.go(async move {
            let result = client.vpn_portal_info(scope.machine, scope.instance).await;
            this.runtime.revealing.set(false);
            if !this.current_epoch(scope.target, epoch) {
                return;
            }
            match result {
                Ok(info) => this.runtime.revealed.set(Some(Revealed {
                    scope,
                    config: info.client_config.clone(),
                })),
                Err(error) => {
                    this.react_to(scope.target, &error);
                    this.runtime
                        .failure
                        .set(Some(RuntimeFailure { scope, error }));
                }
            }
        });
    }

    /// Forgets the revealed client config.
    pub fn hide_vpn_config(&self) {
        self.runtime.revealed.set(None);
    }

    /// Whether the client config is being read (tracked).
    pub fn is_revealing_vpn_config(&self) -> bool {
        self.runtime.revealing.get()
    }

    /// The client config of `scope`'s VPN portal while it is revealed (tracked). It holds a private key.
    pub fn revealed_vpn_config(&self, scope: Scope) -> Option<Secret> {
        self.runtime.revealed.with(|revealed| {
            revealed
                .as_ref()
                .filter(|revealed| revealed.scope == scope)
                .map(|revealed| revealed.config.clone())
        })
    }
}
