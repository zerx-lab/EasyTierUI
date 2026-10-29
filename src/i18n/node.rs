//! Node role: networks (profiles), the network dialog, import / export, peers, logs, managed enrollment.
//!
//! One field per string; `EN` and `ZH_CN` must both set every field (the struct makes a missing one a compile
//! error). Templates use `{name}` placeholders filled by [`super::fill`]. The strings are grouped by screen, each
//! group in its own file.

mod detail;
mod flow;
mod import;
mod list;
mod peers;
mod shared;
mod web;

pub use detail::DetailStrings;
pub use flow::FlowStrings;
pub use import::ImportStrings;
pub use list::ListStrings;
pub use peers::PeersStrings;
pub use shared::SharedStrings;
pub use web::WebStrings;

/// Strings of this area in one language.
#[derive(Debug)]
pub struct NodeStrings {
    /// The text of the macOS administrator prompt that installs or controls a network's system service.
    pub elevation_prompt: &'static str,
    /// Words several screens share: field names, problems, failures, exposures.
    pub shared: &'static SharedStrings,
    /// The Networks page.
    pub list: &'static ListStrings,
    /// The create and join flows.
    pub flow: &'static FlowStrings,
    /// The network dialog.
    pub detail: &'static DetailStrings,
    /// Import and enrollment.
    pub import: &'static ImportStrings,
    /// The Peers page.
    pub peers: &'static PeersStrings,
    /// The web build's pages, which manage a machine of the serving server.
    pub web: &'static WebStrings,
}

pub(super) static EN: NodeStrings = NodeStrings {
    elevation_prompt: "EasyTierUI wants to set up or control the system service of an EasyTier network.",
    shared: &shared::EN,
    list: &list::EN,
    flow: &flow::EN,
    detail: &detail::EN,
    import: &import::EN,
    peers: &peers::EN,
    web: &web::EN,
};

pub(super) static ZH_CN: NodeStrings = NodeStrings {
    elevation_prompt: "EasyTierUI 需要安装或控制 EasyTier 网络的系统服务。",
    shared: &shared::ZH_CN,
    list: &list::ZH_CN,
    flow: &flow::ZH_CN,
    detail: &detail::ZH_CN,
    import: &import::ZH_CN,
    peers: &peers::ZH_CN,
    web: &web::ZH_CN,
};
