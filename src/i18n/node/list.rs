//! Node role: the Networks page — toolbar, summary, the table of networks, the core callout, notices and delete
//! confirmations.

/// Strings of this area in one language.
#[derive(Debug)]
pub struct ListStrings {
    /// Action: create a network.
    pub action_create: &'static str,
    /// Action: join a network.
    pub action_join: &'static str,
    /// Action: import a config, link or QR code.
    pub action_import: &'static str,
    /// Action: enroll in a control plane.
    pub action_managed: &'static str,
    /// Action: start a network.
    pub action_start: &'static str,
    /// Action: stop a network.
    pub action_stop: &'static str,
    /// Action: open a network's settings.
    pub action_settings: &'static str,
    /// Action: open a network's logs.
    pub action_logs: &'static str,
    /// Action: open a network's export.
    pub action_export: &'static str,
    /// The accessible name of a row's "more" button: `{name}`.
    pub action_more: &'static str,
    /// What the row says to screen readers: `{name}`.
    pub card_label: &'static str,
    /// A managed network's tag.
    pub chip_managed: &'static str,
    /// A network without a virtual interface.
    pub chip_no_tun: &'static str,
    /// A network with a virtual interface.
    pub chip_tun: &'static str,
    /// Caption under the headline number.
    pub peers_online: &'static str,
    /// Label before the virtual IP.
    pub virtual_ip: &'static str,
    /// The core is not installed.
    pub core_missing: &'static str,
    /// The installed core is too old.
    pub core_unsupported: &'static str,
    /// Label before the installed core's version in the callout.
    pub core_detected: &'static str,
    /// The callout's action.
    pub core_action: &'static str,
    /// The stored list cannot be read.
    pub notice_damaged: &'static str,
    /// The stored list is from a newer app.
    pub notice_newer: &'static str,
    /// Some stored networks were skipped (`{count}`).
    pub notice_skipped: &'static str,
    /// The default name of an enrolled network.
    pub managed_default_name: &'static str,
    /// A managed network's settings are read-only here.
    pub managed_read_only: &'static str,
    /// Confirmation title for deleting (`{name}`).
    pub delete_title: &'static str,
    /// Confirmation text for deleting.
    pub delete_body: &'static str,
    /// Confirmation title for leaving management (`{name}`).
    pub leave_title: &'static str,
    /// Confirmation text for leaving management.
    pub leave_body: &'static str,
    /// The destructive action of leaving management.
    pub leave_action: &'static str,

    /// The search field's placeholder and accessible name.
    pub search_placeholder: &'static str,
    /// The search field's clear button.
    pub search_clear: &'static str,
    /// The name of the All / Running / Stopped filter.
    pub filter_label: &'static str,
    /// Filter: every network.
    pub filter_all: &'static str,
    /// Filter: running networks.
    pub filter_running: &'static str,
    /// Filter: networks that do not run.
    pub filter_stopped: &'static str,
    /// Column: the network's name.
    pub col_name: &'static str,
    /// Column: the network's state.
    pub col_status: &'static str,
    /// Column: the peers online.
    pub col_peers: &'static str,
    /// Column: received rate.
    pub col_rx: &'static str,
    /// Column: sent rate.
    pub col_tx: &'static str,
    /// Summary: networks running.
    pub summary_running: &'static str,
    /// Summary: of how many networks (`{total}`).
    pub summary_running_of: &'static str,
    /// Summary: peers online.
    pub summary_peers: &'static str,
    /// Summary: how many peers are reached directly.
    pub summary_direct: &'static str,
    /// Summary: how many peers are reached through relays.
    pub summary_relayed: &'static str,
    /// Summary: received rate of all networks.
    pub summary_rx: &'static str,
    /// Summary: sent rate of all networks.
    pub summary_tx: &'static str,
    /// Summary: the EasyTier core (desktops).
    pub summary_core: &'static str,
    /// Summary: the EasyTier library (phones).
    pub summary_library: &'static str,
    /// No network matches the search or the filter: title.
    pub none_match_title: &'static str,
    /// No network matches the search or the filter: body.
    pub none_match_body: &'static str,
    /// Clears the search and the filter.
    pub none_match_action: &'static str,
}

pub(super) static EN: ListStrings = ListStrings {
    action_create: "Create network",
    action_join: "Join network",
    action_import: "Import…",
    action_managed: "Join a control plane…",
    action_start: "Start",
    action_stop: "Stop",
    action_settings: "Settings",
    action_logs: "Logs",
    action_export: "Export",
    action_more: "More actions for {name}",
    card_label: "Network {name}",
    chip_managed: "Managed",
    chip_no_tun: "No TUN",
    chip_tun: "TUN",
    peers_online: "peers online",
    virtual_ip: "Virtual IP",
    core_missing: "EasyTier isn't installed, so no network can start. Install a release in Settings.",
    core_unsupported: "The installed EasyTier is older than this app supports. Switch to a newer release in Settings.",
    core_detected: "Detected core",
    core_action: "Open Settings",
    notice_damaged: "The saved networks couldn't be read. They are left as they are, and nothing can be saved over them.",
    notice_newer: "A newer version of this app wrote the saved networks. They are left as they are; update the app to change them.",
    notice_skipped: "{count} saved networks couldn't be read and were left as they are.",
    managed_default_name: "Control plane",
    managed_read_only: "A control plane manages this network. Its settings can't be changed here.",
    delete_title: "Delete network “{name}”?",
    delete_body: "It stops, its settings and any system service are removed from this device, and its saved secret is forgotten.",
    leave_title: "Leave management of “{name}”?",
    leave_body: "It stops and is removed from this device. The control plane may still list this machine.",
    leave_action: "Leave management",

    search_placeholder: "Search by name or network",
    search_clear: "Clear search",
    filter_label: "Show networks",
    filter_all: "All",
    filter_running: "Running",
    filter_stopped: "Stopped",
    col_name: "Network",
    col_status: "State",
    col_peers: "Peers",
    col_rx: "Received",
    col_tx: "Sent",
    summary_running: "Networks running",
    summary_running_of: "of {total}",
    summary_peers: "Peers online",
    summary_direct: "direct",
    summary_relayed: "relayed",
    summary_rx: "Received",
    summary_tx: "Sent",
    summary_core: "EasyTier core",
    summary_library: "EasyTier library",
    none_match_title: "No network matches",
    none_match_body: "Change the search or the filter to see more of your networks.",
    none_match_action: "Clear search and filter",
};

pub(super) static ZH_CN: ListStrings = ListStrings {
    action_create: "创建网络",
    action_join: "加入网络",
    action_import: "导入…",
    action_managed: "加入控制平面…",
    action_start: "启动",
    action_stop: "停止",
    action_settings: "设置",
    action_logs: "日志",
    action_export: "导出",
    action_more: "“{name}”的更多操作",
    card_label: "网络 {name}",
    chip_managed: "受管理",
    chip_no_tun: "无 TUN",
    chip_tun: "TUN",
    peers_online: "在线对等节点",
    virtual_ip: "虚拟 IP",
    core_missing: "尚未安装 EasyTier，网络无法启动。请在设置中安装一个版本。",
    core_unsupported: "已安装的 EasyTier 版本低于本应用支持的最低版本。请在设置中切换到更新的版本。",
    core_detected: "检测到的内核",
    core_action: "打开设置",
    notice_damaged: "无法读取已保存的网络。它们保持原样，也不能被覆盖保存。",
    notice_newer: "已保存的网络由更新版本的应用写入。它们保持原样，请更新应用后再修改。",
    notice_skipped: "有 {count} 个已保存的网络无法读取，已保持原样。",
    managed_default_name: "控制平面",
    managed_read_only: "此网络由控制平面管理，无法在这里修改设置。",
    delete_title: "删除网络“{name}”？",
    delete_body: "网络会停止，其设置和系统服务会从此设备移除，已保存的密钥也会被清除。",
    leave_title: "退出“{name}”的托管？",
    leave_body: "网络会停止并从此设备移除。控制平面上可能仍会列出这台机器。",
    leave_action: "退出托管",

    search_placeholder: "按名称或网络名称搜索",
    search_clear: "清除搜索",
    filter_label: "筛选网络",
    filter_all: "全部",
    filter_running: "运行中",
    filter_stopped: "未运行",
    col_name: "网络",
    col_status: "状态",
    col_peers: "对等节点",
    col_rx: "接收",
    col_tx: "发送",
    summary_running: "运行中的网络",
    summary_running_of: "共 {total} 个",
    summary_peers: "在线对等节点",
    summary_direct: "直连",
    summary_relayed: "中继",
    summary_rx: "接收速率",
    summary_tx: "发送速率",
    summary_core: "EasyTier 内核",
    summary_library: "EasyTier 库",
    none_match_title: "没有符合条件的网络",
    none_match_body: "调整搜索词或筛选条件，查看更多网络。",
    none_match_action: "清除搜索和筛选",
};
