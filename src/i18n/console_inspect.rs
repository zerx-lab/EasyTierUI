//! Console role, machine drawer: the tabs that read a running network (Node, Routes, Relay, Stats) and the tab
//! switcher.
//!
//! One field per string; `EN` and `ZH_CN` must both set every field. Templates use `{name}` placeholders filled by
//! [`super::fill`]. Terms follow the console strings: a machine is 设备, a network 网络, a node 节点, a peer 对等节点.

/// Strings of this area in one language.
#[derive(Debug)]
pub struct ConsoleInspectStrings {
    // --- The tab switcher ---
    /// Tab: the node's own facts.
    pub tab_node: &'static str,
    /// Tab: the routing table.
    pub tab_routes: &'static str,
    /// Tab: the networks relayed here and the peer map.
    pub tab_relay: &'static str,
    /// Tab: the metrics.
    pub tab_stats: &'static str,
    /// Tab: connectors, mapped listeners and the VPN portal.
    pub tab_connections: &'static str,
    /// Tab: port forwards and proxied subnets.
    pub tab_forwarding: &'static str,
    /// Tab: access rules and open ports.
    pub tab_access: &'static str,
    /// Tab: the core's log level.
    pub tab_logs: &'static str,
    /// The menu beside the tab strip while none of its views shows.
    pub tabs_more: &'static str,
    /// The menu beside the tab strip, as its accessible name.
    pub tabs_more_label: &'static str,

    // --- The running network the tabs are about ---
    /// What shows instead of a per-network tab while no network of the machine runs.
    pub scope_no_network_body: &'static str,

    // --- Every read ---
    /// When the list was read; `{age}` is "12 s ago".
    pub read_ago: &'static str,

    // --- Node ---
    /// Node tab: heading of the identity facts.
    pub node_h_identity: &'static str,
    /// Node tab: heading of the connectivity facts.
    pub node_h_nat: &'static str,
    /// Node tab: heading of the address lists.
    pub node_h_addresses: &'static str,
    /// Node tab: heading of the feature flags.
    pub node_h_features: &'static str,
    /// Node tab: the node's virtual IPv4.
    pub node_virtual_ip: &'static str,
    /// Node tab: the node's peer id.
    pub node_peer_id: &'static str,
    /// Node tab: the hostname the node announces.
    pub node_hostname: &'static str,
    /// Node tab: the core's version.
    pub node_version: &'static str,
    /// Node tab: the UDP NAT type.
    pub node_udp_nat: &'static str,
    /// Node tab: the TCP NAT type.
    pub node_tcp_nat: &'static str,
    /// Node tab: the public port range STUN saw.
    pub node_port_range: &'static str,
    /// Node tab: when STUN last ran.
    pub node_nat_checked: &'static str,
    /// Node tab: what the NAT types mean for connecting.
    pub node_nat_hint: &'static str,
    /// Node tab: where the node listens.
    pub node_listeners: &'static str,
    /// Node tab: the subnets the node proxies.
    pub node_proxy_cidrs: &'static str,
    /// Node tab: the addresses the node is seen at from outside.
    pub node_public_addresses: &'static str,
    /// Node tab: the addresses of the machine's network interfaces.
    pub node_interface_addresses: &'static str,
    /// Node tab: a flag that is set.
    pub flag_yes: &'static str,
    /// Node tab: a flag that is not set.
    pub flag_no: &'static str,
    /// Feature flag: a public server.
    pub flag_public_server: &'static str,
    /// Feature flag: refuses to relay data.
    pub flag_avoid_relay_data: &'static str,
    /// Feature flag: accepts KCP-proxied TCP.
    pub flag_kcp_input: &'static str,
    /// Feature flag: refuses to relay KCP.
    pub flag_no_relay_kcp: &'static str,
    /// Feature flag: syncs its connection list.
    pub flag_conn_list_sync: &'static str,
    /// Feature flag: accepts QUIC-proxied TCP.
    pub flag_quic_input: &'static str,
    /// Feature flag: refuses to relay QUIC.
    pub flag_no_relay_quic: &'static str,
    /// Feature flag: joined with a credential.
    pub flag_credential_peer: &'static str,
    /// Feature flag: wants direct connections only.
    pub flag_need_p2p: &'static str,
    /// Feature flag: does not hole punch.
    pub flag_disable_p2p: &'static str,
    /// Feature flag: hands out public IPv6 addresses.
    pub flag_ipv6_provider: &'static str,

    // --- Routes ---
    /// Routes tab: the table has no row.
    pub routes_empty_title: &'static str,
    /// Routes tab: the table has no row, body.
    pub routes_empty_body: &'static str,
    /// Routes tab: the hostname column.
    pub routes_col_host: &'static str,
    /// Routes tab: the next hop column.
    pub routes_col_next_hop: &'static str,
    /// Routes tab: what the next hop is.
    pub routes_col_next_hop_hint: &'static str,
    /// Routes tab: the proxied subnets column.
    pub routes_col_subnets: &'static str,
    /// Routes tab: a route through other nodes; `{hops}` is the number of hops.
    pub routes_relayed_hops: &'static str,

    // --- Relay ---
    /// Relay tab: heading of the networks relayed here.
    pub relay_h_here: &'static str,
    /// Relay tab: heading of the relay summary.
    pub relay_h_summary: &'static str,
    /// Relay tab: heading of the networks relayed across the mesh.
    pub relay_h_mesh: &'static str,
    /// Relay tab: heading of the peer map.
    pub relay_h_map: &'static str,
    /// Relay tab: no foreign network is relayed here.
    pub relay_here_empty_title: &'static str,
    /// Relay tab: no foreign network is relayed here, body.
    pub relay_here_empty_body: &'static str,
    /// Relay tab: no relay summary.
    pub relay_summary_empty_title: &'static str,
    /// Relay tab: no relay summary, body.
    pub relay_summary_empty_body: &'static str,
    /// Relay tab: no foreign network in the mesh.
    pub relay_mesh_empty_title: &'static str,
    /// Relay tab: no foreign network in the mesh, body.
    pub relay_mesh_empty_body: &'static str,
    /// Relay tab: the peer map is empty.
    pub relay_map_empty_title: &'static str,
    /// Relay tab: the peer map is empty, body.
    pub relay_map_empty_body: &'static str,
    /// Relay tab: how many peers a foreign network has here; `{count}`.
    pub relay_network_peers: &'static str,
    /// Relay tab: the id this node has inside a foreign network; `{id}`.
    pub relay_my_id: &'static str,
    /// Relay tab: a foreign network without a link.
    pub relay_no_links: &'static str,
    /// Relay tab: the tunnel column.
    pub relay_col_tunnel: &'static str,
    /// Relay tab: the remote address column.
    pub relay_col_remote: &'static str,
    /// Relay tab: the relaying node column.
    pub relay_col_node: &'static str,
    /// Relay tab: the network name column.
    pub relay_col_network: &'static str,
    /// Relay tab: the number of networks column.
    pub relay_col_networks: &'static str,
    /// Relay tab: the number of peers column.
    pub relay_col_peers: &'static str,
    /// Relay tab: the report time column.
    pub relay_col_updated: &'static str,
    /// Relay tab: a node of the peer map and how many direct peers it has; `{id}`, `{count}`.
    pub relay_map_node: &'static str,

    // --- Stats ---
    /// Stats tab: the search field.
    pub stats_search: &'static str,
    /// Stats tab: no metric.
    pub stats_empty_title: &'static str,
    /// Stats tab: no metric, body.
    pub stats_empty_body: &'static str,
    /// Stats tab: the metric name column.
    pub stats_col_name: &'static str,
    /// Stats tab: the labels column.
    pub stats_col_labels: &'static str,
    /// Stats tab: the value column.
    pub stats_col_value: &'static str,
    /// Stats tab: copies the metrics as Prometheus text.
    pub stats_copy: &'static str,
    /// Stats tab: the text is being read.
    pub stats_copy_reading: &'static str,
    /// Stats tab: the text is on the clipboard.
    pub stats_copy_done: &'static str,
}

pub(super) static EN: ConsoleInspectStrings = ConsoleInspectStrings {
    tab_node: "Node",
    tab_routes: "Routes",
    tab_relay: "Relay",
    tab_stats: "Stats",
    tab_connections: "Connections",
    tab_forwarding: "Forwarding",
    tab_access: "Access",
    tab_logs: "Logs",
    tabs_more: "More…",
    tabs_more_label: "More views",

    scope_no_network_body: "A network's details show once a network on this machine is running.",

    read_ago: "Read {age}",

    node_h_identity: "Identity",
    node_h_nat: "Connectivity",
    node_h_addresses: "Addresses",
    node_h_features: "Features",
    node_virtual_ip: "Virtual IP",
    node_peer_id: "Peer ID",
    node_hostname: "Hostname",
    node_version: "Core version",
    node_udp_nat: "UDP NAT",
    node_tcp_nat: "TCP NAT",
    node_port_range: "Public ports seen",
    node_nat_checked: "Checked",
    node_nat_hint: "Open internet and full cone connect to peers directly. Symmetric types often need a relay.",
    node_listeners: "Listeners",
    node_proxy_cidrs: "Proxied subnets",
    node_public_addresses: "Public addresses",
    node_interface_addresses: "Interface addresses",
    flag_yes: "Yes",
    flag_no: "No",
    flag_public_server: "Public server",
    flag_avoid_relay_data: "Refuses to relay data",
    flag_kcp_input: "Accepts KCP proxy",
    flag_no_relay_kcp: "Refuses to relay KCP",
    flag_conn_list_sync: "Syncs its connection list",
    flag_quic_input: "Accepts QUIC proxy",
    flag_no_relay_quic: "Refuses to relay QUIC",
    flag_credential_peer: "Joined with a credential",
    flag_need_p2p: "Direct connections only",
    flag_disable_p2p: "Hole punching off",
    flag_ipv6_provider: "Hands out public IPv6",

    routes_empty_title: "No routes yet",
    routes_empty_body: "Other nodes appear here once this node knows a way to them.",
    routes_col_host: "Hostname",
    routes_col_next_hop: "Next hop",
    routes_col_next_hop_hint: "The peer that carries the traffic first. On a direct link it is the node itself.",
    routes_col_subnets: "Subnets",
    routes_relayed_hops: "Relayed · {hops} hops",

    relay_h_here: "Networks relayed here",
    relay_h_summary: "Relay summary",
    relay_h_mesh: "Relayed across the mesh",
    relay_h_map: "Peer map",
    relay_here_empty_title: "No other network is relayed here",
    relay_here_empty_body: "Nodes of other networks that connect through this node would show here.",
    relay_summary_empty_title: "No relay summary",
    relay_summary_empty_body: "The mesh has not reported how much it relays.",
    relay_mesh_empty_title: "No network is relayed in the mesh",
    relay_mesh_empty_body: "No node reports carrying another network's traffic.",
    relay_map_empty_title: "The peer map is empty",
    relay_map_empty_body: "The peer center has not collected any direct link yet.",
    relay_network_peers: "{count} peers",
    relay_my_id: "This node's ID there: {id}",
    relay_no_links: "No peer of this network is connected.",
    relay_col_tunnel: "Tunnel",
    relay_col_remote: "Address",
    relay_col_node: "Relaying node",
    relay_col_network: "Network",
    relay_col_networks: "Networks",
    relay_col_peers: "Peers",
    relay_col_updated: "Reported",
    relay_map_node: "Node {id} · {count} direct",

    stats_search: "Search by metric or label",
    stats_empty_title: "No metrics yet",
    stats_empty_body: "The core has not counted anything for this network.",
    stats_col_name: "Metric",
    stats_col_labels: "Labels",
    stats_col_value: "Value",
    stats_copy: "Copy Prometheus text",
    stats_copy_reading: "Reading…",
    stats_copy_done: "Copied",
};

pub(super) static ZH_CN: ConsoleInspectStrings = ConsoleInspectStrings {
    tab_node: "节点",
    tab_routes: "路由",
    tab_relay: "中继",
    tab_stats: "统计",
    tab_connections: "连接",
    tab_forwarding: "转发",
    tab_access: "访问",
    tab_logs: "日志",
    tabs_more: "更多…",
    tabs_more_label: "更多视图",

    scope_no_network_body: "此设备上的网络开始运行后，这里会显示网络的详细信息。",

    read_ago: "读取于 {age}",

    node_h_identity: "标识",
    node_h_nat: "连通性",
    node_h_addresses: "地址",
    node_h_features: "特性",
    node_virtual_ip: "虚拟 IP",
    node_peer_id: "对等节点 ID",
    node_hostname: "主机名",
    node_version: "内核版本",
    node_udp_nat: "UDP NAT",
    node_tcp_nat: "TCP NAT",
    node_port_range: "探测到的公网端口",
    node_nat_checked: "检测时间",
    node_nat_hint: "开放互联网和完全锥形可与对等节点直连；对称型通常需要中继。",
    node_listeners: "监听地址",
    node_proxy_cidrs: "代理子网",
    node_public_addresses: "公网地址",
    node_interface_addresses: "网卡地址",
    flag_yes: "是",
    flag_no: "否",
    flag_public_server: "公共服务器",
    flag_avoid_relay_data: "拒绝中继数据",
    flag_kcp_input: "接受 KCP 代理",
    flag_no_relay_kcp: "拒绝中继 KCP",
    flag_conn_list_sync: "同步连接列表",
    flag_quic_input: "接受 QUIC 代理",
    flag_no_relay_quic: "拒绝中继 QUIC",
    flag_credential_peer: "通过凭据加入",
    flag_need_p2p: "仅直连",
    flag_disable_p2p: "已关闭打洞",
    flag_ipv6_provider: "分配公网 IPv6",

    routes_empty_title: "还没有路由",
    routes_empty_body: "此节点知道通往其他节点的路径后，它们会显示在这里。",
    routes_col_host: "主机名",
    routes_col_next_hop: "下一跳",
    routes_col_next_hop_hint: "先承载流量的对等节点。直连时就是该节点自己。",
    routes_col_subnets: "子网",
    routes_relayed_hops: "中继 · {hops} 跳",

    relay_h_here: "在此中继的网络",
    relay_h_summary: "中继概览",
    relay_h_mesh: "整个组网中继的网络",
    relay_h_map: "对等节点图",
    relay_here_empty_title: "此处没有中继其他网络",
    relay_here_empty_body: "其他网络的节点经由本节点连接时，会显示在这里。",
    relay_summary_empty_title: "没有中继概览",
    relay_summary_empty_body: "组网中还没有节点报告中继情况。",
    relay_mesh_empty_title: "组网中没有中继的网络",
    relay_mesh_empty_body: "没有节点报告在转发其他网络的流量。",
    relay_map_empty_title: "对等节点图为空",
    relay_map_empty_body: "对等节点中心还没有收集到直连链路。",
    relay_network_peers: "{count} 个对等节点",
    relay_my_id: "本节点在该网络中的 ID：{id}",
    relay_no_links: "该网络没有已连接的对等节点。",
    relay_col_tunnel: "隧道",
    relay_col_remote: "地址",
    relay_col_node: "中继节点",
    relay_col_network: "网络",
    relay_col_networks: "网络数",
    relay_col_peers: "对等节点",
    relay_col_updated: "报告时间",
    relay_map_node: "节点 {id} · {count} 个直连",

    stats_search: "按指标或标签搜索",
    stats_empty_title: "还没有指标",
    stats_empty_body: "内核还没有为此网络计数。",
    stats_col_name: "指标",
    stats_col_labels: "标签",
    stats_col_value: "值",
    stats_copy: "复制 Prometheus 文本",
    stats_copy_reading: "读取中…",
    stats_copy_done: "已复制",
};
