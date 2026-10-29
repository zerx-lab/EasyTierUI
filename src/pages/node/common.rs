//! Helpers the node pages share: words for problems, failures and exposures, test hooks, and the notices that
//! come with every place a secret is entered.

use nsh_base::theme;
use nsh_core::prelude::*;

use crate::{
    components::{
        ButtonKind, CalloutKind, Column, CopyLabels, Icon, Lamp, SecretLabels, action_button,
        app_color, callout, confirm, icon_colored, machine_text, meta_text,
    },
    core::state::NatType,
    i18n::{Strings, fill, node::SharedStrings},
    profile::{Exposure, ProfileId},
    store::{
        Store,
        node::{
            Field, FieldProblem, ImportFailure, ImportReason, ProblemKind, SaveFailure, Status,
        },
    },
};

/// The narrowest window that gets the table's header and columns; below it (and on touch screens) rows stack.
const STACK_BELOW: f32 = 640.0;

/// Whether tables stack here: touch screens and windows narrower than 640 pt.
pub(super) fn stacked() -> bool {
    !is_desktop() || untracked(window).width.0 < STACK_BELOW
}

/// `column`, left out of a wide table while the window is narrower than `below` points; stacked rows keep every
/// column because their facts wrap instead of competing for width.
pub(super) fn responsive(column: Column, below: f32) -> Column {
    if stacked() {
        column
    } else {
        column.hide_below(below)
    }
}

/// Asks, in a confirmation that names network `id`, whether to delete it (or to leave its management), and does it
/// when the answer is yes.
pub(super) fn ask_delete(store: Store, id: ProfileId) {
    let node = store.node();
    let Some(profile) = node.profile(&id) else {
        return;
    };
    let strings = store.strings();
    let list = strings.node.list;
    let (title, body, action) = if profile.is_managed() {
        (list.leave_title, list.leave_body, list.leave_action)
    } else {
        (list.delete_title, list.delete_body, strings.common.delete)
    };
    let asked = confirm(
        fill(title, &[("name", profile.name.as_str())]),
        body,
        action,
        strings.common.cancel,
        true,
    );
    spawn(async move {
        if asked.await {
            node.delete(id);
        }
    });
}

/// A button of `kind` with `icon` before its label: the plain labelled button with the icon placed first.
pub(super) fn icon_label_button(
    kind: ButtonKind,
    icon: Icon,
    label: impl IntoReactive<SharedString>,
    on_press: impl Fn() + 'static,
) -> impl IntoElement {
    let tint = move || match kind {
        ButtonKind::Primary => theme().primary_foreground,
        ButtonKind::Destructive => app_color("destructive-foreground"),
        ButtonKind::Secondary | ButtonKind::Ghost => theme().muted_foreground,
    };
    action_button(kind, label, on_press)
        .gap_1p5()
        .flex_row_reverse()
        .child(icon_colored(icon, 14.0, tint))
}

/// `element` carrying the test id `id`: nsh-base's buttons and switches take no id of their own, so a plain view
/// around them does (a headless test presses `id:<id>`).
pub(super) fn tag(element: impl IntoElement, id: &str) -> View {
    view().test_id(id).child(element)
}

/// The words of a copy button.
pub(super) fn copy_labels(store: Store) -> CopyLabels {
    CopyLabels::new(store.tr(|s| s.common.copy), store.tr(|s| s.common.copied))
}

/// The words of a secret field's buttons.
pub(super) fn secret_labels(store: Store) -> SecretLabels {
    SecretLabels::new(
        store.tr(|s| s.common.show),
        store.tr(|s| s.common.hide),
        copy_labels(store),
    )
}

/// The name of `field` in the user's language.
pub(super) fn field_label(words: &'static SharedStrings, field: Field) -> &'static str {
    match field {
        Field::Name => words.field_name,
        Field::Hostname => words.field_hostname,
        Field::NetworkName => words.field_network_name,
        Field::Secret => words.field_secret,
        Field::Ip => words.field_ip,
        Field::Peers => words.field_peers,
        Field::Listeners => words.field_listeners,
        Field::Mtu => words.field_mtu,
        Field::DevName => words.field_dev_name,
        Field::SubnetProxy => words.field_subnet_proxy,
        Field::ExitNodes => words.field_exit_nodes,
        Field::Socks5 => words.field_socks5,
        Field::PortForwards => words.field_port_forwards,
        Field::RelayWhitelist => words.field_relay_whitelist,
        Field::ConfigServer => words.field_config_server,
        Field::Import => words.field_import,
        Field::Other => words.field_other,
    }
}

/// What is wrong, as a sentence without the field's name.
pub(super) fn reason_text(words: &'static SharedStrings, kind: ProblemKind) -> String {
    let text = |template: &str, args: &[(&str, &str)]| fill(template, args);
    match kind {
        ProblemKind::Required => words.reason_required.to_owned(),
        ProblemKind::TooLong { max } => text(words.reason_too_long, &[("max", &max.to_string())]),
        ProblemKind::ControlChars => words.reason_control_chars.to_owned(),
        ProblemKind::TooMany { max } => text(words.reason_too_many, &[("max", &max.to_string())]),
        ProblemKind::Duplicate => words.reason_duplicate.to_owned(),
        ProblemKind::OutOfRange { min, max } => text(
            words.reason_out_of_range,
            &[("min", &min.to_string()), ("max", &max.to_string())],
        ),
        ProblemKind::NameChars => words.reason_name_chars.to_owned(),
        ProblemKind::NoSpaces => words.reason_no_spaces.to_owned(),
        ProblemKind::NotAnAddress => words.reason_not_an_address.to_owned(),
        ProblemKind::UnknownScheme => words.reason_unknown_scheme.to_owned(),
        ProblemKind::ConnectorOnly => words.reason_connector_only.to_owned(),
        ProblemKind::MissingHost => words.reason_missing_host.to_owned(),
        ProblemKind::HostNotIp => words.reason_host_not_ip.to_owned(),
        ProblemKind::BadPort => words.reason_bad_port.to_owned(),
        ProblemKind::WellKnownPort => words.reason_well_known_port.to_owned(),
        ProblemKind::ExtraParts => words.reason_extra_parts.to_owned(),
        ProblemKind::NotACidr => words.reason_not_a_cidr.to_owned(),
        ProblemKind::BadPrefix { min, max } => text(
            words.reason_bad_prefix,
            &[("min", &min.to_string()), ("max", &max.to_string())],
        ),
        ProblemKind::HostBits => words.reason_host_bits.to_owned(),
        ProblemKind::Reserved => words.reason_reserved.to_owned(),
        ProblemKind::ForwardShape => words.reason_forward_shape.to_owned(),
        ProblemKind::ConfigServerShape => words.reason_config_server_shape.to_owned(),
        ProblemKind::Other => words.reason_other.to_owned(),
    }
}

/// A problem as one sentence naming its field (and the item of a list).
pub(super) fn problem_text(words: &'static SharedStrings, problem: &FieldProblem) -> String {
    let field = field_label(words, problem.field);
    let reason = reason_text(words, problem.kind);
    match problem.entry {
        Some(entry) => fill(
            words.problem_entry,
            &[
                ("field", field),
                ("entry", &entry.to_string()),
                ("reason", &reason),
            ],
        ),
        None => fill(
            words.problem_plain,
            &[("field", field), ("reason", &reason)],
        ),
    }
}

/// The first problem about `field`, worded, for a field's error line.
pub(super) fn error_of(
    strings: &'static Strings,
    problems: &[FieldProblem],
    field: Field,
) -> Option<SharedString> {
    problems
        .iter()
        .find(|problem| problem.field == field)
        .map(|problem| SharedString::from(problem_text(strings.node.shared, problem)))
}

/// Why a save (or a delete) failed.
pub(super) fn save_failure_text(words: &'static SharedStrings, failure: &SaveFailure) -> String {
    match failure {
        SaveFailure::Keychain => words.save_keychain.to_owned(),
        SaveFailure::StorageFull => words.save_storage_full.to_owned(),
        SaveFailure::Damaged => words.save_damaged.to_owned(),
        SaveFailure::NewerSchema => words.save_newer.to_owned(),
        SaveFailure::NoRandomSource => words.save_no_random.to_owned(),
        SaveFailure::Background => words.save_background.to_owned(),
        SaveFailure::Invalid(problem) => problem_text(words, problem),
        SaveFailure::Remove(_) => words.remove_failed.to_owned(),
    }
}

/// A danger callout for a failed save, or nothing: `failure` is read where the view is bound.
pub(super) fn save_failure_view(
    store: Store,
    failure: impl Fn() -> Option<SaveFailure> + 'static,
) -> AnyElement {
    dynamic(move || {
        failure().map(|failure| {
            let message =
                SharedString::from(save_failure_text(store.strings().node.shared, &failure));
            let detail = match &failure {
                SaveFailure::Remove(detail) => Some(detail.clone()),
                _ => None,
            };
            view()
                .gap_1()
                .child(callout(CalloutKind::Danger, message).test_id("save-failure"))
                .when_some(detail, |column, detail| column.child(machine_text(detail)))
        })
    })
    .into_element()
}

/// The notice that goes with every place a secret is typed or pasted: where this device cannot keep secrets.
pub(super) fn secrets_notice(store: Store) -> AnyElement {
    if store.secrets_persistent() {
        return view().into_element();
    }
    callout(CalloutKind::Info, store.tr(|s| s.common.secrets_not_kept))
        .test_id("secrets-not-kept")
        .into_element()
}

/// Why an import was refused, as a key (monospace, if there is one) and a sentence.
pub(super) fn import_failure_view(store: Store, failure: ImportFailure) -> View {
    let words = store.strings().node.shared;
    let line = failure
        .line
        .map(|line| fill(words.import_line, &[("line", &line.to_string())]));
    let (sentence, keyed) = match failure.reason {
        ImportReason::Empty => (words.import_empty.to_owned(), false),
        ImportReason::TooLarge => (words.import_too_large.to_owned(), false),
        ImportReason::Syntax => (words.import_syntax.to_owned(), false),
        ImportReason::WrongType => (words.import_wrong_type.to_owned(), true),
        ImportReason::Missing => (words.import_missing.to_owned(), true),
        ImportReason::Unsupported => (words.import_unsupported.to_owned(), true),
        ImportReason::Invalid(kind) => (reason_text(words, kind), failure.key.is_some()),
        ImportReason::NotALink => (words.import_not_a_link.to_owned(), false),
        ImportReason::LinkVersion => (words.import_link_version.to_owned(), false),
        ImportReason::LinkNoConfig => (words.import_link_no_config.to_owned(), false),
        ImportReason::LinkDamaged => (words.import_link_damaged.to_owned(), false),
        ImportReason::ImageType => (words.import_image_type.to_owned(), false),
        ImportReason::ImageTooLarge => (words.import_image_too_large.to_owned(), false),
        ImportReason::ImageDecode => (words.import_image_decode.to_owned(), false),
        ImportReason::NoQr => (words.import_no_qr.to_owned(), false),
        ImportReason::QrUnreadable => (words.import_qr_unreadable.to_owned(), false),
        ImportReason::PickFailed => (words.import_pick_failed.to_owned(), false),
        ImportReason::NoRandomSource => (words.save_no_random.to_owned(), false),
    };
    let key = if keyed { failure.key } else { None };
    view()
        .gap_1()
        .role(AccessibilityRole::Alert)
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .when_some(key, |row, key| {
                    row.child(machine_text(key).text_color(move || nsh_base::theme().destructive))
                })
                .child(
                    text(sentence)
                        .text_sm()
                        .text_color(move || nsh_base::theme().destructive),
                ),
        )
        .when_some(line, |column, line| column.child(meta_text(line)))
}

/// An exposure as a warning with the thing it concerns in monospace under it.
pub(super) fn exposure_item(store: Store, exposure: &Exposure) -> View {
    let (pick, value): (fn(&'static SharedStrings) -> &'static str, Option<String>) = match exposure
    {
        Exposure::ListenerOnAllInterfaces(listener) => (
            |w| w.exposure_listener_all,
            Some(listener.as_str().to_owned()),
        ),
        Exposure::OffersExitNode => (|w| w.exposure_offers_exit, None),
        Exposure::UsesExitNode(address) => (|w| w.exposure_uses_exit, Some(address.to_string())),
        Exposure::SharesSubnet(subnet) => (|w| w.exposure_shares_subnet, Some(subnet.to_string())),
        Exposure::ForwardsPort { forward, public } => {
            let value = format!(
                "{} {} → {}",
                forward.proto.name(),
                forward.bind,
                forward.dst
            );
            if *public {
                (|w| w.exposure_forward_public, Some(value))
            } else {
                (|w| w.exposure_forward_local, Some(value))
            }
        }
        Exposure::Socks5 { public } => {
            if *public {
                (|w| w.exposure_socks5_public, None)
            } else {
                (|w| w.exposure_socks5_local, None)
            }
        }
        Exposure::EncryptionOff => (|w| w.exposure_encryption_off, None),
        Exposure::ObfuscationOnly => (|w| w.exposure_obfuscation, None),
        Exposure::PrivateModeOff => (|w| w.exposure_private_mode_off, None),
        Exposure::RelaysAllPeerRpc => (|w| w.exposure_relays_all_rpc, None),
        Exposure::WideRelayWhitelist => (|w| w.exposure_wide_whitelist, None),
    };
    view()
        .gap_1()
        .child(callout(CalloutKind::Warning, move || {
            pick(store.strings().node.shared)
        }))
        .when_some(value, |column, value| {
            column.child(view().px_4().child(machine_text(value)))
        })
}

/// The lamp a status shows.
pub(super) fn lamp_of(status: &Status) -> Lamp {
    match status {
        Status::Stopped => Lamp::Idle,
        Status::Starting | Status::Stopping | Status::Removing => Lamp::Busy,
        Status::Running => Lamp::Good,
        Status::Failed(_) => Lamp::Failed,
    }
}

/// The word beside the lamp.
pub(super) fn status_word(strings: &'static Strings, status: &Status) -> &'static str {
    let words = strings.common;
    match status {
        Status::Stopped => words.state_stopped,
        Status::Starting => words.state_starting,
        Status::Running => words.state_running,
        Status::Stopping | Status::Removing => words.state_stopping,
        Status::Failed(_) => words.state_failed,
    }
}

/// A NAT type in the user's language.
pub(super) fn nat_word(store: Store, nat: NatType) -> &'static str {
    let words = store.strings().node.detail;
    match nat {
        NatType::Unknown => words.nat_unknown,
        NatType::OpenInternet => words.nat_open_internet,
        NatType::NoPat => words.nat_no_pat,
        NatType::FullCone => words.nat_full_cone,
        NatType::Restricted => words.nat_restricted,
        NatType::PortRestricted => words.nat_port_restricted,
        NatType::Symmetric => words.nat_symmetric,
        NatType::SymUdpFirewall => words.nat_sym_udp_firewall,
        NatType::SymmetricEasyInc => words.nat_symmetric_easy_inc,
        NatType::SymmetricEasyDec => words.nat_symmetric_easy_dec,
    }
}
