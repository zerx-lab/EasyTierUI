//! The network editor: creating a network on a machine, editing a saved one, and importing a TOML config into a
//! machine. The form is checked here (`core::net` parses every URL and subnet), then the machine's core checks the
//! whole config (`validate_config`) before anything is started or saved.

use nsh_core::prelude::*;
use uuid::Uuid;

use super::ConsoleStore;
use crate::core::{
    acl::{AclConfig, AclError},
    net::{ListenerUrl, PeerUrl, parse_ip_addr, parse_ipv4_inet, parse_ipv4_subnet, parse_port},
    remote::{AclState, Encryption, NetworkConfigForm, Networking, RemoteError, new_instance_id},
    secret::Secret,
    settings::{DEFAULT_VPN_PORTAL_PORT, MappedListener, NodeFlags, PortForward, VpnPortal},
    target::TargetId,
};

/// The encryption choices the editor offers, as option ids: the core's default, three algorithms, `xor`
/// (obfuscation, not encryption) and off.
pub const ENCRYPTION_IDS: [&str; 6] = [
    "default",
    "aes-gcm",
    "aes-256-gcm",
    "chacha20",
    "xor",
    "off",
];

/// How a node finds its peers, as the editor offers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkingChoice {
    /// Through one shared node.
    PublicServer,
    /// Through the peer URLs listed.
    Manual,
    /// Alone: others connect to its listeners.
    Standalone,
}

impl NetworkingChoice {
    /// A stable id for option lists.
    pub fn id(self) -> &'static str {
        match self {
            Self::PublicServer => "public",
            Self::Manual => "manual",
            Self::Standalone => "standalone",
        }
    }

    /// The choice with option id `id`.
    pub fn from_id(id: &str) -> Option<Self> {
        [Self::PublicServer, Self::Manual, Self::Standalone]
            .into_iter()
            .find(|choice| choice.id() == id)
    }
}

/// What the editor is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    /// A new network on a machine.
    Create,
    /// A saved network's config.
    Edit {
        /// The network instance.
        instance: Uuid,
    },
    /// A TOML config read into a machine's new network.
    Import,
}

/// What is wrong with a form, before anything is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormProblem {
    /// No machine is chosen.
    Machine,
    /// The network name is empty.
    Name,
    /// The network secret is empty.
    Secret,
    /// The static virtual IP is not `a.b.c.d/len`.
    StaticIp,
    /// The shared node's address is not a peer URL.
    PublicUrl,
    /// A peer URL (the line) is not valid.
    Peer(String),
    /// A listener URL (the line) is not valid.
    Listener(String),
    /// A proxied subnet (the line) is not valid.
    Proxy(String),
    /// An exit node (the line) is not an IP address.
    ExitNode(String),
    /// The SOCKS5 port is not a port.
    Socks5Port,
    /// The MTU is not a number the core accepts (400 to 1500).
    Mtu,
    /// The interface name is too long or has characters an interface name cannot.
    DevName,
    /// The receive limit is not a positive number of bytes per second.
    RecvLimit,
    /// A mapped listener (the line) is not a valid address.
    MappedListener(String),
    /// A manual route (the line) is not a valid subnet.
    Route(String),
    /// A whitelist pattern (the line) has a space in it.
    Whitelist(String),
    /// The VPN portal's client subnet is not a valid subnet.
    VpnCidr,
    /// The VPN portal's port is not a port.
    VpnPort,
    /// A port forward (the line) is not `tcp|udp bind-address target-address`.
    Forward(String),
    /// The ACL has a problem; the ACL editor shows which.
    Acl(AclError),
    /// There is no TOML to read.
    NothingToImport,
}

/// Why the editor could not finish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorError {
    /// The form is not valid.
    Problem(FormProblem),
    /// The server or the machine's core refused, or could not be asked.
    Remote(RemoteError),
    /// This device has no source of randomness to generate a secret from (a browser).
    NoRandomSource,
}

/// The editor's state.
#[derive(Debug, Clone, PartialEq)]
pub struct Editor {
    /// The dialog shows.
    pub open: bool,
    /// The server.
    pub target: TargetId,
    /// The machine the network is on (chosen in the dialog when importing).
    pub machine: Option<Uuid>,
    /// What it is for.
    pub mode: EditorMode,
    /// A check or save is in flight.
    pub busy: bool,
    /// The saved config is being read.
    pub loading: bool,
    /// Importing: the TOML was read and the form shows what it will do.
    pub reviewing: bool,
    /// Why the last step failed.
    pub error: Option<EditorError>,
}

impl Editor {
    pub(super) fn closed() -> Self {
        Self {
            open: false,
            target: TargetId::PRIMARY,
            machine: None,
            mode: EditorMode::Create,
            busy: false,
            loading: false,
            reviewing: false,
            error: None,
        }
    }
}

/// The editor's form fields (one editor at a time).
#[derive(Clone, Copy)]
pub struct EditorFields {
    /// The network's name.
    pub name: Signal<String>,
    /// The network secret as typed.
    pub secret: Signal<String>,
    /// The hostname peers see; empty for the machine's own.
    pub hostname: Signal<String>,
    /// The static virtual IP with its prefix.
    pub static_ip: Signal<String>,
    /// The shared node's address.
    pub public_url: Signal<String>,
    /// Peer URLs, one per line.
    pub peers: Signal<String>,
    /// Listener URLs, one per line.
    pub listeners: Signal<String>,
    /// Proxied subnets, one per line.
    pub proxy_cidrs: Signal<String>,
    /// The SOCKS5 portal's port.
    pub socks5_port: Signal<String>,
    /// A TOML config to import.
    pub toml: Signal<String>,
    /// The address comes from DHCP.
    pub dhcp: Signal<bool>,
    /// No TUN device.
    pub no_tun: Signal<bool>,
    /// This node is an exit node.
    pub exit_node: Signal<bool>,
    /// Exit nodes this node sends its traffic through (virtual IPs), one per line.
    pub exit_nodes: Signal<String>,
    /// A SOCKS5 portal runs.
    pub socks5: Signal<bool>,
    /// The behavior switches (latency first, disable P2P, KCP, hole punching, ...).
    pub flags: Signal<NodeFlags>,
    /// This node relays RPC for every peer.
    pub relay_all_rpc: Signal<bool>,
    /// Only peers that know the network secret may connect.
    pub private_mode: Signal<bool>,
    /// The MTU as typed; empty for the core's default.
    pub mtu: Signal<String>,
    /// The TUN interface name; empty lets the system choose.
    pub dev_name: Signal<String>,
    /// The limit on received traffic in bytes per second, as typed; empty for none.
    pub recv_limit: Signal<String>,
    /// Public addresses of the listeners (mapped listeners), one per line.
    pub mapped_listeners: Signal<String>,
    /// Manual routes replace the discovered ones.
    pub routes_on: Signal<bool>,
    /// The manual routes (CIDRs), one per line.
    pub routes: Signal<String>,
    /// Only the listed networks are relayed for.
    pub whitelist_on: Signal<bool>,
    /// The relay network whitelist, one pattern per line.
    pub whitelist: Signal<String>,
    /// The WireGuard VPN portal runs.
    pub vpn_on: Signal<bool>,
    /// The addresses handed to WireGuard clients (`10.14.14.0/24`).
    pub vpn_cidr: Signal<String>,
    /// The VPN portal's UDP port as typed.
    pub vpn_port: Signal<String>,
    /// Port forwards, one per line: `tcp 127.0.0.1:8080 10.144.144.2:80`.
    pub forwards: Signal<String>,
    /// The ACL; `None` for no ACL. Holds group secrets: wiped with the form.
    pub acl: Signal<Option<AclConfig>>,
    /// The server's ACL cannot be read by this app: it is kept as it is on save and `acl` is ignored.
    pub acl_unreadable: Signal<bool>,
    /// How peers are found.
    pub networking: Signal<NetworkingChoice>,
    /// The encryption, as one of [`ENCRYPTION_IDS`] (or an algorithm name a newer server uses).
    pub encryption: Signal<String>,
    /// The config an edit or an import starts from; its unknown fields survive the save.
    base: Stored<Option<NetworkConfigForm>>,
}

impl EditorFields {
    pub(super) fn new() -> Self {
        Self {
            name: signal(String::new()),
            secret: signal(String::new()),
            hostname: signal(String::new()),
            static_ip: signal(String::new()),
            public_url: signal(String::new()),
            peers: signal(String::new()),
            listeners: signal(String::new()),
            proxy_cidrs: signal(String::new()),
            socks5_port: signal(String::new()),
            toml: signal(String::new()),
            dhcp: signal(true),
            no_tun: signal(false),
            exit_node: signal(false),
            exit_nodes: signal(String::new()),
            socks5: signal(false),
            flags: signal(NodeFlags::default()),
            relay_all_rpc: signal(false),
            private_mode: signal(true),
            mtu: signal(String::new()),
            dev_name: signal(String::new()),
            recv_limit: signal(String::new()),
            mapped_listeners: signal(String::new()),
            routes_on: signal(false),
            routes: signal(String::new()),
            whitelist_on: signal(false),
            whitelist: signal(String::new()),
            vpn_on: signal(false),
            vpn_cidr: signal(String::new()),
            vpn_port: signal(default_vpn_port()),
            forwards: signal(String::new()),
            acl: signal(None),
            acl_unreadable: signal(false),
            networking: signal(NetworkingChoice::Manual),
            encryption: signal("aes-gcm".to_owned()),
            base: stored(None),
        }
    }

    /// The defaults of a new network: DHCP, manual peers (none yet), a TUN, nothing exposed, `aes-gcm`.
    fn reset(&self) {
        batch(|| {
            self.name.set(String::new());
            self.secret.set(String::new());
            self.hostname.set(String::new());
            self.static_ip.set(String::new());
            self.public_url.set(String::new());
            self.peers.set(String::new());
            self.listeners.set(String::new());
            self.proxy_cidrs.set(String::new());
            self.socks5_port.set(String::new());
            self.toml.set(String::new());
            self.dhcp.set(true);
            self.no_tun.set(false);
            self.exit_node.set(false);
            self.exit_nodes.set(String::new());
            self.socks5.set(false);
            self.flags.set(NodeFlags::default());
            self.relay_all_rpc.set(false);
            self.private_mode.set(true);
            self.mtu.set(String::new());
            self.dev_name.set(String::new());
            self.recv_limit.set(String::new());
            self.mapped_listeners.set(String::new());
            self.routes_on.set(false);
            self.routes.set(String::new());
            self.whitelist_on.set(false);
            self.whitelist.set(String::new());
            self.vpn_on.set(false);
            self.vpn_cidr.set(String::new());
            self.vpn_port.set(default_vpn_port());
            self.forwards.set(String::new());
            self.acl.set(None);
            self.acl_unreadable.set(false);
            self.networking.set(NetworkingChoice::Manual);
            self.encryption.set("aes-gcm".to_owned());
            self.base.replace(None);
        });
    }

    /// Fills the fields from `form`.
    fn load(&self, form: &NetworkConfigForm) {
        batch(|| {
            self.name.set(form.network_name.clone());
            self.secret.set(form.network_secret.expose().to_owned());
            self.hostname.set(form.hostname.clone().unwrap_or_default());
            self.dhcp.set(form.dhcp);
            self.static_ip.set(
                form.virtual_ipv4
                    .map(|net| net.to_string())
                    .unwrap_or_default(),
            );
            match &form.networking {
                Networking::PublicServer { url } => {
                    self.networking.set(NetworkingChoice::PublicServer);
                    self.public_url.set(url.clone());
                }
                Networking::Manual => {
                    self.networking.set(NetworkingChoice::Manual);
                    self.public_url.set(String::new());
                }
                Networking::Standalone => {
                    self.networking.set(NetworkingChoice::Standalone);
                    self.public_url.set(String::new());
                }
            }
            self.peers.set(form.peers.join("\n"));
            self.listeners.set(form.listeners.join("\n"));
            self.proxy_cidrs.set(form.proxy_cidrs.join("\n"));
            self.no_tun.set(form.no_tun);
            self.exit_node.set(form.enable_exit_node);
            self.exit_nodes.set(form.exit_nodes.join("\n"));
            self.socks5.set(form.socks5_port.is_some());
            self.socks5_port.set(
                form.socks5_port
                    .map(|port| port.to_string())
                    .unwrap_or_default(),
            );
            self.flags.set(form.flags);
            self.relay_all_rpc.set(form.relay_all_peer_rpc);
            self.private_mode.set(form.private_mode);
            self.mtu
                .set(form.mtu.map(|mtu| mtu.to_string()).unwrap_or_default());
            self.dev_name.set(form.dev_name.clone().unwrap_or_default());
            self.recv_limit.set(
                form.instance_recv_bps_limit
                    .map(|limit| limit.to_string())
                    .unwrap_or_default(),
            );
            self.mapped_listeners.set(form.mapped_listeners.join("\n"));
            self.routes_on.set(form.manual_routes.is_some());
            self.routes
                .set(form.manual_routes.clone().unwrap_or_default().join("\n"));
            self.whitelist_on.set(form.relay_whitelist.is_some());
            self.whitelist
                .set(form.relay_whitelist.clone().unwrap_or_default().join("\n"));
            self.vpn_on.set(form.vpn_portal.is_some());
            match form.vpn_portal {
                Some(portal) => {
                    self.vpn_cidr.set(portal.client_cidr.to_string());
                    self.vpn_port.set(portal.listen_port.to_string());
                }
                None => {
                    self.vpn_cidr.set(String::new());
                    self.vpn_port.set(DEFAULT_VPN_PORTAL_PORT.to_string());
                }
            }
            self.forwards.set(
                form.port_forwards
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
            match &form.acl {
                AclState::Off => {
                    self.acl.set(None);
                    self.acl_unreadable.set(false);
                }
                AclState::On(config) => {
                    self.acl.set(Some(config.clone()));
                    self.acl_unreadable.set(false);
                }
                AclState::Unreadable => {
                    self.acl.set(None);
                    self.acl_unreadable.set(true);
                }
            }
            self.encryption.set(match &form.encryption {
                Encryption::CoreDefault => "default".to_owned(),
                Encryption::Algorithm(name) => name.clone(),
                Encryption::Disabled => "off".to_owned(),
            });
            self.base.replace(Some(form.clone()));
        });
    }

    /// Wipes the secrets (the network's and the ACL's groups) and the imported text.
    fn wipe(&self) {
        batch(|| {
            self.secret.set(String::new());
            self.toml.set(String::new());
            self.acl.set(None);
            self.acl_unreadable.set(false);
            self.base.replace(None);
        });
    }
}

/// The non-empty trimmed lines of `text`.
fn lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `line` as a subnet or `subnet->mapped subnet`, or `None`.
fn proxy_line(line: &str) -> Option<String> {
    let mut parts = line.splitn(2, "->");
    let real = parts.next()?.trim();
    parse_ipv4_subnet("proxy", real).ok()?;
    match parts.next() {
        None => Some(real.to_owned()),
        Some(mapped) => {
            let mapped = mapped.trim();
            parse_ipv4_subnet("proxy", mapped).ok()?;
            Some(format!("{real}->{mapped}"))
        }
    }
}

/// The valid MTU values of the editor: the official GUI's lower bound up to Ethernet's.
const MTU_RANGE: std::ops::RangeInclusive<u16> = 400..=1500;
/// The longest TUN interface name (the Linux limit).
const MAX_DEV_NAME_CHARS: usize = 15;

/// Whether `name` is usable as an interface name: short, and letters, digits, `-`, `_` and `.` only.
fn dev_name_ok(name: &str) -> bool {
    name.chars().count() <= MAX_DEV_NAME_CHARS
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// The VPN portal's default port as the field's text.
fn default_vpn_port() -> String {
    DEFAULT_VPN_PORTAL_PORT.to_string()
}

impl ConsoleStore {
    /// Whether this device can generate a secret (a browser has no source of randomness).
    pub fn can_generate_secret(&self) -> bool {
        self.can_generate
    }

    /// Whether a listener of the editor binds every interface (shown as an exposure).
    pub fn listeners_expose_all(&self) -> bool {
        lines(&self.fields.listeners.get()).iter().any(|line| {
            ListenerUrl::parse_config("listeners", line)
                .is_ok_and(|url| url.exposes_all_interfaces())
        })
    }

    /// Whether the editor turns the WireGuard VPN portal on, which listens on every interface (shown as an
    /// exposure).
    pub fn vpn_portal_on(&self) -> bool {
        self.fields.vpn_on.get()
    }

    /// Whether a port forward of the editor listens beyond this machine (shown as an exposure).
    pub fn forwards_public(&self) -> bool {
        lines(&self.fields.forwards.get()).iter().any(|line| {
            PortForward::parse_line("forward", line).is_ok_and(|forward| forward.is_public())
        })
    }

    /// Whether the server's port forwards include rows this app cannot read, which stay as they are until the
    /// forward list is edited.
    pub fn forwards_unreadable(&self) -> bool {
        self.fields.base.with(|base| {
            base.as_ref()
                .is_some_and(NetworkConfigForm::forwards_unreadable)
        })
    }

    /// Opens the editor for a new network on `machine`.
    pub fn open_create(&self, id: TargetId, machine: Uuid) {
        self.fields.reset();
        self.editor.set(Editor {
            open: true,
            target: id,
            machine: Some(machine),
            mode: EditorMode::Create,
            busy: false,
            loading: false,
            reviewing: false,
            error: None,
        });
    }

    /// Opens the editor on the saved config of network `instance` of `machine`.
    pub fn open_edit(&self, id: TargetId, machine: Uuid, instance: Uuid) {
        self.fields.reset();
        self.editor.set(Editor {
            open: true,
            target: id,
            machine: Some(machine),
            mode: EditorMode::Edit { instance },
            busy: false,
            loading: true,
            reviewing: false,
            error: None,
        });
        self.load_config(id, machine, instance);
    }

    /// Reads the config the editor edits (also the retry of a failed read).
    pub fn load_config(&self, id: TargetId, machine: Uuid, instance: Uuid) {
        let Some(client) = self.client(id) else {
            return;
        };
        self.editor.update(|editor| {
            editor.loading = true;
            editor.error = None;
        });
        let epoch = self.epoch(id);
        let this = *self;
        self.go(async move {
            let result = client.network_config(machine, instance).await;
            if !this.current_epoch(id, epoch) || !this.editor_is(id, EditorMode::Edit { instance })
            {
                return;
            }
            match result {
                Ok(form) => {
                    this.fields.load(&form);
                    this.editor.update(|editor| {
                        editor.loading = false;
                        editor.reviewing = true;
                    });
                }
                Err(error) => {
                    this.editor.update(|editor| {
                        editor.loading = false;
                        editor.error = Some(EditorError::Remote(error.clone()));
                    });
                    this.react_to(id, &error);
                }
            }
        });
    }

    /// Opens the editor to import a TOML config into one of `id`'s machines.
    pub fn open_import(&self, id: TargetId) {
        self.fields.reset();
        let first = self.machines.with_untracked(|all| {
            all.get(&id)
                .and_then(|feed| feed.value.as_ref())
                .and_then(|machines| machines.first().map(|machine| machine.id))
        });
        self.editor.set(Editor {
            open: true,
            target: id,
            machine: first,
            mode: EditorMode::Import,
            busy: false,
            loading: false,
            reviewing: false,
            error: None,
        });
    }

    /// Whether the open editor is for `id` in `mode`.
    fn editor_is(&self, id: TargetId, mode: EditorMode) -> bool {
        self.editor
            .with_untracked(|editor| editor.open && editor.target == id && editor.mode == mode)
    }

    /// Closes the editor and wipes the secret and the imported text.
    pub fn close_editor(&self) {
        batch(|| {
            self.editor.update(|editor| {
                editor.open = false;
                editor.busy = false;
                editor.loading = false;
            });
            self.fields.wipe();
        });
    }

    /// Chooses the machine an import goes to.
    pub fn set_editor_machine(&self, machine: Uuid) {
        self.editor.update(|editor| editor.machine = Some(machine));
    }

    /// Fills the secret field with a new random secret.
    pub fn generate_secret(&self) {
        match Secret::generate() {
            Ok(secret) => {
                self.fields.secret.set(secret.expose().to_owned());
                self.editor.update(|editor| {
                    if editor.error == Some(EditorError::NoRandomSource) {
                        editor.error = None;
                    }
                });
            }
            Err(error) => {
                log::warn!("a network secret could not be generated: {error}");
                self.editor
                    .update(|editor| editor.error = Some(EditorError::NoRandomSource));
            }
        }
    }

    /// Importing: asks the server to read the pasted TOML, and shows the result in the form for review.
    pub fn read_import(&self) {
        let editor = self.editor.get_untracked();
        if !editor.open || editor.busy || editor.mode != EditorMode::Import {
            return;
        }
        let text = Secret::new(self.fields.toml.get_untracked());
        if text.expose().trim().is_empty() {
            self.set_editor_error(EditorError::Problem(FormProblem::NothingToImport));
            return;
        }
        if editor.machine.is_none() {
            self.set_editor_error(EditorError::Problem(FormProblem::Machine));
            return;
        }
        let id = editor.target;
        let Some(client) = self.client(id) else {
            return;
        };
        self.editor.update(|editor| {
            editor.busy = true;
            editor.error = None;
        });
        let epoch = self.epoch(id);
        let this = *self;
        self.go(async move {
            let result = client.parse_config(text.expose()).await;
            if !this.current_epoch(id, epoch) || !this.editor_is(id, EditorMode::Import) {
                return;
            }
            match result {
                Ok(form) => {
                    this.fields.load(&form);
                    this.editor.update(|editor| {
                        editor.busy = false;
                        editor.reviewing = true;
                    });
                }
                Err(error) => {
                    this.editor.update(|editor| {
                        editor.busy = false;
                        editor.error = Some(EditorError::Remote(error.clone()));
                    });
                    this.react_to(id, &error);
                }
            }
        });
    }

    /// Importing: back from the review to the pasted text.
    pub fn back_to_import(&self) {
        self.editor.update(|editor| {
            editor.reviewing = false;
            editor.error = None;
        });
    }

    fn set_editor_error(&self, error: EditorError) {
        self.editor.update(|editor| editor.error = Some(error));
    }

    /// The form as the fields describe it.
    fn build_form(&self) -> Result<NetworkConfigForm, FormProblem> {
        let fields = &self.fields;
        let mut form = fields
            .base
            .get()
            .unwrap_or_else(|| NetworkConfigForm::new(new_instance_id(), "", Secret::default()));
        let credential_mode = fields.base.with(|base| {
            base.as_ref()
                .is_some_and(|base| base.network_secret.is_empty())
        });
        let name = fields.name.get_untracked().trim().to_owned();
        if name.is_empty() {
            return Err(FormProblem::Name);
        }
        let secret = fields.secret.get_untracked();
        if secret.is_empty() && !credential_mode {
            return Err(FormProblem::Secret);
        }
        form.network_name = name;
        form.network_secret = Secret::new(secret);
        let hostname = fields.hostname.get_untracked().trim().to_owned();
        form.hostname = (!hostname.is_empty()).then_some(hostname);
        form.dhcp = fields.dhcp.get_untracked();
        form.virtual_ipv4 = if form.dhcp {
            None
        } else {
            let net = parse_ipv4_inet("virtual IP", fields.static_ip.get_untracked().trim())
                .map_err(|_| FormProblem::StaticIp)?;
            Some(net)
        };
        let (networking, peers) = match fields.networking.get_untracked() {
            NetworkingChoice::PublicServer => {
                let url = fields.public_url.get_untracked().trim().to_owned();
                PeerUrl::parse("shared node", &url).map_err(|_| FormProblem::PublicUrl)?;
                (Networking::PublicServer { url }, Vec::new())
            }
            NetworkingChoice::Manual => {
                let mut peers = Vec::new();
                for line in lines(&fields.peers.get_untracked()) {
                    let url = PeerUrl::parse("peer", &line)
                        .map_err(|_| FormProblem::Peer(line.clone()))?;
                    peers.push(url.as_str().to_owned());
                }
                (Networking::Manual, peers)
            }
            NetworkingChoice::Standalone => (Networking::Standalone, Vec::new()),
        };
        form.networking = networking;
        form.peers = peers;
        let mut listeners = Vec::new();
        for line in lines(&fields.listeners.get_untracked()) {
            let url = ListenerUrl::parse_config("listener", &line)
                .map_err(|_| FormProblem::Listener(line.clone()))?;
            listeners.push(url.as_str().to_owned());
        }
        form.listeners = listeners;
        let mut proxies = Vec::new();
        for line in lines(&fields.proxy_cidrs.get_untracked()) {
            proxies.push(proxy_line(&line).ok_or_else(|| FormProblem::Proxy(line.clone()))?);
        }
        form.proxy_cidrs = proxies;
        form.no_tun = fields.no_tun.get_untracked();
        form.enable_exit_node = fields.exit_node.get_untracked();
        let mut exit_nodes = Vec::new();
        for line in lines(&fields.exit_nodes.get_untracked()) {
            let address = parse_ip_addr("exit node", &line)
                .map_err(|_| FormProblem::ExitNode(line.clone()))?;
            exit_nodes.push(address.to_string());
        }
        form.exit_nodes = exit_nodes;
        form.socks5_port = if fields.socks5.get_untracked() {
            Some(
                parse_port("SOCKS5 port", &fields.socks5_port.get_untracked())
                    .map_err(|_| FormProblem::Socks5Port)?,
            )
        } else {
            None
        };
        form.flags = fields.flags.get_untracked();
        form.relay_all_peer_rpc = fields.relay_all_rpc.get_untracked();
        form.private_mode = fields.private_mode.get_untracked();
        let mtu = fields.mtu.get_untracked();
        form.mtu = match mtu.trim() {
            "" => None,
            text => Some(
                text.parse::<u16>()
                    .ok()
                    .filter(|mtu| MTU_RANGE.contains(mtu))
                    .ok_or(FormProblem::Mtu)?,
            ),
        };
        let dev_name = fields.dev_name.get_untracked().trim().to_owned();
        if !dev_name.is_empty() && !dev_name_ok(&dev_name) {
            return Err(FormProblem::DevName);
        }
        form.dev_name = (!dev_name.is_empty()).then_some(dev_name);
        let limit = fields.recv_limit.get_untracked();
        form.instance_recv_bps_limit = match limit.trim() {
            "" => None,
            text => Some(
                text.parse::<u64>()
                    .ok()
                    .filter(|limit| *limit >= 1)
                    .ok_or(FormProblem::RecvLimit)?,
            ),
        };
        let mut mapped = Vec::new();
        for line in lines(&fields.mapped_listeners.get_untracked()) {
            let url = MappedListener::parse("mapped listener", &line)
                .map_err(|_| FormProblem::MappedListener(line.clone()))?;
            mapped.push(url.as_str().to_owned());
        }
        form.mapped_listeners = mapped;
        form.manual_routes = if fields.routes_on.get_untracked() {
            let mut routes = Vec::new();
            for line in lines(&fields.routes.get_untracked()) {
                let net = parse_ipv4_subnet("route", &line)
                    .map_err(|_| FormProblem::Route(line.clone()))?;
                routes.push(net.to_string());
            }
            Some(routes)
        } else {
            None
        };
        form.relay_whitelist = if fields.whitelist_on.get_untracked() {
            let patterns = lines(&fields.whitelist.get_untracked());
            // EasyTier stores the list space-separated: a space would split one pattern into two.
            if let Some(bad) = patterns
                .iter()
                .find(|pattern| pattern.chars().any(char::is_whitespace))
            {
                return Err(FormProblem::Whitelist(bad.clone()));
            }
            Some(patterns)
        } else {
            None
        };
        form.vpn_portal = if fields.vpn_on.get_untracked() {
            let client_cidr =
                parse_ipv4_subnet("VPN client subnet", &fields.vpn_cidr.get_untracked())
                    .map_err(|_| FormProblem::VpnCidr)?;
            let listen_port = parse_port("VPN port", &fields.vpn_port.get_untracked())
                .map_err(|_| FormProblem::VpnPort)?;
            Some(VpnPortal {
                client_cidr,
                listen_port,
            })
        } else {
            None
        };
        let mut forwards = Vec::new();
        for line in lines(&fields.forwards.get_untracked()) {
            forwards.push(
                PortForward::parse_line("forward", &line)
                    .map_err(|_| FormProblem::Forward(line.clone()))?,
            );
        }
        form.port_forwards = forwards;
        form.acl = if fields.acl_unreadable.get_untracked() {
            AclState::Unreadable
        } else {
            match fields.acl.get_untracked() {
                Some(config) => {
                    config.check().map_err(FormProblem::Acl)?;
                    AclState::On(config)
                }
                None => AclState::Off,
            }
        };
        form.encryption = match fields.encryption.get_untracked().as_str() {
            "default" => Encryption::CoreDefault,
            "off" => Encryption::Disabled,
            algorithm => Encryption::Algorithm(algorithm.to_owned()),
        };
        Ok(form)
    }

    /// Checks the form, asks the machine's core to validate the whole config, then starts the network (saved on
    /// the server, so it is pushed again when the machine reconnects) or replaces the saved config.
    pub fn submit_editor(&self) {
        let editor = self.editor.get_untracked();
        if !editor.open || editor.busy || editor.loading {
            return;
        }
        let Some(machine) = editor.machine else {
            self.set_editor_error(EditorError::Problem(FormProblem::Machine));
            return;
        };
        let form = match self.build_form() {
            Ok(form) => form,
            Err(problem) => {
                self.set_editor_error(EditorError::Problem(problem));
                return;
            }
        };
        let id = editor.target;
        let Some(client) = self.client(id) else {
            return;
        };
        self.editor.update(|editor| {
            editor.busy = true;
            editor.error = None;
        });
        let epoch = self.epoch(id);
        let this = *self;
        self.go(async move {
            let checked = client.validate_config(machine, &form).await;
            let result = match (checked, editor.mode) {
                (Err(error), _) => Err(error),
                (Ok(_toml), EditorMode::Edit { instance }) => {
                    client.update_network_config(machine, instance, &form).await
                }
                (Ok(_toml), EditorMode::Create | EditorMode::Import) => {
                    client.run_network(machine, &form, true).await
                }
            };
            if !this.current_epoch(id, epoch) {
                return;
            }
            match result {
                Ok(()) => {
                    this.close_editor();
                    this.refresh_networks(id, machine);
                }
                Err(error) => {
                    this.editor.update(|editor| {
                        editor.busy = false;
                        editor.error = Some(EditorError::Remote(error.clone()));
                    });
                    this.react_to(id, &error);
                }
            }
        });
    }
}
