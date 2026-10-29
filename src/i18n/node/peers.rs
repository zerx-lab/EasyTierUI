//! Node role: the Peers page and the peer drawer.

/// Strings of this area in one language.
#[derive(Debug)]
pub struct PeersStrings {
    /// Column: hostname.
    pub col_host: &'static str,
    /// Column: virtual IP.
    pub col_ip: &'static str,
    /// Column: latency.
    pub col_latency: &'static str,
    /// Column: packet loss.
    pub col_loss: &'static str,
    /// Column: the rate received from the peer now.
    pub col_rx: &'static str,
    /// Column: the rate sent to the peer now.
    pub col_tx: &'static str,
    /// Drawer: bytes received from the peer since its link came up.
    pub label_rx_total: &'static str,
    /// Drawer: bytes sent to the peer since its link came up.
    pub label_tx_total: &'static str,
    /// Column: route.
    pub col_route: &'static str,
    /// Column: the tunnel protocol.
    pub col_tunnel: &'static str,
    /// Column: the NAT type.
    pub col_nat: &'static str,
    /// Column: the EasyTier version.
    pub col_version: &'static str,
    /// Tooltip of the latency column (`{warn}`).
    pub latency_hint: &'static str,
    /// Tooltip of the loss column (`{warn}`, `{bad}`).
    pub loss_hint: &'static str,
    /// Route chip: direct.
    pub route_direct: &'static str,
    /// Route chip: relayed (`{hops}`).
    pub route_relayed: &'static str,
    /// Route chip: unknown.
    pub route_unknown: &'static str,
    /// The age of the reading: `{age}` is whole seconds.
    pub updated_ago: &'static str,
    /// The core has not answered yet.
    pub waiting: &'static str,
    /// The network is stopping.
    pub stopping: &'static str,
    /// Nobody else is in the network yet: title.
    pub none_title: &'static str,
    /// Nobody else is in the network yet: body.
    pub none_body: &'static str,
    /// Label before this node's own virtual IP.
    pub your_ip: &'static str,
    /// Caption of the headline number.
    pub peers_online: &'static str,
    /// Where to start the network from.
    pub start_from_networks: &'static str,
    /// The action that opens the Networks page.
    pub open_networks: &'static str,

    /// The network selector's accessible name.
    pub network_label: &'static str,
    /// The network selector's option for every running network.
    pub network_all: &'static str,
    /// The search field's placeholder and accessible name.
    pub search_placeholder: &'static str,
    /// The search field's clear button.
    pub search_clear: &'static str,
    /// The name of the route filter.
    pub route_label: &'static str,
    /// Route filter: every peer.
    pub filter_all: &'static str,
    /// Route filter: direct peers.
    pub filter_direct: &'static str,
    /// Route filter: relayed peers.
    pub filter_relayed: &'static str,
    /// Summary: peers online.
    pub summary_online: &'static str,
    /// Summary: median latency.
    pub summary_latency: &'static str,
    /// Caption of the median: of how many measured peers (`{count}`).
    pub summary_latency_of: &'static str,
    /// Summary: the worst packet loss.
    pub summary_loss: &'static str,
    /// Summary: peers reached directly.
    pub summary_direct: &'static str,
    /// Summary: peers reached through relays.
    pub summary_relayed: &'static str,
    /// Chart: throughput over time.
    pub chart_throughput: &'static str,
    /// Chart: latency of the slowest peers.
    pub chart_latency: &'static str,
    /// Chart: legend of the received series.
    pub legend_received: &'static str,
    /// Chart: legend of the sent series.
    pub legend_sent: &'static str,
    /// Chart: before there are two samples.
    pub chart_collecting: &'static str,
    /// Chart: the left end of the time axis.
    pub chart_span_from: &'static str,
    /// Chart: the right end of the time axis.
    pub chart_span_to: &'static str,
    /// No peer matches the search or the route filter: title.
    pub no_match_title: &'static str,
    /// No peer matches the search or the route filter: body.
    pub no_match_body: &'static str,
    /// Clears the search and the route filter.
    pub no_match_action: &'static str,
    /// A table row's accessible name (`{name}`).
    pub row_label: &'static str,
    /// The peer drawer: its close button.
    pub drawer_close: &'static str,
    /// The peer drawer: the peer left the network while it was open.
    pub drawer_gone: &'static str,
    /// The peer drawer: its network.
    pub label_network: &'static str,
    /// The peer drawer: the core's numeric id.
    pub label_peer_id: &'static str,
    /// The peer drawer: virtual IP with its prefix.
    pub label_ip: &'static str,
    /// The peer drawer: latency chart heading.
    pub chart_peer_latency: &'static str,
    /// The peer drawer: loss chart heading.
    pub chart_peer_loss: &'static str,
    /// The peer drawer: opens the peer's network.
    pub open_network: &'static str,
}

pub(super) static EN: PeersStrings = PeersStrings {
    col_host: "Hostname",
    col_ip: "Virtual IP",
    col_latency: "Latency",
    col_loss: "Loss",
    col_rx: "Received",
    col_tx: "Sent",
    label_rx_total: "Received in total",
    label_tx_total: "Sent in total",
    col_route: "Route",
    col_tunnel: "Tunnel",
    col_nat: "NAT type",
    col_version: "Version",
    latency_hint: "Green below {warn} ms, amber from there on. A bar is full at 3 times that.",
    loss_hint: "Green below {warn} %, amber from there on, red from {bad} %.",
    route_direct: "Direct (P2P)",
    route_relayed: "Relayed, {hops} hops",
    route_unknown: "Route unknown",
    updated_ago: "Updated {age} s ago",
    waiting: "Waiting for the core to answer…",
    stopping: "Stopping…",
    none_title: "No other node yet",
    none_body: "Nodes appear here once they join this network.",
    your_ip: "Your virtual IP",
    peers_online: "peers online",
    start_from_networks: "Start or fix it on the Networks page.",
    open_networks: "Open Networks",

    network_label: "Network",
    network_all: "All networks",
    search_placeholder: "Search by hostname or IP",
    search_clear: "Clear search",
    route_label: "Route",
    filter_all: "All",
    filter_direct: "Direct",
    filter_relayed: "Relayed",
    summary_online: "Peers online",
    summary_latency: "Median latency",
    summary_latency_of: "of {count} measured",
    summary_loss: "Worst loss",
    summary_direct: "direct",
    summary_relayed: "relayed",
    chart_throughput: "Throughput",
    chart_latency: "Latency of the slowest peers",
    legend_received: "Received",
    legend_sent: "Sent",
    chart_collecting: "Collecting samples…",
    chart_span_from: "3 min ago",
    chart_span_to: "now",
    no_match_title: "No peer matches",
    no_match_body: "Change the search or the route filter to see more peers.",
    no_match_action: "Clear search and filter",
    row_label: "Peer {name}",
    drawer_close: "Close peer",
    drawer_gone: "This peer has left the network, or the network stopped. Close this panel to go back.",
    label_network: "Network",
    label_peer_id: "Peer ID",
    label_ip: "Virtual IP",
    chart_peer_latency: "Latency over time",
    chart_peer_loss: "Packet loss over time",
    open_network: "Open network",
};

pub(super) static ZH_CN: PeersStrings = PeersStrings {
    col_host: "主机名",
    col_ip: "虚拟 IP",
    col_latency: "延迟",
    col_loss: "丢包",
    col_rx: "接收",
    col_tx: "发送",
    label_rx_total: "累计接收",
    label_tx_total: "累计发送",
    col_route: "路由",
    col_tunnel: "隧道",
    col_nat: "NAT 类型",
    col_version: "版本",
    latency_hint: "低于 {warn} ms 为绿色，达到后变为琥珀色。条形在其 3 倍时满格。",
    loss_hint: "低于 {warn} % 为绿色，达到后变为琥珀色，达到 {bad} % 变为红色。",
    route_direct: "直连 (P2P)",
    route_relayed: "中继，{hops} 跳",
    route_unknown: "路由未知",
    updated_ago: "{age} 秒前更新",
    waiting: "正在等待内核响应…",
    stopping: "正在停止…",
    none_title: "还没有其他节点",
    none_body: "其他节点加入这个网络后会显示在这里。",
    your_ip: "你的虚拟 IP",
    peers_online: "在线对等节点",
    start_from_networks: "请在“网络”页启动或排查。",
    open_networks: "打开网络",

    network_label: "网络",
    network_all: "全部网络",
    search_placeholder: "按主机名或 IP 搜索",
    search_clear: "清除搜索",
    route_label: "路由",
    filter_all: "全部",
    filter_direct: "直连",
    filter_relayed: "中继",
    summary_online: "在线对等节点",
    summary_latency: "延迟中位数",
    summary_latency_of: "基于 {count} 个有测量值的节点",
    summary_loss: "最高丢包",
    summary_direct: "直连",
    summary_relayed: "中继",
    chart_throughput: "吞吐量",
    chart_latency: "延迟最高的对等节点",
    legend_received: "接收",
    legend_sent: "发送",
    chart_collecting: "正在收集样本…",
    chart_span_from: "3 分钟前",
    chart_span_to: "现在",
    no_match_title: "没有符合条件的对等节点",
    no_match_body: "调整搜索词或路由筛选，查看更多对等节点。",
    no_match_action: "清除搜索和筛选",
    row_label: "对等节点 {name}",
    drawer_close: "关闭对等节点详情",
    drawer_gone: "这个对等节点已离开网络，或网络已停止。关闭此面板即可返回。",
    label_network: "所属网络",
    label_peer_id: "对等节点 ID",
    label_ip: "虚拟 IP",
    chart_peer_latency: "延迟变化",
    chart_peer_loss: "丢包变化",
    open_network: "打开网络",
};
