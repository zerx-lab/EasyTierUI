//! The node role end to end, headless: states per platform, the create and join flows, import, the administrator
//! confirmation, and what survives a restart. No EasyTier program is installed in these runs, so nothing starts;
//! what happens when the core is missing is itself a tested state.

use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use nsh_app_easytierui::{
    core::{
        node::{InstanceKey, NodeBackend, NodeError, NodeLaunch},
        state::{LogLine, NatType, NetworkState, NodeInfo, NodeSnapshot, PeerSummary, RouteKind},
    },
    store::node::testing,
};
use nsh_engine::{Os, StorageChange};
use nsh_testing::{DialogAnswer, Headless};

use crate::Device;

const PROFILES_KEY: &str = "profiles.v1";

/// The profile list as the app stored it.
fn stored_profiles(device: &Device) -> String {
    use nsh_engine::StorageBackend as _;
    device
        .storage
        .load()
        .unwrap()
        .get(PROFILES_KEY)
        .cloned()
        .unwrap_or_default()
}

/// Creates a network called `name` through the dialog and returns the generated secret. It is saved, not started.
fn create_network(app: &mut Headless, name: &str) -> String {
    app.press("id:networks-create >> text:Create network")
        .unwrap();
    app.wait_for("id:flow-name").unwrap();
    app.type_text("id:flow-name", name).unwrap();
    app.press("Next").unwrap();
    app.wait_for("id:flow-network-name").unwrap();
    let secret = app
        .screen()
        .input_text("id:flow-secret")
        .unwrap()
        .unwrap_or_default()
        .to_owned();
    app.press("Next").unwrap();
    app.wait_for("Check and save").unwrap();
    app.press("Save without starting").unwrap();
    app.wait_for("id:network-start").unwrap();
    app.wait_for_gone("id:flow-dialog", Duration::from_secs(3))
        .unwrap();
    secret
}

#[test]
fn a_desktop_without_easytier_says_so_and_points_to_settings() {
    let mut app = Device::desktop(Os::MacOs, 1200.0).start().unwrap();
    app.wait_for("id:core-callout").unwrap();
    assert!(app.shows(
        "EasyTier isn't installed, so no network can start. Install a release in Settings."
    ));
    app.press("id:core-callout >> text:Open Settings").unwrap();
    app.wait_for("Language").unwrap();
    assert!(app.screen().selected("id:page-settings").unwrap());
}

#[test]
fn the_empty_networks_page_offers_create_join_and_import() {
    let mut app = Device::desktop(Os::Linux, 1200.0).start().unwrap();
    app.wait_for("No networks on this device").unwrap();
    for id in [
        "networks-create",
        "networks-join",
        "networks-import",
        "networks-managed",
        "networks-empty-create",
    ] {
        app.wait_for(&format!("id:{id}")).unwrap();
    }
    // Nothing to summarize and nothing to search yet: the table's own empty state carries the primary action.
    assert!(!app.screen().matches("id:summary-running").unwrap());
    assert!(app.screen().matches("id:networks-table").unwrap());
}

#[test]
fn creating_a_network_saves_it_without_the_secret_and_it_survives_a_restart() {
    let device = Device::desktop(Os::MacOs, 1200.0);
    let mut app = device.start().unwrap();
    app.wait_for("No networks on this device").unwrap();
    let secret = create_network(&mut app, "Home lab");
    // The secret was generated for the person (long and not echoed anywhere in storage).
    assert!(
        secret.len() >= 32,
        "a generated secret is at least 128 bits"
    );
    assert!(app.shows("Home lab"));
    assert!(app.screen().enabled("id:network-start").unwrap());
    let stored = stored_profiles(&device);
    assert!(stored.contains("Home lab"));
    assert!(
        !stored.contains(&secret),
        "the secret never reaches the app's storage"
    );
    drop(app);

    let mut again = device.start().unwrap();
    again.wait_for("id:network-start").unwrap();
    assert!(again.shows("Home lab"));
    assert!(!again.shows("No networks on this device"));
}

#[test]
fn the_create_flow_asks_for_one_thing_per_screen_and_checks_it() {
    let mut app = Device::desktop(Os::Windows, 1200.0).start().unwrap();
    app.press("id:networks-create >> text:Create network")
        .unwrap();
    app.wait_for("What do you want to call this network?")
        .unwrap();
    assert!(app.shows("Step 1 of 3"));
    // Next with nothing typed names the problem instead of moving on.
    app.press("Next").unwrap();
    app.wait_for("id:flow-name-error").unwrap();
    assert!(app.shows("Name: This is required."));
    app.type_text("id:flow-name", "Office").unwrap();
    app.press("Next").unwrap();
    app.wait_for("Choose the network name and secret").unwrap();
    assert!(app.shows("Step 2 of 3"));
    // The network name starts as the name, and a secret is already generated (Next would refuse an empty one).
    assert_eq!(
        app.screen().input_text("id:flow-network-name").unwrap(),
        Some("Office")
    );
    app.wait_for("id:flow-secret-warning").unwrap();
    // Where secrets are not kept (these runs use an in-memory keychain) the dialog says so beside the secret.
    assert!(app.screen().matches("id:secrets-not-kept").unwrap());
    app.press("Next").unwrap();
    app.wait_for("Check and save").unwrap();
    assert!(app.shows("Assigned automatically (DHCP)"));
    app.press("Back").unwrap();
    app.wait_for("Choose the network name and secret").unwrap();
}

#[test]
fn joining_checks_the_peer_address_and_shows_where_it_connects() {
    let device = Device::desktop(Os::MacOs, 1200.0);
    let mut app = device.start().unwrap();
    app.press("id:networks-join >> text:Join network").unwrap();
    app.wait_for("Enter the network name and secret you were given")
        .unwrap();
    app.type_text("id:flow-network-name", "office").unwrap();
    app.type_text("id:flow-secret", "correct horse battery staple")
        .unwrap();
    app.press("Next").unwrap();
    app.wait_for("id:flow-peer").unwrap();
    assert!(app.shows("Step 2 of 2"));

    app.type_text("id:flow-peer", "gopher://node.example.net")
        .unwrap();
    app.wait_for("id:flow-peer-error").unwrap();
    assert!(app.shows(
        "Shared nodes and peers: This protocol isn't one EasyTier knows: use tcp, udp, ws, wss, quic, wg or faketcp."
    ));
    // Saving is refused and nothing is stored.
    app.press("Save and start").unwrap();
    app.wait_for("id:flow-peer-error").unwrap();
    assert_eq!(stored_profiles(&device), "");

    app.clear("id:flow-peer").unwrap();
    app.type_text("id:flow-peer", "node.example.net:11010")
        .unwrap();
    app.wait_for("id:flow-peer-url").unwrap();
    assert!(app.shows("tcp://node.example.net:11010"));
    app.wait_for_gone("id:flow-peer-error", Duration::from_secs(2))
        .unwrap();
}

#[test]
fn a_first_start_asks_about_the_administrator_and_a_missing_core_is_explained() {
    let device = Device::desktop(Os::MacOs, 1200.0);
    let mut app = device.start().unwrap();
    app.press("id:networks-join >> text:Join network").unwrap();
    app.wait_for("id:flow-network-name").unwrap();
    app.type_text("id:flow-network-name", "office").unwrap();
    app.type_text("id:flow-secret", "correct horse battery staple")
        .unwrap();
    app.press("Next").unwrap();
    app.wait_for("id:flow-peer").unwrap();
    app.type_text("id:flow-peer", "node.example.net:11010")
        .unwrap();
    app.press("Save and start").unwrap();

    // A TUN network needs the administrator: say so, and what it means, before anything is asked of the system.
    app.wait_for("Start with a network interface?").unwrap();
    assert!(app.shows(
        "The network runs as a system service, so it keeps running after you quit this app, until you stop it here."
    ));
    app.press("Run without TUN").unwrap();

    // Without an installed core it cannot start: the card says why and points to Settings.
    app.wait_for("id:network-failure").unwrap();
    assert!(app.shows("EasyTier isn't installed on this device."));
    assert!(app.shows("No TUN"));
    assert!(stored_profiles(&device).contains("\"tun\":false"));
    app.press("id:failure-settings >> text:Open Settings")
        .unwrap();
    app.wait_for("Language").unwrap();
}

#[test]
fn an_import_shows_what_it_exposes_and_never_starts() {
    let device = Device::desktop(Os::Linux, 1200.0);
    let mut app = device.start().unwrap();
    app.press("id:networks-import >> text:Import…").unwrap();
    app.wait_for("id:import-text").unwrap();
    let config = "dhcp = true\n\
                  listeners = [\"tcp://0.0.0.0:11010\"]\n\
                  [network_identity]\n\
                  network_name = \"office\"\n\
                  network_secret = \"example-secret-do-not-use\"\n\
                  [flags]\n\
                  enable_exit_node = true\n";
    app.type_text("id:import-text", config).unwrap();
    app.press("Preview").unwrap();
    app.wait_for("id:import-name").unwrap();
    assert!(app.shows(
        "Listens on every network interface, so anything this device is connected to can reach it."
    ));
    assert!(app.shows("Peers can send their internet traffic through this device."));
    assert!(!app.screen().matches("id:import-secret").unwrap());
    // Nothing is stored until Save.
    assert_eq!(stored_profiles(&device), "");

    app.press("Save network").unwrap();
    app.wait_for("id:network-start").unwrap();
    assert!(stored_profiles(&device).contains("office"));
    // Saved is not started: the card is stopped and no administrator question was asked.
    assert!(!app.shows("Start with a network interface?"));
    assert!(app.screen().enabled("id:network-start").unwrap());
}

#[test]
fn a_broken_import_names_what_is_wrong_and_keeps_the_dialog() {
    let mut app = Device::desktop(Os::MacOs, 1200.0).start().unwrap();
    app.press("id:networks-import >> text:Import…").unwrap();
    app.wait_for("id:import-text").unwrap();
    app.type_text("id:import-text", "this is = = not toml")
        .unwrap();
    app.press("Preview").unwrap();
    app.wait_for("id:import-failure").unwrap();
    assert!(app.shows("This isn't valid TOML."));
    assert!(app.screen().matches("id:import-text").unwrap());
}

#[test]
fn enrolling_in_a_control_plane_explains_what_it_changes() {
    let mut app = Device::desktop(Os::MacOs, 1200.0).start().unwrap();
    app.press("id:networks-managed >> text:Join a control plane…")
        .unwrap();
    app.wait_for("id:managed-dialog").unwrap();
    app.wait_for("id:managed-explain").unwrap();
    app.type_text("id:managed-server", "not an address")
        .unwrap();
    app.press("Enroll and start").unwrap();
    app.wait_for("id:managed-server-error").unwrap();
}

/// The web build's Node pages: a browser cannot join a network, so they manage a machine of the serving server.
mod web {
    use std::{sync::Arc, time::Duration};

    use nsh_engine::{HttpRequest, Os, StorageBackend as _};
    use nsh_testing::{StubNetwork, StubReply, StubResponse, StubRoute};
    use serde_json::{Value, json};

    use crate::{
        Device,
        console::{INSTANCE, MACHINE, fixture, reply, route, server, sign_in, start},
    };

    /// The second machine's id.
    const OTHER: &str = "00000000-0000-0000-0000-000000000002";

    /// The captured machine list with a second machine, `second-host`, next to it.
    fn two_machines() -> Vec<u8> {
        let mut list: Value = serde_json::from_slice(&fixture("web-machines.json")).unwrap();
        let mut second = list["machines"][0].clone();
        second["info"]["machine_id"] = json!({"part1": 0, "part2": 0, "part3": 0, "part4": 2});
        second["info"]["hostname"] = json!("second-host");
        list["machines"].as_array_mut().unwrap().push(second);
        serde_json::to_vec(&list).unwrap()
    }

    /// A server with two machines: the captured network runs on the first, the second has none.
    fn server_with_two_machines() -> Arc<StubNetwork> {
        let peers = fixture("web-list-peer.json");
        let rpc = StubRoute::Handler(Arc::new(move |_: HttpRequest, answer: StubReply| {
            let mut response = StubResponse::default();
            response.status = 200;
            response.body = peers.clone();
            answer.send(response);
        }));
        Arc::new(StubNetwork::new([
            route("GET", "/api/v1/auth/check_login_status", reply(401, b"")),
            route("POST", "/api/v1/auth/login", reply(200, b"{}")),
            route("GET", "/api/v1/machines", reply(200, &two_machines())),
            route(
                "GET",
                &format!("/api/v1/machines/{MACHINE}/networks"),
                reply(200, &fixture("web-list-networks.json")),
            ),
            route(
                "POST",
                &format!("/api/v1/machines/{MACHINE}/networks/metas"),
                reply(200, &fixture("web-metas.json")),
            ),
            route(
                "GET",
                &format!("/api/v1/machines/{MACHINE}/networks/info/{INSTANCE}"),
                reply(200, &fixture("web-info-one.json")),
            ),
            route(
                "POST",
                &format!("/api/v1/machines/{MACHINE}/proxy-rpc"),
                rpc,
            ),
            route(
                "GET",
                &format!("/api/v1/machines/{OTHER}/networks"),
                reply(200, br#"{"running_inst_ids":[],"disabled_inst_ids":[]}"#),
            ),
        ]))
    }

    #[test]
    fn the_web_build_asks_to_sign_in_instead_of_saying_it_cannot_be_a_node() {
        let mut app = Device::desktop(Os::Web, 1200.0).start().unwrap();
        app.press("id:page-networks").unwrap();
        app.wait_for("Sign in to manage this server").unwrap();
        assert!(!app.shows("This browser can't join a network."));
        app.press("id:page-peers").unwrap();
        app.wait_for("Sign in to manage this server").unwrap();
        assert!(!app.shows("This browser can't join a network."));
    }

    #[test]
    fn the_networks_page_lists_the_picked_machines_networks_and_remembers_the_pick() {
        let stub = server_with_two_machines();
        let device = Device::desktop(Os::Web, 1200.0);
        let mut app = start(&device, &stub);
        sign_in(&mut app);
        app.wait_for("example-host-m").unwrap();
        app.press("id:page-networks").unwrap();
        // Two machines and none chosen: the page asks which one.
        app.wait_for("Choose a machine").unwrap();
        assert!(
            !app.screen()
                .matches(&format!("id:network-{INSTANCE}"))
                .unwrap()
        );

        app.press("id:web-node-machine").unwrap();
        app.wait_for(&format!("id:web-node-machine-{MACHINE}"))
            .unwrap();
        app.press(&format!(
            "id:web-node-machine-{MACHINE} >> text:example-host-m"
        ))
        .unwrap();
        app.wait_for(&format!("id:network-{INSTANCE}")).unwrap();
        assert!(app.screen().matches("id:web-node-create").unwrap());
        assert!(app.screen().matches("id:web-node-import").unwrap());

        let kept = device.storage.load().unwrap();
        assert!(
            kept.iter()
                .any(|(key, value)| key.starts_with("web.node.machine.") && value == MACHINE),
            "{kept:?}"
        );

        // After a restart the same machine is managed without choosing it again.
        let mut again = start(&device, &stub);
        sign_in(&mut again);
        again.press("id:page-networks").unwrap();
        again.wait_for(&format!("id:network-{INSTANCE}")).unwrap();
    }

    #[test]
    fn the_only_machine_is_managed_without_choosing_and_creating_opens_the_editor_for_it() {
        let stub = server();
        let mut app = start(&Device::desktop(Os::Web, 1200.0), &stub);
        sign_in(&mut app);
        app.wait_for("example-host-m").unwrap();
        app.press("id:page-networks").unwrap();
        app.wait_for("id:web-node-create").unwrap();
        app.press("id:web-node-create >> text:/Create network/")
            .unwrap();
        app.wait_for("id:network-editor").unwrap();
    }

    #[test]
    fn the_peers_page_shows_the_peers_of_the_picked_machines_network() {
        let stub = server_with_two_machines();
        let mut app = start(&Device::desktop(Os::Web, 1200.0), &stub);
        sign_in(&mut app);
        app.wait_for("example-host-m").unwrap();
        app.press("id:page-peers").unwrap();
        app.wait_for("Choose a machine").unwrap();
        app.press("id:web-node-machine").unwrap();
        app.wait_for(&format!("id:web-node-machine-{MACHINE}"))
            .unwrap();
        app.press(&format!(
            "id:web-node-machine-{MACHINE} >> text:example-host-m"
        ))
        .unwrap();
        app.wait_for("id:machine-peers-network").unwrap();
        app.press("id:machine-peers-network").unwrap();
        app.wait_for(&format!("id:machine-peers-network-{INSTANCE}"))
            .unwrap();
        app.press(&format!("id:machine-peers-network-{INSTANCE} >> text:/./"))
            .unwrap();
        app.wait_for("365717954").unwrap();
        assert!(app.screen().matches("text:/Direct/").unwrap());
    }

    #[test]
    fn a_server_without_machines_explains_how_a_node_enrolls() {
        let stub = Arc::new(StubNetwork::new([
            route("GET", "/api/v1/auth/check_login_status", reply(401, b"")),
            route("POST", "/api/v1/auth/login", reply(200, b"{}")),
            route("GET", "/api/v1/machines", reply(200, br#"{"machines":[]}"#)),
        ]));
        let mut app = start(&Device::desktop(Os::Web, 1200.0), &stub);
        sign_in(&mut app);
        app.press("id:page-networks").unwrap();
        app.wait_for("No machine is enrolled yet").unwrap();
        assert!(app.screen().matches("text:/--config-server/").unwrap());
        app.press("Open Machines").unwrap();
        app.wait_until("the Machines page opened", Duration::from_secs(5), |app| {
            app.selected("id:page-machines").unwrap_or(false)
        })
        .unwrap();
    }
}

#[test]
fn a_phone_creates_a_network_without_tun_and_is_told_why() {
    let device = Device::phone(Os::Android);
    let mut app = device.start().unwrap();
    app.press("id:page-networks").unwrap();
    app.wait_for("No networks on this device").unwrap();
    // No EasyTier program is asked for on a phone: the core callout belongs to the desktops.
    assert!(!app.screen().matches("id:core-callout").unwrap());
    app.press("id:networks-create >> text:Create network")
        .unwrap();
    app.wait_for("id:flow-name").unwrap();
    app.type_text("id:flow-name", "Phone net").unwrap();
    app.press("Next").unwrap();
    app.wait_for("id:flow-network-name").unwrap();
    app.press("Next").unwrap();
    app.wait_for("id:flow-no-tun").unwrap();
    assert!(app.shows(
        "A virtual network interface (TUN) isn't available in this build of the app on phones, so this network runs without one. Subnet sharing, the SOCKS5 proxy and port forwarding work; other apps can't send traffic to the virtual IP."
    ));
    app.press("Save without starting").unwrap();
    app.wait_for("id:network-start").unwrap();
    assert!(app.shows("No TUN"));
    assert!(stored_profiles(&device).contains("\"tun\":false"));
}

#[test]
fn the_peers_page_waits_for_a_running_network() {
    let mut app = Device::desktop(Os::Windows, 1200.0).start().unwrap();
    app.press("id:page-peers").unwrap();
    app.wait_for("No network is running").unwrap();
    app.press("Open Networks").unwrap();
    app.wait_for("No networks on this device").unwrap();
    assert!(app.screen().selected("id:page-networks").unwrap());
}

#[test]
fn the_networks_page_speaks_chinese_on_a_chinese_system() {
    let mut app = Device::desktop(Os::MacOs, 1200.0)
        .locale("zh-Hans-CN")
        .start()
        .unwrap();
    app.wait_for("此设备上还没有网络").unwrap();
    app.press("id:networks-create >> text:创建网络").unwrap();
    app.wait_for("给这个网络起个名字吧").unwrap();
    app.type_text("id:flow-name", "家庭网络").unwrap();
    app.press("下一步").unwrap();
    app.wait_for("设置网络名称和密钥").unwrap();
    assert!(app.shows("第 2 步，共 3 步"));
    app.press("下一步").unwrap();
    app.wait_for("确认并保存").unwrap();
    app.press("仅保存").unwrap();
    app.wait_for("id:network-start").unwrap();
    assert!(app.shows("家庭网络"));
}

/// The log of a dialog's text, for a test that answers confirmations.
fn answering(app: &mut Headless, press: &'static str) -> Arc<Mutex<Vec<String>>> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    app.on_dialog(move |dialog| {
        let mut asked = log.lock().unwrap_or_else(PoisonError::into_inner);
        asked.push(dialog.title.clone().unwrap_or_default());
        asked.push(dialog.message.clone().unwrap_or_default());
        DialogAnswer::Press(press.to_owned())
    });
    seen
}

/// Opens network "Home lab"'s drawer from its row, on the Overview tab.
fn open_drawer(app: &mut Headless) {
    app.press("label:Network Home lab").unwrap();
    app.wait_for("id:network-drawer").unwrap();
}

/// Opens network "Home lab"'s drawer on its Settings tab.
fn open_settings(app: &mut Headless) {
    open_drawer(app);
    app.press("id:detail-tab-settings >> text:Settings")
        .unwrap();
    app.wait_for("id:detail-name").unwrap();
}

#[test]
fn the_network_drawer_edits_settings_and_warns_about_exposure() {
    let device = Device::desktop(Os::MacOs, 1200.0);
    let mut app = device.start().unwrap();
    create_network(&mut app, "Home lab");
    open_settings(&mut app);
    assert_eq!(
        app.screen().input_text("id:detail-network-name").unwrap(),
        Some("Home lab")
    );
    // The footer is the drawer's own and stays in view: Cancel and Save.
    assert!(app.screen().matches("id:detail-cancel").unwrap());
    assert!(app.screen().matches("id:detail-save").unwrap());

    // A setting that widens exposure says what it does right where it is turned on.
    assert!(
        !app.screen()
            .matches("id:detail-offers-exit-warning")
            .unwrap()
    );
    app.press("id:detail-offers-exit >> role:switch").unwrap();
    app.wait_for("id:detail-offers-exit-warning").unwrap();
    assert!(app.shows("Peers can send their internet traffic through this device."));

    // A bad value is named on its field and keeps the drawer open.
    app.type_text("id:detail-mtu", "99999").unwrap();
    app.wait_for("id:detail-mtu-error").unwrap();
    assert!(app.shows("MTU: Use a number from 1280 to 1500."));
    app.press("id:detail-save >> text:Save").unwrap();
    assert!(app.screen().matches("id:network-drawer").unwrap());
    assert!(!stored_profiles(&device).contains("\"offers_exit_node\":true"));

    app.clear("id:detail-mtu").unwrap();
    app.wait_for_gone("id:detail-mtu-error", Duration::from_secs(2))
        .unwrap();
    app.press("id:detail-save >> text:Save").unwrap();
    app.wait_for_gone("id:network-drawer", Duration::from_secs(2))
        .unwrap();
    assert!(stored_profiles(&device).contains("\"offers_exit_node\":true"));
}

#[test]
fn feature_switches_are_saved_in_the_profile() {
    let device = Device::desktop(Os::MacOs, 1200.0);
    let mut app = device.start().unwrap();
    create_network(&mut app, "Home lab");
    open_settings(&mut app);
    // The old one-off switches are gone: the grid covers them.
    for id in [
        "detail-latency-first",
        "detail-magic-dns",
        "detail-multi-thread",
    ] {
        assert!(!app.screen().matches(&format!("id:{id}")).unwrap());
    }
    for id in [
        "bind_device",
        "enable_kcp_proxy",
        "latency_first",
        "accept_dns",
    ] {
        app.wait_for(&format!("id:detail-flag-{id}")).unwrap();
    }
    assert!(!stored_profiles(&device).contains("\"enable_kcp_proxy\":true"));

    app.press("id:detail-flag-bind_device >> role:switch")
        .unwrap();
    app.press("id:detail-flag-enable_kcp_proxy >> role:switch")
        .unwrap();
    app.press("id:detail-save >> text:Save").unwrap();
    app.wait_for_gone("id:network-drawer", Duration::from_secs(2))
        .unwrap();
    let stored = stored_profiles(&device);
    assert!(stored.contains("\"bind_device\":false"));
    assert!(stored.contains("\"enable_kcp_proxy\":true"));
}

#[test]
fn the_vpn_portal_warns_that_it_listens_everywhere_and_is_saved() {
    let device = Device::desktop(Os::MacOs, 1200.0);
    let mut app = device.start().unwrap();
    create_network(&mut app, "Home lab");
    open_settings(&mut app);
    assert!(!app.screen().matches("id:detail-vpn-portal-cidr").unwrap());
    assert!(
        !app.screen()
            .matches("id:detail-vpn-portal-warning")
            .unwrap()
    );

    app.press("id:detail-vpn-portal >> role:switch").unwrap();
    app.wait_for("id:detail-vpn-portal-warning").unwrap();
    app.wait_for("id:detail-vpn-portal-cidr").unwrap();
    assert_eq!(
        app.screen()
            .input_text("id:detail-vpn-portal-port")
            .unwrap(),
        Some("22022")
    );

    // Switched on without a client subnet, the network cannot be saved.
    app.press("id:detail-save >> text:Save").unwrap();
    app.wait_for("id:detail-vpn-portal-cidr-error").unwrap();
    assert!(app.screen().matches("id:network-drawer").unwrap());
    assert!(!stored_profiles(&device).contains("\"vpn_portal\":{"));

    app.type_text("id:detail-vpn-portal-cidr", "10.14.14.0/24")
        .unwrap();
    app.wait_for_gone("id:detail-vpn-portal-cidr-error", Duration::from_secs(2))
        .unwrap();
    app.press("id:detail-save >> text:Save").unwrap();
    app.wait_for_gone("id:network-drawer", Duration::from_secs(2))
        .unwrap();
    assert!(
        stored_profiles(&device)
            .contains("\"vpn_portal\":{\"client_cidr\":\"10.14.14.0/24\",\"listen_port\":22022}")
    );
}

#[test]
fn an_access_control_rule_is_added_through_its_dialog_and_saved() {
    let device = Device::desktop(Os::MacOs, 1200.0);
    let mut app = device.start().unwrap();
    create_network(&mut app, "Home lab");
    open_settings(&mut app);
    app.wait_for("id:detail-acl-switch").unwrap();
    assert!(!app.screen().matches("id:detail-acl-add-inbound").unwrap());

    app.press("id:detail-acl-switch >> role:switch").unwrap();
    app.wait_for("id:detail-acl-add-inbound").unwrap();
    app.press("id:detail-acl-add-inbound >> text:Add inbound chain")
        .unwrap();
    app.wait_for("id:detail-acl-chain-0-add-rule").unwrap();
    app.press("id:detail-acl-chain-0-add-rule >> text:Add rule")
        .unwrap();
    app.wait_for("id:detail-acl-rule-dialog").unwrap();
    app.type_text("id:detail-acl-rule-name", "Allow SSH")
        .unwrap();
    app.press("id:detail-acl-rule-save >> text:Save").unwrap();
    app.wait_for_gone("id:detail-acl-rule-dialog", Duration::from_secs(2))
        .unwrap();
    app.wait_for("id:detail-acl-rule-0-0").unwrap();
    assert!(app.shows("Allow SSH"));

    app.press("id:detail-save >> text:Save").unwrap();
    app.wait_for_gone("id:network-drawer", Duration::from_secs(2))
        .unwrap();
    let stored = stored_profiles(&device);
    assert!(stored.contains("\"acl\":{"));
    assert!(stored.contains("Allow SSH"));
}

#[test]
fn a_bad_route_or_receive_limit_names_its_field_and_blocks_saving() {
    let device = Device::desktop(Os::MacOs, 1200.0);
    let mut app = device.start().unwrap();
    create_network(&mut app, "Home lab");
    open_settings(&mut app);

    app.press("id:detail-routes-on >> role:switch").unwrap();
    app.wait_for("id:detail-routes").unwrap();
    app.type_text("id:detail-routes", "not a route").unwrap();
    app.wait_for("id:detail-routes-error").unwrap();
    app.type_text("id:detail-recv-limit", "fast").unwrap();
    app.wait_for("id:detail-recv-limit-error").unwrap();
    app.press("id:detail-save >> text:Save").unwrap();
    assert!(app.screen().matches("id:network-drawer").unwrap());
    assert!(!stored_profiles(&device).contains("\"manual_routes\":["));

    // Manual routing with no entry at all is valid: it means no routes.
    app.clear("id:detail-routes").unwrap();
    app.wait_for_gone("id:detail-routes-error", Duration::from_secs(2))
        .unwrap();
    app.clear("id:detail-recv-limit").unwrap();
    app.type_text("id:detail-recv-limit", "1048576").unwrap();
    app.wait_for_gone("id:detail-recv-limit-error", Duration::from_secs(2))
        .unwrap();
    app.press("id:detail-save >> text:Save").unwrap();
    app.wait_for_gone("id:network-drawer", Duration::from_secs(2))
        .unwrap();
    let stored = stored_profiles(&device);
    assert!(stored.contains("\"manual_routes\":[]"));
    assert!(stored.contains("\"instance_recv_bps_limit\":1048576"));
}

#[test]
fn a_managed_network_shows_no_feature_switches_or_access_control() {
    use nsh_engine::StorageBackend as _;

    let device = Device::desktop(Os::MacOs, 1200.0);
    {
        let mut app = device.start().unwrap();
        create_network(&mut app, "Home lab");
    }
    let local = stored_profiles(&device);
    let managed = local.replace("\"source\":\"local\"", "\"source\":\"managed\"");
    assert_ne!(local, managed);
    device
        .storage
        .write(StorageChange::Set(PROFILES_KEY.to_owned(), managed));

    let mut app = device.start().unwrap();
    open_settings(&mut app);
    app.wait_for("id:managed-note").unwrap();
    app.wait_for("id:detail-network-name").unwrap();
    for id in [
        "detail-flag-bind_device",
        "detail-acl-switch",
        "detail-vpn-portal",
        "detail-recv-limit",
        "detail-mapped-listeners",
    ] {
        assert!(!app.screen().matches(&format!("id:{id}")).unwrap(), "{id}");
    }
}

#[test]
fn a_stopped_network_drawer_says_nothing_runs_and_logs_are_empty() {
    let mut app = Device::desktop(Os::Linux, 1200.0).start().unwrap();
    create_network(&mut app, "Home lab");
    open_drawer(&mut app);
    app.wait_for("id:status-quiet").unwrap();
    // A stopped network has no facts to show, a Start in the header and a Close in the footer.
    assert!(!app.screen().matches("id:detail-virtual-ip").unwrap());
    assert!(app.screen().matches("id:detail-start").unwrap());
    assert!(!app.screen().matches("id:detail-save").unwrap());
    app.press("id:detail-tab-logs >> text:Logs").unwrap();
    app.wait_for("id:logs-empty").unwrap();
    app.press("id:detail-close >> text:Close").unwrap();
    app.wait_for_gone("id:network-drawer", Duration::from_secs(2))
        .unwrap();
}

#[test]
fn the_drawer_closes_from_its_header_and_the_row_menu_opens_a_tab() {
    let mut app = Device::desktop(Os::MacOs, 1200.0).start().unwrap();
    create_network(&mut app, "Home lab");
    open_drawer(&mut app);
    app.press("label:Close network").unwrap();
    app.wait_for_gone("id:network-drawer", Duration::from_secs(2))
        .unwrap();

    // The row's more menu goes straight to a tab of the drawer.
    app.press("id:network-more").unwrap();
    app.wait_for("id:network-menu-logs").unwrap();
    app.press("id:network-menu-logs >> text:Logs").unwrap();
    app.wait_for("id:network-drawer").unwrap();
    app.wait_for("id:logs-empty").unwrap();
}

#[test]
fn exporting_leaves_the_secret_out_until_asked() {
    let mut app = Device::desktop(Os::MacOs, 1200.0).start().unwrap();
    let secret = create_network(&mut app, "Home lab");
    open_drawer(&mut app);
    app.press("id:detail-tab-export >> text:Export").unwrap();
    app.wait_for("id:export-toml").unwrap();
    let on_screen = |app: &Headless, needle: &str| {
        app.screen()
            .texts()
            .iter()
            .any(|text| text.contains(needle))
    };
    assert!(on_screen(&app, "network_name = \"Home lab\""));
    assert!(on_screen(&app, "easytierui://import"));
    assert!(
        !on_screen(&app, &secret),
        "the secret is left out by default"
    );
    assert!(app.screen().matches("id:export-qr").unwrap());

    app.press("id:export-include-secret >> role:switch")
        .unwrap();
    app.wait_until(
        "the secret in the export",
        Duration::from_secs(3),
        |screen| {
            screen
                .texts()
                .iter()
                .any(|text| text.contains(secret.as_str()))
        },
    )
    .unwrap();
}

#[test]
fn deleting_a_network_asks_first_and_names_it() {
    let device = Device::desktop(Os::MacOs, 1200.0);
    let mut app = device.start().unwrap();
    create_network(&mut app, "Home lab");
    open_settings(&mut app);

    // Cancel: the network stays.
    let asked = answering(&mut app, "Cancel");
    app.press("id:detail-delete >> text:Delete").unwrap();
    app.wait_until("the confirmation", Duration::from_secs(2), |_| {
        !asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty()
    })
    .unwrap();
    {
        let seen = asked.lock().unwrap_or_else(PoisonError::into_inner);
        assert_eq!(
            seen.first().map(String::as_str),
            Some("Delete network “Home lab”?")
        );
    }
    assert!(stored_profiles(&device).contains("Home lab"));
    assert!(app.screen().matches("id:network-drawer").unwrap());

    // Delete: it is gone from the list and from storage.
    let _ = answering(&mut app, "Delete");
    app.press("id:detail-delete >> text:Delete").unwrap();
    app.wait_for("No networks on this device").unwrap();
    assert!(!stored_profiles(&device).contains("Home lab"));
    assert!(!app.screen().matches("id:network-drawer").unwrap());
}

#[test]
fn the_networks_toolbar_searches_by_name_and_filters_by_state() {
    let mut app = Device::desktop(Os::Linux, 1200.0).start().unwrap();
    create_network(&mut app, "Alpha");
    create_network(&mut app, "Beta");
    assert!(app.screen().matches("label:Network Alpha").unwrap());
    assert!(app.screen().matches("label:Network Beta").unwrap());

    // The search keeps what matches the name (or the network name) and nothing else.
    app.type_text("id:networks-search", "alp").unwrap();
    app.wait_for_gone("label:Network Beta", Duration::from_secs(2))
        .unwrap();
    assert!(app.screen().matches("label:Network Alpha").unwrap());
    app.clear("id:networks-search").unwrap();
    app.wait_for("label:Network Beta").unwrap();

    // Nothing runs, so the Running filter keeps nothing and says how to get the list back.
    app.press("id:networks-filter-running >> text:Running")
        .unwrap();
    app.wait_for("id:networks-clear-filter").unwrap();
    assert!(app.shows("No network matches"));
    assert!(!app.screen().matches("label:Network Alpha").unwrap());
    app.press("id:networks-clear-filter >> text:Clear search and filter")
        .unwrap();
    app.wait_for("label:Network Beta").unwrap();
    assert!(app.screen().matches("label:Network Alpha").unwrap());

    app.press("id:networks-filter-stopped >> text:Stopped")
        .unwrap();
    assert!(app.screen().matches("label:Network Alpha").unwrap());
}

// ----- a core that answers: the peers, the charts and the facts -----

static NEXT_TOKEN: AtomicUsize = AtomicUsize::new(0);

/// A core that answers: networks it starts report a node and three peers whose byte counters grow at every read.
#[derive(Default)]
struct Answering {
    reads: Mutex<BTreeMap<InstanceKey, u64>>,
}

fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |elapsed| elapsed.as_secs_f64() * 1000.0)
}

fn peer(
    id: u32,
    host: &str,
    route: RouteKind,
    latency_ms: Option<f64>,
    loss: Option<f64>,
    reads: u64,
) -> PeerSummary {
    PeerSummary {
        peer_id: id,
        hostname: host.to_owned(),
        virtual_ipv4: Some(
            format!("10.126.126.{}/24", id.saturating_sub(9))
                .parse()
                .unwrap(),
        ),
        route,
        latency_ms,
        loss,
        rx_bytes: Some(reads.saturating_mul(40_000)),
        tx_bytes: Some(reads.saturating_mul(10_000)),
        tunnel: Some("udp".to_owned()),
        nat_type: NatType::FullCone,
        version: Some("2.6.4".to_owned()),
    }
}

impl NodeBackend for Answering {
    fn check(&self, _launch: &NodeLaunch) -> Result<(), NodeError> {
        Ok(())
    }

    fn start(&self, launch: &NodeLaunch) -> Result<(), NodeError> {
        self.reads
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(launch.key.clone(), 0);
        Ok(())
    }

    fn stop(&self, key: &InstanceKey) -> Result<(), NodeError> {
        self.reads
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(key);
        Ok(())
    }

    fn remove(&self, key: &InstanceKey) -> Result<(), NodeError> {
        self.stop(key)
    }

    fn poll(&self, key: &InstanceKey) -> NetworkState {
        let mut reads = self.reads.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(count) = reads.get_mut(key) else {
            return NetworkState::Stopped;
        };
        *count = count.saturating_add(1);
        let reads = *count;
        NetworkState::Running(Box::new(NodeSnapshot {
            read_at_ms: now_ms(),
            node: NodeInfo {
                peer_id: 1,
                hostname: "this-node".to_owned(),
                virtual_ipv4: Some("10.126.126.1/24".parse().unwrap()),
                listeners: vec!["tcp://0.0.0.0:11010".to_owned()],
                public_ips: vec!["203.0.113.7".to_owned()],
                nat_type: NatType::FullCone,
                proxy_cidrs: Vec::new(),
                version: "2.6.4-test".to_owned(),
            },
            peers: vec![
                peer(11, "alpha", RouteKind::Direct, Some(12.0), Some(0.0), reads),
                peer(
                    12,
                    "bravo",
                    RouteKind::Relayed { hops: 2 },
                    Some(180.0),
                    Some(0.12),
                    reads,
                ),
                peer(13, "charlie", RouteKind::Direct, None, None, reads),
            ],
        }))
    }

    fn logs(&self, _key: &InstanceKey) -> Vec<LogLine> {
        Vec::new()
    }

    fn service_installed(&self, _key: &InstanceKey) -> bool {
        false
    }

    fn shutdown(&self) {}
}

/// `device` with the answering core standing in for EasyTier.
fn answering_core(device: Device) -> Device {
    let token = format!("answering-{}", NEXT_TOKEN.fetch_add(1, Ordering::Relaxed));
    testing::register(&token, Arc::new(Answering::default()));
    device
        .storage
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(testing::STORAGE_KEY.to_owned(), token);
    device
}

/// Creates "Home lab", starts it and waits until the core has answered once.
fn run_network(app: &mut Headless, desktop: bool) {
    create_network(app, "Home lab");
    app.press("id:network-start >> text:Start").unwrap();
    if desktop {
        app.wait_for("Start with a network interface?").unwrap();
        app.press("Run without TUN").unwrap();
    }
    app.wait_for_within("id:summary-peers", Duration::from_secs(10))
        .unwrap();
}

#[test]
fn a_running_network_fills_the_summary_and_its_row() {
    let mut app = answering_core(Device::desktop(Os::MacOs, 1200.0))
        .start()
        .unwrap();
    run_network(&mut app, true);
    // The row shows the address without its prefix and the peers by how they are reached.
    assert!(app.shows("10.126.126.1"));
    assert!(!app.shows("10.126.126.1/24"));
    assert!(app.screen().matches("id:network-stop").unwrap());
    assert!(app.shows("direct"));
    assert!(app.shows("relayed"));
    assert!(app.screen().matches("id:summary-running").unwrap());

    // The drawer of a running network carries the node's facts and, before two rates, says it is collecting.
    app.press("label:Network Home lab").unwrap();
    app.wait_for("id:network-drawer").unwrap();
    app.wait_for("id:detail-virtual-ip").unwrap();
    assert!(app.shows("10.126.126.1/24"));
    assert!(app.shows("3 (2 direct, 1 relayed)"));
    assert!(app.shows("2.6.4-test"));
    assert!(app.screen().matches("id:detail-listeners").unwrap());
    assert!(app.screen().matches("id:detail-throughput").unwrap());
    // Saving a running network restarts it, and the footer says so next to Save.
    app.press("id:detail-tab-settings >> text:Settings")
        .unwrap();
    app.wait_for("id:detail-save-note").unwrap();
    assert!(app.shows("Saving restarts this network."));
}

#[test]
fn the_peers_page_lists_searches_filters_and_opens_a_peer() {
    let mut app = answering_core(Device::desktop(Os::MacOs, 1200.0))
        .start()
        .unwrap();
    run_network(&mut app, true);
    app.press("id:page-peers").unwrap();
    app.wait_for_within("id:peer-11", Duration::from_secs(10))
        .unwrap();
    for id in [
        "peer-12",
        "peer-13",
        "summary-online",
        "summary-latency",
        "summary-loss",
    ] {
        assert!(app.screen().matches(&format!("id:{id}")).unwrap(), "{id}");
    }

    // Addresses never carry their prefix here and never wrap; the route says how traffic gets there.
    assert!(app.shows("10.126.126.2"));
    assert!(!app.shows("10.126.126.2/24"));
    assert!(app.shows("Direct (P2P)"));
    assert!(app.shows("Relayed, 2 hops"));
    // Search by hostname, then by address.
    app.type_text("id:peers-search", "brav").unwrap();
    app.wait_for_gone("id:peer-11", Duration::from_secs(2))
        .unwrap();
    assert!(app.screen().matches("id:peer-12").unwrap());
    app.clear("id:peers-search").unwrap();
    app.wait_for("id:peer-11").unwrap();
    app.type_text("id:peers-search", "10.126.126.4").unwrap();
    app.wait_for_gone("id:peer-11", Duration::from_secs(2))
        .unwrap();
    assert!(app.screen().matches("id:peer-13").unwrap());
    app.clear("id:peers-search").unwrap();

    // The route filter keeps the relayed peer only, then the direct ones.
    app.press("id:peers-route-relayed >> text:Relayed").unwrap();
    app.wait_for_gone("id:peer-11", Duration::from_secs(2))
        .unwrap();
    assert!(app.screen().matches("id:peer-12").unwrap());
    app.press("id:peers-route-direct >> text:Direct").unwrap();
    app.wait_for_gone("id:peer-12", Duration::from_secs(2))
        .unwrap();
    assert!(app.screen().matches("id:peer-11").unwrap());
    app.press("id:peers-route-all >> text:All").unwrap();
    app.wait_for("id:peer-12").unwrap();

    // A row opens the peer's drawer: the address with its prefix, its network and its history.
    app.press("label:Peer bravo").unwrap();
    app.wait_for("id:peer-drawer").unwrap();
    app.wait_for("id:peer-ip").unwrap();
    assert!(app.shows("10.126.126.3/24"));
    assert!(app.shows("Home lab"));
    assert!(app.screen().matches("id:peer-latency").unwrap());
    assert!(app.screen().matches("id:peer-loss").unwrap());
    app.press("label:Close peer").unwrap();
    app.wait_for_gone("id:peer-drawer", Duration::from_secs(2))
        .unwrap();
}

#[test]
fn the_charts_draw_and_each_peer_shows_its_rate_once_the_core_has_answered() {
    let mut app = answering_core(Device::desktop(Os::MacOs, 1200.0))
        .start()
        .unwrap();
    run_network(&mut app, true);
    app.press("id:page-peers").unwrap();
    // Three reads make two rates: the charts draw (the empty state before them is the components' test).
    app.wait_for_gone("id:peers-throughput-empty", Duration::from_secs(15))
        .unwrap();
    app.wait_for_gone("id:peers-latency-empty", Duration::from_secs(5))
        .unwrap();
    assert!(app.screen().matches("text:/B\\/s/").unwrap());

    // A peer's own rate, not its byte count: the answering core moves 40 kB a read from every peer.
    app.wait_for("id:peer-11 >> text:/KiB\\/s/").unwrap();
    app.press("label:Peer alpha").unwrap();
    app.wait_for("id:peer-rx-rate").unwrap();
    app.press("label:Close peer").unwrap();
    app.wait_for_gone("id:peer-drawer", Duration::from_secs(2))
        .unwrap();
}

#[test]
fn the_peers_page_with_no_peer_matching_says_so_and_offers_the_way_back() {
    let mut app = answering_core(Device::desktop(Os::MacOs, 1200.0))
        .start()
        .unwrap();
    run_network(&mut app, true);
    app.press("id:page-peers").unwrap();
    app.wait_for_within("id:peer-11", Duration::from_secs(10))
        .unwrap();
    app.type_text("id:peers-search", "nobody").unwrap();
    app.wait_for("id:peers-clear-filter").unwrap();
    assert!(app.shows("No peer matches"));
    app.press("id:peers-clear-filter >> text:Clear search and filter")
        .unwrap();
    app.wait_for("id:peer-11").unwrap();
}

#[test]
fn on_a_phone_peers_stack_and_a_row_opens_a_full_page_drawer() {
    let mut app = answering_core(Device::phone(Os::Android)).start().unwrap();
    app.press("id:page-networks").unwrap();
    app.wait_for("No networks on this device").unwrap();
    run_network(&mut app, false);
    app.back().unwrap();
    app.press("id:page-peers").unwrap();
    app.wait_for_within("id:peer-11", Duration::from_secs(10))
        .unwrap();
    assert!(app.shows("alpha"));
    assert!(app.shows("10.126.126.2"));
    app.press("label:Peer alpha").unwrap();
    app.wait_for("id:peer-drawer").unwrap();
    assert!(app.shows("10.126.126.2/24"));
    app.press("label:Close peer").unwrap();
    app.wait_for_gone("id:peer-drawer", Duration::from_secs(2))
        .unwrap();
}
