//! The access control editor: a switch, the chains with their rules, the groups and the first problem of the whole
//! ACL. The rule and group dialogs live beside it ([`super::rules`], [`super::groups`]).
//!
//! The editor holds no copy of the ACL: `get` and `set` are the host's, every control reads through `get` and writes
//! through `set`. Parts that depend on the number of chains or rules are rebuilt only when that changes, so typing
//! into a name never rebuilds the field.

use nsh_core::prelude::*;

use super::{groups, ops, ops::RuleRow, rules, text::acl_error_text};
use crate::{
    components::{
        ButtonKind, CalloutKind, Icon, IconTone, SelectOption, SwitchRow, action_button, body_text,
        callout, card, confirm, empty_state, field, group, group_row, icon_button, machine_text,
        meta_text, neutral_chip, section_label, segmented, select, switch_row,
    },
    core::acl::{AclAction, AclChain, AclConfig, AclError, ChainKind, MAX_CHAINS, MAX_RULES},
    i18n::{ConfigStrings, fill},
    store::Store,
};

/// Where the editor reads the ACL from.
pub(super) trait Source: Fn() -> Option<AclConfig> + Copy + 'static {}
impl<T> Source for T where T: Fn() -> Option<AclConfig> + Copy + 'static {}

/// Where the editor writes the ACL to.
pub(super) trait Sink: Fn(Option<AclConfig>) + Copy + 'static {}
impl<T> Sink for T where T: Fn(Option<AclConfig>) + Copy + 'static {}

/// What the editor's parts share: the host's accessors, the dialogs' state and the test id prefix. It is a handle
/// (everything in it is `Copy`), so closures take it by value.
#[derive(Clone, Copy)]
pub(super) struct Ctx<G: Source, S: Sink> {
    pub(super) store: Store,
    pub(super) get: G,
    pub(super) set: S,
    pub(super) locked: bool,
    pub(super) prefix: &'static str,
    pub(super) rule_ui: rules::Ui,
    pub(super) group_ui: groups::Ui,
}

/// `element` carrying the test id `id`: nsh-base's buttons take no id of their own, so a plain view around them does.
pub(super) fn tag(element: impl IntoElement, id: &str) -> View {
    view().test_id(id).child(element)
}

/// The word of an action.
pub(super) const fn action_text(words: &ConfigStrings, action: AclAction) -> &'static str {
    match action {
        AclAction::Allow => words.acl_allow,
        AclAction::Drop => words.acl_drop,
    }
}

/// The access control editor of a network.
///
/// `get` reads the ACL (a signal, so the editor follows it): `None` is access control off. `set` receives the next
/// ACL after each edit and `None` when the person turns it off. `locked` shows everything read-only (a network
/// managed by a control plane). Test ids are `{test_prefix}-acl-…`: `switch`, `error`, `add-{inbound|outbound|forward}`,
/// `chain-{i}` with `-name`, `-description`, `-kind`, `-enabled`, `-default`, `-delete`, `-add-rule`, rows
/// `rule-{chain}-{rule}` with `-edit` and `-delete`, the rule dialog `rule-dialog` with `rule-name`, …, `rule-save`,
/// and the group parts `group-add`, `group-{i}` with `-edit` and `-delete`, `joined-{i}`, `group-dialog`.
pub(in crate::pages) fn acl_editor(
    store: Store,
    get: impl Fn() -> Option<AclConfig> + Copy + 'static,
    set: impl Fn(Option<AclConfig>) + Copy + 'static,
    locked: bool,
    test_prefix: &'static str,
) -> View {
    let ctx = Ctx {
        store,
        get,
        set,
        locked,
        prefix: test_prefix,
        rule_ui: rules::Ui::new(),
        group_ui: groups::Ui::new(),
    };
    let on = computed(move || get().is_some());
    let problem = computed(move || get().and_then(|config| config.check().err()));
    view()
        .gap_4()
        .child(ctx.switch(on))
        .child(when(
            move || on.get(),
            move || untracked(|| ctx.body(problem)),
        ))
        .child(ctx.rule_dialog())
        .child(ctx.group_dialog())
}

impl<G: Source, S: Sink> Ctx<G, S> {
    /// The words of the shared settings.
    pub(super) fn words(self) -> &'static ConfigStrings {
        self.store.strings().config
    }

    /// The test id `{prefix}-acl-{rest}`.
    pub(super) fn id(self, rest: &str) -> String {
        format!("{}-acl-{rest}", self.prefix)
    }

    /// The ACL as the host holds it now.
    pub(super) fn config(self) -> Option<AclConfig> {
        (self.get)()
    }

    /// Changes the ACL and hands it to the host; nothing happens while access control is off.
    pub(super) fn edit(self, change: impl FnOnce(&mut AclConfig)) {
        if self.locked {
            return;
        }
        let Some(mut config) = self.config() else {
            return;
        };
        change(&mut config);
        (self.set)(Some(config));
    }

    /// Something of chain `index`.
    pub(super) fn chain<T>(self, index: usize, read: impl FnOnce(&AclChain) -> T) -> Option<T> {
        self.config()
            .and_then(|config| config.acl.chains.get(index).map(read))
    }

    /// Changes chain `index`.
    pub(super) fn edit_chain(self, index: usize, change: impl FnOnce(&mut AclChain)) {
        self.edit(|config| {
            if let Some(chain) = config.acl.chains.get_mut(index) {
                change(chain);
            }
        });
    }

    /// The switch that turns access control on and off.
    fn switch(self, on: Memo<bool>) -> SwitchRow {
        switch_row(
            self.store.tr(|s| s.config.acl_switch),
            move || on.get(),
            move |next| self.toggle(next),
        )
        .description(self.store.tr(|s| s.config.acl_switch_help))
        .disabled(move || self.locked)
        .test_id(self.id("switch"))
    }

    /// Turns access control on (empty) or off (after a confirmation when there is something to lose).
    fn toggle(self, next: bool) {
        if self.locked {
            return;
        }
        let current = self.config();
        if next {
            if current.is_none() {
                (self.set)(Some(AclConfig::default()));
            }
            return;
        }
        let Some(config) = current else {
            return;
        };
        if ops::is_blank(&config) {
            (self.set)(None);
            return;
        }
        let strings = self.store.strings();
        let asked = confirm(
            strings.config.acl_off_title,
            strings.config.acl_off_body,
            strings.config.acl_off_action,
            strings.common.cancel,
            true,
        );
        let set = self.set;
        spawn(async move {
            if asked.await {
                set(None);
            }
        });
    }

    /// The editor while access control is on.
    fn body(self, problem: Memo<Option<AclError>>) -> View {
        view()
            .gap_6()
            .child(self.chains())
            .child(self.groups())
            .child(dynamic(move || {
                problem.get().map(|error| {
                    callout(CalloutKind::Danger, acl_error_text(self.store, &error))
                        .test_id(self.id("error"))
                })
            }))
    }

    // ----- Chains -----

    /// The chains, an empty state while there are none, and the buttons that add one.
    fn chains(self) -> View {
        let store = self.store;
        let count = computed(move || self.config().map_or(0, |config| config.acl.chains.len()));
        view()
            .gap_3()
            .child(section_label(store.tr(|s| s.config.acl_chains)).role(AccessibilityRole::Header))
            .child(when(
                move || count.get() == 0,
                move || {
                    card().child(empty_state(
                        store.tr(|s| s.config.acl_empty_title),
                        store.tr(|s| s.config.acl_empty_body),
                    ))
                },
            ))
            .child(dynamic(move || {
                let chains = count.get();
                untracked(|| {
                    (0..chains)
                        .map(|index| self.chain_card(index))
                        .collect::<Vec<_>>()
                })
            }))
            .child(when(
                move || !self.locked && count.get() < MAX_CHAINS,
                move || self.add_chain_buttons(),
            ))
    }

    /// One button per kind of chain.
    fn add_chain_buttons(self) -> View {
        let mut row = view().flex_row().flex_wrap().gap_2();
        for kind in ChainKind::ALL {
            row = row.child(tag(
                action_button(
                    ButtonKind::Secondary,
                    move || self.words().acl_add_chain(kind),
                    move || self.add_chain(kind),
                ),
                &self.id(&format!("add-{}", ops::kind_id(kind))),
            ));
        }
        row
    }

    /// Adds an empty chain of `kind` named for its kind.
    fn add_chain(self, kind: ChainKind) {
        let name = self.words().acl_kind(kind);
        self.edit(|config| {
            if config.acl.chains.len() < MAX_CHAINS {
                config.acl.chains.push(AclChain::new(kind, name));
            }
        });
    }

    /// Asks whether to delete chain `index`, and does it when the answer is yes.
    fn ask_delete_chain(self, index: usize) {
        let Some((name, kind)) = self.chain(index, |chain| (chain.name.clone(), chain.kind)) else {
            return;
        };
        let strings = self.store.strings();
        let shown = if name.trim().is_empty() {
            strings.config.acl_kind(kind).to_owned()
        } else {
            name
        };
        let asked = confirm(
            fill(strings.config.acl_chain_delete_title, &[("name", &shown)]),
            strings.config.acl_chain_delete_body,
            strings.config.acl_chain_delete,
            strings.common.cancel,
            true,
        );
        spawn(async move {
            if asked.await {
                self.edit(|config| {
                    if index < config.acl.chains.len() {
                        config.acl.chains.remove(index);
                    }
                });
            }
        });
    }

    /// One chain: what it applies to, its name, switches, default action and rules.
    fn chain_card(self, index: usize) -> View {
        let store = self.store;
        let locked = self.locked;
        let kind = move || {
            self.chain(index, |chain| chain.kind)
                .unwrap_or(ChainKind::Inbound)
        };
        let chain_id = move |part: &str| self.id(&format!("chain-{index}-{part}"));
        let header = view()
            .flex_row()
            .items_center()
            .gap_3()
            .child(neutral_chip(move || self.words().acl_kind(kind())))
            .child(
                meta_text(move || self.words().acl_kind_help(kind()))
                    .flex_1()
                    .min_w_0(),
            )
            .child((!locked).then(|| {
                icon_button(
                    Icon::Trash,
                    store.tr(|s| s.config.acl_chain_delete),
                    move || self.ask_delete_chain(index),
                )
                .tone(IconTone::Danger)
                .test_id(chain_id("delete"))
            }));
        let kinds = ChainKind::ALL
            .into_iter()
            .map(|kind| SelectOption::new(ops::kind_id(kind), self.words().acl_kind(kind)))
            .collect::<Vec<_>>();
        card()
            .gap_4()
            .child(header)
            .child(
                field(
                    store.tr(|s| s.config.acl_chain_name),
                    move || {
                        self.chain(index, |chain| chain.name.clone())
                            .unwrap_or_default()
                    },
                    move |text| self.edit_chain(index, |chain| chain.name = text.to_owned()),
                )
                .disabled(locked)
                .test_id(chain_id("name")),
            )
            .child(
                field(
                    store.tr(|s| s.config.acl_chain_description),
                    move || {
                        self.chain(index, |chain| chain.description.clone())
                            .unwrap_or_default()
                    },
                    move |text| {
                        self.edit_chain(index, |chain| chain.description = text.to_owned());
                    },
                )
                .disabled(locked)
                .test_id(chain_id("description")),
            )
            .child(
                select(
                    store.tr(|s| s.config.acl_chain_kind),
                    kinds,
                    move || SharedString::from(ops::kind_id(kind())),
                    move |id| {
                        if let Some(next) = ops::kind_from_id(id) {
                            self.edit_chain(index, |chain| chain.kind = next);
                        }
                    },
                )
                .disabled(locked)
                .test_id(chain_id("kind")),
            )
            .child(
                switch_row(
                    store.tr(|s| s.config.acl_chain_enabled),
                    move || self.chain(index, |chain| chain.enabled).unwrap_or(false),
                    move |on| self.edit_chain(index, |chain| chain.enabled = on),
                )
                .disabled(move || locked)
                .test_id(chain_id("enabled")),
            )
            .child(self.default_action(index))
            .child(when(
                move || {
                    self.chain(index, |chain| {
                        chain.enabled && chain.default_action == AclAction::Drop
                    })
                    .unwrap_or(false)
                },
                move || {
                    callout(
                        CalloutKind::Warning,
                        store.tr(|s| s.config.acl_chain_drops_all),
                    )
                    .test_id(chain_id("drops-all"))
                },
            ))
            .child(self.rules_block(index))
    }

    /// What happens when no rule of chain `index` matches: a choice, or the plain word while read-only.
    fn default_action(self, index: usize) -> AnyElement {
        let store = self.store;
        let action = move || {
            self.chain(index, |chain| chain.default_action)
                .unwrap_or(AclAction::Allow)
        };
        if self.locked {
            return view()
                .gap_1()
                .child(section_label(store.tr(|s| s.config.acl_chain_default)))
                .child(body_text(move || action_text(self.words(), action())))
                .into_element();
        }
        let words = self.words();
        segmented(
            store.tr(|s| s.config.acl_chain_default),
            [AclAction::Allow, AclAction::Drop]
                .into_iter()
                .map(|action| SelectOption::new(ops::action_id(action), action_text(words, action)))
                .collect(),
            move || SharedString::from(ops::action_id(action())),
            move |id| {
                if let Some(next) = ops::action_from_id(id) {
                    self.edit_chain(index, |chain| chain.default_action = next);
                }
            },
            &self.id(&format!("chain-{index}-default")),
        )
        .into_element()
    }

    // ----- Rules of a chain -----

    /// The rules of chain `index`, highest priority first, and the button that adds one.
    fn rules_block(self, index: usize) -> View {
        let store = self.store;
        let rows = computed(move || self.chain(index, ops::rule_rows).unwrap_or_default());
        view()
            .gap_2()
            .child(section_label(store.tr(|s| s.config.acl_rules)).role(AccessibilityRole::Header))
            .child(dynamic(move || {
                let list = rows.get();
                let words = self.words();
                if list.is_empty() {
                    return meta_text(words.acl_rules_empty).into_element();
                }
                let mut rules = group();
                for (place, row) in list.iter().enumerate() {
                    rules = rules.child(self.rule_row(index, row, place == 0));
                }
                view()
                    .gap_2()
                    .child(rules)
                    .child(meta_text(words.acl_rules_order))
                    .into_element()
            }))
            .child(when(
                move || {
                    !self.locked
                        && self
                            .chain(index, |chain| chain.rules.len() < MAX_RULES)
                            .unwrap_or(false)
                },
                move || {
                    view().flex_row().child(tag(
                        action_button(
                            ButtonKind::Secondary,
                            store.tr(|s| s.config.acl_rule_add),
                            move || self.open_rule(index, None),
                        ),
                        &self.id(&format!("chain-{index}-add-rule")),
                    ))
                },
            ))
    }

    /// A rule of chain `chain`: its name, action and priority, what it matches, and its buttons.
    fn rule_row(self, chain: usize, row: &RuleRow, first: bool) -> View {
        let store = self.store;
        let words = self.words();
        let index = row.index;
        let title = if row.name.is_empty() {
            fill(
                words.acl_rule_untitled,
                &[("n", &index.saturating_add(1).to_string())],
            )
        } else {
            row.name.clone()
        };
        let mut heading = view()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(body_text(title).font_weight(FontWeight::MEDIUM))
            .child(neutral_chip(action_text(words, row.action)))
            .child(neutral_chip(fill(
                words.acl_rule_priority_chip,
                &[("n", &row.priority.to_string())],
            )));
        if !row.enabled {
            heading = heading.child(neutral_chip(words.acl_rule_disabled));
        }
        let rule_id = format!("rule-{chain}-{index}");
        group_row(!first)
            .test_id(&self.id(&rule_id))
            .child(
                view()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .py_1()
                    .child(heading)
                    .child(machine_text(row.summary.clone())),
            )
            .child((!self.locked).then(|| {
                view()
                    .flex_row()
                    .items_center()
                    .flex_shrink_0()
                    .child(
                        icon_button(
                            Icon::Edit,
                            store.tr(|s| s.config.acl_rule_edit),
                            move || self.open_rule(chain, Some(index)),
                        )
                        .test_id(self.id(&format!("{rule_id}-edit"))),
                    )
                    .child(
                        icon_button(
                            Icon::Trash,
                            store.tr(|s| s.config.acl_rule_delete),
                            move || self.delete_rule(chain, index),
                        )
                        .tone(IconTone::Danger)
                        .test_id(self.id(&format!("{rule_id}-delete"))),
                    )
            }))
    }

    /// Deletes rule `index` of chain `chain`.
    fn delete_rule(self, chain: usize, index: usize) {
        self.edit_chain(chain, |chain| {
            if index < chain.rules.len() {
                chain.rules.remove(index);
            }
        });
    }
}
