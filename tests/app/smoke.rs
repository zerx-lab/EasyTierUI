//! Real-release smokes (ignored by default), driven through the UI on NSH's headless engine:
//! - the console against a real easytier-web on loopback with a temporary `--db`, and a real `--no-tun` core enrolled
//!   to it;
//! - the node role: adopt an unpacked release in Settings, create a network, start it without TUN, read it back.
//!
//! `cargo test --test app smoke -- --ignored`; `ET_RELEASE_DIR` points at an unpacked release (default
//! `/tmp/easytier-2.6.4/easytier-macos-aarch64`).

use std::{
    net::TcpListener,
    process::{Child, Command, Stdio},
    time::Duration,
};

use nsh_app_easytierui::App;
use nsh_engine::{EngineConfig, Locale, Os, Points, Root, Size};
use nsh_testing::{DialogAnswer, Headless};

use crate::Device;

struct Kill(Child);

impl Drop for Kill {
    fn drop(&mut self) {
        if let Err(error) = self.0.kill() {
            eprintln!("kill: {error}");
        }
        if let Err(error) = self.0.wait() {
            eprintln!("wait: {error}");
        }
    }
}

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn release_dir() -> String {
    std::env::var("ET_RELEASE_DIR")
        .unwrap_or_else(|_| "/tmp/easytier-2.6.4/easytier-macos-aarch64".to_owned())
}

/// The app on `device` with the engine's real HTTP stack.
fn start_with_network(device: &Device) -> Headless {
    let theme = nsh_base::Theme::from_json(include_str!("../../theme.json")).unwrap();
    let mut engine = EngineConfig::new(Root::of::<App>().tokens(theme.into_tokens()));
    let size = Size {
        width: Points(device.width),
        height: Points(device.height),
    };
    engine.viewport = size;
    engine.environment.window = size;
    engine.environment.os = device.os;
    engine.environment.idiom = device.idiom;
    engine.environment.locale = Locale::parse(device.locale).unwrap();
    engine.storage = Some(device.storage.clone());
    engine.network = Some(nsh_engine::net::network());
    let mut app = Headless::start(engine).unwrap();
    app.first_render().unwrap();
    app
}

#[test]
#[ignore = "needs the real 2.6.4 binaries"]
fn console_signs_in_to_a_real_easytier_web_and_lists_the_enrolled_core() {
    let dir = release_dir();
    let tmp = std::env::temp_dir().join(format!("etui-console-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let (api, config, rpc) = (free_port(), free_port(), free_port());
    let _web = Kill(
        Command::new(format!("{dir}/easytier-web"))
            .args(["--db", &tmp.join("et.db").to_string_lossy()])
            .args([
                "--api-server-addr",
                "127.0.0.1",
                "--api-server-port",
                &api.to_string(),
            ])
            .args(["--config-server-port", &config.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    std::thread::sleep(Duration::from_millis(1500));
    let _core = Kill(
        Command::new(format!("{dir}/easytier-core"))
            .env(
                "ET_CONFIG_SERVER",
                format!("udp://127.0.0.1:{config}/admin"),
            )
            .args([
                "--machine-id",
                "00000000-0000-0000-0000-00000000abcd",
                "--no-tun",
            ])
            .args(["--hostname", "smoke-host"])
            .args(["--rpc-portal", &format!("127.0.0.1:{rpc}")])
            .args(["--rpc-portal-whitelist", "127.0.0.1/32,::1/128"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );

    let mut app = start_with_network(&Device::desktop(Os::MacOs, 1200.0));
    app.press("id:page-machines").unwrap();
    app.wait_for("No server selected").unwrap();
    app.press("Add server…").unwrap();
    app.wait_for("Add a server").unwrap();
    app.type_text("id:add-server-url", &format!("http://127.0.0.1:{api}"))
        .unwrap();
    app.press("Add").unwrap();
    app.wait_for("Sign in to manage this server").unwrap();
    app.type_text("id:sign-in-username", "admin").unwrap();
    app.type_text("id:sign-in-password", "admin").unwrap();
    app.press("Sign in").unwrap();
    app.wait_for_within("smoke-host", Duration::from_secs(40))
        .unwrap();
    eprintln!("machines page: {:?}", app.screen().texts());
}

#[test]
#[ignore = "needs the real 2.6.4 binaries"]
fn a_network_created_here_runs_on_the_adopted_release_and_reads_back() {
    let mut app = start_with_network(&Device::desktop(Os::MacOs, 1200.0));
    app.on_dialog(|_| DialogAnswer::Press("Use this release".to_owned()));

    // Settings → EasyTier: use the unpacked release after the confirmation that names its version.
    app.press("id:page-settings").unwrap();
    app.wait_for("id:easytier-custom-dir").unwrap();
    app.type_text("id:easytier-custom-dir", &release_dir())
        .unwrap();
    app.press("Check directory").unwrap();
    app.wait_for_within("Use this release…", Duration::from_secs(20))
        .unwrap();
    app.press("Use this release…").unwrap();
    app.wait_for_within("id:easytier-active-version", Duration::from_secs(20))
        .unwrap();

    // Networks: create one, start it without TUN, and wait for what the core reports.
    app.press("id:page-networks").unwrap();
    app.press("id:networks-create >> text:Create network")
        .unwrap();
    app.wait_for("id:flow-name").unwrap();
    app.type_text("id:flow-name", "Smoke").unwrap();
    app.press("Next").unwrap();
    app.wait_for("id:flow-network-name").unwrap();
    app.press("Next").unwrap();
    app.wait_for("Check and save").unwrap();
    app.press("Save and start").unwrap();
    app.wait_for("Start with a network interface?").unwrap();
    app.press("Run without TUN").unwrap();
    // Running is the lamp's word in the row and the Stop button replaces Start once the core has answered.
    app.wait_for_within("id:summary-peers", Duration::from_secs(40))
        .unwrap();
    assert!(app.screen().matches("id:network-stop").unwrap());
    eprintln!("networks page: {:?}", app.screen().texts());
}
