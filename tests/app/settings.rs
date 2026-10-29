//! Settings → EasyTier on NSH's real engine: the releases GitHub lists come from a stub network, nothing is
//! installed and no program runs.

use std::{sync::Arc, time::Duration};

use nsh_app_easytierui::{App, core::install::AssetTarget};
use nsh_engine::{EngineConfig, Locale, Os, Points, Root, Size};
use nsh_testing::{Headless, StubNetwork, StubResponse, StubRoute};

use crate::Device;

/// The listing's request, as `core::install` sends it.
const RELEASES: &str = "GET https://api.github.com/repos/EasyTier/EasyTier/releases*";

/// How long a test waits for something the app does off the engine thread.
const PATIENCE: Duration = Duration::from_secs(5);

/// `device`'s app, with `network` answering what it fetches (none: `fetch` fails as on a host without a network).
pub fn start_with(device: &Device, network: Option<Arc<StubNetwork>>) -> Result<Headless, String> {
    let theme = nsh_base::Theme::from_json(include_str!("../../theme.json"))
        .map_err(|error| format!("theme.json: {error:?}"))?;
    let mut config = EngineConfig::new(Root::of::<App>().tokens(theme.into_tokens()));
    let size = Size {
        width: Points(device.width),
        height: Points(device.height),
    };
    config.viewport = size;
    config.environment.window = size;
    config.environment.os = device.os;
    config.environment.idiom = device.idiom;
    config.environment.locale =
        Locale::parse(device.locale).ok_or_else(|| format!("locale {:?}", device.locale))?;
    config.storage = Some(device.storage.clone());
    if let Some(network) = network {
        config.network = Some(network);
    }
    let mut app = Headless::start(config).map_err(|error| error.to_string())?;
    app.first_render()?;
    Ok(app)
}

/// Waits until `text` is on screen as a whole text.
pub fn appears(app: &mut Headless, text: &str) -> Result<(), String> {
    app.wait_until(&format!("`{text}` on screen"), PATIENCE, |screen| {
        screen.shows(text)
    })
}

fn releases_fixture() -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/github/releases.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// GitHub's release list, answered from the fixture.
fn github() -> Arc<StubNetwork> {
    let mut response = StubResponse::default();
    response.status = 200;
    response.body = releases_fixture();
    Arc::new(StubNetwork::new([(
        RELEASES.to_owned(),
        StubRoute::Response(response),
    )]))
}

/// Whether the fixture lists a zip for the machine the tests run on.
fn fixture_has_a_zip_for_this_machine() -> bool {
    let text = String::from_utf8_lossy(&releases_fixture()).into_owned();
    AssetTarget::current().is_some_and(|target| text.contains(&target.asset_name("v2.6.4")))
}

#[test]
fn a_desktop_without_a_release_says_so_and_lists_github_releases() {
    let network = github();
    let mut app = start_with(&Device::desktop(Os::MacOs, 1200.0), Some(network.clone())).unwrap();
    app.press("id:page-settings").unwrap();

    // The programs are missing, with the way to get them on the same page.
    appears(&mut app, "Not installed").unwrap();
    appears(
        &mut app,
        "No EasyTier release is installed on this device. Install one below to join networks or host a control \
         plane.",
    )
    .unwrap();

    // GitHub's list, newest first, each listing how its download is checked.
    appears(&mut app, "Available releases").unwrap();
    appears(&mut app, "2.6.4").unwrap();
    appears(&mut app, "Verified by sha256 digest").unwrap();
    // A draft and a tag that is not a version are not releases.
    assert!(!app.screen().shows("2.8.0"));
    assert!(!app.screen().shows("nightly"));

    if fixture_has_a_zip_for_this_machine() {
        assert_eq!(app.screen().count("label:Install 2.6.4").unwrap(), 1);
    } else {
        appears(&mut app, "No download for this device").unwrap();
    }
    // A pre-release older than the app supports, or without a digest, offers no install either.
    assert_eq!(app.screen().count("label:Install 2.5.0").unwrap(), 0);

    let requests = network.requests();
    assert_eq!(
        requests.len(),
        1,
        "only the listing is fetched: {requests:?}"
    );
    assert!(
        requests[0]
            .headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("user-agent")),
        "GitHub requires a User-Agent"
    );
}

#[test]
fn installing_asks_first_and_declining_downloads_nothing() {
    if !fixture_has_a_zip_for_this_machine() {
        return;
    }
    let network = github();
    let mut app = start_with(&Device::desktop(Os::Linux, 1200.0), Some(network.clone())).unwrap();
    app.press("id:page-settings").unwrap();
    appears(&mut app, "Verified by sha256 digest").unwrap();
    app.wait_for("label:Install 2.6.4").unwrap();

    // The headless user backs out of the alert, as the default answer does.
    app.press("label:Install 2.6.4").unwrap();
    app.wait_until("the confirmation", PATIENCE, |screen| {
        screen
            .dialogs()
            .iter()
            .any(|dialog| dialog.title.as_deref() == Some("Install EasyTier 2.6.4?"))
    })
    .unwrap();
    app.wait(Duration::from_millis(200)).unwrap();

    assert_eq!(
        network.requests().len(),
        1,
        "nothing but the listing was fetched"
    );
    assert!(
        app.screen().shows("Install"),
        "the release can still be installed"
    );
}

#[test]
fn an_unreachable_github_is_a_state_with_a_retry() {
    let mut app = start_with(&Device::desktop(Os::Windows, 1200.0), None).unwrap();
    app.press("id:page-settings").unwrap();
    appears(&mut app, "Not installed").unwrap();
    appears(
        &mut app,
        "Couldn't reach GitHub. Check the connection and try again.",
    )
    .unwrap();
    assert!(app.screen().shows("Try again"));
}

#[test]
fn a_browser_points_the_servers_programs_at_deployment() {
    let mut app = Device::desktop(Os::Web, 1200.0).start().unwrap();
    app.press("id:page-settings").unwrap();
    appears(
        &mut app,
        "The EasyTier programs on this server are updated through its deployment files.",
    )
    .unwrap();
    // A browser installs nothing: no installed list, no GitHub.
    assert!(!app.screen().shows("Available releases"));
    app.press("Open deployment").unwrap();
    appears(&mut app, "No deployment files yet").unwrap();
}

#[test]
fn a_phone_shows_the_library_that_updates_with_the_app() {
    let mut app = Device::phone(Os::Ios).start().unwrap();
    app.press("id:page-settings").unwrap();
    appears(&mut app, "Library version").unwrap();
    appears(
        &mut app,
        "EasyTier is built into this app and updates with it.",
    )
    .unwrap();
    assert!(!app.screen().shows("Available releases"));
}

#[test]
fn the_page_follows_the_language() {
    let mut app = start_with(
        &Device::desktop(Os::MacOs, 1200.0).locale("zh-Hans-CN"),
        Some(github()),
    )
    .unwrap();
    app.press("id:page-settings").unwrap();
    appears(&mut app, "未安装").unwrap();
    appears(&mut app, "可用版本").unwrap();
}

#[test]
fn the_summary_strip_reports_what_is_known() {
    let mut app = start_with(&Device::desktop(Os::MacOs, 1200.0), Some(github())).unwrap();
    app.press("id:page-settings").unwrap();

    // Nothing is installed: the active cell says so, there is no count of installed releases, and the table has an
    // empty state of its own.
    appears(&mut app, "Active release").unwrap();
    appears(&mut app, "Not installed").unwrap();
    appears(&mut app, "Oldest supported release").unwrap();
    appears(&mut app, "Nothing installed yet").unwrap();
    assert!(!app.screen().shows("Installed"));

    // Once GitHub answers, the newest stable release (not the pre-release) is the latest one.
    appears(&mut app, "Latest release").unwrap();
    appears(&mut app, "Published").unwrap();
    appears(&mut app, "2026-05-12").unwrap();
    assert!(app.screen().shows("Pre-release"));
}

#[test]
fn a_narrow_window_drops_the_date_column() {
    let mut app = start_with(&Device::desktop(Os::MacOs, 800.0), Some(github())).unwrap();
    app.press("id:page-settings").unwrap();
    appears(&mut app, "Verified by sha256 digest").unwrap();
    appears(&mut app, "2.6.4").unwrap();
    assert!(!app.screen().shows("2026-05-12"));
}

#[test]
fn a_compact_window_stacks_the_releases() {
    let mut app = start_with(&Device::desktop(Os::MacOs, 600.0), Some(github())).unwrap();
    app.press("id:page-settings").unwrap();
    appears(&mut app, "Verified by sha256 digest").unwrap();
    appears(&mut app, "2.6.4").unwrap();
    // A stacked table has no header row.
    assert!(!app.screen().shows("Verification"));
}

#[test]
fn checking_for_updates_asks_github_again() {
    let network = github();
    let mut app = start_with(&Device::desktop(Os::MacOs, 1200.0), Some(network.clone())).unwrap();
    app.press("id:page-settings").unwrap();
    appears(&mut app, "Verified by sha256 digest").unwrap();
    assert_eq!(network.requests().len(), 1);

    app.press("id:settings-check-updates >> text:Check for updates")
        .unwrap();
    let asked = network.clone();
    app.wait_until("the listing is asked for again", PATIENCE, move |_| {
        asked.requests().len() >= 2
    })
    .unwrap();
    appears(&mut app, "Verified by sha256 digest").unwrap();
}

#[test]
fn only_a_desktop_offers_to_check_for_updates() {
    let mut web = Device::desktop(Os::Web, 1200.0).start().unwrap();
    web.press("id:page-settings").unwrap();
    appears(&mut web, "Open deployment").unwrap();
    assert!(!web.screen().shows("Check for updates"));

    let mut phone = Device::phone(Os::Android).start().unwrap();
    phone.press("id:page-settings").unwrap();
    appears(&mut phone, "Library version").unwrap();
    assert!(!phone.screen().shows("Check for updates"));
}
