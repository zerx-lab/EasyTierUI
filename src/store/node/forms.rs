//! The dialogs' operations: the create and join flows, the network dialog (editor, logs, export), import and managed
//! enrollment. Pages change a draft through the `edit_*` functions and press the operations; nothing here is shown.
//!
//! Saving writes the secrets to the keychain first (off the engine thread), then the profile to the app's storage.

use nsh_core::{picker::PickRequest, prelude::*, time::unix_ms};

use super::{
    DetailTab, EditorDraft, ExportFailure, ExportView, Field, FieldProblem, FlowDraft, FlowKind,
    FlowStep, ImportDraft, ImportFailure, ImportReason, ImportStage, LogView, ManagedDraft,
    NodeStore, ProblemKind, SaveFailure, Status, parse_import,
};
use crate::{
    core::{acl::GroupSecrets, secret::Secret, work::off_thread},
    profile::{
        self, Imported, MAX_NAME_CHARS, NetworkProfile, NshStorage, ProfileError, ProfileId,
        RepoError, clean_name, repo,
    },
};

/// A storage failure in words a page can pick a string for.
pub(super) fn save_failure(error: RepoError) -> SaveFailure {
    match error {
        RepoError::Damaged => SaveFailure::Damaged,
        RepoError::NewerSchema { .. } => SaveFailure::NewerSchema,
        RepoError::StorageFull => SaveFailure::StorageFull,
        RepoError::Keychain(_) | RepoError::GroupSecrets => SaveFailure::Keychain,
        RepoError::Profile(ProfileError::NoRandomSource) => SaveFailure::NoRandomSource,
        RepoError::Profile(profile) => SaveFailure::Invalid(FieldProblem::from(&profile)),
        RepoError::Serialize => {
            SaveFailure::Invalid(FieldProblem::of(Field::Other, ProblemKind::Other))
        }
    }
}

/// Shows `imported` in the import dialog's preview.
fn show_preview(draft: &mut ImportDraft, imported: Imported) {
    draft.name = imported.profile.name.clone();
    draft.secret = imported.secret.clone().unwrap_or_default();
    draft.imported = Some(imported);
    draft.stage = ImportStage::Preview;
    draft.failure = None;
    draft.save_failure = None;
    draft.show_errors = false;
}

impl NodeStore {
    /// Stores `profile` (replacing the one with its id) and, if given, its secret and its ACL group secrets: the
    /// keychain first, off the engine thread, then the app's storage. An empty `groups` deletes the stored entry.
    pub(super) async fn persist(
        self,
        profile: NetworkProfile,
        secret: Option<Secret>,
        groups: Option<GroupSecrets>,
    ) -> Result<(), SaveFailure> {
        if !self.alive() {
            return Err(SaveFailure::Background);
        }
        if let Some(groups) = groups {
            let secrets = self.secrets.get();
            let for_groups = profile.clone();
            match off_thread(move || {
                repo::save_group_secrets(secrets.as_ref(), &for_groups, &groups)
            })
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    log::warn!("the ACL group secrets were not kept: {error}");
                    return Err(SaveFailure::Keychain);
                }
                Err(error) => {
                    log::warn!("keeping the ACL group secrets was lost: {error}");
                    return Err(SaveFailure::Background);
                }
            }
        }
        if let Some(secret) = secret {
            let secrets = self.secrets.get();
            let for_secret = profile.clone();
            match off_thread(move || repo::save_secret(secrets.as_ref(), &for_secret, &secret))
                .await
            {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    log::warn!("the secret was not kept: {error}");
                    return Err(SaveFailure::Keychain);
                }
                Err(error) => {
                    log::warn!("keeping the secret was lost: {error}");
                    return Err(SaveFailure::Background);
                }
            }
        }
        if !self.alive() {
            return Err(SaveFailure::Background);
        }
        repo::save_profile(&NshStorage, &profile).map_err(save_failure)?;
        self.upsert(profile);
        Ok(())
    }

    /// Adds or replaces `profile` in the list.
    fn upsert(&self, profile: NetworkProfile) {
        if !self.alive() {
            return;
        }
        let id = profile.id.clone();
        batch(|| {
            self.profiles
                .update(|list| match list.iter_mut().find(|p| p.id == id) {
                    Some(existing) => *existing = profile,
                    None => list.push(profile),
                });
            self.runtime.update(|map| {
                map.entry(id).or_default();
            });
        });
    }

    /// Closes every dialog of the node role.
    pub fn close_dialogs(&self) {
        if !self.alive() {
            return;
        }
        batch(|| {
            self.flow.set(None);
            self.editor.set(None);
            self.import.set(None);
            self.managed.set(None);
            self.detail.set(None);
            self.peer_open.set(None);
            self.tun_confirm.set(None);
        });
    }

    // ----- create and join -----

    /// Opens the create or join flow on its first screen.
    pub fn open_flow(&self, kind: FlowKind) {
        if !self.alive() {
            return;
        }
        self.close_dialogs();
        self.flow.set(Some(FlowDraft::new(kind)));
    }

    /// Closes the flow without saving.
    pub fn close_flow(&self) {
        if self.alive() {
            self.flow.set(None);
        }
    }

    /// Changes the flow's draft.
    pub fn edit_flow(&self, change: impl FnOnce(&mut FlowDraft)) {
        if !self.alive() {
            return;
        }
        self.flow.update(|draft| {
            if let Some(draft) = draft {
                change(draft);
            }
        });
    }

    /// Fills the flow's secret with a fresh random one.
    pub fn flow_generate_secret(&self) {
        match Secret::generate() {
            Ok(secret) => self.edit_flow(|draft| draft.secret = secret),
            Err(error) => log::warn!("no secret generated: {error}"),
        }
    }

    /// Next screen, or the problems of this one.
    pub fn flow_next(&self) {
        let Some(draft) = self.flow.get_untracked() else {
            return;
        };
        if !draft.problems_of(draft.step).is_empty() {
            self.edit_flow(|draft| draft.show_errors = true);
            return;
        }
        let Some(next) = draft.next_step() else {
            return;
        };
        // A created network starts with a generated secret and its name, which the person may change.
        let fresh_secret = if next == FlowStep::Network && draft.kind == FlowKind::Create {
            Secret::generate()
                .map_err(|error| log::warn!("no secret generated: {error}"))
                .ok()
        } else {
            None
        };
        self.edit_flow(|draft| {
            draft.step = next;
            draft.show_errors = false;
            if next == FlowStep::Network && draft.kind == FlowKind::Create {
                if draft.network_name.trim().is_empty() {
                    draft.network_name = clean_name(&draft.name, MAX_NAME_CHARS);
                }
                if draft.secret.is_empty()
                    && let Some(secret) = fresh_secret
                {
                    draft.secret = secret;
                }
            }
        });
    }

    /// The screen before this one, or close the flow from the first.
    pub fn flow_back(&self) {
        let Some(draft) = self.flow.get_untracked() else {
            return;
        };
        match draft.previous_step() {
            Some(previous) => self.edit_flow(|draft| {
                draft.step = previous;
                draft.show_errors = false;
            }),
            None => self.close_flow(),
        }
    }

    /// Saves the network the flow describes; with `start`, starts it (asking first if it needs the administrator).
    pub fn flow_submit(&self, start: bool) {
        let Some(draft) = self.flow.get_untracked() else {
            return;
        };
        if draft.saving {
            return;
        }
        if let Some(step) = draft.first_step_with_problem() {
            self.edit_flow(|draft| {
                draft.step = step;
                draft.show_errors = true;
            });
            return;
        }
        let id = match ProfileId::generate() {
            Ok(id) => id,
            Err(error) => {
                log::warn!("no profile id: {error}");
                self.edit_flow(|draft| draft.failure = Some(SaveFailure::NoRandomSource));
                return;
            }
        };
        let mut profile =
            NetworkProfile::new(id.clone(), draft.display_name(), draft.network_name.trim());
        profile.tun = self.tun_supported();
        if draft.kind == FlowKind::Join {
            match draft.peer_url() {
                Ok(peer) => profile.peers.push(peer),
                Err(problem) => {
                    self.edit_flow(|draft| draft.failure = Some(SaveFailure::Invalid(problem)));
                    return;
                }
            }
        }
        if let Err(error) = profile.validate() {
            let problem = FieldProblem::from(&error);
            self.edit_flow(|draft| draft.failure = Some(SaveFailure::Invalid(problem)));
            return;
        }
        self.edit_flow(|draft| {
            draft.saving = true;
            draft.failure = None;
        });
        let secret = draft.secret.clone();
        let this = *self;
        spawn(async move {
            match this.persist(profile, Some(secret), None).await {
                Ok(()) => {
                    if this.alive() {
                        this.flow.set(None);
                    }
                    if start {
                        this.start(id);
                    }
                }
                Err(failure) => this.edit_flow(|draft| {
                    draft.saving = false;
                    draft.failure = Some(failure);
                }),
            }
        });
    }

    // ----- the network drawer -----

    /// Opens network `id`'s drawer on `tab`.
    pub fn open_detail(&self, id: ProfileId, tab: DetailTab) {
        if !self.alive() {
            return;
        }
        let Some(profile) = self.profile_untracked(&id) else {
            return;
        };
        self.close_dialogs();
        batch(|| {
            self.editor.set(Some(EditorDraft::from_profile(&profile)));
            self.detail.set(Some(id.clone()));
            self.tab.set(tab);
            self.logs.set(LogView::default());
            self.export.set(ExportView::default());
            self.logs_follow.set(false);
        });
        if !profile.is_managed() {
            self.load_editor_secret(profile);
        }
        self.load_for_tab(&id, tab);
    }

    /// Closes the network drawer; unsaved edits are dropped.
    pub fn close_detail(&self) {
        if !self.alive() {
            return;
        }
        batch(|| {
            self.detail.set(None);
            self.editor.set(None);
            self.logs_follow.set(false);
        });
    }

    /// Shows `tab` of the open network drawer.
    pub fn set_tab(&self, tab: DetailTab) {
        if !self.alive() {
            return;
        }
        self.tab.set(tab);
        if let Some(id) = self.detail.get_untracked() {
            self.load_for_tab(&id, tab);
        }
    }

    fn load_for_tab(&self, id: &ProfileId, tab: DetailTab) {
        match tab {
            DetailTab::Logs => self.load_logs(id.clone()),
            DetailTab::Export => self.refresh_export(id.clone()),
            DetailTab::Overview | DetailTab::Settings => {}
        }
    }

    /// Changes the editor's draft.
    pub fn edit_editor(&self, change: impl FnOnce(&mut EditorDraft)) {
        if !self.alive() {
            return;
        }
        self.editor.update(|draft| {
            if let Some(draft) = draft {
                change(draft);
            }
        });
    }

    /// Changes the editor's draft if it is network `id`'s.
    fn edit_editor_of(&self, id: &ProfileId, change: impl FnOnce(&mut EditorDraft)) {
        let same = self
            .editor
            .with_untracked(|draft| draft.as_ref().is_some_and(|draft| draft.id() == id));
        if same {
            self.edit_editor(change);
        }
    }

    /// Reads the saved secret and ACL group secrets into the editor (off the engine thread).
    fn load_editor_secret(&self, profile: NetworkProfile) {
        let secrets = self.secrets.get();
        let id = profile.id.clone();
        let this = *self;
        spawn(async move {
            let answer = off_thread(move || {
                let secret = repo::load_secret(secrets.as_ref(), &profile)?;
                let groups = repo::load_group_secrets(secrets.as_ref(), &profile)?;
                Ok::<_, RepoError>((secret, groups))
            })
            .await;
            if !this.alive() {
                return;
            }
            this.edit_editor_of(&id, |draft| {
                draft.secret_loaded = true;
                match answer {
                    Ok(Ok((secret, groups))) => {
                        match secret {
                            Some(secret) => {
                                if !draft.secret_changed {
                                    draft.secret = secret;
                                }
                            }
                            None => draft.secret_missing = true,
                        }
                        draft.apply_group_secrets(groups);
                    }
                    Ok(Err(error)) => {
                        log::warn!("the keychain would not give the secrets: {error}");
                        draft.failure = Some(SaveFailure::Keychain);
                    }
                    Err(error) => log::warn!("reading the secrets was lost: {error}"),
                }
            });
        });
    }

    /// Puts a fresh random secret in the editor.
    pub fn editor_generate_secret(&self) {
        match Secret::generate() {
            Ok(secret) => self.edit_editor(|draft| {
                draft.secret = secret;
                draft.secret_loaded = true;
                draft.secret_changed = true;
            }),
            Err(error) => log::warn!("no secret generated: {error}"),
        }
    }

    /// Saves the editor's draft; a running network restarts with it.
    pub fn editor_save(&self) {
        let Some(draft) = self.editor.get_untracked() else {
            return;
        };
        if draft.saving {
            return;
        }
        let Ok(profile) = draft.build() else {
            self.edit_editor(|draft| draft.show_errors = true);
            return;
        };
        let secret = (draft.secret_changed && !draft.is_managed()).then(|| draft.secret.clone());
        let groups = draft.group_secrets_to_save();
        let id = profile.id.clone();
        let was_up = matches!(
            self.status_untracked(&id),
            Status::Running | Status::Starting
        );
        self.edit_editor(|draft| {
            draft.saving = true;
            draft.failure = None;
        });
        let this = *self;
        spawn(async move {
            match this.persist(profile, secret, groups).await {
                Ok(()) => {
                    this.close_detail();
                    if was_up {
                        this.restart(id);
                    } else {
                        // The old failure was about the old settings.
                        this.change(&id, |runtime| {
                            if runtime.state == crate::core::state::NetworkState::Stopped {
                                runtime.failure = None;
                            }
                        });
                    }
                }
                Err(failure) => this.edit_editor(|draft| {
                    draft.saving = false;
                    draft.failure = Some(failure);
                }),
            }
        });
    }

    // ----- logs and export -----

    /// Reads network `id`'s recent output into [`NodeStore::logs`].
    pub fn load_logs(&self, id: ProfileId) {
        if !self.alive() {
            return;
        }
        let Some(backend) = self.backend.get() else {
            self.logs.set(LogView {
                id: Some(id),
                lines: Vec::new(),
                loading: false,
                read_at_ms: Some(unix_ms()),
            });
            return;
        };
        let Ok(key) = id.instance_key() else {
            return;
        };
        self.logs.update(|view| {
            if view.id.as_ref() != Some(&id) {
                *view = LogView {
                    id: Some(id.clone()),
                    ..LogView::default()
                };
            }
            view.loading = true;
        });
        let this = *self;
        spawn(async move {
            let answer = off_thread(move || backend.logs(&key)).await;
            if !this.alive() {
                return;
            }
            this.logs.update(|view| {
                if view.id.as_ref() != Some(&id) {
                    return;
                }
                view.loading = false;
                match answer {
                    Ok(lines) => {
                        view.lines = lines;
                        view.read_at_ms = Some(unix_ms());
                    }
                    Err(error) => log::warn!("reading the logs was lost: {error}"),
                }
            });
        });
    }

    /// Makes the export of network `id`'s saved profile, with its secrets if [`ExportView::include_secret`].
    pub fn refresh_export(&self, id: ProfileId) {
        if !self.alive() {
            return;
        }
        let Some(profile) = self.profile_untracked(&id) else {
            return;
        };
        let include = self
            .export
            .with_untracked(|view| view.include_secret && view.id.as_ref() == Some(&id));
        self.export.set(ExportView {
            id: Some(id.clone()),
            include_secret: include,
            loading: true,
            ..ExportView::default()
        });
        if !include {
            self.finish_export(&profile, None, GroupSecrets::default(), include);
            return;
        }
        let secrets = self.secrets.get();
        let this = *self;
        let for_secret = profile.clone();
        spawn(async move {
            let answer = off_thread(move || {
                let secret = repo::load_secret(secrets.as_ref(), &for_secret)?;
                let groups = repo::load_group_secrets(secrets.as_ref(), &for_secret)?;
                Ok::<_, RepoError>((secret, groups))
            })
            .await;
            if !this.alive() {
                return;
            }
            match answer {
                Ok(Ok((secret, groups))) => {
                    this.finish_export(&profile, Some(secret), groups, include);
                }
                Ok(Err(error)) => {
                    log::warn!("the keychain would not give the secrets: {error}");
                    this.fail_export(&profile.id, ExportFailure::Keychain);
                }
                Err(error) => {
                    log::warn!("reading the secrets was lost: {error}");
                    this.fail_export(&profile.id, ExportFailure::Keychain);
                }
            }
        });
    }

    fn fail_export(&self, id: &ProfileId, failure: ExportFailure) {
        self.export.update(|view| {
            if view.id.as_ref() == Some(id) {
                view.loading = false;
                view.failure = Some(failure);
            }
        });
    }

    /// Renders the export. `secret` is `None` when it was not asked for, `Some(None)` when it was and the keychain
    /// has none.
    fn finish_export(
        &self,
        profile: &NetworkProfile,
        secret: Option<Option<Secret>>,
        groups: GroupSecrets,
        include: bool,
    ) {
        let stale = self
            .export
            .with_untracked(|view| view.id.as_ref() != Some(&profile.id));
        if stale {
            return;
        }
        let secret_missing = include && matches!(secret, Some(None));
        let secret = secret.flatten().unwrap_or_default();
        // Group secrets that are not all there cannot be exported: the export then leaves every secret out and says so.
        let groups_incomplete = include
            && profile.acl.as_ref().is_some_and(|acl| {
                acl.declared_groups
                    .iter()
                    .any(|name| groups.get(name).is_none_or(Secret::is_empty))
            });
        let with_secrets = include && !groups_incomplete;
        let (used, groups) = if with_secrets {
            (secret, groups)
        } else {
            (Secret::default(), GroupSecrets::default())
        };
        let rendered = profile.render_with(&used, &groups, with_secrets);
        let link = profile::share_link(profile, Some(&used), &groups, with_secrets);
        let (Ok(toml), Ok(link)) = (rendered, link) else {
            self.fail_export(&profile.id, ExportFailure::Invalid);
            return;
        };
        let (qr, failure) = match profile::qr_matrix(link.expose()) {
            Ok(matrix) => {
                let size = matrix.size();
                let mut modules = Vec::with_capacity(size.saturating_mul(size));
                for y in 0..size {
                    for x in 0..size {
                        modules.push(matrix.is_dark(x, y));
                    }
                }
                (Some((size, modules)), None)
            }
            Err(_) => (None, Some(ExportFailure::QrTooLong)),
        };
        self.export.update(|view| {
            view.loading = false;
            view.toml = Some(toml);
            view.link = Some(link);
            view.qr = qr;
            view.failure = failure;
            view.secret_missing = secret_missing || groups_incomplete;
        });
    }

    /// Includes (or leaves out) the network secret in the export and makes it again.
    pub fn set_include_secret(&self, include: bool) {
        if !self.alive() {
            return;
        }
        self.export.update(|view| view.include_secret = include);
        if let Some(id) = self.detail.get_untracked() {
            self.refresh_export(id);
        }
    }

    // ----- import -----

    /// Opens the import dialog.
    pub fn open_import(&self) {
        if !self.alive() {
            return;
        }
        self.close_dialogs();
        self.import.set(Some(ImportDraft::new()));
    }

    /// Closes the import dialog without saving.
    pub fn close_import(&self) {
        if self.alive() {
            self.import.set(None);
        }
    }

    /// Changes the import dialog's draft.
    pub fn edit_import(&self, change: impl FnOnce(&mut ImportDraft)) {
        if !self.alive() {
            return;
        }
        self.import.update(|draft| {
            if let Some(draft) = draft {
                change(draft);
            }
        });
    }

    /// Reads the pasted text (TOML or a share link) and shows what importing it would do.
    pub fn import_parse(&self) {
        let Some(draft) = self.import.get_untracked() else {
            return;
        };
        match parse_import(&draft.input) {
            Ok(imported) => self.edit_import(|draft| show_preview(draft, imported)),
            Err(failure) => self.edit_import(|draft| draft.failure = Some(failure)),
        }
    }

    /// Lets the person pick an image of a QR code, reads it off the engine thread and shows the preview.
    pub fn import_pick_image(&self) {
        let Some(draft) = self.import.get_untracked() else {
            return;
        };
        if draft.reading {
            return;
        }
        self.edit_import(|draft| {
            draft.reading = true;
            draft.failure = None;
        });
        let picked = pick_images(PickRequest::new(false, 1));
        let this = *self;
        spawn(async move {
            let files = picked.await;
            let Some(file) = files.into_iter().next() else {
                // Cancelled.
                this.edit_import(|draft| draft.reading = false);
                return;
            };
            let bytes = file.data.bytes().to_vec();
            let answer = off_thread(move || profile::import_qr_image(&bytes)).await;
            this.edit_import(|draft| {
                draft.reading = false;
                match answer {
                    Ok(Ok(imported)) => show_preview(draft, imported),
                    Ok(Err(error)) => draft.failure = Some(ImportFailure::from(&error)),
                    Err(error) => {
                        log::warn!("reading the image was lost: {error}");
                        draft.failure = Some(ImportFailure::new(ImportReason::PickFailed));
                    }
                }
            });
        });
    }

    /// Back from the preview to the input.
    pub fn import_back(&self) {
        self.edit_import(|draft| {
            draft.stage = ImportStage::Input;
            draft.imported = None;
            draft.save_failure = None;
        });
    }

    /// Saves the previewed import as a new network. It never starts.
    pub fn import_save(&self) {
        let Some(draft) = self.import.get_untracked() else {
            return;
        };
        if draft.saving {
            return;
        }
        let Some(imported) = draft.imported.clone() else {
            return;
        };
        if !draft.problems().is_empty() {
            self.edit_import(|draft| draft.show_errors = true);
            return;
        }
        let mut profile = imported.profile;
        profile.name = draft.name.trim().to_owned();
        if !self.tun_supported() {
            profile.tun = false;
        }
        let secret = (!draft.secret.is_empty()).then(|| draft.secret.clone());
        let groups = (!imported.group_secrets.is_empty()).then(|| imported.group_secrets.clone());
        self.edit_import(|draft| {
            draft.saving = true;
            draft.save_failure = None;
        });
        let this = *self;
        spawn(async move {
            match this.persist(profile, secret, groups).await {
                Ok(()) => this.close_import(),
                Err(failure) => this.edit_import(|draft| {
                    draft.saving = false;
                    draft.save_failure = Some(failure);
                }),
            }
        });
    }

    // ----- managed enrollment -----

    /// Opens the "join a control plane" dialog; `default_name` is what the network is called unless changed.
    pub fn open_managed(&self, default_name: &str) {
        if !self.alive() {
            return;
        }
        self.close_dialogs();
        self.managed.set(Some(ManagedDraft::new(default_name)));
    }

    /// Closes the dialog without enrolling.
    pub fn close_managed(&self) {
        if self.alive() {
            self.managed.set(None);
        }
    }

    /// Changes the dialog's draft.
    pub fn edit_managed(&self, change: impl FnOnce(&mut ManagedDraft)) {
        if !self.alive() {
            return;
        }
        self.managed.update(|draft| {
            if let Some(draft) = draft {
                change(draft);
            }
        });
    }

    /// Enrolls: saves the managed network and its control-plane address and starts it (as the user's own process).
    pub fn managed_submit(&self) {
        let Some(draft) = self.managed.get_untracked() else {
            return;
        };
        if draft.saving {
            return;
        }
        if !draft.problems().is_empty() {
            self.edit_managed(|draft| draft.show_errors = true);
            return;
        }
        let id = match ProfileId::generate() {
            Ok(id) => id,
            Err(error) => {
                log::warn!("no profile id: {error}");
                self.edit_managed(|draft| draft.failure = Some(SaveFailure::NoRandomSource));
                return;
            }
        };
        let profile = draft.profile(id.clone());
        let address = draft.config_server.clone();
        self.edit_managed(|draft| {
            draft.saving = true;
            draft.failure = None;
        });
        let this = *self;
        spawn(async move {
            match this.persist(profile, Some(address), None).await {
                Ok(()) => {
                    if this.alive() {
                        this.managed.set(None);
                    }
                    this.start(id);
                }
                Err(failure) => this.edit_managed(|draft| {
                    draft.saving = false;
                    draft.failure = Some(failure);
                }),
            }
        });
    }
}
