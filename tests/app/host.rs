//! The control-plane host role on NSH's real engine: the hosted control plane's states without a release, the
//! settings drawer, the capability notices, and the deployment form with its file list. No easytier-web runs and
//! nothing is applied to a server.

use std::{sync::Arc, time::Duration};

use nsh_engine::Os;
use nsh_testing::StubNetwork;

use crate::{
    Device,
    console::{MACHINE, fixture, reply, route, sign_in, start as start_with_server},
    settings::{appears, start_with},
};

/// A server that accepts `admin` / `admin` and answers the control-plane pages' reads with the captured replies.
fn web_server() -> Arc<StubNetwork> {
    Arc::new(StubNetwork::new([
        route("GET", "/api/v1/auth/check_login_status", reply(401, b"")),
        route("POST", "/api/v1/auth/login", reply(200, b"{}")),
        route(
            "GET",
            "/api/v1/machines",
            reply(200, &fixture("web-machines.json")),
        ),
        route(
            "GET",
            "/api/v1/summary",
            reply(200, &fixture("web-summary.json")),
        ),
        route(
            "GET",
            "/api/v1/sessions",
            reply(200, &fixture("web-sessions.json")),
        ),
    ]))
}

/// The text nodes on screen, in tree order.
fn texts(app: &nsh_testing::Headless) -> Vec<String> {
    app.screen()
        .texts()
        .into_iter()
        .map(str::to_owned)
        .collect()
}

/// The position of `text` among the texts on screen.
fn position(app: &nsh_testing::Headless, text: &str) -> Option<usize> {
    texts(app).iter().position(|shown| shown.trim() == text)
}

/// Opens the settings drawer of the Status page.
fn open_settings_drawer(app: &mut nsh_testing::Headless) {
    app.press("id:host-edit-settings >> text:Edit settings")
        .unwrap();
    app.wait_for("id:host-api-port").unwrap();
}

#[test]
fn a_desktop_without_a_release_cannot_host_and_says_where_to_get_one() {
    let mut app = Device::desktop(Os::MacOs, 1200.0).start().unwrap();
    app.press("id:page-host-status").unwrap();

    // Hosting is opt-in: nothing runs until the person starts it.
    appears(&mut app, "No control plane runs on this device").unwrap();
    appears(&mut app, "Stopped").unwrap();
    appears(
        &mut app,
        "Hosting needs an EasyTier release, and none is installed.",
    )
    .unwrap();

    // The config server's exposure is stated before anything starts, with its port and protocol apart.
    appears(&mut app, "UDP 22020").unwrap();
    assert!(
        app.screen().shows(
            "The config server listens on every network interface, not only on this computer. Anyone who can reach \
             that port can try to enroll a machine, so allow it through a firewall only on networks you trust. It \
             listens on:"
        ),
        "texts: {:?}",
        app.screen().texts()
    );

    // Starting without a release fails in words and starts nothing.
    app.press("Start control plane").unwrap();
    app.wait(Duration::from_millis(300)).unwrap();
    assert!(app.screen().shows("Stopped"));
    assert!(!app.screen().shows("Running"));

    // The way out is the EasyTier settings.
    app.press("Open EasyTier settings").unwrap();
    appears(&mut app, "Not installed").unwrap();
}

#[test]
fn the_status_summary_names_the_two_ports_before_anything_runs() {
    let mut app = Device::desktop(Os::Linux, 1200.0).start().unwrap();
    app.press("id:page-host-status").unwrap();
    appears(&mut app, "State").unwrap();
    appears(&mut app, "REST API").unwrap();
    appears(&mut app, "Config server").unwrap();
    app.wait_for("id:summary-api").unwrap();
    app.wait_for("id:summary-config").unwrap();
    // The config server's port as the saved settings have it (tests share one data directory and another test saves
    // its own REST API port, so that cell is only checked to be there); no release is shown while nothing runs.
    appears(&mut app, "22020").unwrap();
    assert!(!app.screen().shows("Release"));
    // The output panel says there is none yet.
    appears(&mut app, "No output yet.").unwrap();
}

#[test]
fn hardening_has_nothing_to_check_before_a_control_plane_exists() {
    let mut app = Device::desktop(Os::Windows, 1200.0).start().unwrap();
    app.press("id:page-hardening").unwrap();
    appears(&mut app, "Nothing to harden yet").unwrap();
    // No checklist, and no "Harden now" in the header, while there is nothing to harden.
    assert!(!app.screen().shows("Harden now"));
    assert_eq!(app.screen().count("id:hardening-checks").unwrap(), 0);
    // The way forward is the status page, where it starts.
    app.press("Open status").unwrap();
    appears(&mut app, "No control plane runs on this device").unwrap();
}

#[test]
fn the_settings_open_in_a_drawer_and_closing_it_drops_unsaved_edits() {
    let mut app = Device::desktop(Os::Linux, 1200.0).start().unwrap();
    app.press("id:page-host-status").unwrap();
    appears(&mut app, "Edit settings").unwrap();
    // The form is not on the page: it lives in the drawer.
    assert_eq!(app.screen().count("id:host-api-port").unwrap(), 0);

    open_settings_drawer(&mut app);
    appears(&mut app, "Control plane settings").unwrap();
    appears(&mut app, "Control plane").unwrap();
    app.clear("id:host-api-port").unwrap();
    app.type_text("id:host-api-port", "4321").unwrap();

    app.press("id:host-settings-drawer-close >> label:Close")
        .unwrap();
    app.wait_for_gone("Control plane settings", Duration::from_secs(2))
        .unwrap();
    // The page still shows the saved port, not the edit.
    assert!(!app.screen().shows("4321"));

    // Opening it again starts from what is saved.
    open_settings_drawer(&mut app);
    app.wait_until("the saved port", Duration::from_secs(5), |screen| {
        screen
            .input_text("id:host-api-port")
            .is_ok_and(|text| text == Some("11211"))
    })
    .unwrap();
}

#[test]
fn the_host_settings_are_kept_across_a_restart() {
    let device = Device::desktop(Os::Linux, 1200.0);
    let mut app = device.start().unwrap();
    app.press("id:page-host-status").unwrap();
    open_settings_drawer(&mut app);
    app.clear("id:host-api-port").unwrap();
    app.type_text("id:host-api-port", "12345").unwrap();
    app.press("Save settings").unwrap();

    let saved = std::env::temp_dir()
        .join(format!("easytierui-tests-{}", std::process::id()))
        .join("web")
        .join("settings.json");
    app.wait_until("the settings file", Duration::from_secs(5), |_| {
        std::fs::read_to_string(&saved).is_ok_and(|text| text.contains("12345"))
    })
    .unwrap();
    // Saving closes the drawer and the page shows the new port.
    app.wait_for_gone("Control plane settings", Duration::from_secs(5))
        .unwrap();
    appears(&mut app, "12345").unwrap();
    drop(app);

    let mut again = device.start().unwrap();
    again.press("id:page-host-status").unwrap();
    appears(&mut again, "12345").unwrap();
    open_settings_drawer(&mut again);
    again
        .wait_until("the saved port", Duration::from_secs(5), |screen| {
            screen
                .input_text("id:host-api-port")
                .is_ok_and(|text| text == Some("12345"))
        })
        .unwrap();
}

#[test]
fn a_port_that_is_not_a_number_is_refused_in_the_drawer() {
    let mut app = Device::desktop(Os::Linux, 1200.0).start().unwrap();
    app.press("id:page-host-status").unwrap();
    open_settings_drawer(&mut app);
    app.clear("id:host-api-port").unwrap();
    app.type_text("id:host-api-port", "abc").unwrap();
    app.press("Save settings").unwrap();
    appears(&mut app, "Enter a port number.").unwrap();
    // The drawer stays open until the edit is fixed or cancelled.
    assert!(app.screen().shows("Control plane settings"));
}

#[test]
fn a_phone_cannot_host_and_deploys_instead() {
    let mut app = Device::phone(Os::Android).start().unwrap();
    app.press("id:page-host-status").unwrap();
    appears(&mut app, "This device can't host a control plane.").unwrap();
    app.press("Deploy a control plane").unwrap();
    // The form is on the phone too.
    appears(&mut app, "Domain").unwrap();
    appears(&mut app, "Docker Compose").unwrap();
}

#[test]
fn on_a_phone_the_files_stack_under_the_form() {
    let mut app = Device::phone(Os::Android).start().unwrap();
    app.press("id:page-deployment").unwrap();
    appears(&mut app, "No deployment files yet").unwrap();
    app.type_text("id:deploy-domain", "vpn.example.com")
        .unwrap();
    appears(&mut app, "README.md").unwrap();
    appears(&mut app, "docker-compose.yml").unwrap();
    // The form comes first, the files after it.
    let form = position(&app, "Domain").unwrap();
    let readme = position(&app, "README.md").unwrap();
    assert!(form < readme, "the form is above the files");
    // The chosen file's text has its Copy.
    app.wait_for("id:deploy-file-README.md-copy").unwrap();
}

#[test]
fn a_browser_shows_the_servers_status_behind_its_sign_in() {
    let stub = web_server();
    let mut app = start_with_server(&Device::desktop(Os::Web, 1200.0), &stub);
    app.press("id:page-host-status").unwrap();
    // Not a capability notice: the page is the serving easytier-web's, so it asks to sign in first.
    sign_in(&mut app);
    app.wait_for(&format!("id:session-{MACHINE}")).unwrap();
    for shown in [
        "Machine sessions",
        "Devices",
        "Reachable",
        "example-host-m",
        "2.6.4-8428a89d",
    ] {
        assert!(app.shows(shown), "{shown}");
    }
    assert!(!app.shows("Open deployment"));
    let requests = stub.requests();
    for path in ["/api/v1/summary", "/api/v1/sessions"] {
        assert!(
            requests.iter().any(|request| request.url.ends_with(path)),
            "{path}"
        );
    }
}

#[test]
fn a_browser_hardens_what_the_api_can_and_sends_the_rest_to_deployment() {
    let stub = web_server();
    let mut app = start_with_server(&Device::desktop(Os::Web, 1200.0), &stub);
    app.press("id:page-hardening").unwrap();
    sign_in(&mut app);
    app.wait_for("id:web-password-action").unwrap();
    for shown in [
        "Change password",
        "Registration is off",
        "The seeded user account's password",
        "Set by deployment",
    ] {
        assert!(app.shows(shown), "{shown}");
    }
    // The seeded account is never tried by signing in: that would replace this browser's session.
    let logins = stub
        .requests()
        .iter()
        .filter(|request| request.url.ends_with("/api/v1/auth/login"))
        .count();
    assert_eq!(logins, 1);
    app.press("id:web-password-action >> text:Change password")
        .unwrap();
    app.wait_for("id:password-new").unwrap();
    app.press("Cancel").unwrap();
    app.press("id:web-open-deployment-registration >> text:Open deployment")
        .unwrap();
    app.wait_for("No deployment files yet").unwrap();
}

#[test]
fn a_valid_domain_renders_the_files_with_the_readme_first() {
    let mut app = Device::desktop(Os::Web, 1200.0).start().unwrap();
    app.press("id:page-deployment").unwrap();
    appears(&mut app, "No deployment files yet").unwrap();

    app.type_text("id:deploy-domain", "vpn.example.com")
        .unwrap();
    appears(&mut app, "README.md").unwrap();
    appears(&mut app, "docker-compose.yml").unwrap();
    appears(&mut app, "Caddyfile").unwrap();
    appears(
        &mut app,
        "Nothing is applied from here. Copy each file to your server, set the permissions shown, and follow the \
         README, which comes first.",
    )
    .unwrap();
    assert!(!app.screen().shows("No deployment files yet"));
    // The list has its columns, and the toolbar counts the files.
    for header in ["File", "Permissions", "Size"] {
        assert!(app.screen().shows(header), "no {header:?} column");
    }
    assert!(
        texts(&app)
            .iter()
            .any(|shown| shown.trim().starts_with("Files: ")),
        "the toolbar does not count the files"
    );

    let readme = position(&app, "README.md").unwrap();
    let compose = position(&app, "docker-compose.yml").unwrap();
    let proxy = position(&app, "Caddyfile").unwrap();
    assert!(
        readme < compose && compose < proxy,
        "the README comes first"
    );

    // The README is shown first, with its own Copy; the other files' text waits until they are chosen.
    app.wait_for("id:deploy-file-README.md-copy").unwrap();
    assert_eq!(
        app.screen().count("id:deploy-file-Caddyfile-copy").unwrap(),
        0
    );

    // Pressing a row of the list shows that file instead.
    app.press("id:deploy-row-Caddyfile").unwrap();
    app.wait_for("id:deploy-file-Caddyfile-copy").unwrap();
    assert_eq!(
        app.screen().count("id:deploy-file-README.md-copy").unwrap(),
        0
    );

    // Until hardening is done the API stays closed at the proxy.
    assert!(app.screen().shows("Closed"));
}

#[test]
fn localhost_is_refused_under_the_domain_field_and_nothing_renders() {
    let mut app = Device::desktop(Os::Web, 1200.0).start().unwrap();
    app.press("id:page-deployment").unwrap();
    app.type_text("id:deploy-domain", "localhost").unwrap();
    appears(&mut app, "Domain doesn't look right.").unwrap();
    assert!(app.screen().shows("No deployment files yet"));
    assert!(!app.screen().shows("docker-compose.yml"));

    // Fixing the field renders the files and clears the error.
    app.clear("id:deploy-domain").unwrap();
    app.type_text("id:deploy-domain", "vpn.example.com")
        .unwrap();
    appears(&mut app, "docker-compose.yml").unwrap();
    assert!(!app.screen().shows("Domain doesn't look right."));
}

#[test]
fn a_port_the_proxy_needs_is_refused_under_its_field() {
    let mut app = Device::desktop(Os::Web, 1200.0).start().unwrap();
    app.press("id:page-deployment").unwrap();
    app.type_text("id:deploy-domain", "vpn.example.com")
        .unwrap();
    appears(&mut app, "docker-compose.yml").unwrap();

    app.clear("id:deploy-api-port").unwrap();
    app.type_text("id:deploy-api-port", "443").unwrap();
    appears(
        &mut app,
        "REST API port collides with another setting or a port the proxy needs.",
    )
    .unwrap();
    assert!(!app.screen().shows("docker-compose.yml"));
}

#[test]
fn the_deployment_page_speaks_chinese_when_the_system_does() {
    let mut app = Device::desktop(Os::MacOs, 1200.0)
        .locale("zh-Hans-CN")
        .start()
        .unwrap();
    app.press("id:page-deployment").unwrap();
    appears(&mut app, "域名").unwrap();
    app.type_text("id:deploy-domain", "localhost").unwrap();
    appears(&mut app, "域名的格式不正确。").unwrap();
    app.clear("id:deploy-domain").unwrap();
    app.type_text("id:deploy-domain", "vpn.example.com")
        .unwrap();
    appears(
        &mut app,
        "这里不会执行任何操作。请把每个文件复制到服务器，按所示设置权限，并按 README 的步骤操作（README 排在最前）。",
    )
    .unwrap();
    assert!(
        texts(&app)
            .iter()
            .any(|shown| shown.trim().starts_with("文件：")),
        "the toolbar does not count the files"
    );
}

#[test]
fn the_hosting_pages_follow_the_system_language() {
    let mut app = start_with(
        &Device::desktop(Os::MacOs, 1200.0).locale("zh-Hans-CN"),
        None,
    )
    .unwrap();
    app.press("id:page-host-status").unwrap();
    appears(&mut app, "启动控制平面").unwrap();
    appears(&mut app, "托管控制平面需要先安装 EasyTier，目前尚未安装。").unwrap();
    appears(&mut app, "编辑设置").unwrap();
}
