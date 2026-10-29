//! Settings → Desktop on NSH's real engine: the choices a desktop keeps across starts, and the notice elsewhere. No
//! window exists headless, so no tray is attached; the login item is the in-memory one (`test.login-item`).

use std::time::Duration;

use nsh_engine::Os;

use crate::Device;

/// How long a test waits for the login item's answer (it is read off the engine thread).
const PATIENCE: Duration = Duration::from_secs(5);

#[test]
fn quiet_start_needs_the_tray_and_the_choice_survives_a_restart() {
    let device = Device::desktop(Os::MacOs, 1200.0);
    let mut app = device.start().unwrap();
    app.press("id:page-settings").unwrap();
    app.wait_for("id:desktop-tray").unwrap();

    // Keeping to the tray is on by default, so starting quietly can be chosen.
    let quiet = "id:desktop-quiet >> role:switch";
    assert!(app.screen().enabled(quiet).unwrap());
    assert!(!app.screen().matches("id:desktop-quiet-needs-tray").unwrap());

    // Without the tray, nothing would bring a hidden window back: starting quietly is off limits, and says why.
    app.press("id:desktop-tray >> role:switch").unwrap();
    app.wait_for("id:desktop-quiet-needs-tray").unwrap();
    assert!(!app.screen().enabled(quiet).unwrap());
    drop(app);

    let mut app = device.start().unwrap();
    app.press("id:page-settings").unwrap();
    app.wait_for("id:desktop-quiet-needs-tray").unwrap();
    assert!(!app.screen().enabled(quiet).unwrap());
}

#[test]
fn the_login_switch_waits_for_the_login_item() {
    let mut app = Device::desktop(Os::Windows, 1200.0).start().unwrap();
    app.press("id:page-settings").unwrap();
    let login = "id:desktop-login >> role:switch";
    app.wait_until("the login item read", PATIENCE, |screen| {
        screen.enabled(login).unwrap_or(false)
    })
    .unwrap();
    app.press(login).unwrap();
    app.wait_until("the login item changed", PATIENCE, |screen| {
        screen.enabled(login).unwrap_or(false)
    })
    .unwrap();
    assert!(!app.screen().matches("id:desktop-login-failed").unwrap());
}

#[test]
fn the_web_build_says_these_belong_to_the_desktop_app() {
    let mut app = Device::desktop(Os::Web, 1200.0).start().unwrap();
    app.press("id:page-settings").unwrap();
    app.wait_for("id:desktop-unsupported").unwrap();
    assert!(!app.screen().matches("id:desktop-tray").unwrap());
}
