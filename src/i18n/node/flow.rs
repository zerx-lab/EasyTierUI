//! Node role: the create and join flows and the confirmation before a first TUN start.

/// Strings of this area in one language.
#[derive(Debug)]
pub struct FlowStrings {
    /// Create flow title.
    pub create_title: &'static str,
    /// Join flow title.
    pub join_title: &'static str,
    /// Progress: `{step}` of `{total}`.
    pub step_counter: &'static str,
    /// Name screen: the question.
    pub name_prompt: &'static str,
    /// Name screen: what the name is.
    pub name_helper: &'static str,
    /// Name screen: example.
    pub name_placeholder: &'static str,
    /// Network screen of a created network: the question.
    pub network_prompt_create: &'static str,
    /// Network screen of a joined network: the question.
    pub network_prompt_join: &'static str,
    /// Network name: what it is.
    pub network_name_helper: &'static str,
    /// Network name: example.
    pub network_name_placeholder: &'static str,
    /// Secret: who can join with it.
    pub secret_warning: &'static str,
    /// Secret: where a created secret is kept (shown when it is generated).
    pub secret_generated_note: &'static str,
    /// Peer screen: the question.
    pub peer_prompt: &'static str,
    /// Peer address: label.
    pub peer_label: &'static str,
    /// Peer address: example with the usual port.
    pub peer_placeholder: &'static str,
    /// Peer address: what a shared node is.
    pub peer_helper: &'static str,
    /// Label before the address the typed text means.
    pub peer_connects_to: &'static str,
    /// Summary screen: the heading.
    pub summary_prompt: &'static str,
    /// Summary: the secret's row.
    pub summary_secret: &'static str,
    /// Summary: where the secret is kept.
    pub summary_secret_kept: &'static str,
    /// Summary: the virtual IP's row.
    pub summary_ip: &'static str,
    /// Summary: how the virtual IP is chosen.
    pub summary_ip_value: &'static str,
    /// Summary: the incoming connections' row.
    pub summary_access: &'static str,
    /// Summary: that there are none.
    pub summary_access_value: &'static str,
    /// Summary: the administrator prompt on desktops.
    pub summary_admin: &'static str,
    /// Why a phone's network runs without TUN.
    pub no_tun_note: &'static str,
    /// Action: save and start.
    pub action_save_start: &'static str,
    /// Action: save without starting.
    pub action_save_only: &'static str,
    /// Busy: saving.
    pub saving: &'static str,
    /// Confirmation: title.
    pub tun_title: &'static str,
    /// Confirmation: the administrator prompt.
    pub tun_admin: &'static str,
    /// Confirmation: it keeps running after the app quits.
    pub tun_service: &'static str,
    /// Confirmation: the alternative.
    pub tun_alternative: &'static str,
    /// Confirmation: go on.
    pub tun_continue: &'static str,
    /// Confirmation: run without TUN.
    pub tun_no_tun: &'static str,
}

pub(super) static EN: FlowStrings = FlowStrings {
    create_title: "Create a network",
    join_title: "Join a network",
    step_counter: "Step {step} of {total}",
    name_prompt: "What do you want to call this network?",
    name_helper: "Only you see this name, on this device.",
    name_placeholder: "Home",
    network_prompt_create: "Choose the network name and secret",
    network_prompt_join: "Enter the network name and secret you were given",
    network_name_helper: "Everyone in the network uses the same name and the same secret.",
    network_name_placeholder: "home-lab",
    secret_warning: "Anyone who has the network name and the secret can join. Share them only with people you trust.",
    secret_generated_note: "A random secret was made for you. Copy it now to give it to the others; you can also copy it later from the network's settings.",
    peer_prompt: "How does this device find the network?",
    peer_label: "Shared node or peer address",
    peer_placeholder: "node.example.net:11010",
    peer_helper: "Enter the address of a shared node or of any peer in the network. A shared node sits on a public address and relays traffic between peers that can't connect directly. Without a protocol in front, tcp is used.",
    peer_connects_to: "Connects to",
    summary_prompt: "Check and save",
    summary_secret: "Secret",
    summary_secret_kept: "Kept in the system keychain",
    summary_ip: "Virtual IP",
    summary_ip_value: "Assigned automatically (DHCP)",
    summary_access: "Incoming connections",
    summary_access_value: "None. Others reach this node through a shared node, or through a listener you add in the network's settings.",
    summary_admin: "Starting asks for administrator approval, because the network creates a virtual network interface (TUN).",
    no_tun_note: "A virtual network interface (TUN) isn't available in this build of the app on phones, so this network runs without one. Subnet sharing, the SOCKS5 proxy and port forwarding work; other apps can't send traffic to the virtual IP.",
    action_save_start: "Save and start",
    action_save_only: "Save without starting",
    saving: "Saving…",
    tun_title: "Start with a network interface?",
    tun_admin: "The virtual network interface (TUN) needs administrator rights. Your system asks you to approve when this network starts and again when it stops.",
    tun_service: "The network runs as a system service, so it keeps running after you quit this app, until you stop it here.",
    tun_alternative: "Without TUN the network runs as part of this app and needs no administrator rights, but only while the app is open, and other apps can't send traffic to the virtual IP.",
    tun_continue: "Continue",
    tun_no_tun: "Run without TUN",
};

pub(super) static ZH_CN: FlowStrings = FlowStrings {
    create_title: "创建网络",
    join_title: "加入网络",
    step_counter: "第 {step} 步，共 {total} 步",
    name_prompt: "给这个网络起个名字吧",
    name_helper: "这个名字只在这台设备上显示。",
    name_placeholder: "家里",
    network_prompt_create: "设置网络名称和密钥",
    network_prompt_join: "填写你拿到的网络名称和密钥",
    network_name_helper: "同一个网络里的所有节点使用相同的名称和密钥。",
    network_name_placeholder: "home-lab",
    secret_warning: "任何知道网络名称和密钥的人都能加入，请只分享给信任的人。",
    secret_generated_note: "已为你生成随机密钥。现在复制下来发给其他人；之后也可以在网络设置中复制。",
    peer_prompt: "这台设备如何找到这个网络？",
    peer_label: "共享节点或对等节点地址",
    peer_placeholder: "node.example.net:11010",
    peer_helper: "填写共享节点或网络中任意一个对等节点的地址。共享节点位于公网地址，会为无法直连的节点中继流量。不写协议时使用 tcp。",
    peer_connects_to: "将连接到",
    summary_prompt: "确认并保存",
    summary_secret: "密钥",
    summary_secret_kept: "保存在系统钥匙串中",
    summary_ip: "虚拟 IP",
    summary_ip_value: "自动分配（DHCP）",
    summary_access: "入站连接",
    summary_access_value: "无。其他人通过共享节点连到这个节点，或通过你在网络设置中添加的监听地址。",
    summary_admin: "启动时需要管理员授权，因为网络会创建虚拟网卡（TUN）。",
    no_tun_note: "本应用在手机上暂不支持虚拟网卡（TUN），因此这个网络不使用虚拟网卡运行。子网共享、SOCKS5 代理和端口转发可用；其他应用不能把流量发往虚拟 IP。",
    action_save_start: "保存并启动",
    action_save_only: "仅保存",
    saving: "正在保存…",
    tun_title: "使用虚拟网卡启动？",
    tun_admin: "虚拟网卡（TUN）需要管理员权限。网络启动和停止时，系统都会请你授权。",
    tun_service: "网络会以系统服务运行，退出本应用后仍会继续运行，直到你在这里停止它。",
    tun_alternative: "不使用 TUN 时，网络随本应用运行，不需要管理员权限，但只在应用打开期间有效，其他应用也不能把流量发往虚拟 IP。",
    tun_continue: "继续",
    tun_no_tun: "不使用 TUN 运行",
};
