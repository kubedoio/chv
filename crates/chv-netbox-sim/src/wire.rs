//! Wire-shape helpers: NetBox 4.x write normalization, natural keys,
//! list filtering, and `limit`/`offset` pagination math.
//!
//! Every rule here traces to the mapping contract's "NetBox REST
//! surface used (v1)" section or to the adapter client's fail-closed
//! parser (`chv-netbox-adapter/src/client.rs`) — the simulator must
//! satisfy both. The golden fixtures under `tests/fixtures/netbox4/`
//! pin the exact response shapes.

use serde_json::{json, Map, Value};
use thiserror::Error;

use crate::kind::SimKind;

/// NetBox's universal 404 detail string.
pub const NOT_FOUND_DETAIL: &str = "Not found.";

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// A request the simulator refuses, in NetBox's error-body shapes.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum WireError {
    /// Unknown object id (or path) — `404 {"detail": "Not found."}`.
    #[error("not found")]
    NotFound,
    /// Field-scoped validation failure — NetBox's
    /// `{"<field>": ["<message>"]}` shape (duplicate natural keys,
    /// missing/invalid fields).
    #[error("{field}: {message}")]
    Field {
        field: &'static str,
        message: &'static str,
    },
    /// Non-field failure — `{"detail": "<message>"}`.
    #[error("{message}")]
    Detail { message: &'static str },
    /// A delete refused because other objects still reference this
    /// one through a PROTECT foreign key (a VLAN referenced by a
    /// prefix). Answered as `409 {"detail": ...}`; real NetBox
    /// surfaces Django's `ProtectedError` through the REST API as
    /// an HTTP 500 — a documented deviation (see the crate docs'
    /// fidelity notes).
    #[error("protected delete: {message}")]
    Protected { message: String },
}

impl WireError {
    /// HTTP status for the error.
    pub fn status(&self) -> u16 {
        match self {
            WireError::NotFound => 404,
            WireError::Protected { .. } => 409,
            WireError::Field { .. } | WireError::Detail { .. } => 400,
        }
    }

    /// NetBox-shaped JSON body for the error.
    pub fn body(&self) -> Value {
        match self {
            WireError::NotFound => json!({ "detail": NOT_FOUND_DETAIL }),
            WireError::Field { field, message } => {
                let mut body = Map::new();
                body.insert((*field).to_string(), json!([message]));
                Value::Object(body)
            }
            WireError::Detail { message } => json!({ "detail": message }),
            WireError::Protected { message } => json!({ "detail": message }),
        }
    }
}

/// NetBox's duplicate-natural-key error for a kind.
pub fn unique_violation(kind: SimKind) -> WireError {
    WireError::Field {
        field: kind.unique_error_field(),
        message: "This field must be unique.",
    }
}

// ---------------------------------------------------------------------------
// Field extraction (fail-closed on the write surface)
// ---------------------------------------------------------------------------

fn field_str<'a>(object: &'a Map<String, Value>, field: &'static str) -> Option<&'a str> {
    object.get(field).and_then(Value::as_str)
}

pub(crate) fn req_string(
    object: &Map<String, Value>,
    field: &'static str,
) -> Result<String, WireError> {
    field_str(object, field)
        .map(str::to_string)
        .ok_or(WireError::Field {
            field,
            message: "This field is required.",
        })
}

pub(crate) fn req_i64(object: &Map<String, Value>, field: &'static str) -> Result<i64, WireError> {
    object
        .get(field)
        .and_then(Value::as_i64)
        .ok_or(WireError::Field {
            field,
            message: "A valid integer is required.",
        })
}

/// A nullable optional string field: absent/`null` → `Value::Null`.
pub(crate) fn opt_nullable_string(
    object: &Map<String, Value>,
    field: &'static str,
) -> Result<Value, WireError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(Value::Null),
        Some(Value::String(s)) => Ok(json!(s)),
        Some(_) => Err(WireError::Field {
            field,
            message: "A valid string is required.",
        }),
    }
}

/// A nullable optional integer field.
pub(crate) fn opt_nullable_i64(
    object: &Map<String, Value>,
    field: &'static str,
) -> Result<Value, WireError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(Value::Null),
        Some(Value::Number(_)) => Ok(object.get(field).expect("checked above").clone()),
        Some(_) => Err(WireError::Field {
            field,
            message: "A valid integer is required.",
        }),
    }
}

/// A nullable optional numeric field (VM `vcpus`/`memory`).
pub(crate) fn opt_nullable_number(
    object: &Map<String, Value>,
    field: &'static str,
) -> Result<Value, WireError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(Value::Null),
        Some(Value::Number(_)) => Ok(object.get(field).expect("checked above").clone()),
        Some(_) => Err(WireError::Field {
            field,
            message: "A valid number is required.",
        }),
    }
}

/// `custom_fields` must be an object of scalars (the mapping
/// contract's custom-field surface); anything else fails closed.
pub(crate) fn normalize_custom_fields(
    object: &Map<String, Value>,
) -> Result<Map<String, Value>, WireError> {
    match object.get("custom_fields") {
        None | Some(Value::Null) => Ok(Map::new()),
        Some(Value::Object(fields)) => {
            for value in fields.values() {
                if !matches!(
                    value,
                    Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null
                ) {
                    return Err(WireError::Field {
                        field: "custom_fields",
                        message: "Custom field values must be scalars.",
                    });
                }
            }
            Ok(fields.clone())
        }
        Some(_) => Err(WireError::Field {
            field: "custom_fields",
            message: "A valid object is required.",
        }),
    }
}

// ---------------------------------------------------------------------------
// Choice fields (status, interface type)
// ---------------------------------------------------------------------------

/// The display label for a choice value. Only the values the adapter
/// client writes have curated labels; anything else echoes the value
/// (real NetBox derives the label from its configured choices).
fn choice_label(value: &str) -> String {
    let label = match value {
        "active" => "Active",
        "offline" => "Offline",
        "staged" => "Staged",
        "planned" => "Planned",
        "decommissioning" => "Decommissioning",
        "virtual" => "Virtual",
        other => other,
    };
    label.to_string()
}

/// Normalize a choice field to NetBox's read form
/// `{"value": ..., "label": ...}`. Accepts the write form (a plain
/// string) or an already-normalized object (PATCH merge input).
pub(crate) fn normalize_choice(
    object: &Map<String, Value>,
    field: &'static str,
    default: &str,
) -> Result<Value, WireError> {
    let value = match object.get(field) {
        None => default.to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Object(choice)) => choice
            .get("value")
            .and_then(Value::as_str)
            .ok_or(WireError::Field {
                field,
                message: "A valid choice value is required.",
            })?
            .to_string(),
        Some(_) => {
            return Err(WireError::Field {
                field,
                message: "A valid choice value is required.",
            })
        }
    };
    Ok(json!({ "value": value, "label": choice_label(&value) }))
}

// ---------------------------------------------------------------------------
// Slugs and address masks
// ---------------------------------------------------------------------------

/// A minimal NetBox-style slug: lowercase, non-alphanumerics to `-`.
pub(crate) fn slugify(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());
    let mut dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            dash = false;
        } else if !dash && !slug.is_empty() {
            slug.push('-');
            dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}

/// NetBox stores IP addresses with a mask; a maskless address is
/// normalized to `/32` (v4) or `/128` (v6).
pub(crate) fn with_mask(address: &str) -> String {
    if address.contains('/') {
        address.to_string()
    } else if address.contains(':') {
        format!("{address}/128")
    } else {
        format!("{address}/32")
    }
}

/// The address without its mask suffix (the client's natural key for
/// IP addresses is mask-independent).
pub(crate) fn strip_mask(address: &str) -> &str {
    match address.split_once('/') {
        Some((ip, _)) => ip,
        None => address,
    }
}

// ---------------------------------------------------------------------------
// Natural keys and list filtering
// ---------------------------------------------------------------------------

/// The natural key of a stored row, as `(field, value)` pairs —
/// the basis of the create/patch uniqueness check, matching the
/// keys the adapter client filters on. One deliberate exception:
/// an IP address's key is the full with-mask address, because
/// NetBox's unique constraint includes the mask (`10.42.0.5/24`
/// and `10.42.0.5/32` are distinct rows). List *filtering* stays
/// mask-independent (see [`row_matches`]).
pub(crate) fn natural_key(kind: SimKind, row: &Value) -> Vec<(String, String)> {
    let name = || {
        row.get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    match kind {
        SimKind::Device | SimKind::VirtualMachine => vec![("name".to_string(), name())],
        SimKind::Interface => {
            let vm = row
                .get("virtual_machine")
                .and_then(|vm| vm.get("name"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            vec![
                ("name".to_string(), name()),
                ("virtual_machine".to_string(), vm),
            ]
        }
        SimKind::Prefix => vec![(
            "prefix".to_string(),
            row.get("prefix")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        )],
        SimKind::Vlan => vec![(
            "vid".to_string(),
            row.get("vid")
                .and_then(Value::as_i64)
                .unwrap_or_default()
                .to_string(),
        )],
        SimKind::IpAddress => vec![(
            "address".to_string(),
            // The full with-mask address: NetBox's unique
            // constraint includes the mask (stored rows always
            // carry one — see `with_mask`).
            row.get("address")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        )],
    }
}

/// Render a scalar custom-field value the way the adapter client's
/// parser stringifies it, so `cf_<field>` filters compare equal.
fn scalar_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

/// Whether a stored row matches the request's filter parameters.
///
/// Recognized parameters are exactly the ones the adapter client
/// sends: the kind's natural-key filters plus `cf_<field>` custom-field
/// filters. `limit`/`offset` are pagination, and unknown parameters
/// are ignored (DRF behavior for non-filter keys).
pub(crate) fn row_matches(
    kind: SimKind,
    row: &Value,
    params: &[(String, String)],
) -> Result<bool, WireError> {
    for (key, value) in params {
        if key == "limit" || key == "offset" {
            continue;
        }
        if let Some(field) = key.strip_prefix("cf_") {
            let stored = row
                .get("custom_fields")
                .and_then(|fields| fields.get(field))
                .map(scalar_to_string);
            if stored.as_deref() != Some(value.as_str()) {
                return Ok(false);
            }
            continue;
        }
        let matched = if key == "name"
            && matches!(
                kind,
                SimKind::Device | SimKind::VirtualMachine | SimKind::Interface
            ) {
            row.get("name").and_then(Value::as_str) == Some(value.as_str())
        } else if key == "virtual_machine" && kind == SimKind::Interface {
            row.get("virtual_machine")
                .and_then(|vm| vm.get("name"))
                .and_then(Value::as_str)
                == Some(value.as_str())
        } else if key == "prefix" && kind == SimKind::Prefix {
            row.get("prefix").and_then(Value::as_str) == Some(value.as_str())
        } else if key == "vid" && kind == SimKind::Vlan {
            let vid: i64 = value.parse().map_err(|_| WireError::Field {
                field: "vid",
                message: "A valid integer is required.",
            })?;
            row.get("vid").and_then(Value::as_i64) == Some(vid)
        } else if key == "address" && kind == SimKind::IpAddress {
            let stored = row
                .get("address")
                .and_then(Value::as_str)
                .map(strip_mask)
                .unwrap_or_default();
            stored == strip_mask(value)
        } else {
            true
        };
        if !matched {
            return Ok(false);
        }
    }
    Ok(true)
}

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------

/// The effective page size for a request: no `limit` → the default
/// page size; `limit=0` → the server max page (NetBox's
/// `MAX_PAGE_SIZE` behavior); otherwise the requested size clamped
/// to the max.
pub(crate) fn effective_limit(requested: Option<usize>, default: usize, max: usize) -> usize {
    match requested {
        None => default,
        Some(0) => max,
        Some(n) => n.min(max),
    }
}

/// Percent-encode a query component (unreserved characters only).
fn urlencode(input: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(input.len());
    for byte in input.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*byte as char);
            }
            _ => {
                out.push('%');
                out.push(HEX[(byte >> 4) as usize] as char);
                out.push(HEX[(byte & 0x0f) as usize] as char);
            }
        }
    }
    out
}

fn link(
    base: &str,
    path: &str,
    filters: &[(String, String)],
    limit: usize,
    offset: Option<usize>,
) -> String {
    let mut pairs: Vec<String> = filters
        .iter()
        .map(|(key, value)| format!("{}={}", urlencode(key), urlencode(value)))
        .collect();
    pairs.push(format!("limit={limit}"));
    if let Some(offset) = offset {
        pairs.push(format!("offset={offset}"));
    }
    format!("{base}{path}?{}", pairs.join("&"))
}

/// The `next`/`previous` links for a page, following the
/// LimitOffsetPagination semantics NetBox 4.x inherits from DRF
/// (`rest_framework/pagination.py`):
///
/// - `next` is present iff `offset + limit < count`, pointing at
///   `offset + limit`;
/// - `previous` is present iff `offset > 0`, pointing at
///   `offset - limit` (the `offset` parameter is removed when the
///   target is 0, like DRF's `remove_query_param`);
/// - links are absolute URLs preserving the filter parameters and
///   carrying the **effective** `limit`. DRF's
///   `get_next_link`/`get_previous_link` — which NetBox 4.x's
///   `OptionalLimitOffsetPagination` inherits unchanged — rewrite
///   the request URL with
///   `replace_query_param(url, "limit", self.limit)`: the request's
///   own `limit`/`offset` parameters are dropped and the effective
///   (defaulted or clamped) page size is appended after the
///   filters, exactly as [`link`] builds them. A request without a
///   `limit` therefore gets the default page size in its links,
///   and an oversized `limit` is echoed as the clamped value.
pub(crate) fn page_links(
    base: &str,
    path: &str,
    filters: &[(String, String)],
    count: usize,
    limit: usize,
    offset: usize,
) -> (Option<String>, Option<String>) {
    // saturating_add: a hostile `offset=18446744073709551615` must
    // not overflow-panic the math (a saturated sum simply exceeds
    // `count`, so there is no next page).
    let next_offset = offset.saturating_add(limit);
    let next = if next_offset < count {
        Some(link(base, path, filters, limit, Some(next_offset)))
    } else {
        None
    };
    let previous = if offset > 0 {
        let previous_offset = offset.saturating_sub(limit);
        Some(link(
            base,
            path,
            filters,
            limit,
            if previous_offset == 0 {
                None
            } else {
                Some(previous_offset)
            },
        ))
    } else {
        None
    };
    (next, previous)
}

// ---------------------------------------------------------------------------
// Display values
// ---------------------------------------------------------------------------

/// NetBox's `display` field for a row.
pub(crate) fn display_of(kind: SimKind, row: &Value) -> String {
    match kind {
        SimKind::Prefix => row
            .get("prefix")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        SimKind::IpAddress => row
            .get("address")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        _ => row
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_limit_matrix() {
        assert_eq!(effective_limit(None, 50, 1000), 50);
        // limit=0 is the server max page.
        assert_eq!(effective_limit(Some(0), 50, 1000), 1000);
        assert_eq!(effective_limit(Some(7), 50, 1000), 7);
        // Oversized limits clamp to the server max.
        assert_eq!(effective_limit(Some(5000), 50, 1000), 1000);
    }

    #[test]
    fn page_links_are_null_at_the_edges() {
        // First page of one: no links at all.
        assert_eq!(
            page_links("http://x", "/api/ipam/vlans/", &[], 1, 50, 0),
            (None, None)
        );
        // Exactly filling the page: next is null (offset+limit >= count).
        assert_eq!(
            page_links("http://x", "/api/ipam/vlans/", &[], 50, 50, 0),
            (None, None)
        );
    }

    #[test]
    fn page_links_follow_drf_limit_offset_semantics() {
        let filters = vec![("cf_chv_architecture_id".to_string(), "arch:1".to_string())];
        // Middle page.
        let (next, previous) = page_links("http://x", "/api/ipam/vlans/", &filters, 5, 2, 2);
        assert_eq!(
            next.as_deref(),
            Some("http://x/api/ipam/vlans/?cf_chv_architecture_id=arch%3A1&limit=2&offset=4")
        );
        assert_eq!(
            previous.as_deref(),
            Some("http://x/api/ipam/vlans/?cf_chv_architecture_id=arch%3A1&limit=2")
        );

        // Last page: next null.
        let (next, previous) = page_links("http://x", "/api/ipam/vlans/", &filters, 5, 2, 4);
        assert_eq!(next, None);
        assert_eq!(
            previous.as_deref(),
            Some("http://x/api/ipam/vlans/?cf_chv_architecture_id=arch%3A1&limit=2&offset=2")
        );
    }

    #[test]
    fn page_links_beyond_count_yield_empty_page_with_previous() {
        // offset past the end: no next, previous still walks back.
        let (next, previous) = page_links("http://x", "/api/ipam/vlans/", &[], 3, 2, 10);
        assert_eq!(next, None);
        assert_eq!(
            previous.as_deref(),
            Some("http://x/api/ipam/vlans/?limit=2&offset=8")
        );
    }

    #[test]
    fn page_links_survive_a_hostile_huge_offset() {
        // usize::MAX must saturate, not overflow-panic (debug
        // builds abort on overflow — this test runs in one).
        let (next, previous) = page_links("http://x", "/api/ipam/vlans/", &[], 3, 50, usize::MAX);
        assert_eq!(next, None, "no next page past the end");
        assert_eq!(
            previous.as_deref(),
            Some("http://x/api/ipam/vlans/?limit=50&offset=18446744073709551565")
        );
    }

    #[test]
    fn protected_errors_answer_409_with_a_detail_body() {
        let error = WireError::Protected {
            message: "Cannot delete VLAN 42.".to_string(),
        };
        assert_eq!(error.status(), 409);
        assert_eq!(
            error.body(),
            serde_json::json!({ "detail": "Cannot delete VLAN 42." })
        );
    }

    #[test]
    fn ip_natural_key_includes_the_mask() {
        // NetBox's unique constraint is on the full with-mask
        // address; the list-filter key stays mask-independent
        // (row_matches strips masks on both sides).
        let key = |address: &str| {
            natural_key(
                SimKind::IpAddress,
                &serde_json::json!({ "address": address }),
            )
        };
        assert_eq!(key("10.42.0.5/24"), key("10.42.0.5/24"));
        assert_ne!(key("10.42.0.5/24"), key("10.42.0.5/32"));
        // Stored rows always carry a mask: a maskless write is
        // normalized to /32 first (see `with_mask`, covered by
        // `masks_are_normalized_and_stripped`).
    }

    #[test]
    fn masks_are_normalized_and_stripped() {
        assert_eq!(with_mask("10.42.0.5"), "10.42.0.5/32");
        assert_eq!(with_mask("fe80::1"), "fe80::1/128");
        assert_eq!(with_mask("10.42.0.5/24"), "10.42.0.5/24");
        assert_eq!(strip_mask("10.42.0.5/24"), "10.42.0.5");
        assert_eq!(strip_mask("10.42.0.5"), "10.42.0.5");
    }

    #[test]
    fn slugs_follow_netbox_style() {
        assert_eq!(slugify("dc1"), "dc1");
        assert_eq!(slugify("My Site!"), "my-site");
        assert_eq!(slugify("chv-env-production"), "chv-env-production");
        assert_eq!(slugify("--x--"), "x");
    }

    #[test]
    fn unique_violation_uses_the_kind_field() {
        assert_eq!(
            unique_violation(SimKind::Vlan).body(),
            serde_json::json!({ "vid": ["This field must be unique."] })
        );
        assert_eq!(
            unique_violation(SimKind::Device).body(),
            serde_json::json!({ "name": ["This field must be unique."] })
        );
    }

    #[test]
    fn not_found_body_is_netbox_shaped() {
        assert_eq!(
            WireError::NotFound.body(),
            serde_json::json!({ "detail": "Not found." })
        );
        assert_eq!(WireError::NotFound.status(), 404);
    }
}
