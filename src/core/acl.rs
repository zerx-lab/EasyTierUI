//! Access control: EasyTier's ACL v1 as domain types shared by local profiles and console configs.
//!
//! An ACL is a list of chains, each applied to one direction of traffic (to this node, from it, or forwarded for its
//! proxied subnets), each a list of rules checked from the highest priority down, then the chain's default action.
//! Rules can also match groups: a group is a name and a secret, declared by the nodes that know it; a node joins the
//! groups it declares and lists as joined, and peers prove membership with the secret.
//!
//! Group secrets are secrets: [`Acl`] (what a profile stores) holds group names only, and the secrets travel beside
//! it in [`GroupSecrets`] (the keychain, a config file, easytier-web's JSON). [`AclConfig`] is the two together, as
//! an editor holds them. [`wire`] is EasyTier's own shape, the same in the TOML config file and easytier-web's JSON.

use std::{collections::BTreeMap, fmt, net::IpAddr, str::FromStr};

use ipnet::IpNet;
use serde::{Deserialize, Serialize};

use crate::core::secret::Secret;

/// The most chains an ACL may have.
pub const MAX_CHAINS: usize = 32;
/// The most rules a chain may have.
pub const MAX_RULES: usize = 256;
/// The most entries of one list of a rule (addresses, ports, groups) and the most groups declared or joined.
pub const MAX_ENTRIES: usize = 64;
/// The longest chain, rule or group name, in characters.
pub const MAX_NAME_CHARS: usize = 64;
/// The longest chain or rule description, in characters.
pub const MAX_DESCRIPTION_CHARS: usize = 256;
/// The highest rule priority EasyTier documents.
pub const MAX_PRIORITY: u32 = 65_535;
/// The priority a new rule starts with.
pub const DEFAULT_PRIORITY: u32 = 1000;

/// Which traffic a chain applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChainKind {
    /// Traffic sent to this node.
    Inbound,
    /// Traffic this node sends.
    Outbound,
    /// Traffic this node forwards into the subnets it proxies.
    Forward,
}

impl ChainKind {
    /// Every kind, in the order the editor offers them.
    pub const ALL: [Self; 3] = [Self::Inbound, Self::Outbound, Self::Forward];

    const fn wire(self) -> i32 {
        match self {
            Self::Inbound => 1,
            Self::Outbound => 2,
            Self::Forward => 3,
        }
    }

    const fn from_wire(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Inbound),
            2 => Some(Self::Outbound),
            3 => Some(Self::Forward),
            _ => None,
        }
    }
}

/// What happens to matching traffic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AclAction {
    /// Let it through.
    Allow,
    /// Drop it silently.
    Drop,
}

impl AclAction {
    const fn wire(self) -> i32 {
        match self {
            Self::Allow => 1,
            Self::Drop => 2,
        }
    }

    /// EasyTier lets through anything a matching rule or a default does not drop, so its `Noop` (0) is `Allow`.
    const fn from_wire(value: i32) -> Option<Self> {
        match value {
            0 | 1 => Some(Self::Allow),
            2 => Some(Self::Drop),
            _ => None,
        }
    }
}

/// The protocol a rule matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AclProtocol {
    /// Every protocol.
    Any,
    /// TCP.
    Tcp,
    /// UDP.
    Udp,
    /// ICMP (IPv4).
    Icmp,
    /// ICMPv6.
    Icmpv6,
}

impl AclProtocol {
    /// Every protocol, in the order the editor offers them.
    pub const ALL: [Self; 5] = [Self::Any, Self::Tcp, Self::Udp, Self::Icmp, Self::Icmpv6];

    /// Whether rules of this protocol can match ports.
    pub const fn has_ports(self) -> bool {
        matches!(self, Self::Any | Self::Tcp | Self::Udp)
    }

    /// Its name in the editor (`TCP`, `ICMPv6`); [`AclProtocol::Any`] is a word the UI translates.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Tcp => "TCP",
            Self::Udp => "UDP",
            Self::Icmp => "ICMP",
            Self::Icmpv6 => "ICMPv6",
        }
    }

    const fn wire(self) -> i32 {
        match self {
            Self::Tcp => 1,
            Self::Udp => 2,
            Self::Icmp => 3,
            Self::Icmpv6 => 4,
            Self::Any => 5,
        }
    }

    const fn from_wire(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Tcp),
            2 => Some(Self::Udp),
            3 => Some(Self::Icmp),
            4 => Some(Self::Icmpv6),
            5 => Some(Self::Any),
            _ => None,
        }
    }
}

/// Why a range was rejected. None repeats the rejected text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RangeError {
    /// Nothing was entered.
    #[error("this is empty")]
    Empty,
    /// Not an IP address or `address/prefix`.
    #[error("not an IP address or a network such as 10.144.144.0/24")]
    BadIp,
    /// A network written with bits set beyond its prefix.
    #[error("the address has bits set beyond the prefix; write the network address")]
    HostBits,
    /// Not a port or `first-last` with first ≤ last.
    #[error("not a port or a range such as 1000-2000")]
    BadPort,
}

/// An address or a network a rule matches: `10.144.144.2` (one host) or `10.144.144.0/24`, IPv4 or IPv6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IpRange(IpNet);

impl IpRange {
    /// Parses `text`: an address (a host) or a network written as its network address.
    ///
    /// # Errors
    ///
    /// [`RangeError::Empty`], [`RangeError::BadIp`], or [`RangeError::HostBits`] (EasyTier refuses host bits).
    pub fn parse(text: &str) -> Result<Self, RangeError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(RangeError::Empty);
        }
        if !text.contains('/') {
            let addr: IpAddr = text.parse().map_err(|_| RangeError::BadIp)?;
            return Ok(Self(IpNet::from(addr)));
        }
        let net: IpNet = text.parse().map_err(|_| RangeError::BadIp)?;
        if net.addr() != net.network() {
            return Err(RangeError::HostBits);
        }
        Ok(Self(net))
    }

    /// The network.
    pub const fn net(self) -> IpNet {
        self.0
    }
}

impl fmt::Display for IpRange {
    /// A single host without its prefix (`10.144.144.2`), a network with it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.prefix_len() == self.0.max_prefix_len() {
            write!(f, "{}", self.0.addr())
        } else {
            write!(f, "{}", self.0)
        }
    }
}

impl FromStr for IpRange {
    type Err = RangeError;

    fn from_str(text: &str) -> Result<Self, RangeError> {
        Self::parse(text)
    }
}

impl TryFrom<String> for IpRange {
    type Error = RangeError;

    fn try_from(text: String) -> Result<Self, RangeError> {
        Self::parse(&text)
    }
}

impl From<IpRange> for String {
    fn from(range: IpRange) -> Self {
        range.to_string()
    }
}

impl Serialize for IpRange {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for IpRange {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// A port or an inclusive range of ports: `80`, `1000-2000`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortRange {
    first: u16,
    last: u16,
}

impl PortRange {
    /// The range `first..=last`, or `None` when `first > last`.
    pub const fn new(first: u16, last: u16) -> Option<Self> {
        if first > last {
            None
        } else {
            Some(Self { first, last })
        }
    }

    /// Parses `80` or `1000-2000`.
    ///
    /// # Errors
    ///
    /// [`RangeError::Empty`] or [`RangeError::BadPort`].
    pub fn parse(text: &str) -> Result<Self, RangeError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(RangeError::Empty);
        }
        let port = |part: &str| part.trim().parse::<u16>().map_err(|_| RangeError::BadPort);
        let (first, last) = match text.split_once('-') {
            Some((first, last)) => (port(first)?, port(last)?),
            None => {
                let single = port(text)?;
                (single, single)
            }
        };
        Self::new(first, last).ok_or(RangeError::BadPort)
    }

    /// The first port.
    pub const fn first(self) -> u16 {
        self.first
    }

    /// The last port (the same as the first for a single port).
    pub const fn last(self) -> u16 {
        self.last
    }
}

impl fmt::Display for PortRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.first == self.last {
            write!(f, "{}", self.first)
        } else {
            write!(f, "{}-{}", self.first, self.last)
        }
    }
}

impl FromStr for PortRange {
    type Err = RangeError;

    fn from_str(text: &str) -> Result<Self, RangeError> {
        Self::parse(text)
    }
}

impl Serialize for PortRange {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for PortRange {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// Serde default for the switches that are on unless turned off.
const fn yes() -> bool {
    true
}

/// One rule: what it matches, and what happens to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AclRule {
    /// Its name (may be empty).
    #[serde(default)]
    pub name: String,
    /// What it is for (may be empty).
    #[serde(default)]
    pub description: String,
    /// Higher is checked first, 0 to [`MAX_PRIORITY`].
    #[serde(default)]
    pub priority: u32,
    /// Whether it is checked at all.
    #[serde(default = "yes")]
    pub enabled: bool,
    /// The protocol it matches.
    pub protocol: AclProtocol,
    /// Source addresses it matches; empty matches any.
    #[serde(default)]
    pub source_ips: Vec<IpRange>,
    /// Destination addresses it matches; empty matches any.
    #[serde(default)]
    pub destination_ips: Vec<IpRange>,
    /// Source ports it matches (TCP / UDP); empty matches any.
    #[serde(default)]
    pub source_ports: Vec<PortRange>,
    /// Destination ports it matches (TCP / UDP); empty matches any.
    #[serde(default)]
    pub destination_ports: Vec<PortRange>,
    /// What happens to matching traffic.
    pub action: AclAction,
    /// Packets per second it lets through; 0 for no limit.
    #[serde(default)]
    pub rate_limit: u32,
    /// Packets allowed above the rate in a burst; 0 for none.
    #[serde(default)]
    pub burst_limit: u32,
    /// Whether replies of an allowed connection are allowed too (connection tracking).
    #[serde(default)]
    pub stateful: bool,
    /// Groups the source must belong to; empty matches any.
    #[serde(default)]
    pub source_groups: Vec<String>,
    /// Groups the destination must belong to; empty matches any.
    #[serde(default)]
    pub destination_groups: Vec<String>,
}

impl AclRule {
    /// A rule named `name` that allows everything, enabled, at [`DEFAULT_PRIORITY`].
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            priority: DEFAULT_PRIORITY,
            enabled: true,
            protocol: AclProtocol::Any,
            source_ips: Vec::new(),
            destination_ips: Vec::new(),
            source_ports: Vec::new(),
            destination_ports: Vec::new(),
            action: AclAction::Allow,
            rate_limit: 0,
            burst_limit: 0,
            stateful: false,
            source_groups: Vec::new(),
            destination_groups: Vec::new(),
        }
    }
}

/// A chain: rules for one direction of traffic, and what happens when none matches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AclChain {
    /// Its name (may be empty).
    #[serde(default)]
    pub name: String,
    /// What it is for (may be empty).
    #[serde(default)]
    pub description: String,
    /// The traffic it applies to.
    pub kind: ChainKind,
    /// Whether it applies at all.
    #[serde(default = "yes")]
    pub enabled: bool,
    /// What happens when no rule matches.
    pub default_action: AclAction,
    /// Its rules.
    #[serde(default)]
    pub rules: Vec<AclRule>,
}

impl AclChain {
    /// An enabled chain of `kind` named `name`, with no rules, that allows by default.
    pub fn new(kind: ChainKind, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            kind,
            enabled: true,
            default_action: AclAction::Allow,
            rules: Vec::new(),
        }
    }
}

/// An ACL without its secrets: what a profile stores.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Acl {
    /// The chains.
    #[serde(default)]
    pub chains: Vec<AclChain>,
    /// The groups this node knows (their secrets are in [`GroupSecrets`]).
    #[serde(default)]
    pub declared_groups: Vec<String>,
    /// The groups this node belongs to.
    #[serde(default)]
    pub joined_groups: Vec<String>,
}

/// The secrets of declared groups, by group name.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct GroupSecrets(BTreeMap<String, Secret>);

impl fmt::Debug for GroupSecrets {
    /// The names, never the secrets.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.0.keys()).finish()
    }
}

impl GroupSecrets {
    /// The secret of group `name`.
    pub fn get(&self, name: &str) -> Option<&Secret> {
        self.0.get(name)
    }

    /// Sets the secret of group `name`.
    pub fn insert(&mut self, name: impl Into<String>, secret: Secret) {
        self.0.insert(name.into(), secret);
    }

    /// Forgets the secret of group `name`.
    pub fn remove(&mut self, name: &str) -> Option<Secret> {
        self.0.remove(name)
    }

    /// Moves the secret of `old` to `new` (a renamed group).
    pub fn rename(&mut self, old: &str, new: &str) {
        if let Some(secret) = self.0.remove(old) {
            self.0.insert(new.to_owned(), secret);
        }
    }

    /// Keeps only the secrets of groups in `names`.
    pub fn retain_names(&mut self, names: &[String]) {
        self.0.retain(|name, _| names.contains(name));
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The secrets as one keychain value (a JSON object of name to secret).
    ///
    /// # Errors
    ///
    /// [`AclError`] at [`AclPlace::Acl`] if the serializer refuses (it does not for strings).
    pub fn to_keychain(&self) -> Result<Secret, AclError> {
        let plain: BTreeMap<&str, &str> = self
            .0
            .iter()
            .map(|(name, secret)| (name.as_str(), secret.expose()))
            .collect();
        serde_json::to_string(&plain)
            .map(Secret::new)
            .map_err(|_| AclError::new(AclPlace::Acl, AclProblem::Unreadable))
    }

    /// The secrets from a keychain value written by [`GroupSecrets::to_keychain`].
    ///
    /// # Errors
    ///
    /// [`AclProblem::Unreadable`] when the value is not such an object.
    pub fn from_keychain(value: &Secret) -> Result<Self, AclError> {
        let plain: BTreeMap<String, String> = serde_json::from_str(value.expose())
            .map_err(|_| AclError::new(AclPlace::Acl, AclProblem::Unreadable))?;
        Ok(Self(
            plain
                .into_iter()
                .map(|(name, secret)| (name, Secret::new(secret)))
                .collect(),
        ))
    }
}

/// An ACL with the secrets of the groups it declares: what an editor and a config file hold.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AclConfig {
    /// The chains and group names.
    pub acl: Acl,
    /// The secrets of the declared groups.
    pub group_secrets: GroupSecrets,
}

/// Where in an ACL a problem is. Indexes count from 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AclPlace {
    /// The ACL as a whole.
    Acl,
    /// A chain.
    Chain(usize),
    /// A rule of a chain.
    Rule {
        /// The chain.
        chain: usize,
        /// The rule.
        rule: usize,
    },
    /// A declared group.
    DeclaredGroup(usize),
    /// A joined group.
    JoinedGroup(usize),
}

impl fmt::Display for AclPlace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Acl => f.write_str("acl"),
            Self::Chain(chain) => write!(f, "acl chain {}", chain.saturating_add(1)),
            Self::Rule { chain, rule } => write!(
                f,
                "acl chain {} rule {}",
                chain.saturating_add(1),
                rule.saturating_add(1)
            ),
            Self::DeclaredGroup(group) => write!(f, "acl group {}", group.saturating_add(1)),
            Self::JoinedGroup(group) => write!(f, "acl joined group {}", group.saturating_add(1)),
        }
    }
}

/// What is wrong at an [`AclPlace`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AclProblem {
    /// More entries than allowed.
    TooMany {
        /// The limit.
        max: usize,
    },
    /// A name or description longer than allowed, in characters.
    TooLong {
        /// The limit.
        max: usize,
    },
    /// A name that must be given is empty, or contains control characters.
    BadName,
    /// The same group name twice.
    Duplicate,
    /// A declared group without a secret.
    MissingSecret,
    /// A priority above [`MAX_PRIORITY`].
    Priority,
    /// Ports on a rule whose protocol has none (ICMP).
    PortsWithoutPorts,
    /// An address, network or port range EasyTier cannot read.
    Range(RangeError),
    /// A value EasyTier's ACL does not define (an unknown chain type, action or protocol), or text that is not an
    /// ACL at all.
    Unreadable,
}

/// A problem of an ACL: where, and what.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{place}: {problem:?}")]
pub struct AclError {
    /// Where.
    pub place: AclPlace,
    /// What.
    pub problem: AclProblem,
}

impl AclError {
    /// The problem `problem` at `place`.
    pub const fn new(place: AclPlace, problem: AclProblem) -> Self {
        Self { place, problem }
    }
}

/// Whether `name` is usable as a group name: not empty, no control characters.
fn bad_name(name: &str) -> bool {
    name.trim().is_empty() || name.chars().any(char::is_control)
}

fn check_len(text: &str, max: usize, place: AclPlace) -> Result<(), AclError> {
    if text.chars().count() > max {
        return Err(AclError::new(place, AclProblem::TooLong { max }));
    }
    Ok(())
}

fn check_count(len: usize, max: usize, place: AclPlace) -> Result<(), AclError> {
    if len > max {
        return Err(AclError::new(place, AclProblem::TooMany { max }));
    }
    Ok(())
}

impl AclRule {
    fn check(&self, place: AclPlace) -> Result<(), AclError> {
        check_len(&self.name, MAX_NAME_CHARS, place)?;
        check_len(&self.description, MAX_DESCRIPTION_CHARS, place)?;
        if self.priority > MAX_PRIORITY {
            return Err(AclError::new(place, AclProblem::Priority));
        }
        for len in [
            self.source_ips.len(),
            self.destination_ips.len(),
            self.source_ports.len(),
            self.destination_ports.len(),
            self.source_groups.len(),
            self.destination_groups.len(),
        ] {
            check_count(len, MAX_ENTRIES, place)?;
        }
        if !self.protocol.has_ports()
            && !(self.source_ports.is_empty() && self.destination_ports.is_empty())
        {
            return Err(AclError::new(place, AclProblem::PortsWithoutPorts));
        }
        for group in self.source_groups.iter().chain(&self.destination_groups) {
            if bad_name(group) {
                return Err(AclError::new(place, AclProblem::BadName));
            }
            check_len(group, MAX_NAME_CHARS, place)?;
        }
        Ok(())
    }
}

impl Acl {
    /// Checks limits and names. The group secrets are checked by [`AclConfig::check`].
    ///
    /// # Errors
    ///
    /// The first problem found, in order: chains, their rules, declared groups, joined groups.
    pub fn check(&self) -> Result<(), AclError> {
        check_count(self.chains.len(), MAX_CHAINS, AclPlace::Acl)?;
        for (index, chain) in self.chains.iter().enumerate() {
            let place = AclPlace::Chain(index);
            check_len(&chain.name, MAX_NAME_CHARS, place)?;
            check_len(&chain.description, MAX_DESCRIPTION_CHARS, place)?;
            check_count(chain.rules.len(), MAX_RULES, place)?;
            for (rule_index, rule) in chain.rules.iter().enumerate() {
                rule.check(AclPlace::Rule {
                    chain: index,
                    rule: rule_index,
                })?;
            }
        }
        check_count(self.declared_groups.len(), MAX_ENTRIES, AclPlace::Acl)?;
        for (index, name) in self.declared_groups.iter().enumerate() {
            let place = AclPlace::DeclaredGroup(index);
            if bad_name(name) {
                return Err(AclError::new(place, AclProblem::BadName));
            }
            check_len(name, MAX_NAME_CHARS, place)?;
            if self
                .declared_groups
                .iter()
                .take(index)
                .any(|earlier| earlier == name)
            {
                return Err(AclError::new(place, AclProblem::Duplicate));
            }
        }
        check_count(self.joined_groups.len(), MAX_ENTRIES, AclPlace::Acl)?;
        for (index, name) in self.joined_groups.iter().enumerate() {
            let place = AclPlace::JoinedGroup(index);
            if bad_name(name) {
                return Err(AclError::new(place, AclProblem::BadName));
            }
            check_len(name, MAX_NAME_CHARS, place)?;
            if self
                .joined_groups
                .iter()
                .take(index)
                .any(|earlier| earlier == name)
            {
                return Err(AclError::new(place, AclProblem::Duplicate));
            }
        }
        Ok(())
    }
}

impl AclConfig {
    /// Checks the ACL, and that every declared group has a secret.
    ///
    /// # Errors
    ///
    /// The first problem [`Acl::check`] finds, then [`AclProblem::MissingSecret`] at the first declared group
    /// without one.
    pub fn check(&self) -> Result<(), AclError> {
        self.acl.check()?;
        for (index, name) in self.acl.declared_groups.iter().enumerate() {
            if self.group_secrets.get(name).is_none_or(Secret::is_empty) {
                return Err(AclError::new(
                    AclPlace::DeclaredGroup(index),
                    AclProblem::MissingSecret,
                ));
            }
        }
        Ok(())
    }

    /// EasyTier's shape of this ACL; group secrets in clear, so the result belongs only in a config file or a
    /// request body.
    pub fn to_wire(&self) -> wire::Acl {
        let acl = &self.acl;
        let group = (!acl.declared_groups.is_empty() || !acl.joined_groups.is_empty()).then(|| {
            wire::GroupInfo {
                declares: acl
                    .declared_groups
                    .iter()
                    .map(|name| wire::GroupIdentity {
                        group_name: name.clone(),
                        group_secret: self
                            .group_secrets
                            .get(name)
                            .map(|secret| secret.expose().to_owned())
                            .unwrap_or_default(),
                    })
                    .collect(),
                members: acl.joined_groups.clone(),
            }
        });
        wire::Acl {
            acl_v1: Some(wire::AclV1 {
                chains: acl.chains.iter().map(chain_to_wire).collect(),
                group,
            }),
        }
    }

    /// Reads EasyTier's shape (untrusted: an imported file, a server's JSON), then checks it with
    /// [`Acl::check`]. A missing `acl_v1` is an empty ACL; declared groups without a secret are kept (the check of
    /// [`AclConfig::check`] names them).
    ///
    /// # Errors
    ///
    /// [`AclProblem::Unreadable`] for an unknown chain type, action or protocol; [`AclProblem::Range`] for an
    /// address or port EasyTier cannot read; then what [`Acl::check`] finds.
    pub fn from_wire(wire: &wire::Acl) -> Result<Self, AclError> {
        let Some(v1) = &wire.acl_v1 else {
            return Ok(Self::default());
        };
        let mut chains = Vec::with_capacity(v1.chains.len().min(MAX_CHAINS));
        check_count(v1.chains.len(), MAX_CHAINS, AclPlace::Acl)?;
        for (index, chain) in v1.chains.iter().enumerate() {
            chains.push(chain_from_wire(index, chain)?);
        }
        let mut config = Self {
            acl: Acl {
                chains,
                declared_groups: Vec::new(),
                joined_groups: Vec::new(),
            },
            group_secrets: GroupSecrets::default(),
        };
        if let Some(group) = &v1.group {
            check_count(group.declares.len(), MAX_ENTRIES, AclPlace::Acl)?;
            check_count(group.members.len(), MAX_ENTRIES, AclPlace::Acl)?;
            for declare in &group.declares {
                config.acl.declared_groups.push(declare.group_name.clone());
                if !declare.group_secret.is_empty() {
                    config.group_secrets.insert(
                        declare.group_name.clone(),
                        Secret::new(declare.group_secret.clone()),
                    );
                }
            }
            config.acl.joined_groups.clone_from(&group.members);
        }
        config.acl.check()?;
        Ok(config)
    }
}

fn chain_to_wire(chain: &AclChain) -> wire::Chain {
    wire::Chain {
        name: chain.name.clone(),
        chain_type: chain.kind.wire(),
        description: chain.description.clone(),
        enabled: chain.enabled,
        default_action: chain.default_action.wire(),
        rules: chain.rules.iter().map(rule_to_wire).collect(),
    }
}

fn texts<T: ToString>(items: &[T]) -> Vec<String> {
    items.iter().map(ToString::to_string).collect()
}

fn rule_to_wire(rule: &AclRule) -> wire::Rule {
    wire::Rule {
        name: rule.name.clone(),
        description: rule.description.clone(),
        priority: rule.priority,
        enabled: rule.enabled,
        protocol: rule.protocol.wire(),
        ports: texts(&rule.destination_ports),
        source_ips: texts(&rule.source_ips),
        destination_ips: texts(&rule.destination_ips),
        source_ports: texts(&rule.source_ports),
        action: rule.action.wire(),
        rate_limit: rule.rate_limit,
        burst_limit: rule.burst_limit,
        stateful: rule.stateful,
        source_groups: rule.source_groups.clone(),
        destination_groups: rule.destination_groups.clone(),
    }
}

fn chain_from_wire(index: usize, chain: &wire::Chain) -> Result<AclChain, AclError> {
    let place = AclPlace::Chain(index);
    let unreadable = AclError::new(place, AclProblem::Unreadable);
    let kind = ChainKind::from_wire(chain.chain_type).ok_or(unreadable)?;
    let default_action = AclAction::from_wire(chain.default_action).ok_or(unreadable)?;
    check_count(chain.rules.len(), MAX_RULES, place)?;
    let mut rules = Vec::with_capacity(chain.rules.len());
    for (rule_index, rule) in chain.rules.iter().enumerate() {
        rules.push(rule_from_wire(
            AclPlace::Rule {
                chain: index,
                rule: rule_index,
            },
            rule,
        )?);
    }
    Ok(AclChain {
        name: chain.name.clone(),
        description: chain.description.clone(),
        kind,
        enabled: chain.enabled,
        default_action,
        rules,
    })
}

fn ranges<T>(
    place: AclPlace,
    items: &[String],
    parse: fn(&str) -> Result<T, RangeError>,
) -> Result<Vec<T>, AclError> {
    check_count(items.len(), MAX_ENTRIES, place)?;
    items
        .iter()
        .map(|item| parse(item).map_err(|error| AclError::new(place, AclProblem::Range(error))))
        .collect()
}

fn rule_from_wire(place: AclPlace, rule: &wire::Rule) -> Result<AclRule, AclError> {
    let unreadable = AclError::new(place, AclProblem::Unreadable);
    Ok(AclRule {
        name: rule.name.clone(),
        description: rule.description.clone(),
        priority: rule.priority,
        enabled: rule.enabled,
        protocol: AclProtocol::from_wire(rule.protocol).ok_or(unreadable)?,
        source_ips: ranges(place, &rule.source_ips, IpRange::parse)?,
        destination_ips: ranges(place, &rule.destination_ips, IpRange::parse)?,
        source_ports: ranges(place, &rule.source_ports, PortRange::parse)?,
        destination_ports: ranges(place, &rule.ports, PortRange::parse)?,
        action: AclAction::from_wire(rule.action).ok_or(unreadable)?,
        rate_limit: rule.rate_limit,
        burst_limit: rule.burst_limit,
        stateful: rule.stateful,
        source_groups: rule.source_groups.clone(),
        destination_groups: rule.destination_groups.clone(),
    })
}

/// EasyTier's ACL as it is written in the TOML config file (`[acl.acl_v1]`, `[[acl.acl_v1.chains]]`, ...) and in
/// easytier-web's `NetworkConfig.acl` JSON: protobuf messages whose enums are numbers. Read tolerantly (every field
/// defaults). None of the structs derives `Debug`: group secrets are in clear.
pub mod wire {
    use serde::{Deserialize, Serialize};

    /// The `acl` table / object.
    #[derive(Clone, Default, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Acl {
        /// Version 1, the only one.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub acl_v1: Option<AclV1>,
    }

    /// `acl_v1`: tables only, chains first.
    #[derive(Clone, Default, Serialize, Deserialize)]
    #[serde(default)]
    pub struct AclV1 {
        /// The chains.
        pub chains: Vec<Chain>,
        /// The groups.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub group: Option<GroupInfo>,
    }

    /// A chain: plain values first, its rules (tables) last, as TOML requires.
    #[derive(Clone, Default, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Chain {
        /// Name.
        pub name: String,
        /// 1 inbound, 2 outbound, 3 forward.
        pub chain_type: i32,
        /// Description.
        pub description: String,
        /// Enabled.
        pub enabled: bool,
        /// 1 allow, 2 drop (0 is treated as allow).
        pub default_action: i32,
        /// Rules.
        pub rules: Vec<Rule>,
    }

    /// A rule.
    #[derive(Clone, Default, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Rule {
        /// Name.
        pub name: String,
        /// Description.
        pub description: String,
        /// Priority, higher first.
        pub priority: u32,
        /// Enabled.
        pub enabled: bool,
        /// 1 TCP, 2 UDP, 3 ICMP, 4 ICMPv6, 5 any.
        pub protocol: i32,
        /// Destination ports (`80`, `1000-2000`).
        pub ports: Vec<String>,
        /// Source addresses or networks.
        pub source_ips: Vec<String>,
        /// Destination addresses or networks.
        pub destination_ips: Vec<String>,
        /// Source ports.
        pub source_ports: Vec<String>,
        /// 1 allow, 2 drop.
        pub action: i32,
        /// Packets per second, 0 for none.
        pub rate_limit: u32,
        /// Burst, 0 for none.
        pub burst_limit: u32,
        /// Connection tracking.
        pub stateful: bool,
        /// Source groups.
        pub source_groups: Vec<String>,
        /// Destination groups.
        pub destination_groups: Vec<String>,
    }

    /// `group`.
    #[derive(Clone, Default, Serialize, Deserialize)]
    #[serde(default)]
    pub struct GroupInfo {
        /// Groups this node knows, with their secrets.
        pub declares: Vec<GroupIdentity>,
        /// Groups this node belongs to.
        pub members: Vec<String>,
    }

    /// One declared group.
    #[derive(Clone, Default, Serialize, Deserialize)]
    #[serde(default)]
    pub struct GroupIdentity {
        /// Name.
        pub group_name: String,
        /// Secret, in clear.
        pub group_secret: String,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> AclConfig {
        let mut rule = AclRule::new("web");
        rule.protocol = AclProtocol::Tcp;
        rule.destination_ports = vec![
            PortRange::parse("80").unwrap(),
            PortRange::parse("8000-8080").unwrap(),
        ];
        rule.source_ips = vec![
            IpRange::parse("10.144.144.0/24").unwrap(),
            IpRange::parse("fd00::1").unwrap(),
        ];
        rule.action = AclAction::Drop;
        rule.source_groups = vec!["admins".into()];
        let mut chain = AclChain::new(ChainKind::Inbound, "Inbound");
        chain.default_action = AclAction::Drop;
        chain.rules.push(rule);
        let mut secrets = GroupSecrets::default();
        secrets.insert("admins", Secret::new("example-group-secret"));
        AclConfig {
            acl: Acl {
                chains: vec![chain],
                declared_groups: vec!["admins".into()],
                joined_groups: vec!["admins".into()],
            },
            group_secrets: secrets,
        }
    }

    #[test]
    fn ranges_parse_what_easytier_reads_and_refuse_the_rest() {
        assert_eq!(
            IpRange::parse(" 10.0.0.1 ").unwrap().to_string(),
            "10.0.0.1"
        );
        assert_eq!(
            IpRange::parse("10.0.0.0/8").unwrap().to_string(),
            "10.0.0.0/8"
        );
        assert_eq!(
            IpRange::parse("fd00::/64").unwrap().to_string(),
            "fd00::/64"
        );
        assert_eq!(IpRange::parse("10.0.0.1/24"), Err(RangeError::HostBits));
        assert_eq!(IpRange::parse("10.0.0/24"), Err(RangeError::BadIp));
        assert_eq!(IpRange::parse(""), Err(RangeError::Empty));
        assert_eq!(PortRange::parse("0-65535").unwrap().to_string(), "0-65535");
        assert_eq!(PortRange::parse(" 443 ").unwrap().to_string(), "443");
        assert_eq!(PortRange::parse("2000-1000"), Err(RangeError::BadPort));
        assert_eq!(PortRange::parse("70000"), Err(RangeError::BadPort));
        assert_eq!(PortRange::parse("1-2-3"), Err(RangeError::BadPort));
    }

    #[test]
    fn wire_round_trips_with_secrets() {
        let config = sample();
        let back = AclConfig::from_wire(&config.to_wire()).unwrap();
        assert_eq!(back, config);
    }

    #[test]
    fn wire_numbers_match_easytier() {
        let wire = sample().to_wire();
        let json = serde_json::to_value(&wire).unwrap();
        let chain = &json["acl_v1"]["chains"][0];
        assert_eq!(chain["chain_type"], 1);
        assert_eq!(chain["default_action"], 2);
        assert_eq!(chain["rules"][0]["protocol"], 1);
        assert_eq!(chain["rules"][0]["action"], 2);
        assert_eq!(
            chain["rules"][0]["ports"],
            serde_json::json!(["80", "8000-8080"])
        );
        assert_eq!(
            json["acl_v1"]["group"]["declares"][0]["group_secret"],
            "example-group-secret"
        );
    }

    #[test]
    fn upstream_toml_reads_with_defaults() {
        // EasyTier 2.6.4's own test config (`common/config.rs`).
        let text = r#"
[[acl_v1.chains]]
name = "subnet_proxy_protect"
chain_type = 3
enabled = true
default_action = 2

[[acl_v1.chains.rules]]
name = "allow_my_devices"
priority = 1000
action = 1
source_ips = ["10.172.192.2/32"]
protocol = 5
enabled = true
"#;
        let wire: wire::Acl = toml::from_str(text).unwrap();
        let config = AclConfig::from_wire(&wire).unwrap();
        let chain = &config.acl.chains[0];
        assert_eq!(chain.kind, ChainKind::Forward);
        assert_eq!(chain.default_action, AclAction::Drop);
        assert_eq!(chain.rules[0].protocol, AclProtocol::Any);
        assert_eq!(chain.rules[0].source_ips[0].to_string(), "10.172.192.2");
        let rendered = toml::to_string(&config.to_wire()).unwrap();
        let again: wire::Acl = toml::from_str(&rendered).unwrap();
        assert_eq!(AclConfig::from_wire(&again).unwrap(), config);
    }

    #[test]
    fn unknown_values_and_bad_ranges_name_their_place() {
        let mut wire = sample().to_wire();
        if let Some(v1) = wire.acl_v1.as_mut() {
            v1.chains[0].rules[0].protocol = 9;
        }
        assert_eq!(
            AclConfig::from_wire(&wire),
            Err(AclError::new(
                AclPlace::Rule { chain: 0, rule: 0 },
                AclProblem::Unreadable
            ))
        );
        let mut wire = sample().to_wire();
        if let Some(v1) = wire.acl_v1.as_mut() {
            v1.chains[0].rules[0].source_ips = vec!["10.0.0.1/8".into()];
        }
        assert_eq!(
            AclConfig::from_wire(&wire),
            Err(AclError::new(
                AclPlace::Rule { chain: 0, rule: 0 },
                AclProblem::Range(RangeError::HostBits)
            ))
        );
    }

    #[test]
    fn checks_secrets_names_and_ports() {
        let mut config = sample();
        assert_eq!(config.check(), Ok(()));
        config.group_secrets.remove("admins");
        assert_eq!(
            config.check(),
            Err(AclError::new(
                AclPlace::DeclaredGroup(0),
                AclProblem::MissingSecret
            ))
        );
        let mut config = sample();
        config.acl.declared_groups.push("admins".into());
        assert_eq!(
            config.acl.check(),
            Err(AclError::new(
                AclPlace::DeclaredGroup(1),
                AclProblem::Duplicate
            ))
        );
        let mut config = sample();
        config.acl.chains[0].rules[0].protocol = AclProtocol::Icmp;
        assert_eq!(
            config.acl.check(),
            Err(AclError::new(
                AclPlace::Rule { chain: 0, rule: 0 },
                AclProblem::PortsWithoutPorts
            ))
        );
    }

    #[test]
    fn keychain_value_round_trips_and_debug_hides_secrets() {
        let secrets = sample().group_secrets;
        let stored = secrets.to_keychain().unwrap();
        assert_eq!(GroupSecrets::from_keychain(&stored).unwrap(), secrets);
        assert!(!format!("{secrets:?}").contains("example-group-secret"));
        assert!(GroupSecrets::from_keychain(&Secret::new("not json")).is_err());
    }
}
