//! The console machine drawer's runtime tabs: connections, forwarding, access and logs of a running network.
//!
//! One field per string; `EN` and `ZH_CN` must both set every field (the struct makes a missing one a compile
//! error). Templates use `{name}` placeholders filled by [`super::fill`].

/// Strings of this area in one language.
#[derive(Debug)]
pub struct ConsoleRuntimeStrings {
    // --- Shared ---
    /// Note on every changing tab: a change applies to the running network and is not saved.
    pub note_unsaved: &'static str,
    /// Button of that note: open the network's config editor.
    pub edit_config: &'static str,
    /// Button of the failure banner: hide it.
    pub dismiss: &'static str,
    /// Warning when a list could not be read again but an older value shows.
    pub refresh_failed: &'static str,
    /// Button: add the value typed in a form.
    pub add: &'static str,
    /// Button: send the node settings typed in a form.
    pub apply: &'static str,
    /// Button and confirmation label: remove one entry.
    pub remove: &'static str,
    /// Confirmation label: remove every entry of a list.
    pub clear: &'static str,
    /// Meta line: a change is being sent.
    pub applying: &'static str,
    /// Protocol choice: TCP.
    pub proto_tcp: &'static str,
    /// Protocol choice: UDP.
    pub proto_udp: &'static str,
    /// Column header: a URL.
    pub col_url: &'static str,
    /// Column header: the state of a link.
    pub col_status: &'static str,
    /// Column header: a protocol.
    pub col_protocol: &'static str,
    /// Column header: a local address and port.
    pub col_bind: &'static str,
    /// Column header: where traffic goes.
    pub col_destination: &'static str,
    /// Column header: a subnet.
    pub col_subnet: &'static str,
    /// Column header: the range a subnet is announced as.
    pub col_announced_as: &'static str,
    /// Column header: a manual route.
    pub col_route: &'static str,
    /// Column header: an IP address.
    pub col_address: &'static str,
    /// Column header: ports or port ranges.
    pub col_ports: &'static str,
    /// Column header: an ACL rule.
    pub col_rule: &'static str,
    /// Column header: what an ACL rule does.
    pub col_action: &'static str,
    /// Column header: packets counted.
    pub col_packets: &'static str,
    /// Column header: bytes counted.
    pub col_bytes: &'static str,
    /// Column header: a tracked connection's source.
    pub col_source: &'static str,
    /// Column header: a tracked connection's state.
    pub col_state: &'static str,
    /// Column header: a counter's name.
    pub col_counter: &'static str,
    /// Column header: a counter's value.
    pub col_value: &'static str,

    // --- Connections ---
    /// Section title: connectors.
    pub connectors_title: &'static str,
    /// Section sentence: what a connector is.
    pub connectors_hint: &'static str,
    /// Shown when there is no connector.
    pub connectors_empty: &'static str,
    /// Field label: a connector's URL.
    pub field_connector: &'static str,
    /// Placeholder for any peer or listener URL.
    pub placeholder_url: &'static str,
    /// Link state: connected.
    pub status_connected: &'static str,
    /// Link state: dialing.
    pub status_connecting: &'static str,
    /// Link state: down, the core retries.
    pub status_disconnected: &'static str,
    /// Link state: not one this app knows.
    pub status_unknown: &'static str,
    /// Confirmation title for removing a connector; `{url}`.
    pub remove_connector_title: &'static str,
    /// Confirmation body for removing a connector.
    pub remove_connector_body: &'static str,
    /// Button: remove every connector.
    pub connectors_clear: &'static str,
    /// Confirmation title for removing every connector.
    pub clear_connectors_title: &'static str,
    /// Confirmation body for removing every connector.
    pub clear_connectors_body: &'static str,
    /// Section title: mapped listeners.
    pub mapped_title: &'static str,
    /// Section sentence: what a mapped listener is.
    pub mapped_hint: &'static str,
    /// Shown when there is no mapped listener.
    pub mapped_empty: &'static str,
    /// Field label: a mapped listener's URL.
    pub field_mapped_listener: &'static str,
    /// Confirmation title for removing a mapped listener; `{url}`.
    pub remove_mapped_title: &'static str,
    /// Confirmation body for removing a mapped listener.
    pub remove_mapped_body: &'static str,
    /// Button: remove every mapped listener.
    pub mapped_clear: &'static str,
    /// Confirmation title for removing every mapped listener.
    pub clear_mapped_title: &'static str,
    /// Confirmation body for removing every mapped listener.
    pub clear_mapped_body: &'static str,

    // --- Forwarding ---
    /// Section title: port forwards.
    pub forwards_title: &'static str,
    /// Section sentence: what a port forward does.
    pub forwards_hint: &'static str,
    /// Shown when there is no port forward.
    pub forwards_empty: &'static str,
    /// Field label: the address the node listens on.
    pub field_bind: &'static str,
    /// Placeholder: the address the node listens on.
    pub placeholder_bind: &'static str,
    /// Field label: where traffic goes.
    pub field_destination: &'static str,
    /// Placeholder: where traffic goes.
    pub placeholder_destination: &'static str,
    /// Confirmation title for removing a port forward; `{protocol}`, `{bind}`, `{dst}`.
    pub remove_forward_title: &'static str,
    /// Confirmation body for removing a port forward.
    pub remove_forward_body: &'static str,
    /// Section title: subnet proxies.
    pub subnets_title: &'static str,
    /// Section sentence: what a subnet proxy exposes.
    pub subnets_hint: &'static str,
    /// Shown when there is no subnet proxy.
    pub subnets_empty: &'static str,
    /// Field label: the subnet to proxy.
    pub field_subnet: &'static str,
    /// Placeholder: the subnet to proxy.
    pub placeholder_subnet: &'static str,
    /// Field label: the range the subnet is announced as (optional).
    pub field_subnet_mapped: &'static str,
    /// Placeholder: the range the subnet is announced as.
    pub placeholder_subnet_mapped: &'static str,
    /// Confirmation title for removing a subnet proxy; `{cidr}`.
    pub remove_subnet_title: &'static str,
    /// Confirmation body for removing a subnet proxy.
    pub remove_subnet_body: &'static str,
    /// Section title: manual routes.
    pub routes_title: &'static str,
    /// Section sentence: what a manual route is and when it applies.
    pub routes_hint: &'static str,
    /// Shown when there is no manual route.
    pub routes_empty: &'static str,
    /// Warning: the network's config has manual routing off, so routes added here do nothing.
    pub routes_off_note: &'static str,
    /// Field label: a manual route.
    pub field_route: &'static str,
    /// Placeholder: a manual route.
    pub placeholder_route: &'static str,
    /// Confirmation title for removing a manual route; `{value}`.
    pub remove_route_title: &'static str,
    /// Confirmation body for removing a manual route.
    pub remove_route_body: &'static str,
    /// Section title: exit nodes.
    pub exit_nodes_title: &'static str,
    /// Section sentence: what an exit node does.
    pub exit_nodes_hint: &'static str,
    /// Shown when there is no exit node.
    pub exit_nodes_empty: &'static str,
    /// Field label: an exit node's virtual IP.
    pub field_exit_node: &'static str,
    /// Placeholder: an exit node's virtual IP.
    pub placeholder_exit_node: &'static str,
    /// Confirmation title for removing an exit node; `{value}`.
    pub remove_exit_node_title: &'static str,
    /// Confirmation body for removing an exit node.
    pub remove_exit_node_body: &'static str,
    /// Section title: the node's own settings.
    pub node_title: &'static str,
    /// Section sentence: what the settings change.
    pub node_hint: &'static str,
    /// Field label: the hostname.
    pub field_hostname: &'static str,
    /// Placeholder: the hostname.
    pub placeholder_hostname: &'static str,
    /// Helper: the hostname now; `{name}`.
    pub hostname_current: &'static str,
    /// Helper: the node announces the machine's own hostname.
    pub hostname_default: &'static str,
    /// Field label: the virtual IPv4 with its prefix.
    pub field_virtual_ip: &'static str,
    /// Placeholder: the virtual IPv4 with its prefix.
    pub placeholder_virtual_ip: &'static str,
    /// Helper: the address now; `{ip}`.
    pub virtual_ip_current: &'static str,
    /// Helper: the network assigns the address by DHCP, so a typed one is ignored.
    pub virtual_ip_dhcp: &'static str,
    /// Helper: the node has no address.
    pub virtual_ip_none: &'static str,
    /// Switch label: do not relay other nodes' data.
    pub relay_off_label: &'static str,
    /// Switch description.
    pub relay_off_help: &'static str,
    /// Section title: the VPN portal.
    pub vpn_title: &'static str,
    /// Section sentence: what the VPN portal is.
    pub vpn_hint: &'static str,
    /// Shown when the network has no VPN portal.
    pub vpn_none: &'static str,
    /// Fact label: the portal's kind.
    pub vpn_type: &'static str,
    /// Meta line: how many clients are connected; `{count}`.
    pub vpn_clients: &'static str,
    /// Meta line: no client is connected.
    pub vpn_no_clients: &'static str,
    /// Warning before the client config is shown: it holds a private key.
    pub vpn_config_secret: &'static str,
    /// Button: read and show the client config.
    pub vpn_show_config: &'static str,
    /// Warning while the client config is shown.
    pub vpn_config_shown: &'static str,
    /// Button: hide the client config again.
    pub vpn_hide_config: &'static str,

    // --- Access ---
    /// Section title: the ports open to the network.
    pub whitelist_title: &'static str,
    /// Section sentence: what an open port is.
    pub whitelist_hint: &'static str,
    /// Shown when no port is open.
    pub whitelist_empty: &'static str,
    /// Field label: a port or range.
    pub field_port: &'static str,
    /// Placeholder: a port or range.
    pub placeholder_port: &'static str,
    /// Confirmation title for closing a port; `{protocol}`, `{port}`.
    pub remove_port_title: &'static str,
    /// Confirmation body for closing a port.
    pub remove_port_body: &'static str,
    /// Button: close every TCP port.
    pub clear_tcp: &'static str,
    /// Button: close every UDP port.
    pub clear_udp: &'static str,
    /// Confirmation title for closing every port of a protocol; `{protocol}`.
    pub clear_ports_title: &'static str,
    /// Confirmation body for closing every port of a protocol.
    pub clear_ports_body: &'static str,
    /// Section title: the ACL.
    pub acl_title: &'static str,
    /// Section sentence: what the ACL counters are.
    pub acl_hint: &'static str,
    /// Caption: the ACL rules.
    pub acl_rules: &'static str,
    /// Shown when there is no ACL rule.
    pub acl_rules_empty: &'static str,
    /// Caption: the tracked connections.
    pub acl_connections: &'static str,
    /// Shown when no connection is tracked.
    pub acl_connections_empty: &'static str,
    /// Caption: the core's counters.
    pub acl_counters: &'static str,
    /// Shown when the core reports no counter.
    pub acl_counters_empty: &'static str,
    /// Rule action: allow.
    pub acl_allow: &'static str,
    /// Rule action: drop.
    pub acl_drop: &'static str,
    /// Rule action: only count.
    pub acl_count_only: &'static str,
    /// Rule state: switched off.
    pub acl_rule_off: &'static str,
    /// Any protocol.
    pub acl_any: &'static str,
    /// A value this app does not know.
    pub acl_unknown: &'static str,
    /// Connection state: first packet seen.
    pub conn_new: &'static str,
    /// Connection state: both directions seen.
    pub conn_established: &'static str,
    /// Connection state: related to another connection.
    pub conn_related: &'static str,
    /// Connection state: not valid.
    pub conn_invalid: &'static str,

    // --- Logs ---
    /// Note: the level is the whole core's and is not saved.
    pub logs_note: &'static str,
    /// Section title: the log level.
    pub logs_title: &'static str,
    /// Section sentence.
    pub logs_hint: &'static str,
    /// Select label: the log level.
    pub field_log_level: &'static str,
    /// Meta line under the select.
    pub log_level_help: &'static str,
    /// Level: no logging.
    pub level_disabled: &'static str,
    /// Level: errors.
    pub level_error: &'static str,
    /// Level: warnings.
    pub level_warning: &'static str,
    /// Level: information.
    pub level_info: &'static str,
    /// Level: debugging.
    pub level_debug: &'static str,
    /// Level: everything.
    pub level_trace: &'static str,
}

pub(super) static EN: ConsoleRuntimeStrings = ConsoleRuntimeStrings {
    note_unsaved: "Changes here apply now, to the running network only. They are lost when the machine reconnects or \
                   restarts unless you save them in the network's config.",
    edit_config: "Edit config",
    dismiss: "Dismiss",
    refresh_failed: "Couldn't read this again. Showing the last result.",
    add: "Add",
    apply: "Apply",
    remove: "Remove",
    clear: "Remove all",
    applying: "Applying…",
    proto_tcp: "TCP",
    proto_udp: "UDP",
    col_url: "Address",
    col_status: "Link",
    col_protocol: "Protocol",
    col_bind: "Listens on",
    col_destination: "Sends to",
    col_subnet: "Subnet",
    col_announced_as: "Announced as",
    col_route: "Route",
    col_address: "Address",
    col_ports: "Ports",
    col_rule: "Rule",
    col_action: "Action",
    col_packets: "Packets",
    col_bytes: "Data",
    col_source: "From",
    col_state: "State",
    col_counter: "Counter",
    col_value: "Value",

    connectors_title: "Connectors",
    connectors_hint: "Peers this node dials by itself and keeps a link to.",
    connectors_empty: "No connectors. This node only dials the peers its network config lists.",
    field_connector: "Connector",
    placeholder_url: "tcp://203.0.113.5:11010",
    status_connected: "Connected",
    status_connecting: "Connecting",
    status_disconnected: "Disconnected",
    status_unknown: "Unknown",
    remove_connector_title: "Remove connector {url}?",
    remove_connector_body: "This node stops dialing it and drops the link. The change applies to the running network \
                            only.",
    connectors_clear: "Remove all connectors",
    clear_connectors_title: "Remove all connectors?",
    clear_connectors_body: "This node stops dialing every connector listed here and drops their links. The change \
                            applies to the running network only.",
    mapped_title: "Mapped listeners",
    mapped_hint: "Public addresses other nodes reach this node's listeners at, for example behind NAT or a proxy.",
    mapped_empty: "No mapped listeners.",
    field_mapped_listener: "Mapped listener",
    remove_mapped_title: "Remove mapped listener {url}?",
    remove_mapped_body: "Other nodes stop learning this address. The change applies to the running network only.",
    mapped_clear: "Remove all mapped listeners",
    clear_mapped_title: "Remove all mapped listeners?",
    clear_mapped_body: "Other nodes stop learning these addresses. The change applies to the running network only.",

    forwards_title: "Port forwards",
    forwards_hint: "Connections to an address on this node are sent on to another address through the network.",
    forwards_empty: "No port forwards.",
    field_bind: "Listen on",
    placeholder_bind: "127.0.0.1:2222",
    field_destination: "Send to",
    placeholder_destination: "10.144.144.2:22",
    remove_forward_title: "Remove the {protocol} forward from {bind} to {dst}?",
    remove_forward_body: "Connections made through it end now. The change applies to the running network only.",
    subnets_title: "Subnet proxies",
    subnets_hint: "Subnets this node makes reachable for the whole network. Every node in the network can then reach \
                   what is on them.",
    subnets_empty: "No subnet proxies.",
    field_subnet: "Subnet",
    placeholder_subnet: "192.168.50.0/24",
    field_subnet_mapped: "Announce as (optional)",
    placeholder_subnet_mapped: "10.10.50.0/24",
    remove_subnet_title: "Stop proxying {cidr}?",
    remove_subnet_body: "Other nodes lose the way to it through this node. The change applies to the running network \
                         only.",
    routes_title: "Manual routes",
    routes_hint: "Extra subnets this node sends through the network. They only take effect while the network's config \
                  has manual routes turned on.",
    routes_empty: "No manual routes.",
    routes_off_note: "Manual routing is off in this network's config, so routes added here have no effect. Turn it on in \
                      the config and save it.",
    field_route: "Route",
    placeholder_route: "10.0.0.0/16",
    remove_route_title: "Remove the route {value}?",
    remove_route_body: "Traffic for it stops using this route. The change applies to the running network only.",
    exit_nodes_title: "Exit nodes",
    exit_nodes_hint: "Peers this node sends internet traffic through, by their virtual IP.",
    exit_nodes_empty: "No exit nodes.",
    field_exit_node: "Exit node",
    placeholder_exit_node: "10.144.144.2",
    remove_exit_node_title: "Remove the exit node {value}?",
    remove_exit_node_body: "Internet traffic stops going through it. The change applies to the running network only.",
    node_title: "Node settings",
    node_hint: "What this node announces and whether it relays for others.",
    field_hostname: "Hostname",
    placeholder_hostname: "my-node",
    hostname_current: "Now: {name}",
    hostname_default: "Now: this machine's own hostname",
    field_virtual_ip: "Virtual IP",
    placeholder_virtual_ip: "10.144.144.1/24",
    virtual_ip_current: "Now: {ip}",
    virtual_ip_dhcp: "This network assigns the address by DHCP. A typed address is ignored.",
    virtual_ip_none: "Now: no address",
    relay_off_label: "Don't relay other nodes' data",
    relay_off_help: "Other nodes route around this one. Turn it on for a node that should only carry its own traffic.",
    vpn_title: "VPN portal",
    vpn_hint: "A WireGuard server on this node that lets devices without EasyTier join the network.",
    vpn_none: "This network has no VPN portal.",
    vpn_type: "Type",
    vpn_clients: "{count} connected",
    vpn_no_clients: "No client connected.",
    vpn_config_secret: "The client config holds the client's private key. Anyone who sees it can connect as that \
                        client. It is read only when you ask.",
    vpn_show_config: "Show client config",
    vpn_config_shown: "This is a private key. Don't paste it anywhere you don't trust. Hide it when you are done.",
    vpn_hide_config: "Hide client config",

    whitelist_title: "Open ports",
    whitelist_hint: "Ports on this node that the network may reach. A port is a number, a range is two joined by a \
                     dash.",
    whitelist_empty: "No open ports.",
    field_port: "Port or range",
    placeholder_port: "8080 or 5000-5100",
    remove_port_title: "Close {protocol} port {port}?",
    remove_port_body: "Nodes in the network can no longer reach it. The change applies to the running network only.",
    clear_tcp: "Close all TCP ports",
    clear_udp: "Close all UDP ports",
    clear_ports_title: "Close all {protocol} ports?",
    clear_ports_body: "Nodes in the network can no longer reach any of them. The change applies to the running \
                       network only.",
    acl_title: "Access rules",
    acl_hint: "How often the node's access rules matched. Read-only; they change in the network's config.",
    acl_rules: "Rules",
    acl_rules_empty: "No rules.",
    acl_connections: "Tracked connections",
    acl_connections_empty: "No connection is tracked.",
    acl_counters: "Counters",
    acl_counters_empty: "The core reports no counters.",
    acl_allow: "Allow",
    acl_drop: "Drop",
    acl_count_only: "Count only",
    acl_rule_off: "Off",
    acl_any: "Any",
    acl_unknown: "Unknown",
    conn_new: "New",
    conn_established: "Established",
    conn_related: "Related",
    conn_invalid: "Invalid",

    logs_note: "The log level belongs to the machine's whole core, not to one network, and is not saved: the core \
                returns to its own setting when it restarts.",
    logs_title: "Log level",
    logs_hint: "How much the core on this machine writes to its log.",
    field_log_level: "Log level",
    log_level_help: "Debug and Trace write a lot. Use them while you look for a problem.",
    level_disabled: "Off",
    level_error: "Error",
    level_warning: "Warning",
    level_info: "Info",
    level_debug: "Debug",
    level_trace: "Trace",
};

pub(super) static ZH_CN: ConsoleRuntimeStrings = ConsoleRuntimeStrings {
    note_unsaved: "这里的更改立即生效，但只作用于正在运行的网络。设备重新连接或重启后会丢失，除非保存到网络的配置中。",
    edit_config: "编辑配置",
    dismiss: "忽略",
    refresh_failed: "无法再次读取，显示的是上次的结果。",
    add: "添加",
    apply: "应用",
    remove: "移除",
    clear: "全部移除",
    applying: "正在应用…",
    proto_tcp: "TCP",
    proto_udp: "UDP",
    col_url: "地址",
    col_status: "连接",
    col_protocol: "协议",
    col_bind: "监听地址",
    col_destination: "转发到",
    col_subnet: "子网",
    col_announced_as: "对外显示为",
    col_route: "路由",
    col_address: "地址",
    col_ports: "端口",
    col_rule: "规则",
    col_action: "动作",
    col_packets: "数据包",
    col_bytes: "流量",
    col_source: "来源",
    col_state: "状态",
    col_counter: "计数器",
    col_value: "值",

    connectors_title: "连接器",
    connectors_hint: "此节点主动拨号并保持连接的对等节点。",
    connectors_empty: "没有连接器。此节点只会连接网络配置中列出的对等节点。",
    field_connector: "连接器",
    placeholder_url: "tcp://203.0.113.5:11010",
    status_connected: "已连接",
    status_connecting: "连接中",
    status_disconnected: "已断开",
    status_unknown: "未知",
    remove_connector_title: "移除连接器 {url}？",
    remove_connector_body: "此节点将不再拨号并断开该连接。更改只作用于正在运行的网络。",
    connectors_clear: "移除全部连接器",
    clear_connectors_title: "移除全部连接器？",
    clear_connectors_body: "此节点将不再拨号这里列出的所有连接器并断开它们。更改只作用于正在运行的网络。",
    mapped_title: "映射监听器",
    mapped_hint: "其他节点访问此节点监听器所用的公网地址，例如位于 NAT 或代理之后时。",
    mapped_empty: "没有映射监听器。",
    field_mapped_listener: "映射监听器",
    remove_mapped_title: "移除映射监听器 {url}？",
    remove_mapped_body: "其他节点将不再获知该地址。更改只作用于正在运行的网络。",
    mapped_clear: "移除全部映射监听器",
    clear_mapped_title: "移除全部映射监听器？",
    clear_mapped_body: "其他节点将不再获知这些地址。更改只作用于正在运行的网络。",

    forwards_title: "端口转发",
    forwards_hint: "发往此节点某个地址的连接，会通过网络转发到另一个地址。",
    forwards_empty: "没有端口转发。",
    field_bind: "监听地址",
    placeholder_bind: "127.0.0.1:2222",
    field_destination: "转发到",
    placeholder_destination: "10.144.144.2:22",
    remove_forward_title: "移除从 {bind} 到 {dst} 的 {protocol} 转发？",
    remove_forward_body: "经由它建立的连接会立即断开。更改只作用于正在运行的网络。",
    subnets_title: "子网代理",
    subnets_hint: "由此节点让整个网络都能访问的子网。网络中的每个节点都能访问其中的设备。",
    subnets_empty: "没有子网代理。",
    field_subnet: "子网",
    placeholder_subnet: "192.168.50.0/24",
    field_subnet_mapped: "对外显示为（可选）",
    placeholder_subnet_mapped: "10.10.50.0/24",
    remove_subnet_title: "停止代理 {cidr}？",
    remove_subnet_body: "其他节点将无法再经由此节点访问它。更改只作用于正在运行的网络。",
    routes_title: "手动路由",
    routes_hint: "此节点额外经网络发送的子网。只有网络配置开启了手动路由时才会生效。",
    routes_empty: "没有手动路由。",
    routes_off_note: "此网络的配置没有开启手动路由，在这里添加的路由不会生效。请在配置中开启并保存。",
    field_route: "路由",
    placeholder_route: "10.0.0.0/16",
    remove_route_title: "移除路由 {value}？",
    remove_route_body: "发往它的流量将不再使用这条路由。更改只作用于正在运行的网络。",
    exit_nodes_title: "出口节点",
    exit_nodes_hint: "此节点把互联网流量发往的对等节点，以其虚拟 IP 表示。",
    exit_nodes_empty: "没有出口节点。",
    field_exit_node: "出口节点",
    placeholder_exit_node: "10.144.144.2",
    remove_exit_node_title: "移除出口节点 {value}？",
    remove_exit_node_body: "互联网流量将不再经过它。更改只作用于正在运行的网络。",
    node_title: "节点设置",
    node_hint: "此节点对外宣告的信息，以及是否为其他节点中继。",
    field_hostname: "主机名",
    placeholder_hostname: "my-node",
    hostname_current: "当前：{name}",
    hostname_default: "当前：此设备自己的主机名",
    field_virtual_ip: "虚拟 IP",
    placeholder_virtual_ip: "10.144.144.1/24",
    virtual_ip_current: "当前：{ip}",
    virtual_ip_dhcp: "此网络通过 DHCP 分配地址，填写的地址会被忽略。",
    virtual_ip_none: "当前：没有地址",
    relay_off_label: "不为其他节点中继数据",
    relay_off_help: "其他节点会绕开此节点。适用于只承载自己流量的节点。",
    vpn_title: "VPN 门户",
    vpn_hint: "此节点上的 WireGuard 服务，让没有安装 EasyTier 的设备也能加入网络。",
    vpn_none: "此网络没有 VPN 门户。",
    vpn_type: "类型",
    vpn_clients: "{count} 个客户端已连接",
    vpn_no_clients: "没有客户端连接。",
    vpn_config_secret: "客户端配置包含客户端的私钥，任何看到它的人都能以该客户端的身份连接。只有你点击后才会读取。",
    vpn_show_config: "显示客户端配置",
    vpn_config_shown: "这是一把私钥，不要粘贴到不可信的地方。用完请隐藏。",
    vpn_hide_config: "隐藏客户端配置",

    whitelist_title: "开放端口",
    whitelist_hint: "网络可以访问的此节点端口。单个端口写数字，范围用短横线连接两个数字。",
    whitelist_empty: "没有开放端口。",
    field_port: "端口或范围",
    placeholder_port: "8080 或 5000-5100",
    remove_port_title: "关闭 {protocol} 端口 {port}？",
    remove_port_body: "网络中的节点将无法再访问它。更改只作用于正在运行的网络。",
    clear_tcp: "关闭全部 TCP 端口",
    clear_udp: "关闭全部 UDP 端口",
    clear_ports_title: "关闭全部 {protocol} 端口？",
    clear_ports_body: "网络中的节点将无法再访问其中任何一个。更改只作用于正在运行的网络。",
    acl_title: "访问规则",
    acl_hint: "此节点的访问规则命中的次数。只读，需在网络配置中修改。",
    acl_rules: "规则",
    acl_rules_empty: "没有规则。",
    acl_connections: "被跟踪的连接",
    acl_connections_empty: "没有被跟踪的连接。",
    acl_counters: "计数器",
    acl_counters_empty: "内核没有报告计数器。",
    acl_allow: "放行",
    acl_drop: "丢弃",
    acl_count_only: "仅计数",
    acl_rule_off: "已关闭",
    acl_any: "任意",
    acl_unknown: "未知",
    conn_new: "新建",
    conn_established: "已建立",
    conn_related: "关联",
    conn_invalid: "无效",

    logs_note: "日志级别属于此设备的整个内核，而不是某一个网络，且不会保存：内核重启后会恢复它自己的设置。",
    logs_title: "日志级别",
    logs_hint: "此设备上的内核向日志写入多少内容。",
    field_log_level: "日志级别",
    log_level_help: "调试和跟踪会写入大量内容，排查问题时再使用。",
    level_disabled: "关闭",
    level_error: "错误",
    level_warning: "警告",
    level_info: "信息",
    level_debug: "调试",
    level_trace: "跟踪",
};
