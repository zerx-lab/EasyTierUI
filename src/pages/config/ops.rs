//! Edits of an [`AclConfig`] that the editor makes, as plain functions: renaming and removing a group keeps every
//! rule, membership and secret consistent; the problem checks used by the dialogs reuse the domain's own validation.

use crate::core::{
    acl::{
        Acl, AclAction, AclChain, AclConfig, AclPlace, AclProblem, AclProtocol, AclRule, ChainKind,
        GroupSecrets, MAX_ENTRIES, MAX_RULES,
    },
    secret::Secret,
};

/// What the editor lists of one rule: enough to draw its row and nothing it would redraw on while the rule is
/// untouched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuleRow {
    /// Its place in the chain's list (what edit and delete address).
    pub(super) index: usize,
    /// Its name; empty when it has none.
    pub(super) name: String,
    /// Whether it is checked at all.
    pub(super) enabled: bool,
    /// What happens to matching traffic.
    pub(super) action: AclAction,
    /// Higher is checked first.
    pub(super) priority: u32,
    /// What it matches, in machine voice.
    pub(super) summary: String,
}

/// The rows of `chain`, the highest priority first; equal priorities keep their order in the chain.
pub(super) fn rule_rows(chain: &AclChain) -> Vec<RuleRow> {
    let mut rows: Vec<RuleRow> = chain
        .rules
        .iter()
        .enumerate()
        .map(|(index, rule)| RuleRow {
            index,
            name: rule.name.clone(),
            enabled: rule.enabled,
            action: rule.action,
            priority: rule.priority,
            summary: super::draft::summary(rule),
        })
        .collect();
    rows.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.index.cmp(&b.index))
    });
    rows
}

/// The id a chain kind goes by in test ids.
pub(super) const fn kind_id(kind: ChainKind) -> &'static str {
    match kind {
        ChainKind::Inbound => "inbound",
        ChainKind::Outbound => "outbound",
        ChainKind::Forward => "forward",
    }
}

/// The id an action goes by in segmented controls.
pub(super) const fn action_id(action: AclAction) -> &'static str {
    match action {
        AclAction::Allow => "allow",
        AclAction::Drop => "drop",
    }
}

/// The action an id of [`action_id`] stands for.
pub(super) fn action_from_id(id: &str) -> Option<AclAction> {
    [AclAction::Allow, AclAction::Drop]
        .into_iter()
        .find(|action| action_id(*action) == id)
}

/// The id a chain kind goes by in selects.
pub(super) fn kind_from_id(id: &str) -> Option<ChainKind> {
    ChainKind::ALL.into_iter().find(|kind| kind_id(*kind) == id)
}

/// The id a protocol goes by in selects.
pub(super) const fn protocol_id(protocol: AclProtocol) -> &'static str {
    match protocol {
        AclProtocol::Any => "any",
        AclProtocol::Tcp => "tcp",
        AclProtocol::Udp => "udp",
        AclProtocol::Icmp => "icmp",
        AclProtocol::Icmpv6 => "icmpv6",
    }
}

/// The protocol an id of [`protocol_id`] stands for.
pub(super) fn protocol_from_id(id: &str) -> Option<AclProtocol> {
    AclProtocol::ALL
        .into_iter()
        .find(|protocol| protocol_id(*protocol) == id)
}

/// Whether there is nothing in `config`: no chains and no groups.
pub(super) fn is_blank(config: &AclConfig) -> bool {
    config.acl.chains.is_empty()
        && config.acl.declared_groups.is_empty()
        && config.acl.joined_groups.is_empty()
}

/// Replaces `old` by `new` in `names`, keeping the first of any duplicates that results.
fn replace_name(names: &mut Vec<String>, old: &str, new: &str) {
    let mut out: Vec<String> = Vec::with_capacity(names.len());
    for name in names.drain(..) {
        let name = if name == old { new.to_owned() } else { name };
        if !out.contains(&name) {
            out.push(name);
        }
    }
    *names = out;
}

/// Renames the group `old` to `new` everywhere: the declaration, the memberships, every rule that matches it, and
/// its secret.
pub(super) fn rename_group(config: &mut AclConfig, old: &str, new: &str) {
    if old == new {
        return;
    }
    for name in &mut config.acl.declared_groups {
        if name == old {
            new.clone_into(name);
        }
    }
    replace_name(&mut config.acl.joined_groups, old, new);
    for chain in &mut config.acl.chains {
        for rule in &mut chain.rules {
            replace_name(&mut rule.source_groups, old, new);
            replace_name(&mut rule.destination_groups, old, new);
        }
    }
    config.group_secrets.rename(old, new);
}

/// Removes the group `name` everywhere: the declaration, the membership, the condition on it in every rule, and its
/// secret.
pub(super) fn remove_group(config: &mut AclConfig, name: &str) {
    config.acl.declared_groups.retain(|group| group != name);
    config.acl.joined_groups.retain(|group| group != name);
    for chain in &mut config.acl.chains {
        for rule in &mut chain.rules {
            rule.source_groups.retain(|group| group != name);
            rule.destination_groups.retain(|group| group != name);
        }
    }
    config.group_secrets.remove(name);
}

/// How many rules match on the group `name`.
pub(super) fn rules_using(config: &AclConfig, name: &str) -> usize {
    config
        .acl
        .chains
        .iter()
        .flat_map(|chain| &chain.rules)
        .filter(|rule| {
            rule.source_groups.iter().any(|group| group == name)
                || rule.destination_groups.iter().any(|group| group == name)
        })
        .count()
}

/// Makes this node a member of the group `name`, or not.
pub(super) fn set_joined(config: &mut AclConfig, name: &str, joined: bool) {
    let member = config.acl.joined_groups.iter().any(|group| group == name);
    match (joined, member) {
        (true, false) => config.acl.joined_groups.push(name.to_owned()),
        (false, true) => config.acl.joined_groups.retain(|group| group != name),
        _ => {}
    }
}

/// The groups a rule may name: the declared ones, the memberships, and `current` (what the rule already names, so
/// editing it never drops a group declared elsewhere).
pub(super) fn known_groups(config: &AclConfig, current: &[String]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for name in config
        .acl
        .declared_groups
        .iter()
        .chain(&config.acl.joined_groups)
        .chain(current)
    {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    names
}

/// What is wrong with declaring the group `name` with `secret` at place `editing` (`None`: a new group), apart from
/// problems elsewhere in the ACL.
pub(super) fn group_problem(
    config: &AclConfig,
    editing: Option<usize>,
    name: &str,
    secret: &Secret,
) -> Option<AclProblem> {
    let mut secrets = GroupSecrets::default();
    secrets.insert(name, secret.clone());
    let probe = AclConfig {
        acl: Acl {
            declared_groups: vec![name.to_owned()],
            ..Acl::default()
        },
        group_secrets: secrets,
    };
    if let Err(error) = probe.check() {
        return Some(error.problem);
    }
    let taken = config
        .acl
        .declared_groups
        .iter()
        .enumerate()
        .any(|(index, group)| Some(index) != editing && group == name);
    if taken {
        return Some(AclProblem::Duplicate);
    }
    if editing.is_none() && config.acl.declared_groups.len() >= MAX_ENTRIES {
        return Some(AclProblem::TooMany { max: MAX_ENTRIES });
    }
    None
}

/// What is wrong with `rule` as an addition to a chain that already has `existing` rules (`editing`: it replaces
/// one of them instead), apart from problems elsewhere in the ACL.
pub(super) fn rule_problem(existing: usize, editing: bool, rule: &AclRule) -> Option<AclProblem> {
    if !editing && existing >= MAX_RULES {
        return Some(AclProblem::TooMany { max: MAX_RULES });
    }
    let mut chain = AclChain::new(ChainKind::Inbound, "");
    chain.rules.push(rule.clone());
    let probe = Acl {
        chains: vec![chain],
        ..Acl::default()
    };
    match probe.check() {
        Err(error) if matches!(error.place, AclPlace::Rule { .. }) => Some(error.problem),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::acl::{MAX_NAME_CHARS, PortRange};

    fn with_group() -> AclConfig {
        let mut config = AclConfig::default();
        config.acl.declared_groups = vec!["ops".to_owned(), "dev".to_owned()];
        config.acl.joined_groups = vec!["ops".to_owned()];
        config.group_secrets.insert("ops", Secret::new("s1"));
        config.group_secrets.insert("dev", Secret::new("s2"));
        let mut chain = AclChain::new(ChainKind::Inbound, "in");
        let mut rule = AclRule::new("ssh");
        rule.source_groups = vec!["ops".to_owned()];
        rule.destination_groups = vec!["dev".to_owned(), "ops".to_owned()];
        chain.rules.push(rule);
        config.acl.chains.push(chain);
        config
    }

    fn rule_groups(config: &AclConfig) -> (Vec<String>, Vec<String>) {
        let rule = &config.acl.chains[0].rules[0];
        (rule.source_groups.clone(), rule.destination_groups.clone())
    }

    #[test]
    fn rows_are_sorted_by_priority_then_position() {
        let mut chain = AclChain::new(ChainKind::Forward, "f");
        for (name, priority) in [("a", 10), ("b", 500), ("c", 10), ("d", 500)] {
            let mut rule = AclRule::new(name);
            rule.priority = priority;
            chain.rules.push(rule);
        }
        let order: Vec<(&str, usize)> = rule_rows(&chain)
            .iter()
            .map(|row| (["a", "b", "c", "d"][row.index], row.index))
            .collect();
        assert_eq!(order, [("b", 1), ("d", 3), ("a", 0), ("c", 2)]);
    }

    #[test]
    fn renaming_a_group_follows_it_everywhere() {
        let mut config = with_group();
        rename_group(&mut config, "ops", "admins");
        assert_eq!(config.acl.declared_groups, ["admins", "dev"]);
        assert_eq!(config.acl.joined_groups, ["admins"]);
        assert_eq!(
            rule_groups(&config),
            (
                vec!["admins".to_owned()],
                vec!["dev".to_owned(), "admins".to_owned()]
            )
        );
        assert!(config.group_secrets.get("ops").is_none());
        assert_eq!(
            config.group_secrets.get("admins").map(Secret::expose),
            Some("s1")
        );
        assert!(config.check().is_ok());
    }

    #[test]
    fn renaming_onto_a_name_a_rule_already_names_does_not_duplicate() {
        let mut config = with_group();
        rename_group(&mut config, "ops", "dev");
        let (source, destination) = rule_groups(&config);
        assert_eq!(source, ["dev"]);
        assert_eq!(destination, ["dev"]);
    }

    #[test]
    fn removing_a_group_cleans_every_reference_and_its_secret() {
        let mut config = with_group();
        remove_group(&mut config, "ops");
        assert_eq!(config.acl.declared_groups, ["dev"]);
        assert!(config.acl.joined_groups.is_empty());
        assert_eq!(rule_groups(&config), (vec![], vec!["dev".to_owned()]));
        assert!(config.group_secrets.get("ops").is_none());
        assert!(config.group_secrets.get("dev").is_some());
        assert!(config.check().is_ok());
    }

    #[test]
    fn rules_using_counts_rules_not_mentions() {
        let config = with_group();
        assert_eq!(rules_using(&config, "ops"), 1);
        assert_eq!(rules_using(&config, "dev"), 1);
        assert_eq!(rules_using(&config, "nobody"), 0);
    }

    #[test]
    fn joining_is_idempotent() {
        let mut config = with_group();
        set_joined(&mut config, "dev", true);
        set_joined(&mut config, "dev", true);
        assert_eq!(config.acl.joined_groups, ["ops", "dev"]);
        set_joined(&mut config, "ops", false);
        set_joined(&mut config, "ops", false);
        assert_eq!(config.acl.joined_groups, ["dev"]);
    }

    #[test]
    fn known_groups_merge_without_repeats_and_keep_what_a_rule_names() {
        let config = with_group();
        let names = known_groups(&config, &["elsewhere".to_owned(), "dev".to_owned()]);
        assert_eq!(names, ["ops", "dev", "elsewhere"]);
    }

    #[test]
    fn a_group_needs_a_name_a_secret_and_a_free_name() {
        let config = with_group();
        let secret = Secret::new("x");
        assert_eq!(
            group_problem(&config, None, "  ", &secret),
            Some(AclProblem::BadName)
        );
        assert_eq!(
            group_problem(&config, None, "new", &Secret::default()),
            Some(AclProblem::MissingSecret)
        );
        assert_eq!(
            group_problem(&config, None, "dev", &secret),
            Some(AclProblem::Duplicate)
        );
        assert_eq!(group_problem(&config, Some(1), "dev", &secret), None);
        assert_eq!(
            group_problem(&config, Some(1), "ops", &secret),
            Some(AclProblem::Duplicate)
        );
        let long = "g".repeat(MAX_NAME_CHARS.saturating_add(1));
        assert_eq!(
            group_problem(&config, None, &long, &secret),
            Some(AclProblem::TooLong {
                max: MAX_NAME_CHARS
            })
        );
        assert_eq!(group_problem(&config, None, "fresh", &secret), None);
    }

    #[test]
    fn a_full_chain_takes_no_new_rule_but_still_edits() {
        let rule = AclRule::new("r");
        assert_eq!(
            rule_problem(MAX_RULES, false, &rule),
            Some(AclProblem::TooMany { max: MAX_RULES })
        );
        assert_eq!(rule_problem(MAX_RULES, true, &rule), None);
    }

    #[test]
    fn a_rule_with_ports_on_icmp_is_a_problem() {
        let mut rule = AclRule::new("ping");
        rule.protocol = AclProtocol::Icmp;
        rule.destination_ports = PortRange::parse("80").into_iter().collect();
        assert_eq!(
            rule_problem(0, false, &rule),
            Some(AclProblem::PortsWithoutPorts)
        );
    }

    #[test]
    fn ids_round_trip() {
        for kind in ChainKind::ALL {
            assert_eq!(kind_from_id(kind_id(kind)), Some(kind));
        }
        for protocol in AclProtocol::ALL {
            assert_eq!(protocol_from_id(protocol_id(protocol)), Some(protocol));
        }
        for action in [AclAction::Allow, AclAction::Drop] {
            assert_eq!(action_from_id(action_id(action)), Some(action));
        }
        assert_eq!(action_from_id("noop"), None);
    }

    #[test]
    fn blank_means_no_chains_and_no_groups() {
        assert!(is_blank(&AclConfig::default()));
        assert!(!is_blank(&with_group()));
    }
}
