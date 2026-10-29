//! Node role in the web build: the Networks and Peers pages manage one machine of the server that served the page.

/// Strings of this area in one language.
#[derive(Debug)]
pub struct WebStrings {
    /// The machine picker's label.
    pub picker_label: &'static str,
    /// The picker's entry that stands for no choice.
    pub picker_choose: &'static str,
    /// No machine is chosen yet: title.
    pub pick_title: &'static str,
    /// No machine is chosen yet: what the pages do with the choice.
    pub pick_body: &'static str,
    /// No machine is enrolled: title.
    pub none_title: &'static str,
    /// No machine is enrolled: how a node enrolls.
    pub none_body: &'static str,
    /// No machine is enrolled: the way to the Machines page.
    pub none_action: &'static str,
    /// The remembered machine is no longer listed: title.
    pub gone_title: &'static str,
    /// The remembered machine is no longer listed: what to do.
    pub gone_body: &'static str,
    /// The chosen machine has not reported lately.
    pub offline: &'static str,
}

pub(in crate::i18n) static EN: WebStrings = WebStrings {
    picker_label: "Machine",
    picker_choose: "Choose a machine",
    pick_title: "Choose a machine",
    pick_body: "In the browser, networks and peers belong to a machine enrolled on this server. Choose the one to \
                manage.",
    none_title: "No machine is enrolled yet",
    none_body: "A machine appears here after its EasyTier node starts with --config-server pointing at this \
                server and an account on it. Enroll one, then come back.",
    none_action: "Open Machines",
    gone_title: "That machine is no longer on this server",
    gone_body: "The machine you managed here is not listed any more. Choose another one.",
    offline: "This machine has not reported lately. It may be offline, so changes might not reach it.",
};

pub(in crate::i18n) static ZH_CN: WebStrings = WebStrings {
    picker_label: "机器",
    picker_choose: "选择机器",
    pick_title: "选择一台机器",
    pick_body: "在浏览器中，网络和对等节点属于已加入本服务器的机器。请选择要管理的那一台。",
    none_title: "还没有机器加入",
    none_body: "机器的 EasyTier 节点以 --config-server 指向本服务器并使用其上的账号启动后，就会出现在这里。\
                请先加入一台，再回来。",
    none_action: "打开机器页",
    gone_title: "该机器已不在本服务器上",
    gone_body: "你之前管理的机器已不在列表中。请选择另一台。",
    offline: "这台机器近期没有上报，可能已离线，修改不一定能送达。",
};
