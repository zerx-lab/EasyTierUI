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
pub(crate) const MACHINE: &str = "00000000-0000-0000-0000-000000000001";
/// The MD5 hex of `admin`, what easytier-web's login takes.
const ADMIN_DIGEST: &str = "21232f297a57a5a743894a0e4a801fc3";
/// The network the captured server pushed to that machine.
pub(crate) const INSTANCE: &str = "00000000-0000-0000-0000-0000000000aa";

pub(crate) fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/easytier-2.6.4/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

pub(crate) fn reply(status: u16, body: &[u8]) -> StubRoute {
    let mut response = StubResponse::default();
    response.status = status;
    response.body = body.to_vec();
    StubRoute::Response(response)
}

/// A route for `path` below `/api/v1`, whatever the server's base is (the web build's is relative).
pub(crate) fn route(method: &str, path: &str, route: StubRoute) -> (String, StubRoute) {
    (format!("{method} *{path}"), route)
}

/// A server that accepts `admin` / `admin` and lists the captured machine, with no network on it.
pub(crate) fn server() -> Arc<StubNetwork> {
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
pub(crate) fn start(device: &Device, network: &Arc<StubNetwork>) -> Headless {
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
pub(crate) fn sign_in(app: &mut Headless) {
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
pub(crate) fn server_with_a_network() -> Arc<StubNetwork> {
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
    network_server(rpc)
}

/// The server of [`server_with_a_network`] with `rpc` answering the calls of `proxy-rpc`.
pub(crate) fn network_server(rpc: StubRoute) -> Arc<StubNetwork> {
    network_server_with_config(rpc, fixture("web-get-config.json"))
}

/// [`network_server`] whose saved config is `config` (the JSON `GET .../networks/config/{inst}` answers); the
/// machine's core accepts the config the editor checks and the replacement it saves.
pub(crate) fn network_server_with_config(rpc: StubRoute, config: Vec<u8>) -> Arc<StubNetwork> {
    Arc::new(StubNetwork::new(network_routes(rpc, config)))
}

/// The routes of [`network_server_with_config`]; a route put before them answers in their place.
fn network_routes(rpc: StubRoute, config: Vec<u8>) -> Vec<(String, StubRoute)> {
    vec![
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
            reply(200, &config),
        ),
        route(
            "PUT",
            &format!("/api/v1/machines/{MACHINE}/networks/config/{INSTANCE}"),
            reply(200, b"{}"),
        ),
        route(
            "POST",
            &format!("/api/v1/machines/{MACHINE}/validate-config"),
            reply(200, &fixture("web-validate.json")),
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
    ]
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
fn a_machine_that_misses_one_read_keeps_its_rows_and_warns_only_when_it_misses_again() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    // The core answers the first info read, then times out as easytier-web reports it (504 from the REST route).
    let reads = Arc::new(AtomicUsize::new(0));
    let info = fixture("web-info-one.json");
    let flaky = StubRoute::Handler(Arc::new(move |_: HttpRequest, reply: StubReply| {
        let mut answer = StubResponse::default();
        if reads.fetch_add(1, Ordering::SeqCst) == 0 {
            answer.status = 200;
            answer.body = info.clone();
        } else {
            answer.status = 504;
            answer.body = br#"{"message":"Timeout(Elapsed(()))"}"#.to_vec();
        }
        reply.send(answer);
    }));
    let mut routes = vec![route(
        "GET",
        &format!("/api/v1/machines/{MACHINE}/networks/info/{INSTANCE}"),
        flaky,
    )];
    routes.extend(network_routes(
        reply(200, b"{}"),
        fixture("web-get-config.json"),
    ));
    let stub = Arc::new(StubNetwork::new(routes));
    let mut app = start(&Device::desktop(Os::Web, 1200.0), &stub);
    sign_in(&mut app);
    app.wait_for("example-host-m").unwrap();
    app.press("id:page-server-networks").unwrap();
    app.wait_for("10.144.145.1/24").unwrap();
    let warning = "Couldn't read the networks of example-host-m.";

    // One missed read: the row stays with its data marked as old, and no warning yet.
    app.press("id:console-refresh >> role:button").unwrap();
    app.wait_for(&format!("id:network-stale-{INSTANCE}"))
        .unwrap();
    assert!(app.shows("10.144.145.1/24"));
    assert!(!app.shows(warning));

    // A second one in a row: the warning with the reason.
    app.press("id:console-refresh >> role:button").unwrap();
    app.wait_for(warning).unwrap();
    assert!(app.shows(
        "The device didn't answer the server. Check that EasyTier runs on it and its connection to the server is \
         stable."
    ));
}

/// A server with the captured network whose core answers the read-only services of `proxy-rpc` with the captured
/// replies, or, with `silent`, does not answer any call (easytier-web's 500 `RPC Error: Timeout`).
fn server_with_a_core(silent: bool) -> Arc<StubNetwork> {
    let answers: Vec<(&'static str, Vec<u8>)> = [
        ("ListPeer", "web-list-peer.json"),
        ("ShowNodeInfo", "web-rpc-show-node-info.json"),
        ("ListRoute", "web-rpc-list-route.json"),
        ("ListForeignNetwork", "web-rpc-list-foreign-network.json"),
        (
            "ListGlobalForeignNetwork",
            "web-rpc-list-global-foreign-network.json",
        ),
        (
            "GetForeignNetworkSummary",
            "web-rpc-foreign-network-summary.json",
        ),
        ("GetGlobalPeerMap", "web-rpc-global-peer-map.json"),
        ("GetStats", "web-rpc-stats.json"),
        ("GetPrometheusStats", "web-rpc-prometheus-stats.json"),
    ]
    .into_iter()
    .map(|(method, file)| (method, fixture(file)))
    .collect();
    let timeout = fixture("web-rpc-proxy-rpc-timeout.json");
    let rpc = StubRoute::Handler(Arc::new(move |request: HttpRequest, reply: StubReply| {
        let method = json_body(&request)["method_name"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let mut answer = StubResponse::default();
        if silent {
            answer.status = 500;
            answer.body = timeout.clone();
        } else {
            answer.status = 200;
            answer.body = answers
                .iter()
                .find(|(known, _)| *known == method)
                .map_or_else(|| br#"{"success":true}"#.to_vec(), |(_, body)| body.clone());
        }
        reply.send(answer);
    }));
    network_server(rpc)
}

/// Signs in, opens the captured machine's drawer and waits for its network to be listed.
fn open_drawer(stub: &Arc<StubNetwork>) -> Headless {
    let mut app = start(&Device::desktop(Os::Web, 1200.0), stub);
    sign_in(&mut app);
    app.wait_for("example-host-m").unwrap();
    app.press(&format!("id:machine-{MACHINE}")).unwrap();
    app.wait_for("Machine ID").unwrap();
    app.wait_for(&format!("id:network-more-{INSTANCE}"))
        .unwrap();
    app
}

/// Opens the tab `tab` (named `label`) from the menu beside the drawer's tab strip.
fn open_more_tab(app: &mut Headless, tab: &str, label: &str) {
    app.press("id:machine-tab-more").unwrap();
    app.wait_for(&format!("id:machine-tab-more-{tab}")).unwrap();
    app.press(&format!("id:machine-tab-more-{tab} >> text:{label}"))
        .unwrap();
}

#[test]
fn the_node_tab_shows_the_virtual_ip_and_the_nat_type_of_the_running_network() {
    let stub = server_with_a_core(false);
    let mut app = open_drawer(&stub);
    app.press("id:machine-tab-node >> text:Node").unwrap();
    app.wait_for("id:node-virtual-ip").unwrap();
    assert!(app.shows("10.144.145.1/24"));
    // The captured STUN result: UDP port-restricted, TCP symmetric.
    app.wait_for("Port-restricted cone").unwrap();
    assert!(app.shows("Symmetric"));
    assert!(app.shows("2.6.4-8428a89d"));
    // The subnet it proxies and a feature flag.
    assert!(app.shows("10.250.0.0/24"));
    assert!(app.shows("Accepts KCP proxy"));

    // The call named the running network, not just any.
    let asked = stub
        .requests()
        .iter()
        .filter(|request| request.url.ends_with("/proxy-rpc"))
        .map(json_body)
        .any(|body| {
            body["method_name"] == "ShowNodeInfo"
                && body["payload"]["instance"]["selector"]["Id"]["part4"] == 170
        });
    assert!(asked);
}

#[test]
fn the_routes_tab_lists_the_peer_with_its_way_and_address() {
    let mut app = open_drawer(&server_with_a_core(false));
    app.press("id:machine-tab-routes >> text:Routes").unwrap();
    app.wait_for("example-host-p").unwrap();
    assert!(app.shows("10.144.145.2/24"));
    assert!(app.screen().matches("text:/Direct/").unwrap());
}

#[test]
fn the_relay_tab_shows_the_other_network_this_node_relays() {
    let mut app = open_drawer(&server_with_a_core(false));
    open_more_tab(&mut app, "relay", "Relay");
    app.wait_for("Networks relayed here").unwrap();
    app.wait_for("id:machine-relay-foreign").unwrap();
    assert!(app.screen().matches("text:/example-other/").unwrap());
    // The peer map lists the mesh's direct links by peer id.
    app.wait_for("id:machine-relay-map").unwrap();
    assert!(app.shows("696501495"));
}

#[test]
fn the_stats_tab_lists_metrics_and_copies_them_as_prometheus_text() {
    let mut app = open_drawer(&server_with_a_core(false));
    open_more_tab(&mut app, "stats", "Stats");
    app.wait_for("compression_bytes_rx_after").unwrap();
    assert!(app.shows("network_name=example-net"));

    // The search keeps the metrics that match.
    app.type_text("id:machine-stats-search", "no-such-metric")
        .unwrap();
    app.wait_for("Nothing matches your search").unwrap();
    assert!(!app.shows("compression_bytes_rx_after"));
    app.press("id:machine-stats-search-clear >> role:button")
        .unwrap();
    app.wait_for("compression_bytes_rx_after").unwrap();

    app.press("id:machine-stats-copy >> role:button").unwrap();
    app.wait_until(
        "the Prometheus text is on the clipboard",
        Duration::from_secs(5),
        |screen| {
            screen
                .clipboard()
                .is_some_and(|text| text.contains("# TYPE compression_bytes_rx_after counter"))
        },
    )
    .unwrap();
}

#[test]
fn a_core_that_does_not_answer_is_named_on_the_tab_not_hidden() {
    let mut app = open_drawer(&server_with_a_core(true));
    app.press("id:machine-tab-node >> text:Node").unwrap();
    app.wait_for("text:/didn't answer the server/").unwrap();
    assert!(app.shows("Couldn't load this."));
    // Another tab says the same; nothing shows as an empty list.
    app.press("id:machine-tab-routes >> text:Routes").unwrap();
    app.wait_for("text:/didn't answer the server/").unwrap();
    assert!(!app.shows("No routes yet"));
}

#[test]
fn the_tab_strip_fits_a_phone_and_the_rest_of_the_tabs_are_in_its_menu() {
    let stub = server_with_a_core(false);
    let mut app = start(&Device::phone(Os::Web), &stub);
    if app.screen().matches("id:page-machines").unwrap() {
        app.press("id:page-machines").unwrap();
    }
    sign_in(&mut app);
    app.wait_for("example-host-m").unwrap();
    app.press(&format!("id:machine-{MACHINE}")).unwrap();
    app.wait_for("Machine ID").unwrap();
    for tab in ["networks", "peers", "node", "routes"] {
        assert!(
            app.screen()
                .matches(&format!("id:machine-tab-{tab}"))
                .unwrap()
        );
    }
    assert!(app.screen().matches("id:machine-tab-more").unwrap());
    open_more_tab(&mut app, "stats", "Stats");
    app.wait_for("compression_bytes_rx_after").unwrap();
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
    // The tab strip and its menu are in Chinese too.
    assert!(app.shows("路由"));
    app.press("id:machine-tab-more").unwrap();
    app.wait_for("id:machine-tab-more-relay").unwrap();
    assert!(app.shows("中继"));
}

// --- The runtime tabs: changes to a running network ---------------------------------------------------------------

/// A server with the captured network whose core answers the runtime tabs' reads with the captured replies and
/// accepts every change (the captured `PatchConfig` reply is `{}`).
fn server_with_a_runtime_core() -> Arc<StubNetwork> {
    let answers: Vec<(&'static str, Vec<u8>)> = [
        ("ListPeer", "web-list-peer.json"),
        ("ListConnector", "web-rpc-list-connector-patched.json"),
        ("ListMappedListener", "web-rpc-list-mapped-listener.json"),
        ("ListPortForward", "web-rpc-list-port-forward.json"),
        ("GetConfig", "web-rpc-get-config.json"),
        ("GetVpnPortalInfo", "web-rpc-vpn-portal.json"),
        ("GetWhitelist", "web-rpc-whitelist-patched.json"),
        ("GetAclStats", "web-rpc-acl-stats.json"),
        ("GetLoggerConfig", "web-rpc-logger-get.json"),
        ("PatchConfig", "web-rpc-patch-config.json"),
    ]
    .into_iter()
    .map(|(method, file)| (method, fixture(file)))
    .collect();
    let rpc = StubRoute::Handler(Arc::new(move |request: HttpRequest, reply: StubReply| {
        let method = json_body(&request)["method_name"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let mut answer = StubResponse::default();
        answer.status = 200;
        answer.body = answers
            .iter()
            .find(|(known, _)| *known == method)
            .map_or_else(|| br#"{"success":true}"#.to_vec(), |(_, body)| body.clone());
        reply.send(answer);
    }));
    network_server(rpc)
}

/// The bodies of the `proxy-rpc` calls to `method` that `stub` received, in order.
fn rpc_calls(stub: &StubNetwork, method: &str) -> Vec<Value> {
    stub.requests()
        .iter()
        .filter(|request| request.url.ends_with("/proxy-rpc"))
        .map(json_body)
        .filter(|body| body["method_name"] == method)
        .collect()
}

/// Waits until `stub` received `count` calls to `method`.
fn wait_for_calls(app: &mut Headless, stub: &Arc<StubNetwork>, method: &'static str, count: usize) {
    let stub = stub.clone();
    app.wait_until(
        &format!("{count} call(s) to {method}"),
        Duration::from_secs(5),
        move |_| rpc_calls(&stub, method).len() >= count,
    )
    .unwrap();
}

/// The `bind` / `dst` / `socket_type` object of a port forward, as the core's JSON writes it.
fn forward_json(bind_port: u16, dst_port: u16, socket_type: u8) -> Value {
    json!({
        "bind_addr": {"ip": {"Ipv4": {"addr": 2_130_706_433_u32}}, "port": bind_port},
        "dst_addr": {"ip": {"Ipv4": {"addr": 177_246_466_u32}}, "port": dst_port},
        "socket_type": socket_type,
    })
}

#[test]
fn adding_a_port_forward_sends_a_patch_with_the_forward_and_all_six_lists() {
    let stub = server_with_a_runtime_core();
    let mut app = open_drawer(&stub);
    open_more_tab(&mut app, "forwarding", "Forwarding");
    app.wait_for("id:forward-bind").unwrap();
    // The forward the core already has is listed, and the tab says what a change does and does not do.
    app.wait_for(r"text:/127\.0\.0\.1:43050/").unwrap();
    assert!(
        app.screen()
            .matches("text:/apply now, to the running network only/")
            .unwrap()
    );
    // The captured config has manual routing off: the routes section says a route added here does nothing.
    app.wait_for("id:routes-off-note").unwrap();

    app.type_text("id:forward-bind", "127.0.0.1:2222").unwrap();
    app.type_text("id:forward-dst", "10.144.145.2:80").unwrap();
    app.press("id:forward-add >> role:button").unwrap();
    wait_for_calls(&mut app, &stub, "PatchConfig", 1);

    let calls = rpc_calls(&stub, "PatchConfig");
    assert_eq!(calls.len(), 1);
    let payload = &calls[0]["payload"];
    let patch = &payload["patch"];
    assert_eq!(
        patch["port_forwards"],
        json!([{"action": 0, "cfg": forward_json(2222, 80, 0)}])
    );
    // The core needs all six repeated lists, empty or not; nothing else changes.
    for list in [
        "proxy_networks",
        "routes",
        "exit_nodes",
        "mapped_listeners",
        "connectors",
    ] {
        assert_eq!(patch[list], json!([]), "{list}");
    }
    assert!(patch.get("acl").is_none());
    assert!(patch.get("hostname").is_none());
    // It names the running network.
    assert_eq!(payload["instance"]["selector"]["Id"]["part4"], 170);
    // The list is read again afterwards.
    wait_for_calls(&mut app, &stub, "ListPortForward", 2);
}

#[test]
fn removing_a_port_forward_asks_first_naming_it() {
    let stub = server_with_a_runtime_core();
    let mut app = open_drawer(&stub);
    app.answer_dialogs_myself();
    open_more_tab(&mut app, "forwarding", "Forwarding");
    app.wait_for(r"text:/127\.0\.0\.1:43050/").unwrap();

    let remove = "id:port-forwards-remove-TCP-127-0-0-1-43050-10-144-145-2-22 >> role:button";
    app.press(remove).unwrap();
    let said = {
        let dialog = app.pending_dialog().unwrap();
        format!(
            "{} {}",
            dialog.title.clone().unwrap_or_default(),
            dialog.message.clone().unwrap_or_default()
        )
    };
    assert!(
        said.contains("TCP")
            && said.contains("127.0.0.1:43050")
            && said.contains("10.144.145.2:22"),
        "{said}"
    );
    assert!(rpc_calls(&stub, "PatchConfig").is_empty());
    // Cancelling removes nothing.
    app.answer_dialog(DialogAnswer::Press("Cancel".to_owned()))
        .unwrap();
    app.wait(Duration::from_millis(200)).unwrap();
    assert!(rpc_calls(&stub, "PatchConfig").is_empty());

    app.press(remove).unwrap();
    app.answer_dialog(DialogAnswer::Press("Remove".to_owned()))
        .unwrap();
    wait_for_calls(&mut app, &stub, "PatchConfig", 1);
    let calls = rpc_calls(&stub, "PatchConfig");
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0]["payload"]["patch"]["port_forwards"],
        json!([{"action": 1, "cfg": forward_json(43050, 22, 0)}])
    );
}

#[test]
fn an_invalid_connector_url_is_refused_on_the_form_without_a_request() {
    let stub = server_with_a_runtime_core();
    let mut app = open_drawer(&stub);
    open_more_tab(&mut app, "connections", "Connections");
    app.wait_for("id:connector-url").unwrap();
    // The connector the core has is listed with its link.
    app.wait_for(r"text:/127\.0\.0\.1:43015/").unwrap();
    assert!(app.shows("Connected"));

    app.type_text("id:connector-url", "not a url").unwrap();
    app.press("id:connector-add >> role:button").unwrap();
    app.wait_for("text:/The request isn't valid/").unwrap();
    app.wait(Duration::from_millis(200)).unwrap();
    assert!(rpc_calls(&stub, "PatchConfig").is_empty());

    // Fixing the value sends it.
    app.clear("id:connector-url").unwrap();
    app.type_text("id:connector-url", "tcp://203.0.113.5:11010")
        .unwrap();
    app.press("id:connector-add >> role:button").unwrap();
    wait_for_calls(&mut app, &stub, "PatchConfig", 1);
    let calls = rpc_calls(&stub, "PatchConfig");
    assert_eq!(
        calls[0]["payload"]["patch"]["connectors"],
        json!([{"action": 0, "url": {"url": "tcp://203.0.113.5:11010"}}])
    );
    // The refusal of the earlier value is gone once a value was accepted.
    app.wait_for_gone("text:/The request isn't valid/", Duration::from_secs(5))
        .unwrap();
}

#[test]
fn clearing_the_mapped_listeners_asks_first_and_sends_a_clear() {
    let stub = server_with_a_runtime_core();
    let mut app = open_drawer(&stub);
    app.answer_dialogs_myself();
    open_more_tab(&mut app, "connections", "Connections");
    app.wait_for(r"text:/127\.0\.0\.1:43013/").unwrap();

    app.press("id:mapped-listeners-clear >> role:button")
        .unwrap();
    assert!(app.pending_dialog().is_some());
    assert!(rpc_calls(&stub, "PatchConfig").is_empty());
    app.answer_dialog(DialogAnswer::Press("Remove all".to_owned()))
        .unwrap();
    wait_for_calls(&mut app, &stub, "PatchConfig", 1);
    let calls = rpc_calls(&stub, "PatchConfig");
    // A CLEAR carries a url the core can parse: an unparsable one would stop the whole core.
    assert_eq!(
        calls[0]["payload"]["patch"]["mapped_listeners"],
        json!([{"action": 2, "url": {"url": "tcp://0.0.0.0:0"}}])
    );
    assert_eq!(calls[0]["payload"]["patch"]["connectors"], json!([]));
}

#[test]
fn the_vpn_client_config_with_its_private_key_shows_only_after_the_button() {
    let stub = server_with_a_runtime_core();
    let mut app = open_drawer(&stub);
    open_more_tab(&mut app, "forwarding", "Forwarding");
    app.wait_for("id:vpn-config-secret").unwrap();
    assert!(app.shows("wireguard"));
    // The portal was read, but its config is nowhere on screen.
    assert!(
        !app.screen()
            .matches("text:/example-private-key-do-not-use/")
            .unwrap()
    );
    assert!(!app.screen().matches("text:/PrivateKey/").unwrap());

    app.press("id:vpn-config-reveal >> role:button").unwrap();
    app.wait_for("text:/example-private-key-do-not-use/")
        .unwrap();
    assert!(
        app.screen()
            .matches("text:/This is a private key/")
            .unwrap()
    );

    app.press("id:vpn-config-hide >> role:button").unwrap();
    app.wait_for_gone(
        "text:/example-private-key-do-not-use/",
        Duration::from_secs(2),
    )
    .unwrap();
}

#[test]
fn changing_the_log_level_sends_set_logger_config_for_the_whole_core() {
    let stub = server_with_a_runtime_core();
    let mut app = open_drawer(&stub);
    open_more_tab(&mut app, "logs", "Logs");
    app.wait_for("id:log-level").unwrap();
    assert!(
        app.screen()
            .matches("text:/belongs to the machine's whole core/")
            .unwrap()
    );

    app.press("id:log-level").unwrap();
    app.wait_for("id:log-level-3").unwrap();
    app.press("id:log-level-3 >> text:/./").unwrap();
    wait_for_calls(&mut app, &stub, "SetLoggerConfig", 1);
    let calls = rpc_calls(&stub, "SetLoggerConfig");
    // The level is the core's: the call names no network.
    assert_eq!(calls[0]["payload"], json!({"level": 3}));
    // The level is read back.
    wait_for_calls(&mut app, &stub, "GetLoggerConfig", 2);
}

#[test]
fn opening_a_port_sends_the_whitelist_patch_and_the_acl_counters_show() {
    let stub = server_with_a_runtime_core();
    let mut app = open_drawer(&stub);
    open_more_tab(&mut app, "access", "Access");
    app.wait_for("id:whitelist-port").unwrap();
    // The open ports and the ACL's global counters, both captured.
    app.wait_for("id:whitelist-row-TCP-80").unwrap();
    app.wait_for("CacheMaxSize").unwrap();
    assert!(app.shows("1024"));

    app.type_text("id:whitelist-port", "8080").unwrap();
    app.press("id:whitelist-add >> role:button").unwrap();
    wait_for_calls(&mut app, &stub, "PatchConfig", 1);
    let patch = rpc_calls(&stub, "PatchConfig")[0]["payload"]["patch"].clone();
    assert_eq!(
        patch["acl"],
        json!({
            "tcp_whitelist": [{"action": 0, "value": "8080"}],
            "udp_whitelist": [],
        })
    );
    assert_eq!(patch["connectors"], json!([]));
}

#[test]
fn the_unsaved_note_opens_the_network_config_editor() {
    let stub = server_with_a_runtime_core();
    let mut app = open_drawer(&stub);
    open_more_tab(&mut app, "connections", "Connections");
    app.wait_for("id:runtime-note").unwrap();
    app.press("Edit config").unwrap();
    app.wait_for("Edit network").unwrap();
    app.wait_for("id:editor-name").unwrap();
}

#[test]
fn the_runtime_tabs_speak_chinese() {
    let stub = server_with_a_runtime_core();
    let mut app = start(
        &Device::desktop(Os::Web, 1200.0).locale("zh-Hans-CN"),
        &stub,
    );
    app.wait_for("登录后管理此服务器").unwrap();
    app.type_text("id:sign-in-username", "admin").unwrap();
    app.type_text("id:sign-in-password", "admin").unwrap();
    app.press("登录").unwrap();
    app.wait_for("example-host-m").unwrap();
    app.press(&format!("id:machine-{MACHINE}")).unwrap();
    app.wait_for(&format!("id:network-more-{INSTANCE}"))
        .unwrap();
    app.press("id:machine-tab-more").unwrap();
    app.wait_for("id:machine-tab-more-connections").unwrap();
    app.press("id:machine-tab-more-connections >> text:/./")
        .unwrap();
    app.wait_for("id:connector-url").unwrap();
    assert!(app.screen().matches("text:/这里的更改立即生效/").unwrap());
    assert!(app.shows("映射监听器"));
}

// ----- The network editor covers every setting -----

/// The captured network's saved config, as the server sends it.
fn saved_config() -> Value {
    serde_json::from_slice(&fixture("web-get-config.json")).unwrap()
}

/// A server that saves `config` as the captured network's config, with the config editor open on it.
fn open_editor(config: &Value) -> (Arc<StubNetwork>, Headless) {
    let rpc = reply(200, br#"{"success":true}"#);
    let stub = network_server_with_config(rpc, serde_json::to_vec(config).unwrap());
    let mut app = start(&Device::desktop(Os::Web, 1200.0), &stub);
    sign_in(&mut app);
    app.wait_for("example-host-m").unwrap();
    app.press("id:page-server-networks").unwrap();
    app.wait_for(&format!("id:network-{INSTANCE}")).unwrap();
    app.press(&format!("id:network-{INSTANCE}")).unwrap();
    app.wait_for("Edit network").unwrap();
    app.wait_for("id:editor-name").unwrap();
    (stub, app)
}

/// Presses Save and returns the config the server received as the network's replacement.
fn save_edit(app: &mut Headless, stub: &Arc<StubNetwork>) -> Value {
    app.press("Save changes").unwrap();
    app.wait_for_gone("Edit network", Duration::from_secs(5))
        .unwrap();
    let requests = stub.requests();
    let put = requests
        .iter()
        .find(|request| {
            request.method == "PUT"
                && request
                    .url
                    .ends_with(&format!("/machines/{MACHINE}/networks/config/{INSTANCE}"))
        })
        .unwrap();
    json_body(put)["config"].clone()
}

/// The keys whose value differs between two configs (a key only one has counts), sorted.
fn changed_keys(before: &Value, after: &Value) -> Vec<String> {
    let empty = serde_json::Map::new();
    let before = before.as_object().unwrap_or(&empty);
    let after = after.as_object().unwrap_or(&empty);
    let mut keys: Vec<String> = before
        .keys()
        .chain(after.keys())
        .filter(|key| before.get(*key) != after.get(*key))
        .cloned()
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

/// Whether a request that checks a config reached the server.
fn validated(stub: &StubNetwork) -> bool {
    stub.requests()
        .iter()
        .any(|request| request.url.ends_with("/validate-config"))
}

#[test]
fn saving_an_edited_config_sends_exactly_the_keys_that_were_changed() {
    let saved = saved_config();
    // An untouched form: what the editor writes back for a config it did not change.
    let baseline = {
        let (stub, mut app) = open_editor(&saved);
        save_edit(&mut app, &stub)
    };
    assert_eq!(baseline["network_secret"], saved["network_secret"]);
    assert_eq!(baseline["hostname"], saved["hostname"]);
    assert_eq!(baseline["listener_urls"], saved["listener_urls"]);
    assert_eq!(baseline["mtu"], saved["mtu"]);
    assert_eq!(baseline["advanced_settings"], saved["advanced_settings"]);

    let (stub, mut app) = open_editor(&saved);
    // Every section is in the form.
    for section in ["Connectivity", "Behavior", "Sharing", "Relaying"] {
        assert!(app.shows(section), "the form has a {section} section");
    }
    app.wait_for("id:editor-acl-switch").unwrap();
    app.press("id:editor-flag-latency_first >> role:switch")
        .unwrap();
    app.press("id:editor-vpn >> role:switch").unwrap();
    app.wait_for("id:editor-vpn-cidr").unwrap();
    app.type_text("id:editor-vpn-cidr", "10.14.14.0/24")
        .unwrap();
    app.clear("id:editor-vpn-port").unwrap();
    app.type_text("id:editor-vpn-port", "22222").unwrap();
    app.type_text("id:editor-exit-nodes", "10.144.145.2")
        .unwrap();
    app.type_text("id:editor-forwards", "tcp 127.0.0.1:8080 10.144.145.2:80")
        .unwrap();
    let edited = save_edit(&mut app, &stub);

    assert_eq!(
        changed_keys(&baseline, &edited),
        [
            "enable_vpn_portal",
            "exit_nodes",
            "latency_first",
            "port_forwards",
            "vpn_portal_client_network_addr",
            "vpn_portal_client_network_len",
            "vpn_portal_listen_port",
        ]
    );
    assert_eq!(edited["latency_first"], json!(true));
    assert_eq!(edited["enable_vpn_portal"], json!(true));
    assert_eq!(edited["vpn_portal_client_network_addr"], "10.14.14.0");
    assert_eq!(edited["vpn_portal_client_network_len"], json!(24));
    assert_eq!(edited["vpn_portal_listen_port"], json!(22222));
    assert_eq!(edited["exit_nodes"], json!(["10.144.145.2"]));
    assert_eq!(
        edited["port_forwards"],
        json!([{
            "bind_ip": "127.0.0.1",
            "bind_port": 8080,
            "dst_ip": "10.144.145.2",
            "dst_port": 80,
            "proto": "tcp",
        }])
    );
}

#[test]
fn an_acl_chain_with_a_rule_is_saved_as_acl_v1_with_easytiers_numbers() {
    let (stub, mut app) = open_editor(&saved_config());
    app.press("id:editor-acl-switch >> role:switch").unwrap();
    app.wait_for("id:editor-acl-add-inbound").unwrap();
    app.press("id:editor-acl-add-inbound >> role:button")
        .unwrap();
    app.wait_for("id:editor-acl-chain-0-add-rule").unwrap();
    app.press("id:editor-acl-chain-0-add-rule >> role:button")
        .unwrap();
    app.wait_for("id:editor-acl-rule-dialog").unwrap();
    app.type_text("id:editor-acl-rule-name", "allow-office")
        .unwrap();
    app.type_text("id:editor-acl-rule-source-ips", "10.144.145.7")
        .unwrap();
    app.press("id:editor-acl-rule-save >> role:button").unwrap();
    app.wait_for_gone("id:editor-acl-rule-dialog", Duration::from_secs(2))
        .unwrap();
    app.wait_for("id:editor-acl-rule-0-0").unwrap();

    let config = save_edit(&mut app, &stub);
    let chain = &config["acl"]["acl_v1"]["chains"][0];
    // 1 inbound; allow when no rule matches is 1; the rule allows (1) every protocol (5).
    assert_eq!(chain["chain_type"], json!(1));
    assert_eq!(chain["enabled"], json!(true));
    assert_eq!(chain["default_action"], json!(1));
    assert_eq!(
        config["acl"]["acl_v1"]["chains"].as_array().map(Vec::len),
        Some(1)
    );
    let rule = &chain["rules"][0];
    assert_eq!(rule["name"], "allow-office");
    assert_eq!(rule["priority"], json!(1000));
    assert_eq!(rule["action"], json!(1));
    assert_eq!(rule["protocol"], json!(5));
    assert_eq!(rule["source_ips"], json!(["10.144.145.7"]));
}

#[test]
fn an_acl_the_app_cannot_read_shows_a_note_and_is_sent_back_unchanged() {
    let mut config = saved_config();
    config["acl"] = json!({
        "acl_v1": {"chains": [{
            "name": "from-a-newer-server",
            "chain_type": 9,
            "enabled": true,
            "default_action": 1,
            "rules": [],
        }]}
    });
    let kept = config["acl"].clone();
    let (stub, mut app) = open_editor(&config);
    app.wait_for("id:editor-acl-unreadable").unwrap();
    app.wait_for("text:/can't be edited here/").unwrap();
    assert!(!app.screen().matches("id:editor-acl-switch").unwrap());

    let saved = save_edit(&mut app, &stub);
    assert_eq!(saved["acl"], kept);
}

#[test]
fn an_invalid_exit_node_or_route_blocks_the_save_and_names_the_field() {
    let (stub, mut app) = open_editor(&saved_config());
    app.type_text("id:editor-exit-nodes", "not-an-ip").unwrap();
    app.press("Save changes").unwrap();
    app.wait_for("id:editor-exit-nodes-error").unwrap();
    app.wait_for("text:/isn't a valid exit node/").unwrap();
    assert!(!validated(&stub));

    app.clear("id:editor-exit-nodes").unwrap();
    app.press("id:editor-routes-on >> role:switch").unwrap();
    app.wait_for("id:editor-routes").unwrap();
    app.type_text("id:editor-routes", "192.168.0.0/33").unwrap();
    app.press("Save changes").unwrap();
    app.wait_for("id:editor-routes-error").unwrap();
    app.wait_for("text:/isn't a valid route/").unwrap();
    assert!(!validated(&stub));
    assert!(app.shows("Edit network"), "the form stays open");
}
