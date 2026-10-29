//! The console's REST client on NSH's real engine: `fetch` answered by a stub network, no server involved.
//!
//! A probe component runs the client in a `spawn` task, as the store does, and renders what it saw.

use std::sync::Arc;

use nsh_app_easytierui::core::{
    remote::{RemoteClient, RemoteError, ServerAddress},
    secret::Secret,
};
use nsh_core::prelude::*;
use nsh_engine::{EngineConfig, HttpBody, HttpRequest, Idiom, Os, Points, Root, Size};
use nsh_testing::{Headless, StubNetwork, StubReply, StubResponse, StubRoute};

const BASE: &str = "http://127.0.0.1:41211";
const COOKIE: &str = "example-session-cookie";

fn response(status: u16, headers: &[(&str, &str)], body: &[u8]) -> StubRoute {
    let mut response = StubResponse::default();
    response.status = status;
    response.headers = headers
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    response.body = body.to_vec();
    StubRoute::Response(response)
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/easytier-2.6.4/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// Answers `/machines` only to a request that carries the session cookie the login set, as easytier-web does.
fn machines_behind_the_cookie() -> StubRoute {
    let body = fixture("web-machines.json");
    StubRoute::Handler(Arc::new(move |request: HttpRequest, reply: StubReply| {
        let signed_in = request
            .headers
            .iter()
            .any(|(name, value)| name == "Cookie" && value == &format!("id={COOKIE}"));
        let mut answer = StubResponse::default();
        if signed_in {
            answer.status = 200;
            answer.body = body.clone();
        } else {
            answer.status = 401;
        }
        reply.send(answer);
    }))
}

fn network() -> Arc<StubNetwork> {
    Arc::new(StubNetwork::new([
        (
            format!("POST {BASE}/api/v1/auth/login"),
            response(
                200,
                &[(
                    "Set-Cookie",
                    &format!("id={COOKIE}; HttpOnly; SameSite=Lax; Path=/; Max-Age=86400"),
                )],
                b"{}",
            ),
        ),
        (
            format!("GET {BASE}/api/v1/machines"),
            machines_behind_the_cookie(),
        ),
        // The session ends on the server: unauthenticated REST answers 401 with an empty body.
        (
            format!("GET {BASE}/api/v1/summary"),
            response(401, &[], b""),
        ),
    ]))
}

/// Signs in, lists the machines, then hits an endpoint that answers 401, and describes it all in one line.
async fn scenario() -> String {
    let Ok(address) = ServerAddress::parse(BASE) else {
        return "bad address".to_owned();
    };
    let client = RemoteClient::new(address);
    if let Err(error) = client.login("admin", &Secret::new("admin")).await {
        return format!("login failed: {error}");
    }
    let cookie = if client.has_session_cookie() {
        "held"
    } else {
        "missing"
    };
    let machines = match client.machines().await {
        Ok(machines) => machines
            .iter()
            .map(|machine| format!("{} {}", machine.hostname, machine.os))
            .collect::<Vec<_>>()
            .join(", "),
        Err(error) => return format!("machines failed: {error}"),
    };
    let ended = match client.summary().await {
        Err(RemoteError::Unauthorized) => "unauthorized",
        Err(_) => "other error",
        Ok(_) => "unexpected success",
    };
    let after = if client.has_session_cookie() {
        "held"
    } else {
        "dropped"
    };
    format!(
        "{} signed in, cookie {cookie}; machines: {machines}; 401 is {ended}, cookie {after}",
        client.account().unwrap_or_default()
    )
}

#[component]
fn Probe() -> impl IntoElement {
    let line = signal(String::from("working"));
    spawn(async move {
        let result = scenario().await;
        line.try_set(result);
    });
    text(move || SharedString::from(line.get()))
}

fn start(network: Arc<StubNetwork>) -> Headless {
    let mut config = EngineConfig::new(Root::of::<Probe>());
    let size = Size {
        width: Points(800.0),
        height: Points(600.0),
    };
    config.viewport = size;
    config.environment.window = size;
    config.environment.os = Os::Web;
    config.environment.idiom = Idiom::Desktop;
    config.network = Some(network);
    let mut app = Headless::start(config).unwrap();
    app.first_render().unwrap();
    app
}

#[test]
fn login_keeps_the_cookie_and_lists_machines_then_a_401_drops_it() {
    let stub = network();
    let mut app = start(stub.clone());
    app.wait_for(
        "admin signed in, cookie held; machines: example-host-m macOS 27.2; 401 is unauthorized, cookie dropped",
    )
    .unwrap();

    let requests = stub.requests();
    let login = &requests[0];
    assert_eq!(login.method, "POST");
    let body = match &login.body {
        HttpBody::Bytes(bytes) => String::from_utf8(bytes.clone()).unwrap(),
        other => panic!("login body is {other:?}"),
    };
    // The password travels as its MD5 hex, never as typed.
    let sent: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        sent,
        serde_json::json!({"username": "admin", "password": "21232f297a57a5a743894a0e4a801fc3"})
    );
    // The cookie is sent on the calls after the login and not on the login itself.
    let cookie_of = |index: usize| {
        requests[index]
            .headers
            .iter()
            .find(|(name, _)| name == "Cookie")
            .map(|(_, value)| value.clone())
    };
    assert_eq!(cookie_of(0), None);
    assert_eq!(cookie_of(1), Some(format!("id={COOKIE}")));
    assert_eq!(cookie_of(2), Some(format!("id={COOKIE}")));
}

#[test]
fn a_request_with_no_route_is_a_network_error_not_a_hang() {
    // Nothing answers: the stub fails the request, which is a transport error.
    let mut app = start(Arc::new(
        StubNetwork::new(Vec::<(String, StubRoute)>::new()),
    ));
    app.wait_for("text:/login failed: could not reach the server/")
        .unwrap();
}
