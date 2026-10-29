//! The network editor: creating a network on a machine, editing a saved one, and importing a TOML config into a
//! machine. The form is checked here (`core::net` parses every URL and subnet), then the machine's core checks the
//! whole config (`validate_config`) before anything is started or saved.

use nsh_core::prelude::*;
use uuid::Uuid;

use super::ConsoleStore;
use crate::core::{
    net::{ListenerUrl, PeerUrl, parse_ipv4_inet, parse_ipv4_subnet, parse_port},
    remote::{Encryption, NetworkConfigForm, Networking, RemoteError, new_instance_id},
    secret::Secret,
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
    /// The SOCKS5 port is not a port.
    Socks5Port,
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
    /// A SOCKS5 portal runs.
    pub socks5: Signal<bool>,
    /// Routes are chosen by latency first.
    pub latency_first: Signal<bool>,
    /// Direct links are off.
    pub disable_p2p: Signal<bool>,
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
            socks5: signal(false),
            latency_first: signal(false),
            disable_p2p: signal(false),
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
            self.socks5.set(false);
            self.latency_first.set(false);
            self.disable_p2p.set(false);
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
            self.socks5.set(form.socks5_port.is_some());
            self.socks5_port.set(
                form.socks5_port
                    .map(|port| port.to_string())
                    .unwrap_or_default(),
            );
            self.latency_first.set(form.latency_first);
            self.disable_p2p.set(form.disable_p2p);
            self.encryption.set(match &form.encryption {
                Encryption::CoreDefault => "default".to_owned(),
                Encryption::Algorithm(name) => name.clone(),
                Encryption::Disabled => "off".to_owned(),
            });
            self.base.replace(Some(form.clone()));
        });
    }

    /// Wipes the secret and the imported text.
    fn wipe(&self) {
        batch(|| {
            self.secret.set(String::new());
            self.toml.set(String::new());
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
        form.socks5_port = if fields.socks5.get_untracked() {
            Some(
                parse_port("SOCKS5 port", &fields.socks5_port.get_untracked())
                    .map_err(|_| FormProblem::Socks5Port)?,
            )
        } else {
            None
        };
        form.latency_first = fields.latency_first.get_untracked();
        form.disable_p2p = fields.disable_p2p.get_untracked();
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
