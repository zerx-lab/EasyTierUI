//! The rule dialog: every field of one rule over signals that live as long as the editor, loaded when the dialog
//! opens and applied only when every field reads fine.

use nsh_core::prelude::*;

use super::{
    acl::{Ctx, Sink, Source, tag},
    draft::{FieldError, RuleErrors, RuleInput},
    ops,
    text::{field_error_text, problem_text},
};
use crate::{
    components::{
        ButtonKind, CalloutKind, DialogFrame, Field as TextField, SelectOption, action_button,
        callout, dialog_footer, dialog_frame, field, meta_text, section_label, segmented, select,
        switch_row,
    },
    core::acl::{AclAction, AclProtocol, AclRule},
    i18n::Pick,
};

/// Which rule the dialog is about: a rule of a chain, or a new one for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    chain: usize,
    rule: Option<usize>,
}

/// The dialog's state. A handle of signals, created once with the editor.
#[derive(Clone, Copy)]
pub(super) struct Ui {
    slot: Signal<Option<Slot>>,
    name: Signal<String>,
    description: Signal<String>,
    enabled: Signal<bool>,
    action: Signal<AclAction>,
    protocol: Signal<AclProtocol>,
    source_ips: Signal<String>,
    destination_ips: Signal<String>,
    source_ports: Signal<String>,
    destination_ports: Signal<String>,
    priority: Signal<String>,
    stateful: Signal<bool>,
    rate_limit: Signal<String>,
    burst_limit: Signal<String>,
    source_groups: Signal<Vec<String>>,
    destination_groups: Signal<Vec<String>>,
    /// Whether saving was tried: the dialog then says why it did not work.
    submitted: Signal<bool>,
}

impl Ui {
    /// A closed dialog.
    pub(super) fn new() -> Self {
        Self {
            slot: signal(None),
            name: signal(String::new()),
            description: signal(String::new()),
            enabled: signal(true),
            action: signal(AclAction::Allow),
            protocol: signal(AclProtocol::Any),
            source_ips: signal(String::new()),
            destination_ips: signal(String::new()),
            source_ports: signal(String::new()),
            destination_ports: signal(String::new()),
            priority: signal(String::new()),
            stateful: signal(false),
            rate_limit: signal(String::new()),
            burst_limit: signal(String::new()),
            source_groups: signal(Vec::new()),
            destination_groups: signal(Vec::new()),
            submitted: signal(false),
        }
    }

    /// Fills the fields from `input`.
    fn load(self, input: &RuleInput) {
        batch(|| {
            self.name.set(input.name.clone());
            self.description.set(input.description.clone());
            self.enabled.set(input.enabled);
            self.action.set(input.action);
            self.protocol.set(input.protocol);
            self.source_ips.set(input.source_ips.clone());
            self.destination_ips.set(input.destination_ips.clone());
            self.source_ports.set(input.source_ports.clone());
            self.destination_ports.set(input.destination_ports.clone());
            self.priority.set(input.priority.clone());
            self.stateful.set(input.stateful);
            self.rate_limit.set(input.rate_limit.clone());
            self.burst_limit.set(input.burst_limit.clone());
            self.source_groups.set(input.source_groups.clone());
            self.destination_groups
                .set(input.destination_groups.clone());
            self.submitted.set(false);
        });
    }

    /// The fields as they are now (read where a binding runs, it follows them).
    fn input(self) -> RuleInput {
        RuleInput {
            name: self.name.get(),
            description: self.description.get(),
            enabled: self.enabled.get(),
            action: self.action.get(),
            protocol: self.protocol.get(),
            source_ips: self.source_ips.get(),
            destination_ips: self.destination_ips.get(),
            source_ports: self.source_ports.get(),
            destination_ports: self.destination_ports.get(),
            priority: self.priority.get(),
            stateful: self.stateful.get(),
            rate_limit: self.rate_limit.get(),
            burst_limit: self.burst_limit.get(),
            source_groups: self.source_groups.get(),
            destination_groups: self.destination_groups.get(),
        }
    }
}

impl<G: Source, S: Sink> Ctx<G, S> {
    /// Opens the dialog on rule `rule` of chain `chain` (`None`: a new rule for it).
    pub(super) fn open_rule(self, chain: usize, rule: Option<usize>) {
        let input = match rule {
            Some(index) => self
                .chain(chain, |chain| {
                    chain.rules.get(index).map(RuleInput::from_rule)
                })
                .flatten(),
            None => self.chain(chain, |_| RuleInput::from_rule(&AclRule::new(""))),
        };
        let Some(input) = input else {
            return;
        };
        batch(|| {
            self.rule_ui.load(&input);
            self.rule_ui.slot.set(Some(Slot { chain, rule }));
        });
    }

    /// The dialog, closed until [`Ctx::open_rule`].
    pub(super) fn rule_dialog(self) -> DialogFrame {
        let slot = self.rule_ui.slot;
        dialog_frame(
            move || slot.get().is_some(),
            move || {
                let editing = slot.get().is_some_and(|slot| slot.rule.is_some());
                let words = self.words();
                if editing {
                    words.acl_rule_edit
                } else {
                    words.acl_rule_add
                }
            },
            move || slot.set(None),
        )
        .wide()
        .body(move || untracked(|| self.rule_form()))
        .footer(move || untracked(|| self.rule_footer()))
        .test_id(self.id("rule-dialog"))
    }

    /// A text field over `value`.
    fn text_field(self, label: Pick, value: Signal<String>, id: &str) -> TextField {
        field(
            self.store.tr(label),
            move || value.get(),
            move |text| value.set(text.to_owned()),
        )
        .test_id(self.id(id))
    }

    /// The error line of a field: what `pick` finds wrong with it in the fields as they are now.
    fn field_error(
        self,
        pick: fn(&RuleErrors) -> Option<FieldError>,
    ) -> impl Fn() -> Option<SharedString> + 'static {
        move || {
            let errors = self.rule_ui.input().errors();
            pick(&errors).map(|error| SharedString::from(field_error_text(self.words(), error)))
        }
    }

    /// The fields.
    fn rule_form(self) -> View {
        let ui = self.rule_ui;
        let store = self.store;
        let words = self.words();
        let protocols = AclProtocol::ALL
            .into_iter()
            .map(|protocol| {
                let label = match protocol {
                    AclProtocol::Any => words.acl_protocol_any,
                    other => other.label(),
                };
                SelectOption::new(ops::protocol_id(protocol), label)
            })
            .collect::<Vec<_>>();
        let ports = move || {
            view()
                .gap_4()
                .child(
                    self.text_field(
                        |s| s.config.acl_rule_source_ports,
                        ui.source_ports,
                        "rule-source-ports",
                    )
                    .multiline(2)
                    .mono()
                    .placeholder("1000-2000")
                    .error(self.field_error(|errors| errors.source_ports)),
                )
                .child(
                    self.text_field(
                        |s| s.config.acl_rule_destination_ports,
                        ui.destination_ports,
                        "rule-destination-ports",
                    )
                    .multiline(2)
                    .mono()
                    .placeholder("22, 80, 443")
                    .helper(store.tr(|s| s.config.acl_rule_ports_help))
                    .error(self.field_error(|errors| errors.destination_ports)),
                )
        };
        view()
            .gap_4()
            .child(self.text_field(|s| s.config.acl_rule_name, ui.name, "rule-name"))
            .child(self.text_field(
                |s| s.config.acl_rule_description,
                ui.description,
                "rule-description",
            ))
            .child(
                switch_row(
                    store.tr(|s| s.config.acl_rule_enabled),
                    move || ui.enabled.get(),
                    move |on| ui.enabled.set(on),
                )
                .test_id(self.id("rule-enabled")),
            )
            .child(segmented(
                store.tr(|s| s.config.acl_rule_action),
                [AclAction::Allow, AclAction::Drop]
                    .into_iter()
                    .map(|action| {
                        SelectOption::new(
                            ops::action_id(action),
                            super::acl::action_text(words, action),
                        )
                    })
                    .collect(),
                move || SharedString::from(ops::action_id(ui.action.get())),
                move |id| {
                    if let Some(action) = ops::action_from_id(id) {
                        ui.action.set(action);
                    }
                },
                &self.id("rule-action"),
            ))
            .child(
                select(
                    store.tr(|s| s.config.acl_rule_protocol),
                    protocols,
                    move || SharedString::from(ops::protocol_id(ui.protocol.get())),
                    move |id| {
                        if let Some(protocol) = ops::protocol_from_id(id) {
                            ui.protocol.set(protocol);
                        }
                    },
                )
                .test_id(self.id("rule-protocol")),
            )
            .child(
                self.text_field(
                    |s| s.config.acl_rule_source_ips,
                    ui.source_ips,
                    "rule-source-ips",
                )
                .multiline(2)
                .mono()
                .placeholder("10.144.144.0/24")
                .error(self.field_error(|errors| errors.source_ips)),
            )
            .child(
                self.text_field(
                    |s| s.config.acl_rule_destination_ips,
                    ui.destination_ips,
                    "rule-destination-ips",
                )
                .multiline(2)
                .mono()
                .placeholder("10.144.144.2")
                .helper(store.tr(|s| s.config.acl_rule_ips_help))
                .error(self.field_error(|errors| errors.destination_ips)),
            )
            .child(
                when(move || ui.protocol.get().has_ports(), ports)
                    .otherwise(move || meta_text(store.tr(|s| s.config.acl_rule_no_ports))),
            )
            .child(self.group_pickers())
            .child(
                self.text_field(|s| s.config.acl_rule_priority, ui.priority, "rule-priority")
                    .mono()
                    .placeholder("1000")
                    .helper(store.tr(|s| s.config.acl_rule_priority_help))
                    .error(self.field_error(|errors| errors.priority)),
            )
            .child(
                switch_row(
                    store.tr(|s| s.config.acl_rule_stateful),
                    move || ui.stateful.get(),
                    move |on| ui.stateful.set(on),
                )
                .description(store.tr(|s| s.config.acl_rule_stateful_help))
                .test_id(self.id("rule-stateful")),
            )
            .child(
                self.text_field(
                    |s| s.config.acl_rule_rate_limit,
                    ui.rate_limit,
                    "rule-rate-limit",
                )
                .mono()
                .placeholder("0")
                .helper(store.tr(|s| s.config.acl_rule_limit_help))
                .error(self.field_error(|errors| errors.rate_limit)),
            )
            .child(
                self.text_field(
                    |s| s.config.acl_rule_burst_limit,
                    ui.burst_limit,
                    "rule-burst-limit",
                )
                .mono()
                .placeholder("0")
                .error(self.field_error(|errors| errors.burst_limit)),
            )
            .child(self.problem_line())
    }

    /// The pickers of the source and destination groups, or a note while no group is known.
    fn group_pickers(self) -> View {
        let ui = self.rule_ui;
        let store = self.store;
        let current = [
            ui.source_groups.get_untracked(),
            ui.destination_groups.get_untracked(),
        ]
        .concat();
        let names = self
            .config()
            .map(|config| ops::known_groups(&config, &current))
            .unwrap_or_default();
        if names.is_empty() {
            return view().child(meta_text(store.tr(|s| s.config.acl_rule_no_groups)));
        }
        view()
            .gap_4()
            .child(self.group_picker(
                |s| s.config.acl_rule_source_groups,
                ui.source_groups,
                "rule-source-group",
                &names,
            ))
            .child(self.group_picker(
                |s| s.config.acl_rule_destination_groups,
                ui.destination_groups,
                "rule-destination-group",
                &names,
            ))
            .child(meta_text(store.tr(|s| s.config.acl_rule_groups_help)))
    }

    /// A switch per group: on when the rule matches on it.
    fn group_picker(
        self,
        label: Pick,
        chosen: Signal<Vec<String>>,
        id: &str,
        names: &[String],
    ) -> View {
        let mut list = view().gap_2().child(section_label(self.store.tr(label)));
        for (index, name) in names.iter().enumerate() {
            let shown = name.clone();
            let member = name.clone();
            let toggled = name.clone();
            list = list.child(
                switch_row(
                    move || shown.clone(),
                    move || chosen.with(|groups| groups.contains(&member)),
                    move |on| {
                        chosen.update(|groups| {
                            groups.retain(|group| *group != toggled);
                            if on {
                                groups.push(toggled.clone());
                            }
                        });
                    },
                )
                .test_id(self.id(&format!("{id}-{index}"))),
            );
        }
        list
    }

    /// Why the rule cannot be saved yet, once saving was tried or as soon as the ACL's own limits say so.
    fn problem_line(self) -> AnyElement {
        let ui = self.rule_ui;
        dynamic(move || {
            let (existing, editing) = match ui.slot.get() {
                Some(Slot { chain, rule }) => (
                    self.chain(chain, |chain| chain.rules.len()).unwrap_or(0),
                    rule.is_some(),
                ),
                None => (0, false),
            };
            match ui.input().build() {
                Ok(rule) => ops::rule_problem(existing, editing, &rule).map(|problem| {
                    callout(CalloutKind::Danger, problem_text(self.words(), problem))
                        .test_id(self.id("rule-problem"))
                }),
                Err(_) => ui.submitted.get().then(|| {
                    callout(
                        CalloutKind::Danger,
                        self.store.tr(|s| s.config.acl_rule_fix_fields),
                    )
                    .test_id(self.id("rule-fix"))
                }),
            }
        })
    }

    /// Cancel and Save.
    fn rule_footer(self) -> View {
        let store = self.store;
        let slot = self.rule_ui.slot;
        dialog_footer()
            .child(tag(
                action_button(
                    ButtonKind::Ghost,
                    store.tr(|s| s.common.cancel),
                    move || slot.set(None),
                ),
                &self.id("rule-cancel"),
            ))
            .child(tag(
                action_button(
                    ButtonKind::Primary,
                    store.tr(|s| s.common.save),
                    move || self.save_rule(),
                ),
                &self.id("rule-save"),
            ))
    }

    /// Applies the fields to the chain when every one reads fine and the ACL's limits hold; otherwise the dialog
    /// stays open and says what is wrong.
    fn save_rule(self) {
        let ui = self.rule_ui;
        let Some(Slot { chain, rule }) = ui.slot.get_untracked() else {
            return;
        };
        let Ok(built) = ui.input().build() else {
            ui.submitted.set(true);
            return;
        };
        let existing = self.chain(chain, |chain| chain.rules.len()).unwrap_or(0);
        if ops::rule_problem(existing, rule.is_some(), &built).is_some() {
            ui.submitted.set(true);
            return;
        }
        self.edit_chain(chain, |chain| match rule {
            Some(index) => {
                if let Some(target) = chain.rules.get_mut(index) {
                    *target = built;
                }
            }
            None => chain.rules.push(built),
        });
        ui.slot.set(None);
    }
}
