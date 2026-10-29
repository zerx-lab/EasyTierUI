//! Console role: servers, signing in, machines, server networks and configs, credentials.
//!
//! One field per string; `EN` and `ZH_CN` must both set every field (the struct makes a missing one a compile
//! error). Templates use `{name}` placeholders filled by [`super::fill`]. In Chinese a machine is 设备 (as the
//! page title is), a network 网络, a node 节点, a peer 对等节点, a shared node 共享节点.

/// Strings of this area in one language.
#[derive(Debug)]
pub struct ConsoleStrings {
    // --- Signing in ---
    /// The username field.
    pub username: &'static str,
    /// The password field.
    pub password: &'static str,
    /// The sign-in button.
    pub sign_in: &'static str,
    /// Shown beside the sign-in button while signing in.
    pub signing_in: &'static str,
    /// The sign-in form's lead line.
    pub sign_in_title: &'static str,
    /// The session ended: sign in again.
    pub sign_in_expired: &'static str,
    /// The password was changed, which ends the session.
    pub sign_in_password_changed: &'static str,
    /// The server did not answer.
    pub unreachable: &'static str,
    /// The username or password is empty.
    pub sign_in_missing: &'static str,
    /// easytier-web receives an MD5 of the password: non-loopback servers must use https.
    pub sign_in_digest_note: &'static str,
    /// The session cannot outlive the app on this device.
    pub session_not_kept: &'static str,
    /// Lamp word: the server did not answer.
    pub state_unreachable: &'static str,
    /// Lamp word: the session ended.
    pub state_expired: &'static str,

    // --- The server and its account ---
    /// Removes a server from the app.
    pub remove_server: &'static str,
    /// Signs out of a server.
    pub sign_out: &'static str,
    /// Confirmation title; `{server}`.
    pub sign_out_title: &'static str,
    /// Confirmation body.
    pub sign_out_body: &'static str,
    /// Confirmation title; `{server}`.
    pub remove_title: &'static str,
    /// Confirmation body.
    pub remove_body: &'static str,
    /// Changes the account's password (button and dialog title).
    pub change_password: &'static str,
    /// What happens after the password changed.
    pub password_note: &'static str,
    /// The new password field.
    pub password_new: &'static str,
    /// The repeated new password field.
    pub password_confirm: &'static str,
    /// The new password is empty.
    pub password_empty: &'static str,
    /// The two entries differ.
    pub password_mismatch: &'static str,
    /// "Updated {age} s ago": how old the page's data is; `{age}` is whole seconds.
    pub read_age: &'static str,
    /// The data was read just now.
    pub read_now: &'static str,

    // --- Adding a server ---
    /// The dialog's title.
    pub add_server_title: &'static str,
    /// The address field.
    pub add_server_url: &'static str,
    /// Helper under the address field.
    pub add_server_url_help: &'static str,
    /// The name field.
    pub add_server_name: &'static str,
    /// Helper under the name field.
    pub add_server_name_help: &'static str,
    /// The dialog's confirm button.
    pub add_server_submit: &'static str,
    /// Plain http to another machine.
    pub add_error_insecure: &'static str,
    /// Not an address.
    pub add_error_invalid: &'static str,
    /// Already listed; `{name}`.
    pub add_error_duplicate: &'static str,
    /// No ids left.
    pub add_error_full: &'static str,

    // --- Failed requests ---
    /// A 401.
    pub err_session_ended: &'static str,
    /// A refused login.
    pub err_bad_credentials: &'static str,
    /// The machine left the server.
    pub err_machine_gone: &'static str,
    /// The server refused; `{message}` is its own words.
    pub err_rejected: &'static str,
    /// No answer in time.
    pub err_timeout: &'static str,
    /// easytier-web could not reach the machine's core (it timed out or the call failed).
    pub err_machine_no_answer: &'static str,
    /// No answer at all; `{message}`.
    pub err_network: &'static str,
    /// The reply was not what this client reads.
    pub err_decode: &'static str,
    /// Plain http to another machine.
    pub err_insecure: &'static str,
    /// The server address is not usable.
    pub err_invalid_address: &'static str,
    /// The request was inconsistent; `{message}`.
    pub err_invalid_request: &'static str,
    /// A read failed (the lead above its reason).
    pub load_failed: &'static str,
    /// A refresh failed; what is shown is the last reading.
    pub refresh_failed: &'static str,
    /// An enable failed; `{name}`.
    pub failed_enable: &'static str,
    /// A disable failed; `{name}`.
    pub failed_disable: &'static str,
    /// A delete failed; `{name}`.
    pub failed_delete: &'static str,
    /// A revoke failed; `{name}`.
    pub failed_revoke: &'static str,

    // --- Page chrome ---
    /// Placeholder of the machine search field.
    pub search_machines: &'static str,
    /// Placeholder of the network search field.
    pub search_networks: &'static str,
    /// Clears a search field.
    pub search_clear: &'static str,
    /// A search matches nothing: the lead.
    pub no_match_title: &'static str,
    /// A search matches nothing: what to do.
    pub no_match_body: &'static str,
    /// Names the row menus' button.
    pub more_actions: &'static str,
    /// Names the account menu's button.
    pub account_menu: &'static str,
    /// Names the machine drawer's tabs.
    pub tabs_label: &'static str,
    /// Tab: a machine's networks.
    pub tab_networks: &'static str,
    /// Tab: the peers of one of its networks.
    pub tab_peers: &'static str,
    /// Asks to choose a network on the peers tab.
    pub peers_pick_network: &'static str,
    /// Kind chip of the credential drawer.
    pub kind_credential: &'static str,
    /// Summary: machines enrolled.
    pub sum_machines: &'static str,
    /// Summary: networks running across the machines.
    pub sum_networks_running: &'static str,
    /// Summary: core versions in use.
    pub sum_versions: &'static str,
    /// Summary: distinct network names.
    pub sum_network_names: &'static str,
    /// Summary: network instances across the machines.
    pub sum_instances: &'static str,
    /// Summary: peers across the networks.
    pub sum_peers: &'static str,
    /// Column: a machine.
    pub col_machine: &'static str,
    /// Column: status.
    pub col_status: &'static str,
    /// Column: when the machine last reported.
    pub col_reported: &'static str,
    /// Column: virtual IP.
    pub col_virtual_ip: &'static str,
    /// Column: peers.
    pub col_peers: &'static str,
    /// Tooltip of the peers column.
    pub col_peers_hint: &'static str,
    /// Column: a network.
    pub col_network: &'static str,
    /// Column: a peer.
    pub col_peer: &'static str,
    /// Column: how a peer is reached.
    pub col_route: &'static str,
    /// Column: a credential's id.
    pub col_credential: &'static str,
    /// Column: ACL groups.
    pub col_groups: &'static str,
    /// Column: what a credential allows.
    pub col_options: &'static str,
    /// An age under a second.
    pub ago_now: &'static str,
    /// An age in seconds; `{age}`.
    pub ago_seconds: &'static str,
    /// An age in minutes; `{age}`.
    pub ago_minutes: &'static str,
    /// An age in hours; `{age}`.
    pub ago_hours: &'static str,
    /// An age in days; `{age}`.
    pub ago_days: &'static str,
    /// Unit after a count: "of {total}".
    pub sum_of_total: &'static str,
    /// Opens a network's machine in its drawer.
    pub open_machine: &'static str,
    /// Tooltip of the latency column.
    pub col_latency_hint: &'static str,
    /// Tooltip of the loss column.
    pub col_loss_hint: &'static str,
    /// The machine runs no network, so there are no peers to list.
    pub peers_no_network_body: &'static str,

    // --- Machines ---
    /// No machine is enrolled.
    pub machines_empty_title: &'static str,
    /// How a machine appears.
    pub machines_empty_body: &'static str,
    /// Column: operating system.
    pub col_os: &'static str,
    /// Column: version.
    pub col_version: &'static str,
    /// Column: running networks.
    pub col_networks: &'static str,
    /// Column: location.
    pub col_location: &'static str,
    /// Column: latency.
    pub col_latency: &'static str,
    /// Column: packet loss.
    pub col_loss: &'static str,
    /// Column: bytes received.
    pub col_received: &'static str,
    /// Column: bytes sent.
    pub col_sent: &'static str,
    /// Chip: the machine reported recently.
    pub machine_online: &'static str,
    /// Chip: the machine has not reported lately.
    pub machine_offline: &'static str,
    /// The machine in detail left the server.
    pub machine_gone_title: &'static str,
    /// What to do then.
    pub machine_gone_body: &'static str,
    /// Fact label: where the machine connects from.
    pub machine_connected_from: &'static str,
    /// Fact label: the machine's id.
    pub machine_id: &'static str,
    /// A machine runs no network.
    pub machine_networks_empty_title: &'static str,
    /// What to do then.
    pub machine_networks_empty_body: &'static str,
    /// Creates a network on the machine in detail.
    pub create_network: &'static str,

    // --- Networks ---
    /// Imports a TOML config into a machine.
    pub import_config: &'static str,
    /// A machine's networks could not be read; `{host}`.
    pub networks_machine_failed: &'static str,
    /// No machine runs a network.
    pub networks_empty_title: &'static str,
    /// What to do then.
    pub networks_empty_body: &'static str,
    /// How many machines run a network; `{count}`.
    pub machines_count: &'static str,
    /// Caption after a direct-peer count.
    pub peers_direct: &'static str,
    /// Caption after a relayed-peer count.
    pub peers_relayed: &'static str,
    /// Lamp word: the network runs.
    pub net_running: &'static str,
    /// Lamp word: the network is saved and off.
    pub net_disabled: &'static str,
    /// Lamp word: the network reported an error.
    pub net_error: &'static str,
    /// Chip: the console may not edit this network.
    pub net_read_only: &'static str,
    /// Beside a row whose machine's last read failed: the shown data is older; `{age}` is whole seconds.
    pub net_stale: &'static str,
    /// The same, under a second old.
    pub net_stale_now: &'static str,
    /// Switches a network on.
    pub net_enable: &'static str,
    /// Switches a network off.
    pub net_disable: &'static str,
    /// Opens a network's peers.
    pub net_peers: &'static str,
    /// Opens a network's config for editing.
    pub net_edit_config: &'static str,
    /// Confirmation title; `{name}`.
    pub delete_network_title: &'static str,
    /// Confirmation body; `{name}`, `{host}`.
    pub delete_network_body: &'static str,

    // --- Peers ---
    /// No peer.
    pub peers_empty_title: &'static str,
    /// Why.
    pub peers_empty_body: &'static str,
    /// Chip: a direct link exists.
    pub peer_chip_direct: &'static str,
    /// Chip: no direct link.
    pub peer_chip_relayed: &'static str,

    // --- Credentials ---
    /// One sentence on what a credential is.
    pub credential_explain: &'static str,
    /// There is no way to block a device; disable its network.
    pub credential_no_block: &'static str,
    /// The machine picker's label.
    pub credentials_machine: &'static str,
    /// The network picker's label.
    pub credentials_network: &'static str,
    /// The machine picker's empty option.
    pub credentials_choose_machine: &'static str,
    /// The network picker's empty option.
    pub credentials_choose_network: &'static str,
    /// Nothing is picked yet.
    pub credentials_pick_machine: &'static str,
    /// A machine is picked, a network is not.
    pub credentials_pick_network: &'static str,
    /// The machine runs no network.
    pub credentials_no_network_title: &'static str,
    /// What to do then.
    pub credentials_no_network_body: &'static str,
    /// Section label of the credential list.
    pub credentials_section: &'static str,
    /// Opens the generate form.
    pub generate_credential: &'static str,
    /// No credential yet.
    pub credentials_empty_title: &'static str,
    /// What to do.
    pub credentials_empty_body: &'static str,
    /// A credential has no ACL group.
    pub credential_no_groups: &'static str,
    /// Chip: the node may relay.
    pub credential_relay: &'static str,
    /// Chip: several nodes may use it.
    pub credential_reusable: &'static str,
    /// Column: when a credential expires (UTC).
    pub credential_expires: &'static str,
    /// Label before the allowed subnets.
    pub credential_subnets: &'static str,
    /// Revokes a credential.
    pub revoke: &'static str,
    /// Confirmation title; `{id}`.
    pub revoke_title: &'static str,
    /// Confirmation body.
    pub revoke_body: &'static str,
    /// The generate form's title.
    pub credential_form_title: &'static str,
    /// Lifetime select label.
    pub credential_lifetime: &'static str,
    /// One hour.
    pub lifetime_1h: &'static str,
    /// One day.
    pub lifetime_1d: &'static str,
    /// Seven days.
    pub lifetime_7d: &'static str,
    /// Thirty days.
    pub lifetime_30d: &'static str,
    /// ACL groups field.
    pub credential_groups: &'static str,
    /// Helper under it.
    pub credential_groups_help: &'static str,
    /// Relay switch.
    pub credential_allow_relay: &'static str,
    /// Its description.
    pub credential_allow_relay_help: &'static str,
    /// Its exposure warning.
    pub credential_relay_exposed: &'static str,
    /// Reusable switch.
    pub credential_reusable_label: &'static str,
    /// Its description.
    pub credential_reusable_help: &'static str,
    /// Its exposure warning.
    pub credential_reusable_exposed: &'static str,
    /// The form's confirm button.
    pub credential_generate: &'static str,
    /// Title of the dialog that shows the secret once.
    pub issued_title: &'static str,
    /// Store the secret now.
    pub issued_warning: &'static str,
    /// Label of the credential's id.
    pub issued_id: &'static str,
    /// Label of the secret.
    pub issued_secret: &'static str,
    /// Closes that dialog.
    pub issued_done: &'static str,

    // --- Network editor sections ---
    /// Section: name, secret and hostname.
    pub editor_section_identity: &'static str,
    /// Section: address and how peers are found.
    pub editor_section_connect: &'static str,
    /// Section: what the node shares.
    pub editor_section_share: &'static str,
    /// Section: relaying for other nodes and networks.
    pub editor_section_relay: &'static str,

    // --- The network editor ---
    /// Title when creating; `{host}`.
    pub editor_title_create: &'static str,
    /// Title when editing.
    pub editor_title_edit: &'static str,
    /// Title when importing.
    pub editor_title_import: &'static str,
    /// Network name field.
    pub editor_name: &'static str,
    /// Network secret field.
    pub editor_secret: &'static str,
    /// Hostname field.
    pub editor_hostname: &'static str,
    /// Helper under it.
    pub editor_hostname_help: &'static str,
    /// Virtual IP control label.
    pub editor_address: &'static str,
    /// Option: addressed by DHCP.
    pub editor_dhcp: &'static str,
    /// Option: a fixed address.
    pub editor_static: &'static str,
    /// Fixed address field.
    pub editor_static_ip: &'static str,
    /// Peer discovery control label.
    pub editor_networking: &'static str,
    /// Option: through a shared node.
    pub editor_net_public: &'static str,
    /// Option: through listed peers.
    pub editor_net_manual: &'static str,
    /// Option: alone.
    pub editor_net_standalone: &'static str,
    /// Shared node field.
    pub editor_public_url: &'static str,
    /// Helper under it.
    pub editor_public_url_help: &'static str,
    /// Peers field.
    pub editor_peers: &'static str,
    /// Helper under it.
    pub editor_peers_help: &'static str,
    /// Note for a node that dials no one.
    pub editor_standalone_note: &'static str,
    /// Listeners field.
    pub editor_listeners: &'static str,
    /// Helper under it.
    pub editor_listeners_help: &'static str,
    /// Warning for a listener on every interface.
    pub editor_listeners_exposed: &'static str,
    /// Shared subnets field.
    pub editor_proxy: &'static str,
    /// Helper under it.
    pub editor_proxy_help: &'static str,
    /// Warning for shared subnets.
    pub editor_proxy_exposed: &'static str,
    /// No-TUN switch.
    pub editor_no_tun: &'static str,
    /// Its description.
    pub editor_no_tun_help: &'static str,
    /// Exit node switch.
    pub editor_exit_node: &'static str,
    /// Its description.
    pub editor_exit_node_help: &'static str,
    /// Its exposure warning.
    pub editor_exit_exposed: &'static str,
    /// SOCKS5 switch.
    pub editor_socks5: &'static str,
    /// Its description.
    pub editor_socks5_help: &'static str,
    /// Its exposure warning.
    pub editor_socks5_exposed: &'static str,
    /// SOCKS5 port field.
    pub editor_socks5_port: &'static str,
    /// MTU field.
    pub editor_mtu: &'static str,
    /// Helper under it.
    pub editor_mtu_help: &'static str,
    /// Interface name field.
    pub editor_dev_name: &'static str,
    /// Helper under it.
    pub editor_dev_name_help: &'static str,
    /// Exit nodes this node uses: field.
    pub editor_exit_nodes: &'static str,
    /// Helper under it.
    pub editor_exit_nodes_help: &'static str,
    /// Port forwards field.
    pub editor_forwards: &'static str,
    /// Warning for a forward that listens beyond this machine.
    pub editor_forwards_exposed: &'static str,
    /// Warning: forwards the server holds that this app can't read.
    pub editor_forwards_unreadable: &'static str,
    /// Relay-all-RPC switch.
    pub editor_relay_all_rpc: &'static str,
    /// Its description.
    pub editor_relay_all_rpc_help: &'static str,
    /// Its exposure warning.
    pub editor_relay_all_rpc_exposed: &'static str,
    /// Private mode switch.
    pub editor_private_mode: &'static str,
    /// Its description.
    pub editor_private_mode_help: &'static str,
    /// Warning while private mode is off.
    pub editor_private_mode_off: &'static str,
    /// Relay whitelist switch.
    pub editor_whitelist: &'static str,
    /// Its description.
    pub editor_whitelist_help: &'static str,
    /// The whitelist's field.
    pub editor_whitelist_field: &'static str,
    /// Helper under it.
    pub editor_whitelist_field_help: &'static str,
    /// Access control the server holds that this app can't read.
    pub editor_acl_unreadable: &'static str,
    /// Encryption select label.
    pub editor_encryption: &'static str,
    /// Peers must agree.
    pub editor_encryption_help: &'static str,
    /// Warning for XOR and off.
    pub editor_encryption_weak: &'static str,
    /// Option: the core's default.
    pub enc_default: &'static str,
    /// Option: AES-GCM.
    pub enc_aes_gcm: &'static str,
    /// Option: AES-256-GCM.
    pub enc_aes_256_gcm: &'static str,
    /// Option: ChaCha20.
    pub enc_chacha20: &'static str,
    /// Option: XOR, which is obfuscation.
    pub enc_xor: &'static str,
    /// Option: no encryption.
    pub enc_off: &'static str,
    /// Import: machine select label.
    pub editor_import_machine: &'static str,
    /// Import: TOML field.
    pub editor_import_toml: &'static str,
    /// Helper under it.
    pub editor_import_help: &'static str,
    /// Warning above an imported config's form.
    pub editor_import_review: &'static str,
    /// Reads the pasted TOML.
    pub editor_read: &'static str,
    /// Back from the review to the pasted text.
    pub editor_back: &'static str,
    /// Starts the imported network.
    pub editor_run: &'static str,
    /// Creates the network.
    pub editor_create: &'static str,
    /// Saves the edit.
    pub editor_save: &'static str,
    /// No machine is chosen.
    pub problem_machine: &'static str,
    /// The name is empty.
    pub problem_name: &'static str,
    /// The secret is empty.
    pub problem_secret: &'static str,
    /// The fixed IP is malformed.
    pub problem_static_ip: &'static str,
    /// The shared node's address is malformed.
    pub problem_public_url: &'static str,
    /// A peer line is malformed; `{line}`.
    pub problem_peer: &'static str,
    /// A listener line is malformed; `{line}`.
    pub problem_listener: &'static str,
    /// A subnet line is malformed; `{line}`.
    pub problem_proxy: &'static str,
    /// An exit node line is not an IP address; `{line}`.
    pub problem_exit_node: &'static str,
    /// The SOCKS5 port is not a port.
    pub problem_socks5_port: &'static str,
    /// The MTU is out of range.
    pub problem_mtu: &'static str,
    /// The interface name is invalid.
    pub problem_dev_name: &'static str,
    /// The receive limit is not a positive number.
    pub problem_recv_limit: &'static str,
    /// A mapped listener line is malformed; `{line}`.
    pub problem_mapped_listener: &'static str,
    /// A manual route line is malformed; `{line}`.
    pub problem_route: &'static str,
    /// A whitelist line has a space; `{line}`.
    pub problem_whitelist: &'static str,
    /// The VPN client subnet is malformed.
    pub problem_vpn_cidr: &'static str,
    /// The VPN port is not a port.
    pub problem_vpn_port: &'static str,
    /// A port forward line is malformed; `{line}`.
    pub problem_forward: &'static str,
    /// The ACL has a problem (the ACL editor shows which).
    pub problem_acl: &'static str,
    /// No TOML was pasted.
    pub problem_nothing_to_import: &'static str,
    /// A browser cannot generate a secret.
    pub problem_no_random: &'static str,
}

pub(super) static EN: ConsoleStrings = ConsoleStrings {
    username: "Username",
    password: "Password",
    sign_in: "Sign in",
    signing_in: "Signing in…",
    sign_in_title: "Sign in to manage this server",
    sign_in_expired: "The session on this server ended. Sign in again to continue.",
    sign_in_password_changed: "The password was changed and the session ended. Sign in with the new password.",
    unreachable: "Can't reach this server. Check its address and your connection, then try again.",
    sign_in_missing: "Enter the username and the password.",
    sign_in_digest_note: "easytier-web receives an MD5 digest of the password, which protects nothing on the way. \
                          Every server except this machine must use https://.",
    session_not_kept: "This device can't keep the session. You'll sign in again after the app quits.",
    state_unreachable: "Unreachable",
    state_expired: "Session ended",

    remove_server: "Remove server",
    sign_out: "Sign out",
    sign_out_title: "Sign out of {server}?",
    sign_out_body: "Its machines and networks keep running. You'll need to sign in again to manage them.",
    remove_title: "Remove server “{server}”?",
    remove_body: "The app signs out and forgets this server. The server itself is not changed.",
    change_password: "Change password",
    password_note: "The server ends this session when the password changes. Sign in again with the new one.",
    password_new: "New password",
    password_confirm: "New password again",
    password_empty: "Enter the new password.",
    password_mismatch: "The two entries don't match.",
    read_age: "Updated {age} s ago",
    read_now: "Updated just now",

    add_server_title: "Add a server",
    add_server_url: "Server address",
    add_server_url_help: "The address of an easytier-web server. It must start with https://, except for this machine.",
    add_server_name: "Name (optional)",
    add_server_name_help: "Shown in the target switcher. Leave it empty to use the server's host.",
    add_server_submit: "Add",
    add_error_insecure: "Plain http:// is only allowed for this machine. Use https://.",
    add_error_invalid: "That isn't a usable address. Start it with https:// (or http:// for this machine).",
    add_error_duplicate: "This server is already listed as “{name}”.",
    add_error_full: "No room for more servers.",

    err_session_ended: "The session ended. Sign in again.",
    err_bad_credentials: "The server refused this username or password.",
    err_machine_gone: "This machine is no longer connected to the server.",
    err_rejected: "The server refused: {message}",
    err_timeout: "The server didn't answer in time.",
    err_machine_no_answer: "The device didn't answer the server. Check that EasyTier runs on it and its connection to the server is stable.",
    err_network: "Couldn't reach the server: {message}",
    err_decode: "The server's reply couldn't be read. It may run a different version of easytier-web.",
    err_insecure: "Plain http:// is only allowed for this machine. Use https://.",
    err_invalid_address: "The server address isn't valid.",
    err_invalid_request: "The request isn't valid: {message}",
    load_failed: "Couldn't load this.",
    refresh_failed: "Couldn't refresh. You are seeing the last reading.",
    failed_enable: "Couldn't enable “{name}”.",
    failed_disable: "Couldn't disable “{name}”.",
    failed_delete: "Couldn't delete “{name}”.",
    failed_revoke: "Couldn't revoke credential {name}.",

    search_machines: "Search by name, system or version",
    search_networks: "Search by network, machine or IP",
    search_clear: "Clear search",
    no_match_title: "Nothing matches your search",
    no_match_body: "Check the spelling, or clear the search to see everything.",
    more_actions: "More actions",
    account_menu: "Account",
    tabs_label: "Show",
    tab_networks: "Networks",
    tab_peers: "Peers",
    peers_pick_network: "Choose one of this machine's running networks to see its peers.",
    kind_credential: "Credential",
    sum_machines: "Machines",
    sum_networks_running: "Networks running",
    sum_versions: "Core versions",
    sum_network_names: "Networks",
    sum_instances: "Instances",
    sum_peers: "Peers",
    col_machine: "Machine",
    col_status: "Status",
    col_reported: "Last report",
    col_virtual_ip: "Virtual IP",
    col_peers: "Peers",
    col_peers_hint: "Peers in total, then those reached directly and those through relays.",
    col_network: "Network",
    col_peer: "Peer",
    col_route: "Route",
    col_credential: "Credential",
    col_groups: "Groups",
    col_options: "Allows",
    ago_now: "just now",
    ago_seconds: "{age} s ago",
    ago_minutes: "{age} min ago",
    ago_hours: "{age} h ago",
    ago_days: "{age} d ago",
    sum_of_total: "of {total}",
    open_machine: "Machine details",
    col_latency_hint: "Green under 100 ms, amber above.",
    col_loss_hint: "Green under 2 %, amber from 2 %, red from 10 %.",
    peers_no_network_body: "Peers show once a network on this machine is running.",

    machines_empty_title: "No machines yet",
    machines_empty_body: "A node appears here once it is set up to use this server as its config server.",
    col_os: "System",
    col_version: "Version",
    col_networks: "Networks",
    col_location: "Location",
    col_latency: "Latency",
    col_loss: "Loss",
    col_received: "Received",
    col_sent: "Sent",
    machine_online: "Online",
    machine_offline: "Offline",
    machine_gone_title: "This machine isn't connected",
    machine_gone_body: "It left the server, or the server restarted. Close this panel to see what is connected.",
    machine_connected_from: "Connected from",
    machine_id: "Machine ID",
    machine_networks_empty_title: "No networks on this machine",
    machine_networks_empty_body: "Create one to put this machine in a network.",
    create_network: "Create network on this machine",

    import_config: "Import config…",
    networks_machine_failed: "Couldn't read the networks of {host}.",
    networks_empty_title: "No networks yet",
    networks_empty_body: "None of the machines runs a network. Create one from a machine's page.",
    machines_count: "Machines: {count}",
    peers_direct: "direct",
    peers_relayed: "relayed",
    net_running: "Running",
    net_disabled: "Disabled",
    net_error: "Error",
    net_read_only: "Managed",
    net_stale: "Not refreshed · data from {age} s ago",
    net_stale_now: "Not refreshed",
    net_enable: "Enable",
    net_disable: "Disable",
    net_peers: "Peers",
    net_edit_config: "Edit config",
    delete_network_title: "Delete network “{name}”?",
    delete_network_body: "It stops on {host} and its saved config is removed from the server.",

    peers_empty_title: "No peers",
    peers_empty_body: "No other node has joined this network on this machine yet.",
    peer_chip_direct: "Direct",
    peer_chip_relayed: "Relayed",

    credential_explain: "A credential is a key a node can join this network with instead of the network secret. It \
                         expires after the time you choose, and you can revoke it here.",
    credential_no_block: "easytier-web 2.6.4 has no way to block a single device. To cut a machine off, disable its \
                          network on the Machines page.",
    credentials_machine: "Machine",
    credentials_network: "Network",
    credentials_choose_machine: "Choose a machine…",
    credentials_choose_network: "Choose a network…",
    credentials_pick_machine: "Choose a machine and one of its running networks to see its credentials.",
    credentials_pick_network: "Choose one of this machine's running networks.",
    credentials_no_network_title: "No running network on this machine",
    credentials_no_network_body: "Credentials belong to a running network. Enable one on the Machines page.",
    credentials_section: "Credentials",
    generate_credential: "Generate credential",
    credentials_empty_title: "No credentials yet",
    credentials_empty_body: "Generate one to let a node join this network without its secret.",
    credential_no_groups: "No groups",
    credential_relay: "May relay",
    credential_reusable: "Reusable",
    credential_expires: "Expires (UTC)",
    credential_subnets: "Subnets",
    revoke: "Revoke",
    revoke_title: "Revoke credential {id}?",
    revoke_body: "Nodes can no longer join with it. This can't be undone.",
    credential_form_title: "Generate credential",
    credential_lifetime: "Valid for",
    lifetime_1h: "1 hour",
    lifetime_1d: "1 day",
    lifetime_7d: "7 days",
    lifetime_30d: "30 days",
    credential_groups: "ACL groups",
    credential_groups_help: "Optional. Separate groups with commas.",
    credential_allow_relay: "Allow relaying",
    credential_allow_relay_help: "The node can forward other nodes' traffic.",
    credential_relay_exposed: "This node will carry other peers' traffic through its connection.",
    credential_reusable_label: "Reusable",
    credential_reusable_help: "Several nodes can join with the same credential.",
    credential_reusable_exposed: "Anyone who holds the secret can join, until it expires.",
    credential_generate: "Generate",
    issued_title: "Credential created",
    issued_warning: "Store this secret now. It is shown only once and can't be read again.",
    issued_id: "Credential ID",
    issued_secret: "Secret",
    issued_done: "I've stored it",

    editor_section_identity: "Network",
    editor_section_connect: "Connectivity",
    editor_section_share: "Sharing",
    editor_section_relay: "Relaying",

    editor_title_create: "Create network on {host}",
    editor_title_edit: "Edit network",
    editor_title_import: "Import config",
    editor_name: "Network name",
    editor_secret: "Network secret",
    editor_hostname: "Hostname (optional)",
    editor_hostname_help: "What peers call this node. Empty uses the machine's own.",
    editor_address: "Virtual IP",
    editor_dhcp: "Automatic",
    editor_static: "Fixed",
    editor_static_ip: "Fixed virtual IP",
    editor_networking: "How peers are found",
    editor_net_public: "Shared node",
    editor_net_manual: "Peers",
    editor_net_standalone: "None",
    editor_public_url: "Shared node address",
    editor_public_url_help: "A shared node relays traffic between the peers that reach it.",
    editor_peers: "Peer addresses",
    editor_peers_help: "One per line. Without peers the node waits for others to connect.",
    editor_standalone_note: "This node dials no one; other nodes connect to its listeners.",
    editor_listeners: "Listeners",
    editor_listeners_help: "One per line. Empty uses the core's defaults, which listen on every interface.",
    editor_listeners_exposed: "A listener on 0.0.0.0 or :: accepts connections from every network this machine is on.",
    editor_proxy: "Subnets to share",
    editor_proxy_help: "One per line: 192.168.1.0/24, or 192.168.1.0/24->10.50.0.0/24 to show peers the second \
                        subnet instead. Peers reach these subnets through this node.",
    editor_proxy_exposed: "Everyone in this network can reach these subnets through this node.",
    editor_no_tun: "No TUN device",
    editor_no_tun_help: "Runs without a virtual network interface, so no administrator rights are needed. The \
                         machine's own apps can't use the network.",
    editor_exit_node: "Exit node",
    editor_exit_node_help: "Other nodes can send their internet traffic through this one.",
    editor_exit_exposed: "This machine will forward other nodes' internet traffic.",
    editor_socks5: "SOCKS5 proxy",
    editor_socks5_help: "Opens a local proxy into the network.",
    editor_socks5_exposed: "Anything that can reach this port can reach the network.",
    editor_socks5_port: "SOCKS5 port",
    editor_mtu: "MTU",
    editor_mtu_help: "From 400 to 1500. Empty uses the core's default.",
    editor_dev_name: "Interface name",
    editor_dev_name_help: "The TUN interface's name, such as et0. Empty lets the system choose.",
    editor_exit_nodes: "Exit nodes to use",
    editor_exit_nodes_help: "The virtual IP of each exit node, one per line. Internet traffic from this node leaves \
                             through them.",
    editor_forwards: "Port forwards",
    editor_forwards_exposed: "A forward that listens on an address other than this machine's own is open to every \
                              network this machine is on.",
    editor_forwards_unreadable: "The server holds port forwards this app can't read. They stay as they are until you \
                                 edit this list, then they are dropped.",
    editor_relay_all_rpc: "Relay control traffic for every peer",
    editor_relay_all_rpc_help: "Also passes on the control messages of peers from other networks. Only a public \
                                shared node needs this.",
    editor_relay_all_rpc_exposed: "Relays control traffic for every peer, not only for this network.",
    editor_private_mode: "Private mode",
    editor_private_mode_help: "Refuses peers from other networks that don't know this network's secret.",
    editor_private_mode_off: "Private mode is off: peers from other networks are accepted without the secret.",
    editor_whitelist: "Relay only for listed networks",
    editor_whitelist_help: "Without it this node relays for any network.",
    editor_whitelist_field: "Network names",
    editor_whitelist_field_help: "One network name per line. A * stands for any characters.",
    editor_acl_unreadable: "This network's access control can't be edited here. The server's rules are kept as they \
                            are when you save.",
    editor_encryption: "Encryption",
    editor_encryption_help: "Every peer in the network must use the same setting.",
    editor_encryption_weak: "XOR only scrambles traffic and Off sends it in the clear. Use them only for tests or \
                            links you already trust.",
    enc_default: "Core default (AES-GCM)",
    enc_aes_gcm: "AES-GCM",
    enc_aes_256_gcm: "AES-256-GCM",
    enc_chacha20: "ChaCha20",
    enc_xor: "XOR (obfuscation, not encryption)",
    enc_off: "Off",
    editor_import_machine: "Machine",
    editor_import_toml: "Config file (TOML)",
    editor_import_help: "Paste a config file. The server reads it; nothing runs until you review it and choose Run.",
    editor_import_review: "Review before it runs. An imported config is untrusted: check the peers, listeners, \
                           subnets and encryption below.",
    editor_read: "Read config",
    editor_back: "Back",
    editor_run: "Run network",
    editor_create: "Create network",
    editor_save: "Save changes",
    problem_machine: "Choose a machine.",
    problem_name: "Enter a network name.",
    problem_secret: "Enter a network secret, or use Generate.",
    problem_static_ip: "The fixed virtual IP must look like 10.144.144.1/24.",
    problem_public_url: "The shared node's address isn't valid. Use a form like tcp://host:11010.",
    problem_peer: "“{line}” isn't a valid peer address.",
    problem_listener: "“{line}” isn't a valid listener address.",
    problem_proxy: "“{line}” isn't a valid subnet. Use a form like 192.168.1.0/24.",
    problem_exit_node: "“{line}” isn't a valid exit node. Use a virtual IP like 10.144.144.2.",
    problem_socks5_port: "The SOCKS5 port must be a number from 1 to 65535.",
    problem_mtu: "The MTU must be a number from 400 to 1500, or empty for the default.",
    problem_dev_name: "The interface name is at most 15 characters: letters, digits, “-”, “_” and “.”.",
    problem_recv_limit: "The receive limit must be a whole number of bytes per second above 0, or empty for no limit.",
    problem_mapped_listener: "“{line}” isn't a valid mapped listener. Use a form like tcp://203.0.113.5:11010.",
    problem_route: "“{line}” isn't a valid route. Use a subnet like 192.168.0.0/16.",
    problem_whitelist: "“{line}” contains a space. Write one network name per line; * matches any characters.",
    problem_vpn_cidr: "The VPN client subnet isn't valid. Use a form like 10.14.14.0/24.",
    problem_vpn_port: "The VPN port must be a number from 1 to 65535.",
    problem_forward: "“{line}” isn't a valid port forward. Use a form like tcp 127.0.0.1:8080 10.144.144.2:80.",
    problem_acl: "The access control rules have an error. Fix it in the access control section, then save.",
    problem_nothing_to_import: "Paste a config first.",
    problem_no_random: "This browser can't generate a secret. Type one of at least 20 characters.",
};

pub(super) static ZH_CN: ConsoleStrings = ConsoleStrings {
    username: "用户名",
    password: "密码",
    sign_in: "登录",
    signing_in: "正在登录…",
    sign_in_title: "登录后管理此服务器",
    sign_in_expired: "此服务器上的会话已结束，请重新登录。",
    sign_in_password_changed: "密码已修改，会话已结束。请用新密码登录。",
    unreachable: "无法连接到此服务器。请检查地址和网络后重试。",
    sign_in_missing: "请输入用户名和密码。",
    sign_in_digest_note: "easytier-web 收到的是密码的 MD5 摘要，传输途中起不到保护作用。除本机外的服务器都必须使用 \
                          https://。",
    session_not_kept: "此设备无法保存会话，应用退出后需要重新登录。",
    state_unreachable: "无法连接",
    state_expired: "会话已结束",

    remove_server: "移除服务器",
    sign_out: "退出登录",
    sign_out_title: "退出 {server} 的登录？",
    sign_out_body: "它的设备和网络会继续运行。要再次管理，需要重新登录。",
    remove_title: "移除服务器“{server}”？",
    remove_body: "应用会退出登录并忘记此服务器，服务器本身不受影响。",
    change_password: "修改密码",
    password_note: "密码修改后，服务器会结束当前会话，请用新密码重新登录。",
    password_new: "新密码",
    password_confirm: "再次输入新密码",
    password_empty: "请输入新密码。",
    password_mismatch: "两次输入不一致。",
    read_age: "{age} 秒前更新",
    read_now: "刚刚更新",

    add_server_title: "添加服务器",
    add_server_url: "服务器地址",
    add_server_url_help: "easytier-web 服务器的地址。必须以 https:// 开头，本机除外。",
    add_server_name: "名称（可选）",
    add_server_name_help: "显示在目标切换器中。留空则使用服务器的主机名。",
    add_server_submit: "添加",
    add_error_insecure: "只有本机可以使用明文 http://，请改用 https://。",
    add_error_invalid: "这不是可用的地址。请以 https:// 开头（本机可用 http://）。",
    add_error_duplicate: "此服务器已在列表中，名称为“{name}”。",
    add_error_full: "无法再添加更多服务器。",

    err_session_ended: "会话已结束，请重新登录。",
    err_bad_credentials: "服务器拒绝了该用户名或密码。",
    err_machine_gone: "此设备已不再连接到服务器。",
    err_rejected: "服务器拒绝了请求：{message}",
    err_timeout: "服务器未在限定时间内响应。",
    err_machine_no_answer: "设备没有响应服务器的请求。请检查该设备上的 EasyTier 是否正常运行、与服务器的连接是否稳定。",
    err_network: "无法连接到服务器：{message}",
    err_decode: "无法读取服务器的回复，它运行的 easytier-web 版本可能不同。",
    err_insecure: "只有本机可以使用明文 http://，请改用 https://。",
    err_invalid_address: "服务器地址无效。",
    err_invalid_request: "请求无效：{message}",
    load_failed: "无法加载。",
    refresh_failed: "刷新失败，当前显示的是上一次读取的内容。",
    failed_enable: "无法启用“{name}”。",
    failed_disable: "无法停用“{name}”。",
    failed_delete: "无法删除“{name}”。",
    failed_revoke: "无法吊销凭据 {name}。",

    search_machines: "按名称、系统或版本搜索",
    search_networks: "按网络、设备或 IP 搜索",
    search_clear: "清除搜索",
    no_match_title: "没有符合搜索条件的结果",
    no_match_body: "请检查拼写，或清除搜索以查看全部。",
    more_actions: "更多操作",
    account_menu: "账户",
    tabs_label: "显示",
    tab_networks: "网络",
    tab_peers: "对等节点",
    peers_pick_network: "选择此设备上正在运行的网络，即可查看它的对等节点。",
    kind_credential: "凭据",
    sum_machines: "设备",
    sum_networks_running: "运行中的网络",
    sum_versions: "内核版本",
    sum_network_names: "网络",
    sum_instances: "实例",
    sum_peers: "对等节点",
    col_machine: "设备",
    col_status: "状态",
    col_reported: "最近上报",
    col_virtual_ip: "虚拟 IP",
    col_peers: "对等节点",
    col_peers_hint: "对等节点总数，其后依次为直连和经中继的数量。",
    col_network: "网络",
    col_peer: "对等节点",
    col_route: "路径",
    col_credential: "凭据",
    col_groups: "分组",
    col_options: "权限",
    ago_now: "刚刚",
    ago_seconds: "{age} 秒前",
    ago_minutes: "{age} 分钟前",
    ago_hours: "{age} 小时前",
    ago_days: "{age} 天前",
    sum_of_total: "共 {total}",
    open_machine: "设备详情",
    col_latency_hint: "低于 100 毫秒为绿色，高于则为琥珀色。",
    col_loss_hint: "低于 2 % 为绿色，2 % 起为琥珀色，10 % 起为红色。",
    peers_no_network_body: "此设备上的网络开始运行后，这里会显示对等节点。",

    machines_empty_title: "还没有设备",
    machines_empty_body: "节点设置为使用此服务器作为配置服务器后，会出现在这里。",
    col_os: "系统",
    col_version: "版本",
    col_networks: "网络",
    col_location: "位置",
    col_latency: "延迟",
    col_loss: "丢包",
    col_received: "接收",
    col_sent: "发送",
    machine_online: "在线",
    machine_offline: "离线",
    machine_gone_title: "此设备未连接",
    machine_gone_body: "它已断开与服务器的连接，或服务器已重启。关闭此面板，查看当前在线的设备。",
    machine_connected_from: "连接来源",
    machine_id: "设备 ID",
    machine_networks_empty_title: "此设备上没有网络",
    machine_networks_empty_body: "创建一个网络，让此设备加入组网。",
    create_network: "在此设备上创建网络",

    import_config: "导入配置…",
    networks_machine_failed: "无法读取 {host} 上的网络。",
    networks_empty_title: "还没有网络",
    networks_empty_body: "没有设备在运行网络。请在设备页面中创建。",
    machines_count: "设备：{count}",
    peers_direct: "直连",
    peers_relayed: "中继",
    net_running: "运行中",
    net_disabled: "已停用",
    net_error: "出错",
    net_read_only: "托管",
    net_stale: "未能刷新 · {age} 秒前的数据",
    net_stale_now: "未能刷新",
    net_enable: "启用",
    net_disable: "停用",
    net_peers: "对等节点",
    net_edit_config: "编辑配置",
    delete_network_title: "删除网络“{name}”？",
    delete_network_body: "它会在 {host} 上停止，其保存的配置会从服务器删除。",

    peers_empty_title: "没有对等节点",
    peers_empty_body: "此设备上的这个网络里还没有其他节点加入。",
    peer_chip_direct: "直连",
    peer_chip_relayed: "中继",

    credential_explain: "凭据是节点加入此网络时可以代替网络密钥使用的密钥。它在你设定的时间后过期，也可以在这里吊销。",
    credential_no_block: "easytier-web 2.6.4 没有屏蔽单个设备的功能。要让某台设备断开，请在“设备”页面停用它的网络。",
    credentials_machine: "设备",
    credentials_network: "网络",
    credentials_choose_machine: "选择设备…",
    credentials_choose_network: "选择网络…",
    credentials_pick_machine: "选择一台设备及其正在运行的网络，即可查看凭据。",
    credentials_pick_network: "请选择此设备上正在运行的网络。",
    credentials_no_network_title: "此设备上没有运行中的网络",
    credentials_no_network_body: "凭据属于运行中的网络。请在“设备”页面启用一个网络。",
    credentials_section: "凭据",
    generate_credential: "生成凭据",
    credentials_empty_title: "还没有凭据",
    credentials_empty_body: "生成一个，节点即可不用网络密钥加入此网络。",
    credential_no_groups: "无分组",
    credential_relay: "可中继",
    credential_reusable: "可重复使用",
    credential_expires: "过期时间（UTC）",
    credential_subnets: "子网",
    revoke: "吊销",
    revoke_title: "吊销凭据 {id}？",
    revoke_body: "节点将无法再用它加入网络，此操作无法撤销。",
    credential_form_title: "生成凭据",
    credential_lifetime: "有效期",
    lifetime_1h: "1 小时",
    lifetime_1d: "1 天",
    lifetime_7d: "7 天",
    lifetime_30d: "30 天",
    credential_groups: "ACL 分组",
    credential_groups_help: "可选，多个分组用逗号分隔。",
    credential_allow_relay: "允许中继",
    credential_allow_relay_help: "该节点可以转发其他节点的流量。",
    credential_relay_exposed: "该节点会用自己的连接为其他对等节点转发流量。",
    credential_reusable_label: "可重复使用",
    credential_reusable_help: "多个节点可以使用同一个凭据加入。",
    credential_reusable_exposed: "任何持有该密钥的人都能加入，直到它过期。",
    credential_generate: "生成",
    issued_title: "凭据已创建",
    issued_warning: "请立即保存此密钥。它只显示这一次，之后无法再次查看。",
    issued_id: "凭据 ID",
    issued_secret: "密钥",
    issued_done: "我已保存",

    editor_section_identity: "网络",
    editor_section_connect: "连接",
    editor_section_share: "共享",
    editor_section_relay: "中继",

    editor_title_create: "在 {host} 上创建网络",
    editor_title_edit: "编辑网络",
    editor_title_import: "导入配置",
    editor_name: "网络名称",
    editor_secret: "网络密钥",
    editor_hostname: "主机名（可选）",
    editor_hostname_help: "其他对等节点看到的名称。留空则使用设备自己的主机名。",
    editor_address: "虚拟 IP",
    editor_dhcp: "自动分配",
    editor_static: "固定",
    editor_static_ip: "固定虚拟 IP",
    editor_networking: "如何发现对等节点",
    editor_net_public: "共享节点",
    editor_net_manual: "对等节点",
    editor_net_standalone: "不连接",
    editor_public_url: "共享节点地址",
    editor_public_url_help: "共享节点会在连接到它的对等节点之间中继流量。",
    editor_peers: "对等节点地址",
    editor_peers_help: "每行一个。没有对等节点时，本节点等待其他节点来连接。",
    editor_standalone_note: "此节点不主动连接任何节点，由其他节点连接它的监听地址。",
    editor_listeners: "监听地址",
    editor_listeners_help: "每行一个。留空则使用内核默认设置，会在所有网络接口上监听。",
    editor_listeners_exposed: "监听 0.0.0.0 或 :: 会接受来自此设备所连每个网络的连接。",
    editor_proxy: "共享的子网",
    editor_proxy_help: "每行一个：192.168.1.0/24，或写成 192.168.1.0/24->10.50.0.0/24，让其他节点看到后一个子网。对等节点可以通过此节点访问这些子网。",
    editor_proxy_exposed: "此网络中的所有节点都能通过此节点访问这些子网。",
    editor_no_tun: "不创建 TUN 设备",
    editor_no_tun_help: "不创建虚拟网络接口，无需管理员权限，但设备上的应用无法使用该网络。",
    editor_exit_node: "出口节点",
    editor_exit_node_help: "其他节点可以把上网流量经由此节点转发。",
    editor_exit_exposed: "此设备会转发其他节点的上网流量。",
    editor_socks5: "SOCKS5 代理",
    editor_socks5_help: "在本机打开一个通向该网络的代理。",
    editor_socks5_exposed: "任何能访问该端口的程序都能访问这个网络。",
    editor_socks5_port: "SOCKS5 端口",
    editor_mtu: "MTU",
    editor_mtu_help: "400 到 1500 之间。留空使用核心的默认值。",
    editor_dev_name: "网卡名称",
    editor_dev_name_help: "TUN 网卡的名称，例如 et0。留空由系统决定。",
    editor_exit_nodes: "使用的出口节点",
    editor_exit_nodes_help: "每行填写一个出口节点的虚拟 IP。此节点的上网流量会经由它们发出。",
    editor_forwards: "端口转发",
    editor_forwards_exposed: "监听地址不是本机自身的转发，会对此设备所连的每个网络开放。",
    editor_forwards_unreadable: "服务器上有此应用读不懂的端口转发。在你编辑此列表之前它们保持原样，编辑后会被丢弃。",
    editor_relay_all_rpc: "为所有对等节点中继控制流量",
    editor_relay_all_rpc_help: "同时转发其他网络中节点的控制消息。只有公网共享节点需要开启。",
    editor_relay_all_rpc_exposed: "为所有对等节点中继控制流量，而不只是本网络。",
    editor_private_mode: "私有模式",
    editor_private_mode_help: "拒绝不知道本网络密钥的其他网络节点。",
    editor_private_mode_off: "私有模式已关闭：其他网络的节点无需密钥即可接入。",
    editor_whitelist: "只为列出的网络中继",
    editor_whitelist_help: "关闭时，此节点为任何网络提供中继。",
    editor_whitelist_field: "网络名称",
    editor_whitelist_field_help: "每行填写一个网络名称，* 代表任意字符。",
    editor_acl_unreadable: "此网络的访问控制无法在这里编辑。保存时服务器上的规则保持原样。",
    editor_encryption: "加密",
    editor_encryption_help: "网络中的所有对等节点必须使用相同的设置。",
    editor_encryption_weak: "XOR 只是混淆流量，关闭则以明文传输。仅用于测试或你已信任的链路。",
    enc_default: "内核默认（AES-GCM）",
    enc_aes_gcm: "AES-GCM",
    enc_aes_256_gcm: "AES-256-GCM",
    enc_chacha20: "ChaCha20",
    enc_xor: "XOR（仅混淆，不是加密）",
    enc_off: "关闭",
    editor_import_machine: "设备",
    editor_import_toml: "配置文件（TOML）",
    editor_import_help: "粘贴配置文件。服务器会读取它；在你检查并选择运行之前不会启动任何网络。",
    editor_import_review: "运行前请先检查。导入的配置不受信任：请核对下方的对等节点、监听地址、子网和加密设置。",
    editor_read: "读取配置",
    editor_back: "返回",
    editor_run: "运行网络",
    editor_create: "创建网络",
    editor_save: "保存更改",
    problem_machine: "请选择设备。",
    problem_name: "请输入网络名称。",
    problem_secret: "请输入网络密钥，或点击“生成”。",
    problem_static_ip: "固定虚拟 IP 的格式应为 10.144.144.1/24。",
    problem_public_url: "共享节点地址无效，请使用 tcp://host:11010 这样的格式。",
    problem_peer: "“{line}”不是有效的对等节点地址。",
    problem_listener: "“{line}”不是有效的监听地址。",
    problem_proxy: "“{line}”不是有效的子网，请使用 192.168.1.0/24 这样的格式。",
    problem_exit_node: "“{line}”不是有效的出口节点，请使用 10.144.144.2 这样的虚拟 IP。",
    problem_socks5_port: "SOCKS5 端口必须是 1 到 65535 之间的数字。",
    problem_mtu: "MTU 必须是 400 到 1500 之间的数字，留空则使用默认值。",
    problem_dev_name: "网卡名称最多 15 个字符，只能包含字母、数字、“-”、“_”和“.”。",
    problem_recv_limit: "接收速率限制必须是大于 0 的整数（字节/秒），留空则不限速。",
    problem_mapped_listener: "“{line}”不是有效的映射监听地址，请使用 tcp://203.0.113.5:11010 这样的格式。",
    problem_route: "“{line}”不是有效的路由网段，请使用 192.168.0.0/16 这样的格式。",
    problem_whitelist: "“{line}”含有空格。每行填写一个网络名称，可使用 * 通配。",
    problem_vpn_cidr: "VPN 客户端网段无效，请使用 10.14.14.0/24 这样的格式。",
    problem_vpn_port: "VPN 端口必须是 1 到 65535 之间的数字。",
    problem_forward: "“{line}”不是有效的端口转发，请使用 tcp 127.0.0.1:8080 10.144.144.2:80 这样的格式。",
    problem_acl: "访问控制规则有错误，请在访问控制中修正后再保存。",
    problem_nothing_to_import: "请先粘贴配置。",
    problem_no_random: "浏览器无法生成密钥。请输入至少 20 个字符的密钥。",
};
