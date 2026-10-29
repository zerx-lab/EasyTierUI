//! Node role: words several pages share — field names, what is wrong with a value, why a save failed, what an
//! import refused, what a setting exposes and why a network is not running.
//!
//! Sentences never carry machine data (addresses, versions, exit codes): pages show those in monospace beside them.

/// Strings of this area in one language.
#[derive(Debug)]
pub struct SharedStrings {
    /// Field: the name shown in the app.
    pub field_name: &'static str,
    /// Field: the hostname peers see.
    pub field_hostname: &'static str,
    /// Field: the EasyTier network name.
    pub field_network_name: &'static str,
    /// Field: the network secret.
    pub field_secret: &'static str,
    /// Field: the virtual IP.
    pub field_ip: &'static str,
    /// Field: peers and shared nodes.
    pub field_peers: &'static str,
    /// Field: listeners.
    pub field_listeners: &'static str,
    /// Field: the MTU.
    pub field_mtu: &'static str,
    /// Field: the network interface name.
    pub field_dev_name: &'static str,
    /// Field: shared subnets.
    pub field_subnet_proxy: &'static str,
    /// Field: exit nodes.
    pub field_exit_nodes: &'static str,
    /// Field: the SOCKS5 proxy.
    pub field_socks5: &'static str,
    /// Field: port forwards.
    pub field_port_forwards: &'static str,
    /// Field: the receive speed limit.
    pub field_recv_limit: &'static str,
    /// Field: mapped listeners.
    pub field_mapped_listeners: &'static str,
    /// Field: manual routes.
    pub field_routes: &'static str,
    /// Field: the VPN portal's client network.
    pub field_vpn_portal_cidr: &'static str,
    /// Field: the VPN portal's port.
    pub field_vpn_portal_port: &'static str,
    /// Field: access control.
    pub field_acl: &'static str,
    /// Field: the relay whitelist.
    pub field_relay_whitelist: &'static str,
    /// Field: the control plane's address.
    pub field_config_server: &'static str,
    /// Field: pasted config or link.
    pub field_import: &'static str,
    /// Field: anything else.
    pub field_other: &'static str,

    /// A problem in one field: `{field}` and `{reason}`.
    pub problem_plain: &'static str,
    /// A problem in one item of a list field: `{field}`, `{entry}` (1-based) and `{reason}`.
    pub problem_entry: &'static str,
    /// Reason: required.
    pub reason_required: &'static str,
    /// Reason: too long (`{max}`).
    pub reason_too_long: &'static str,
    /// Reason: control characters.
    pub reason_control_chars: &'static str,
    /// Reason: too many entries (`{max}`).
    pub reason_too_many: &'static str,
    /// Reason: duplicate entry.
    pub reason_duplicate: &'static str,
    /// Reason: number out of range (`{min}`, `{max}`).
    pub reason_out_of_range: &'static str,
    /// Reason: interface name characters.
    pub reason_name_chars: &'static str,
    /// Reason: pattern with spaces.
    pub reason_no_spaces: &'static str,
    /// Reason: not an address.
    pub reason_not_an_address: &'static str,
    /// Reason: unknown protocol.
    pub reason_unknown_scheme: &'static str,
    /// Reason: protocol only finds peers.
    pub reason_connector_only: &'static str,
    /// Reason: no host.
    pub reason_missing_host: &'static str,
    /// Reason: listener host must be an IP.
    pub reason_host_not_ip: &'static str,
    /// Reason: bad port.
    pub reason_bad_port: &'static str,
    /// Reason: ws / wss listener needs another port.
    pub reason_well_known_port: &'static str,
    /// Reason: extra URL parts.
    pub reason_extra_parts: &'static str,
    /// Reason: not address/prefix.
    pub reason_not_a_cidr: &'static str,
    /// Reason: bad prefix length (`{min}`, `{max}`).
    pub reason_bad_prefix: &'static str,
    /// Reason: host bits set.
    pub reason_host_bits: &'static str,
    /// Reason: reserved address.
    pub reason_reserved: &'static str,
    /// Reason: port forward shape.
    pub reason_forward_shape: &'static str,
    /// Reason: control-plane address shape.
    pub reason_config_server_shape: &'static str,
    /// Reason: a mapped subnet of another size.
    pub reason_mapped_size: &'static str,
    /// Reason: a number that must be above zero.
    pub reason_must_be_positive: &'static str,
    /// Reason: the access-control section has a problem (its editor shows where).
    pub reason_acl: &'static str,
    /// Reason: anything else.
    pub reason_other: &'static str,

    /// Save failed: the keychain.
    pub save_keychain: &'static str,
    /// Save failed: storage full.
    pub save_storage_full: &'static str,
    /// Save failed: damaged list.
    pub save_damaged: &'static str,
    /// Save failed: newer app wrote the list.
    pub save_newer: &'static str,
    /// Save failed: no secure random source.
    pub save_no_random: &'static str,
    /// Save failed: the background work was lost.
    pub save_background: &'static str,
    /// Delete failed: the service or files could not be removed.
    pub remove_failed: &'static str,

    /// Import refused: nothing entered.
    pub import_empty: &'static str,
    /// Import refused: too large.
    pub import_too_large: &'static str,
    /// Import refused: not TOML.
    pub import_syntax: &'static str,
    /// Import refused: the line of a syntax error (`{line}`).
    pub import_line: &'static str,
    /// Import refused: a key has the wrong kind of value (the key is shown before it).
    pub import_wrong_type: &'static str,
    /// Import refused: a key is missing.
    pub import_missing: &'static str,
    /// Import refused: a key's value is not supported.
    pub import_unsupported: &'static str,
    /// Import refused: neither a config nor a link.
    pub import_not_a_link: &'static str,
    /// Import refused: link from a newer app.
    pub import_link_version: &'static str,
    /// Import refused: link without a config.
    pub import_link_no_config: &'static str,
    /// Import refused: damaged link.
    pub import_link_damaged: &'static str,
    /// Import refused: not PNG / JPEG.
    pub import_image_type: &'static str,
    /// Import refused: image too large.
    pub import_image_too_large: &'static str,
    /// Import refused: image not decodable.
    pub import_image_decode: &'static str,
    /// Import refused: no QR code.
    pub import_no_qr: &'static str,
    /// Import refused: unreadable QR code.
    pub import_qr_unreadable: &'static str,
    /// Import refused: the image could not be opened.
    pub import_pick_failed: &'static str,

    /// Exposure: listener on all interfaces.
    pub exposure_listener_all: &'static str,
    /// Exposure: peers use this node as an exit node.
    pub exposure_offers_exit: &'static str,
    /// Exposure: this node uses an exit node.
    pub exposure_uses_exit: &'static str,
    /// Exposure: a subnet is shared.
    pub exposure_shares_subnet: &'static str,
    /// Exposure: a port forward other devices can reach.
    pub exposure_forward_public: &'static str,
    /// Exposure: a port forward on this device.
    pub exposure_forward_local: &'static str,
    /// Exposure: a SOCKS5 proxy other devices can use.
    pub exposure_socks5_public: &'static str,
    /// Exposure: a SOCKS5 proxy on this device.
    pub exposure_socks5_local: &'static str,
    /// Exposure: encryption off.
    pub exposure_encryption_off: &'static str,
    /// Exposure: XOR obfuscation.
    pub exposure_obfuscation: &'static str,
    /// Exposure: private mode off.
    pub exposure_private_mode_off: &'static str,
    /// Exposure: relays all peers' control traffic.
    pub exposure_relays_all_rpc: &'static str,
    /// Exposure: relay whitelist too wide.
    pub exposure_wide_whitelist: &'static str,
    /// Exposure: the VPN portal.
    pub exposure_vpn_portal: &'static str,
    /// Exposure: the OS forwards subnet traffic.
    pub exposure_forward_by_system: &'static str,

    /// A network failed: no privileges.
    pub failure_no_privileges: &'static str,
    /// A network failed: no EasyTier installed.
    pub failure_core_missing: &'static str,
    /// A network failed: EasyTier too old (the version is shown beside).
    pub failure_core_unsupported: &'static str,
    /// A network failed: the core rejected the settings (its message is shown beside).
    pub failure_config_rejected: &'static str,
    /// A network failed: the core exited (the code and last output are shown beside).
    pub failure_exited: &'static str,
    /// A network failed: the core does not answer (its message is shown beside).
    pub failure_unreachable: &'static str,
    /// A network failed: the administrator prompt was declined.
    pub failure_elevation_denied: &'static str,
    /// A network failed: anything else (the message is shown beside).
    pub failure_other: &'static str,
    /// A network failed: its secret is not in the keychain.
    pub failure_secret_missing: &'static str,
    /// Stopping failed (the message is shown beside).
    pub failure_stop: &'static str,
    /// Label before an exit code.
    pub failure_exit_code: &'static str,
    /// Label before the last lines of output.
    pub failure_last_output: &'static str,
    /// Action: open the settings of the network.
    pub failure_action_edit: &'static str,
    /// Action: open the app's Settings (EasyTier versions).
    pub failure_action_settings: &'static str,
    /// Action: run without TUN.
    pub failure_action_no_tun: &'static str,
    /// Action: try again.
    pub failure_action_retry: &'static str,
    /// Action: restart.
    pub failure_action_restart: &'static str,
    /// Action: show the logs.
    pub failure_action_logs: &'static str,
    /// Action: enter the secret again.
    pub failure_action_secret: &'static str,
}

pub(super) static EN: SharedStrings = SharedStrings {
    field_name: "Name",
    field_hostname: "Hostname",
    field_network_name: "Network name",
    field_secret: "Network secret",
    field_ip: "Virtual IP",
    field_peers: "Shared nodes and peers",
    field_listeners: "Listeners",
    field_mtu: "MTU",
    field_dev_name: "Interface name",
    field_subnet_proxy: "Shared subnets",
    field_exit_nodes: "Exit nodes",
    field_socks5: "SOCKS5 proxy",
    field_port_forwards: "Port forwards",
    field_recv_limit: "Receive speed limit",
    field_mapped_listeners: "Mapped listeners",
    field_routes: "Manual routes",
    field_vpn_portal_cidr: "VPN portal client network",
    field_vpn_portal_port: "VPN portal port",
    field_acl: "Access control",
    field_relay_whitelist: "Relay whitelist",
    field_config_server: "Control plane address",
    field_import: "Config or link",
    field_other: "This setting",

    problem_plain: "{field}: {reason}",
    problem_entry: "{field}, item {entry}: {reason}",
    reason_required: "This is required.",
    reason_too_long: "Use at most {max} characters.",
    reason_control_chars: "Remove line breaks and other control characters.",
    reason_too_many: "Use at most {max} entries.",
    reason_duplicate: "This is listed more than once.",
    reason_out_of_range: "Use a number from {min} to {max}.",
    reason_name_chars: "Use letters, digits, “-”, “_” and “.” only.",
    reason_no_spaces: "A pattern can't contain spaces. Put each on its own line.",
    reason_not_an_address: "This isn't a valid address.",
    reason_unknown_scheme: "This protocol isn't one EasyTier knows: use tcp, udp, ws, wss, quic, wg or faketcp.",
    reason_connector_only: "This protocol only finds peers. It can't be used to listen.",
    reason_missing_host: "The host is missing.",
    reason_host_not_ip: "A listener's host must be an IP address.",
    reason_bad_port: "The port must be a number from 1 to 65535.",
    reason_well_known_port: "Use an explicit port other than 80 for ws or 443 for wss.",
    reason_extra_parts: "Remove the user name, path, query or fragment.",
    reason_not_a_cidr: "Write it as an address, a slash and a prefix length.",
    reason_bad_prefix: "The prefix length must be from {min} to {max}.",
    reason_host_bits: "Write the network address: the bits after the prefix must be zero.",
    reason_reserved: "This address can't be used here.",
    reason_forward_shape: "Write each forward as protocol, local address and destination address.",
    reason_config_server_shape: "Use the address your control plane gave you: protocol, host, port and account.",
    reason_mapped_size: "The mapped network must be the same size as the subnet it stands for.",
    reason_must_be_positive: "Enter a whole number above 0, or leave it empty.",
    reason_acl: "Something in the access-control section isn't valid. Check the access-control section.",
    reason_other: "This value isn't valid.",

    save_keychain: "The system keychain wouldn't keep the secret. Nothing was saved.",
    save_storage_full: "The app's storage is full. Nothing was saved.",
    save_damaged: "The saved networks can't be read, so nothing can be saved over them.",
    save_newer: "A newer version of this app wrote the saved networks. Update the app to change them.",
    save_no_random: "This device can't make a secure random id. Nothing was saved.",
    save_background: "Saving didn't finish. Try again.",
    remove_failed: "The network couldn't be removed completely, so it was kept. Try again.",

    import_empty: "Paste a config or a share link, or pick a picture of a QR code.",
    import_too_large: "This is larger than any EasyTier config.",
    import_syntax: "This isn't valid TOML.",
    import_line: "The first problem is on line {line}.",
    import_wrong_type: "has the wrong kind of value.",
    import_missing: "is missing.",
    import_unsupported: "has a value this app doesn't support.",
    import_not_a_link: "This isn't an EasyTier config or an EasyTierUI share link.",
    import_link_version: "This share link comes from a newer version of the app.",
    import_link_no_config: "This share link carries no config.",
    import_link_damaged: "This share link is damaged.",
    import_image_type: "Pick a PNG or JPEG picture.",
    import_image_too_large: "This picture is too large.",
    import_image_decode: "This picture couldn't be read.",
    import_no_qr: "No QR code was found in this picture.",
    import_qr_unreadable: "The QR code couldn't be read. Try a larger or sharper picture.",
    import_pick_failed: "The picture couldn't be opened.",

    exposure_listener_all: "Listens on every network interface, so anything this device is connected to can reach it.",
    exposure_offers_exit: "Peers can send their internet traffic through this device.",
    exposure_uses_exit: "This device sends its traffic through an exit node.",
    exposure_shares_subnet: "Peers can reach this subnet through this device.",
    exposure_forward_public: "Forwards a port that other devices on your network can reach.",
    exposure_forward_local: "Forwards a port on this device into the network.",
    exposure_socks5_public: "A SOCKS5 proxy that other devices on your network can use.",
    exposure_socks5_local: "A SOCKS5 proxy on this device.",
    exposure_encryption_off: "Traffic between peers is not encrypted. Every peer must use the same setting.",
    exposure_obfuscation: "XOR only obfuscates traffic; it is not encryption. Every peer must use the same setting.",
    exposure_private_mode_off: "Private mode is off: peers from other networks are accepted without the secret.",
    exposure_relays_all_rpc: "Relays control traffic for every peer, not only for this network.",
    exposure_wide_whitelist: "Relays for any network without needing its secret.",
    exposure_vpn_portal: "WireGuard clients can join the network through this node, from every network this device is on.",
    exposure_forward_by_system: "Subnet traffic is forwarded by the operating system, so the system's firewall and routing rules apply to it.",

    failure_no_privileges: "Creating the network interface needs administrator rights.",
    failure_core_missing: "EasyTier isn't installed on this device.",
    failure_core_unsupported: "The installed EasyTier is older than this app supports.",
    failure_config_rejected: "EasyTier rejected these settings.",
    failure_exited: "The core stopped on its own.",
    failure_unreachable: "The core is running but isn't answering.",
    failure_elevation_denied: "Administrator approval wasn't given, so nothing was started.",
    failure_other: "The network couldn't run.",
    failure_secret_missing: "This network's secret isn't saved on this device.",
    failure_stop: "It couldn't be stopped.",
    failure_exit_code: "Exit code",
    failure_last_output: "Last output",
    failure_action_edit: "Edit settings",
    failure_action_settings: "Open Settings",
    failure_action_no_tun: "Run without TUN",
    failure_action_retry: "Try again",
    failure_action_restart: "Restart",
    failure_action_logs: "Show logs",
    failure_action_secret: "Enter the secret",
};

pub(super) static ZH_CN: SharedStrings = SharedStrings {
    field_name: "名称",
    field_hostname: "主机名",
    field_network_name: "网络名称",
    field_secret: "网络密钥",
    field_ip: "虚拟 IP",
    field_peers: "共享节点与对等节点",
    field_listeners: "监听地址",
    field_mtu: "MTU",
    field_dev_name: "网卡名称",
    field_subnet_proxy: "共享子网",
    field_exit_nodes: "出口节点",
    field_socks5: "SOCKS5 代理",
    field_port_forwards: "端口转发",
    field_recv_limit: "接收速率限制",
    field_mapped_listeners: "映射监听地址",
    field_routes: "手动路由",
    field_vpn_portal_cidr: "VPN 门户客户端网段",
    field_vpn_portal_port: "VPN 门户端口",
    field_acl: "访问控制",
    field_relay_whitelist: "中继白名单",
    field_config_server: "控制平面地址",
    field_import: "配置或链接",
    field_other: "此设置",

    problem_plain: "{field}：{reason}",
    problem_entry: "{field}，第 {entry} 项：{reason}",
    reason_required: "此项必填。",
    reason_too_long: "最多 {max} 个字符。",
    reason_control_chars: "请去掉换行等控制字符。",
    reason_too_many: "最多 {max} 项。",
    reason_duplicate: "重复了。",
    reason_out_of_range: "请输入 {min} 到 {max} 之间的数字。",
    reason_name_chars: "只能使用字母、数字以及“-”“_”“.”。",
    reason_no_spaces: "规则中不能有空格，请每行写一条。",
    reason_not_an_address: "不是有效的地址。",
    reason_unknown_scheme: "EasyTier 不支持这个协议，请使用 tcp、udp、ws、wss、quic、wg 或 faketcp。",
    reason_connector_only: "这个协议只用于发现对等节点，不能用来监听。",
    reason_missing_host: "缺少主机。",
    reason_host_not_ip: "监听地址的主机必须是 IP 地址。",
    reason_bad_port: "端口必须是 1 到 65535 之间的数字。",
    reason_well_known_port: "ws 请勿使用 80 端口、wss 请勿使用 443 端口，请写明其他端口。",
    reason_extra_parts: "请去掉用户名、路径、查询参数或片段。",
    reason_not_a_cidr: "请写成地址、斜杠和前缀长度的形式。",
    reason_bad_prefix: "前缀长度须在 {min} 到 {max} 之间。",
    reason_host_bits: "请写网络地址：前缀之后的位必须为 0。",
    reason_reserved: "这个地址不能在此使用。",
    reason_forward_shape: "每条转发依次写协议、本机地址和目标地址。",
    reason_config_server_shape: "请使用控制平面提供的地址：协议、主机、端口和账号。",
    reason_mapped_size: "映射后的网段必须与它所代表的子网大小相同。",
    reason_must_be_positive: "请输入大于 0 的整数，或留空。",
    reason_acl: "访问控制部分有无效内容，请检查访问控制部分。",
    reason_other: "这个值无效。",

    save_keychain: "系统钥匙串没有保存密钥，没有保存任何内容。",
    save_storage_full: "应用存储空间已满，没有保存任何内容。",
    save_damaged: "已保存的网络无法读取，因此不能覆盖保存。",
    save_newer: "已保存的网络由更新版本的应用写入，请更新应用后再修改。",
    save_no_random: "此设备无法生成安全的随机编号，没有保存任何内容。",
    save_background: "保存没有完成，请重试。",
    remove_failed: "网络没能完全移除，因此已保留，请重试。",

    import_empty: "请粘贴配置或分享链接，或选择二维码图片。",
    import_too_large: "内容比任何 EasyTier 配置都大。",
    import_syntax: "不是有效的 TOML。",
    import_line: "第一处问题在第 {line} 行。",
    import_wrong_type: "的值类型不对。",
    import_missing: "缺失。",
    import_unsupported: "的值本应用不支持。",
    import_not_a_link: "这不是 EasyTier 配置，也不是 EasyTierUI 分享链接。",
    import_link_version: "这个分享链接来自更新版本的应用。",
    import_link_no_config: "这个分享链接不含配置。",
    import_link_damaged: "这个分享链接已损坏。",
    import_image_type: "请选择 PNG 或 JPEG 图片。",
    import_image_too_large: "图片太大。",
    import_image_decode: "无法读取这张图片。",
    import_no_qr: "图片中没有找到二维码。",
    import_qr_unreadable: "无法识别二维码，请换一张更大或更清晰的图片。",
    import_pick_failed: "无法打开这张图片。",

    exposure_listener_all: "监听所有网络接口，此设备连接的任何网络都能访问它。",
    exposure_offers_exit: "对等节点可以通过此设备访问互联网。",
    exposure_uses_exit: "此设备的流量会经由出口节点发出。",
    exposure_shares_subnet: "对等节点可以通过此设备访问这个子网。",
    exposure_forward_public: "转发的端口，你网络中的其他设备也能访问。",
    exposure_forward_local: "把此设备上的一个端口转发到网络中。",
    exposure_socks5_public: "你网络中的其他设备也能使用这个 SOCKS5 代理。",
    exposure_socks5_local: "此设备上的 SOCKS5 代理。",
    exposure_encryption_off: "对等节点之间的流量没有加密。所有节点必须使用相同的设置。",
    exposure_obfuscation: "XOR 只是混淆流量，并非加密。所有节点必须使用相同的设置。",
    exposure_private_mode_off: "私有模式已关闭：其他网络的节点无需密钥即可接入。",
    exposure_relays_all_rpc: "为所有对等节点中继控制流量，而不只是本网络。",
    exposure_wide_whitelist: "为任何网络提供中继，无需对方的密钥。",
    exposure_vpn_portal: "WireGuard 客户端可以经本节点接入网络，这台设备所在的每个网络都能连到它。",
    exposure_forward_by_system: "子网流量由操作系统转发，因此系统的防火墙和路由规则会作用于它。",

    failure_no_privileges: "创建虚拟网卡需要管理员权限。",
    failure_core_missing: "此设备上没有安装 EasyTier。",
    failure_core_unsupported: "已安装的 EasyTier 版本低于本应用支持的最低版本。",
    failure_config_rejected: "EasyTier 拒绝了这些设置。",
    failure_exited: "内核自行退出了。",
    failure_unreachable: "内核在运行，但没有响应。",
    failure_elevation_denied: "没有获得管理员授权，因此没有启动。",
    failure_other: "网络无法运行。",
    failure_secret_missing: "此设备上没有保存这个网络的密钥。",
    failure_stop: "无法停止。",
    failure_exit_code: "退出码",
    failure_last_output: "最后的输出",
    failure_action_edit: "编辑设置",
    failure_action_settings: "打开设置",
    failure_action_no_tun: "不使用 TUN 运行",
    failure_action_retry: "重试",
    failure_action_restart: "重新启动",
    failure_action_logs: "查看日志",
    failure_action_secret: "输入密钥",
};
