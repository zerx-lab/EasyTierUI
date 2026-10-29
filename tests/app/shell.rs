//! The shell: the desktop title bar, both layouts, capability states and the language.

use nsh_core::{
    Command,
    desktop::{DesktopCommand, TitleBarStyle},
};
use nsh_engine::{Os, Points};

use crate::Device;

#[test]
fn desktops_hide_the_system_title_bar_and_draw_their_own() {
    let app = Device::desktop(Os::MacOs, 1200.0).start().unwrap();
    let hidden = Command::Desktop(Box::new(DesktopCommand::SetTitleBar {
        style: TitleBarStyle::Hidden,
        height: Some(Points(44.0)),
    }));
    assert!(app.screen().sent(&hidden));
}

#[test]
fn the_web_build_keeps_the_browser_chrome() {
    let app = Device::desktop(Os::Web, 1200.0).start().unwrap();
    let asked = app
        .screen()
        .commands()
        .iter()
        .any(|(_, command)| matches!(command, Command::Desktop(c) if matches!(**c, DesktopCommand::SetTitleBar { .. })));
    assert!(!asked);
}

#[test]
fn the_sidebar_switches_pages_and_marks_the_selected_one() {
    let mut app = Device::desktop(Os::Windows, 1200.0).start().unwrap();
    app.wait_for("No networks on this device").unwrap();
    app.press("id:page-peers").unwrap();
    app.wait_for("No network is running").unwrap();
    assert!(app.screen().selected("id:page-peers").unwrap());
    assert!(!app.screen().selected("id:page-networks").unwrap());
    app.press("id:page-host-status").unwrap();
    app.wait_for("No control plane runs on this device")
        .unwrap();
}

#[test]
fn a_browser_cannot_be_a_node_and_offers_the_servers_nodes() {
    let mut app = Device::desktop(Os::Web, 1200.0).start().unwrap();
    // The served page's own server is the target: it opens on its console, which asks to sign in.
    app.wait_for("Sign in to manage this server").unwrap();
    app.press("id:page-networks").unwrap();
    app.wait_for("This browser can't join a network.").unwrap();
    app.press("Manage this server's nodes").unwrap();
    app.wait_for("Sign in to manage this server").unwrap();
    assert!(app.screen().selected("id:page-machines").unwrap());
}

#[test]
fn a_browser_changes_its_control_plane_through_deployment() {
    let mut app = Device::desktop(Os::Web, 1200.0).start().unwrap();
    app.press("id:page-hardening").unwrap();
    app.wait_for("Open deployment").unwrap();
    app.press("Open deployment").unwrap();
    app.wait_for("No deployment files yet").unwrap();
}

#[test]
fn a_phone_cannot_host_and_offers_deployment() {
    let mut app = Device::phone(Os::Ios).start().unwrap();
    app.press("id:page-host-status").unwrap();
    app.wait_for("This device can't host a control plane.")
        .unwrap();
    app.press("Deploy a control plane").unwrap();
    app.wait_for("No deployment files yet").unwrap();
}

#[test]
fn a_narrow_desktop_window_stacks_pages_and_goes_back() {
    let mut app = Device::desktop(Os::Linux, 500.0).start().unwrap();
    app.press("id:page-peers").unwrap();
    app.wait_for("No network is running").unwrap();
    app.press("Back").unwrap();
    app.wait_for_gone("No network is running", std::time::Duration::from_secs(2))
        .unwrap();
    assert!(app.shows("Peers"));
}

#[test]
fn a_chinese_system_gets_chinese_until_the_user_picks_english() {
    let device = Device::desktop(Os::MacOs, 1200.0).locale("zh-Hans-CN");
    let mut app = device.start().unwrap();
    app.wait_for("此设备上还没有网络").unwrap();
    app.press("id:page-settings").unwrap();
    app.press("English").unwrap();
    app.wait_for("Settings").unwrap();
    drop(app);

    // The choice outlives the run.
    let mut again = device.start().unwrap();
    again.wait_for("No networks on this device").unwrap();
}

#[test]
fn the_sidebar_hides_and_stays_hidden_after_a_restart() {
    let device = Device::desktop(Os::MacOs, 1200.0);
    let mut app = device.start().unwrap();
    app.press("label:Hide sidebar").unwrap();
    app.wait_for("label:Show sidebar").unwrap();
    drop(app);

    let mut again = device.start().unwrap();
    again.wait_for("label:Show sidebar").unwrap();
    again.press("label:Show sidebar").unwrap();
    again.wait_for("label:Hide sidebar").unwrap();
}
