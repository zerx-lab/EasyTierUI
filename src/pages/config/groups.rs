//! Groups: the ones this node declares (a name and a secret, edited in a dialog so renaming and duplicate names are
//! settled on Save, never mid-typing) and the ones it belongs to.

use nsh_core::prelude::*;

use super::{
    acl::{Ctx, Sink, Source, tag},
    ops,
    text::problem_text,
};
use crate::{
    components::{
        ButtonKind, CopyLabels, DialogFrame, HintLevel, Icon, IconTone, SecretLabels,
        action_button, body_text, confirm, dialog_footer, dialog_frame, field, group, group_row,
        icon_button, meta_text, secret_field, section_label, switch_row,
    },
    core::{
        acl::{AclProblem, MAX_ENTRIES},
        secret::{Secret, Strength},
    },
    i18n::fill,
};

/// Which group the dialog is about: the declared group at `index`, or a new one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    index: Option<usize>,
}

/// The dialog's state. A handle of signals, created once with the editor.
#[derive(Clone, Copy)]
pub(super) struct Ui {
    slot: Signal<Option<Slot>>,
    name: Signal<String>,
    secret: Signal<String>,
    /// Whether saving was tried: the dialog then says what is missing.
    submitted: Signal<bool>,
    /// Whether the last press of Generate found no secure random source.
    generate_failed: Signal<bool>,
}

impl Ui {
    /// A closed dialog.
    pub(super) fn new() -> Self {
        Self {
            slot: signal(None),
            name: signal(String::new()),
            secret: signal(String::new()),
            submitted: signal(false),
            generate_failed: signal(false),
        }
    }
}

/// A group in the membership list.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Member {
    name: String,
    /// Whether this node declares the group (a membership may name a group declared elsewhere).
    declared: bool,
}

impl<G: Source, S: Sink> Ctx<G, S> {
    /// The groups section: declared groups and memberships.
    pub(super) fn groups(self) -> View {
        let store = self.store;
        view()
            .gap_4()
            .child(
                view()
                    .gap_1()
                    .child(
                        section_label(store.tr(|s| s.config.acl_groups))
                            .role(AccessibilityRole::Header),
                    )
                    .child(meta_text(store.tr(|s| s.config.acl_groups_help))),
            )
            .child(self.declared_list())
            .child(self.joined_list())
    }

    // ----- Declared groups -----

    /// The declared groups with their buttons.
    fn declared_list(self) -> View {
        let store = self.store;
        let rows = computed(move || {
            self.config()
                .map(|config| {
                    config
                        .acl
                        .declared_groups
                        .iter()
                        .map(|name| {
                            let has_secret = config
                                .group_secrets
                                .get(name)
                                .is_some_and(|secret| !secret.is_empty());
                            (name.clone(), has_secret)
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        });
        view()
            .gap_2()
            .child(section_label(store.tr(|s| s.config.acl_declared)))
            .child(meta_text(store.tr(|s| s.config.acl_declared_help)))
            .child(dynamic(move || {
                let list = rows.get();
                let words = self.words();
                if list.is_empty() {
                    return meta_text(words.acl_groups_empty).into_element();
                }
                let mut groups = group();
                for (index, (name, has_secret)) in list.iter().enumerate() {
                    groups = groups.child(self.declared_row(index, name, *has_secret));
                }
                groups.into_element()
            }))
            .child(when(
                move || !self.locked && rows.get().len() < MAX_ENTRIES,
                move || {
                    view().flex_row().child(tag(
                        action_button(
                            ButtonKind::Secondary,
                            store.tr(|s| s.config.acl_group_add),
                            move || self.open_group(None),
                        ),
                        &self.id("group-add"),
                    ))
                },
            ))
    }

    /// A declared group: its name, whether it has a secret, and its buttons.
    fn declared_row(self, index: usize, name: &str, has_secret: bool) -> View {
        let store = self.store;
        let words = self.words();
        let row_id = format!("group-{index}");
        group_row(index > 0)
            .test_id(&self.id(&row_id))
            .child(body_text(name.to_owned()).flex_1().min_w_0())
            .child(meta_text(if has_secret {
                words.acl_group_secret_set
            } else {
                words.acl_group_secret_missing
            }))
            .child((!self.locked).then(|| {
                view()
                    .flex_row()
                    .items_center()
                    .flex_shrink_0()
                    .child(
                        icon_button(
                            Icon::Edit,
                            store.tr(|s| s.config.acl_group_edit),
                            move || self.open_group(Some(index)),
                        )
                        .test_id(self.id(&format!("{row_id}-edit"))),
                    )
                    .child(
                        icon_button(
                            Icon::Trash,
                            store.tr(|s| s.config.acl_group_delete),
                            move || self.ask_delete_group(index),
                        )
                        .tone(IconTone::Danger)
                        .test_id(self.id(&format!("{row_id}-delete"))),
                    )
            }))
    }

    /// Asks whether to delete the declared group at `index` (naming what else changes), and does it when the answer
    /// is yes.
    fn ask_delete_group(self, index: usize) {
        let Some(config) = self.config() else {
            return;
        };
        let Some(name) = config.acl.declared_groups.get(index).cloned() else {
            return;
        };
        let count = ops::rules_using(&config, &name).to_string();
        let strings = self.store.strings();
        let asked = confirm(
            fill(strings.config.acl_group_delete_title, &[("name", &name)]),
            fill(strings.config.acl_group_delete_body, &[("count", &count)]),
            strings.config.acl_group_delete,
            strings.common.cancel,
            true,
        );
        spawn(async move {
            if asked.await {
                self.edit(|config| ops::remove_group(config, &name));
            }
        });
    }

    // ----- Memberships -----

    /// The groups this node belongs to: a switch per declared group, and the memberships of groups it does not
    /// declare, which can be left.
    fn joined_list(self) -> View {
        let store = self.store;
        let rows = computed(move || {
            let Some(config) = self.config() else {
                return Vec::new();
            };
            let mut members: Vec<Member> = config
                .acl
                .declared_groups
                .iter()
                .map(|name| Member {
                    name: name.clone(),
                    declared: true,
                })
                .collect();
            for name in &config.acl.joined_groups {
                if !config.acl.declared_groups.contains(name) {
                    members.push(Member {
                        name: name.clone(),
                        declared: false,
                    });
                }
            }
            members
        });
        view()
            .gap_2()
            .child(section_label(store.tr(|s| s.config.acl_joined)))
            .child(meta_text(store.tr(|s| s.config.acl_joined_help)))
            .child(dynamic(move || {
                let list = rows.get();
                if list.is_empty() {
                    return meta_text(self.words().acl_joined_none).into_element();
                }
                let mut members = view().gap_2();
                for (index, member) in list.iter().enumerate() {
                    members = members.child(self.member_row(index, member));
                }
                members.into_element()
            }))
    }

    /// Whether this node belongs to the group `name`.
    fn is_joined(self, name: &str) -> bool {
        self.config()
            .is_some_and(|config| config.acl.joined_groups.iter().any(|group| group == name))
    }

    /// A membership: a switch for a declared group, a note and a way out for any other.
    fn member_row(self, index: usize, member: &Member) -> AnyElement {
        let store = self.store;
        if member.declared {
            let shown = member.name.clone();
            let checked = member.name.clone();
            let changed = member.name.clone();
            let locked = self.locked;
            return switch_row(
                move || shown.clone(),
                move || self.is_joined(&checked),
                move |on| self.edit(|config| ops::set_joined(config, &changed, on)),
            )
            .disabled(move || locked)
            .test_id(self.id(&format!("joined-{index}")))
            .into_element();
        }
        let name = member.name.clone();
        let words = self.words();
        group_row(false)
            .child(
                view()
                    .flex_1()
                    .min_w_0()
                    .child(body_text(member.name.clone()))
                    .child(meta_text(words.acl_joined_undeclared)),
            )
            .child((!self.locked).then(|| {
                icon_button(
                    Icon::Close,
                    store.tr(|s| s.config.acl_joined_remove),
                    move || self.edit(|config| ops::set_joined(config, &name, false)),
                )
                .test_id(self.id(&format!("joined-remove-{index}")))
            }))
            .into_element()
    }

    // ----- The dialog -----

    /// Opens the dialog on the declared group at `index` (`None`: a new group, with a generated secret).
    pub(super) fn open_group(self, index: Option<usize>) {
        let ui = self.group_ui;
        let (name, secret) = match index {
            Some(index) => {
                let Some(config) = self.config() else {
                    return;
                };
                let Some(name) = config.acl.declared_groups.get(index).cloned() else {
                    return;
                };
                let secret = config
                    .group_secrets
                    .get(&name)
                    .map(|secret| secret.expose().to_owned())
                    .unwrap_or_default();
                (name, secret)
            }
            None => (
                String::new(),
                Secret::generate()
                    .map(|secret| secret.expose().to_owned())
                    .unwrap_or_default(),
            ),
        };
        batch(|| {
            ui.name.set(name);
            ui.secret.set(secret);
            ui.submitted.set(false);
            ui.generate_failed.set(false);
            ui.slot.set(Some(Slot { index }));
        });
    }

    /// The dialog, closed until [`Ctx::open_group`].
    pub(super) fn group_dialog(self) -> DialogFrame {
        let slot = self.group_ui.slot;
        dialog_frame(
            move || slot.get().is_some(),
            move || {
                let editing = slot.get().is_some_and(|slot| slot.index.is_some());
                let words = self.words();
                if editing {
                    words.acl_group_edit
                } else {
                    words.acl_group_add
                }
            },
            move || slot.set(None),
        )
        .body(move || untracked(|| self.group_form()))
        .footer(move || untracked(|| self.group_footer()))
        .test_id(self.id("group-dialog"))
    }

    /// What is wrong with the name and secret as typed.
    fn group_problem(self) -> Option<AclProblem> {
        let ui = self.group_ui;
        let slot = ui.slot.get()?;
        let config = self.config()?;
        let name = ui.name.get();
        let secret = Secret::new(ui.secret.get());
        ops::group_problem(&config, slot.index, name.trim(), &secret)
    }

    /// The name and the secret.
    fn group_form(self) -> View {
        let ui = self.group_ui;
        let store = self.store;
        let labels = SecretLabels::new(
            store.tr(|s| s.common.show),
            store.tr(|s| s.common.hide),
            CopyLabels::new(store.tr(|s| s.common.copy), store.tr(|s| s.common.copied)),
        );
        view()
            .gap_4()
            .child(
                field(
                    store.tr(|s| s.config.acl_group_name),
                    move || ui.name.get(),
                    move |text| ui.name.set(text.to_owned()),
                )
                .error(move || {
                    let problem = match self.group_problem()? {
                        AclProblem::MissingSecret => return None,
                        AclProblem::BadName
                            if !ui.submitted.get() && ui.name.get().trim().is_empty() =>
                        {
                            return None;
                        }
                        other => other,
                    };
                    Some(SharedString::from(problem_text(self.words(), problem)))
                })
                .test_id(self.id("group-name")),
            )
            .child(
                secret_field(
                    store.tr(|s| s.config.acl_group_secret),
                    move || ui.secret.get(),
                    move |text| {
                        batch(|| {
                            ui.secret.set(text.to_owned());
                            ui.generate_failed.set(false);
                        });
                    },
                    labels,
                )
                .generate(store.tr(|s| s.common.generate), move || {
                    self.generate_secret();
                })
                .hint(
                    move || {
                        let secret = ui.secret.get();
                        if secret.is_empty() {
                            return SharedString::default();
                        }
                        let common = self.store.strings().common;
                        SharedString::from(match Secret::new(secret).strength() {
                            Strength::Weak => common.strength_weak,
                            Strength::Fair => common.strength_fair,
                            Strength::Strong => common.strength_strong,
                        })
                    },
                    move || match Secret::new(ui.secret.get()).strength() {
                        Strength::Strong => HintLevel::Strong,
                        Strength::Fair => HintLevel::Fair,
                        Strength::Weak => HintLevel::Weak,
                    },
                )
                .error(move || {
                    if ui.generate_failed.get() {
                        return Some(SharedString::from(self.words().acl_group_generate_failed));
                    }
                    match self.group_problem() {
                        Some(AclProblem::MissingSecret) if ui.submitted.get() => {
                            Some(SharedString::from(problem_text(
                                self.words(),
                                AclProblem::MissingSecret,
                            )))
                        }
                        _ => None,
                    }
                })
                .test_id(self.id("group-secret")),
            )
            .child(meta_text(store.tr(|s| s.config.acl_group_secret_help)))
    }

    /// Puts a fresh random secret in the field, or says this device has no random source to make one from.
    fn generate_secret(self) {
        let ui = self.group_ui;
        match Secret::generate() {
            Ok(secret) => batch(|| {
                ui.secret.set(secret.expose().to_owned());
                ui.generate_failed.set(false);
            }),
            Err(_) => ui.generate_failed.set(true),
        }
    }

    /// Cancel and Save.
    fn group_footer(self) -> View {
        let store = self.store;
        let slot = self.group_ui.slot;
        dialog_footer()
            .child(tag(
                action_button(
                    ButtonKind::Ghost,
                    store.tr(|s| s.common.cancel),
                    move || slot.set(None),
                ),
                &self.id("group-cancel"),
            ))
            .child(tag(
                action_button(
                    ButtonKind::Primary,
                    store.tr(|s| s.common.save),
                    move || self.save_group(),
                ),
                &self.id("group-save"),
            ))
    }

    /// Declares the group (or applies the rename and the new secret) when the name and secret are fine; otherwise
    /// the dialog stays open and says what is wrong.
    fn save_group(self) {
        let ui = self.group_ui;
        let Some(slot) = ui.slot.get_untracked() else {
            return;
        };
        let Some(config) = self.config() else {
            return;
        };
        let name = ui.name.get_untracked().trim().to_owned();
        let secret = Secret::new(ui.secret.get_untracked());
        if ops::group_problem(&config, slot.index, &name, &secret).is_some() {
            ui.submitted.set(true);
            return;
        }
        self.edit(move |config| match slot.index {
            Some(index) => {
                if let Some(old) = config.acl.declared_groups.get(index).cloned() {
                    ops::rename_group(config, &old, &name);
                    config.group_secrets.insert(name, secret);
                }
            }
            None => {
                config.acl.declared_groups.push(name.clone());
                config.group_secrets.insert(name, secret);
            }
        });
        ui.slot.set(None);
    }
}
