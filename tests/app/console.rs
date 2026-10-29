//! The console role: signing in to the server that served the web build, the add-server dialog, an ended session,
//! and creating a network on a machine. easytier-web is stubbed with the captured 2.6.4 replies, so the requests the
//! pages make are the real ones.

use std::{sync::Arc, time::Duration};

use nsh_app_easytierui::App;
use nsh_engine::{
    EngineConfig, HttpBody, HttpRequest, Locale, Os, Points, Root, Size, StorageBackend,
};
use nsh_testing::{DialogAnswer, Headless, StubNetwork, StubReply, StubResponse, StubRoute};
use serde_json::{Value, json};

use crate::Device;

/// The machine the captured `web-machines.json` lists.
const MACHINE: &str = "00000000-0000-0000-0000-000000000001";
/// The MD5 hex of `admin`, what easytier-web's login takes.
const ADMIN_DIGEST: &str = "21232f297a57a5a743894a0e4a801fc3";
/// The network the captured server pushed to that machine.
const INSTANCE: &str = "00000000-0000-0000-0000-0000000000aa";

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/easytier-2.6.4/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn reply(status: u16, body: &[u8]) -> StubRoute {
    let mut response = StubResponse::default();
    response.status = status;
    response.body = body.to_vec();
    StubRoute::Response(response)
}

/// A route for `path` below `/api/v1`, whatever the server's base is (the web build's is relative).
fn route(method: &str, path: &str, route: StubRoute) -> (String, StubRoute) {
    (format!("{method} *{path}"), route)
}

/// A server that accepts `admin` / `admin` and lists the captured machine, with no network on it.
fn server() -> Arc<StubNetwork> {
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
            &format!("/api/v1/machines/{MACHINE}/networks"),
            reply(200, br#"{"running_inst_ids":[],"disabled_inst_ids":[]}"#),
        ),
        route(
            "POST",
            &format!("/api/v1/machines/{MACHINE}/validate-config"),
            reply(200, &fixture("web-validate.json")),
        ),
        route(
            "POST",
            &format!("/api/v1/machines/{MACHINE}/networks"),
            reply(200, &fixture("web-run.json")),
        ),
    ]))
}

/// Starts the app on `device` with `network` answering every request.
fn start(device: &Device, network: &Arc<StubNetwork>) -> Headless {
    let theme = nsh_base::Theme::from_json(include_str!("../../theme.json")).unwrap();
    let mut config = EngineConfig::new(Root::of::<App>().tokens(theme.into_tokens()));
    let size = Size {
        width: Points(device.width),
        height: Points(device.height),
    };
    config.viewport = size;
    config.environment.window = size;
    config.environment.os = device.os;
    config.environment.idiom = device.idiom;
    config.environment.locale = Locale::parse(device.locale).unwrap();
    config.storage = Some(device.storage.clone());
    config.network = Some(network.clone());
    let mut app = Headless::start(config).unwrap();
    app.first_render().unwrap();
    app
}

/// The JSON a request carried.
fn json_body(request: &HttpRequest) -> Value {
    match &request.body {
        HttpBody::Bytes(bytes) => serde_json::from_slice(bytes).unwrap(),
        other => panic!("the body is {other:?}"),
    }
}

/// Fills the sign-in form with `admin` / `admin` and presses the button.
fn sign_in(app: &mut Headless) {
    app.wait_for("Sign in to manage this server").unwrap();
    app.type_text("id:sign-in-username", "admin").unwrap();
    app.type_text("id:sign-in-password", "admin").unwrap();
    app.press("Sign in").unwrap();
}

#[test]
fn the_web_build_signs_in_to_the_server_that_served_it_and_lists_its_machines() {
    let stub = server();
    let mut app = start(&Device::desktop(Os::Web, 1200.0), &stub);
    // The served page's own server is the target: it opens on its machines, asking to sign in.
    sign_in(&mut app);
    app.wait_for("example-host-m").unwrap();
    assert!(app.shows("macOS 27.2"));
    assert!(app.shows("2.6.4-8428a89d"));

    let requests = stub.requests();
    let login = requests
        .iter()
        .find(|request| request.url.ends_with("/api/v1/auth/login"))
        .unwrap();
    // The password travels as its digest, never as typed.
    assert_eq!(
        json_body(login),
        json!({"username": "admin", "password": ADMIN_DIGEST})
    );
}

#[test]
fn a_wrong_password_is_reported_and_the_form_stays() {
    let stub = Arc::new(StubNetwork::new([
        route("GET", "/api/v1/auth/check_login_status", reply(401, b"")),
        route(
            "POST",
            "/api/v1/auth/login",
            reply(401, br#"{"message":"Invalid credentials"}"#),
        ),
    ]));
    let mut app = start(&Device::desktop(Os::Web, 1200.0), &stub);
    sign_in(&mut app);
    app.wait_for("The server refused this username or password.")
        .unwrap();
    assert!(app.shows("Sign in"));
    assert!(!app.shows("Machines: "));
}

#[test]
fn a_server_that_does_not_answer_offers_a_retry() {
    let stub = Arc::new(StubNetwork::new(Vec::<(String, StubRoute)>::new()));
    let mut app = start(&Device::desktop(Os::Web, 1200.0), &stub);
    app.wait_for("text:/Can't reach this server/").unwrap();
    assert!(app.shows("Unreachable"));
    app.press("Try again").unwrap();
    app.wait_for("text:/Can't reach this server/").unwrap();
}

#[test]
fn a_401_on_the_machines_shows_the_ended_session_and_the_sign_in_form() {
    // The browser still holds a session cookie: the check passes, then the machine list says it is gone.
    let stub = Arc::new(StubNetwork::new([
        route("GET", "/api/v1/auth/check_login_status", reply(200, b"{}")),
        route("GET", "/api/v1/machines", reply(401, b"")),
    ]));
    let mut app = start(&Device::desktop(Os::Web, 1200.0), &stub);
    app.wait_for("text:/session on this server ended/").unwrap();
    assert!(app.shows("Session ended"));
    assert!(app.shows("Username"));
    assert!(app.shows("Sign in"));
}

#[test]
fn the_sign_in_form_speaks_chinese_on_a_chinese_system() {
    let stub = server();
    let mut app = start(
        &Device::desktop(Os::Web, 1200.0).locale("zh-Hans-CN"),
        &stub,
    );
    app.wait_for("登录后管理此服务器").unwrap();
    assert!(app.shows("用户名"));
}

#[test]
fn a_device_without_a_server_offers_to_add_one_and_a_desktop_to_host_one() {
    let stub = server();
    let mut desktop = start(&Device::desktop(Os::MacOs, 1200.0), &stub);
    desktop.press("id:page-machines").unwrap();
    desktop.wait_for("No server selected").unwrap();
    assert!(desktop.shows("Add server…"));
    assert!(desktop.shows("Host a control plane…"));

    // A phone cannot host one: the way to a control plane is a server added here or deployed.
    let mut phone = start(&Device::phone(Os::Ios), &stub);
    phone.press("id:page-machines").unwrap();
    phone.wait_for("No server selected").unwrap();
    assert!(phone.shows("Add server…"));
    assert!(!phone.shows("Host a control plane…"));
}

#[test]
fn plain_http_to_another_machine_is_refused_on_the_field() {
    let device = Device::desktop(Os::MacOs, 1200.0);
    let mut app = start(&device, &server());
    app.press("id:page-machines").unwrap();
    app.wait_for("No server selected").unwrap();
    app.press("Add server…").unwrap();
    app.wait_for("Add a server").unwrap();
    app.type_text("id:add-server-url", "http://example.com")
        .unwrap();
    app.press("Add").unwrap();
    app.wait_for("Plain http:// is only allowed for this machine. Use https://.")
        .unwrap();
    // Nothing was added or kept.
    assert!(
        !device
            .storage
            .load()
            .unwrap()
            .contains_key("console.servers.v1")
    );

    // Loopback is allowed over http.
    app.clear("id:add-server-url").unwrap();
    app.type_text("id:add-server-url", "http://127.0.0.1:11211")
        .unwrap();
    app.press("Add").unwrap();
    app.wait_for("Sign in to manage this server").unwrap();
    assert!(
        device
            .storage
            .load()
            .unwrap()
            .contains_key("console.servers.v1")
    );
}

#[test]
fn an_added_server_is_kept_across_a_restart_and_no_password_is() {
    let device = Device::desktop(Os::MacOs, 1200.0);
    let stub = server();
    let mut app = start(&device, &stub);
    app.press("id:page-machines").unwrap();
    app.wait_for("No server selected").unwrap();
    app.press("Add server…").unwrap();
    app.wait_for("Add a server").unwrap();
    app.type_text("id:add-server-url", "https://et.example.com/")
        .unwrap();
    app.type_text("id:add-server-name", "Office").unwrap();
    app.press("Add").unwrap();
    // The new server is selected, on its machines, asking to sign in.
    app.wait_for("Sign in to manage this server").unwrap();
    assert!(app.shows("https://et.example.com"));
    app.type_text("id:sign-in-username", "admin").unwrap();
    app.type_text("id:sign-in-password", "never-stored-example")
        .unwrap();
    drop(app);

    let kept = device.storage.load().unwrap();
    let servers: Value = serde_json::from_str(&kept["console.servers.v1"]).unwrap();
    assert_eq!(
        servers,
        json!([{"id": 16, "url": "https://et.example.com", "name": "Office"}])
    );
    // Neither the typed password nor any password key reached storage.
    assert!(
        kept.values()
            .all(|value| !value.contains("never-stored-example"))
    );
    assert!(kept.keys().all(|key| !key.contains("password")));

    // After a restart the server is a target again, and nothing was asked of it yet.
    let mut again = start(&device, &stub);
    again.press("This device").unwrap();
    again.wait_for("Office").unwrap();
    again.press("Office").unwrap();
    again.wait_for("Sign in to manage this server").unwrap();
    assert!(again.shows("https://et.example.com"));
    assert!(stub.requests().is_empty());
}

#[test]
fn creating_a_network_on_a_machine_validates_then_runs_the_same_config() {
    let stub = server();
    let mut app = start(&Device::desktop(Os::Web, 1200.0), &stub);
    sign_in(&mut app);
    app.wait_for("example-host-m").unwrap();
    app.press(&format!("id:machine-{MACHINE}")).unwrap();
    app.wait_for("Create network on this machine").unwrap();
    app.press("Create network on this machine").unwrap();
    app.wait_for("Create network on example-host-m").unwrap();

    // A form that is not complete says what is missing and sends nothing.
    app.press("Create network").unwrap();
    app.wait_for("Enter a network name.").unwrap();
    assert!(
        stub.requests()
            .iter()
            .all(|request| !request.url.ends_with("/validate-config"))
    );

    app.type_text("id:editor-name", "example-net").unwrap();
    app.type_text("id:editor-secret", "example-secret-do-not-use")
        .unwrap();
    app.press("Create network").unwrap();
    app.wait_for_gone("Create network on example-host-m", Duration::from_secs(5))
        .unwrap();

    let requests = stub.requests();
    let at = |suffix: &str, method: &str| {
        requests
            .iter()
            .position(|request| request.method == method && request.url.ends_with(suffix))
            .unwrap()
    };
    let validate = at(&format!("/machines/{MACHINE}/validate-config"), "POST");
    let run = at(&format!("/machines/{MACHINE}/networks"), "POST");
    assert!(validate < run, "the core checks the config before it runs");

    let checked = json_body(&requests[validate]);
    let started = json_body(&requests[run]);
    assert_eq!(checked["config"], started["config"]);
    assert_eq!(started["save"], json!(true));
    let config = &started["config"];
    assert_eq!(config["network_name"], "example-net");
    assert_eq!(config["network_secret"], "example-secret-do-not-use");
    // Safe defaults: an address from DHCP, listed peers (none yet), a TUN, nothing exposed, aes-gcm.
    assert_eq!(config["dhcp"], json!(true));
    assert_eq!(config["networking_method"], json!(1));
    assert_eq!(config["no_tun"], json!(false));
    assert_eq!(config["enable_exit_node"], json!(false));
    assert_eq!(config["disable_encryption"], json!(false));
    assert_eq!(config["encryption_algorithm"], "aes-gcm");
    assert_eq!(config["proxy_cidrs"], json!([]));
    assert_eq!(config["peer_urls"], json!([]));
    assert!(
        config["instance_id"]
            .as_str()
            .is_some_and(|id| id.len() == 36)
    );
}

/// A server with the captured network on the captured machine, and the core behind it answering the credential
/// calls of `proxy-rpc` as the captured 2.6.4 core did.
fn server_with_a_network() -> Arc<StubNetwork> {
    let list = fixture("web-credential-list.json");
    let generate = fixture("web-credential-generate.json");
    let peers = fixture("web-list-peer.json");
    let rpc = StubRoute::Handler(Arc::new(move |request: HttpRequest, reply: StubReply| {
        let method = json_body(&request)["method_name"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let mut answer = StubResponse::default();
        answer.status = 200;
        answer.body = match method.as_str() {
            "ListCredentials" => list.clone(),
            "GenerateCredential" => generate.clone(),
            "ListPeer" => peers.clone(),
            _ => br#"{"success":true}"#.to_vec(),
        };
        reply.send(answer);
    }));
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
            "GET",
            &format!("/api/v1/machines/{MACHINE}/networks/config/{INSTANCE}"),
            reply(200, &fixture("web-get-config.json")),
        ),
        route(
            "PUT",
            &format!("/api/v1/machines/{MACHINE}/networks/{INSTANCE}"),
            reply(200, b"{}"),
        ),
        route(
            "DELETE",
            &format!("/api/v1/machines/{MACHINE}/networks/{INSTANCE}"),
            reply(200, b"{}"),
        ),
        route(
            "POST",
            &format!("/api/v1/machines/{MACHINE}/proxy-rpc"),
            rpc,
        ),
    ]))
}

/// Opens the network's more menu in the machine drawer and chooses Delete (which asks first).
fn delete_from_menu(app: &mut Headless) {
    app.press(&format!("id:network-more-{INSTANCE}")).unwrap();
    app.wait_for(&format!("id:network-delete-{INSTANCE}"))
        .unwrap();
    app.press(&format!("id:network-delete-{INSTANCE} >> text:Delete"))
        .unwrap();
}

#[test]
fn a_network_is_switched_off_and_deleted_after_the_confirmation_names_it() {
    let stub = server_with_a_network();
    let mut app = start(&Device::desktop(Os::Web, 1200.0), &stub);
    sign_in(&mut app);
    app.wait_for("example-host-m").unwrap();
    app.press(&format!("id:machine-{MACHINE}")).unwrap();
    app.wait_for("example-net").unwrap();
    assert!(app.shows("Running"));
    assert!(app.shows("10.144.145.1/24"));

    app.press("Disable").unwrap();
    let sent = stub.clone();
    app.wait_until(
        "the switch-off was sent",
        Duration::from_secs(5),
        move |_| {
            sent.requests()
                .iter()
                .any(|request| request.method == "PUT")
        },
    )
    .unwrap();
    let switch = stub
        .requests()
        .into_iter()
        .find(|request| request.method == "PUT")
        .unwrap();
    assert!(switch.url.ends_with(&format!("/networks/{INSTANCE}")));
    assert_eq!(json_body(&switch), json!({"disabled": true}));

    // Cancelling the confirmation deletes nothing; confirming it does.
    delete_from_menu(&mut app);
    app.wait(Duration::from_millis(200)).unwrap();
    assert!(
        stub.requests()
            .iter()
            .all(|request| request.method != "DELETE")
    );
    app.on_dialog(|_| DialogAnswer::Press("Delete".to_owned()));
    delete_from_menu(&mut app);
    let sent = stub.clone();
    app.wait_until("the delete was sent", Duration::from_secs(5), move |_| {
        sent.requests()
            .iter()
            .any(|request| request.method == "DELETE")
    })
    .unwrap();
}

#[test]
fn a_credential_is_generated_listed_and_its_secret_shown_once() {
    let stub = server_with_a_network();
    let mut app = start(&Device::desktop(Os::Web, 1200.0), &stub);
    sign_in(&mut app);
    app.wait_for("example-host-m").unwrap();
    app.press("id:page-credentials").unwrap();
    app.wait_for("text:/A credential is a key/").unwrap();
    // The note that there is no device-blocking endpoint is on the page.
    assert!(app.shows("easytier-web 2.6.4 has no way to block a single device. To cut a machine off, disable its network on the Machines page."));

    app.press("id:credentials-machine").unwrap();
    app.wait_for(&format!("id:credentials-machine-{MACHINE}"))
        .unwrap();
    // The option's wrapper carries the id; the press lands on the menu item through its label.
    app.press(&format!(
        "id:credentials-machine-{MACHINE} >> text:example-host-m"
    ))
    .unwrap();
    app.wait_for("id:credentials-network").unwrap();
    app.press("id:credentials-network").unwrap();
    app.wait_for(&format!("id:credentials-network-{INSTANCE}"))
        .unwrap();
    app.press(&format!("id:credentials-network-{INSTANCE} >> text:/./"))
        .unwrap();
    app.wait_for("0c2dcf86-37c3-4db6-b1ef-7167739c2d3b")
        .unwrap();
    assert!(app.shows("g1"));
    assert!(app.shows("May relay"));

    app.press("Generate credential").unwrap();
    app.wait_for("Valid for").unwrap();
    // The form is a drawer: its panel carries the id and the kind chip says what it is.
    assert!(app.screen().matches("id:credential-form").unwrap());
    app.press("Generate").unwrap();
    app.wait_for("Credential created").unwrap();
    assert!(app.shows("Store this secret now. It is shown only once and can't be read again."));
    assert!(app.shows("example-secret-do-not-use"));
    app.press("I've stored it").unwrap();
    app.wait_for_gone("Credential created", Duration::from_secs(2))
        .unwrap();
    assert!(!app.shows("example-secret-do-not-use"));
}

#[test]
fn a_machine_row_opens_its_drawer_with_facts_and_tabs_and_the_close_button_closes_it() {
    let stub = server();
    let mut app = start(&Device::desktop(Os::Web, 1200.0), &stub);
    sign_in(&mut app);
    app.wait_for("example-host-m").unwrap();
    // The strip above the table counts what the server listed.
    assert!(app.shows("Core versions"));
    assert!(!app.shows("Machine ID"));

    app.press(&format!("id:machine-{MACHINE}")).unwrap();
    app.wait_for("Machine ID").unwrap();
    assert!(app.shows("Connected from"));
    assert!(app.shows(MACHINE));
    // The machine runs no network: its networks tab says so, and the peers tab has nothing to list either.
    app.wait_for("No networks on this machine").unwrap();
    app.press("id:machine-tab-peers >> text:Peers").unwrap();
    app.wait_for("No running network on this machine").unwrap();

    app.press("id:machine-drawer-close >> role:button").unwrap();
    app.wait_for_gone("Machine ID", Duration::from_secs(2))
        .unwrap();
    // The table is still there.
    assert!(app.shows("macOS 27.2"));
}

#[test]
fn searching_machines_filters_the_table_and_an_empty_result_can_be_cleared() {
    let stub = server();
    let mut app = start(&Device::desktop(Os::Web, 1200.0), &stub);
    sign_in(&mut app);
    app.wait_for("example-host-m").unwrap();

    // The search is case-insensitive and reads the system column too.
    app.type_text("id:machine-search", "MACOS").unwrap();
    app.wait(Duration::from_millis(100)).unwrap();
    assert!(app.shows("example-host-m"));
    app.press("id:machine-search-clear >> role:button").unwrap();

    app.type_text("id:machine-search", "no-such-machine")
        .unwrap();
    app.wait_for("Nothing matches your search").unwrap();
    assert!(!app.shows("macOS 27.2"));
    app.press("id:machine-search-clear >> role:button").unwrap();
    app.wait_for("macOS 27.2").unwrap();
    assert!(!app.shows("Nothing matches your search"));
}

#[test]
fn the_networks_page_groups_by_name_filters_and_a_row_opens_the_config_editor_in_a_drawer() {
    let stub = server_with_a_network();
    let mut app = start(&Device::desktop(Os::Web, 1200.0), &stub);
    sign_in(&mut app);
    app.wait_for("example-host-m").unwrap();
    app.press("id:page-server-networks").unwrap();
    // The group header names the network and counts its machines; the row carries the machine and the address.
    app.wait_for("Machines: 1").unwrap();
    assert!(app.shows("example-net"));
    assert!(app.shows("10.144.145.1/24"));
    // The page's primary action is in the header.
    assert!(app.shows("Import config…"));

    app.type_text("id:network-search", "no-such-network")
        .unwrap();
    app.wait_for("Nothing matches your search").unwrap();
    assert!(!app.shows("10.144.145.1/24"));
    app.press("id:network-search-clear >> role:button").unwrap();
    app.wait_for("Machines: 1").unwrap();

    app.press(&format!("id:network-{INSTANCE}")).unwrap();
    app.wait_for("Edit network").unwrap();
    app.wait_for("id:editor-name").unwrap();
    assert!(app.shows("Connectivity"));
    app.press("id:network-editor-close >> role:button").unwrap();
    app.wait_for_gone("Edit network", Duration::from_secs(2))
        .unwrap();
}

#[test]
fn the_peers_of_a_running_network_open_in_the_machine_drawer() {
    let stub = server_with_a_network();
    let mut app = start(&Device::desktop(Os::Web, 1200.0), &stub);
    sign_in(&mut app);
    app.wait_for("example-host-m").unwrap();
    app.press("id:page-server-networks").unwrap();
    app.wait_for("Machines: 1").unwrap();

    // From the row's menu: the machine's drawer opens on its peers tab with the network chosen.
    app.press(&format!("id:network-more-{INSTANCE}")).unwrap();
    app.wait_for(&format!("id:network-peers-{INSTANCE}"))
        .unwrap();
    app.press(&format!("id:network-peers-{INSTANCE} >> text:Peers"))
        .unwrap();
    app.wait_for("365717954").unwrap();
    assert!(app.screen().matches("text:/Direct/").unwrap());
}

#[test]
fn on_a_phone_machines_stack_and_the_drawer_is_a_full_page_closed_by_its_button() {
    let stub = server();
    let mut app = start(&Device::phone(Os::Web), &stub);
    if app.screen().matches("id:page-machines").unwrap() {
        app.press("id:page-machines").unwrap();
    }
    sign_in(&mut app);
    app.wait_for("example-host-m").unwrap();
    assert!(app.shows("macOS 27.2"));

    app.press(&format!("id:machine-{MACHINE}")).unwrap();
    app.wait_for("Machine ID").unwrap();
    app.press("id:machine-drawer-close >> role:button").unwrap();
    app.wait_for_gone("Machine ID", Duration::from_secs(2))
        .unwrap();
}

#[test]
fn the_console_speaks_chinese_in_its_tables_and_drawers() {
    let stub = server();
    let mut app = start(
        &Device::desktop(Os::Web, 1200.0).locale("zh-Hans-CN"),
        &stub,
    );
    app.wait_for("登录后管理此服务器").unwrap();
    app.type_text("id:sign-in-username", "admin").unwrap();
    app.type_text("id:sign-in-password", "admin").unwrap();
    app.press("登录").unwrap();
    app.wait_for("example-host-m").unwrap();
    assert!(app.shows("内核版本"));
    app.press(&format!("id:machine-{MACHINE}")).unwrap();
    app.wait_for("设备 ID").unwrap();
    assert!(app.shows("此设备上没有网络"));
}
