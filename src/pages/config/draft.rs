//! What the rule dialog holds while it is open: the text of each field, parsed into an [`AclRule`] only when every
//! field is valid, with the problem of each field that is not.

use crate::core::acl::{
    AclAction, AclProtocol, AclRule, IpRange, MAX_PRIORITY, PortRange, RangeError,
};

/// Why a field of the rule dialog cannot be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FieldError {
    /// Not a whole number from 0 to `max`.
    Number {
        /// The highest value allowed.
        max: u32,
    },
    /// One entry of a list is not valid.
    Entry {
        /// Which entry, counting from 1.
        entry: usize,
        /// How many entries the list has.
        total: usize,
        /// What is wrong with it.
        error: RangeError,
    },
}

/// The problem of each field, `None` where the field reads fine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct RuleErrors {
    pub(super) source_ips: Option<FieldError>,
    pub(super) destination_ips: Option<FieldError>,
    pub(super) source_ports: Option<FieldError>,
    pub(super) destination_ports: Option<FieldError>,
    pub(super) priority: Option<FieldError>,
    pub(super) rate_limit: Option<FieldError>,
    pub(super) burst_limit: Option<FieldError>,
}

impl RuleErrors {
    /// Whether any field has a problem.
    pub(super) const fn any(&self) -> bool {
        self.source_ips.is_some()
            || self.destination_ips.is_some()
            || self.source_ports.is_some()
            || self.destination_ports.is_some()
            || self.priority.is_some()
            || self.rate_limit.is_some()
            || self.burst_limit.is_some()
    }
}

/// The dialog's fields as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuleInput {
    pub(super) name: String,
    pub(super) description: String,
    pub(super) enabled: bool,
    pub(super) action: AclAction,
    pub(super) protocol: AclProtocol,
    pub(super) source_ips: String,
    pub(super) destination_ips: String,
    pub(super) source_ports: String,
    pub(super) destination_ports: String,
    pub(super) priority: String,
    pub(super) stateful: bool,
    pub(super) rate_limit: String,
    pub(super) burst_limit: String,
    pub(super) source_groups: Vec<String>,
    pub(super) destination_groups: Vec<String>,
}

/// The entries of a list typed with commas, semicolons or line breaks between them; empty entries are skipped.
fn entries(text: &str) -> Vec<&str> {
    text.split([',', ';', '\n', '\r'])
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .collect()
}

/// Each entry of `text` read with `parse`, or the first that is not valid.
fn parse_list<T>(
    text: &str,
    parse: fn(&str) -> Result<T, RangeError>,
) -> Result<Vec<T>, FieldError> {
    let items = entries(text);
    let total = items.len();
    items
        .into_iter()
        .enumerate()
        .map(|(index, entry)| {
            parse(entry).map_err(|error| FieldError::Entry {
                entry: index.saturating_add(1),
                total,
                error,
            })
        })
        .collect()
}

/// A whole number of digits only, at most `max`; `empty` is what an empty field means (`None`: it is an error).
fn parse_number(text: &str, max: u32, empty: Option<u32>) -> Result<u32, FieldError> {
    let text = text.trim();
    let bad = FieldError::Number { max };
    if text.is_empty() {
        return empty.ok_or(bad);
    }
    if !text.chars().all(|c| c.is_ascii_digit()) {
        return Err(bad);
    }
    match text.parse::<u32>() {
        Ok(value) if value <= max => Ok(value),
        _ => Err(bad),
    }
}

/// `result`'s value, or the default with the error stored in `slot`.
fn take<T: Default>(result: Result<T, FieldError>, slot: &mut Option<FieldError>) -> T {
    result.unwrap_or_else(|error| {
        *slot = Some(error);
        T::default()
    })
}

/// A list as the dialog shows it: one entry per line.
fn lines<T: ToString>(items: &[T]) -> String {
    items
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

/// A limit as the dialog shows it: empty for none.
fn limit(value: u32) -> String {
    if value == 0 {
        String::new()
    } else {
        value.to_string()
    }
}

impl RuleInput {
    /// The fields of `rule`.
    pub(super) fn from_rule(rule: &AclRule) -> Self {
        Self {
            name: rule.name.clone(),
            description: rule.description.clone(),
            enabled: rule.enabled,
            action: rule.action,
            protocol: rule.protocol,
            source_ips: lines(&rule.source_ips),
            destination_ips: lines(&rule.destination_ips),
            source_ports: lines(&rule.source_ports),
            destination_ports: lines(&rule.destination_ports),
            priority: rule.priority.to_string(),
            stateful: rule.stateful,
            rate_limit: limit(rule.rate_limit),
            burst_limit: limit(rule.burst_limit),
            source_groups: rule.source_groups.clone(),
            destination_groups: rule.destination_groups.clone(),
        }
    }

    /// The rule the fields describe, or the problem of each field that cannot be read. Ports of a protocol without
    /// ports are left out whatever the fields hold.
    pub(super) fn build(&self) -> Result<AclRule, Box<RuleErrors>> {
        let mut errors = RuleErrors::default();
        let ports = self.protocol.has_ports();
        let source_ips = take(
            parse_list(&self.source_ips, IpRange::parse),
            &mut errors.source_ips,
        );
        let destination_ips = take(
            parse_list(&self.destination_ips, IpRange::parse),
            &mut errors.destination_ips,
        );
        let source_ports = if ports {
            take(
                parse_list(&self.source_ports, PortRange::parse),
                &mut errors.source_ports,
            )
        } else {
            Vec::new()
        };
        let destination_ports = if ports {
            take(
                parse_list(&self.destination_ports, PortRange::parse),
                &mut errors.destination_ports,
            )
        } else {
            Vec::new()
        };
        let priority = take(
            parse_number(&self.priority, MAX_PRIORITY, None),
            &mut errors.priority,
        );
        let rate_limit = take(
            parse_number(&self.rate_limit, u32::MAX, Some(0)),
            &mut errors.rate_limit,
        );
        let burst_limit = take(
            parse_number(&self.burst_limit, u32::MAX, Some(0)),
            &mut errors.burst_limit,
        );
        if errors.any() {
            return Err(Box::new(errors));
        }
        Ok(AclRule {
            name: self.name.trim().to_owned(),
            description: self.description.trim().to_owned(),
            priority,
            enabled: self.enabled,
            protocol: self.protocol,
            source_ips,
            destination_ips,
            source_ports,
            destination_ports,
            action: self.action,
            rate_limit,
            burst_limit,
            stateful: self.stateful,
            source_groups: self.source_groups.clone(),
            destination_groups: self.destination_groups.clone(),
        })
    }

    /// The problem of each field (none when the fields read fine).
    pub(super) fn errors(&self) -> RuleErrors {
        self.build().err().map(|errors| *errors).unwrap_or_default()
    }
}

/// What a rule matches, in machine voice: `TCP 10.144.144.0/24 @ops → * :22,80`. `*` stands for any; `:` introduces
/// ports and `@` groups.
pub(super) fn summary(rule: &AclRule) -> String {
    let protocol = match rule.protocol {
        AclProtocol::Any => "*",
        other => other.label(),
    };
    let side = |ips: &[IpRange], ports: &[PortRange], groups: &[String]| {
        let mut out = if ips.is_empty() {
            "*".to_owned()
        } else {
            join(ips)
        };
        if !ports.is_empty() {
            out.push_str(" :");
            out.push_str(&join(ports));
        }
        if !groups.is_empty() {
            out.push_str(" @");
            out.push_str(&groups.join(","));
        }
        out
    };
    format!(
        "{protocol} {} → {}",
        side(&rule.source_ips, &rule.source_ports, &rule.source_groups),
        side(
            &rule.destination_ips,
            &rule.destination_ports,
            &rule.destination_groups
        )
    )
}

/// The items joined by commas.
fn join<T: ToString>(items: &[T]) -> String {
    items
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> RuleInput {
        RuleInput::from_rule(&AclRule::new("web"))
    }

    #[test]
    fn entries_split_on_commas_semicolons_and_lines() {
        assert_eq!(entries("a, b;c\nd\r\n\n ,e "), ["a", "b", "c", "d", "e"]);
        assert!(entries(" , \n").is_empty());
    }

    #[test]
    fn a_fresh_rule_reads_back_unchanged() {
        let rule = AclRule::new("web");
        assert_eq!(input().build(), Ok(rule));
    }

    #[test]
    fn an_existing_rule_round_trips_through_its_fields() {
        let mut rule = AclRule::new("db");
        rule.description = "database".to_owned();
        rule.priority = 65_535;
        rule.protocol = AclProtocol::Tcp;
        rule.action = AclAction::Drop;
        rule.stateful = true;
        rule.rate_limit = 100;
        rule.burst_limit = 5;
        rule.source_ips = ["10.144.144.0/24", "fd00::1"]
            .iter()
            .filter_map(|text| IpRange::parse(text).ok())
            .collect();
        rule.destination_ports = ["80", "1000-2000"]
            .iter()
            .filter_map(|text| PortRange::parse(text).ok())
            .collect();
        rule.source_groups = vec!["ops".to_owned()];
        assert_eq!(RuleInput::from_rule(&rule).build(), Ok(rule));
    }

    #[test]
    fn lists_report_the_entry_that_is_wrong() {
        let mut fields = input();
        fields.source_ips = "10.0.0.1, nope, 10.0.0.2".to_owned();
        assert_eq!(
            fields.errors().source_ips,
            Some(FieldError::Entry {
                entry: 2,
                total: 3,
                error: RangeError::BadIp
            })
        );
        fields.source_ips = "10.0.0.5/24".to_owned();
        assert_eq!(
            fields.errors().source_ips,
            Some(FieldError::Entry {
                entry: 1,
                total: 1,
                error: RangeError::HostBits
            })
        );
        fields.source_ips = String::new();
        fields.destination_ports = "80, 70-60".to_owned();
        assert_eq!(
            fields.errors().destination_ports,
            Some(FieldError::Entry {
                entry: 2,
                total: 2,
                error: RangeError::BadPort
            })
        );
    }

    #[test]
    fn the_priority_is_a_number_up_to_the_maximum() {
        let mut fields = input();
        for good in ["0", " 65535 ", "007"] {
            fields.priority = good.to_owned();
            assert_eq!(fields.errors().priority, None, "{good}");
        }
        for bad in ["", "65536", "-1", "+5", "1e3", "ten", "4294967296"] {
            fields.priority = bad.to_owned();
            assert_eq!(
                fields.errors().priority,
                Some(FieldError::Number { max: MAX_PRIORITY }),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn empty_limits_mean_none_and_junk_is_an_error() {
        let mut fields = input();
        fields.rate_limit = String::new();
        fields.burst_limit = "0".to_owned();
        let rule = fields.build();
        assert!(rule.is_ok_and(|rule| rule.rate_limit == 0 && rule.burst_limit == 0));
        fields.rate_limit = "fast".to_owned();
        assert_eq!(
            fields.errors().rate_limit,
            Some(FieldError::Number { max: u32::MAX })
        );
        fields.rate_limit = "4294967296".to_owned();
        assert!(fields.errors().rate_limit.is_some());
    }

    #[test]
    fn ports_of_icmp_are_ignored_even_when_malformed() {
        let mut fields = input();
        fields.protocol = AclProtocol::Icmp;
        fields.source_ports = "garbage".to_owned();
        fields.destination_ports = "80".to_owned();
        let rule = fields.build();
        assert!(rule.is_ok_and(|rule| {
            rule.source_ports.is_empty() && rule.destination_ports.is_empty()
        }));
    }

    #[test]
    fn names_are_trimmed() {
        let mut fields = input();
        fields.name = "  spaced  ".to_owned();
        fields.description = "\tnote ".to_owned();
        let rule = fields.build();
        assert!(rule.is_ok_and(|rule| rule.name == "spaced" && rule.description == "note"));
    }

    #[test]
    fn every_bad_field_is_reported_at_once() {
        let mut fields = input();
        fields.source_ips = "x".to_owned();
        fields.priority = "x".to_owned();
        fields.burst_limit = "x".to_owned();
        let errors = fields.errors();
        assert!(errors.source_ips.is_some());
        assert!(errors.priority.is_some());
        assert!(errors.burst_limit.is_some());
        assert!(errors.destination_ips.is_none());
        assert!(errors.any());
    }

    #[test]
    fn the_summary_is_a_machine_line() {
        let mut rule = AclRule::new("x");
        assert_eq!(summary(&rule), "* * → *");
        rule.protocol = AclProtocol::Tcp;
        rule.source_ips = IpRange::parse("10.144.144.0/24").into_iter().collect();
        rule.source_groups = vec!["ops".to_owned(), "dev".to_owned()];
        rule.destination_ports = ["22", "80-90"]
            .iter()
            .filter_map(|text| PortRange::parse(text).ok())
            .collect();
        assert_eq!(summary(&rule), "TCP 10.144.144.0/24 @ops,dev → * :22,80-90");
        rule.protocol = AclProtocol::Icmpv6;
        rule.destination_ports.clear();
        assert_eq!(summary(&rule), "ICMPv6 10.144.144.0/24 @ops,dev → *");
    }
}
