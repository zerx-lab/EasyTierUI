//! Node role: the import dialog and the "join a control plane" dialog.
//!
//! One field per string; `EN` and `ZH_CN` must both set every field.

/// Strings of this area in one language.
#[derive(Debug)]
pub struct ImportStrings {
    /// Import dialog: title.
    pub title: &'static str,
    /// Input: the field's label.
    pub input_label: &'static str,
    /// Input: the field's placeholder (a sketch of the two accepted shapes).
    pub input_placeholder: &'static str,
    /// Input: helper under the field.
    pub input_help: &'static str,
    /// Input: the button that picks a QR code image.
    pub pick_image: &'static str,
    /// Input: nothing is applied or started before Save.
    pub nothing_applied: &'static str,
    /// Input: the Preview button.
    pub action_preview: &'static str,
    /// Preview: the Back button.
    pub action_back: &'static str,
    /// Preview: the Save button.
    pub action_save: &'static str,
    /// Preview: heading.
    pub preview_heading: &'static str,
    /// Preview: what the screen is.
    pub preview_help: &'static str,
    /// Preview: the display-name field's helper.
    pub name_help: &'static str,
    /// Preview: heading of the warnings about wider exposure.
    pub exposures_heading: &'static str,
    /// Preview: row label, network name.
    pub row_network: &'static str,
    /// Preview: row label, how the address is chosen.
    pub row_address: &'static str,
    /// Preview: the address is picked automatically.
    pub address_dhcp: &'static str,
    /// Preview: the address is fixed (the address follows in monospace).
    pub address_static: &'static str,
    /// Preview: the node has no virtual address.
    pub address_none: &'static str,
    /// Preview: row label, hostname.
    pub row_hostname: &'static str,
    /// Preview: the device's own hostname is used.
    pub hostname_default: &'static str,
    /// Preview: row label, peers it connects to.
    pub row_peers: &'static str,
    /// Preview: row label, listeners.
    pub row_listeners: &'static str,
    /// Preview: label before a listener's port.
    pub listener_port: &'static str,
    /// Preview: a listener binds every interface.
    pub listener_all_interfaces: &'static str,
    /// Preview: row label, exit nodes it uses.
    pub row_exit_nodes: &'static str,
    /// Preview: row label, subnets it shares.
    pub row_subnets: &'static str,
    /// Preview: row label, port forwards.
    pub row_forwards: &'static str,
    /// Preview: a forward is reachable beyond this device.
    pub forward_public: &'static str,
    /// Preview: row label, SOCKS5 proxy.
    pub row_socks5: &'static str,
    /// Preview: row label, encryption.
    pub row_encryption: &'static str,
    /// Preview: traffic is not protected.
    pub encryption_off: &'static str,
    /// Preview: the XOR algorithm is obfuscation only.
    pub encryption_obfuscation: &'static str,
    /// Preview: row label, private mode.
    pub row_private_mode: &'static str,
    /// Preview: row label, network interface.
    pub row_tun: &'static str,
    /// Preview: it creates a TUN interface (needs administrator rights).
    pub tun_yes: &'static str,
    /// Preview: it runs without a TUN interface.
    pub tun_no: &'static str,
    /// Preview: this device cannot create a TUN interface, so it runs without.
    pub tun_unsupported: &'static str,
    /// Preview: a yes.
    pub yes: &'static str,
    /// Preview: a no.
    pub no: &'static str,
    /// Preview: an empty list.
    pub none: &'static str,
    /// Preview: callout, settings that were not applied.
    pub ignored_heading: &'static str,
    /// Preview: callout, the config would have opened default listeners.
    pub listeners_dropped: &'static str,
    /// Preview: the secret came with the import and is kept in the keychain.
    pub secret_kept: &'static str,
    /// Preview: the import has no secret, so one can be typed.
    pub secret_missing: &'static str,
    /// Preview: the secret field's helper.
    pub secret_help: &'static str,
    /// Preview: an imported network never starts by itself.
    pub never_starts: &'static str,
    /// Row: manual routes.
    pub row_routes: &'static str,
    /// Manual routes are on with no routes.
    pub routes_empty: &'static str,
    /// Row: mapped listeners.
    pub row_mapped_listeners: &'static str,
    /// Row: the VPN portal.
    pub row_vpn_portal: &'static str,
    /// Row: the receive speed limit.
    pub row_recv_limit: &'static str,
    /// The limit; `{limit}` is bytes per second.
    pub recv_limit_value: &'static str,
    /// Row: access control.
    pub row_acl: &'static str,
    /// The access control's size; `{chains}` and `{rules}` are counts.
    pub acl_summary: &'static str,
    /// Declared groups lack secrets; `{count}` is how many.
    pub acl_groups_missing: &'static str,
    /// Managed dialog: title.
    pub managed_title: &'static str,
    /// Managed dialog: what it is.
    pub managed_explain: &'static str,
    /// Managed dialog: the address field's label.
    pub managed_server_label: &'static str,
    /// Managed dialog: the address field's placeholder (the shape, not a real server).
    pub managed_server_placeholder: &'static str,
    /// Managed dialog: the address field's helper.
    pub managed_server_help: &'static str,
    /// Managed dialog: the hostname field's helper.
    pub managed_hostname_help: &'static str,
    /// Managed dialog: the Enroll button.
    pub managed_enroll: &'static str,
}

pub(super) static EN: ImportStrings = ImportStrings {
    title: "Import a network",
    input_label: "Config or share link",
    input_placeholder: "Paste an EasyTier config (TOML) or an easytierui://import?… link",
    input_help: "Nothing you paste leaves this device.",
    pick_image: "Pick a QR code image…",
    nothing_applied: "Nothing is applied or started until you save.",
    action_preview: "Preview",
    action_back: "Back",
    action_save: "Save network",
    preview_heading: "What this import would do",
    preview_help: "Check it before you save. Nothing has been applied yet.",
    name_help: "The name shown in this app.",
    exposures_heading: "Review these first",
    row_network: "Network",
    row_address: "Virtual IP",
    address_dhcp: "Chosen automatically",
    address_static: "Fixed address",
    address_none: "No virtual IP (relay only)",
    row_hostname: "Hostname",
    hostname_default: "This device's own",
    row_peers: "Peers",
    row_listeners: "Listeners",
    listener_port: "Port",
    listener_all_interfaces: "All network interfaces",
    row_exit_nodes: "Exit nodes",
    row_subnets: "Shared subnets",
    row_forwards: "Port forwards",
    forward_public: "Reachable beyond this device",
    row_socks5: "SOCKS5 proxy",
    row_encryption: "Encryption",
    encryption_off: "Off: peer traffic is not protected",
    encryption_obfuscation: "Obfuscation only, not encryption",
    row_private_mode: "Private mode",
    row_tun: "Network interface",
    tun_yes: "Creates a TUN interface (needs administrator rights)",
    tun_no: "Runs without TUN",
    tun_unsupported: "This device cannot create a TUN interface, so it will run without TUN",
    yes: "On",
    no: "Off",
    none: "None",
    ignored_heading: "These settings were not applied",
    listeners_dropped: "This config does not set listeners, so EasyTier would have opened its default listeners on every network interface. The import has no listeners instead.",
    secret_kept: "The network secret comes with this import and will be kept in the keychain.",
    secret_missing: "This import has no network secret. Enter it to join a private network.",
    secret_help: "Leave it empty only if the network has no secret.",
    never_starts: "An imported network never starts by itself. Start it from the list when you are ready.",
    row_routes: "Manual routes",
    routes_empty: "On, with no routes",
    row_mapped_listeners: "Mapped listeners",
    row_vpn_portal: "VPN portal",
    row_recv_limit: "Receive speed limit",
    recv_limit_value: "{limit} bytes per second",
    row_acl: "Access control",
    acl_summary: "{chains} chains, {rules} rules",
    acl_groups_missing: "{count} declared groups have no secret. Add them in the access-control settings before starting.",
    managed_title: "Join a control plane",
    managed_explain: "This network is managed by a control plane (easytier-web). It runs as your own process without TUN, so there is no administrator prompt, and its settings become read-only here.",
    managed_server_label: "Control plane address",
    managed_server_placeholder: "udp://host:22020/account",
    managed_server_help: "It identifies your account on the control plane, so it is kept as a secret.",
    managed_hostname_help: "Optional. Leave empty to use this device's own hostname.",
    managed_enroll: "Enroll and start",
};

pub(super) static ZH_CN: ImportStrings = ImportStrings {
    title: "导入网络",
    input_label: "配置或分享链接",
    input_placeholder: "粘贴 EasyTier 配置（TOML）或 easytierui://import?… 链接",
    input_help: "粘贴的内容不会离开这台设备。",
    pick_image: "选择二维码图片…",
    nothing_applied: "保存之前不会应用或启动任何内容。",
    action_preview: "预览",
    action_back: "返回",
    action_save: "保存网络",
    preview_heading: "导入后会发生什么",
    preview_help: "保存前请先确认。目前尚未应用任何内容。",
    name_help: "在本应用中显示的名称。",
    exposures_heading: "请先留意这些",
    row_network: "网络",
    row_address: "虚拟 IP",
    address_dhcp: "自动分配",
    address_static: "固定地址",
    address_none: "无虚拟 IP（仅中继）",
    row_hostname: "主机名",
    hostname_default: "使用本设备的主机名",
    row_peers: "对等节点",
    row_listeners: "监听",
    listener_port: "端口",
    listener_all_interfaces: "所有网络接口",
    row_exit_nodes: "出口节点",
    row_subnets: "共享的子网",
    row_forwards: "端口转发",
    forward_public: "本设备以外也能访问",
    row_socks5: "SOCKS5 代理",
    row_encryption: "加密",
    encryption_off: "关闭：对等节点之间的流量不受保护",
    encryption_obfuscation: "仅混淆，不是加密",
    row_private_mode: "私有模式",
    row_tun: "网络接口",
    tun_yes: "会创建 TUN 接口（需要管理员权限）",
    tun_no: "不使用 TUN 运行",
    tun_unsupported: "本设备无法创建 TUN 接口，将不使用 TUN 运行",
    yes: "开",
    no: "关",
    none: "无",
    ignored_heading: "以下设置未被应用",
    listeners_dropped: "此配置没有设置监听，EasyTier 会在所有网络接口上打开默认监听。导入时改为不设置任何监听。",
    secret_kept: "网络密钥随此次导入而来，将保存在钥匙串中。",
    secret_missing: "此次导入不含网络密钥。加入私有网络请在此输入。",
    secret_help: "只有网络本身没有密钥时才可留空。",
    never_starts: "导入的网络不会自动启动。准备好后请在列表中手动启动。",
    row_routes: "手动路由",
    routes_empty: "已开启，但没有任何路由",
    row_mapped_listeners: "映射监听地址",
    row_vpn_portal: "VPN 门户",
    row_recv_limit: "接收速率限制",
    recv_limit_value: "{limit} 字节/秒",
    row_acl: "访问控制",
    acl_summary: "{chains} 条链，{rules} 条规则",
    acl_groups_missing: "有 {count} 个声明的分组没有密钥。启动前请在访问控制设置中补上。",
    managed_title: "加入控制平面",
    managed_explain: "此网络由控制平面（easytier-web）管理。它以你自己的进程运行，不使用 TUN，因此不会弹出管理员授权，其设置在这里将变为只读。",
    managed_server_label: "控制平面地址",
    managed_server_placeholder: "udp://host:22020/account",
    managed_server_help: "它标识你在控制平面上的账户，因此作为密钥保存。",
    managed_hostname_help: "可选。留空则使用本设备的主机名。",
    managed_enroll: "加入并启动",
};
