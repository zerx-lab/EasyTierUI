//! The create and join flows: one decision per step, then a profile saved (and started on request).

use super::problem::{Field, FieldProblem, ProblemKind, SaveFailure};
use crate::{
    core::{net::PeerUrl, secret::Secret},
    profile::{MAX_NAME_CHARS, clean_name},
};

/// Which flow is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowKind {
    /// A new network: the secret is generated here and shared with whoever may join.
    Create,
    /// An existing network: its name and secret come from whoever runs it, and a shared node or peer leads in.
    Join,
}

/// A screen of a flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowStep {
    /// Create: what to call the network in this app.
    Name,
    /// The EasyTier network name and its secret.
    Network,
    /// Join: the shared node or peer to connect to.
    Peer,
    /// Create: what will be saved.
    Summary,
}

/// The state of an open create or join flow. Secrets are [`Secret`]s; pages read them only to fill the field.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowDraft {
    /// Create or join.
    pub kind: FlowKind,
    /// The screen shown.
    pub step: FlowStep,
    /// Create: the name shown in the app.
    pub name: String,
    /// The EasyTier network name.
    pub network_name: String,
    /// The network secret.
    pub secret: Secret,
    /// Join: the peer or shared node as typed (`host`, `host:port` or a URL).
    pub peer: String,
    /// Whether empty required fields show their error (after a Next that was refused).
    pub show_errors: bool,
    /// The profile is being saved.
    pub saving: bool,
    /// Why the last save failed.
    pub failure: Option<SaveFailure>,
}

impl FlowDraft {
    /// A fresh flow of `kind` on its first screen.
    pub fn new(kind: FlowKind) -> Self {
        Self {
            kind,
            step: Self::steps(kind)
                .first()
                .copied()
                .unwrap_or(FlowStep::Network),
            name: String::new(),
            network_name: String::new(),
            secret: Secret::default(),
            peer: String::new(),
            show_errors: false,
            saving: false,
            failure: None,
        }
    }

    /// The screens of a flow, in order.
    pub fn steps(kind: FlowKind) -> &'static [FlowStep] {
        match kind {
            FlowKind::Create => &[FlowStep::Name, FlowStep::Network, FlowStep::Summary],
            FlowKind::Join => &[FlowStep::Network, FlowStep::Peer],
        }
    }

    /// The screen number (1-based) and the number of screens, for "Step 2 of 3".
    pub fn position(&self) -> (usize, usize) {
        let steps = Self::steps(self.kind);
        let at = steps
            .iter()
            .position(|step| *step == self.step)
            .unwrap_or_default();
        (at.saturating_add(1), steps.len())
    }

    /// Whether this is the last screen (the one that saves).
    pub fn is_last(&self) -> bool {
        Self::steps(self.kind).last() == Some(&self.step)
    }

    /// The screen after this one, if any.
    pub fn next_step(&self) -> Option<FlowStep> {
        let steps = Self::steps(self.kind);
        let at = steps.iter().position(|step| *step == self.step)?;
        steps.get(at.saturating_add(1)).copied()
    }

    /// The screen before this one, if any.
    pub fn previous_step(&self) -> Option<FlowStep> {
        let steps = Self::steps(self.kind);
        let at = steps.iter().position(|step| *step == self.step)?;
        steps.get(at.checked_sub(1)?).copied()
    }

    /// The name the profile gets: the typed one when creating, the network name when joining.
    pub fn display_name(&self) -> String {
        match self.kind {
            FlowKind::Create => clean_name(&self.name, MAX_NAME_CHARS),
            FlowKind::Join => clean_name(&self.network_name, MAX_NAME_CHARS),
        }
    }

    /// Every problem of `step`.
    pub fn problems_of(&self, step: FlowStep) -> Vec<FieldProblem> {
        let mut found = Vec::new();
        match step {
            FlowStep::Name => text_problems(&mut found, Field::Name, &self.name),
            FlowStep::Network => {
                text_problems(&mut found, Field::NetworkName, &self.network_name);
                if self.secret.is_empty() {
                    found.push(FieldProblem::of(Field::Secret, ProblemKind::Required));
                }
            }
            FlowStep::Peer => {
                if let Err(problem) = self.peer_url() {
                    found.push(problem);
                }
            }
            FlowStep::Summary => {}
        }
        found
    }

    /// The problems of the screen shown that the person should see now: all of them after a refused Next, otherwise
    /// only those about something typed (an empty field is not an error before the person has had a go).
    pub fn visible_problems(&self) -> Vec<FieldProblem> {
        let all = self.problems_of(self.step);
        if self.show_errors {
            all
        } else {
            all.into_iter()
                .filter(|problem| problem.kind != ProblemKind::Required)
                .collect()
        }
    }

    /// The first screen with a problem, if any (Save checks them all).
    pub fn first_step_with_problem(&self) -> Option<FlowStep> {
        Self::steps(self.kind)
            .iter()
            .copied()
            .find(|step| !self.problems_of(*step).is_empty())
    }

    /// The typed peer as a URL: a bare `host` or `host:port` is a `tcp://` address.
    ///
    /// # Errors
    ///
    /// The problem with the text, under [`Field::Peers`].
    pub fn peer_url(&self) -> Result<PeerUrl, FieldProblem> {
        parse_peer(&self.peer).map_err(|problem| FieldProblem {
            entry: None,
            ..problem
        })
    }
}

/// A name-like text's problems under `field`.
fn text_problems(found: &mut Vec<FieldProblem>, field: Field, text: &str) {
    if text.trim().is_empty() {
        found.push(FieldProblem::of(field, ProblemKind::Required));
    } else if text.chars().any(char::is_control) {
        found.push(FieldProblem::of(field, ProblemKind::ControlChars));
    } else if text.trim().chars().count() > MAX_NAME_CHARS {
        found.push(FieldProblem::of(
            field,
            ProblemKind::TooLong {
                max: MAX_NAME_CHARS,
            },
        ));
    }
}

/// A peer typed as a URL, or as `host` / `host:port` (then over `tcp`).
///
/// # Errors
///
/// The problem with the text, under [`Field::Peers`] (entry 1).
pub fn parse_peer(text: &str) -> Result<PeerUrl, FieldProblem> {
    parse_peer_at("peers[0]", text)
}

/// As [`parse_peer`], reporting under the field name `field` (`peers[3]`).
///
/// # Errors
///
/// The problem with the text.
pub fn parse_peer_at(field: &str, text: &str) -> Result<PeerUrl, FieldProblem> {
    let text = text.trim();
    if text.is_empty() {
        return Err(FieldProblem::of(Field::Peers, ProblemKind::Required));
    }
    let written = if text.contains("://") {
        text.to_owned()
    } else {
        format!("tcp://{text}")
    };
    PeerUrl::parse(field, &written).map_err(|error| FieldProblem::from(&error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_goes_name_network_summary_and_join_network_peer() {
        let mut create = FlowDraft::new(FlowKind::Create);
        assert_eq!(create.step, FlowStep::Name);
        assert_eq!(create.position(), (1, 3));
        create.step = FlowStep::Summary;
        assert!(create.is_last());
        assert_eq!(create.previous_step(), Some(FlowStep::Network));
        let join = FlowDraft::new(FlowKind::Join);
        assert_eq!(join.step, FlowStep::Network);
        assert_eq!(join.next_step(), Some(FlowStep::Peer));
    }

    #[test]
    fn empty_required_fields_are_quiet_until_a_next_was_refused() {
        let mut draft = FlowDraft::new(FlowKind::Join);
        assert!(draft.visible_problems().is_empty());
        assert_eq!(draft.problems_of(FlowStep::Network).len(), 2);
        draft.show_errors = true;
        assert_eq!(draft.visible_problems().len(), 2);
    }

    #[test]
    fn peers_may_be_typed_without_a_scheme_and_bad_ones_are_named() {
        let mut draft = FlowDraft::new(FlowKind::Join);
        draft.peer = "node.example.net:11010".to_owned();
        assert_eq!(
            draft.peer_url().unwrap().as_str(),
            "tcp://node.example.net:11010"
        );
        draft.peer = "tcp://".to_owned();
        assert!(draft.peer_url().is_err());
        draft.peer = "gopher://example.net".to_owned();
        let problem = draft.peer_url().unwrap_err();
        assert_eq!(problem.field, Field::Peers);
        assert_eq!(problem.kind, ProblemKind::UnknownScheme);
    }

    #[test]
    fn display_names_come_from_the_name_or_the_network() {
        let mut draft = FlowDraft::new(FlowKind::Create);
        draft.name = "  家庭网络 ".to_owned();
        assert_eq!(draft.display_name(), "家庭网络");
        draft.kind = FlowKind::Join;
        draft.network_name = "office".to_owned();
        assert_eq!(draft.display_name(), "office");
    }
}
