//! easytier-web's REST API as pure data: the requests to send and how a reply is judged, with no I/O.
//!
//! [`client`](super::client) sends these through NSH's `fetch`; the tests here run without an engine.

use std::fmt;

use md5::{Digest, Md5};
use nsh_core::prelude::{HttpRequest, HttpResponse};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    address::ServerAddress, error::RemoteError, ids::UuidQuad, patch::ConfigPatch,
    rpc_model::LogLevel,
};
use crate::core::secret::Secret;

/// The session cookie's name.
const COOKIE_NAME: &str = "id";

/// How a `Set-Cookie` part of the session cookie starts.
const COOKIE_PREFIX: &str = "id=";

/// Longest server message kept in an error.
const MAX_MESSAGE_CHARS: usize = 300;

/// The credential service behind `proxy-rpc`.
const CREDENTIAL_SERVICE: &str = "api.instance.CredentialManageRpcService";

/// The peer service behind `proxy-rpc`.
const PEER_SERVICE: &str = "api.instance.PeerManageRpcService";

/// The peer-center service behind `proxy-rpc` (it takes no instance).
const PEER_CENTER_SERVICE: &str = "api.instance.PeerCenterManageRpcService";

/// The connector service behind `proxy-rpc`.
const CONNECTOR_SERVICE: &str = "api.instance.ConnectorManageRpcService";

/// The mapped-listener service behind `proxy-rpc`.
const MAPPED_LISTENER_SERVICE: &str = "api.instance.MappedListenerManageRpcService";

/// The VPN portal service behind `proxy-rpc`.
const VPN_PORTAL_SERVICE: &str = "api.instance.VpnPortalRpcService";

/// The ACL service behind `proxy-rpc`.
const ACL_SERVICE: &str = "api.instance.AclManageRpcService";

/// The port-forward service behind `proxy-rpc`.
const PORT_FORWARD_SERVICE: &str = "api.instance.PortForwardManageRpcService";

/// The metrics service behind `proxy-rpc`.
const STATS_SERVICE: &str = "api.instance.StatsRpcService";

/// The logger service behind `proxy-rpc` (it takes no instance: the level is the core process's).
const LOGGER_SERVICE: &str = "api.logger.LoggerRpcService";

/// The config service behind `proxy-rpc`.
const CONFIG_SERVICE: &str = "api.config.ConfigRpcService";

/// How easytier-web words a core that did not answer a `proxy-rpc` call in time (`RPC Error: Timeout(Elapsed(()))`).
const PROXY_TIMEOUT_PREFIX: &str = "RPC Error: Timeout";

/// An HTTP method easytier-web uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Method {
    Get,
    Post,
    Put,
    Delete,
}

impl Method {
    /// The method's name on the wire.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
        }
    }
}

/// One call to the REST API: a method, a path below `/api/v1` and an optional JSON body.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ApiRequest {
    /// The method.
    pub method: Method,
    /// The path below `/api/v1`, starting with `/`.
    pub path: String,
    /// The JSON body. It can hold a password digest or a network secret, so `Debug` leaves it out.
    pub body: Option<String>,
}

impl fmt::Debug for ApiRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiRequest")
            .field("method", &self.method)
            .field("path", &self.path)
            .field("body", &self.body.as_ref().map(|_| "***"))
            .finish()
    }
}

impl ApiRequest {
    fn new(method: Method, path: String) -> Self {
        Self {
            method,
            path,
            body: None,
        }
    }

    fn with_body(mut self, body: &Value) -> Self {
        self.body = Some(body.to_string());
        self
    }
}

/// What a reply did to the session cookie.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum CookieUpdate {
    /// No `Set-Cookie` for the session: on the web build the browser holds the cookie and hides the header.
    #[default]
    Unchanged,
    /// A new session cookie value.
    Set(Secret),
    /// The server ended the session (`id=; Max-Age=0`).
    Cleared,
}

/// A reply, reduced to what the client needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApiResponse {
    /// The HTTP status.
    pub status: u16,
    /// The body.
    pub body: Vec<u8>,
    /// What it did to the session cookie.
    pub cookie: CookieUpdate,
}

impl ApiResponse {
    /// Reduces the platform's response.
    pub(crate) fn from_http(response: HttpResponse) -> Self {
        Self {
            status: response.status(),
            cookie: cookie_update(&response.head.headers),
            body: response.body,
        }
    }

    /// The body of a success; otherwise the error the status and body mean.
    pub(crate) fn into_success(self) -> Result<Vec<u8>, RemoteError> {
        check_status(self.status, &self.body)?;
        Ok(self.body)
    }
}

/// Lowercase hex MD5 of a password: what easytier-web's own frontend sends for login, registration and password
/// changes. It is a transport pre-hash (the server stores argon2 of it), not protection: the login is only ever
/// sent over HTTPS or loopback.
pub fn password_digest(password: &Secret) -> String {
    let digest = Md5::digest(password.expose().as_bytes());
    let mut hex = String::with_capacity(digest.len().saturating_mul(2));
    for byte in digest.iter() {
        hex.push(hex_digit(byte >> 4));
        hex.push(hex_digit(byte & 0x0f));
    }
    hex
}

/// The lowercase hex digit of a nibble (`0..=15`).
fn hex_digit(nibble: u8) -> char {
    match nibble {
        0..=9 => char::from(b'0'.saturating_add(nibble)),
        _ => char::from(b'a'.saturating_add(nibble.saturating_sub(10))),
    }
}

// --- Requests -----------------------------------------------------------------------------------------------

/// `POST /auth/login`.
pub(crate) fn login(username: &str, password: &Secret) -> ApiRequest {
    ApiRequest::new(Method::Post, "/auth/login".into()).with_body(&json!({
        "username": username,
        "password": password_digest(password),
    }))
}

/// `GET /auth/logout` (a GET in easytier-web).
pub(crate) fn logout() -> ApiRequest {
    ApiRequest::new(Method::Get, "/auth/logout".into())
}

/// `GET /auth/check_login_status`.
pub(crate) fn check_login_status() -> ApiRequest {
    ApiRequest::new(Method::Get, "/auth/check_login_status".into())
}

/// `PUT /auth/password`.
pub(crate) fn change_password(new_password: &Secret) -> ApiRequest {
    ApiRequest::new(Method::Put, "/auth/password".into())
        .with_body(&json!({ "new_password": password_digest(new_password) }))
}

/// `GET /summary`.
pub(crate) fn summary() -> ApiRequest {
    ApiRequest::new(Method::Get, "/summary".into())
}

/// `GET /sessions`.
pub(crate) fn sessions() -> ApiRequest {
    ApiRequest::new(Method::Get, "/sessions".into())
}

/// `GET /machines`.
pub(crate) fn machines() -> ApiRequest {
    ApiRequest::new(Method::Get, "/machines".into())
}

/// `GET /machines/{mid}/networks`.
pub(crate) fn machine_networks(machine: Uuid) -> ApiRequest {
    ApiRequest::new(Method::Get, format!("/machines/{machine}/networks"))
}

/// `GET /machines/{mid}/networks/info/{inst}`: the per-instance form, because the bulk form is a GET with a JSON
/// body, which browsers forbid.
pub(crate) fn network_info(machine: Uuid, instance: Uuid) -> ApiRequest {
    ApiRequest::new(
        Method::Get,
        format!("/machines/{machine}/networks/info/{instance}"),
    )
}

/// `POST /machines/{mid}/networks/metas`.
pub(crate) fn network_metas(machine: Uuid, instances: &[Uuid]) -> ApiRequest {
    ApiRequest::new(Method::Post, format!("/machines/{machine}/networks/metas"))
        .with_body(&json!({ "instance_ids": instances }))
}

/// `GET /machines/{mid}/networks/config/{inst}`.
pub(crate) fn network_config(machine: Uuid, instance: Uuid) -> ApiRequest {
    ApiRequest::new(
        Method::Get,
        format!("/machines/{machine}/networks/config/{instance}"),
    )
}

/// `POST /machines/{mid}/validate-config`.
pub(crate) fn validate_config(machine: Uuid, config: &Value) -> ApiRequest {
    ApiRequest::new(Method::Post, format!("/machines/{machine}/validate-config"))
        .with_body(&json!({ "config": config }))
}

/// `POST /machines/{mid}/networks`: starts an instance; `save` keeps it for re-push on reconnect.
pub(crate) fn run_network(machine: Uuid, config: &Value, save: bool) -> ApiRequest {
    ApiRequest::new(Method::Post, format!("/machines/{machine}/networks"))
        .with_body(&json!({ "config": config, "save": save }))
}

/// `PUT /machines/{mid}/networks/config/{inst}`; the config's `instance_id` must be `{inst}`.
pub(crate) fn update_network_config(machine: Uuid, instance: Uuid, config: &Value) -> ApiRequest {
    ApiRequest::new(
        Method::Put,
        format!("/machines/{machine}/networks/config/{instance}"),
    )
    .with_body(&json!({ "config": config }))
}

/// `PUT /machines/{mid}/networks/{inst}`.
pub(crate) fn set_network_disabled(machine: Uuid, instance: Uuid, disabled: bool) -> ApiRequest {
    ApiRequest::new(
        Method::Put,
        format!("/machines/{machine}/networks/{instance}"),
    )
    .with_body(&json!({ "disabled": disabled }))
}

/// `DELETE /machines/{mid}/networks/{inst}`.
pub(crate) fn delete_network(machine: Uuid, instance: Uuid) -> ApiRequest {
    ApiRequest::new(
        Method::Delete,
        format!("/machines/{machine}/networks/{instance}"),
    )
}

/// `POST /generate-config` (needs no session).
pub(crate) fn generate_config(config: &Value) -> ApiRequest {
    ApiRequest::new(Method::Post, "/generate-config".into()).with_body(&json!({ "config": config }))
}

/// `POST /parse-config` (needs no session).
pub(crate) fn parse_config(toml: &str) -> ApiRequest {
    ApiRequest::new(Method::Post, "/parse-config".into()).with_body(&json!({ "toml_config": toml }))
}

/// `POST /machines/{mid}/proxy-rpc`: a call to a service of the machine's core.
fn proxy_rpc(machine: Uuid, service: &str, method: &str, payload: Value) -> ApiRequest {
    ApiRequest::new(Method::Post, format!("/machines/{machine}/proxy-rpc")).with_body(&json!({
        "service_name": service,
        "method_name": method,
        "payload": payload,
    }))
}

/// The protobuf JSON that names an instance (`InstanceIdentifier` with its `Id` selector). The core falls back
/// to its only instance when the selector is missing or misspelled, so a wrong key would read the wrong network.
fn instance_selector(instance: Uuid) -> Value {
    json!({ "selector": { "Id": UuidQuad::from(instance) } })
}

/// A `proxy-rpc` call whose payload only names the instance.
fn instance_call(machine: Uuid, service: &str, method: &str, instance: Uuid) -> ApiRequest {
    proxy_rpc(
        machine,
        service,
        method,
        json!({ "instance": instance_selector(instance) }),
    )
}

/// `PeerManageRpcService/ListPeer`.
pub(crate) fn list_peers(machine: Uuid, instance: Uuid) -> ApiRequest {
    instance_call(machine, PEER_SERVICE, "ListPeer", instance)
}

/// `PeerManageRpcService/ShowNodeInfo`.
pub(crate) fn show_node_info(machine: Uuid, instance: Uuid) -> ApiRequest {
    instance_call(machine, PEER_SERVICE, "ShowNodeInfo", instance)
}

/// `PeerManageRpcService/ListRoute`.
pub(crate) fn list_routes(machine: Uuid, instance: Uuid) -> ApiRequest {
    instance_call(machine, PEER_SERVICE, "ListRoute", instance)
}

/// `PeerManageRpcService/ListForeignNetwork`. `include_trusted_keys` is required by the core and sent `false`:
/// the keys are not needed and stay on the machine.
pub(crate) fn list_foreign_networks(machine: Uuid, instance: Uuid) -> ApiRequest {
    proxy_rpc(
        machine,
        PEER_SERVICE,
        "ListForeignNetwork",
        json!({
            "instance": instance_selector(instance),
            "include_trusted_keys": false,
        }),
    )
}

/// `PeerManageRpcService/ListGlobalForeignNetwork`.
pub(crate) fn list_global_foreign_networks(machine: Uuid, instance: Uuid) -> ApiRequest {
    instance_call(machine, PEER_SERVICE, "ListGlobalForeignNetwork", instance)
}

/// `PeerManageRpcService/GetForeignNetworkSummary`.
pub(crate) fn foreign_network_summary(machine: Uuid, instance: Uuid) -> ApiRequest {
    instance_call(machine, PEER_SERVICE, "GetForeignNetworkSummary", instance)
}

/// `PeerManageRpcService/ListPublicIpv6Info`.
pub(crate) fn list_public_ipv6(machine: Uuid, instance: Uuid) -> ApiRequest {
    instance_call(machine, PEER_SERVICE, "ListPublicIpv6Info", instance)
}

/// `PeerCenterManageRpcService/GetGlobalPeerMap`: no instance, and `digest` 0 asks for the whole map.
pub(crate) fn global_peer_map(machine: Uuid) -> ApiRequest {
    proxy_rpc(
        machine,
        PEER_CENTER_SERVICE,
        "GetGlobalPeerMap",
        json!({ "digest": 0 }),
    )
}

/// `ConnectorManageRpcService/ListConnector`.
pub(crate) fn list_connectors(machine: Uuid, instance: Uuid) -> ApiRequest {
    instance_call(machine, CONNECTOR_SERVICE, "ListConnector", instance)
}

/// `MappedListenerManageRpcService/ListMappedListener`.
pub(crate) fn list_mapped_listeners(machine: Uuid, instance: Uuid) -> ApiRequest {
    instance_call(
        machine,
        MAPPED_LISTENER_SERVICE,
        "ListMappedListener",
        instance,
    )
}

/// `VpnPortalRpcService/GetVpnPortalInfo`.
pub(crate) fn vpn_portal_info(machine: Uuid, instance: Uuid) -> ApiRequest {
    instance_call(machine, VPN_PORTAL_SERVICE, "GetVpnPortalInfo", instance)
}

/// `AclManageRpcService/GetAclStats`.
pub(crate) fn acl_stats(machine: Uuid, instance: Uuid) -> ApiRequest {
    instance_call(machine, ACL_SERVICE, "GetAclStats", instance)
}

/// `AclManageRpcService/GetWhitelist`.
pub(crate) fn whitelist(machine: Uuid, instance: Uuid) -> ApiRequest {
    instance_call(machine, ACL_SERVICE, "GetWhitelist", instance)
}

/// `PortForwardManageRpcService/ListPortForward`.
pub(crate) fn list_port_forwards(machine: Uuid, instance: Uuid) -> ApiRequest {
    instance_call(machine, PORT_FORWARD_SERVICE, "ListPortForward", instance)
}

/// `StatsRpcService/GetStats`.
pub(crate) fn stats(machine: Uuid, instance: Uuid) -> ApiRequest {
    instance_call(machine, STATS_SERVICE, "GetStats", instance)
}

/// `StatsRpcService/GetPrometheusStats`.
pub(crate) fn prometheus_stats(machine: Uuid, instance: Uuid) -> ApiRequest {
    instance_call(machine, STATS_SERVICE, "GetPrometheusStats", instance)
}

/// `LoggerRpcService/GetLoggerConfig`: no instance.
pub(crate) fn get_logger_config(machine: Uuid) -> ApiRequest {
    proxy_rpc(machine, LOGGER_SERVICE, "GetLoggerConfig", json!({}))
}

/// `LoggerRpcService/SetLoggerConfig`: no instance; the core sets the level for the whole process.
pub(crate) fn set_logger_config(machine: Uuid, level: LogLevel) -> ApiRequest {
    proxy_rpc(
        machine,
        LOGGER_SERVICE,
        "SetLoggerConfig",
        json!({ "level": level.code() }),
    )
}

/// `ConfigRpcService/GetConfig`: the running config (it includes the network secret).
pub(crate) fn get_config(machine: Uuid, instance: Uuid) -> ApiRequest {
    instance_call(machine, CONFIG_SERVICE, "GetConfig", instance)
}

/// `ConfigRpcService/PatchConfig`: changes the running instance only.
pub(crate) fn patch_config(machine: Uuid, instance: Uuid, patch: &ConfigPatch) -> ApiRequest {
    proxy_rpc(
        machine,
        CONFIG_SERVICE,
        "PatchConfig",
        json!({
            "patch": patch.to_json(),
            "instance": instance_selector(instance),
        }),
    )
}

/// `CredentialManageRpcService/ListCredentials`.
pub(crate) fn list_credentials(machine: Uuid, instance: Uuid) -> ApiRequest {
    instance_call(machine, CREDENTIAL_SERVICE, "ListCredentials", instance)
}

/// What a new credential allows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialRequest {
    /// How long it stays valid; the core rejects 0.
    pub ttl_seconds: u64,
    /// ACL groups for the node that uses it.
    pub groups: Vec<String>,
    /// Whether that node may relay other nodes' traffic.
    pub allow_relay: bool,
    /// Subnets that node may proxy; empty restricts nothing.
    pub allowed_proxy_cidrs: Vec<String>,
    /// A chosen id, reused if it already exists; `None` lets the core pick one.
    pub credential_id: Option<String>,
    /// Whether several nodes may use it at once.
    pub reusable: bool,
}

impl CredentialRequest {
    /// A one-hour, single-use credential with no groups and no relay: the least it can allow.
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            ttl_seconds,
            groups: Vec::new(),
            allow_relay: false,
            allowed_proxy_cidrs: Vec::new(),
            credential_id: None,
            reusable: false,
        }
    }
}

/// `CredentialManageRpcService/GenerateCredential`. Every field is sent: the core reads the JSON into a protobuf
/// struct that requires the non-optional ones (omitting `groups` fails with `missing field`).
pub(crate) fn generate_credential(
    machine: Uuid,
    instance: Uuid,
    request: &CredentialRequest,
) -> ApiRequest {
    proxy_rpc(
        machine,
        CREDENTIAL_SERVICE,
        "GenerateCredential",
        json!({
            "groups": request.groups,
            "allow_relay": request.allow_relay,
            "allowed_proxy_cidrs": request.allowed_proxy_cidrs,
            "ttl_seconds": request.ttl_seconds,
            "credential_id": request.credential_id,
            "reusable": request.reusable,
            "instance": instance_selector(instance),
        }),
    )
}

/// `CredentialManageRpcService/RevokeCredential`.
pub(crate) fn revoke_credential(machine: Uuid, instance: Uuid, credential_id: &str) -> ApiRequest {
    proxy_rpc(
        machine,
        CREDENTIAL_SERVICE,
        "RevokeCredential",
        json!({
            "credential_id": credential_id,
            "instance": instance_selector(instance),
        }),
    )
}

// --- Going on the wire ----------------------------------------------------------------------------------------

/// The platform request for `request` on `address`, with the session cookie when there is one. The result can
/// hold the cookie: never log it.
pub(crate) fn http_request(
    address: &ServerAddress,
    cookie: Option<&Secret>,
    request: &ApiRequest,
) -> HttpRequest {
    let mut http = HttpRequest::new(request.method.as_str(), address.url(&request.path))
        .header("Accept", "application/json");
    if let Some(body) = &request.body {
        http = http
            .header("Content-Type", "application/json")
            .body(body.as_bytes());
    }
    if let Some(cookie) = cookie {
        // The browser forbids this header and sends its own cookie jar's instead; native stacks send it as is.
        http = http.header("Cookie", format!("{COOKIE_NAME}={}", cookie.expose()));
    }
    http
}

// --- Judging replies ------------------------------------------------------------------------------------------

/// What the `Set-Cookie` headers of a reply do to the session cookie (`id`).
///
/// Header values are scanned for an `id=` cookie among their `;` and `,` separated parts: a native stack may
/// join several `Set-Cookie` lines with commas, and `Expires` dates contain one too, which never starts with
/// `id=`. An empty value or `Max-Age=0` ends the session.
pub(crate) fn cookie_update(headers: &[(String, String)]) -> CookieUpdate {
    let mut update = CookieUpdate::Unchanged;
    for (_, value) in headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("set-cookie"))
    {
        let parts: Vec<&str> = value.split([';', ',']).map(str::trim).collect();
        let Some(session) = parts
            .iter()
            .find_map(|part| part.strip_prefix(COOKIE_PREFIX))
        else {
            continue;
        };
        let expired = parts
            .iter()
            .any(|part| part.eq_ignore_ascii_case("max-age=0"));
        update = if session.is_empty() || expired {
            CookieUpdate::Cleared
        } else {
            CookieUpdate::Set(Secret::new(session))
        };
    }
    update
}

/// Judges a status and body: `Ok` for 2xx, otherwise the matching [`RemoteError`].
///
/// - 401 with an empty body: [`RemoteError::Unauthorized`] (no session). With a `{"message"}` body it is a
///   refusal (a wrong login), so [`RemoteError::Rejected`].
/// - 404 with a message (`Client not found`, `Session not found`): [`RemoteError::NotFound`]. A 404 without one
///   is another server or a wrong path: [`RemoteError::Rejected`].
/// - 502 / 504 with easytier-web's JSON `{"message"}` body: [`RemoteError::MachineNoAnswer`] (its call to the
///   machine's core failed or timed out). A proxy's 502 / 504 without that body stays a rejection.
/// - 500 from `proxy-rpc` whose message starts with `RPC Error: Timeout`: the same (easytier-web reports a core
///   that does not answer as a 500 there). Other 500s, such as `Instance not found`, stay rejections.
/// - anything else: [`RemoteError::Rejected`] with the server's `message` (or `error`), or the first line of a
///   plain-text body, or `HTTP <status>`.
pub(crate) fn check_status(status: u16, body: &[u8]) -> Result<(), RemoteError> {
    if (200..300).contains(&status) {
        return Ok(());
    }
    let message = error_message(body);
    let is_json_object = body.trim_ascii_start().starts_with(b"{");
    match (status, message) {
        (401, None) if body.iter().all(u8::is_ascii_whitespace) => Err(RemoteError::Unauthorized),
        (404, Some(message)) => Err(RemoteError::NotFound(message)),
        (502 | 504, Some(message)) if is_json_object => Err(RemoteError::MachineNoAnswer(message)),
        (500, Some(message)) if is_json_object && message.starts_with(PROXY_TIMEOUT_PREFIX) => {
            Err(RemoteError::MachineNoAnswer(message))
        }
        (_, message) => Err(RemoteError::Rejected {
            status,
            message: message.unwrap_or_else(|| format!("HTTP {status}")),
        }),
    }
}

/// The server's words in an error body.
fn error_message(body: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(body);
    let text = text.trim();
    if text.is_empty() || text.starts_with('<') {
        return None;
    }
    let message = match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(object)) => object
            .get("message")
            .or_else(|| object.get("error"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        Ok(_) => None,
        Err(_) => text.lines().next().map(str::to_owned),
    }?;
    let message: String = message.chars().take(MAX_MESSAGE_CHARS).collect();
    (!message.trim().is_empty()).then_some(message)
}

/// Reads a reply body as `T`. The error names the kind and position of the failure, never the offending value.
pub(crate) fn decode<T: DeserializeOwned>(body: &[u8]) -> Result<T, RemoteError> {
    serde_json::from_slice(body).map_err(|error| {
        RemoteError::Decode(format!(
            "{} at line {} column {}",
            decode_kind(&error),
            error.line(),
            error.column()
        ))
    })
}

/// A word for what kind of failure serde_json hit (its own message can quote the value).
fn decode_kind(error: &serde_json::Error) -> &'static str {
    use serde_json::error::Category;
    match error.classify() {
        Category::Io => "read error",
        Category::Syntax => "invalid JSON",
        Category::Data => "unexpected shape",
        Category::Eof => "truncated JSON",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(format!(
            "{}/tests/fixtures/easytier-2.6.4/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    fn headers(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn md5_of_admin_is_what_the_frontend_sends() {
        assert_eq!(
            password_digest(&Secret::new("admin")),
            "21232f297a57a5a743894a0e4a801fc3"
        );
        assert_eq!(
            password_digest(&Secret::new("user")),
            "ee11cbb19052e40b07aac0ca060c23ee"
        );
        assert_eq!(
            password_digest(&Secret::new("")),
            "d41d8cd98f00b204e9800998ecf8427e"
        );
    }

    #[test]
    fn login_body_is_username_and_digest() {
        let request = login("admin", &Secret::new("admin"));
        assert_eq!(request.method, Method::Post);
        assert_eq!(request.path, "/auth/login");
        let body: Value = serde_json::from_str(request.body.as_deref().unwrap()).unwrap();
        assert_eq!(
            body,
            json!({"username": "admin", "password": "21232f297a57a5a743894a0e4a801fc3"})
        );
        // The password itself is nowhere in the request, and Debug hides the digest.
        assert!(
            !request
                .body
                .as_deref()
                .unwrap()
                .contains("\"admin\",\"password\":\"admin\"")
        );
        assert!(!format!("{request:?}").contains("21232f"));
    }

    #[test]
    fn change_password_sends_the_digest_of_the_new_password() {
        let request = change_password(&Secret::new("example-new-pass"));
        assert_eq!(request.method, Method::Put);
        let body: Value = serde_json::from_str(request.body.as_deref().unwrap()).unwrap();
        assert_eq!(
            body,
            json!({"new_password": "38cfe0b2a2e7b9be1c13b2cfbc1c990e"})
        );
    }

    #[test]
    fn requests_use_the_documented_paths() {
        let machine: Uuid = "00000000-0000-0000-0000-000000000001".parse().unwrap();
        let instance: Uuid = "00000000-0000-0000-0000-0000000000aa".parse().unwrap();
        assert_eq!(
            network_info(machine, instance).path,
            "/machines/00000000-0000-0000-0000-000000000001/networks/info/00000000-0000-0000-0000-0000000000aa"
        );
        assert_eq!(network_info(machine, instance).body, None);
        let request = set_network_disabled(machine, instance, true);
        assert_eq!(request.method, Method::Put);
        assert_eq!(request.body.as_deref(), Some("{\"disabled\":true}"));
        assert_eq!(delete_network(machine, instance).method, Method::Delete);
        let metas = network_metas(machine, &[instance]);
        assert_eq!(
            metas.body.as_deref(),
            Some("{\"instance_ids\":[\"00000000-0000-0000-0000-0000000000aa\"]}")
        );
    }

    #[test]
    fn rpc_requests_name_the_instance_by_selector() {
        let machine: Uuid = "00000000-0000-0000-0000-000000000001".parse().unwrap();
        let instance: Uuid = "00000000-0000-0000-0000-0000000000aa".parse().unwrap();
        let request = list_peers(machine, instance);
        let body: Value = serde_json::from_str(request.body.as_deref().unwrap()).unwrap();
        assert_eq!(body["service_name"], "api.instance.PeerManageRpcService");
        assert_eq!(body["method_name"], "ListPeer");
        assert_eq!(
            body["payload"]["instance"]["selector"]["Id"],
            json!({"part1": 0, "part2": 0, "part3": 0, "part4": 170})
        );

        let mut spec = CredentialRequest::new(3600);
        spec.groups = vec!["g1".into()];
        let request = generate_credential(machine, instance, &spec);
        let body: Value = serde_json::from_str(request.body.as_deref().unwrap()).unwrap();
        let payload = &body["payload"];
        // Every field is present, as the core's protobuf JSON requires.
        for key in [
            "groups",
            "allow_relay",
            "allowed_proxy_cidrs",
            "ttl_seconds",
            "credential_id",
            "reusable",
            "instance",
        ] {
            assert!(payload.get(key).is_some(), "{key}");
        }
        assert_eq!(payload["ttl_seconds"], 3600);
        assert_eq!(payload["groups"], json!(["g1"]));
        assert_eq!(payload["credential_id"], Value::Null);
    }

    #[test]
    fn http_requests_carry_the_cookie_and_json_headers() {
        let address = ServerAddress::parse("http://127.0.0.1:11211").unwrap();
        let cookie = Secret::new("abc+/=");
        let http = http_request(
            &address,
            Some(&cookie),
            &login("admin", &Secret::new("admin")),
        );
        assert_eq!(http.method, "POST");
        assert_eq!(http.url, "http://127.0.0.1:11211/api/v1/auth/login");
        let header = |name: &str| {
            http.headers
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(header("Cookie"), Some("id=abc+/="));
        assert_eq!(header("Content-Type"), Some("application/json"));
        let bare = http_request(&address, None, &summary());
        assert!(bare.headers.iter().all(|(n, _)| n != "Cookie"));
        assert!(bare.headers.iter().all(|(n, _)| n != "Content-Type"));
    }

    #[test]
    fn set_cookie_is_read_from_the_login_header() {
        // Captured from easytier-web 2.6.4 on loopback.
        let login = headers(&[
            ("content-type", "application/json"),
            (
                "Set-Cookie",
                "id=V3rNYjDtv+ioySSI+AT7k6hy3Ojdh0Qq/7N3QRBTZT0=Eeq9S0h6M64w6ffVVZfEAA; HttpOnly; SameSite=Lax; Path=/; Max-Age=86400",
            ),
        ]);
        assert_eq!(
            cookie_update(&login),
            CookieUpdate::Set(Secret::new(
                "V3rNYjDtv+ioySSI+AT7k6hy3Ojdh0Qq/7N3QRBTZT0=Eeq9S0h6M64w6ffVVZfEAA"
            ))
        );
    }

    #[test]
    fn logout_clears_and_absence_changes_nothing() {
        let logout = headers(&[(
            "set-cookie",
            "id=; Path=/; Max-Age=0; Expires=Wed, 29 Oct 2025 00:45:03 GMT",
        )]);
        assert_eq!(cookie_update(&logout), CookieUpdate::Cleared);
        assert_eq!(
            cookie_update(&headers(&[("content-type", "application/json")])),
            CookieUpdate::Unchanged
        );
        // Another cookie that merely ends in "id" is not the session.
        assert_eq!(
            cookie_update(&headers(&[("set-cookie", "sid=zzz; Path=/")])),
            CookieUpdate::Unchanged
        );
    }

    #[test]
    fn joined_set_cookie_lines_are_split_on_commas() {
        // NSURLSession joins repeated headers with ", ".
        let joined = headers(&[(
            "Set-Cookie",
            "theme=dark; Expires=Wed, 21 Oct 2026 07:28:00 GMT, id=token123; HttpOnly; Path=/",
        )]);
        assert_eq!(
            cookie_update(&joined),
            CookieUpdate::Set(Secret::new("token123"))
        );
    }

    #[test]
    fn statuses_map_to_errors_the_ui_acts_on() {
        assert_eq!(check_status(200, b"{}"), Ok(()));
        assert_eq!(check_status(200, b""), Ok(()));
        // Unauthenticated REST: 401, empty body.
        assert_eq!(
            check_status(401, &fixture("web-after-logout.json")),
            Err(RemoteError::Unauthorized)
        );
        // Wrong password: 401 with a message.
        assert_eq!(
            check_status(401, &fixture("web-login-bad.json")),
            Err(RemoteError::Rejected {
                status: 401,
                message: "Invalid credentials".into()
            })
        );
        assert_eq!(
            check_status(404, &fixture("web-machine-notfound.json")),
            Err(RemoteError::NotFound("Client not found".into()))
        );
        assert_eq!(
            check_status(400, &fixture("web-save-mismatch.json")),
            Err(RemoteError::Rejected {
                status: 400,
                message: "Instance ID mismatch".into()
            })
        );
        assert_eq!(
            check_status(400, &fixture("web-proxy-rpc-bad.json")),
            Err(RemoteError::Rejected {
                status: 400,
                message: "Unknown service: nope".into()
            })
        );
        // 422 is plain text from axum.
        let Err(RemoteError::Rejected {
            status: 422,
            message,
        }) = check_status(422, &fixture("web-login-malformed.json"))
        else {
            panic!("422 is a rejection");
        };
        assert!(message.starts_with("Failed to deserialize the JSON body"));
        // A proxy's HTML error page says nothing useful; a bare 404 is not "machine gone".
        assert_eq!(
            check_status(502, b"<html>Bad gateway</html>"),
            Err(RemoteError::Rejected {
                status: 502,
                message: "HTTP 502".into()
            })
        );
        assert_eq!(
            check_status(404, b""),
            Err(RemoteError::Rejected {
                status: 404,
                message: "HTTP 404".into()
            })
        );
        // easytier-web's own gateway errors: the machine's core did not answer, the server is fine.
        assert_eq!(
            check_status(504, br#"{"message":"Timeout(Elapsed(()))"}"#),
            Err(RemoteError::MachineNoAnswer("Timeout(Elapsed(()))".into()))
        );
        assert_eq!(
            check_status(502, b""),
            Err(RemoteError::Rejected {
                status: 502,
                message: "HTTP 502".into()
            })
        );
    }

    #[test]
    fn a_proxy_rpc_timeout_is_the_machine_not_answering() {
        // Captured: easytier-web answers 500 when the machine's core stays silent.
        assert_eq!(
            check_status(500, &fixture("web-rpc-proxy-rpc-timeout.json")),
            Err(RemoteError::MachineNoAnswer(
                "RPC Error: Timeout(Elapsed(()))".into()
            ))
        );
        // Other core-side failures are the core's refusal, not silence.
        let unknown = br#"{"message":"RPC Error: ExecutionError(Instance not found or API service not available)"}"#;
        assert!(matches!(
            check_status(500, unknown),
            Err(RemoteError::Rejected { status: 500, .. })
        ));
        assert!(matches!(
            check_status(500, &fixture("web-rpc-tcp-proxy-unavailable.json")),
            Err(RemoteError::Rejected { status: 500, .. })
        ));
        // A 500 that is not easytier-web's JSON stays a rejection even if it starts alike.
        assert!(matches!(
            check_status(500, b"RPC Error: Timeout"),
            Err(RemoteError::Rejected { status: 500, .. })
        ));
    }

    const MACHINE: &str = "00000000-0000-0000-0000-000000000001";
    const INSTANCE: &str = "00000000-0000-0000-0000-0000000000cc";

    fn payload_of(request: &ApiRequest) -> Value {
        let body: Value = serde_json::from_str(request.body.as_deref().unwrap()).unwrap();
        assert_eq!(request.method, Method::Post);
        assert_eq!(request.path, format!("/machines/{MACHINE}/proxy-rpc"));
        body
    }

    #[test]
    fn every_instance_service_names_the_instance_by_id_selector() {
        let machine: Uuid = MACHINE.parse().unwrap();
        let instance: Uuid = INSTANCE.parse().unwrap();
        let calls: Vec<(ApiRequest, &str, &str)> = vec![
            (
                show_node_info(machine, instance),
                "PeerManageRpcService",
                "ShowNodeInfo",
            ),
            (
                list_routes(machine, instance),
                "PeerManageRpcService",
                "ListRoute",
            ),
            (
                list_foreign_networks(machine, instance),
                "PeerManageRpcService",
                "ListForeignNetwork",
            ),
            (
                list_global_foreign_networks(machine, instance),
                "PeerManageRpcService",
                "ListGlobalForeignNetwork",
            ),
            (
                foreign_network_summary(machine, instance),
                "PeerManageRpcService",
                "GetForeignNetworkSummary",
            ),
            (
                list_public_ipv6(machine, instance),
                "PeerManageRpcService",
                "ListPublicIpv6Info",
            ),
            (
                list_connectors(machine, instance),
                "ConnectorManageRpcService",
                "ListConnector",
            ),
            (
                list_mapped_listeners(machine, instance),
                "MappedListenerManageRpcService",
                "ListMappedListener",
            ),
            (
                vpn_portal_info(machine, instance),
                "VpnPortalRpcService",
                "GetVpnPortalInfo",
            ),
            (
                acl_stats(machine, instance),
                "AclManageRpcService",
                "GetAclStats",
            ),
            (
                whitelist(machine, instance),
                "AclManageRpcService",
                "GetWhitelist",
            ),
            (
                list_port_forwards(machine, instance),
                "PortForwardManageRpcService",
                "ListPortForward",
            ),
            (stats(machine, instance), "StatsRpcService", "GetStats"),
            (
                prometheus_stats(machine, instance),
                "StatsRpcService",
                "GetPrometheusStats",
            ),
            (
                get_config(machine, instance),
                "ConfigRpcService",
                "GetConfig",
            ),
            (
                patch_config(machine, instance, &ConfigPatch::new()),
                "ConfigRpcService",
                "PatchConfig",
            ),
        ];
        for (request, service, method) in calls {
            let body = payload_of(&request);
            let name = body["service_name"].as_str().unwrap();
            assert!(name.ends_with(service), "{name}");
            assert!(name.starts_with("api."), "{name}");
            assert_eq!(body["method_name"], method);
            assert_eq!(
                body["payload"]["instance"]["selector"]["Id"],
                json!({"part1": 0, "part2": 0, "part3": 0, "part4": 204}),
                "{method}"
            );
        }
    }

    #[test]
    fn rpc_payloads_the_core_demands_are_complete() {
        let machine: Uuid = MACHINE.parse().unwrap();
        let instance: Uuid = INSTANCE.parse().unwrap();
        // `include_trusted_keys` is required; trusted keys are never asked for.
        let body = payload_of(&list_foreign_networks(machine, instance));
        assert_eq!(body["payload"]["include_trusted_keys"], false);
        // The peer center and the logger take no instance.
        let body = payload_of(&global_peer_map(machine));
        assert_eq!(
            body["service_name"],
            "api.instance.PeerCenterManageRpcService"
        );
        assert_eq!(body["payload"], json!({"digest": 0}));
        let body = payload_of(&get_logger_config(machine));
        assert_eq!(body["service_name"], "api.logger.LoggerRpcService");
        assert_eq!(body["payload"], json!({}));
        let body = payload_of(&set_logger_config(machine, LogLevel::Debug));
        assert_eq!(body["method_name"], "SetLoggerConfig");
        assert_eq!(body["payload"], json!({"level": 4}));
    }

    #[test]
    fn patch_config_payloads_equal_the_captured_requests() {
        use super::super::rpc_model::TransportProtocol::{Tcp, Udp};
        let machine: Uuid = MACHINE.parse().unwrap();
        let instance: Uuid = INSTANCE.parse().unwrap();
        let captured = |name: &str| serde_json::from_slice::<Value>(&fixture(name)).unwrap();

        let mut add = ConfigPatch::new();
        add.hostname("example-host-m2").unwrap();
        add.add_port_forward(Udp, "127.0.0.1:43051", "10.144.145.2:53")
            .unwrap();
        add.add_proxy_network("10.251.0.0/24", None).unwrap();
        add.add_route("10.252.0.0/16").unwrap();
        add.add_connector("tcp://127.0.0.1:43015").unwrap();
        add.add_mapped_listener("tcp://127.0.0.1:43016").unwrap();
        add.add_whitelist_port(Tcp, "80").unwrap();
        add.add_whitelist_port(Udp, "53").unwrap();
        add.add_exit_node("10.144.145.2").unwrap();
        let body = payload_of(&patch_config(machine, instance, &add));
        assert_eq!(
            body["payload"],
            captured("web-rpc-patch-config-request.json")
        );

        let mut remove = ConfigPatch::new();
        remove
            .remove_port_forward(Udp, "127.0.0.1:43051", "10.144.145.2:53")
            .unwrap();
        remove.remove_proxy_network("10.251.0.0/24").unwrap();
        remove.clear_connectors();
        let body = payload_of(&patch_config(machine, instance, &remove));
        assert_eq!(
            body["payload"],
            captured("web-rpc-patch-config-remove-request.json")
        );
    }

    #[test]
    fn decode_errors_never_quote_the_value() {
        let error = decode::<Vec<u32>>(br#"["example-secret-do-not-use"]"#).unwrap_err();
        let RemoteError::Decode(text) = &error else {
            panic!("a decode error");
        };
        assert!(!text.contains("example-secret"), "{text}");
        assert!(text.contains("unexpected shape"));
        assert!(matches!(decode::<u32>(b"{"), Err(RemoteError::Decode(_))));
    }
}
