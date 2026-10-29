//! Node role: the network dialog — status, settings, logs and export tabs.
//!
//! One field per string; `EN` and `ZH_CN` must both set every field.

/// Strings of this area in one language.
#[derive(Debug)]
pub struct DetailStrings {
    /// Name of the tab strip, for screen readers.
    pub tabs_label: &'static str,
    /// Tab: the running node.
    pub tab_overview: &'static str,
    /// Tab: the network's settings.
    pub tab_settings: &'static str,
    /// Tab: the core's output.
    pub tab_logs: &'static str,
    /// Tab: config, link and QR code.
    pub tab_export: &'static str,

    /// Action: restart a running network.
    pub action_restart: &'static str,
    /// The last start failed while the network runs on with its old settings.
    pub start_failed_running: &'static str,
    /// Status: a stopped network has no details.
    pub quiet_stopped: &'static str,
    /// Status: a starting network has no details yet.
    pub quiet_starting: &'static str,
    /// Status: a stopping network has no details.
    pub quiet_stopping: &'static str,
    /// Status: a network being deleted has no details.
    pub quiet_removing: &'static str,
    /// Status: a failed network has no details.
    pub quiet_failed: &'static str,
    /// Label: the hostname peers see.
    pub label_hostname: &'static str,
    /// Label: the addresses this node listens on.
    pub label_listeners: &'static str,
    /// Label: the public addresses STUN found.
    pub label_public_ips: &'static str,
    /// Label: the NAT type.
    pub label_nat: &'static str,
    /// Label: the subnets this node proxies.
    pub label_subnets: &'static str,
    /// Label: the version of the running core.
    pub label_core_version: &'static str,
    /// A list with nothing in it.
    pub word_none: &'static str,
    /// NAT type: not determined.
    pub nat_unknown: &'static str,
    /// NAT type: open internet.
    pub nat_open_internet: &'static str,
    /// NAT type: no port translation.
    pub nat_no_pat: &'static str,
    /// NAT type: full cone.
    pub nat_full_cone: &'static str,
    /// NAT type: restricted cone.
    pub nat_restricted: &'static str,
    /// NAT type: port restricted cone.
    pub nat_port_restricted: &'static str,
    /// NAT type: symmetric.
    pub nat_symmetric: &'static str,
    /// NAT type: symmetric UDP firewall.
    pub nat_sym_udp_firewall: &'static str,
    /// NAT type: symmetric with predictable increasing ports.
    pub nat_symmetric_easy_inc: &'static str,
    /// NAT type: symmetric with predictable decreasing ports.
    pub nat_symmetric_easy_dec: &'static str,

    /// Settings section: name and secret.
    pub section_general: &'static str,
    /// Settings section: how the node connects.
    pub section_connection: &'static str,
    /// Settings section: what this node shares and how it routes.
    pub section_sharing: &'static str,
    /// Settings section: relaying for other networks.
    pub section_relaying: &'static str,
    /// Settings section: deleting the network.
    pub section_remove: &'static str,
    /// The secret is not saved on this device.
    pub secret_missing: &'static str,
    /// Name: what it is.
    pub name_help: &'static str,
    /// Hostname: what it is.
    pub hostname_help: &'static str,
    /// Address mode: label.
    pub address_label: &'static str,
    /// Address mode: assigned automatically.
    pub address_dhcp: &'static str,
    /// Address mode: typed in.
    pub address_static: &'static str,
    /// Address mode: no address.
    pub address_none: &'static str,
    /// Address: what the modes mean.
    pub address_help: &'static str,
    /// Peers: what to enter, one per line.
    pub peers_help: &'static str,
    /// Switch: accept incoming connections.
    pub listeners_switch: &'static str,
    /// Switch: what accepting incoming connections does.
    pub listeners_switch_help: &'static str,
    /// Listeners: what to enter, one per line.
    pub listeners_help: &'static str,
    /// Switch: the virtual network interface.
    pub tun_label: &'static str,
    /// Switch: what the interface does.
    pub tun_help: &'static str,
    /// The interface is not available on this device.
    pub tun_unavailable: &'static str,
    /// Select: encryption.
    pub encryption_label: &'static str,
    /// Encryption: AES-GCM.
    pub enc_aes_gcm: &'static str,
    /// Encryption: AES-256-GCM.
    pub enc_aes_256_gcm: &'static str,
    /// Encryption: ChaCha20.
    pub enc_chacha20: &'static str,
    /// Encryption: XOR.
    pub enc_xor: &'static str,
    /// Encryption: off.
    pub enc_off: &'static str,
    /// Encryption: every peer must match.
    pub encryption_help: &'static str,
    /// MTU: what it is and what empty means.
    pub mtu_help: &'static str,
    /// Interface name: what it is and what empty means.
    pub dev_name_help: &'static str,
    /// Switch: offer this node as an exit node.
    pub offers_exit: &'static str,
    /// Switch: what an exit node is.
    pub offers_exit_help: &'static str,
    /// Switch: send traffic through exit nodes.
    pub uses_exit: &'static str,
    /// Switch: what using an exit node does.
    pub uses_exit_help: &'static str,
    /// Exit nodes: what to enter, one per line.
    pub exit_nodes_help: &'static str,
    /// Switch: share subnets.
    pub subnet_switch: &'static str,
    /// Switch: what sharing a subnet does.
    pub subnet_switch_help: &'static str,
    /// Subnets: what to enter, one per line.
    pub subnets_help: &'static str,
    /// Switch: serve a SOCKS5 proxy.
    pub socks5_switch: &'static str,
    /// Switch: what the proxy does.
    pub socks5_switch_help: &'static str,
    /// SOCKS5: what to enter.
    pub socks5_help: &'static str,
    /// Switch: forward ports.
    pub forwards_switch: &'static str,
    /// Switch: what a forward does.
    pub forwards_switch_help: &'static str,
    /// Switch: relay every peer's control traffic.
    pub relay_all_rpc: &'static str,
    /// Switch: what relaying control traffic means.
    pub relay_all_rpc_help: &'static str,
    /// Switch: private mode.
    pub private_mode: &'static str,
    /// Switch: what private mode does.
    pub private_mode_help: &'static str,
    /// Segmented: which networks this node relays for.
    pub whitelist_label: &'static str,
    /// Relay for any network.
    pub whitelist_any: &'static str,
    /// Relay only for the listed networks.
    pub whitelist_only: &'static str,
    /// Whitelist: what to enter, one per line.
    pub whitelist_help: &'static str,
    /// Next to Save: a running network restarts.
    pub saving_restarts: &'static str,
    /// Deleting: what it does.
    pub remove_help: &'static str,
    /// Leaving management: what it does.
    pub leave_help: &'static str,

    /// Logs: what the lines are.
    pub logs_note: &'static str,
    /// Logs: nothing to show.
    pub logs_empty: &'static str,
    /// The follow switch of the log tab.
    pub logs_follow: &'static str,
    /// The drawer's close button.
    pub drawer_close: &'static str,
    /// The Overview tab: the node's own facts.
    pub section_node: &'static str,
    /// The Overview tab: the peers fact (`{online}`, `{direct}`, `{relayed}`).
    pub peers_mix: &'static str,

    /// Switch: put the network secret in the export.
    pub export_include_secret: &'static str,
    /// Switch: what the secret in the export means.
    pub export_include_secret_help: &'static str,
    /// Warning while the secret is part of the export.
    pub export_secret_exposure: &'static str,
    /// The export has no secret.
    pub export_secret_left_out: &'static str,
    /// Label: the config file.
    pub export_toml: &'static str,
    /// Label: the share link.
    pub export_link: &'static str,
    /// The link carries more than the network.
    pub export_link_note: &'static str,
    /// Label: the QR code.
    pub export_qr: &'static str,
    /// The QR code, for screen readers.
    pub export_qr_label: &'static str,
    /// The link is too long for a QR code.
    pub export_qr_too_long: &'static str,
    /// The keychain refused the secret.
    pub export_keychain: &'static str,
    /// The profile cannot be exported.
    pub export_invalid: &'static str,
    /// The secret was asked for and is not saved.
    pub export_secret_missing: &'static str,
}

pub(super) static EN: DetailStrings = DetailStrings {
    tabs_label: "Network view",
    tab_overview: "Overview",
    tab_settings: "Settings",
    tab_logs: "Logs",
    tab_export: "Export",

    action_restart: "Restart",
    start_failed_running: "The last start failed, so this network keeps running with its previous settings. Open the logs to see why.",
    quiet_stopped: "This network is stopped. Its virtual IP, listeners and peers appear here once it runs.",
    quiet_starting: "This network is starting. Its virtual IP, listeners and peers appear here once it runs.",
    quiet_stopping: "This network is stopping. Nothing is connected any more.",
    quiet_removing: "This network is being deleted.",
    quiet_failed: "This network failed and isn't running. Its virtual IP, listeners and peers appear here once it runs.",
    label_hostname: "Hostname",
    label_listeners: "Listening on",
    label_public_ips: "Public addresses",
    label_nat: "NAT type",
    label_subnets: "Shared subnets",
    label_core_version: "Core version",
    word_none: "None",
    nat_unknown: "Unknown",
    nat_open_internet: "Open internet",
    nat_no_pat: "No port translation",
    nat_full_cone: "Full cone",
    nat_restricted: "Restricted cone",
    nat_port_restricted: "Port-restricted cone",
    nat_symmetric: "Symmetric",
    nat_sym_udp_firewall: "Symmetric UDP firewall",
    nat_symmetric_easy_inc: "Symmetric, ports increase",
    nat_symmetric_easy_dec: "Symmetric, ports decrease",

    section_general: "General",
    section_connection: "Connection",
    section_sharing: "Sharing and routing",
    section_relaying: "Relaying",
    section_remove: "Remove",
    secret_missing: "The secret of this network isn't saved on this device. Enter it again to save it.",
    name_help: "Only you see this name, on this device.",
    hostname_help: "The name other peers see. Leave it empty to use this device's own.",
    address_label: "Virtual IP",
    address_dhcp: "Automatic",
    address_static: "Fixed",
    address_none: "None",
    address_help: "Automatic asks the network for a free address. A fixed address must not be used by another peer.",
    peers_help: "One shared node or peer per line. Without a protocol, tcp is used.",
    listeners_switch: "Accept incoming connections",
    listeners_switch_help: "Other peers can connect to this node directly, which gives you more direct links.",
    listeners_help: "One address per line, with the protocol and an IP address.",
    tun_label: "Virtual network interface (TUN)",
    tun_help: "Lets other apps on this device send traffic to the virtual IP. Starting needs administrator approval.",
    tun_unavailable: "Not available on this device. This network runs without an interface.",
    encryption_label: "Encryption",
    enc_aes_gcm: "AES-GCM",
    enc_aes_256_gcm: "AES-256-GCM",
    enc_chacha20: "ChaCha20",
    enc_xor: "XOR (obfuscation, not encryption)",
    enc_off: "Off",
    encryption_help: "Every peer in the network must use the same setting, or they can't talk to each other.",
    mtu_help: "Leave it empty to use EasyTier's default.",
    dev_name_help: "The name of the network interface. Leave it empty to let the system choose.",
    offers_exit: "Offer this node as an exit node",
    offers_exit_help: "Peers that choose this node send their internet traffic out through it.",
    uses_exit: "Send my traffic through exit nodes",
    uses_exit_help: "Internet traffic from this device leaves through the exit nodes you list.",
    exit_nodes_help: "The virtual IP of each exit node, one per line.",
    subnet_switch: "Share subnets",
    subnet_switch_help: "Makes networks behind this device reachable for all peers.",
    subnets_help: "One subnet per line, such as the LAN behind this device. Write 192.168.50.0/24->10.50.0.0/24 to show peers the subnet as another range of the same size.",
    socks5_switch: "SOCKS5 proxy",
    socks5_switch_help: "Apps that speak SOCKS5 reach the network through this proxy.",
    socks5_help: "A port keeps the proxy on this device. Enter address:port to listen elsewhere.",
    forwards_switch: "Forward ports",
    forwards_switch_help: "A port on this device leads to an address inside the network.",
    relay_all_rpc: "Relay control traffic for every peer",
    relay_all_rpc_help: "Also passes on the control messages of peers from other networks. Only a public shared node needs this.",
    private_mode: "Private mode",
    private_mode_help: "Refuses peers from other networks that don't know this network's secret.",
    whitelist_label: "Relay for",
    whitelist_any: "Any network",
    whitelist_only: "Only the listed networks",
    whitelist_help: "One network name per line. A * stands for any characters.",
    saving_restarts: "Saving restarts this network.",
    remove_help: "Stops the network and removes it, its settings and its saved secret from this device.",
    leave_help: "Stops the network and removes it from this device.",

    logs_note: "Recent output of the core for this network. Secrets are hidden.",
    logs_empty: "No output yet. The core writes here once the network starts.",
    logs_follow: "Follow",
    drawer_close: "Close network",
    section_node: "This node",
    peers_mix: "{online} ({direct} direct, {relayed} relayed)",

    export_include_secret: "Include the network secret",
    export_include_secret_help: "Off by default. Without it, whoever imports the config enters the secret themselves.",
    export_secret_exposure: "Anyone with this config or link can join the network.",
    export_secret_left_out: "The network secret isn't part of this export.",
    export_toml: "Config file (TOML)",
    export_link: "Share link",
    export_link_note: "The link is a copy of this profile, so it also carries this node's address and hostname.",
    export_qr: "QR code",
    export_qr_label: "QR code of the share link",
    export_qr_too_long: "The link is too long for a QR code. Use the config file or the link instead.",
    export_keychain: "The system keychain wouldn't give out the secret, so nothing was exported.",
    export_invalid: "This network's settings can't be exported as they are. Fix them in Settings first.",
    export_secret_missing: "The secret of this network isn't saved on this device, so the export has none.",
};

pub(super) static ZH_CN: DetailStrings = DetailStrings {
    tabs_label: "网络视图",
    tab_overview: "概览",
    tab_settings: "设置",
    tab_logs: "日志",
    tab_export: "导出",

    action_restart: "重启",
    start_failed_running: "上一次启动失败，网络仍以之前的设置运行。打开日志查看原因。",
    quiet_stopped: "网络已停止。运行后，这里会显示虚拟 IP、监听地址和对等节点。",
    quiet_starting: "网络正在启动。运行后，这里会显示虚拟 IP、监听地址和对等节点。",
    quiet_stopping: "网络正在停止，已不再连接。",
    quiet_removing: "正在删除这个网络。",
    quiet_failed: "网络启动失败，当前没有运行。运行后，这里会显示虚拟 IP、监听地址和对等节点。",
    label_hostname: "主机名",
    label_listeners: "监听地址",
    label_public_ips: "公网地址",
    label_nat: "NAT 类型",
    label_subnets: "共享的子网",
    label_core_version: "内核版本",
    word_none: "无",
    nat_unknown: "未知",
    nat_open_internet: "开放互联网",
    nat_no_pat: "无端口转换",
    nat_full_cone: "完全锥形",
    nat_restricted: "受限锥形",
    nat_port_restricted: "端口受限锥形",
    nat_symmetric: "对称型",
    nat_sym_udp_firewall: "对称 UDP 防火墙",
    nat_symmetric_easy_inc: "对称型，端口递增",
    nat_symmetric_easy_dec: "对称型，端口递减",

    section_general: "常规",
    section_connection: "连接",
    section_sharing: "共享与路由",
    section_relaying: "中继",
    section_remove: "移除",
    secret_missing: "此设备上没有保存这个网络的密钥。请重新输入后保存。",
    name_help: "这个名字只在这台设备上显示。",
    hostname_help: "其他节点看到的名称。留空则使用这台设备自己的名称。",
    address_label: "虚拟 IP",
    address_dhcp: "自动",
    address_static: "固定",
    address_none: "无",
    address_help: "自动：由网络分配一个空闲地址。固定：地址不能与其他节点重复。",
    peers_help: "每行一个共享节点或对等节点。不写协议时使用 tcp。",
    listeners_switch: "接受入站连接",
    listeners_switch_help: "其他节点可以直接连到这个节点，更容易建立直连。",
    listeners_help: "每行一个地址，包含协议和 IP 地址。",
    tun_label: "虚拟网卡（TUN）",
    tun_help: "让这台设备上的其他应用向虚拟 IP 发送流量。启动时需要管理员授权。",
    tun_unavailable: "此设备不支持。这个网络不使用虚拟网卡运行。",
    encryption_label: "加密",
    enc_aes_gcm: "AES-GCM",
    enc_aes_256_gcm: "AES-256-GCM",
    enc_chacha20: "ChaCha20",
    enc_xor: "XOR（仅混淆，不是加密）",
    enc_off: "关闭",
    encryption_help: "网络中的所有节点必须使用相同的设置，否则无法互相通信。",
    mtu_help: "留空则使用 EasyTier 的默认值。",
    dev_name_help: "虚拟网卡的名称。留空则由系统决定。",
    offers_exit: "把这个节点作为出口节点",
    offers_exit_help: "选择此节点的对等节点会经它访问互联网。",
    uses_exit: "让我的流量经出口节点发出",
    uses_exit_help: "这台设备访问互联网的流量会经你列出的出口节点发出。",
    exit_nodes_help: "每行填写一个出口节点的虚拟 IP。",
    subnet_switch: "共享子网",
    subnet_switch_help: "让这台设备背后的网络可被所有对等节点访问。",
    subnets_help: "每行一个子网，例如这台设备所在的局域网。写成 192.168.50.0/24->10.50.0.0/24，可让对等节点看到同样大小的另一个网段。",
    socks5_switch: "SOCKS5 代理",
    socks5_switch_help: "支持 SOCKS5 的应用可以通过这个代理访问网络。",
    socks5_help: "只填端口时，代理仅限这台设备使用。填写“地址:端口”可监听其他地址。",
    forwards_switch: "转发端口",
    forwards_switch_help: "把这台设备上的一个端口转到网络内的某个地址。",
    relay_all_rpc: "为所有对等节点中继控制流量",
    relay_all_rpc_help: "同时转发其他网络中节点的控制消息。只有公网共享节点需要开启。",
    private_mode: "私有模式",
    private_mode_help: "拒绝不知道本网络密钥的其他网络节点。",
    whitelist_label: "中继范围",
    whitelist_any: "任何网络",
    whitelist_only: "仅限列出的网络",
    whitelist_help: "每行一个网络名称，* 代表任意字符。",
    saving_restarts: "保存后会重启这个网络。",
    remove_help: "停止网络，并从这台设备移除它、它的设置和已保存的密钥。",
    leave_help: "停止网络，并从这台设备移除它。",

    logs_note: "这个网络的内核最近的输出，密钥已隐藏。",
    logs_empty: "还没有输出。网络启动后，内核会在这里写入日志。",
    logs_follow: "跟随",
    drawer_close: "关闭网络详情",
    section_node: "本节点",
    peers_mix: "{online}（直连 {direct}，中继 {relayed}）",

    export_include_secret: "包含网络密钥",
    export_include_secret_help: "默认关闭。不包含时，导入配置的人需要自己填写密钥。",
    export_secret_exposure: "任何拿到这份配置或链接的人都能加入这个网络。",
    export_secret_left_out: "导出的内容不含网络密钥。",
    export_toml: "配置文件（TOML）",
    export_link: "分享链接",
    export_link_note: "链接是这份配置的副本，因此也带有这个节点的地址和主机名。",
    export_qr: "二维码",
    export_qr_label: "分享链接的二维码",
    export_qr_too_long: "链接太长，无法生成二维码。请改用配置文件或链接。",
    export_keychain: "系统钥匙串没有交出密钥，因此没有导出任何内容。",
    export_invalid: "这个网络当前的设置无法导出。请先在设置中修正。",
    export_secret_missing: "此设备上没有保存这个网络的密钥，因此导出的内容不含密钥。",
};
