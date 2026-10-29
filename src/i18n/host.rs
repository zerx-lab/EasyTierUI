//! Control-plane host role: the hosted easytier-web's status, first-run hardening, deployment files.
//!
//! One field per string; `EN` and `ZH_CN` must both set every field (the struct makes a missing one a compile
//! error). Templates use `{name}` placeholders filled by [`super::fill`].

/// Strings of this area in one language.
#[derive(Debug)]
pub struct HostStrings {
    /// Section label: the process and its actions.
    pub section_control: &'static str,
    /// Section label: the control plane's settings.
    pub section_settings: &'static str,
    /// Section label: where the control plane listens.
    pub section_addresses: &'static str,
    /// Section label: the process output.
    pub section_logs: &'static str,
    /// Row label: the release the control plane runs.
    pub version_label: &'static str,
    /// Row label: the REST API.
    pub api_label: &'static str,
    /// Scope chip: the REST API is reachable from this computer only.
    pub api_scope: &'static str,
    /// Row label: the config server.
    pub config_label: &'static str,
    /// Scope chip: the config server listens on every interface.
    pub config_scope: &'static str,
    /// Starts the control plane.
    pub start: &'static str,
    /// Stops the control plane.
    pub stop: &'static str,
    /// Stops and starts the control plane with the saved settings.
    pub restart: &'static str,
    /// Saves the settings without starting.
    pub save_settings: &'static str,
    /// Field label: the REST API port.
    pub field_api_port: &'static str,
    /// Helper of the REST API port.
    pub help_api_port: &'static str,
    /// Field label: the config server port.
    pub field_config_port: &'static str,
    /// Helper of the config server port.
    pub help_config_port: &'static str,
    /// Field label: the config server protocol.
    pub field_protocol: &'static str,
    /// Protocol: UDP.
    pub protocol_udp: &'static str,
    /// Protocol: TCP.
    pub protocol_tcp: &'static str,
    /// Protocol: WebSocket.
    pub protocol_ws: &'static str,
    /// Settings take effect at the next start.
    pub settings_applies: &'static str,
    /// The config server is reachable from the network; the port and protocol follow in monospace.
    pub exposure_note: &'static str,
    /// The saved settings could not be read.
    pub settings_corrupt: &'static str,
    /// Hosting needs a release and none is installed.
    pub core_missing: &'static str,
    /// Hosting needs a newer release.
    pub core_unsupported: &'static str,
    /// The active release has no easytier-web.
    pub program_missing: &'static str,
    /// Opens Settings → EasyTier.
    pub open_settings: &'static str,
    /// After the first start: replace the default passwords.
    pub next_harden: &'static str,
    /// Opens the Hardening page.
    pub open_hardening: &'static str,
    /// No output yet.
    pub logs_empty: &'static str,
    /// A port field holds no number.
    pub port_not_number: &'static str,
    /// The ports are out of range or equal.
    pub ports_invalid: &'static str,

    /// Section label: the checklist.
    pub checks_label: &'static str,
    /// Check: the default admin password is replaced.
    pub check_admin: &'static str,
    /// Check: the default user password is replaced.
    pub check_user: &'static str,
    /// Check: registration is off.
    pub check_registration: &'static str,
    /// Check: the REST API listens on this computer only.
    pub check_api: &'static str,
    /// Check: the config server is reachable from the network.
    pub check_config: &'static str,
    /// Lamp word: the check passed.
    pub check_done: &'static str,
    /// Lamp word: the check failed or is still open.
    pub check_open: &'static str,
    /// Lamp word: not checked in this session.
    pub check_unknown: &'static str,
    /// Lamp word: the API is on loopback only.
    pub check_api_ok: &'static str,
    /// Lamp word: the config server is open on every interface.
    pub check_config_exposed: &'static str,
    /// What hardening does.
    pub harden_intro: &'static str,
    /// Runs hardening.
    pub harden_now: &'static str,
    /// Section label: passwords the user may choose.
    pub passwords_label: &'static str,
    /// Field label: the new admin password.
    pub field_admin_password: &'static str,
    /// Field label: the new user password.
    pub field_user_password: &'static str,
    /// Leave empty to generate; the minimum length.
    pub passwords_help: &'static str,
    /// A typed password is too short.
    pub password_too_short: &'static str,
    /// Section label: what hardening did.
    pub result_label: &'static str,
    /// Hardening finished.
    pub result_complete: &'static str,
    /// Hardening stopped early.
    pub result_stopped: &'static str,
    /// Nothing needed to change.
    pub result_already: &'static str,
    /// The admin password is in the keychain.
    pub stored_admin: &'static str,
    /// The user password is in the keychain.
    pub stored_user: &'static str,
    /// The admin password is not in the keychain.
    pub not_stored_admin: &'static str,
    /// The user password is not in the keychain.
    pub not_stored_user: &'static str,
    /// Section label: passwords kept in the keychain.
    pub reveal_label: &'static str,
    /// Shows the passwords.
    pub reveal: &'static str,
    /// Reveal is on request only.
    pub reveal_note: &'static str,
    /// Row label: the admin password.
    pub reveal_admin: &'static str,
    /// Row label: the user password.
    pub reveal_user: &'static str,
    /// No password in the keychain.
    pub reveal_none: &'static str,
    /// Selects the hosted control plane in the console.
    pub manage_in_console: &'static str,
    /// Opens the Status page, where the control plane starts.
    pub open_status: &'static str,

    /// Section label: the server.
    pub section_server: &'static str,
    /// Section label: how the server runs EasyTier.
    pub section_runtime: &'static str,
    /// Section label: ports.
    pub section_ports: &'static str,
    /// Section label: the server's own node.
    pub section_node: &'static str,
    /// Section label: hardening progress.
    pub section_hardening: &'static str,
    /// Section label: the rendered files.
    pub section_files: &'static str,
    /// Field label: the public domain.
    pub field_domain: &'static str,
    /// Helper of the domain.
    pub help_domain: &'static str,
    /// Field label: the ACME email.
    pub field_email: &'static str,
    /// Helper of the ACME email.
    pub help_email: &'static str,
    /// Selector label: the reverse proxy.
    pub field_proxy: &'static str,
    /// Proxy: Caddy.
    pub proxy_caddy: &'static str,
    /// Proxy: nginx.
    pub proxy_nginx: &'static str,
    /// What each proxy needs.
    pub help_proxy: &'static str,
    /// Selector label: the runtime.
    pub field_runtime: &'static str,
    /// Runtime: Docker Compose.
    pub runtime_compose: &'static str,
    /// Runtime: systemd.
    pub runtime_systemd: &'static str,
    /// What Docker Compose uses.
    pub help_runtime_compose: &'static str,
    /// What systemd installs.
    pub help_runtime_systemd: &'static str,
    /// Selector label: the server's CPU.
    pub field_arch: &'static str,
    /// Architecture: x86-64.
    pub arch_x86_64: &'static str,
    /// Architecture: ARM64.
    pub arch_aarch64: &'static str,
    /// Switch: the server is also a node.
    pub node_switch: &'static str,
    /// What the node does.
    pub node_description: &'static str,
    /// Field label: the node's owner account.
    pub field_owner: &'static str,
    /// Helper of the owner account.
    pub help_owner: &'static str,
    /// Selector label: hardening progress.
    pub field_hardening: &'static str,
    /// Hardening not done yet.
    pub hardening_pending: &'static str,
    /// Hardening done.
    pub hardening_done: &'static str,
    /// What the hardening progress changes.
    pub hardening_explain: &'static str,
    /// Row label: the public API path.
    pub api_path_label: &'static str,
    /// The API path is closed at the proxy.
    pub api_closed: &'static str,
    /// The API path is proxied.
    pub api_open: &'static str,
    /// Nothing is applied from here.
    pub files_note: &'static str,
    /// Meta label before a file's permissions.
    pub file_mode_label: &'static str,
    /// After 0600: only the owner can read.
    pub mode_owner_only: &'static str,
    /// After 0755: runnable.
    pub mode_executable: &'static str,
    /// The pinned release data is invalid.
    pub error_pinned: &'static str,
    /// Field name used in errors: the domain.
    pub error_field_domain: &'static str,
    /// Field name used in errors: the email.
    pub error_field_email: &'static str,
    /// Field name used in errors: the REST API port.
    pub error_field_api_port: &'static str,
    /// Field name used in errors: the config server port.
    pub error_field_config_port: &'static str,
    /// Field name used in errors: the owner.
    pub error_field_owner: &'static str,
    /// Error: empty (`{field}`).
    pub reason_empty: &'static str,
    /// Error: too long (`{field}`).
    pub reason_too_long: &'static str,
    /// Error: too short (`{field}`).
    pub reason_too_short: &'static str,
    /// Error: a character that is not allowed (`{field}`).
    pub reason_bad_character: &'static str,
    /// Error: wrong shape (`{field}`).
    pub reason_bad_format: &'static str,
    /// Error: reserved or guessable (`{field}`).
    pub reason_reserved: &'static str,
    /// Error: collides with another setting (`{field}`).
    pub reason_conflict: &'static str,
    /// Error: out of range (`{field}`).
    pub reason_out_of_range: &'static str,
    /// Error: not https (`{field}`).
    pub reason_insecure: &'static str,
    /// Error: does not match the release (`{field}`).
    pub reason_mismatch: &'static str,

    /// Problem: no release installed.
    pub problem_core_missing: &'static str,
    /// Problem: the release is too old (its version follows in monospace).
    pub problem_core_unsupported: &'static str,
    /// Problem: no easytier-web in the release.
    pub problem_program_missing: &'static str,
    /// Problem: settings invalid.
    pub problem_ports: &'static str,
    /// Problem: the saved settings are unreadable.
    pub problem_settings_corrupt: &'static str,
    /// Problem: the REST API port is taken (the port follows in monospace).
    pub problem_port_api: &'static str,
    /// Problem: the config server port is taken (the port follows in monospace).
    pub problem_port_config: &'static str,
    /// Problem: already running.
    pub problem_already_running: &'static str,
    /// Problem: not running.
    pub problem_not_running: &'static str,
    /// Problem: could not start (the reason follows in monospace).
    pub problem_spawn: &'static str,
    /// Problem: could not stop (the reason follows in monospace).
    pub problem_stop: &'static str,
    /// Problem: exited on its own (the last output follows in monospace).
    pub problem_exited: &'static str,
    /// Problem: the API never answered.
    pub problem_timeout: &'static str,
    /// Problem: a file failed (the OS message follows in monospace).
    pub problem_files: &'static str,
    /// Problem: the keychain failed (the message follows in monospace).
    pub problem_keychain: &'static str,
    /// Problem: no secure random source.
    pub problem_random: &'static str,
    /// Problem: a password is empty (the account follows in monospace).
    pub problem_password_empty: &'static str,
    /// Problem: a password is too short (the account follows in monospace).
    pub problem_password_short: &'static str,
    /// Problem: the API failed (the message follows in monospace).
    pub problem_api: &'static str,
    /// Problem: an account was changed elsewhere (the account follows in monospace).
    pub problem_password_unknown: &'static str,
    /// Problem: a password change could not be confirmed (the account follows in monospace).
    pub problem_unconfirmed: &'static str,
    /// Problem: registration is still open.
    pub problem_registration_open: &'static str,
    /// Problem: the task ended without a result.
    pub problem_interrupted: &'static str,
    /// Problem: not available on this platform.
    pub problem_unavailable: &'static str,

    /// Summary label: the control plane's state.
    pub state_label: &'static str,
    /// Kind chip in a drawer header: the hosted control plane.
    pub kind_control_plane: &'static str,
    /// Opens the settings drawer.
    pub edit_settings: &'static str,
    /// Title of the settings drawer.
    pub settings_title: &'static str,
    /// Fact label: where the REST API listens.
    pub address_api: &'static str,
    /// Fact label: where the config server listens.
    pub address_config: &'static str,
    /// Fact label: whether anyone can register accounts.
    pub registration_label: &'static str,
    /// Fact value: registration is off.
    pub registration_off: &'static str,
    /// Fact value: registration is open.
    pub registration_on: &'static str,
    /// Table column: what is checked.
    pub col_check: &'static str,
    /// Table column: what the check covers.
    pub col_detail: &'static str,
    /// Table column: how the check stands.
    pub col_status: &'static str,
    /// What the admin check covers.
    pub detail_admin: &'static str,
    /// What the user check covers.
    pub detail_user: &'static str,
    /// What the registration check covers.
    pub detail_registration: &'static str,
    /// What the API check covers.
    pub detail_api: &'static str,
    /// What the config server check covers.
    pub detail_config: &'static str,
    /// Check: the owner account is registered.
    pub check_owner: &'static str,
    /// What the owner check covers.
    pub detail_owner: &'static str,
    /// Row action: registers the owner account.
    pub action_register: &'static str,
    /// Row action: opens the Status page's settings.
    pub action_view_settings: &'static str,
    /// How many checks are done (`{done}`, `{total}`).
    pub checks_summary: &'static str,
    /// Section label: the owner account.
    pub owner_label: &'static str,
    /// What the owner account is for and what the step asks of the person.
    pub owner_intro: &'static str,
    /// Starts the owner registration.
    pub owner_register: &'static str,
    /// The registration window is opening.
    pub owner_preparing: &'static str,
    /// Field label: the text in the captcha image.
    pub owner_captcha_label: &'static str,
    /// Helper of the captcha field.
    pub owner_captcha_help: &'static str,
    /// Accessible name of the captcha image.
    pub owner_captcha_image: &'static str,
    /// Submits the captcha text.
    pub owner_submit: &'static str,
    /// The owner account is registered.
    pub owner_done: &'static str,
    /// Field label: the enrollment address.
    pub owner_url_label: &'static str,
    /// What the enrollment address is for and why it is a secret.
    pub owner_url_note: &'static str,
    /// The enrollment address holds a placeholder to replace.
    pub owner_url_placeholder: &'static str,
    /// Problem: the captcha text did not match.
    pub problem_owner_captcha: &'static str,
    /// Problem: an owner account already exists.
    pub problem_owner_already: &'static str,
    /// Problem: the registration window closed.
    pub problem_owner_not_started: &'static str,

    /// Toolbar chip: how many files the plan renders (`{count}`).
    pub files_count: &'static str,
    /// Table column: the file's name.
    pub col_file: &'static str,
    /// Table column: the file's permissions.
    pub col_mode: &'static str,
    /// Table column: the file's size.
    pub col_size: &'static str,
}

pub(super) static EN: HostStrings = HostStrings {
    section_control: "Control plane",
    section_settings: "Settings",
    section_addresses: "Where it listens",
    section_logs: "Output",
    version_label: "Release",
    api_label: "REST API",
    api_scope: "This computer only",
    config_label: "Config server",
    config_scope: "Every network interface",
    start: "Start control plane",
    stop: "Stop",
    restart: "Restart",
    save_settings: "Save settings",
    field_api_port: "REST API port",
    help_api_port: "The REST API always listens on this computer only.",
    field_config_port: "Config server port",
    help_config_port: "Machines you manage fetch their configs over this port.",
    field_protocol: "Config server protocol",
    protocol_udp: "UDP",
    protocol_tcp: "TCP",
    protocol_ws: "WebSocket",
    settings_applies: "Changes apply the next time the control plane starts.",
    exposure_note: "The config server listens on every network interface, not only on this computer. Anyone who \
                    can reach that port can try to enroll a machine, so allow it through a firewall only on \
                    networks you trust. It listens on:",
    settings_corrupt: "The saved settings couldn't be read, so the defaults are shown. Saving replaces them.",
    core_missing: "Hosting needs an EasyTier release, and none is installed.",
    core_unsupported: "The active EasyTier release is older than this app supports.",
    program_missing: "The active EasyTier release has no easytier-web program.",
    open_settings: "Open EasyTier settings",
    next_harden: "Next: replace the default accounts' passwords and register the owner account before anyone else \
                  can reach this control plane.",
    open_hardening: "Open hardening",
    logs_empty: "No output yet.",
    port_not_number: "Enter a port number.",
    ports_invalid: "Ports run from 1024 to 65535, and the two must differ.",

    checks_label: "Checklist",
    check_admin: "Default admin password replaced",
    check_user: "Default user password replaced",
    check_registration: "Registration disabled",
    check_api: "REST API reachable from this computer only",
    check_config: "Config server exposure",
    check_done: "Done",
    check_open: "Not done",
    check_unknown: "Not checked yet",
    check_api_ok: "This computer only",
    check_config_exposed: "Open on every interface",
    harden_intro: "easytier-web starts with two well-known accounts. Hardening replaces both passwords and keeps the \
                   new ones in your system keychain. Registration is off from the first start; the owner account \
                   below is the one account you create.",
    harden_now: "Harden now",
    passwords_label: "New passwords",
    field_admin_password: "New admin password",
    field_user_password: "New user password",
    passwords_help: "Leave a field empty to generate a strong password. A password you type needs at least 8 \
                     characters.",
    password_too_short: "Use at least 8 characters, or leave it empty to generate one.",
    result_label: "Result",
    result_complete: "Hardening is complete.",
    result_stopped: "Hardening stopped before it finished. Nothing was left half-changed that the checklist doesn't \
                     show.",
    result_already: "Both default passwords were already replaced, so nothing was changed.",
    stored_admin: "The admin password is in your keychain.",
    stored_user: "The user password is in your keychain.",
    not_stored_admin: "The admin password is not in your keychain, so the console can't sign in with it.",
    not_stored_user: "The user password is not in your keychain.",
    reveal_label: "Passwords in the keychain",
    reveal: "Reveal passwords",
    reveal_note: "They are shown only when you ask. Copy them into a password manager.",
    reveal_admin: "Admin password",
    reveal_user: "User password",
    reveal_none: "Not in the keychain",
    manage_in_console: "Manage in console",
    open_status: "Open status",

    section_server: "Server",
    section_runtime: "How it runs",
    section_ports: "Ports",
    section_node: "The server as a node",
    section_hardening: "Hardening",
    section_files: "Files",
    field_domain: "Domain",
    help_domain: "A public domain name that points at the server.",
    field_email: "ACME contact email (optional)",
    help_email: "The certificate authority writes to it before a certificate expires. Leave it empty to skip.",
    field_proxy: "Reverse proxy",
    proxy_caddy: "Caddy",
    proxy_nginx: "nginx",
    help_proxy: "Caddy gets and renews HTTPS certificates by itself. With nginx you bring the certificates; \
                 certbot works.",
    field_runtime: "Runtime",
    runtime_compose: "Docker Compose",
    runtime_systemd: "systemd",
    help_runtime_compose: "Runs the official EasyTier image, pinned by its digest.",
    help_runtime_systemd: "Installs the official release zip after checking its sha256, and runs it as services.",
    field_arch: "Server CPU",
    arch_x86_64: "x86-64",
    arch_aarch64: "ARM64",
    node_switch: "Also run a node on this server",
    node_description: "Adds an easytier-core that enrolls with this control plane, so the server can relay \
                       traffic.",
    field_owner: "Owner account name",
    help_owner: "The easytier-web account the node enrolls under. You register it during hardening. The names \
                 admin and user are not allowed.",
    field_hardening: "Hardening",
    hardening_pending: "Not done yet",
    hardening_done: "Done",
    hardening_explain: "Until hardening is done, the reverse proxy keeps the API closed and registration stays open \
                        so you can create the owner account. Reach the API through an SSH tunnel to finish \
                        hardening, then set this to Done and copy the files again.",
    api_path_label: "Public API path",
    api_closed: "Closed",
    api_open: "Open",
    files_note: "Nothing is applied from here. Copy each file to your server, set the permissions shown, and \
                 follow the README, which comes first.",
    file_mode_label: "Permissions",
    mode_owner_only: "owner only",
    mode_executable: "executable",
    error_pinned: "The release data pinned in this app is invalid. Update the app.",
    error_field_domain: "Domain",
    error_field_email: "ACME email",
    error_field_api_port: "REST API port",
    error_field_config_port: "Config server port",
    error_field_owner: "Owner account name",
    reason_empty: "{field} is required.",
    reason_too_long: "{field} is too long.",
    reason_too_short: "{field} is too short.",
    reason_bad_character: "{field} contains a character that isn't allowed.",
    reason_bad_format: "{field} doesn't look right.",
    reason_reserved: "{field} is reserved or too easy to guess.",
    reason_conflict: "{field} collides with another setting or a port the proxy needs.",
    reason_out_of_range: "{field} is out of range.",
    reason_insecure: "{field} must be a plain https address.",
    reason_mismatch: "{field} doesn't match the selected release.",

    problem_core_missing: "EasyTier isn't installed. Install a release in Settings.",
    problem_core_unsupported: "The active EasyTier release is older than this app supports.",
    problem_program_missing: "The active EasyTier release has no easytier-web program.",
    problem_ports: "The ports can't be used.",
    problem_settings_corrupt: "The saved settings couldn't be read.",
    problem_port_api: "Another program already uses the REST API port.",
    problem_port_config: "Another program already uses the config server port.",
    problem_already_running: "The control plane is already running.",
    problem_not_running: "The control plane isn't running.",
    problem_spawn: "easytier-web couldn't start.",
    problem_stop: "easytier-web couldn't be stopped.",
    problem_exited: "easytier-web stopped unexpectedly.",
    problem_timeout: "easytier-web started, but its API didn't answer in time.",
    problem_files: "A file in the app's data folder couldn't be read or written.",
    problem_keychain: "The system keychain refused the request.",
    problem_random: "No secure random source is available to generate a password.",
    problem_password_empty: "A password is empty.",
    problem_password_short: "A password is too short.",
    problem_api: "The control plane's API answered something unexpected.",
    problem_password_unknown: "An account was changed elsewhere and its password isn't in your keychain, so it can't \
                              be managed from here.",
    problem_unconfirmed: "A password change couldn't be confirmed.",
    problem_registration_open: "Registration is still open after the restart.",
    problem_interrupted: "The task ended without a result. Try again.",
    problem_unavailable: "This isn't available on this platform.",

    state_label: "State",
    kind_control_plane: "Control plane",
    edit_settings: "Edit settings",
    settings_title: "Control plane settings",
    address_api: "REST API address",
    address_config: "Config server address",
    registration_label: "Registration",
    registration_off: "Off",
    registration_on: "Open",
    col_check: "Check",
    col_detail: "What it covers",
    col_status: "Status",
    detail_admin: "easytier-web ships with a well-known admin account. Until its password is replaced, anyone who \
                   reaches the API can sign in.",
    detail_user: "The seeded user account has the same problem and the same fix.",
    detail_registration: "With registration off nobody can create accounts. It stays off from the first start.",
    detail_api: "The REST API only listens on 127.0.0.1.",
    detail_config: "Machines connect here, so it has to be reachable. Allow it through a firewall only on networks \
                    you trust.",
    check_owner: "Owner account registered",
    detail_owner: "Machines enroll under this account, whose name nobody can guess, rather than under admin or \
                   user.",
    action_register: "Register",
    action_view_settings: "View settings",
    checks_summary: "{done} of {total} checks done",
    owner_label: "Owner account",
    owner_intro: "Machines enroll under an owner account whose name nobody can guess. easytier-web asks for a \
                  captcha before it creates an account, so this step needs you: registration opens on this \
                  computer only, for as long as it takes.",
    owner_register: "Register owner account",
    owner_preparing: "Preparing\u{2026}",
    owner_captcha_label: "Text in the image",
    owner_captcha_help: "Type the characters you see. The image is valid for this registration only.",
    owner_captcha_image: "Captcha image",
    owner_submit: "Create account",
    owner_done: "The owner account is registered. Its name and password are in your keychain.",
    owner_url_label: "Enrollment address",
    owner_url_note: "Give this address to a machine to enroll it under the owner account. It contains the account \
                     name, so treat it like a password.",
    owner_url_placeholder: "No network address was found for this computer. Replace <address-of-this-computer> \
                            with the address other machines reach it by.",
    problem_owner_captcha: "That text doesn't match the image. Try again.",
    problem_owner_already: "An owner account is already registered.",
    problem_owner_not_started: "The registration window closed. Register again.",

    files_count: "Files: {count}",
    col_file: "File",
    col_mode: "Permissions",
    col_size: "Size",
};

pub(super) static ZH_CN: HostStrings = HostStrings {
    section_control: "控制平面",
    section_settings: "设置",
    section_addresses: "监听位置",
    section_logs: "输出",
    version_label: "版本",
    api_label: "REST API",
    api_scope: "仅限本机",
    config_label: "配置服务器",
    config_scope: "所有网络接口",
    start: "启动控制平面",
    stop: "停止",
    restart: "重启",
    save_settings: "保存设置",
    field_api_port: "REST API 端口",
    help_api_port: "REST API 始终只监听本机。",
    field_config_port: "配置服务器端口",
    help_config_port: "受管理的机器通过此端口获取配置。",
    field_protocol: "配置服务器协议",
    protocol_udp: "UDP",
    protocol_tcp: "TCP",
    protocol_ws: "WebSocket",
    settings_applies: "更改会在下次启动控制平面时生效。",
    exposure_note: "配置服务器会监听本机所有的网络接口，而不只是本机。能访问该端口的人都可以尝试注册机器，\
                    所以只应在你信任的网络上让防火墙放行它。监听位置：",
    settings_corrupt: "无法读取已保存的设置，当前显示的是默认值。保存后会将其替换。",
    core_missing: "托管控制平面需要先安装 EasyTier，目前尚未安装。",
    core_unsupported: "当前使用的 EasyTier 版本低于本应用支持的最低版本。",
    program_missing: "当前使用的 EasyTier 版本不含 easytier-web 程序。",
    open_settings: "打开 EasyTier 设置",
    next_harden: "下一步：在其他人能访问此控制平面之前，先替换默认账号的密码，并注册所属账号。",
    open_hardening: "打开安全加固",
    logs_empty: "暂无输出。",
    port_not_number: "请输入端口号。",
    ports_invalid: "端口范围为 1024 至 65535，且两个端口不能相同。",

    checks_label: "检查清单",
    check_admin: "已替换默认的 admin 密码",
    check_user: "已替换默认的 user 密码",
    check_registration: "已关闭注册",
    check_api: "REST API 仅限本机访问",
    check_config: "配置服务器的暴露情况",
    check_done: "已完成",
    check_open: "未完成",
    check_unknown: "尚未检查",
    check_api_ok: "仅限本机",
    check_config_exposed: "所有接口均可访问",
    harden_intro: "easytier-web 启动时带有两个众所周知的账号。安全加固会替换这两个密码，并把新密码保存到系统钥匙串。\
                   注册从首次启动起就是关闭的；下面的所属账号是你要创建的唯一账号。",
    harden_now: "立即加固",
    passwords_label: "新密码",
    field_admin_password: "新的 admin 密码",
    field_user_password: "新的 user 密码",
    passwords_help: "留空则自动生成高强度密码。手动输入的密码至少需要 8 个字符。",
    password_too_short: "至少 8 个字符；留空则自动生成。",
    result_label: "结果",
    result_complete: "安全加固已完成。",
    result_stopped: "安全加固在完成前中止。清单未显示的内容没有被改动。",
    result_already: "两个默认密码早已替换，因此没有做任何更改。",
    stored_admin: "admin 密码已保存在钥匙串中。",
    stored_user: "user 密码已保存在钥匙串中。",
    not_stored_admin: "admin 密码不在钥匙串中，控制台无法用它登录。",
    not_stored_user: "user 密码不在钥匙串中。",
    reveal_label: "钥匙串中的密码",
    reveal: "显示密码",
    reveal_note: "只在你要求时显示。请复制到密码管理器中保存。",
    reveal_admin: "admin 密码",
    reveal_user: "user 密码",
    reveal_none: "不在钥匙串中",
    manage_in_console: "在控制台中管理",
    open_status: "打开状态页",

    section_server: "服务器",
    section_runtime: "运行方式",
    section_ports: "端口",
    section_node: "服务器自身作为节点",
    section_hardening: "安全加固",
    section_files: "文件",
    field_domain: "域名",
    help_domain: "指向这台服务器的公网域名。",
    field_email: "ACME 联系邮箱（可选）",
    help_email: "证书到期前，证书颁发机构会写信到这个邮箱。留空则不填。",
    field_proxy: "反向代理",
    proxy_caddy: "Caddy",
    proxy_nginx: "nginx",
    help_proxy: "Caddy 会自动申请并续期 HTTPS 证书；使用 nginx 则需要你自备证书（certbot 可用）。",
    field_runtime: "运行环境",
    runtime_compose: "Docker Compose",
    runtime_systemd: "systemd",
    help_runtime_compose: "运行官方 EasyTier 镜像，并用摘要固定版本。",
    help_runtime_systemd: "校验 sha256 后安装官方发布压缩包，并以服务方式运行。",
    field_arch: "服务器 CPU",
    arch_x86_64: "x86-64",
    arch_aarch64: "ARM64",
    node_switch: "让这台服务器同时作为节点",
    node_description: "加入一个注册到此控制平面的 easytier-core，使服务器可以中继流量。",
    field_owner: "所属账号名",
    help_owner: "节点注册到的 easytier-web 账号，需要你在加固时自行注册。不能使用 admin 和 user。",
    field_hardening: "安全加固",
    hardening_pending: "尚未完成",
    hardening_done: "已完成",
    hardening_explain: "加固完成之前，反向代理会保持 API 关闭，并保持注册开放，方便你创建所属账号。\
                        请通过 SSH 隧道访问 API 完成加固，然后把这里改为“已完成”，再重新复制文件。",
    api_path_label: "公开的 API 路径",
    api_closed: "已关闭",
    api_open: "已开放",
    files_note: "这里不会执行任何操作。请把每个文件复制到服务器，按所示设置权限，并按 README 的步骤操作（README 排在最前）。",
    file_mode_label: "权限",
    mode_owner_only: "仅所有者",
    mode_executable: "可执行",
    error_pinned: "应用内固定的发布数据无效，请更新应用。",
    error_field_domain: "域名",
    error_field_email: "ACME 邮箱",
    error_field_api_port: "REST API 端口",
    error_field_config_port: "配置服务器端口",
    error_field_owner: "所属账号名",
    reason_empty: "请填写{field}。",
    reason_too_long: "{field}太长。",
    reason_too_short: "{field}太短。",
    reason_bad_character: "{field}含有不允许的字符。",
    reason_bad_format: "{field}的格式不正确。",
    reason_reserved: "{field}是保留名称，或太容易被猜到。",
    reason_conflict: "{field}与其他设置或代理需要的端口冲突。",
    reason_out_of_range: "{field}超出范围。",
    reason_insecure: "{field}必须是普通的 https 地址。",
    reason_mismatch: "{field}与所选版本不匹配。",

    problem_core_missing: "尚未安装 EasyTier，请先在设置中安装一个版本。",
    problem_core_unsupported: "当前使用的 EasyTier 版本低于本应用支持的最低版本。",
    problem_program_missing: "当前使用的 EasyTier 版本不含 easytier-web 程序。",
    problem_ports: "这些端口无法使用。",
    problem_settings_corrupt: "无法读取已保存的设置。",
    problem_port_api: "REST API 端口已被其他程序占用。",
    problem_port_config: "配置服务器端口已被其他程序占用。",
    problem_already_running: "控制平面已在运行。",
    problem_not_running: "控制平面没有运行。",
    problem_spawn: "easytier-web 无法启动。",
    problem_stop: "easytier-web 无法停止。",
    problem_exited: "easytier-web 意外退出。",
    problem_timeout: "easytier-web 已启动，但其 API 没有及时响应。",
    problem_files: "无法读取或写入应用数据文件夹中的文件。",
    problem_keychain: "系统钥匙串拒绝了请求。",
    problem_random: "没有可用的安全随机源，无法生成密码。",
    problem_password_empty: "有一个密码为空。",
    problem_password_short: "有一个密码太短。",
    problem_api: "控制平面的 API 返回了意外的内容。",
    problem_password_unknown: "有一个账号已在别处被修改，且它的密码不在你的钥匙串中，因此无法在这里管理。",
    problem_unconfirmed: "无法确认密码已更改。",
    problem_registration_open: "重启后注册仍然开放。",
    problem_interrupted: "任务意外结束，没有得到结果，请重试。",
    problem_unavailable: "此平台不支持该功能。",

    state_label: "状态",
    kind_control_plane: "控制平面",
    edit_settings: "编辑设置",
    settings_title: "控制平面设置",
    address_api: "REST API 地址",
    address_config: "配置服务器地址",
    registration_label: "注册",
    registration_off: "已关闭",
    registration_on: "开放",
    col_check: "检查项",
    col_detail: "检查内容",
    col_status: "状态",
    detail_admin: "easytier-web 自带一个众所周知的 admin 账号。在密码被替换之前，任何能访问 API 的人都能登录。",
    detail_user: "预置的 user 账号有同样的问题，处理方式也一样。",
    detail_registration: "注册关闭后，任何人都无法创建账号。首次启动起它就保持关闭。",
    detail_api: "REST API 只监听 127.0.0.1。",
    detail_config: "机器通过这里连接，所以它必须可达。只在你信任的网络上让防火墙放行它。",
    check_owner: "已注册所属账号",
    detail_owner: "机器注册到这个名称无法被猜到的账号，而不是 admin 或 user。",
    action_register: "注册",
    action_view_settings: "查看设置",
    checks_summary: "已完成 {done}/{total} 项检查",
    owner_label: "所属账号",
    owner_intro: "机器会注册到一个名称无法被猜到的所属账号。easytier-web 在创建账号前要求输入验证码，所以这一步需要你：\
                  注册只在这台电脑上、在所需的短暂时间内开放。",
    owner_register: "注册所属账号",
    owner_preparing: "准备中\u{2026}",
    owner_captcha_label: "图片中的文字",
    owner_captcha_help: "输入你看到的字符。该图片只对这次注册有效。",
    owner_captcha_image: "验证码图片",
    owner_submit: "创建账号",
    owner_done: "所属账号已注册，它的名称和密码已保存在钥匙串中。",
    owner_url_label: "注册地址",
    owner_url_note: "把这个地址交给要注册的机器，它就会归入所属账号。地址里含有账号名，请像密码一样对待。",
    owner_url_placeholder: "没有找到这台电脑的网络地址。请把 <address-of-this-computer> 替换为其他机器访问它所用的地址。",
    problem_owner_captcha: "输入的文字与图片不符，请重试。",
    problem_owner_already: "已经注册过所属账号。",
    problem_owner_not_started: "注册窗口已关闭，请重新注册。",

    files_count: "文件：{count}",
    col_file: "文件",
    col_mode: "权限",
    col_size: "大小",
};
