//! The roles an installation combines.

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
}
