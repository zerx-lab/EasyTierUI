//! The roles an installation combines, and the work mode that picks which of them the shell shows.

use super::platform::Platform;

/// Something this installation does; roles combine, switching one never loses another's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    /// This device in the mesh (`easytier-core`).
    Node,
    /// Managing an easytier-web server's machines, networks and credentials.
    Console,
    /// Running easytier-web, or deploying it to a server.
    Host,
}

impl Role {
    /// Every role, in the order the shell lists them.
    pub const ALL: [Role; 3] = [Role::Node, Role::Console, Role::Host];

    /// Its key in a stored [`WorkMode`].
    fn key(self) -> &'static str {
        match self {
            Role::Node => "node",
            Role::Console => "console",
            Role::Host => "host",
        }
    }

    fn from_key(key: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|role| role.key() == key)
    }
}

/// The roles the shell shows: never empty, so there is always a page to open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoleSet {
    node: bool,
    console: bool,
    host: bool,
}

impl RoleSet {
    /// Every role.
    pub const ALL: RoleSet = RoleSet {
        node: true,
        console: true,
        host: true,
    };
    /// The node alone.
    pub const NODE: RoleSet = RoleSet {
        node: true,
        console: false,
        host: false,
    };

    /// The set of `roles`; `None` when there are none.
    pub fn of(roles: impl IntoIterator<Item = Role>) -> Option<RoleSet> {
        let mut set = RoleSet {
            node: false,
            console: false,
            host: false,
        };
        for role in roles {
            *set.slot(role) = true;
        }
        (!set.is_empty()).then_some(set)
    }

    /// Whether `role` is in the set.
    pub fn contains(self, role: Role) -> bool {
        match role {
            Role::Node => self.node,
            Role::Console => self.console,
            Role::Host => self.host,
        }
    }

    /// The set with `role` added (`on`) or removed; `None` when that would leave it empty.
    pub fn with(mut self, role: Role, on: bool) -> Option<RoleSet> {
        *self.slot(role) = on;
        (!self.is_empty()).then_some(self)
    }

    /// Its roles, in the order the shell lists them.
    pub fn roles(self) -> Vec<Role> {
        Role::ALL
            .into_iter()
            .filter(|role| self.contains(*role))
            .collect()
    }

    fn is_empty(self) -> bool {
        !(self.node || self.console || self.host)
    }

    fn slot(&mut self, role: Role) -> &mut bool {
        match role {
            Role::Node => &mut self.node,
            Role::Console => &mut self.console,
            Role::Host => &mut self.host,
        }
    }
}

/// How the user uses this installation, chosen on the first run and in Settings: which roles the shell shows.
/// Hiding a role stops nothing and forgets nothing; the user changes the mode at any time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkMode {
    /// A device that joins networks: the node only.
    Client,
    /// Everything: the node, consoles of easytier-web servers, hosting and deploying a control plane.
    Control,
    /// The roles the user picked.
    Custom(RoleSet),
}

impl WorkMode {
    /// The two presets the first run offers.
    pub const PRESETS: [WorkMode; 2] = [WorkMode::Client, WorkMode::Control];

    /// The roles the shell shows in this mode.
    pub fn roles(self) -> RoleSet {
        match self {
            WorkMode::Client => RoleSet::NODE,
            WorkMode::Control => RoleSet::ALL,
            WorkMode::Custom(roles) => roles,
        }
    }

    /// The mode a fresh installation has on `platform` without asking; `None` asks on the first run. The web build
    /// is a server's dashboard, a control plane's own: it never asks.
    pub fn implied_by(platform: Platform) -> Option<WorkMode> {
        match platform {
            Platform::Web => Some(WorkMode::Control),
            Platform::MacOs
            | Platform::Windows
            | Platform::Linux
            | Platform::Ios
            | Platform::Android => None,
        }
    }

    /// How it is stored: `client`, `control`, or `custom:` and its roles' keys (`custom:node,console`).
    pub fn as_key(self) -> String {
        match self {
            WorkMode::Client => "client".to_owned(),
            WorkMode::Control => "control".to_owned(),
            WorkMode::Custom(roles) => {
                let keys: Vec<_> = roles.roles().into_iter().map(Role::key).collect();
                format!("custom:{}", keys.join(","))
            }
        }
    }

    /// The mode stored as `key`; `None` for anything [`Self::as_key`] does not write.
    pub fn from_key(key: &str) -> Option<WorkMode> {
        match key {
            "client" => Some(WorkMode::Client),
            "control" => Some(WorkMode::Control),
            _ => {
                let roles = key.strip_prefix("custom:")?;
                let parsed: Option<Vec<Role>> = roles.split(',').map(Role::from_key).collect();
                RoleSet::of(parsed?).map(WorkMode::Custom)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_survives_storage() {
        let custom = RoleSet::of([Role::Console, Role::Host]).unwrap();
        for mode in [
            WorkMode::Client,
            WorkMode::Control,
            WorkMode::Custom(custom),
        ] {
            assert_eq!(WorkMode::from_key(&mode.as_key()), Some(mode));
        }
        assert_eq!(WorkMode::Custom(custom).as_key(), "custom:console,host");
    }

    #[test]
    fn a_stored_mode_that_is_not_one_is_refused() {
        for key in [
            "",
            "Client",
            "custom:",
            "custom:node,",
            "custom:node,admin",
            "all",
        ] {
            assert_eq!(WorkMode::from_key(key), None, "{key:?}");
        }
    }

    #[test]
    fn a_role_set_is_never_empty() {
        assert_eq!(RoleSet::of([]), None);
        assert_eq!(RoleSet::NODE.with(Role::Node, false), None);
        let both = RoleSet::NODE.with(Role::Host, true).unwrap();
        assert_eq!(both.roles(), vec![Role::Node, Role::Host]);
        assert_eq!(
            both.with(Role::Node, false).unwrap().roles(),
            vec![Role::Host]
        );
    }

    #[test]
    fn only_the_web_build_skips_the_first_run_choice() {
        assert_eq!(WorkMode::implied_by(Platform::Web), Some(WorkMode::Control));
        for platform in [
            Platform::MacOs,
            Platform::Windows,
            Platform::Linux,
            Platform::Ios,
            Platform::Android,
        ] {
            assert_eq!(WorkMode::implied_by(platform), None);
        }
    }
}
