//! Every user-facing string, in zh-CN and en.
//!
//! [`Strings`] has one field per string, so a language without a string does not compile. Write each language
//! natively; EasyTier's terms stay consistent (network 网络, node 节点, peer 对等节点, core 内核, control plane 控制平面).

mod common;
mod config;
mod console;
mod console_inspect;
mod console_runtime;
mod desktop;
mod en;
mod host;
pub mod node;
mod versions;
mod web_host;
mod zh_cn;

pub use common::CommonStrings;
pub use config::ConfigStrings;
pub use console::ConsoleStrings;
pub use console_inspect::ConsoleInspectStrings;
pub use console_runtime::ConsoleRuntimeStrings;
pub use desktop::DesktopStrings;
pub use host::HostStrings;
pub use node::NodeStrings;
pub use versions::VersionStrings;
pub use web_host::WebHostStrings;

/// A language the UI is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// 简体中文.
    ZhCn,
    /// English.
    En,
}

impl Language {
    /// The UI language for a BCP 47 tag (`zh-Hans-CN`, `zh-TW`, `en-US`): Chinese for every `zh` tag, English
    /// otherwise.
    pub fn from_tag(tag: &str) -> Self {
        let primary = tag.split(['-', '_']).next().unwrap_or_default();
        if primary.eq_ignore_ascii_case("zh") {
            Self::ZhCn
        } else {
            Self::En
        }
    }

    /// Its strings.
    pub fn strings(self) -> &'static Strings {
        match self {
            Self::ZhCn => &zh_cn::STRINGS,
            Self::En => &en::STRINGS,
        }
    }
}

/// The system's preferred UI language tag. NSH's `locale()` is the region format (`NSLocale.currentLocale`), which
/// a bundle declaring only an `en` localization reports as `en_CN` on a Chinese system; the preferred-language list
/// is what the user chose as the system language. `locale()` is still read so a system change re-renders.
pub fn system_language_tag() -> String {
    let region_format = nsh_core::prelude::locale();
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(tag) = sys_locale::get_locale() {
        return tag;
    }
    region_format.as_str().to_owned()
}

/// The language the user chose in Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LanguageChoice {
    /// The system's language.
    #[default]
    System,
    /// One language, whatever the system's.
    Fixed(Language),
}

impl LanguageChoice {
    /// Every choice, in the order Settings lists them.
    pub const ALL: [LanguageChoice; 3] = [
        LanguageChoice::System,
        LanguageChoice::Fixed(Language::ZhCn),
        LanguageChoice::Fixed(Language::En),
    ];

    /// The language to show, given the system's locale tag.
    pub fn resolve(self, system_tag: &str) -> Language {
        match self {
            Self::System => Language::from_tag(system_tag),
            Self::Fixed(language) => language,
        }
    }

    /// How it is kept in storage.
    pub fn as_key(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Fixed(Language::ZhCn) => "zh-CN",
            Self::Fixed(Language::En) => "en",
        }
    }

    /// The choice kept as `key`; `None` for a value this version does not know.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|choice| choice.as_key() == key)
    }
}

/// Picks one string of [`Strings`]: how callers name a string independently of the language.
pub type Pick = fn(&'static Strings) -> &'static str;

/// The UI's strings in one language.
#[derive(Debug)]
pub struct Strings {
    /// The product name.
    pub app_name: &'static str,
    /// Back to the previous page.
    pub back: &'static str,
    /// The sidebar toggle while the sidebar shows (for screen readers and tests).
    pub sidebar_hide: &'static str,
    /// The sidebar toggle while the sidebar is hidden.
    pub sidebar_show: &'static str,

    /// Role: this device in the mesh.
    pub role_node: &'static str,
    /// Role: managing an easytier-web server.
    pub role_console: &'static str,
    /// Role: hosting or deploying easytier-web.
    pub role_host: &'static str,
    /// What the node role shows, in the work-mode settings.
    pub role_node_body: &'static str,
    /// What the console role shows.
    pub role_console_body: &'static str,
    /// What the host role shows.
    pub role_host_body: &'static str,

    /// First run: the work-mode question.
    pub mode_title: &'static str,
    /// What the choice decides, and that it can change later.
    pub mode_intro: &'static str,
    /// Work mode: a device that joins networks.
    pub mode_client: &'static str,
    /// What the client mode shows.
    pub mode_client_body: &'static str,
    /// Work mode: everything, for administrators.
    pub mode_control: &'static str,
    /// What the control mode shows.
    pub mode_control_body: &'static str,
    /// Work mode: the roles the user picks.
    pub mode_custom: &'static str,
    /// What the custom mode shows, and that one role stays on.
    pub mode_custom_body: &'static str,
    /// First run: confirms the choice.
    pub mode_continue: &'static str,
    /// Settings: what hiding a role does (nothing stops).
    pub mode_hidden_note: &'static str,
    /// Sidebar: the node is hidden, and this device's networks run.
    pub hidden_node_running: &'static str,
    /// Sidebar: the host is hidden, and the hosted control plane runs.
    pub hidden_host_running: &'static str,

    /// Page: the node's networks.
    pub page_networks: &'static str,
    /// Page: the node's peers.
    pub page_peers: &'static str,
    /// Page: a server's machines.
    pub page_machines: &'static str,
    /// Page: a server's networks.
    pub page_server_networks: &'static str,
    /// Page: a server's credentials.
    pub page_credentials: &'static str,
    /// Page: the hosted control plane's state.
    pub page_host_status: &'static str,
    /// Page: first-run hardening of the hosted control plane.
    pub page_hardening: &'static str,
    /// Page: server deployment files.
    pub page_deployment: &'static str,
    /// Page: the app's settings.
    pub page_settings: &'static str,

    /// The target switcher, for screen readers.
    pub target_switcher: &'static str,
    /// The name of this device as a target.
    pub target_this_device: &'static str,
    /// The name of the server that served the web build.
    pub target_this_server: &'static str,
    /// Target state: reachable, nothing running.
    pub status_stopped: &'static str,
    /// Target state: not connected yet.
    pub status_not_connected: &'static str,
    /// The name of the easytier-web this desktop hosts, as a target.
    pub target_hosted: &'static str,
    /// The switcher's action that adds an easytier-web server.
    pub target_add_server: &'static str,
    /// The switcher's action that opens hosting a control plane on this desktop.
    pub target_host_here: &'static str,
    /// Kind chip: the target is this device.
    pub kind_this_device: &'static str,
    /// Kind chip: the target is an easytier-web server.
    pub kind_server: &'static str,
    /// Kind chip: the target is the control plane this desktop hosts.
    pub kind_hosted: &'static str,
    /// Kind chip of a drawer: a network.
    pub kind_network: &'static str,
    /// Kind chip of a drawer: a machine reported by a server.
    pub kind_machine: &'static str,
    /// Kind chip of a drawer: a peer of a running network.
    pub kind_peer: &'static str,

    /// Capability notice: a phone cannot host a control plane.
    pub notice_mobile_host: &'static str,
    /// Its remote alternative.
    pub notice_mobile_host_action: &'static str,

    /// Networks page, no profiles.
    pub networks_empty_title: &'static str,
    /// What will show there.
    pub networks_empty_body: &'static str,
    /// Peers page, no network running.
    pub peers_empty_title: &'static str,
    /// What will show there.
    pub peers_empty_body: &'static str,
    /// Console pages with this device as the target.
    pub console_no_server_title: &'static str,
    /// What the console does and where a server is chosen.
    pub console_no_server_body: &'static str,
    /// Console pages with a server the app has not connected to.
    pub console_not_connected_title: &'static str,
    /// What connecting shows.
    pub console_not_connected_body: &'static str,
    /// Host status page on a desktop, nothing hosted.
    pub host_stopped_title: &'static str,
    /// How hosting starts, and its defaults.
    pub host_stopped_body: &'static str,
    /// Hardening page, nothing hosted.
    pub hardening_idle_title: &'static str,
    /// What it will check.
    pub hardening_idle_body: &'static str,
    /// Deployment page, nothing generated.
    pub deployment_empty_title: &'static str,
    /// What it generates, and that nothing is applied.
    pub deployment_empty_body: &'static str,

    /// Settings: the work-mode section.
    pub settings_mode: &'static str,
    /// Settings: the language section.
    pub settings_language: &'static str,
    /// Language choice: follow the system.
    pub language_system: &'static str,
    /// Language choice: Chinese (always written in Chinese).
    pub language_zh_cn: &'static str,
    /// Language choice: English (always written in English).
    pub language_en: &'static str,
    /// Settings: the about section.
    pub settings_about: &'static str,
    /// The app's version, a label before the number.
    pub settings_version: &'static str,

    /// Words every area shares.
    pub common: &'static CommonStrings,
    /// The settings both network editors share (behavior switches, VPN portal, access control).
    pub config: &'static ConfigStrings,
    /// The node role's strings.
    pub node: &'static NodeStrings,
    /// The console role's strings.
    pub console: &'static ConsoleStrings,
    /// The console machine drawer's tabs that read a running network (node facts, routes, relay, stats).
    pub console_inspect: &'static ConsoleInspectStrings,
    /// The console machine drawer's tabs that change a running network (connections, forwarding, access, logs).
    pub console_runtime: &'static ConsoleRuntimeStrings,
    /// The control-plane host role's strings (status, hardening, deployment).
    pub host: &'static HostStrings,
    /// The desktop app's own behavior (tray, login item, quiet start) and its tray menu.
    pub desktop: &'static DesktopStrings,
    /// EasyTier versions on this desktop.
    pub versions: &'static VersionStrings,
    /// The control-plane pages in the web build (the serving easytier-web, through its REST API).
    pub web_host: &'static WebHostStrings,
}

/// `template` with each `{name}` replaced by its value from `args`; unknown placeholders stay as written so a
/// missing argument shows instead of vanishing.
pub fn fill(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        let (before, from_brace) = rest.split_at_checked(start).unwrap_or((rest, ""));
        out.push_str(before);
        let after_brace = from_brace.get(1..).unwrap_or_default();
        let Some(end) = after_brace.find('}') else {
            out.push_str(from_brace);
            return out;
        };
        let name = after_brace.get(..end).unwrap_or_default();
        match args.iter().find(|(key, _)| *key == name) {
            Some((_, value)) => out.push_str(value),
            None => {
                out.push('{');
                out.push_str(name);
                out.push('}');
            }
        }
        rest = after_brace.get(end.saturating_add(1)..).unwrap_or_default();
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_chinese_tag_picks_chinese() {
        for tag in ["zh", "zh-CN", "zh-Hans-CN", "zh-TW", "zh_HK", "ZH-hant"] {
            assert_eq!(Language::from_tag(tag), Language::ZhCn, "{tag}");
        }
    }

    #[test]
    fn other_tags_fall_back_to_english() {
        for tag in ["en-US", "de-DE", "ja", "", "zhx"] {
            assert_eq!(Language::from_tag(tag), Language::En, "{tag}");
        }
    }

    #[test]
    fn a_fixed_choice_wins_over_the_system() {
        assert_eq!(
            LanguageChoice::Fixed(Language::En).resolve("zh-CN"),
            Language::En
        );
        assert_eq!(LanguageChoice::System.resolve("zh-CN"), Language::ZhCn);
    }

    #[test]
    fn stored_choices_round_trip_and_unknown_ones_are_rejected() {
        for choice in LanguageChoice::ALL {
            assert_eq!(LanguageChoice::from_key(choice.as_key()), Some(choice));
        }
        assert_eq!(LanguageChoice::from_key("fr"), None);
    }

    #[test]
    fn templates_fill_known_placeholders_and_keep_unknown_ones() {
        assert_eq!(
            fill("Delete profile “{name}”?", &[("name", "家")]),
            "Delete profile “家”?"
        );
        assert_eq!(fill("{a} and {b}", &[("a", "1")]), "1 and {b}");
        assert_eq!(fill("no braces", &[]), "no braces");
        assert_eq!(fill("open {brace", &[]), "open {brace");
    }

    #[test]
    fn both_languages_fill_every_shared_word() {
        for language in [Language::ZhCn, Language::En] {
            let strings = language.strings();
            assert!(!strings.common.cancel.is_empty());
            assert!(strings.common.updated_ago.contains("{age}"));
        }
    }
}
