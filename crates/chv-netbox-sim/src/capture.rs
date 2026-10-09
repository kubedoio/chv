//! Live-capture support for the real-NetBox qualification lane
//! (ADR-024 decision 4, issue kubedoio/chv#586, PR 5).
//!
//! Two consumers share this module so the qualification lane stays
//! coherent:
//!
//! - the composed suite's real backend arm
//!   (`chv-controlplane-service`'s `netbox_projection_sim_tests`,
//!   driven by `scripts/netbox-qualify.sh`), and
//! - the fixture recorder (`tests/record_fixtures.rs`, the
//!   `--record` drift-tripwire refresh).
//!
//! [`LiveNetBox`] speaks plain HTTP(S) to a real NetBox REST API with
//! the six endpoint families [`SimKind`] defines — listing with
//! pagination, creating, deleting — plus a generic request escape
//! hatch for the recorder's site/custom-field provisioning. Error
//! messages never include the token.
//!
//! [`normalize_object`] reduces a live API row to the simulator's
//! dump/read form. That field set is the **drift-sensitive core** of
//! the qualification lane: running the same scenarios against the
//! simulator and against a real NetBox only proves the simulator
//! models reality if both backends' state dumps are comparable, and
//! this function is what makes them comparable. Divergences that
//! survive it (a renamed field, a different nested-relation shape, a
//! choice `value` NetBox no longer emits) are exactly what the
//! qualification run is meant to catch.

use serde_json::{json, Map, Value};
use thiserror::Error;

use crate::kind::SimKind;

/// Env var carrying the qualification instance's base URL.
pub const QUALIFICATION_URL_ENV: &str = "NETBOX_QUALIFICATION_URL";
/// Env var carrying the qualification instance's API token.
pub const QUALIFICATION_TOKEN_ENV: &str = "NETBOX_QUALIFICATION_TOKEN";

/// Page size for list requests: comfortably above the canonical
/// one-object-per-family content while staying under NetBox's default
/// `MAX_PAGE_SIZE` (1000), so pagination behavior is still exercised.
const LIST_LIMIT: usize = 200;

/// Deletion order for [`LiveNetBox::delete_all`]: children first. A
/// VLAN referenced by a prefix cannot be deleted (NetBox protects the
/// reference), and deleting a VM cascades its interfaces, so the
/// leaf-to-root sweep resolves every dependency; a second sweep
/// catches anything a race re-created.
const DELETE_ORDER: [SimKind; 6] = [
    SimKind::IpAddress,
    SimKind::Interface,
    SimKind::Prefix,
    SimKind::Vlan,
    SimKind::VirtualMachine,
    SimKind::Device,
];

/// A live-capture request failure. The `Display` form carries the
/// method, URL, status, and response body — never the token.
#[derive(Debug, Error)]
pub enum CaptureError {
    /// The instance answered with an unexpected HTTP status.
    #[error("{method} {url} -> {status}: {body}")]
    Status {
        method: String,
        url: String,
        status: u16,
        body: String,
    },
    /// The request could not be sent or the response not read.
    #[error("{method} {url} failed: {message}")]
    Transport {
        method: String,
        url: String,
        message: String,
    },
    /// The response could not be interpreted.
    #[error("{method} {url} returned an unusable response: {message}")]
    Invalid {
        method: String,
        url: String,
        message: String,
    },
}

/// A client for a live NetBox REST API over the six endpoint families
/// the adapter client uses. Constructed with the same
/// `NETBOX_QUALIFICATION_URL`/`NETBOX_QUALIFICATION_TOKEN` pair the
/// qualification lane provides.
pub struct LiveNetBox {
    http: reqwest::Client,
    base: String,
    token: String,
}

impl LiveNetBox {
    /// A client for `base_url` (trailing slash tolerated) presenting
    /// `token` as `Authorization: Token …` on every request.
    pub fn new(base_url: &str, token: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            base: base_url.trim_end_matches('/').to_string(),
            token: token.to_string(),
        }
    }

    /// The base URL every request is anchored at.
    pub fn base_url(&self) -> &str {
        &self.base
    }

    /// The API token this client presents.
    pub fn token(&self) -> &str {
        &self.token
    }

    /// An authenticated request against `target` — an API path
    /// relative to the base URL, or an absolute URL (a pagination
    /// `next` link). Returns the status and the parsed body (`null`
    /// for empty bodies, e.g. `204` deletes).
    pub async fn request(
        &self,
        method: reqwest::Method,
        target: &str,
        body: Option<&Value>,
    ) -> Result<(u16, Value), CaptureError> {
        let url = if target.starts_with("http://") || target.starts_with("https://") {
            target.to_string()
        } else {
            format!("{}{}", self.base, target)
        };
        let mut request = self
            .http
            .request(method.clone(), &url)
            .header("Authorization", format!("Token {}", self.token));
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request
            .send()
            .await
            .map_err(|error| CaptureError::Transport {
                method: method.to_string(),
                url: url.clone(),
                message: error.to_string(),
            })?;
        let status = response.status().as_u16();
        let text = response
            .text()
            .await
            .map_err(|error| CaptureError::Transport {
                method: method.to_string(),
                url: url.clone(),
                message: error.to_string(),
            })?;
        let value = if text.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).map_err(|error| CaptureError::Invalid {
                method: method.to_string(),
                url: url.clone(),
                message: format!("body is not JSON: {error}"),
            })?
        };
        Ok((status, value))
    }

    /// Every row of `kind`, following `next` links across pages (the
    /// links are re-anchored onto this client's base URL so a host
    /// the instance misreports cannot derail the walk).
    pub async fn list(&self, kind: SimKind) -> Result<Vec<Value>, CaptureError> {
        let mut target = format!("{}?limit={}", kind.api_path(), LIST_LIMIT);
        let mut rows = Vec::new();
        loop {
            let (status, page) = self.request(reqwest::Method::GET, &target, None).await?;
            if status != 200 {
                return Err(CaptureError::Status {
                    method: "GET".into(),
                    url: target,
                    status,
                    body: page.to_string(),
                });
            }
            if let Some(results) = page.get("results").and_then(Value::as_array) {
                rows.extend(results.iter().cloned());
            }
            target = match page.get("next").and_then(Value::as_str) {
                Some(next) => self.rebase(next)?,
                None => return Ok(rows),
            };
        }
    }

    /// `GET /api/status/` — a real NetBox answers with its version; a
    /// simulator 404s everything outside the six families, so an
    /// error here identifies the simulator (the fixture recorder uses
    /// that to pick its write dialect).
    pub async fn api_status(&self) -> Result<Value, CaptureError> {
        let (status, value) = self
            .request(reqwest::Method::GET, "/api/status/", None)
            .await?;
        if status == 200 {
            Ok(value)
        } else {
            Err(CaptureError::Status {
                method: "GET".into(),
                url: format!("{}/api/status/", self.base),
                status,
                body: value.to_string(),
            })
        }
    }

    /// Create one object of `kind`; returns the created row (with the
    /// instance-assigned id).
    pub async fn create(&self, kind: SimKind, body: &Value) -> Result<Value, CaptureError> {
        let (status, created) = self
            .request(reqwest::Method::POST, kind.api_path(), Some(body))
            .await?;
        if status == 201 {
            Ok(created)
        } else {
            Err(CaptureError::Status {
                method: "POST".into(),
                url: format!("{}{}", self.base, kind.api_path()),
                status,
                body: created.to_string(),
            })
        }
    }

    /// Delete `(kind, id)`; `404` counts as success (idempotent).
    pub async fn delete(&self, kind: SimKind, id: i64) -> Result<(), CaptureError> {
        let url = format!("{}{id}/", kind.api_path());
        let (status, body) = self.request(reqwest::Method::DELETE, &url, None).await?;
        if status == 204 || status == 404 {
            Ok(())
        } else {
            Err(CaptureError::Status {
                method: "DELETE".into(),
                url: format!("{}{}", self.base, url),
                status,
                body: body.to_string(),
            })
        }
    }

    /// Delete every object in the six families (children first, two
    /// sweeps), leaving the instance empty for the next scenario.
    /// Returns the number of objects deleted.
    pub async fn delete_all(&self) -> Result<usize, CaptureError> {
        let mut deleted = 0usize;
        for _sweep in 0..2 {
            for kind in DELETE_ORDER {
                for row in self.list(kind).await? {
                    if let Some(id) = row.get("id").and_then(Value::as_i64) {
                        self.delete(kind, id).await?;
                        deleted += 1;
                    }
                }
            }
            if self.count_all().await? == 0 {
                return Ok(deleted);
            }
        }
        let mut per_kind = Vec::new();
        for kind in DELETE_ORDER {
            per_kind.push(format!("{}: {}", kind, self.list(kind).await?.len()));
        }
        Err(CaptureError::Invalid {
            method: "DELETE".into(),
            url: self.base.clone(),
            message: format!(
                "objects remain after two deletion sweeps ({})",
                per_kind.join(", ")
            ),
        })
    }

    async fn count_all(&self) -> Result<usize, CaptureError> {
        let mut total = 0;
        for kind in SimKind::ALL {
            total += self.list(kind).await?.len();
        }
        Ok(total)
    }

    /// Re-anchor a pagination link (which carries the host the
    /// instance believes it has) onto this client's base URL.
    fn rebase(&self, next: &str) -> Result<String, CaptureError> {
        let parsed = reqwest::Url::parse(next).map_err(|error| CaptureError::Invalid {
            method: "GET".into(),
            url: next.to_string(),
            message: format!("invalid pagination link: {error}"),
        })?;
        let mut url = format!("{}{}", self.base, parsed.path());
        if let Some(query) = parsed.query() {
            url.push('?');
            url.push_str(query);
        }
        Ok(url)
    }
}

// ---------------------------------------------------------------------------
// Normalization
// ---------------------------------------------------------------------------

/// Reduce one live API row of `kind` to the simulator's dump/read
/// form — the exact field set the simulator's `read_form` emits, so a
/// real instance's state and the simulator's `/__state` dump compare
/// structurally.
///
/// Kept per kind (everything else the live serializer sends is
/// dropped — a real NetBox row carries dozens of fields the six-family
/// contract never reads):
///
/// - every kind: `id`, `url` (rebuilt from `base`), `display`,
///   `created`/`last_updated` (when present), `tags` as
///   `[{id, name, slug}]`, `custom_fields` with **null values
///   dropped** (a real NetBox materializes every field assigned to
///   the content type with `null`; the simulator stores only written
///   keys — dropping nulls is what makes the two comparable);
/// - devices: `name`, `status` (`{value, label}`), `site`
///   (`{id, name}`);
/// - virtual machines: `name`, `status`, `cluster`/`device`
///   (`{id, name}`), `vcpus`, `memory`;
/// - interfaces: `name`, `virtual_machine` (`{id, name}`),
///   `description`, `type` (`{value, label}`);
/// - prefixes: `prefix`, `vlan` (`{id, vid, name}`), `description`;
/// - vlans: `vid`, `name`;
/// - IP addresses: `address`, `assigned_object_type`,
///   `assigned_object_id`, and `assigned_object` reduced to
///   `{id, name, virtual_machine: {id, name}}` (the simulator derives
///   the same shape from its interface table).
pub fn normalize_object(kind: SimKind, row: &Value, base: &str) -> Value {
    let id = row.get("id").and_then(Value::as_i64).unwrap_or_default();
    let mut out = Map::new();
    out.insert("id".to_string(), json!(id));
    out.insert(
        "url".to_string(),
        json!(format!("{base}{}{id}/", kind.api_path())),
    );
    out.insert(
        "display".to_string(),
        row.get("display").cloned().unwrap_or(Value::Null),
    );
    match kind {
        SimKind::Device => {
            out.insert("name".to_string(), scalar(row, "name"));
            out.insert("status".to_string(), choice(row, "status"));
            out.insert("site".to_string(), nested_name(row, "site"));
            out.insert("tags".to_string(), tags(row));
            out.insert("custom_fields".to_string(), custom_fields(row));
        }
        SimKind::VirtualMachine => {
            out.insert("name".to_string(), scalar(row, "name"));
            out.insert("status".to_string(), choice(row, "status"));
            out.insert("cluster".to_string(), nested_name(row, "cluster"));
            out.insert("device".to_string(), nested_name(row, "device"));
            out.insert("vcpus".to_string(), scalar(row, "vcpus"));
            out.insert("memory".to_string(), scalar(row, "memory"));
            out.insert("tags".to_string(), tags(row));
            out.insert("custom_fields".to_string(), custom_fields(row));
        }
        SimKind::Interface => {
            out.insert("name".to_string(), scalar(row, "name"));
            out.insert(
                "virtual_machine".to_string(),
                nested_name(row, "virtual_machine"),
            );
            out.insert("description".to_string(), scalar(row, "description"));
            out.insert("type".to_string(), choice(row, "type"));
            out.insert("tags".to_string(), tags(row));
            out.insert("custom_fields".to_string(), custom_fields(row));
        }
        SimKind::Prefix => {
            out.insert("prefix".to_string(), scalar(row, "prefix"));
            out.insert("vlan".to_string(), vlan_ref(row));
            out.insert("description".to_string(), scalar(row, "description"));
            out.insert("tags".to_string(), tags(row));
            out.insert("custom_fields".to_string(), custom_fields(row));
        }
        SimKind::Vlan => {
            out.insert("vid".to_string(), scalar(row, "vid"));
            out.insert("name".to_string(), scalar(row, "name"));
            out.insert("tags".to_string(), tags(row));
            out.insert("custom_fields".to_string(), custom_fields(row));
        }
        SimKind::IpAddress => {
            out.insert("address".to_string(), scalar(row, "address"));
            out.insert(
                "assigned_object_type".to_string(),
                scalar(row, "assigned_object_type"),
            );
            out.insert(
                "assigned_object_id".to_string(),
                scalar(row, "assigned_object_id"),
            );
            out.insert("assigned_object".to_string(), assigned_object(row));
            out.insert("tags".to_string(), tags(row));
            out.insert("custom_fields".to_string(), custom_fields(row));
        }
    }
    for field in ["created", "last_updated"] {
        if let Some(value) = row.get(field) {
            out.insert(field.to_string(), value.clone());
        }
    }
    Value::Object(out)
}

/// A field copied verbatim (`null` when absent), except that integral
/// float numbers are canonicalized to integers: NetBox 4.7 types
/// `VirtualMachine.vcpus` as a `DecimalField`
/// (netbox/virtualization/models/virtualmachines.py), so a live row
/// answers `2.0` where the simulator's shape — and every scenario
/// assertion — carries `2`, and `serde_json` treats the two as
/// unequal numbers. Fractional values pass through untouched (a real
/// deployment may carry fractional vCPUs).
fn scalar(row: &Value, field: &str) -> Value {
    match row.get(field) {
        Some(Value::Number(number)) => canonical_number(number),
        other => other.cloned().unwrap_or(Value::Null),
    }
}

/// `2.0` → `2`; any non-integral number is returned unchanged.
fn canonical_number(number: &serde_json::Number) -> Value {
    if let Some(float) = number.as_f64() {
        if float.is_finite() && float.fract() == 0.0 && float.abs() < 9.0e18 {
            return Value::from(float as i64);
        }
    }
    Value::Number(number.clone())
}

/// A choice field in NetBox's `{value, label}` read form.
fn choice(row: &Value, field: &str) -> Value {
    match row.get(field) {
        Some(Value::Object(choice)) => json!({
            "value": choice.get("value").cloned().unwrap_or(Value::Null),
            "label": choice.get("label").cloned().unwrap_or(Value::Null),
        }),
        // The simulator's write form (a bare value) is wrapped with
        // the value as its own label; live rows always arrive as
        // objects, so this only fires on simulator-shaped input.
        Some(Value::String(value)) => json!({ "value": value, "label": value }),
        _ => Value::Null,
    }
}

/// A named relation in the simulator's minimal `{id, name}` form.
fn nested_name(row: &Value, field: &str) -> Value {
    match row.get(field) {
        Some(Value::Object(relation)) => json!({
            "id": relation.get("id").cloned().unwrap_or(Value::Null),
            "name": relation.get("name").cloned().unwrap_or(Value::Null),
        }),
        _ => Value::Null,
    }
}

/// A prefix's VLAN reference in the `{id, vid, name}` form.
fn vlan_ref(row: &Value) -> Value {
    match row.get("vlan") {
        Some(Value::Object(vlan)) => json!({
            "id": vlan.get("id").cloned().unwrap_or(Value::Null),
            "vid": vlan.get("vid").cloned().unwrap_or(Value::Null),
            "name": vlan.get("name").cloned().unwrap_or(Value::Null),
        }),
        _ => Value::Null,
    }
}

/// Tags in NetBox's nested `[{id, name, slug}]` list form.
fn tags(row: &Value) -> Value {
    match row.get("tags") {
        Some(Value::Array(items)) => Value::Array(
            items
                .iter()
                .map(|tag| match tag {
                    Value::Object(tag) => json!({
                        "id": tag.get("id").cloned().unwrap_or(Value::Null),
                        "name": tag.get("name").cloned().unwrap_or(Value::Null),
                        "slug": tag.get("slug").cloned().unwrap_or(Value::Null),
                    }),
                    _ => Value::Null,
                })
                .collect(),
        ),
        _ => json!([]),
    }
}

/// Custom fields with null values dropped (see [`normalize_object`]).
fn custom_fields(row: &Value) -> Value {
    let mut fields = Map::new();
    if let Some(Value::Object(object)) = row.get("custom_fields") {
        for (key, value) in object {
            if !value.is_null() {
                fields.insert(key.clone(), value.clone());
            }
        }
    }
    Value::Object(fields)
}

/// An IP address's `assigned_object` reduced to the shape the
/// simulator derives from its interface table.
fn assigned_object(row: &Value) -> Value {
    match row.get("assigned_object") {
        Some(assignment @ Value::Object(_)) => json!({
            "id": assignment.get("id").cloned().unwrap_or(Value::Null),
            "name": assignment.get("name").cloned().unwrap_or(Value::Null),
            "virtual_machine": nested_name(assignment, "virtual_machine"),
        }),
        _ => Value::Null,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::SimState;
    use serde_json::json;

    /// The normalization must be the identity on rows the simulator
    /// itself produced: the qualification suite compares real dumps
    /// byte-for-byte between phases, and a sim-bin qualification run
    /// (the local stand-in for a real instance) only passes if this
    /// holds — any difference is a normalization bug.
    #[test]
    fn normalization_is_the_identity_on_simulator_rows() {
        let mut state = SimState::new();
        state
            .create(
                SimKind::Vlan,
                &json!({ "vid": 42, "name": "backend", "tags": ["chv-team"],
                         "custom_fields": { "chv_managed_by": "chv" } }),
            )
            .expect("vlan");
        state
            .create(
                SimKind::Prefix,
                &json!({ "prefix": "10.42.0.0/24", "vlan": { "vid": 42 },
                         "description": "backend" }),
            )
            .expect("prefix");
        let device = state
            .create(
                SimKind::Device,
                &json!({ "name": "chv-node-01", "status": "active",
                         "site": { "name": "dc1" }, "tags": ["chv-team"],
                         "device_type": { "slug": chv_netbox_adapter::CHV_NETBOX_DEVICE_TYPE },
                         "role": { "slug": chv_netbox_adapter::CHV_NETBOX_DEVICE_ROLE } }),
            )
            .expect("device");
        let vm = state
            .create(
                SimKind::VirtualMachine,
                &json!({ "name": "vm-01", "status": "active",
                         "device": { "name": "chv-node-01" },
                         "vcpus": 2, "memory": 2048 }),
            )
            .expect("vm");
        let interface = state
            .create(
                SimKind::Interface,
                &json!({ "name": "backend", "virtual_machine": { "name": "vm-01" },
                         "description": "backend", "type": "virtual" }),
            )
            .expect("interface");
        state
            .create(
                SimKind::IpAddress,
                &json!({ "address": "10.42.0.5/24",
                         "assigned_object_type": "virtualization.vminterface",
                         "assigned_object_id": interface }),
            )
            .expect("assigned ip");
        // Bare rows: no device on the VM, an unassigned address.
        let bare_vm = state
            .create(SimKind::VirtualMachine, &json!({ "name": "vm-bare" }))
            .expect("bare vm");
        state
            .create(SimKind::IpAddress, &json!({ "address": "10.42.0.6/32" }))
            .expect("unassigned ip");

        let base = "http://127.0.0.1:1";
        let cases = [
            (SimKind::Vlan, vec![1]),
            (SimKind::Prefix, vec![2]),
            (SimKind::Device, vec![device]),
            (SimKind::VirtualMachine, vec![vm, bare_vm]),
            (SimKind::Interface, vec![interface]),
            (SimKind::IpAddress, vec![6, 8]),
        ];
        for (kind, ids) in cases {
            for id in ids {
                let row = state.row(kind, id).expect("stored row");
                let read = state.read_form(kind, row, base);
                assert_eq!(
                    normalize_object(kind, &read, base),
                    read,
                    "{kind} {id}: normalization must not alter a simulator row"
                );
            }
        }
    }

    /// A real-shaped device row — extra serializer fields, nested
    /// relations with `url`/`display`/`slug`, unset custom fields —
    /// reduces to exactly the sim-comparable field set.
    #[test]
    fn real_shaped_rows_reduce_to_the_sim_comparable_field_set() {
        let row = json!({
            "id": 7,
            "url": "http://live.example.com/api/dcim/devices/7/",
            "display": "chv-node-01",
            "name": "chv-node-01",
            "status": { "value": "active", "label": "Active" },
            "site": {
                "id": 4, "url": "http://live.example.com/api/dcim/sites/4/",
                "display": "dc1", "name": "dc1", "slug": "dc1"
            },
            "tags": [{
                "id": 1, "url": "http://live.example.com/api/extras/tags/1/",
                "display": "chv-team", "name": "chv-team", "slug": "chv-team",
                "color": "9e9e9e"
            }],
            "custom_fields": {
                "chv_external_id": "arch:x", "chv_managed_state": null
            },
            "created": "2026-10-09T12:00:00.000000Z",
            "last_updated": "2026-10-09T12:00:00.000000Z",
            "serial": "", "face": null, "parent": null,
            "device_type": { "id": 1, "display": "chv-host", "name": "chv-host" }
        });
        assert_eq!(
            normalize_object(SimKind::Device, &row, "http://netbox.example.com"),
            json!({
                "id": 7,
                "url": "http://netbox.example.com/api/dcim/devices/7/",
                "display": "chv-node-01",
                "name": "chv-node-01",
                "status": { "value": "active", "label": "Active" },
                "site": { "id": 4, "name": "dc1" },
                "tags": [{ "id": 1, "name": "chv-team", "slug": "chv-team" }],
                "custom_fields": { "chv_external_id": "arch:x" },
                "created": "2026-10-09T12:00:00.000000Z",
                "last_updated": "2026-10-09T12:00:00.000000Z"
            })
        );
    }

    /// A real-shaped VM row: NetBox 4.7 types `vcpus` as a
    /// `DecimalField` (netbox/virtualization/models/virtualmachines.py),
    /// so live rows answer `2.0` where the simulator's shape carries
    /// `2` — integral floats canonicalize to integers, fractional
    /// vCPUs pass through untouched.
    #[test]
    fn real_shaped_vm_decimal_scalars_canonicalize_to_integers() {
        let integral = json!({
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
        let normalized = normalize_object(
            SimKind::VirtualMachine,
            &integral,
            "http://netbox.example.com",
        );
        assert_eq!(normalized["vcpus"], json!(2), "DecimalField 2.0 -> 2");
        assert_eq!(normalized["memory"], json!(2048));

        let fractional = json!({
            "id": 10,
            "name": "vm-02",
            "status": { "value": "active", "label": "Active" },
            "cluster": null,
            "device": null,
            "vcpus": 0.5,
            "memory": null,
            "tags": [],
            "custom_fields": {}
        });
        let normalized = normalize_object(
            SimKind::VirtualMachine,
            &fractional,
            "http://netbox.example.com",
        );
        assert_eq!(normalized["vcpus"], json!(0.5), "fractional vCPUs survive");
    }

    /// A real-shaped IP address row: the `assigned_object` serializer
    /// is reduced to the simulator's derived `{id, name,
    /// virtual_machine}` form.
    #[test]
    fn real_shaped_ip_assignments_reduce_to_the_derived_form() {
        let row = json!({
            "id": 21,
            "url": "http://live.example.com/api/ipam/ip-addresses/21/",
            "display": "10.42.0.5/24",
            "address": "10.42.0.5/24",
            "assigned_object_type": "virtualization.vminterface",
            "assigned_object_id": 11,
            "assigned_object": {
                "id": 11,
                "url": "http://live.example.com/api/virtualization/interfaces/11/",
                "display": "backend",
                "name": "backend",
                "virtual_machine": {
                    "id": 9, "url": "http://live.example.com/api/virtualization/virtual-machines/9/",
                    "display": "vm-01", "name": "vm-01"
                }
            },
            "tags": [],
            "custom_fields": {},
            "created": "2026-10-09T12:00:00.000000Z",
            "last_updated": "2026-10-09T12:00:00.000000Z"
        });
        let normalized = normalize_object(SimKind::IpAddress, &row, "http://netbox.example.com");
        assert_eq!(
            normalized["assigned_object"],
            json!({
                "id": 11, "name": "backend",
                "virtual_machine": { "id": 9, "name": "vm-01" }
            })
        );
        assert_eq!(normalized["tags"], json!([]));
    }
}
