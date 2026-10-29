//! The Settings tab: every field of the profile as a form over the store's editor draft. Switches that widen exposure
//! carry the warning right where they are turned on. A managed network shows its fields read-only, except the name
//! and the hostname.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::super::common::{
    ask_delete, error_of, exposure_item, secret_labels, secrets_notice, tag,
};
use crate::{
    components::{
        ButtonKind, CalloutKind, Field as TextField, HintLevel, SelectOption, SwitchRow, body_text,
        busy_button, callout, field, meta_text, secret_field, segmented, select, switch_row,
    },
    core::{
        net::BASE_PORT,
        secret::{Secret, Strength},
        settings::DEFAULT_VPN_PORTAL_PORT,
    },
    i18n::{Strings, node::DetailStrings},
    pages::config::{acl_editor, flag_switches},
    profile::{DEFAULT_MTU, Encryption, Exposure, ProfileId},
    store::{
        Store,
        node::{EditorDraft, Field as ProblemField, FieldProblem, IpChoice, NodeStore},
    },
};

/// What the form's parts share: the store, the problems of the draft and whether the network is read-only.
#[derive(Clone, Copy)]
struct Form {
    store: Store,
    node: NodeStore,
    problems: Memo<Vec<FieldProblem>>,
    exposures: Memo<Vec<Exposure>>,
    managed: bool,
}

impl Form {
    /// Something of the draft (the default when there is none).
    fn read<T: Default>(self, pick: impl FnOnce(&EditorDraft) -> T) -> T {
        self.node
            .editor()
            .with(|draft| draft.as_ref().map(pick).unwrap_or_default())
    }

    /// The error line of `problem`'s field.
    fn error(self, problem: ProblemField) -> impl Fn() -> Option<SharedString> + 'static {
        move || error_of(self.store.strings(), &self.problems.get(), problem)
    }

    /// A text field over the draft; `locked` makes it read-only.
    fn input(
        self,
        label: impl IntoReactive<SharedString>,
        get: fn(&EditorDraft) -> String,
        set: fn(&mut EditorDraft, &str),
        problem: ProblemField,
        locked: bool,
        id: &'static str,
    ) -> TextField {
        let node = self.node;
        field(
            label,
            move || self.read(get),
            move |text| node.edit_editor(|draft| set(draft, text)),
        )
        .error(self.error(problem))
        .disabled(locked)
        .test_id(id)
    }

    /// A text field that a managed network shows read-only.
    fn text(
        self,
        label: impl IntoReactive<SharedString>,
        get: fn(&EditorDraft) -> String,
        set: fn(&mut EditorDraft, &str),
        problem: ProblemField,
        id: &'static str,
    ) -> TextField {
        self.input(label, get, set, problem, self.managed, id)
    }

    /// A text field that a managed network can still edit.
    fn open_text(
        self,
        label: impl IntoReactive<SharedString>,
        get: fn(&EditorDraft) -> String,
        set: fn(&mut EditorDraft, &str),
        problem: ProblemField,
        id: &'static str,
    ) -> TextField {
        self.input(label, get, set, problem, false, id)
    }

    /// A switch over a flag of the draft.
    fn toggle(
        self,
        label: impl IntoReactive<SharedString>,
        get: fn(&EditorDraft) -> bool,
        set: fn(&mut EditorDraft, bool),
        id: &'static str,
    ) -> SwitchRow {
        let node = self.node;
        switch_row(
            label,
            move || self.read(get),
            move |on| node.edit_editor(|draft| set(draft, on)),
        )
        .test_id(id)
    }

    /// The warnings of the exposures `keep` picks, one under the other (nothing while none applies).
    fn warnings(self, keep: fn(&Exposure) -> bool) -> AnyElement {
        let store = self.store;
        let exposures = self.exposures;
        dynamic(move || {
            exposures
                .get()
                .iter()
                .filter(|exposure| keep(exposure))
                .map(|exposure| exposure_item(store, exposure))
                .collect::<Vec<_>>()
        })
    }

    /// `switch`, and `body` under it while the flag `on` of the draft holds.
    fn gated(
        self,
        switch: SwitchRow,
        on: fn(&EditorDraft) -> bool,
        body: impl Fn() -> View + 'static,
    ) -> View {
        view()
            .gap_2()
            .child(switch)
            .child(when(move || self.read(on), body))
    }
}

/// The tab of network `id`.
pub(super) fn tab(store: Store, id: ProfileId, managed: bool) -> View {
    let node = store.node();
    let problems = computed(move || {
        node.editor().with(|draft| {
            draft
                .as_ref()
                .map(EditorDraft::problems)
                .unwrap_or_default()
        })
    });
    let exposures = computed(move || {
        node.editor().with(|draft| {
            draft
                .as_ref()
                .map(EditorDraft::exposures)
                .unwrap_or_default()
        })
    });
    let form = Form {
        store,
        node,
        problems,
        exposures,
        managed,
    };
    view()
        .gap_8()
        .child(managed.then(|| {
            callout(
                CalloutKind::Info,
                store.tr(|s| s.node.list.managed_read_only),
            )
            .test_id("managed-note")
        }))
        .child(general(form))
        .child(connection(form))
        .child((!managed).then(|| flags(form)))
        .child((!managed).then(|| sharing(form)))
        .child((!managed).then(|| relaying(form)))
        .child((!managed).then(|| access(form)))
        .child(remove(form, id))
}

/// A section: its heading over a hairline, then its fields 16 pt apart.
fn section(store: Store, label: fn(&'static Strings) -> &'static str) -> View {
    view().gap_4().child(
        view()
            .pb_2()
            .border_b_1()
            .border_color(move || theme().border)
            .child(
                body_text(move || label(store.strings()))
                    .font_weight(FontWeight::MEDIUM)
                    .role(AccessibilityRole::Header)
                    .text_color(move || theme().foreground),
            ),
    )
}

// ----- General -----

/// Name, network name, secret and hostname.
fn general(form: Form) -> View {
    let Form { store, node, .. } = form;
    let mut column = section(store, |s| s.node.detail.section_general)
        .child(
            form.open_text(
                store.tr(|s| s.node.shared.field_name),
                |draft| draft.name.clone(),
                |draft, text| draft.name = text.to_owned(),
                ProblemField::Name,
                "detail-name",
            )
            .helper(store.tr(|s| s.node.detail.name_help)),
        )
        .child(form.text(
            store.tr(|s| s.node.shared.field_network_name),
            |draft| draft.network_name.clone(),
            |draft, text| draft.network_name = text.to_owned(),
            ProblemField::NetworkName,
            "detail-network-name",
        ));
    if !form.managed {
        let secret = secret_field(
            store.tr(|s| s.node.shared.field_secret),
            move || form.read(|draft| draft.secret.expose().to_owned()),
            move |text| {
                node.edit_editor(|draft| {
                    draft.secret = Secret::new(text);
                    draft.secret_changed = true;
                    draft.secret_loaded = true;
                });
            },
            secret_labels(store),
        )
        .generate(store.tr(|s| s.common.generate), move || {
            node.editor_generate_secret();
        })
        .hint(
            move || {
                let common = store.strings().common;
                let (loaded, strength) = form.read(|draft| {
                    (
                        draft.secret_loaded,
                        (!draft.secret.is_empty()).then(|| draft.secret.strength()),
                    )
                });
                SharedString::from(match (loaded, strength) {
                    (false, _) => common.loading,
                    (true, None) => "",
                    (true, Some(Strength::Weak)) => common.strength_weak,
                    (true, Some(Strength::Fair)) => common.strength_fair,
                    (true, Some(Strength::Strong)) => common.strength_strong,
                })
            },
            move || match form
                .read(|draft| (!draft.secret.is_empty()).then(|| draft.secret.strength()))
            {
                Some(Strength::Strong) => HintLevel::Strong,
                Some(Strength::Fair) => HintLevel::Fair,
                Some(Strength::Weak) | None => HintLevel::Weak,
            },
        )
        .error(form.error(ProblemField::Secret))
        .test_id("detail-secret");
        column = column
            .child(secret)
            .child(when(
                move || form.read(|draft| draft.secret_missing && !draft.secret_changed),
                move || {
                    callout(
                        CalloutKind::Info,
                        store.tr(|s| s.node.detail.secret_missing),
                    )
                    .test_id("detail-secret-missing")
                },
            ))
            .child(secrets_notice(store));
    }
    column.child(
        form.open_text(
            store.tr(|s| s.node.shared.field_hostname),
            |draft| draft.hostname.clone(),
            |draft, text| draft.hostname = text.to_owned(),
            ProblemField::Hostname,
            "detail-hostname-input",
        )
        .helper(store.tr(|s| s.node.detail.hostname_help)),
    )
}

// ----- Connection -----

/// The id an address mode goes by in the segmented control.
const fn ip_id(choice: IpChoice) -> &'static str {
    match choice {
        IpChoice::Dhcp => "dhcp",
        IpChoice::Static => "static",
        IpChoice::None => "none",
    }
}

/// The address mode an id of the segmented control stands for.
fn ip_choice(id: &str) -> Option<IpChoice> {
    [IpChoice::Dhcp, IpChoice::Static, IpChoice::None]
        .into_iter()
        .find(|choice| ip_id(*choice) == id)
}

/// The id an encryption goes by in the select (the name in the config file).
fn encryption_id(encryption: Encryption) -> &'static str {
    encryption.algorithm().unwrap_or("off")
}

/// The name of an encryption choice.
fn encryption_name(words: &'static DetailStrings, encryption: Encryption) -> &'static str {
    match encryption {
        Encryption::AesGcm => words.enc_aes_gcm,
        Encryption::Aes256Gcm => words.enc_aes_256_gcm,
        Encryption::ChaCha20 => words.enc_chacha20,
        Encryption::Xor => words.enc_xor,
        Encryption::Off => words.enc_off,
    }
}

/// Address, peers, listeners, TUN, encryption, MTU and interface name.
fn connection(form: Form) -> View {
    let Form { store, node, .. } = form;
    let mut column = section(store, |s| s.node.detail.section_connection);
    if !form.managed {
        let words = store.strings().node.detail;
        let mut options = vec![
            SelectOption::new(ip_id(IpChoice::Dhcp), words.address_dhcp),
            SelectOption::new(ip_id(IpChoice::Static), words.address_static),
        ];
        // Only a profile that already has no address (an import can make one) is offered the choice.
        if form.read(|draft| draft.ip == IpChoice::None) {
            options.push(SelectOption::new(ip_id(IpChoice::None), words.address_none));
        }
        column = column.child(
            view()
                .gap_2()
                .child(segmented(
                    store.tr(|s| s.node.detail.address_label),
                    options,
                    move || SharedString::from(form.read(|draft| ip_id(draft.ip))),
                    move |picked| {
                        if let Some(choice) = ip_choice(picked) {
                            node.edit_editor(|draft| draft.ip = choice);
                        }
                    },
                    "detail-ip-mode",
                ))
                .child(meta_text(store.tr(|s| s.node.detail.address_help))),
        );
    }
    column = column
        .child(when(
            move || form.read(|draft| draft.ip == IpChoice::Static),
            move || {
                form.text(
                    store.tr(|s| s.node.shared.field_ip),
                    |draft| draft.ip_text.clone(),
                    |draft, text| draft.ip_text = text.to_owned(),
                    ProblemField::Ip,
                    "detail-ip",
                )
                .mono()
                .placeholder("10.144.144.1/24")
            },
        ))
        .child(
            form.text(
                store.tr(|s| s.node.shared.field_peers),
                |draft| draft.peers.clone(),
                |draft, text| draft.peers = text.to_owned(),
                ProblemField::Peers,
                "detail-peers",
            )
            .multiline(3)
            .mono()
            .placeholder(format!("tcp://node.example.net:{BASE_PORT}"))
            .helper(store.tr(|s| s.node.detail.peers_help)),
        );
    let listeners = move || {
        view()
            .gap_2()
            .child(
                form.text(
                    store.tr(|s| s.node.shared.field_listeners),
                    |draft| draft.listeners.clone(),
                    |draft, text| draft.listeners = text.to_owned(),
                    ProblemField::Listeners,
                    "detail-listeners-input",
                )
                .multiline(3)
                .mono()
                .placeholder(format!("tcp://0.0.0.0:{BASE_PORT}"))
                .helper(store.tr(|s| s.node.detail.listeners_help)),
            )
            .child(
                form.warnings(|exposure| matches!(exposure, Exposure::ListenerOnAllInterfaces(_))),
            )
    };
    if form.managed {
        column = column.child(when(
            move || form.read(|draft| draft.listeners_on),
            listeners,
        ));
    } else {
        column = column
            .child(
                form.gated(
                    form.toggle(
                        store.tr(|s| s.node.detail.listeners_switch),
                        |draft| draft.listeners_on,
                        |draft, on| draft.listeners_on = on,
                        "detail-listeners-on",
                    )
                    .description(store.tr(|s| s.node.detail.listeners_switch_help)),
                    |draft| draft.listeners_on,
                    listeners,
                ),
            )
            .child(
                form.text(
                    store.tr(|s| s.config.mapped_listeners),
                    |draft| draft.mapped_listeners.clone(),
                    |draft, text| draft.mapped_listeners = text.to_owned(),
                    ProblemField::MappedListeners,
                    "detail-mapped-listeners",
                )
                .multiline(3)
                .mono()
                .placeholder("tcp://203.0.113.5:11010")
                .helper(store.tr(|s| s.config.mapped_listeners_help)),
            )
            .child(tun(form));
    }
    column
        .child(encryption(form))
        .child(
            form.text(
                store.tr(|s| s.node.shared.field_mtu),
                |draft| draft.mtu.clone(),
                |draft, text| draft.mtu = text.to_owned(),
                ProblemField::Mtu,
                "detail-mtu",
            )
            .mono()
            .placeholder(DEFAULT_MTU.to_string())
            .helper(store.tr(|s| s.node.detail.mtu_help)),
        )
        .child(
            form.text(
                store.tr(|s| s.node.shared.field_dev_name),
                |draft| draft.dev_name.clone(),
                |draft, text| draft.dev_name = text.to_owned(),
                ProblemField::DevName,
                "detail-dev-name",
            )
            .mono()
            .helper(store.tr(|s| s.node.detail.dev_name_help)),
        )
}

/// The virtual network interface: a switch where it can exist, a disabled row and its reason where it cannot.
fn tun(form: Form) -> View {
    let store = form.store;
    if form.node.tun_supported() {
        return view().child(
            form.toggle(
                store.tr(|s| s.node.detail.tun_label),
                |draft| draft.tun,
                |draft, on| draft.tun = on,
                "detail-tun",
            )
            .description(store.tr(|s| s.node.detail.tun_help)),
        );
    }
    view()
        .gap_2()
        .child(
            switch_row(store.tr(|s| s.node.detail.tun_label), || false, |_| {})
                .description(store.tr(|s| s.node.detail.tun_unavailable))
                .disabled(|| true)
                .test_id("detail-tun"),
        )
        .child(
            callout(CalloutKind::Info, store.tr(|s| s.node.flow.no_tun_note))
                .test_id("detail-no-tun"),
        )
}

/// The encryption choice, with the warning of the weak ones under it.
fn encryption(form: Form) -> View {
    let Form { store, node, .. } = form;
    let words = store.strings().node.detail;
    let options = Encryption::ALL
        .into_iter()
        .map(|choice| SelectOption::new(encryption_id(choice), encryption_name(words, choice)))
        .collect::<Vec<_>>();
    view()
        .gap_2()
        .child(
            select(
                store.tr(|s| s.node.detail.encryption_label),
                options,
                move || SharedString::from(form.read(|draft| encryption_id(draft.encryption))),
                move |picked| {
                    if let Some(choice) = Encryption::ALL
                        .into_iter()
                        .find(|choice| encryption_id(*choice) == picked)
                    {
                        node.edit_editor(|draft| draft.encryption = choice);
                    }
                },
            )
            .disabled(form.managed)
            .test_id("detail-encryption"),
        )
        .child(meta_text(store.tr(|s| s.node.detail.encryption_help)))
        .child(form.warnings(|exposure| {
            matches!(
                exposure,
                Exposure::ObfuscationOnly | Exposure::EncryptionOff
            )
        }))
}

// ----- Feature switches -----

/// The core's behavior switches, grouped by what they are about. Not shown for a managed network.
fn flags(form: Form) -> View {
    let Form { store, node, .. } = form;
    section(store, |s| s.config.section_flags).child(flag_switches(
        store,
        move || form.read(|draft| draft.flags),
        move |flag, on| node.edit_editor(|draft| flag.set(&mut draft.flags, on)),
        form.managed,
        "detail",
    ))
}

// ----- Sharing and routing -----

/// What this node offers or uses beyond its own traffic: exit nodes, subnets, SOCKS5, port forwards, the VPN portal,
/// manual routes and the receive limit. Not shown for a managed network.
fn sharing(form: Form) -> View {
    let Form { store, .. } = form;
    let shared = |pick: fn(&'static crate::i18n::node::SharedStrings) -> &'static str| {
        move || pick(store.strings().node.shared)
    };
    let exit_nodes = move || {
        view().child(
            form.text(
                store.tr(|s| s.node.shared.field_exit_nodes),
                |draft| draft.exit_nodes.clone(),
                |draft, text| draft.exit_nodes = text.to_owned(),
                ProblemField::ExitNodes,
                "detail-exit-nodes",
            )
            .multiline(3)
            .mono()
            .placeholder("10.144.144.2")
            .helper(store.tr(|s| s.node.detail.exit_nodes_help)),
        )
    };
    let subnets = move || {
        view().child(
            form.text(
                store.tr(|s| s.node.shared.field_subnet_proxy),
                |draft| draft.subnets.clone(),
                |draft, text| draft.subnets = text.to_owned(),
                ProblemField::SubnetProxy,
                "detail-subnets-input",
            )
            .multiline(3)
            .mono()
            .placeholder("192.168.50.0/24->10.50.0.0/24")
            .helper(store.tr(|s| s.node.detail.subnets_help)),
        )
    };
    let socks5 = move || {
        view()
            .gap_2()
            .child(
                form.text(
                    store.tr(|s| s.node.shared.field_socks5),
                    |draft| draft.socks5.clone(),
                    |draft, text| draft.socks5 = text.to_owned(),
                    ProblemField::Socks5,
                    "detail-socks5-input",
                )
                .mono()
                .placeholder("1080")
                .helper(store.tr(|s| s.node.detail.socks5_help)),
            )
            .child(form.warnings(|exposure| matches!(exposure, Exposure::Socks5 { public: true })))
    };
    let forwards = move || {
        view()
            .gap_2()
            .child(
                form.text(
                    store.tr(|s| s.node.shared.field_port_forwards),
                    |draft| draft.forwards.clone(),
                    |draft, text| draft.forwards = text.to_owned(),
                    ProblemField::PortForwards,
                    "detail-forwards-input",
                )
                .multiline(3)
                .mono()
                .placeholder("tcp 127.0.0.1:8080 10.144.144.2:80")
                .helper(store.tr(|s| s.config.port_forwards_help)),
            )
            .child(form.warnings(|exposure| {
                matches!(exposure, Exposure::ForwardsPort { public: true, .. })
            }))
    };
    let vpn_portal = move || {
        view()
            .gap_4()
            .child(
                form.text(
                    store.tr(|s| s.config.vpn_portal_cidr),
                    |draft| draft.vpn_portal_cidr.clone(),
                    |draft, text| draft.vpn_portal_cidr = text.to_owned(),
                    ProblemField::VpnPortalCidr,
                    "detail-vpn-portal-cidr",
                )
                .mono()
                .placeholder("10.14.14.0/24")
                .helper(store.tr(|s| s.config.vpn_portal_cidr_help)),
            )
            .child(
                form.text(
                    store.tr(|s| s.config.vpn_portal_port),
                    |draft| draft.vpn_portal_port.clone(),
                    |draft, text| draft.vpn_portal_port = text.to_owned(),
                    ProblemField::VpnPortalPort,
                    "detail-vpn-portal-port",
                )
                .mono()
                .placeholder(DEFAULT_VPN_PORTAL_PORT.to_string()),
            )
    };
    let routes = move || {
        view().child(
            form.text(
                store.tr(|s| s.config.manual_routes_field),
                |draft| draft.routes.clone(),
                |draft, text| draft.routes = text.to_owned(),
                ProblemField::Routes,
                "detail-routes",
            )
            .multiline(3)
            .mono()
            .placeholder("192.168.0.0/16"),
        )
    };
    section(store, |s| s.node.detail.section_sharing)
        .child(
            form.toggle(
                store.tr(|s| s.node.detail.offers_exit),
                |draft| draft.offers_exit_node,
                |draft, on| draft.offers_exit_node = on,
                "detail-offers-exit",
            )
            .description(store.tr(|s| s.node.detail.offers_exit_help))
            .exposure(shared(|w| w.exposure_offers_exit)),
        )
        .child(
            form.gated(
                form.toggle(
                    store.tr(|s| s.node.detail.uses_exit),
                    |draft| draft.exit_nodes_on,
                    |draft, on| draft.exit_nodes_on = on,
                    "detail-uses-exit",
                )
                .description(store.tr(|s| s.node.detail.uses_exit_help))
                .exposure(shared(|w| w.exposure_uses_exit)),
                |draft| draft.exit_nodes_on,
                exit_nodes,
            ),
        )
        .child(
            form.gated(
                form.toggle(
                    store.tr(|s| s.node.detail.subnet_switch),
                    |draft| draft.subnet_on,
                    |draft, on| draft.subnet_on = on,
                    "detail-share-subnets",
                )
                .description(store.tr(|s| s.node.detail.subnet_switch_help))
                .exposure(shared(|w| w.exposure_shares_subnet)),
                |draft| draft.subnet_on,
                subnets,
            ),
        )
        .child(
            form.gated(
                form.toggle(
                    store.tr(|s| s.node.detail.socks5_switch),
                    |draft| draft.socks5_on,
                    |draft, on| draft.socks5_on = on,
                    "detail-socks5",
                )
                .description(store.tr(|s| s.node.detail.socks5_switch_help))
                .exposure(shared(|w| w.exposure_socks5_local)),
                |draft| draft.socks5_on,
                socks5,
            ),
        )
        .child(
            form.gated(
                form.toggle(
                    store.tr(|s| s.node.detail.forwards_switch),
                    |draft| draft.forwards_on,
                    |draft, on| draft.forwards_on = on,
                    "detail-forwards",
                )
                .description(store.tr(|s| s.node.detail.forwards_switch_help))
                .exposure(shared(|w| w.exposure_forward_local)),
                |draft| draft.forwards_on,
                forwards,
            ),
        )
        .child(
            form.gated(
                form.toggle(
                    store.tr(|s| s.config.vpn_portal),
                    |draft| draft.vpn_portal_on,
                    |draft, on| draft.vpn_portal_on = on,
                    "detail-vpn-portal",
                )
                .description(store.tr(|s| s.config.vpn_portal_help))
                .exposure(store.tr(|s| s.config.vpn_portal_exposure)),
                |draft| draft.vpn_portal_on,
                vpn_portal,
            ),
        )
        .child(
            form.gated(
                form.toggle(
                    store.tr(|s| s.config.manual_routes),
                    |draft| draft.routes_on,
                    |draft, on| draft.routes_on = on,
                    "detail-routes-on",
                )
                .description(store.tr(|s| s.config.manual_routes_help)),
                |draft| draft.routes_on,
                routes,
            ),
        )
        .child(
            form.text(
                store.tr(|s| s.config.recv_limit),
                |draft| draft.recv_limit.clone(),
                |draft, text| draft.recv_limit = text.to_owned(),
                ProblemField::RecvLimit,
                "detail-recv-limit",
            )
            .mono()
            .placeholder(store.tr(|s| s.config.recv_limit_placeholder))
            .helper(store.tr(|s| s.config.recv_limit_help)),
        )
}

// ----- Relaying -----

/// Whether this node relays for other networks, and which. Not shown for a managed network.
fn relaying(form: Form) -> View {
    let Form { store, node, .. } = form;
    let shared = |pick: fn(&'static crate::i18n::node::SharedStrings) -> &'static str| {
        move || pick(store.strings().node.shared)
    };
    let whitelist = view()
        .gap_2()
        .child(segmented(
            store.tr(|s| s.node.detail.whitelist_label),
            vec![
                SelectOption::new("any", store.strings().node.detail.whitelist_any),
                SelectOption::new("only", store.strings().node.detail.whitelist_only),
            ],
            move || {
                SharedString::from(if form.read(|draft| draft.whitelist_only) {
                    "only"
                } else {
                    "any"
                })
            },
            move |picked| node.edit_editor(|draft| draft.whitelist_only = picked == "only"),
            "detail-whitelist-mode",
        ))
        .child(when(
            move || form.read(|draft| draft.whitelist_only),
            move || {
                form.text(
                    store.tr(|s| s.node.shared.field_relay_whitelist),
                    |draft| draft.whitelist.clone(),
                    |draft, text| draft.whitelist = text.to_owned(),
                    ProblemField::RelayWhitelist,
                    "detail-whitelist",
                )
                .multiline(3)
                .mono()
                .placeholder("team-*")
                .helper(store.tr(|s| s.node.detail.whitelist_help))
            },
        ))
        .child(form.warnings(|exposure| matches!(exposure, Exposure::WideRelayWhitelist)));
    section(store, |s| s.node.detail.section_relaying)
        .child(
            form.toggle(
                store.tr(|s| s.node.detail.relay_all_rpc),
                |draft| draft.relay_all_peer_rpc,
                |draft, on| draft.relay_all_peer_rpc = on,
                "detail-relay-all-rpc",
            )
            .description(store.tr(|s| s.node.detail.relay_all_rpc_help))
            .exposure(shared(|w| w.exposure_relays_all_rpc)),
        )
        .child(
            view()
                .gap_2()
                .child(
                    form.toggle(
                        store.tr(|s| s.node.detail.private_mode),
                        |draft| draft.private_mode,
                        |draft, on| draft.private_mode = on,
                        "detail-private-mode",
                    )
                    .description(store.tr(|s| s.node.detail.private_mode_help)),
                )
                .child(form.warnings(|exposure| matches!(exposure, Exposure::PrivateModeOff))),
        )
        .child(whitelist)
}

// ----- Access control -----

/// The access control list: chains, rules and groups. Not shown for a managed network.
fn access(form: Form) -> View {
    let Form { store, node, .. } = form;
    section(store, |s| s.config.section_acl).child(acl_editor(
        store,
        move || form.read(|draft| draft.acl.clone()),
        move |acl| node.edit_editor(|draft| draft.acl = acl),
        form.managed,
        "detail",
    ))
}

// ----- Remove -----

/// Delete (or leave management), behind a confirmation that names the network: a row of its own at the bottom.
fn remove(form: Form, id: ProfileId) -> View {
    let Form {
        store,
        node,
        managed,
        ..
    } = form;
    let busy = computed({
        let id = id.clone();
        move || node.is_busy(&id)
    });
    let label: fn(&'static crate::i18n::Strings) -> &'static str = if managed {
        |s| s.node.list.leave_action
    } else {
        |s| s.common.delete
    };
    section(store, |s| s.node.detail.section_remove).child(
        view()
            .flex_row()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_4()
            .p_4()
            .rounded(move || theme().radius)
            .border_1()
            .border_color(move || theme().border)
            .child(
                meta_text(move || {
                    let words = store.strings().node.detail;
                    if managed {
                        words.leave_help
                    } else {
                        words.remove_help
                    }
                })
                .flex_1()
                .min_w_0(),
            )
            .child(tag(
                busy_button(
                    ButtonKind::Secondary,
                    move || label(store.strings()),
                    move || busy.get(),
                    move || ask_delete(store, id.clone()),
                ),
                "detail-delete",
            )),
    )
}
