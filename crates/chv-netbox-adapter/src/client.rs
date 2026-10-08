//! Thin NetBox REST client for the projection (mapping contract v1,
//! "NetBox REST surface used").
//!
//! Bounded to the six endpoints of the contract — devices, virtual
//! machines, virtualization interfaces, prefixes, vlans, ip-addresses —
//! with one method per need and nothing generic. Every response is
//! parsed fail-closed: only the contract fields are read, required
//! fields must be present, and any object shape outside the contract is
//! an error, never a best-effort parse.
//!
//! # Security
//!
//! - **HTTPS only**: [`NetBoxClient::new`] rejects non-`https://`
//!   endpoints with [`ClientError::HttpsRequired`] — there is no
//!   production plain-HTTP escape hatch. (A `test-http` feature exists
//!   so wiremock integration tests can exercise the real request path
//!   over HTTP; it is dev-dependency-only.)
//! - The API token lives in [`NetBoxToken`], whose `Debug`/`Display`
//!   print `<redacted>`. It is attached as `Authorization: Token …` on
//!   every request and **never** appears in any error `Display`, log
//!   line, or parsed value by construction — token material only ever
//!   flows into the header via [`NetBoxToken::expose`].

use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use serde_json::{json, Value};
use thiserror::Error;

use crate::mapping::{NetBoxKind, NetBoxObject};
use crate::plan::NetBoxRemoteObject;

/// Page size for list requests.
pub const PAGE_LIMIT: usize = 50;
/// Hard bound on followed `next` links — fail closed instead of paging
/// forever on a hostile or misbehaving endpoint.
pub const MAX_PAGES: usize = 10;

/// Total per-request timeout (matches the component spec's bounded
/// dry-run/export budget).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Placeholder mirroring `crate::mapping`'s `UNSET` for optional content
/// fields: the pure core diffs flattened string maps, so an absent
/// remote value must render exactly like an absent desired value.
const UNSET: &str = "(unset)";

/// Cap on error bodies echoed into [`ClientError::Api`] messages.
const MAX_ERROR_BODY: usize = 512;

// ---------------------------------------------------------------------------
// Token
// ---------------------------------------------------------------------------

/// Opaque wrapper for the NetBox API token.
///
/// `Debug` and `Display` are manually implemented to print
/// `"<redacted>"` so the token can never leak through a `{:?}`/`{}`
/// format (log lines, panic messages, test failures). Plaintext is only
/// reachable through [`NetBoxToken::expose`].
#[derive(Clone)]
pub struct NetBoxToken(String);

impl NetBoxToken {
    pub fn new(token: String) -> Self {
        Self(token)
    }

    /// Return the plaintext token. **HTTP header use only** — the value
    /// must never be logged, stored in an error, or embedded in an
    /// outcome/event.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for NetBoxToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("\"<redacted>\"")
    }
}

impl fmt::Display for NetBoxToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Structured client failures. No variant ever carries token material:
/// the token is header-only, so neither URLs nor bodies nor error text
/// can contain it.
#[derive(Debug, Error)]
pub enum ClientError {
    #[error("netbox endpoint must use https (NETBOX_HTTPS_REQUIRED): {endpoint}")]
    HttpsRequired { endpoint: String },

    #[error("invalid netbox endpoint: {reason}")]
    InvalidEndpoint { reason: String },

    #[error("netbox authentication failed (401/403); check the configured token")]
    AuthFailed,

    #[error("netbox unreachable: {reason}")]
    Unreachable { reason: String },

    #[error("netbox api error (status {status}): {message}")]
    Api { status: u16, message: String },

    #[error("netbox response violated the mapping contract: {reason}")]
    MalformedResponse { reason: String },

    #[error("netbox pagination exceeded {max_pages} pages; refusing to continue")]
    PaginationLimit { max_pages: usize },

    #[error("netbox lookup for {query} matched more than one object (ambiguous remote state)")]
    AmbiguousLookup { query: String },

    #[error("failed to build http client: {reason}")]
    ClientBuild { reason: String },
}

// ---------------------------------------------------------------------------
// Wire shape
// ---------------------------------------------------------------------------

/// A remote NetBox object paired with its NetBox row id.
///
/// The pure core's [`NetBoxRemoteObject`] is deliberately id-free
/// (plans are pure data); mutations need the id, so the client carries
/// it alongside.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteNetBoxObject {
    pub netbox_id: i64,
    pub object: NetBoxRemoteObject,
}

/// API path for a kind (mapping contract "NetBox REST surface used").
const fn kind_api_path(kind: NetBoxKind) -> &'static str {
    match kind {
        NetBoxKind::Device => "/api/dcim/devices/",
        NetBoxKind::VirtualMachine => "/api/virtualization/virtual-machines/",
        NetBoxKind::Interface => "/api/virtualization/interfaces/",
        NetBoxKind::Prefix => "/api/ipam/prefixes/",
        NetBoxKind::Vlan => "/api/ipam/vlans/",
        NetBoxKind::IpAddress => "/api/ipam/ip-addresses/",
    }
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// NetBox REST client. Clone-able (the underlying `reqwest::Client`
/// pools connections).
#[derive(Clone)]
pub struct NetBoxClient {
    /// Scheme-normalized base URL without a trailing slash, e.g.
    /// `https://netbox.example.com`.
    base: String,
    token: NetBoxToken,
    http: reqwest::Client,
}

impl fmt::Debug for NetBoxClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetBoxClient")
            .field("base", &self.base)
            .field("token", &self.token)
            .finish_non_exhaustive()
    }
}

impl NetBoxClient {
    /// Build a client for `endpoint`. **HTTPS only** (fail-closed at
    /// construction; the component spec forbids plain-HTTP transport
    /// for the token): any non-`https` scheme is
    /// [`ClientError::HttpsRequired`].
    pub fn new(endpoint: &str, token: NetBoxToken) -> Result<Self, ClientError> {
        let base = normalize_endpoint(endpoint, true)?;
        Self::with_base(base, token)
    }

    /// Test-only constructor that accepts plain-HTTP endpoints so the
    /// wiremock integration suites can exercise the real request path
    /// (wiremock serves plain HTTP). Guarded behind the `test-http`
    /// feature, which only dev-dependencies enable.
    #[cfg(any(test, feature = "test-http"))]
    pub fn new_unchecked_for_tests(
        endpoint: &str,
        token: NetBoxToken,
    ) -> Result<Self, ClientError> {
        let base = normalize_endpoint(endpoint, false)?;
        Self::with_base(base, token)
    }

    fn with_base(base: String, token: NetBoxToken) -> Result<Self, ClientError> {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .map_err(|err| ClientError::ClientBuild {
                reason: err.to_string(),
            })?;
        Ok(Self { base, token, http })
    }

    // -- request plumbing --------------------------------------------------

    /// Execute one authenticated request and map failures to structured
    /// errors. 2xx responses are parsed as JSON; empty bodies (204)
    /// become `Value::Null`.
    async fn request(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<&Value>,
    ) -> Result<Value, ClientError> {
        let mut request = self.http.request(method, url).header(
            reqwest::header::AUTHORIZATION,
            format!("Token {}", self.token.expose()),
        );
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request
            .send()
            .await
            .map_err(|err| ClientError::Unreachable {
                reason: err.to_string(),
            })?;
        let status = response.status().as_u16();
        if (200..300).contains(&status) {
            let bytes = response
                .bytes()
                .await
                .map_err(|err| ClientError::MalformedResponse {
                    reason: format!("failed to read response body: {err}"),
                })?;
            if bytes.is_empty() {
                return Ok(Value::Null);
            }
            serde_json::from_slice(&bytes).map_err(|err| ClientError::MalformedResponse {
                reason: format!("response is not valid JSON: {err}"),
            })
        } else if status == 401 || status == 403 {
            Err(ClientError::AuthFailed)
        } else {
            let message = response
                .text()
                .await
                .unwrap_or_default()
                .chars()
                .take(MAX_ERROR_BODY)
                .collect();
            Err(ClientError::Api { status, message })
        }
    }

    /// Build a list URL with URL-encoded query parameters.
    fn list_url(&self, kind: NetBoxKind, params: &[(&str, &str)]) -> Result<String, ClientError> {
        let base = format!("{}{}", self.base, kind_api_path(kind));
        let url = reqwest::Url::parse_with_params(&base, params).map_err(|err| {
            ClientError::InvalidEndpoint {
                reason: format!("failed to build list url: {err}"),
            }
        })?;
        Ok(url.into())
    }

    /// Fetch every page of a list query, following `next` links, bounded
    /// at [`MAX_PAGES`]. Each result row is parsed fail-closed into a
    /// [`RemoteNetBoxObject`].
    async fn list(
        &self,
        kind: NetBoxKind,
        params: &[(&str, &str)],
    ) -> Result<Vec<RemoteNetBoxObject>, ClientError> {
        let mut url = self.list_url(kind, params)?;
        let mut objects = Vec::new();
        for _ in 0..MAX_PAGES {
            let page = self
                .request(reqwest::Method::GET, &url, None)
                .await
                .and_then(|body| parse_list_page(&body))?;
            for row in &page.results {
                objects.push(parse_remote(kind, row)?);
            }
            match page.next {
                Some(next) => {
                    // The `next` link is server-controlled: only follow
                    // links that stay inside our endpoint.
                    if !next.starts_with(&self.base) {
                        return Err(ClientError::MalformedResponse {
                            reason: format!("pagination 'next' link escapes the endpoint: {next}"),
                        });
                    }
                    url = next;
                }
                None => return Ok(objects),
            }
        }
        Err(ClientError::PaginationLimit {
            max_pages: MAX_PAGES,
        })
    }

    /// Natural-key lookup: `None` when nothing matches, an error when
    /// the query is ambiguous (more than one match — fail closed, the
    /// natural key must identify at most one object).
    async fn lookup_one(
        &self,
        kind: NetBoxKind,
        params: &[(&str, &str)],
        query: &str,
    ) -> Result<Option<RemoteNetBoxObject>, ClientError> {
        let limit = PAGE_LIMIT.to_string();
        let mut all = params.to_vec();
        all.push(("limit", limit.as_str()));
        let url = self.list_url(kind, &all)?;
        let page = self
            .request(reqwest::Method::GET, &url, None)
            .await
            .and_then(|body| parse_list_page(&body))?;
        if page.results.len() > 1 || page.results.len() >= PAGE_LIMIT {
            return Err(ClientError::AmbiguousLookup {
                query: query.to_string(),
            });
        }
        match page.results.first() {
            None => Ok(None),
            Some(row) => Ok(Some(parse_remote(kind, row)?)),
        }
    }

    // -- list by architecture (owned + stale candidates) -------------------

    /// Shared implementation of the six `list_*_by_architecture`
    /// methods: custom-field filter `?cf_<field>=<architecture_id>`
    /// (the mapping contract's filtering surface). Covers owned objects
    /// and stale candidates alike — both carry the architecture-id
    /// custom field.
    async fn list_by_architecture(
        &self,
        kind: NetBoxKind,
        arch_custom_field: &str,
        architecture_id: &str,
    ) -> Result<Vec<RemoteNetBoxObject>, ClientError> {
        let cf_field = arch_custom_field_cf(arch_custom_field);
        self.list(kind, &[(cf_field.as_str(), architecture_id)])
            .await
    }

    /// All devices of this architecture. See
    /// [`NetBoxClient::list_by_architecture`].
    pub async fn list_devices_by_architecture(
        &self,
        arch_custom_field: &str,
        architecture_id: &str,
    ) -> Result<Vec<RemoteNetBoxObject>, ClientError> {
        self.list_by_architecture(NetBoxKind::Device, arch_custom_field, architecture_id)
            .await
    }

    /// All virtual machines of this architecture.
    pub async fn list_virtual_machines_by_architecture(
        &self,
        arch_custom_field: &str,
        architecture_id: &str,
    ) -> Result<Vec<RemoteNetBoxObject>, ClientError> {
        self.list_by_architecture(
            NetBoxKind::VirtualMachine,
            arch_custom_field,
            architecture_id,
        )
        .await
    }

    /// All interfaces of this architecture.
    pub async fn list_interfaces_by_architecture(
        &self,
        arch_custom_field: &str,
        architecture_id: &str,
    ) -> Result<Vec<RemoteNetBoxObject>, ClientError> {
        self.list_by_architecture(NetBoxKind::Interface, arch_custom_field, architecture_id)
            .await
    }

    /// All prefixes of this architecture.
    pub async fn list_prefixes_by_architecture(
        &self,
        arch_custom_field: &str,
        architecture_id: &str,
    ) -> Result<Vec<RemoteNetBoxObject>, ClientError> {
        self.list_by_architecture(NetBoxKind::Prefix, arch_custom_field, architecture_id)
            .await
    }

    /// All VLANs of this architecture.
    pub async fn list_vlans_by_architecture(
        &self,
        arch_custom_field: &str,
        architecture_id: &str,
    ) -> Result<Vec<RemoteNetBoxObject>, ClientError> {
        self.list_by_architecture(NetBoxKind::Vlan, arch_custom_field, architecture_id)
            .await
    }

    /// All IP addresses of this architecture.
    pub async fn list_ip_addresses_by_architecture(
        &self,
        arch_custom_field: &str,
        architecture_id: &str,
    ) -> Result<Vec<RemoteNetBoxObject>, ClientError> {
        self.list_by_architecture(NetBoxKind::IpAddress, arch_custom_field, architecture_id)
            .await
    }

    // -- natural-key lookups (foreign-occupancy detection) ------------------

    /// Device by `name` (the Device natural key).
    pub async fn get_device_by_name(
        &self,
        name: &str,
    ) -> Result<Option<RemoteNetBoxObject>, ClientError> {
        self.lookup_one(
            NetBoxKind::Device,
            &[("name", name)],
            &format!("device name={name}"),
        )
        .await
    }

    /// VirtualMachine by `name` (the VM natural key).
    pub async fn get_virtual_machine_by_name(
        &self,
        name: &str,
    ) -> Result<Option<RemoteNetBoxObject>, ClientError> {
        self.lookup_one(
            NetBoxKind::VirtualMachine,
            &[("name", name)],
            &format!("virtual machine name={name}"),
        )
        .await
    }

    /// Interface by `name` + parent virtual machine name (the Interface
    /// natural key).
    pub async fn get_interface_by_name(
        &self,
        name: &str,
        virtual_machine: &str,
    ) -> Result<Option<RemoteNetBoxObject>, ClientError> {
        self.lookup_one(
            NetBoxKind::Interface,
            &[("name", name), ("virtual_machine", virtual_machine)],
            &format!("interface name={name} virtual_machine={virtual_machine}"),
        )
        .await
    }

    /// Prefix by CIDR (the Prefix natural key).
    pub async fn get_prefix_by_cidr(
        &self,
        prefix: &str,
    ) -> Result<Option<RemoteNetBoxObject>, ClientError> {
        self.lookup_one(
            NetBoxKind::Prefix,
            &[("prefix", prefix)],
            &format!("prefix={prefix}"),
        )
        .await
    }

    /// VLAN by `vid`. The pure core's VLAN natural key is the vid alone
    /// (mapping contract: `vid` (+ group)), so the occupancy probe must
    /// filter on vid only — a `name` filter would miss a foreign VLAN
    /// squatting on the same vid and turn a detectable conflict into a
    /// doomed create.
    pub async fn get_vlan_by_vid(
        &self,
        vid: u32,
    ) -> Result<Option<RemoteNetBoxObject>, ClientError> {
        self.lookup_one(
            NetBoxKind::Vlan,
            &[("vid", &vid.to_string())],
            &format!("vlan vid={vid}"),
        )
        .await
    }

    /// IPAddress by `address` (maskless). Remote addresses carry a
    /// NetBox mask suffix; parsing normalizes both sides so the natural
    /// key comparison is mask-independent.
    pub async fn get_ip_address_by_address(
        &self,
        address: &str,
    ) -> Result<Option<RemoteNetBoxObject>, ClientError> {
        self.lookup_one(
            NetBoxKind::IpAddress,
            &[("address", address)],
            &format!("ip address={address}"),
        )
        .await
    }

    // -- mutations -----------------------------------------------------------

    /// Create an object (POST the mapped fields + custom fields).
    /// Returns the NetBox id of the created row.
    pub async fn create_object(&self, object: &NetBoxObject) -> Result<i64, ClientError> {
        let body = self.build_body(object).await?;
        let url = format!("{}{}", self.base, kind_api_path(object.kind()));
        let response = self
            .request(reqwest::Method::POST, &url, Some(&body))
            .await?;
        response
            .get("id")
            .and_then(Value::as_i64)
            .ok_or_else(|| ClientError::MalformedResponse {
                reason: "create response carries no id".to_string(),
            })
    }

    /// Patch an object by its NetBox id with the desired object's mapped
    /// fields + custom fields.
    pub async fn update_object(
        &self,
        netbox_id: i64,
        object: &NetBoxObject,
    ) -> Result<(), ClientError> {
        let body = self.build_body(object).await?;
        let url = format!("{}{}{netbox_id}/", self.base, kind_api_path(object.kind()));
        self.request(reqwest::Method::PATCH, &url, Some(&body))
            .await?;
        Ok(())
    }

    /// Mark a chv-owned object stale: PATCH the managed-state custom
    /// field plus NetBox's decommissioning status for devices and
    /// virtual machines (mapping contract "Retention"; other kinds have
    /// no decommissioning status and only carry the custom field).
    pub async fn mark_stale(
        &self,
        netbox_id: i64,
        kind: NetBoxKind,
        managed_state_field: &str,
    ) -> Result<(), ClientError> {
        let mut body = json!({
            "custom_fields": { (managed_state_field): "stale" },
        });
        if matches!(kind, NetBoxKind::Device | NetBoxKind::VirtualMachine) {
            body["status"] = json!("decommissioning");
        }
        let url = format!("{}{}{netbox_id}/", self.base, kind_api_path(kind));
        self.request(reqwest::Method::PATCH, &url, Some(&body))
            .await?;
        Ok(())
    }

    /// Delete an object by its NetBox id. The runner only calls this
    /// under `delete` retention and only after re-verifying ownership
    /// from the remote marker (belt-and-braces on top of the pure plan).
    pub async fn delete_object(&self, netbox_id: i64, kind: NetBoxKind) -> Result<(), ClientError> {
        let url = format!("{}{}{netbox_id}/", self.base, kind_api_path(kind));
        self.request(reqwest::Method::DELETE, &url, None).await?;
        Ok(())
    }

    /// Build the write body for one object. Nullable facts stay unset
    /// (`null`/omitted) — contract rule 2. IP-address assignment is
    /// resolved through an interface lookup: NetBox assigns IPs to
    /// interfaces by id, and the contract's kind order creates IP
    /// addresses *before* their interfaces, so the lookup may
    /// legitimately miss (the runner performs an assignment fix-up pass
    /// after the main loop).
    async fn build_body(&self, object: &NetBoxObject) -> Result<Value, ClientError> {
        match object {
            NetBoxObject::Device(d) => Ok(json!({
                "name": d.name,
                "site": d.site.as_ref().map(|s| json!({ "name": s })),
                "status": d.status.as_str(),
                "tags": d.tags,
                "custom_fields": d.custom_fields,
            })),
            NetBoxObject::VirtualMachine(v) => Ok(json!({
                "name": v.name,
                "status": v.status.as_str(),
                "cluster": v.cluster.as_ref().map(|c| json!({ "name": c })),
                "device": v.device.as_ref().map(|d| json!({ "name": d })),
                "vcpus": v.cpu,
                "memory": v.memory_mb,
                "tags": v.tags,
                "custom_fields": v.custom_fields,
            })),
            NetBoxObject::Interface(i) => Ok(json!({
                "name": i.name,
                "virtual_machine": { "name": i.virtual_machine },
                "description": i.description,
                "type": "virtual",
                "tags": i.tags,
                "custom_fields": i.custom_fields,
            })),
            NetBoxObject::Prefix(p) => Ok(json!({
                "prefix": p.prefix,
                "vlan": p.vlan.map(|vid| json!({ "vid": vid })),
                "description": p.description,
                "tags": p.tags,
                "custom_fields": p.custom_fields,
            })),
            NetBoxObject::Vlan(v) => Ok(json!({
                "vid": v.vid,
                "name": v.name,
                "tags": v.tags,
                "custom_fields": v.custom_fields,
            })),
            NetBoxObject::IpAddress(a) => {
                let mut body = json!({
                    "address": a.address,
                    "tags": a.tags,
                    "custom_fields": a.custom_fields,
                });
                if let Some(assigned) = &a.assigned_to_interface {
                    // "<vm>/<interface>" → the interface's NetBox id.
                    if let Some((vm, iface)) = assigned.split_once('/') {
                        if let Some(remote) = self.get_interface_by_name(iface, vm).await? {
                            body["assigned_object_type"] = json!("virtualization.vminterface");
                            body["assigned_object_id"] = json!(remote.netbox_id);
                        }
                    }
                }
                Ok(body)
            }
        }
    }
}

/// The NetBox custom-field filter form: `?cf_<field>=`.
fn arch_custom_field_cf(field: &str) -> String {
    format!("cf_{field}")
}

/// Normalize an endpoint: parse it, enforce the scheme, strip a trailing
/// slash.
fn normalize_endpoint(endpoint: &str, require_https: bool) -> Result<String, ClientError> {
    let url = reqwest::Url::parse(endpoint).map_err(|err| ClientError::InvalidEndpoint {
        reason: err.to_string(),
    })?;
    if require_https && url.scheme() != "https" {
        return Err(ClientError::HttpsRequired {
            endpoint: endpoint.to_string(),
        });
    }
    if !matches!(url.scheme(), "https" | "http") {
        return Err(ClientError::InvalidEndpoint {
            reason: format!("unsupported scheme {:?}", url.scheme()),
        });
    }
    let mut serialized = url.to_string();
    if serialized.ends_with('/') {
        serialized.pop();
    }
    Ok(serialized)
}

// ---------------------------------------------------------------------------
// Fail-closed wire parsing
// ---------------------------------------------------------------------------

/// One list page. `next` and `results` are the only fields consumed;
/// `results` is required (its absence is a contract violation).
#[derive(serde::Deserialize)]
struct ListPage {
    next: Option<String>,
    results: Vec<Value>,
}

fn parse_list_page(body: &Value) -> Result<ListPage, ClientError> {
    serde_json::from_value(body.clone()).map_err(|err| ClientError::MalformedResponse {
        reason: format!("list response is not a paginated result set: {err}"),
    })
}

/// Parse one remote object of `kind` — the wire JSON → pure core
/// conversion, fail-closed on required fields.
pub fn parse_remote(kind: NetBoxKind, body: &Value) -> Result<RemoteNetBoxObject, ClientError> {
    let netbox_id = req_i64(body, "id")?;
    let object = match kind {
        NetBoxKind::Device => NetBoxRemoteObject {
            kind,
            natural_key: [("name".to_string(), req_str(body, "name")?)].into(),
            custom_fields: parse_custom_fields(body)?,
            content: device_content(body)?,
        },
        NetBoxKind::VirtualMachine => NetBoxRemoteObject {
            kind,
            natural_key: [("name".to_string(), req_str(body, "name")?)].into(),
            custom_fields: parse_custom_fields(body)?,
            content: vm_content(body)?,
        },
        NetBoxKind::Interface => {
            let vm = req_nested_name(body, "virtual_machine")?;
            NetBoxRemoteObject {
                kind,
                natural_key: [
                    ("name".to_string(), req_str(body, "name")?),
                    ("virtual_machine".to_string(), vm.clone()),
                ]
                .into(),
                custom_fields: parse_custom_fields(body)?,
                content: interface_content(body, vm)?,
            }
        }
        NetBoxKind::Prefix => NetBoxRemoteObject {
            kind,
            natural_key: [("prefix".to_string(), req_str(body, "prefix")?)].into(),
            custom_fields: parse_custom_fields(body)?,
            content: prefix_content(body)?,
        },
        NetBoxKind::Vlan => {
            let vid = req_i64(body, "vid")?;
            NetBoxRemoteObject {
                kind,
                natural_key: [("vid".to_string(), vid.to_string())].into(),
                custom_fields: parse_custom_fields(body)?,
                content: vlan_content(body, vid)?,
            }
        }
        NetBoxKind::IpAddress => {
            let raw_address = req_str(body, "address")?;
            let address = strip_address_mask(&raw_address);
            NetBoxRemoteObject {
                kind,
                natural_key: [("address".to_string(), address.to_string())].into(),
                custom_fields: parse_custom_fields(body)?,
                content: ip_address_content(body, address)?,
            }
        }
    };
    Ok(RemoteNetBoxObject { netbox_id, object })
}

fn device_content(body: &Value) -> Result<BTreeMap<String, String>, ClientError> {
    Ok([
        ("name".to_string(), req_str(body, "name")?),
        (
            "site".to_string(),
            opt_nested_name(body, "site").unwrap_or_else(|| UNSET.to_string()),
        ),
        ("status".to_string(), status_value(body)?),
        ("tags".to_string(), join_tags(body)?),
    ]
    .into())
}

fn vm_content(body: &Value) -> Result<BTreeMap<String, String>, ClientError> {
    Ok([
        ("name".to_string(), req_str(body, "name")?),
        ("status".to_string(), status_value(body)?),
        (
            "cluster".to_string(),
            opt_nested_name(body, "cluster").unwrap_or_else(|| UNSET.to_string()),
        ),
        (
            "device".to_string(),
            opt_nested_name(body, "device").unwrap_or_else(|| UNSET.to_string()),
        ),
        (
            "cpu".to_string(),
            opt_number(body, "vcpus").unwrap_or_else(|| UNSET.to_string()),
        ),
        (
            "memory_mb".to_string(),
            opt_number(body, "memory").unwrap_or_else(|| UNSET.to_string()),
        ),
        ("tags".to_string(), join_tags(body)?),
    ]
    .into())
}

fn interface_content(
    body: &Value,
    virtual_machine: String,
) -> Result<BTreeMap<String, String>, ClientError> {
    Ok([
        ("name".to_string(), req_str(body, "name")?),
        ("virtual_machine".to_string(), virtual_machine),
        (
            "description".to_string(),
            opt_str_field(body, "description").unwrap_or_default(),
        ),
        ("tags".to_string(), join_tags(body)?),
    ]
    .into())
}

fn prefix_content(body: &Value) -> Result<BTreeMap<String, String>, ClientError> {
    let vlan = match body.get("vlan") {
        None | Some(Value::Null) => UNSET.to_string(),
        Some(v) => {
            let vid = v.get("vid").and_then(Value::as_i64).ok_or_else(|| {
                ClientError::MalformedResponse {
                    reason: "prefix vlan object carries no vid".to_string(),
                }
            })?;
            vid.to_string()
        }
    };
    Ok([
        ("prefix".to_string(), req_str(body, "prefix")?),
        ("vlan".to_string(), vlan),
        (
            "description".to_string(),
            opt_str_field(body, "description").unwrap_or_default(),
        ),
        ("tags".to_string(), join_tags(body)?),
    ]
    .into())
}

fn vlan_content(body: &Value, vid: i64) -> Result<BTreeMap<String, String>, ClientError> {
    Ok([
        ("vid".to_string(), vid.to_string()),
        ("name".to_string(), req_str(body, "name")?),
        ("tags".to_string(), join_tags(body)?),
    ]
    .into())
}

fn ip_address_content(
    body: &Value,
    address: &str,
) -> Result<BTreeMap<String, String>, ClientError> {
    let assigned = assigned_interface(body).unwrap_or_else(|| UNSET.to_string());
    Ok([
        ("address".to_string(), address.to_string()),
        ("assigned_to_interface".to_string(), assigned),
        ("tags".to_string(), join_tags(body)?),
    ]
    .into())
}

// -- parsing helpers ---------------------------------------------------------

fn req_str(body: &Value, field: &'static str) -> Result<String, ClientError> {
    body.get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ClientError::MalformedResponse {
            reason: format!("required field {field:?} is missing or not a string"),
        })
}

fn req_i64(body: &Value, field: &'static str) -> Result<i64, ClientError> {
    body.get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| ClientError::MalformedResponse {
            reason: format!("required field {field:?} is missing or not an integer"),
        })
}

fn req_nested_name(body: &Value, field: &'static str) -> Result<String, ClientError> {
    body.get(field)
        .and_then(|v| v.get("name"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ClientError::MalformedResponse {
            reason: format!("required nested field {field}.name is missing or not a string"),
        })
}

fn opt_str_field(body: &Value, field: &str) -> Option<String> {
    body.get(field).and_then(Value::as_str).map(str::to_string)
}

fn opt_nested_name(body: &Value, field: &str) -> Option<String> {
    body.get(field)?.get("name")?.as_str().map(str::to_string)
}

fn opt_number(body: &Value, field: &str) -> Option<String> {
    body.get(field)
        .and_then(Value::as_number)
        .map(number_to_string)
}

fn status_value(body: &Value) -> Result<String, ClientError> {
    body.get("status")
        .and_then(|s| s.get("value"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ClientError::MalformedResponse {
            reason: "required field \"status\".\"value\" is missing".to_string(),
        })
}

/// Tags arrive either as plain strings (list endpoints) or nested
/// objects (detail endpoints); both NetBox forms are accepted, anything
/// else fails closed.
fn join_tags(body: &Value) -> Result<String, ClientError> {
    let Some(tags) = body.get("tags").and_then(Value::as_array) else {
        return Ok(String::new());
    };
    let mut out = Vec::with_capacity(tags.len());
    for tag in tags {
        match tag {
            Value::String(s) => out.push(s.clone()),
            Value::Object(obj) => {
                let name = obj
                    .get("slug")
                    .or_else(|| obj.get("name"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| ClientError::MalformedResponse {
                        reason: "tag object carries neither slug nor name".to_string(),
                    })?;
                out.push(name.to_string());
            }
            _ => {
                return Err(ClientError::MalformedResponse {
                    reason: "tag is neither a string nor an object".to_string(),
                })
            }
        }
    }
    Ok(out.join(","))
}

/// Custom fields: only scalar values are inside the contract. Nulls are
/// skipped (nullable facts stay unset); arrays/objects fail closed.
fn parse_custom_fields(body: &Value) -> Result<BTreeMap<String, String>, ClientError> {
    let mut out = BTreeMap::new();
    let Some(Value::Object(map)) = body.get("custom_fields") else {
        return Ok(out);
    };
    for (key, value) in map {
        match value {
            Value::String(s) => {
                out.insert(key.clone(), s.clone());
            }
            Value::Number(n) => {
                out.insert(key.clone(), number_to_string(n));
            }
            Value::Bool(b) => {
                out.insert(key.clone(), b.to_string());
            }
            Value::Null => {}
            _ => {
                return Err(ClientError::MalformedResponse {
                    reason: format!(
                    "custom field {key:?} carries a non-scalar value; outside the mapping contract"
                ),
                })
            }
        }
    }
    Ok(out)
}

/// Numbers render like the pure core's `u32::to_string` wherever the
/// value is integral (`2.0` → `"2"`), so live-fact diffs compare equal.
fn number_to_string(n: &serde_json::Number) -> String {
    if let Some(i) = n.as_i64() {
        i.to_string()
    } else if let Some(f) = n.as_f64() {
        if f.fract() == 0.0 {
            format!("{}", f as i64)
        } else {
            f.to_string()
        }
    } else {
        n.to_string()
    }
}

/// `"<vm>/<interface>"` when the address is assigned to a VM interface;
/// `None` otherwise (unassigned or assigned to a non-VM object — the
/// diff treats both as unset for our purposes).
fn assigned_interface(body: &Value) -> Option<String> {
    let assigned = body.get("assigned_object")?;
    if !assigned.is_object() {
        return None;
    }
    let iface = assigned.get("name")?.as_str()?;
    let vm = assigned.get("virtual_machine")?.get("name")?.as_str()?;
    Some(format!("{vm}/{iface}"))
}

/// NetBox returns addresses with a mask suffix (`10.0.0.1/24`); the
/// pure core's natural key and content are maskless.
fn strip_address_mask(address: &str) -> &str {
    match address.split_once('/') {
        Some((ip, _)) => ip,
        None => address,
    }
}

// ---------------------------------------------------------------------------
// Tests — wire fixtures, redaction, HTTPS enforcement (no HTTP needed)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_debug_and_display_are_redacted() {
        let token = NetBoxToken::new("super-secret-netbox-token".to_string());
        assert_eq!(format!("{token:?}"), "\"<redacted>\"");
        assert_eq!(format!("{token}"), "<redacted>");
        // Nested formatting must not leak either.
        let client_dbg = format!(
            "{:?}",
            NetBoxClient::new_unchecked_for_tests("http://127.0.0.1:1", token.clone())
                .expect("test client")
        );
        assert!(!client_dbg.contains("super-secret-netbox-token"));
    }

    #[test]
    fn https_is_enforced_at_construction() {
        let err = NetBoxClient::new("http://netbox.example.com", NetBoxToken::new("t".into()))
            .expect_err("plain http must be rejected");
        assert!(matches!(err, ClientError::HttpsRequired { .. }));
        // No scheme / garbage → invalid endpoint, still no client.
        assert!(matches!(
            NetBoxClient::new("not-a-url", NetBoxToken::new("t".into())),
            Err(ClientError::InvalidEndpoint { .. })
        ));
        // Trailing slash is normalized away.
        let client = NetBoxClient::new("https://netbox.example.com/", NetBoxToken::new("t".into()))
            .expect("https accepted");
        assert_eq!(client.base, "https://netbox.example.com");
    }

    #[test]
    fn device_fixture_parses_to_remote_object() {
        let body = json!({
            "id": 7,
            "url": "https://netbox.example.com/api/dcim/devices/7/",
            "display": "chv-node-01",
            "name": "chv-node-01",
            "status": { "value": "active", "label": "Active" },
            "site": { "id": 1, "name": "dc1", "slug": "dc1" },
            "tags": ["chv-team", "chv-env-production"],
            "custom_fields": {
                "chv_external_id": "arch:arch_01HX:server/chv-node-01:3",
                "chv_managed_by": "chv",
                "chv_cpu_cores": 8,
                "foreign_field": null
            },
            "serial": "", "device_type": { "name": "chv-host" }
        });
        let remote = parse_remote(NetBoxKind::Device, &body).expect("parses");
        assert_eq!(remote.netbox_id, 7);
        assert_eq!(remote.object.kind, NetBoxKind::Device);
        assert_eq!(
            remote.object.natural_key,
            [("name".to_string(), "chv-node-01".to_string())].into()
        );
        // Numbers stringify so they diff equal against the pure core's
        // u32 rendering.
        assert_eq!(
            remote
                .object
                .custom_fields
                .get("chv_cpu_cores")
                .map(String::as_str),
            Some("8")
        );
        assert_eq!(
            remote.object.content.get("site").map(String::as_str),
            Some("dc1")
        );
        assert_eq!(
            remote.object.content.get("tags").map(String::as_str),
            Some("chv-team,chv-env-production")
        );
        // Nulls are skipped, not stored as "null".
        assert!(!remote.object.custom_fields.contains_key("foreign_field"));
    }

    #[test]
    fn virtual_machine_fixture_parses_with_declared_resources() {
        let body = json!({
            "id": 9,
            "name": "vm-01",
            "status": { "value": "active", "label": "Active" },
            "cluster": null,
            "device": { "id": 7, "name": "chv-node-01" },
            "vcpus": 2.0,
            "memory": 2048,
            "tags": [],
            "custom_fields": {}
        });
        let remote = parse_remote(NetBoxKind::VirtualMachine, &body).expect("parses");
        assert_eq!(
            remote.object.content.get("cpu").map(String::as_str),
            Some("2")
        );
        assert_eq!(
            remote.object.content.get("memory_mb").map(String::as_str),
            Some("2048")
        );
        assert_eq!(
            remote.object.content.get("cluster").map(String::as_str),
            Some("(unset)")
        );
        assert_eq!(
            remote.object.content.get("device").map(String::as_str),
            Some("chv-node-01")
        );
    }

    #[test]
    fn interface_fixture_parses_with_parent_natural_key() {
        let body = json!({
            "id": 11,
            "name": "backend",
            "virtual_machine": { "id": 9, "name": "vm-01" },
            "description": "backend",
            "type": "virtual",
            "tags": [{ "slug": "chv-team", "name": "chv-team" }]
        });
        let remote = parse_remote(NetBoxKind::Interface, &body).expect("parses");
        assert_eq!(
            remote.object.natural_key,
            [
                ("name".to_string(), "backend".to_string()),
                ("virtual_machine".to_string(), "vm-01".to_string())
            ]
            .into()
        );
        // Nested tag objects are accepted (detail-endpoint form).
        assert_eq!(
            remote.object.content.get("tags").map(String::as_str),
            Some("chv-team")
        );
    }

    #[test]
    fn prefix_fixture_parses_vlan_link() {
        let body = json!({
            "id": 3,
            "prefix": "10.42.0.0/24",
            "vlan": { "id": 42, "vid": 42, "name": "backend" },
            "description": "backend (vlan)",
            "tags": []
        });
        let remote = parse_remote(NetBoxKind::Prefix, &body).expect("parses");
        assert_eq!(
            remote.object.natural_key,
            [("prefix".to_string(), "10.42.0.0/24".to_string())].into()
        );
        assert_eq!(
            remote.object.content.get("vlan").map(String::as_str),
            Some("42")
        );
    }

    #[test]
    fn vlan_fixture_parses_vid_natural_key() {
        let body = json!({ "id": 42, "vid": 42, "name": "backend", "tags": [] });
        let remote = parse_remote(NetBoxKind::Vlan, &body).expect("parses");
        assert_eq!(
            remote.object.natural_key,
            [("vid".to_string(), "42".to_string())].into()
        );
    }

    #[test]
    fn ip_address_fixture_strips_mask_and_parses_assignment() {
        let body = json!({
            "id": 21,
            "address": "10.42.0.5/24",
            "assigned_object": {
                "id": 11,
                "name": "backend",
                "virtual_machine": { "id": 9, "name": "vm-01" }
            },
            "tags": []
        });
        let remote = parse_remote(NetBoxKind::IpAddress, &body).expect("parses");
        assert_eq!(
            remote.object.natural_key,
            [("address".to_string(), "10.42.0.5".to_string())].into()
        );
        assert_eq!(
            remote
                .object
                .content
                .get("assigned_to_interface")
                .map(String::as_str),
            Some("vm-01/backend")
        );

        // Unassigned → "(unset)".
        let unassigned = json!({ "id": 22, "address": "10.42.0.6/32", "assigned_object": null });
        let remote = parse_remote(NetBoxKind::IpAddress, &unassigned).expect("parses");
        assert_eq!(
            remote
                .object
                .content
                .get("assigned_to_interface")
                .map(String::as_str),
            Some("(unset)")
        );
    }

    #[test]
    fn missing_required_fields_fail_closed() {
        // Device without a name.
        let body = json!({ "id": 7, "status": { "value": "active" } });
        assert!(matches!(
            parse_remote(NetBoxKind::Device, &body),
            Err(ClientError::MalformedResponse { .. })
        ));
        // Device without status.value.
        let body = json!({ "id": 7, "name": "n" });
        assert!(matches!(
            parse_remote(NetBoxKind::Device, &body),
            Err(ClientError::MalformedResponse { .. })
        ));
        // Row without id.
        let body = json!({ "name": "n" });
        assert!(matches!(
            parse_remote(NetBoxKind::Device, &body),
            Err(ClientError::MalformedResponse { .. })
        ));
        // Interface without a parent VM.
        let body = json!({ "id": 11, "name": "backend" });
        assert!(matches!(
            parse_remote(NetBoxKind::Interface, &body),
            Err(ClientError::MalformedResponse { .. })
        ));
    }

    #[test]
    fn non_scalar_custom_fields_fail_closed() {
        let body = json!({
            "id": 7,
            "name": "n",
            "status": { "value": "active" },
            "custom_fields": { "outside": ["a", "b"] }
        });
        assert!(matches!(
            parse_remote(NetBoxKind::Device, &body),
            Err(ClientError::MalformedResponse { .. })
        ));
    }

    #[test]
    fn list_page_requires_results() {
        let ok = json!({ "count": 1, "next": null, "results": [{ "id": 1 }] });
        assert!(parse_list_page(&ok).is_ok());
        let missing = json!({ "count": 0, "next": null });
        assert!(matches!(
            parse_list_page(&missing),
            Err(ClientError::MalformedResponse { .. })
        ));
    }
}
