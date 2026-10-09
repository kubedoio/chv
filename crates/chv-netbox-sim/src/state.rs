//! In-memory simulator state: per-kind object tables, id assignment,
//! fault injection, and the `__`-prefixed control-plane operations.
//!
//! Stored rows are the *normalized write form* plus `id`, `created`,
//! and `last_updated`. The NetBox read form (`url`, `display`, and
//! the derived `assigned_object` for IP addresses) is produced by
//! [`SimState::read_form`] at response time, so URLs always match the
//! host the request actually reached.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::config::NetboxSimConfig;
use crate::fault::FaultConfig;
use crate::kind::SimKind;
use crate::wire::{
    display_of, natural_key, normalize_choice, normalize_custom_fields, opt_nullable_i64,
    opt_nullable_number, opt_nullable_string, req_i64, req_string, row_matches, slugify,
    unique_violation, with_mask, WireError,
};

/// NetBox timestamp format: microsecond ISO-8601 UTC.
fn now_iso() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.6fZ")
        .to_string()
}

// ---------------------------------------------------------------------------
// Seed payload
// ---------------------------------------------------------------------------

/// Bulk-load payload for `POST /__seed` and the `netbox-sim` binary's
/// `--seed-file`. Each collection key matches a
/// [`SimKind::collection`]; entries are the write-body form of the
/// kind plus optional caller-chosen `id`, `created`, and
/// `last_updated` (defaults: auto-assigned id, now).
///
/// Unlike `POST`, seeding does **not** enforce natural-key
/// uniqueness — tests deliberately seed ambiguous remote state
/// (several rows sharing a natural key or external id), which is
/// exactly the client's "degrade to conflict" path.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeedPayload {
    #[serde(default)]
    pub devices: Vec<Value>,
    #[serde(default)]
    pub virtual_machines: Vec<Value>,
    #[serde(default)]
    pub interfaces: Vec<Value>,
    #[serde(default)]
    pub prefixes: Vec<Value>,
    #[serde(default)]
    pub vlans: Vec<Value>,
    #[serde(default)]
    pub ip_addresses: Vec<Value>,
}

impl SeedPayload {
    /// The entries for one kind.
    pub fn for_kind(&self, kind: SimKind) -> &[Value] {
        match kind {
            SimKind::Device => &self.devices,
            SimKind::VirtualMachine => &self.virtual_machines,
            SimKind::Interface => &self.interfaces,
            SimKind::Prefix => &self.prefixes,
            SimKind::Vlan => &self.vlans,
            SimKind::IpAddress => &self.ip_addresses,
        }
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// The simulator's mutable state. All operations are synchronous and
/// non-blocking; the server locks briefly per request and never
/// holds the lock across an await.
pub struct SimState {
    /// Normalized rows per kind, ordered by id (NetBox's default
    /// list ordering for these endpoints).
    rows: BTreeMap<SimKind, BTreeMap<i64, Value>>,
    /// Monotonic object-id counter.
    next_id: i64,
    /// Synthetic ids for nested relations that do not (yet) point at
    /// a real row (see the crate docs' fidelity notes), keyed by
    /// `(relation tag, name-or-vid)`.
    relation_ids: BTreeMap<(&'static str, String), i64>,
    next_relation_id: i64,
    /// Injected faults: `None` key is the global scope, `Some(kind)`
    /// a per-kind override that replaces the global scope.
    faults: BTreeMap<Option<SimKind>, FaultConfig>,
}

impl Default for SimState {
    fn default() -> Self {
        Self::new()
    }
}

impl SimState {
    pub fn new() -> Self {
        let rows = SimKind::ALL
            .into_iter()
            .map(|kind| (kind, BTreeMap::new()))
            .collect();
        Self {
            rows,
            next_id: 1,
            relation_ids: BTreeMap::new(),
            next_relation_id: 1,
            faults: BTreeMap::new(),
        }
    }

    /// The stored row for `(kind, id)`.
    pub fn row(&self, kind: SimKind, id: i64) -> Option<&Value> {
        self.rows.get(&kind)?.get(&id)
    }

    // -- relation registry ------------------------------------------------

    fn relation_id(&mut self, tag: &'static str, key: String) -> i64 {
        if let Some(id) = self.relation_ids.get(&(tag, key.clone())) {
            return *id;
        }
        let id = self.next_relation_id;
        self.next_relation_id += 1;
        self.relation_ids.insert((tag, key), id);
        id
    }

    /// The id of the first row of `kind` whose natural key matches
    /// `name` (rows are created parents-first, so a match is
    /// unambiguous in practice).
    fn row_id_by_name(&self, kind: SimKind, name: &str) -> Option<i64> {
        let table = self.rows.get(&kind)?;
        table
            .values()
            .find(|row| {
                row.get("name").and_then(Value::as_str) == Some(name)
                    || (kind == SimKind::Vlan
                        && row
                            .get("vid")
                            .and_then(Value::as_i64)
                            .map(|vid| vid.to_string())
                            .as_deref()
                            == Some(name))
            })
            .and_then(|row| row.get("id"))
            .and_then(Value::as_i64)
    }

    /// Normalize a `{"name": ...}` relation reference to
    /// `{"id": ..., "name": ...}`, preferring a real row id when one
    /// exists (falls back to the synthetic registry).
    fn normalize_named_ref(
        &mut self,
        kind: SimKind,
        tag: &'static str,
        value: Option<&Value>,
        field: &'static str,
    ) -> Result<Value, WireError> {
        match value {
            None | Some(Value::Null) => Ok(Value::Null),
            Some(Value::Object(relation)) => {
                let name =
                    relation
                        .get("name")
                        .and_then(Value::as_str)
                        .ok_or(WireError::Field {
                            field,
                            message: "Must carry a name.",
                        })?;
                let id = self
                    .row_id_by_name(kind, name)
                    .unwrap_or_else(|| self.relation_id(tag, name.to_string()));
                Ok(json!({ "id": id, "name": name }))
            }
            Some(_) => Err(WireError::Field {
                field,
                message: "A valid object is required.",
            }),
        }
    }

    /// Normalize a prefix's `{"vid": ...}` VLAN reference to
    /// `{"id": ..., "vid": ..., "name": ...}`.
    fn normalize_vlan_ref(&mut self, value: Option<&Value>) -> Result<Value, WireError> {
        match value {
            None | Some(Value::Null) => Ok(Value::Null),
            Some(Value::Object(vlan)) => {
                let vid = vlan
                    .get("vid")
                    .and_then(Value::as_i64)
                    .ok_or(WireError::Field {
                        field: "vlan",
                        message: "Must carry a vid.",
                    })?;
                let existing = self.rows.get(&SimKind::Vlan).and_then(|table| {
                    table
                        .values()
                        .find(|row| row.get("vid").and_then(Value::as_i64) == Some(vid))
                        .map(|row| {
                            (
                                row.get("id").and_then(Value::as_i64).unwrap_or_default(),
                                row.get("name").and_then(Value::as_str).map(str::to_string),
                            )
                        })
                });
                let (id, name) = match existing {
                    Some(found) => found,
                    None => (self.relation_id("vlan", vid.to_string()), None),
                };
                Ok(json!({ "id": id, "vid": vid, "name": name }))
            }
            Some(_) => Err(WireError::Field {
                field: "vlan",
                message: "A valid object is required.",
            }),
        }
    }

    /// Normalize a tags list (write form: slugs; merge form: nested
    /// objects) to NetBox's nested read form.
    fn normalize_tags(&mut self, object: &Map<String, Value>) -> Result<Value, WireError> {
        match object.get("tags") {
            None | Some(Value::Null) => Ok(json!([])),
            Some(Value::Array(tags)) => {
                let mut out = Vec::with_capacity(tags.len());
                for tag in tags {
                    let (name, slug) = match tag {
                        Value::String(slug) => (slug.clone(), slugify(slug)),
                        Value::Object(tag) => {
                            let name = tag
                                .get("name")
                                .or_else(|| tag.get("slug"))
                                .and_then(Value::as_str)
                                .ok_or(WireError::Field {
                                    field: "tags",
                                    message: "Expected tag names.",
                                })?
                                .to_string();
                            let slug = tag
                                .get("slug")
                                .and_then(Value::as_str)
                                .map(str::to_string)
                                .unwrap_or_else(|| slugify(&name));
                            (name, slug)
                        }
                        _ => {
                            return Err(WireError::Field {
                                field: "tags",
                                message: "Expected tag names.",
                            })
                        }
                    };
                    let id = self.relation_id("tag", slug.clone());
                    out.push(json!({ "id": id, "name": name, "slug": slug }));
                }
                Ok(json!(out))
            }
            Some(_) => Err(WireError::Field {
                field: "tags",
                message: "Expected a list of tags.",
            }),
        }
    }

    /// Normalize a write body (create, patch-merge, or seed entry) to
    /// the stored form for `kind`. Only contract fields are kept;
    /// everything the client does not send is defaulted per kind.
    fn normalize_write(
        &mut self,
        kind: SimKind,
        body: &Value,
    ) -> Result<Map<String, Value>, WireError> {
        let Some(object) = body.as_object() else {
            return Err(WireError::Detail {
                message: "Invalid request body.",
            });
        };
        let mut out = Map::new();
        match kind {
            SimKind::Device => {
                out.insert("name".into(), json!(req_string(object, "name")?));
                out.insert(
                    "status".into(),
                    normalize_choice(object, "status", "active")?,
                );
                out.insert(
                    "site".into(),
                    self.normalize_named_ref(SimKind::Device, "site", object.get("site"), "site")?,
                );
                out.insert("tags".into(), self.normalize_tags(object)?);
                out.insert(
                    "custom_fields".into(),
                    json!(normalize_custom_fields(object)?),
                );
            }
            SimKind::VirtualMachine => {
                out.insert("name".into(), json!(req_string(object, "name")?));
                out.insert(
                    "status".into(),
                    normalize_choice(object, "status", "active")?,
                );
                out.insert(
                    "cluster".into(),
                    self.normalize_named_ref(
                        SimKind::VirtualMachine,
                        "cluster",
                        object.get("cluster"),
                        "cluster",
                    )?,
                );
                out.insert(
                    "device".into(),
                    self.normalize_named_ref(
                        SimKind::Device,
                        "device",
                        object.get("device"),
                        "device",
                    )?,
                );
                out.insert("vcpus".into(), opt_nullable_number(object, "vcpus")?);
                out.insert("memory".into(), opt_nullable_number(object, "memory")?);
                out.insert("tags".into(), self.normalize_tags(object)?);
                out.insert(
                    "custom_fields".into(),
                    json!(normalize_custom_fields(object)?),
                );
            }
            SimKind::Interface => {
                out.insert("name".into(), json!(req_string(object, "name")?));
                out.insert(
                    "virtual_machine".into(),
                    self.normalize_named_ref(
                        SimKind::VirtualMachine,
                        "virtual_machine",
                        object.get("virtual_machine"),
                        "virtual_machine",
                    )?,
                );
                out.insert(
                    "description".into(),
                    opt_nullable_string(object, "description")?,
                );
                out.insert("type".into(), normalize_choice(object, "type", "virtual")?);
                out.insert("tags".into(), self.normalize_tags(object)?);
                out.insert(
                    "custom_fields".into(),
                    json!(normalize_custom_fields(object)?),
                );
            }
            SimKind::Prefix => {
                out.insert("prefix".into(), json!(req_string(object, "prefix")?));
                out.insert("vlan".into(), self.normalize_vlan_ref(object.get("vlan"))?);
                out.insert(
                    "description".into(),
                    opt_nullable_string(object, "description")?,
                );
                out.insert("tags".into(), self.normalize_tags(object)?);
                out.insert(
                    "custom_fields".into(),
                    json!(normalize_custom_fields(object)?),
                );
            }
            SimKind::Vlan => {
                out.insert("vid".into(), json!(req_i64(object, "vid")?));
                out.insert("name".into(), json!(req_string(object, "name")?));
                out.insert("tags".into(), self.normalize_tags(object)?);
                out.insert(
                    "custom_fields".into(),
                    json!(normalize_custom_fields(object)?),
                );
            }
            SimKind::IpAddress => {
                out.insert(
                    "address".into(),
                    json!(with_mask(&req_string(object, "address")?)),
                );
                out.insert(
                    "assigned_object_type".into(),
                    opt_nullable_string(object, "assigned_object_type")?,
                );
                out.insert(
                    "assigned_object_id".into(),
                    opt_nullable_i64(object, "assigned_object_id")?,
                );
                out.insert("tags".into(), self.normalize_tags(object)?);
                out.insert(
                    "custom_fields".into(),
                    json!(normalize_custom_fields(object)?),
                );
            }
        }
        Ok(out)
    }

    /// Whether any row other than `except_id` already occupies
    /// `key`.
    fn natural_key_taken(
        &self,
        kind: SimKind,
        key: &[(String, String)],
        except_id: Option<i64>,
    ) -> bool {
        self.rows
            .get(&kind)
            .map(|table| {
                table
                    .iter()
                    .any(|(id, row)| Some(*id) != except_id && natural_key(kind, row) == key)
            })
            .unwrap_or(false)
    }

    // -- NetBox-surface mutations ------------------------------------------

    /// `POST`: normalize, enforce natural-key uniqueness, assign
    /// `id`/`url`/timestamps, store. Returns the new id.
    pub fn create(&mut self, kind: SimKind, body: &Value) -> Result<i64, WireError> {
        let mut stored = self.normalize_write(kind, body)?;
        let key = natural_key(kind, &Value::Object(stored.clone()));
        if self.natural_key_taken(kind, &key, None) {
            return Err(unique_violation(kind));
        }
        let id = self.next_id;
        self.next_id += 1;
        let now = now_iso();
        stored.insert("id".into(), json!(id));
        stored.insert("created".into(), json!(now));
        stored.insert("last_updated".into(), json!(now));
        if let Some(table) = self.rows.get_mut(&kind) {
            table.insert(id, Value::Object(stored));
        }
        Ok(id)
    }

    /// `PATCH`: merge the patch into the stored row (custom fields
    /// merge per key — see the crate docs' fidelity notes), re-check
    /// natural-key uniqueness, bump `last_updated`.
    pub fn patch(&mut self, kind: SimKind, id: i64, body: &Value) -> Result<(), WireError> {
        let current = match self.row(kind, id) {
            Some(Value::Object(current)) => current.clone(),
            _ => return Err(WireError::NotFound),
        };
        let Some(patch) = body.as_object() else {
            return Err(WireError::Detail {
                message: "Invalid request body.",
            });
        };
        // Top-level: patch fields replace current fields.
        let mut merged = current.clone();
        for (key, value) in patch {
            merged.insert(key.clone(), value.clone());
        }
        // custom_fields: merge per key so partial patches (the
        // adapter's `mark_stale`) keep unmentioned keys.
        if let Some(Value::Object(patch_fields)) = patch.get("custom_fields") {
            let mut merged_fields = current
                .get("custom_fields")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            for (key, value) in patch_fields {
                merged_fields.insert(key.clone(), value.clone());
            }
            merged.insert("custom_fields".to_string(), Value::Object(merged_fields));
        }
        let mut stored = self.normalize_write(kind, &Value::Object(merged))?;
        let key = natural_key(kind, &Value::Object(stored.clone()));
        if self.natural_key_taken(kind, &key, Some(id)) {
            return Err(unique_violation(kind));
        }
        stored.insert("id".into(), json!(id));
        stored.insert(
            "created".into(),
            current
                .get("created")
                .cloned()
                .unwrap_or_else(|| json!(now_iso())),
        );
        stored.insert("last_updated".into(), json!(now_iso()));
        if let Some(table) = self.rows.get_mut(&kind) {
            table.insert(id, Value::Object(stored));
        }
        Ok(())
    }

    /// `DELETE`: remove the row, applying the cascade semantics the
    /// client's write paths rely on — deleting a VM removes its
    /// interfaces (and unassigns their IP addresses), deleting an
    /// interface unassigns its IP addresses, deleting a VLAN clears
    /// the reference from prefixes.
    pub fn delete(&mut self, kind: SimKind, id: i64) -> Result<(), WireError> {
        let row = {
            let Some(table) = self.rows.get_mut(&kind) else {
                return Err(WireError::NotFound);
            };
            let Some(row) = table.remove(&id) else {
                return Err(WireError::NotFound);
            };
            row
        };
        match kind {
            SimKind::VirtualMachine => {
                if let Some(vm_name) = row.get("name").and_then(Value::as_str) {
                    let interface_ids: Vec<i64> = self
                        .rows
                        .get(&SimKind::Interface)
                        .map(|table| {
                            table
                                .iter()
                                .filter(|(_, interface)| {
                                    interface
                                        .get("virtual_machine")
                                        .and_then(|vm| vm.get("name"))
                                        .and_then(Value::as_str)
                                        == Some(vm_name)
                                })
                                .map(|(interface_id, _)| *interface_id)
                                .collect()
                        })
                        .unwrap_or_default();
                    for interface_id in interface_ids {
                        if let Some(table) = self.rows.get_mut(&SimKind::Interface) {
                            table.remove(&interface_id);
                        }
                        self.unassign_ips_of_interface(interface_id);
                    }
                }
            }
            SimKind::Interface => self.unassign_ips_of_interface(id),
            SimKind::Vlan => {
                if let Some(vid) = row.get("vid").and_then(Value::as_i64) {
                    if let Some(table) = self.rows.get_mut(&SimKind::Prefix) {
                        for prefix in table.values_mut() {
                            if prefix
                                .get("vlan")
                                .and_then(|vlan| vlan.get("vid"))
                                .and_then(Value::as_i64)
                                == Some(vid)
                            {
                                if let Some(map) = prefix.as_object_mut() {
                                    map.insert("vlan".to_string(), Value::Null);
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn unassign_ips_of_interface(&mut self, interface_id: i64) {
        if let Some(table) = self.rows.get_mut(&SimKind::IpAddress) {
            for address in table.values_mut() {
                if address.get("assigned_object_id").and_then(Value::as_i64) == Some(interface_id) {
                    if let Some(map) = address.as_object_mut() {
                        map.insert("assigned_object_type".to_string(), Value::Null);
                        map.insert("assigned_object_id".to_string(), Value::Null);
                    }
                }
            }
        }
    }

    // -- NetBox-surface queries --------------------------------------------

    /// Filtered rows for a list request: every row matching the
    /// request's query parameters, in id order, plus the total count.
    pub fn list(
        &self,
        kind: SimKind,
        params: &[(String, String)],
    ) -> Result<(usize, Vec<Value>), WireError> {
        let table = self.rows.get(&kind);
        let mut matched = Vec::new();
        if let Some(table) = table {
            for row in table.values() {
                if row_matches(kind, row, params)? {
                    matched.push(row.clone());
                }
            }
        }
        let count = matched.len();
        Ok((count, matched))
    }

    /// The NetBox read form of a stored row: adds `url` and `display`
    /// under `base` (e.g. `http://127.0.0.1:PORT`) and, for IP
    /// addresses, the derived `assigned_object`.
    pub fn read_form(&self, kind: SimKind, row: &Value, base: &str) -> Value {
        let mut object = row.clone();
        let id = row.get("id").and_then(Value::as_i64).unwrap_or_default();
        if let Some(map) = object.as_object_mut() {
            map.insert(
                "url".to_string(),
                json!(format!("{base}{}{id}/", kind.api_path())),
            );
            map.insert("display".to_string(), json!(display_of(kind, row)));
        }
        if kind == SimKind::IpAddress {
            if let Some(map) = object.as_object_mut() {
                map.insert(
                    "assigned_object".to_string(),
                    self.derive_assigned_object(row),
                );
            }
        }
        object
    }

    /// Resolve an IP address's `assigned_object` from the interface
    /// table (the only assignment type the adapter client writes).
    fn derive_assigned_object(&self, row: &Value) -> Value {
        let assigned_type = row.get("assigned_object_type").and_then(Value::as_str);
        let assigned_id = row.get("assigned_object_id").and_then(Value::as_i64);
        match (assigned_type, assigned_id) {
            (Some("virtualization.vminterface"), Some(interface_id)) => {
                match self.row(SimKind::Interface, interface_id) {
                    Some(interface) => json!({
                        "id": interface_id,
                        "name": interface.get("name").cloned().unwrap_or(Value::Null),
                        "virtual_machine": interface.get("virtual_machine").cloned().unwrap_or(Value::Null),
                    }),
                    None => Value::Null,
                }
            }
            _ => Value::Null,
        }
    }

    // -- control plane ------------------------------------------------------

    /// `POST /__seed`: bulk-load objects. See [`SeedPayload`] for the
    /// relaxed rules (caller-chosen ids, no uniqueness enforcement).
    pub fn seed(&mut self, payload: &SeedPayload) -> Result<BTreeMap<SimKind, usize>, WireError> {
        let mut counts = BTreeMap::new();
        for kind in SimKind::ALL {
            let mut seeded = 0usize;
            for entry in payload.for_kind(kind) {
                let mut stored = self.normalize_write(kind, entry)?;
                let id = match entry.get("id").and_then(Value::as_i64) {
                    Some(id) => id,
                    None => {
                        let id = self.next_id;
                        self.next_id += 1;
                        id
                    }
                };
                if self
                    .rows
                    .get(&kind)
                    .is_some_and(|table| table.contains_key(&id))
                {
                    return Err(WireError::Detail {
                        message: "Seed entry reuses an existing id.",
                    });
                }
                let created = entry
                    .get("created")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(now_iso);
                let last_updated = entry
                    .get("last_updated")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| created.clone());
                stored.insert("id".into(), json!(id));
                stored.insert("created".into(), json!(created));
                stored.insert("last_updated".into(), json!(last_updated));
                // Keep the id counter monotonic past caller-chosen ids.
                self.next_id = self.next_id.max(id + 1);
                if let Some(table) = self.rows.get_mut(&kind) {
                    table.insert(id, Value::Object(stored));
                }
                seeded += 1;
            }
            counts.insert(kind, seeded);
        }
        Ok(counts)
    }

    /// `POST /__reset`: clear objects, faults, and counters.
    pub fn reset(&mut self) {
        for table in self.rows.values_mut() {
            table.clear();
        }
        self.next_id = 1;
        self.relation_ids.clear();
        self.next_relation_id = 1;
        self.faults.clear();
    }

    /// `GET /__state`: full state dump (read-form rows under the
    /// request's host, faults, next id).
    pub fn dump(&self, base: &str) -> Value {
        let mut objects = Map::new();
        for kind in SimKind::ALL {
            let rows: Vec<Value> = self
                .rows
                .get(&kind)
                .map(|table| {
                    table
                        .values()
                        .map(|row| self.read_form(kind, row, base))
                        .collect()
                })
                .unwrap_or_default();
            objects.insert(kind.collection().to_string(), json!(rows));
        }
        json!({
            "objects": objects,
            "faults": self.faults_value(),
            "next_id": self.next_id,
        })
    }

    /// `POST /__faults`: set (replace) the fault configuration for a
    /// scope.
    pub fn set_fault(&mut self, kind: Option<SimKind>, fault: FaultConfig) {
        self.faults.insert(kind, fault);
    }

    /// The fault configuration applying to requests for `kind`
    /// (a per-kind entry replaces the global one).
    pub fn effective_fault(&self, kind: SimKind) -> FaultConfig {
        self.faults
            .get(&Some(kind))
            .or_else(|| self.faults.get(&None))
            .cloned()
            .unwrap_or_default()
    }

    /// The current fault configuration, in the `/__state` shape.
    pub fn faults_value(&self) -> Value {
        let mut kinds = Map::new();
        let mut global = Value::Null;
        for (scope, fault) in &self.faults {
            match scope {
                None => global = json!(fault),
                Some(kind) => {
                    kinds.insert(kind.as_str().to_string(), json!(fault));
                }
            }
        }
        json!({ "global": global, "kinds": kinds })
    }
}

// ---------------------------------------------------------------------------
// Shared handle
// ---------------------------------------------------------------------------

/// The state plus the immutable config, shared between the request
/// handlers and the [`crate::NetboxSim`] handle.
pub struct SimShared {
    config: NetboxSimConfig,
    state: Mutex<SimState>,
}

impl SimShared {
    pub fn new(config: NetboxSimConfig) -> Self {
        Self {
            config,
            state: Mutex::new(SimState::new()),
        }
    }

    /// The simulator configuration.
    pub fn config(&self) -> &NetboxSimConfig {
        &self.config
    }

    /// Lock the state. Poisoning (a panic while holding the lock,
    /// which the handlers never do) recovers the inner state instead
    /// of panicking in a request handler.
    pub fn lock(&self) -> MutexGuard<'_, SimState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn device_body(name: &str) -> Value {
        json!({
            "name": name,
            "status": "active",
            "site": { "name": "dc1" },
            "tags": ["chv-team"],
            "custom_fields": { "chv_managed_by": "chv" },
        })
    }

    #[test]
    fn create_assigns_ids_and_timestamps() {
        let mut state = SimState::new();
        let id = state
            .create(SimKind::Device, &device_body("chv-node-01"))
            .expect("create");
        assert_eq!(id, 1);
        let row = state.row(SimKind::Device, id).expect("stored");
        assert!(row.get("created").and_then(Value::as_str).is_some());
        assert!(row.get("last_updated").and_then(Value::as_str).is_some());
        // Status is normalized to the read (choice object) form.
        assert_eq!(row["status"]["value"], json!("active"));
        // Site reference resolves through the registry.
        assert_eq!(row["site"]["name"], json!("dc1"));
        // Tags are normalized to NetBox's nested form.
        assert_eq!(row["tags"][0]["slug"], json!("chv-team"));
    }

    #[test]
    fn duplicate_natural_key_is_rejected() {
        let mut state = SimState::new();
        state
            .create(SimKind::Device, &device_body("dup"))
            .expect("first create");
        let err = state
            .create(SimKind::Device, &device_body("dup"))
            .expect_err("duplicate rejected");
        assert_eq!(
            err.body(),
            json!({ "name": ["This field must be unique."] })
        );
    }

    #[test]
    fn patch_merges_custom_fields_per_key() {
        let mut state = SimState::new();
        let id = state
            .create(SimKind::Device, &device_body("chv-node-01"))
            .expect("create");
        // A partial custom_fields patch (the adapter's mark_stale
        // shape) must keep the unmentioned keys.
        state
            .patch(
                SimKind::Device,
                id,
                &json!({
                    "status": "decommissioning",
                    "custom_fields": { "chv_managed_state": "stale" },
                }),
            )
            .expect("patch");
        let row = state.row(SimKind::Device, id).expect("stored");
        assert_eq!(row["status"]["value"], json!("decommissioning"));
        assert_eq!(row["custom_fields"]["chv_managed_by"], json!("chv"));
        assert_eq!(row["custom_fields"]["chv_managed_state"], json!("stale"));
    }

    #[test]
    fn patch_unknown_id_is_not_found() {
        let mut state = SimState::new();
        let err = state
            .patch(SimKind::Device, 99, &json!({ "status": "offline" }))
            .expect_err("unknown id");
        assert_eq!(err, WireError::NotFound);
    }

    #[test]
    fn deleting_a_vm_cascades_to_interfaces_and_ip_assignments() {
        let mut state = SimState::new();
        let vm = state
            .create(
                SimKind::VirtualMachine,
                &json!({ "name": "vm-01", "status": "active" }),
            )
            .expect("vm");
        let interface = state
            .create(
                SimKind::Interface,
                &json!({
                    "name": "backend",
                    "virtual_machine": { "name": "vm-01" },
                    "description": "backend",
                    "type": "virtual",
                }),
            )
            .expect("interface");
        let ip = state
            .create(
                SimKind::IpAddress,
                &json!({
                    "address": "10.42.0.5",
                    "assigned_object_type": "virtualization.vminterface",
                    "assigned_object_id": interface,
                }),
            )
            .expect("ip");
        state
            .delete(SimKind::VirtualMachine, vm)
            .expect("delete vm");
        assert!(state.row(SimKind::Interface, interface).is_none());
        let row = state.row(SimKind::IpAddress, ip).expect("ip survives");
        // The assignment was cleared, not dangled.
        assert_eq!(row["assigned_object_id"], Value::Null);
        // The derived read form shows no assignment either.
        let read = state.read_form(SimKind::IpAddress, row, "http://x");
        assert_eq!(read["assigned_object"], Value::Null);
    }

    #[test]
    fn seed_allows_duplicate_natural_keys_and_chosen_ids() {
        let mut state = SimState::new();
        let payload: SeedPayload = serde_json::from_value(json!({
            "vlans": [
                { "id": 42, "vid": 42, "name": "backend", "created": "2026-10-09T12:00:00.000000Z" },
                { "id": 43, "vid": 42, "name": "backend-2" },
            ]
        }))
        .expect("parses");
        let counts = state.seed(&payload).expect("seeds");
        assert_eq!(counts[&SimKind::Vlan], 2);
        assert!(state.row(SimKind::Vlan, 42).is_some());
        assert!(state.row(SimKind::Vlan, 43).is_some());
        // Caller-chosen created is honored.
        assert_eq!(
            state.row(SimKind::Vlan, 42).expect("seeded vlan")["created"],
            json!("2026-10-09T12:00:00.000000Z")
        );
        // The id counter moved past the chosen ids.
        let next = state
            .create(SimKind::Vlan, &json!({ "vid": 44, "name": "next" }))
            .expect("create");
        assert_eq!(next, 44);
    }

    #[test]
    fn reset_clears_objects_faults_and_counters() {
        let mut state = SimState::new();
        state
            .seed(&SeedPayload {
                vlans: vec![json!({ "vid": 42, "name": "backend" })],
                ..SeedPayload::default()
            })
            .expect("seed");
        state.set_fault(
            None,
            FaultConfig {
                rate_limit: true,
                ..FaultConfig::default()
            },
        );
        state.reset();
        let dump = state.dump("http://x");
        assert_eq!(dump["next_id"], json!(1));
        assert_eq!(dump["faults"]["global"], Value::Null);
        assert_eq!(dump["objects"]["vlans"], json!([]));
    }

    #[test]
    fn per_kind_fault_replaces_global_scope() {
        let mut state = SimState::new();
        state.set_fault(
            None,
            FaultConfig {
                rate_limit: true,
                ..FaultConfig::default()
            },
        );
        state.set_fault(Some(SimKind::Vlan), FaultConfig::default());
        assert!(state.effective_fault(SimKind::Prefix).rate_limit);
        // The (inactive) per-kind entry replaces the global one.
        assert!(!state.effective_fault(SimKind::Vlan).rate_limit);
    }

    #[test]
    fn interface_parent_prefers_a_real_vm_row() {
        let mut state = SimState::new();
        let vm = state
            .create(
                SimKind::VirtualMachine,
                &json!({ "name": "vm-01", "status": "active" }),
            )
            .expect("vm");
        let interface = state
            .create(
                SimKind::Interface,
                &json!({ "name": "backend", "virtual_machine": { "name": "vm-01" } }),
            )
            .expect("interface");
        let row = state.row(SimKind::Interface, interface).expect("stored");
        assert_eq!(row["virtual_machine"]["id"], json!(vm));
        // Without a row, the synthetic registry still resolves.
        let other = state
            .create(
                SimKind::Interface,
                &json!({ "name": "backend", "virtual_machine": { "name": "ghost-vm" } }),
            )
            .expect("interface");
        let row = state.row(SimKind::Interface, other).expect("stored");
        // First synthetic registry allocation.
        assert_eq!(row["virtual_machine"]["id"], json!(1));
        assert_eq!(row["virtual_machine"]["name"], json!("ghost-vm"));
    }
}
