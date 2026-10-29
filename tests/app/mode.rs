//! The work mode: the first run's question, what each mode lists, and changing it in Settings.

use std::time::Duration;

use nsh_engine::Os;

use crate::Device;

const QUESTION: &str = "How will you use EasyTierUI?";
const CONTROL_SUMMARY: &str = "Everything a client does, plus managing easytier-web servers and hosting or \
                               deploying a control plane.";
const GONE: Duration = Duration::from_secs(2);

#[test]
fn the_first_run_asks_once_and_a_client_lists_only_its_node() {
    let device = Device::desktop(Os::MacOs, 1200.0).first_run();
    let mut app = device.start().unwrap();
    app.wait_for(QUESTION).unwrap();
    assert!(app.screen().selected("id:mode-client").unwrap());
    app.press("Continue").unwrap();
    app.wait_for("No networks on this device").unwrap();
    assert!(app.screen().matches("id:page-peers").unwrap());
    assert!(!app.screen().matches("id:page-machines").unwrap());
    assert!(!app.screen().matches("id:page-host-status").unwrap());
    // One role: no section label above its pages.
    assert!(!app.shows("Node"));
    drop(app);

    let mut again = device.start().unwrap();
    again.wait_for("No networks on this device").unwrap();
    assert!(!again.shows(QUESTION));
}

#[test]
fn choosing_control_lists_every_role() {
    let mut app = Device::desktop(Os::Windows, 1200.0)
        .first_run()
        .start()
        .unwrap();
    app.wait_for(QUESTION).unwrap();
    app.press("id:mode-control").unwrap();
    assert!(app.screen().selected("id:mode-control").unwrap());
    assert!(!app.screen().selected("id:mode-client").unwrap());
    app.press("Continue").unwrap();
    app.wait_for("No networks on this device").unwrap();
    for page in ["page-networks", "page-machines", "page-host-status"] {
        assert!(
            app.screen().matches(&format!("id:{page}")).unwrap(),
            "{page}"
        );
    }
    assert!(app.shows("Console"));
}

#[test]
fn a_phone_asks_too() {
    let mut app = Device::phone(Os::Android).first_run().start().unwrap();
    app.wait_for(QUESTION).unwrap();
    app.press("Continue").unwrap();
    app.wait_for("id:page-peers").unwrap();
    assert!(!app.screen().matches("id:page-machines").unwrap());
}

#[test]
fn the_web_build_is_a_control_plane_and_never_asks() {
    let mut app = Device::desktop(Os::Web, 1200.0)
        .first_run()
        .start()
        .unwrap();
    app.wait_for("Sign in to manage this server").unwrap();
    assert!(!app.shows(QUESTION));
    assert!(app.screen().matches("id:page-deployment").unwrap());
}

#[test]
fn settings_switch_the_mode_and_the_choice_is_kept() {
    let device = Device::desktop(Os::Linux, 1200.0);
    let mut app = device.start().unwrap();
    app.press("id:page-settings").unwrap();
    app.wait_for("Work mode").unwrap();
    assert!(app.shows(CONTROL_SUMMARY));
    app.press("Client").unwrap();
    app.wait_for_gone("id:page-hardening", GONE).unwrap();
    assert!(!app.screen().matches("id:page-machines").unwrap());
    assert!(app.screen().matches("id:page-settings").unwrap());
    drop(app);

    let mut again = device.start().unwrap();
    again.wait_for("No networks on this device").unwrap();
    assert!(!again.screen().matches("id:page-machines").unwrap());
    again.press("id:page-settings").unwrap();
    again.press("Control").unwrap();
    again.wait_for("id:page-machines").unwrap();
}

#[test]
fn a_custom_mode_lists_the_roles_turned_on_and_keeps_one() {
    let mut app = Device::desktop(Os::MacOs, 1200.0).start().unwrap();
    app.press("id:page-settings").unwrap();
    app.press("Custom").unwrap();
    app.wait_for("id:mode-role-console").unwrap();
    app.press("id:mode-role-console >> role:switch").unwrap();
    app.wait_for_gone("id:page-machines", GONE).unwrap();
    assert!(app.screen().matches("id:page-host-status").unwrap());
    assert!(
        app.screen()
            .enabled("id:mode-role-node >> role:switch")
            .unwrap()
    );
    app.press("id:mode-role-host >> role:switch").unwrap();
    app.wait_for_gone("id:page-host-status", GONE).unwrap();

    // The node is the last role on: its switch is off limits.
    assert!(
        !app.screen()
            .enabled("id:mode-role-node >> role:switch")
            .unwrap()
    );
    assert!(app.screen().matches("id:page-networks").unwrap());
}
