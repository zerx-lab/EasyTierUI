//! English.

use super::Strings;

pub(super) static STRINGS: Strings = Strings {
    app_name: "EasyTierUI",
    back: "Back",
    sidebar_hide: "Hide sidebar",
    sidebar_show: "Show sidebar",

    role_node: "Node",
    role_console: "Console",
    role_host: "Control plane",

    page_networks: "Networks",
    page_peers: "Peers",
    page_machines: "Machines",
    page_server_networks: "Networks",
    page_credentials: "Credentials",
    page_host_status: "Status",
    page_hardening: "Hardening",
    page_deployment: "Deployment",
    page_settings: "Settings",

    target_switcher: "Switch target",
    target_this_device: "This device",
    target_this_server: "This server",
    status_stopped: "Stopped",
    status_not_connected: "Not connected",

    notice_browser_node: "This browser can't join a network.",
    notice_browser_node_action: "Manage this server's nodes",
    notice_browser_host: "This browser can't run EasyTier's programs. This server's control plane changes through \
                          its deployment files.",
    notice_browser_host_action: "Open deployment",
    notice_mobile_host: "This device can't host a control plane.",
    notice_mobile_host_action: "Deploy a control plane",

    networks_empty_title: "No networks on this device",
    networks_empty_body: "Networks you create or join appear here, with their state and virtual IP.",
    peers_empty_title: "No network is running",
    peers_empty_body: "Peers appear here once a network on this device is running.",
    console_no_server_title: "No server selected",
    console_no_server_body: "The console manages an easytier-web server's machines, networks and credentials. Pick \
                             a server in the target switcher.",
    console_not_connected_title: "Not connected to this server",
    console_not_connected_body: "Its machines, networks and credentials appear here once you sign in.",
    host_stopped_title: "No control plane runs on this device",
    host_stopped_body: "Hosting easytier-web here is opt-in. Before it starts, its default accounts are replaced and \
                        its API listens on loopback only.",
    hardening_idle_title: "Nothing to harden yet",
    hardening_idle_body: "Once a control plane runs here, this page checks its default passwords, registration and \
                          exposed ports.",
    deployment_empty_title: "No deployment files yet",
    deployment_empty_body: "Docker Compose, systemd and reverse-proxy files for a server are generated here with \
                            safe defaults. Nothing is applied to a server from this app.",

    settings_language: "Language",
    language_system: "System",
    language_zh_cn: "简体中文",
    language_en: "English",
    settings_about: "About",
    settings_version: "Version",
};
