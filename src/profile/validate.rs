//! Validation: the checks that make a profile safe to store, render and launch. Every problem names its field.

use std::{collections::HashSet, hash::Hash};

use super::{
    error::ProfileError,
    model::{
        IpMode, MAX_DEV_NAME_CHARS, MAX_HOSTNAME_CHARS, MAX_LIST_ENTRIES, MAX_NAME_CHARS,
        MTU_RANGE, NetworkProfile, RelayWhitelist,
    },
};
use crate::core::{
    net::{self, NetError},
    settings::PortForward,
};

/// The most characters in one relay-whitelist pattern.
const MAX_PATTERN_CHARS: usize = 64;

/// Checks a name-like text: not blank, no control characters, at most `max` characters.
fn check_text(field: &str, text: &str, max: usize) -> Result<(), ProfileError> {
    if text.trim().is_empty() {
        return Err(ProfileError::Empty {
            field: field.to_owned(),
        });
    }
    if text.chars().any(char::is_control) {
        return Err(ProfileError::ControlChars {
            field: field.to_owned(),
        });
    }
    if text.chars().count() > max {
        return Err(ProfileError::TooLong {
            field: field.to_owned(),
            max,
        });
    }
    Ok(())
}

/// Pushes the error of `result`, if any.
fn note(problems: &mut Vec<ProfileError>, result: Result<(), ProfileError>) {
    if let Err(error) = result {
        problems.push(error);
    }
}

/// Checks a list's size and, with `key`, that no two entries are the same.
fn check_list<T, K: Hash + Eq>(
    problems: &mut Vec<ProfileError>,
    field: &str,
    items: &[T],
    key: impl Fn(&T) -> K,
) {
    if items.len() > MAX_LIST_ENTRIES {
        problems.push(ProfileError::TooMany {
            field: field.to_owned(),
            max: MAX_LIST_ENTRIES,
        });
        return;
    }
    let mut seen = HashSet::new();
    for (index, item) in items.iter().enumerate() {
        if !seen.insert(key(item)) {
            problems.push(ProfileError::Duplicate {
                field: format!("{field}[{index}]"),
            });
        }
    }
}

fn check_forward(problems: &mut Vec<ProfileError>, index: usize, forward: &PortForward) {
    if forward.bind.port() == 0 {
        problems.push(
            NetError::BadPort {
                field: format!("port_forwards[{index}].bind"),
            }
            .into(),
        );
    }
    if forward.dst.port() == 0 {
        problems.push(
            NetError::BadPort {
                field: format!("port_forwards[{index}].dst"),
            }
            .into(),
        );
    }
    if forward.dst.ip().is_unspecified() || forward.dst.ip().is_multicast() {
        problems.push(
            NetError::Reserved {
                field: format!("port_forwards[{index}].dst"),
            }
            .into(),
        );
    }
}

impl NetworkProfile {
    /// Every problem with the profile, in field order; empty when it is valid. For forms that mark all bad fields.
    pub fn problems(&self) -> Vec<ProfileError> {
        let mut problems = Vec::new();
        note(
            &mut problems,
            check_text("name", &self.name, MAX_NAME_CHARS),
        );
        if let Some(hostname) = &self.hostname {
            note(
                &mut problems,
                check_text("hostname", hostname, MAX_HOSTNAME_CHARS),
            );
        }
        if self.is_managed() {
            // A control plane pushes everything else; the stored defaults are never rendered.
            return problems;
        }
        note(
            &mut problems,
            check_text("network_name", &self.network_name, MAX_NAME_CHARS),
        );
        if let IpMode::Static(net) = self.ip
            && let Err(error) = net::check_ipv4_inet("ip", net)
        {
            problems.push(error.into());
        }
        check_list(&mut problems, "peers", &self.peers, |peer| {
            peer.as_str().to_owned()
        });
        check_list(&mut problems, "listeners", &self.listeners, |listener| {
            listener.as_str().to_owned()
        });
        // Two listeners of one scheme on one port and address cannot both bind, even if written differently.
        let mut bound = HashSet::new();
        for (index, listener) in self.listeners.iter().enumerate() {
            let key = (listener.scheme(), listener.ip(), listener.effective_port());
            if !bound.insert(key) {
                problems.push(ProfileError::Duplicate {
                    field: format!("listeners[{index}]"),
                });
            }
        }
        if let Some(mtu) = self.mtu {
            let (min, max) = MTU_RANGE;
            if !(min..=max).contains(&mtu) {
                problems.push(ProfileError::OutOfRange {
                    field: "mtu".to_owned(),
                    min: u32::from(min),
                    max: u32::from(max),
                });
            }
        }
        if let Some(dev_name) = &self.dev_name {
            note(
                &mut problems,
                check_text("dev_name", dev_name, MAX_DEV_NAME_CHARS),
            );
            if !dev_name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            {
                problems.push(ProfileError::Invalid {
                    field: "dev_name".to_owned(),
                    reason: "use letters, digits, '-', '_' and '.' only",
                });
            }
        }
        check_list(
            &mut problems,
            "subnet_proxy",
            &self.subnet_proxy,
            |subnet| subnet.cidr,
        );
        for (index, subnet) in self.subnet_proxy.iter().enumerate() {
            let field = format!("subnet_proxy[{index}]");
            if let Err(error) = net::check_ipv4_subnet(&field, subnet.cidr) {
                problems.push(error.into());
            }
            if let Some(mapped) = subnet.mapped {
                let field = format!("{field}.mapped");
                if let Err(error) = net::check_ipv4_subnet(&field, mapped) {
                    problems.push(error.into());
                } else if mapped.prefix_len() != subnet.cidr.prefix_len() {
                    problems.push(ProfileError::Invalid {
                        field,
                        reason: "must be the same size as the shared subnet",
                    });
                }
            }
        }
        check_list(
            &mut problems,
            "mapped_listeners",
            &self.mapped_listeners,
            |listener| listener.as_str().to_owned(),
        );
        if self.instance_recv_bps_limit == Some(0) {
            problems.push(ProfileError::Invalid {
                field: "instance_recv_bps_limit".to_owned(),
                reason: "must be more than 0 (leave it unset for no limit)",
            });
        }
        if let Some(routes) = &self.manual_routes {
            check_list(&mut problems, "routes", routes, |route| *route);
            for (index, route) in routes.iter().enumerate() {
                // Any prefix from /0 to /32 is a route, but the address must be the network's own.
                if route.addr() != route.network() {
                    problems.push(
                        NetError::HostBits {
                            field: format!("routes[{index}]"),
                        }
                        .into(),
                    );
                }
            }
        }
        if let Some(portal) = self.vpn_portal {
            if let Err(error) = net::check_ipv4_subnet("vpn_portal.client_cidr", portal.client_cidr)
            {
                problems.push(error.into());
            }
            if portal.listen_port == 0 {
                problems.push(
                    NetError::BadPort {
                        field: "vpn_portal.listen_port".to_owned(),
                    }
                    .into(),
                );
            }
        }
        if let Some(acl) = &self.acl
            && let Err(error) = acl.check()
        {
            problems.push(ProfileError::Acl(error));
        }
        check_list(&mut problems, "exit_nodes", &self.exit_nodes, |node| *node);
        for (index, node) in self.exit_nodes.iter().enumerate() {
            if node.is_unspecified() || node.is_multicast() || node.is_loopback() {
                problems.push(
                    NetError::Reserved {
                        field: format!("exit_nodes[{index}]"),
                    }
                    .into(),
                );
            }
        }
        if self.socks5.is_some_and(|socks5| socks5.port == 0) {
            problems.push(
                NetError::BadPort {
                    field: "socks5".to_owned(),
                }
                .into(),
            );
        }
        check_list(
            &mut problems,
            "port_forwards",
            &self.port_forwards,
            |forward| (forward.proto, forward.bind),
        );
        for (index, forward) in self.port_forwards.iter().enumerate() {
            check_forward(&mut problems, index, forward);
        }
        if let RelayWhitelist::Only(names) = &self.relay_whitelist {
            check_list(&mut problems, "relay_whitelist", names, |name| name.clone());
            for (index, name) in names.iter().enumerate() {
                let field = format!("relay_whitelist[{index}]");
                if name.chars().any(char::is_whitespace) {
                    // EasyTier stores the list space-separated; a space would split one pattern into two.
                    problems.push(ProfileError::Invalid {
                        field,
                        reason: "must not contain spaces",
                    });
                } else {
                    note(&mut problems, check_text(&field, name, MAX_PATTERN_CHARS));
                }
            }
        }
        problems
    }

    /// Checks the profile.
    ///
    /// # Errors
    ///
    /// The first problem [`NetworkProfile::problems`] finds, naming its field.
    pub fn validate(&self) -> Result<(), ProfileError> {
        match self.problems().into_iter().next() {
            Some(problem) => Err(problem),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{
            acl::{Acl, AclChain, ChainKind},
            settings::{MappedListener, Proto, VpnPortal},
        },
        profile::model::{ProfileId, Socks5, SubnetProxy},
    };

    fn profile() -> NetworkProfile {
        NetworkProfile::new(
            ProfileId::parse("0123456789abcdef0123456789abcdef").unwrap(),
            "Home",
            "home-net",
        )
    }

    fn field_of(profile: &NetworkProfile) -> String {
        profile.validate().unwrap_err().field().unwrap().to_owned()
    }

    #[test]
    fn a_default_profile_is_valid() {
        assert_eq!(profile().validate(), Ok(()));
        assert!(profile().problems().is_empty());
    }

    #[test]
    fn names_may_be_cjk_but_not_blank_long_or_control() {
        let mut p = profile();
        p.name = "家庭网络".repeat(16);
        assert_eq!(p.validate(), Ok(()));
        p.name = "家庭网络".repeat(17);
        assert!(matches!(
            p.validate(),
            Err(ProfileError::TooLong { max: 64, .. })
        ));
        assert_eq!(field_of(&p), "name");
        p.name = "   ".to_owned();
        assert!(matches!(p.validate(), Err(ProfileError::Empty { .. })));
        p.name = "a\nb".to_owned();
        assert!(matches!(
            p.validate(),
            Err(ProfileError::ControlChars { .. })
        ));
        let mut p = profile();
        p.network_name = "n".repeat(65);
        assert_eq!(field_of(&p), "network_name");
        p.network_name = String::new();
        assert!(matches!(p.validate(), Err(ProfileError::Empty { .. })));
        let mut p = profile();
        p.hostname = Some("主机".repeat(17));
        assert_eq!(field_of(&p), "hostname");
        p.hostname = Some("主机".repeat(16));
        assert_eq!(p.validate(), Ok(()));
    }

    #[test]
    fn static_addresses_subnets_and_exit_nodes_are_checked() {
        let mut p = profile();
        p.ip = IpMode::Static("10.144.144.1/24".parse().unwrap());
        assert_eq!(p.validate(), Ok(()));
        p.ip = IpMode::Static("10.144.144.0/24".parse().unwrap());
        assert_eq!(field_of(&p), "ip");
        let mut p = profile();
        p.subnet_proxy = vec![SubnetProxy::plain("192.168.50.7/24".parse().unwrap())];
        assert_eq!(field_of(&p), "subnet_proxy[0]");
        let mut p = profile();
        p.exit_nodes = vec!["0.0.0.0".parse().unwrap()];
        assert_eq!(field_of(&p), "exit_nodes[0]");
    }

    #[test]
    fn mapped_subnets_routes_portal_limit_and_acl_are_checked() {
        let subnet = |text: &str| -> SubnetProxy { SubnetProxy::parse_line("s", text).unwrap() };
        let mut p = profile();
        p.subnet_proxy = vec![subnet("192.168.50.0/24->10.50.0.0/24")];
        assert_eq!(p.validate(), Ok(()));
        p.subnet_proxy = vec![subnet("192.168.50.0/24->10.50.0.0/16")];
        assert_eq!(field_of(&p), "subnet_proxy[0].mapped");
        p.subnet_proxy = vec![
            subnet("192.168.50.0/24"),
            subnet("192.168.50.0/24->10.1.0.0/24"),
        ];
        assert_eq!(field_of(&p), "subnet_proxy[1]");

        let mut p = profile();
        p.manual_routes = Some(Vec::new());
        assert_eq!(p.validate(), Ok(()));
        p.manual_routes = Some(vec![
            "0.0.0.0/0".parse().unwrap(),
            "192.168.0.0/16".parse().unwrap(),
        ]);
        assert_eq!(p.validate(), Ok(()));
        p.manual_routes = Some(vec!["192.168.1.5/16".parse().unwrap()]);
        assert_eq!(field_of(&p), "routes[0]");
        p.manual_routes = Some(vec!["10.0.0.0/8".parse().unwrap(); 2]);
        assert_eq!(field_of(&p), "routes[1]");

        let mut p = profile();
        p.vpn_portal = Some(VpnPortal {
            client_cidr: "10.14.14.0/24".parse().unwrap(),
            listen_port: 22022,
        });
        assert_eq!(p.validate(), Ok(()));
        p.vpn_portal = Some(VpnPortal {
            client_cidr: "10.14.14.7/24".parse().unwrap(),
            listen_port: 22022,
        });
        assert_eq!(field_of(&p), "vpn_portal.client_cidr");
        p.vpn_portal = Some(VpnPortal {
            client_cidr: "10.14.14.0/24".parse().unwrap(),
            listen_port: 0,
        });
        assert_eq!(field_of(&p), "vpn_portal.listen_port");

        let mut p = profile();
        p.instance_recv_bps_limit = Some(1);
        assert_eq!(p.validate(), Ok(()));
        p.instance_recv_bps_limit = Some(0);
        assert_eq!(field_of(&p), "instance_recv_bps_limit");

        let mut p = profile();
        p.mapped_listeners =
            vec![MappedListener::parse("m", "tcp://203.0.113.5:11223").unwrap(); 2];
        assert_eq!(field_of(&p), "mapped_listeners[1]");

        let mut p = profile();
        p.acl = Some(Acl::default());
        assert_eq!(p.validate(), Ok(()));
        p.acl = Some(Acl {
            chains: vec![AclChain::new(ChainKind::Inbound, "c"); crate::core::acl::MAX_CHAINS + 1],
            ..Acl::default()
        });
        assert!(matches!(p.validate(), Err(ProfileError::Acl(_))));
        assert_eq!(field_of(&p), "acl");
    }

    #[test]
    fn mtu_dev_name_socks5_and_forwards_are_checked() {
        let mut p = profile();
        p.mtu = Some(1380);
        assert_eq!(p.validate(), Ok(()));
        p.mtu = Some(100);
        assert!(matches!(
            p.validate(),
            Err(ProfileError::OutOfRange {
                min: 1280,
                max: 1500,
                ..
            })
        ));
        let mut p = profile();
        p.dev_name = Some("et 0".to_owned());
        assert_eq!(field_of(&p), "dev_name");
        p.dev_name = Some("et0".to_owned());
        assert_eq!(p.validate(), Ok(()));
        let mut p = profile();
        p.socks5 = Some(Socks5::local(0));
        assert_eq!(field_of(&p), "socks5");
        let mut p = profile();
        p.port_forwards = vec![PortForward {
            proto: Proto::Tcp,
            bind: "127.0.0.1:0".parse().unwrap(),
            dst: "10.144.144.2:80".parse().unwrap(),
        }];
        assert_eq!(field_of(&p), "port_forwards[0].bind");
    }

    #[test]
    fn duplicates_and_oversized_lists_are_rejected() {
        let mut p = profile();
        p.peers = vec!["tcp://203.0.113.7:11010".parse().unwrap(); 2];
        assert_eq!(field_of(&p), "peers[1]");
        let mut p = profile();
        // Written differently, bound identically.
        p.listeners = vec![
            "tcp://0.0.0.0".parse().unwrap(),
            "tcp://0.0.0.0:11010".parse().unwrap(),
        ];
        assert_eq!(field_of(&p), "listeners[1]");
        let mut p = profile();
        p.peers = (1000_u16..1065)
            .map(|port| format!("tcp://203.0.113.7:{port}").parse().unwrap())
            .collect();
        assert!(matches!(
            p.validate(),
            Err(ProfileError::TooMany { max: 64, .. })
        ));
    }

    #[test]
    fn whitelist_entries_cannot_hide_a_second_pattern() {
        let mut p = profile();
        p.relay_whitelist = RelayWhitelist::Only(vec!["team-*".to_owned(), "a b".to_owned()]);
        assert_eq!(field_of(&p), "relay_whitelist[1]");
        p.relay_whitelist = RelayWhitelist::Only(vec!["team-*".to_owned()]);
        assert_eq!(p.validate(), Ok(()));
        p.relay_whitelist = RelayWhitelist::Only(Vec::new());
        assert_eq!(p.validate(), Ok(()));
    }

    #[test]
    fn managed_profiles_only_need_a_name() {
        let mut p = NetworkProfile::managed(
            ProfileId::parse("0123456789abcdef0123456789abcdef").unwrap(),
            "Office",
        );
        assert_eq!(p.validate(), Ok(()));
        p.name = String::new();
        assert_eq!(field_of(&p), "name");
    }
}
