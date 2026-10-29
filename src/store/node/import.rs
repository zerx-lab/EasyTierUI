//! Import (pasted TOML, a share link or a QR image) and managed enrollment: drafts and the parsing that turns
//! untrusted text into something to preview. Nothing here starts or stores anything.

use super::problem::{Field, FieldProblem, ImportFailure, ImportReason, ProblemKind, SaveFailure};
use crate::{
    core::secret::Secret,
    profile::{
        self, Imported, MAX_HOSTNAME_CHARS, MAX_NAME_CHARS, NetworkProfile, ProfileId,
        share::LINK_PREFIX,
    },
};

/// Where the import dialog is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportStage {
    /// Asking for the text or an image.
    Input,
    /// Showing what the import would do.
    Preview,
}

/// The import dialog's state.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportDraft {
    /// The pasted TOML or link (it may hold a secret).
    pub input: Secret,
    /// The screen shown.
    pub stage: ImportStage,
    /// The parsed import, once there is one.
    pub imported: Option<Imported>,
    /// The name the profile will get.
    pub name: String,
    /// The network secret, from the import or typed here.
    pub secret: Secret,
    /// Why the input was refused.
    pub failure: Option<ImportFailure>,
    /// A picked image is being read.
    pub reading: bool,
    /// The profile is being saved.
    pub saving: bool,
    /// Why the save failed.
    pub save_failure: Option<SaveFailure>,
    /// A refused save shows its field errors.
    pub show_errors: bool,
}

impl ImportDraft {
    /// An empty dialog.
    pub fn new() -> Self {
        Self {
            input: Secret::default(),
            stage: ImportStage::Input,
            imported: None,
            name: String::new(),
            secret: Secret::default(),
            failure: None,
            reading: false,
            saving: false,
            save_failure: None,
            show_errors: false,
        }
    }

    /// Problems with what would be saved (the name).
    pub fn problems(&self) -> Vec<FieldProblem> {
        let name = self.name.trim();
        if name.is_empty() {
            vec![FieldProblem::of(Field::Name, ProblemKind::Required)]
        } else if name.chars().any(char::is_control) {
            vec![FieldProblem::of(Field::Name, ProblemKind::ControlChars)]
        } else if name.chars().count() > MAX_NAME_CHARS {
            vec![FieldProblem::of(
                Field::Name,
                ProblemKind::TooLong {
                    max: MAX_NAME_CHARS,
                },
            )]
        } else {
            Vec::new()
        }
    }
}

impl Default for ImportDraft {
    fn default() -> Self {
        Self::new()
    }
}

/// Reads `text` as a share link or an EasyTier TOML config: a new profile and what it would do, not applied.
///
/// # Errors
///
/// Why the text is not an import, with the key or line when there is one.
pub fn parse_import(text: &Secret) -> Result<Imported, ImportFailure> {
    let trimmed = text.expose().trim();
    if trimmed.is_empty() {
        return Err(ImportFailure::new(ImportReason::Empty));
    }
    if trimmed.starts_with(LINK_PREFIX) {
        profile::parse_share_link(trimmed).map_err(|error| ImportFailure::from(&error))
    } else {
        profile::import_toml(trimmed).map_err(|error| ImportFailure::from(&error))
    }
}

/// The managed-enrollment dialog's state.
#[derive(Debug, Clone, PartialEq)]
pub struct ManagedDraft {
    /// The name shown in the app.
    pub name: String,
    /// The control plane's address, `udp://host:22020/account`: an enrollment credential, so a secret.
    pub config_server: Secret,
    /// The hostname to report (empty: the device's own).
    pub hostname: String,
    /// Empty or invalid fields show their errors.
    pub show_errors: bool,
    /// The profile is being saved.
    pub saving: bool,
    /// Why the save failed.
    pub failure: Option<SaveFailure>,
}

impl ManagedDraft {
    /// A dialog named `name` by default.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            config_server: Secret::default(),
            hostname: String::new(),
            show_errors: false,
            saving: false,
            failure: None,
        }
    }

    /// Every problem of the form.
    pub fn problems(&self) -> Vec<FieldProblem> {
        let mut found = Vec::new();
        let name = self.name.trim();
        if name.is_empty() {
            found.push(FieldProblem::of(Field::Name, ProblemKind::Required));
        } else if name.chars().count() > MAX_NAME_CHARS {
            found.push(FieldProblem::of(
                Field::Name,
                ProblemKind::TooLong {
                    max: MAX_NAME_CHARS,
                },
            ));
        }
        let hostname = self.hostname.trim();
        if hostname.chars().count() > MAX_HOSTNAME_CHARS {
            found.push(FieldProblem::of(
                Field::Hostname,
                ProblemKind::TooLong {
                    max: MAX_HOSTNAME_CHARS,
                },
            ));
        } else if hostname.chars().any(char::is_control) {
            found.push(FieldProblem::of(Field::Hostname, ProblemKind::ControlChars));
        }
        if self.config_server.is_empty() {
            found.push(FieldProblem::of(Field::ConfigServer, ProblemKind::Required));
        } else if profile::check_config_server(&self.config_server).is_err() {
            found.push(FieldProblem::of(
                Field::ConfigServer,
                ProblemKind::ConfigServerShape,
            ));
        }
        found
    }

    /// The problems to show now: empty required fields stay quiet until a save was refused.
    pub fn visible_problems(&self) -> Vec<FieldProblem> {
        let all = self.problems();
        if self.show_errors {
            all
        } else {
            all.into_iter()
                .filter(|problem| problem.kind != ProblemKind::Required)
                .collect()
        }
    }

    /// The managed profile this form makes, with `id`.
    pub fn profile(&self, id: ProfileId) -> NetworkProfile {
        let mut profile = NetworkProfile::managed(id, self.name.trim());
        let hostname = self.hostname.trim();
        profile.hostname = (!hostname.is_empty()).then(|| hostname.to_owned());
        profile
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_import_says_so() {
        let failure = parse_import(&Secret::new("  \n")).unwrap_err();
        assert_eq!(failure.reason, ImportReason::Empty);
    }

    #[test]
    fn garbage_names_its_line() {
        let bad = parse_import(&Secret::new("this is = = not toml")).unwrap_err();
        assert_eq!(bad.reason, ImportReason::Syntax);
        assert!(bad.line.is_some());
    }

    #[test]
    fn a_link_that_is_not_one_is_refused_by_name() {
        let bad = parse_import(&Secret::new("easytierui://import?v=9&c=AAAA")).unwrap_err();
        assert_eq!(bad.reason, ImportReason::LinkVersion);
    }

    #[test]
    fn enrollment_needs_a_name_and_a_control_plane_address() {
        let mut draft = ManagedDraft::new("Office");
        assert_eq!(draft.problems().len(), 1);
        assert!(draft.visible_problems().is_empty());
        draft.config_server = Secret::new("not an address");
        assert_eq!(
            draft.problems(),
            vec![FieldProblem::of(
                Field::ConfigServer,
                ProblemKind::ConfigServerShape
            )]
        );
        draft.config_server = Secret::new("udp://cp.example.net:22020/office-7f3a");
        assert!(draft.problems().is_empty());
    }
}
