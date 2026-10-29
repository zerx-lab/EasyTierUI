//! The words for what is wrong with an ACL or with a field of the rule dialog.

use super::draft::FieldError;
use crate::{
    core::acl::{AclError, AclPlace, AclProblem, MAX_PRIORITY, RangeError},
    i18n::{ConfigStrings, fill},
    store::Store,
};

/// A count that starts at 0 as people count: from 1.
fn nth(index: usize) -> String {
    index.saturating_add(1).to_string()
}

/// Where a problem is, in words.
fn place_text(words: &ConfigStrings, place: AclPlace) -> String {
    match place {
        AclPlace::Acl => words.acl_place_acl.to_owned(),
        AclPlace::Chain(chain) => fill(words.acl_place_chain, &[("chain", &nth(chain))]),
        AclPlace::Rule { chain, rule } => fill(
            words.acl_place_rule,
            &[("chain", &nth(chain)), ("rule", &nth(rule))],
        ),
        AclPlace::DeclaredGroup(group) => fill(words.acl_place_declared, &[("group", &nth(group))]),
        AclPlace::JoinedGroup(group) => fill(words.acl_place_joined, &[("group", &nth(group))]),
    }
}

/// Why an address or port was refused, as a clause that follows "invalid:".
pub(super) const fn range_text(words: &ConfigStrings, error: RangeError) -> &'static str {
    match error {
        RangeError::Empty => words.acl_range_empty,
        RangeError::BadIp => words.acl_range_bad_ip,
        RangeError::HostBits => words.acl_range_host_bits,
        RangeError::BadPort => words.acl_range_bad_port,
    }
}

/// What is wrong, in words and without saying where.
pub(super) fn problem_text(words: &ConfigStrings, problem: AclProblem) -> String {
    match problem {
        AclProblem::TooMany { max } => {
            fill(words.acl_problem_too_many, &[("max", &max.to_string())])
        }
        AclProblem::TooLong { max } => {
            fill(words.acl_problem_too_long, &[("max", &max.to_string())])
        }
        AclProblem::BadName => words.acl_problem_bad_name.to_owned(),
        AclProblem::Duplicate => words.acl_problem_duplicate.to_owned(),
        AclProblem::MissingSecret => words.acl_problem_missing_secret.to_owned(),
        AclProblem::Priority => fill(
            words.acl_problem_priority,
            &[("max", &MAX_PRIORITY.to_string())],
        ),
        AclProblem::PortsWithoutPorts => words.acl_problem_ports.to_owned(),
        AclProblem::Range(error) => fill(
            words.acl_problem_range,
            &[("problem", range_text(words, error))],
        ),
        AclProblem::Unreadable => words.acl_problem_unreadable.to_owned(),
    }
}

/// The words for a problem of an ACL, with where it is: "Chain 2, rule 3: The priority is above 65535." The hosts
/// show this under the editor and for the problem that blocks a save.
pub(in crate::pages) fn acl_error_text(store: Store, error: &AclError) -> String {
    let words = store.strings().config;
    fill(
        words.acl_error_line,
        &[
            ("place", &place_text(words, error.place)),
            ("problem", &problem_text(words, error.problem)),
        ],
    )
}

/// The words for a field of the rule dialog that cannot be read.
pub(super) fn field_error_text(words: &ConfigStrings, error: FieldError) -> String {
    match error {
        FieldError::Number { max } => fill(words.acl_number_error, &[("max", &max.to_string())]),
        FieldError::Entry {
            entry,
            total,
            error,
        } => {
            let problem = range_text(words, error);
            if total > 1 {
                fill(
                    words.acl_entry_error,
                    &[("n", &entry.to_string()), ("problem", problem)],
                )
            } else {
                problem.to_owned()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{core::acl::MAX_ENTRIES, i18n::Language};

    fn words(language: Language) -> &'static ConfigStrings {
        language.strings().config
    }

    #[test]
    fn places_count_from_one_in_both_languages() {
        let rule = AclPlace::Rule { chain: 1, rule: 2 };
        assert_eq!(place_text(words(Language::En), rule), "Chain 2, rule 3");
        assert_eq!(
            place_text(words(Language::ZhCn), rule),
            "第 2 条规则链的第 3 条规则"
        );
        assert_eq!(
            place_text(words(Language::En), AclPlace::DeclaredGroup(0)),
            "Declared group 1"
        );
    }

    #[test]
    fn every_problem_has_words_without_leftover_placeholders() {
        let problems = [
            AclProblem::TooMany { max: MAX_ENTRIES },
            AclProblem::TooLong { max: 64 },
            AclProblem::BadName,
            AclProblem::Duplicate,
            AclProblem::MissingSecret,
            AclProblem::Priority,
            AclProblem::PortsWithoutPorts,
            AclProblem::Range(RangeError::Empty),
            AclProblem::Range(RangeError::BadIp),
            AclProblem::Range(RangeError::HostBits),
            AclProblem::Range(RangeError::BadPort),
            AclProblem::Unreadable,
        ];
        for language in [Language::En, Language::ZhCn] {
            for problem in problems {
                let text = problem_text(words(language), problem);
                assert!(!text.is_empty());
                assert!(!text.contains('{'), "{problem:?}: {text}");
            }
        }
    }

    #[test]
    fn a_priority_problem_names_the_limit() {
        assert!(problem_text(words(Language::En), AclProblem::Priority).contains("65535"));
    }

    #[test]
    fn a_single_entry_error_is_the_clause_and_a_list_error_names_the_entry() {
        let single = FieldError::Entry {
            entry: 1,
            total: 1,
            error: RangeError::BadPort,
        };
        let many = FieldError::Entry {
            entry: 2,
            total: 3,
            error: RangeError::BadPort,
        };
        let en = words(Language::En);
        assert_eq!(field_error_text(en, single), en.acl_range_bad_port);
        assert!(field_error_text(en, many).starts_with("Entry 2: "));
        assert!(field_error_text(words(Language::ZhCn), many).starts_with("第 2 项："));
        assert!(field_error_text(en, FieldError::Number { max: 9 }).contains('9'));
    }
}
