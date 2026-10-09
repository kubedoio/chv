//! Golden-fixture recorder — the `--record` half of the real-NetBox
//! qualification lane (ADR-024 decision 4, issue kubedoio/chv#586,
//! PR 5 of the #586 campaign).
//!
//! One `#[ignore]`d test, run only by `scripts/netbox-qualify.sh
//! --record`: it empties the qualification instance, provisions the
//! prerequisites a real NetBox needs (site, device type, device
//! role, tag, the `chv_` custom fields), seeds the canonical one-object-per-family
//! set, captures the six list responses, and rewrites
//! `tests/fixtures/netbox4/` — the golden fixtures the simulator's
//! fidelity suite pins. Refreshing the fixtures through this recorder
//! (never by hand) is what makes them recorded evidence rather than
//! an authored hypothesis.
//!
//! # Environment
//!
//! - `NETBOX_QUALIFICATION_URL` + `NETBOX_QUALIFICATION_TOKEN`
//!   (required): the live instance, the same pair the qualification
//!   suite uses.
//! - `NETBOX_RECORD_BASE_URL` (optional): the stable base the
//!   recorded `url` fields are rewritten onto. The qualify script
//!   passes `http://netbox.example.com` to match the fixtures'
//!   canonical base; the default is the qualification URL itself.
//! - `NETBOX_FIXTURE_DIR` (optional): output directory, defaulting
//!   to this crate's `tests/fixtures/netbox4`.
//!
//! # Determinism
//!
//! A live capture is not byte-reproducible (ids, timestamps, and the
//! instance's host all vary), so the recorder canonicalizes exactly
//! those: object ids are renumbered to a stable `1..N` in fixture
//! order (devices, virtual machines, interfaces, prefixes, vlans, ip
//! addresses), six-family relation ids are remapped through the same
//! table (site, tag, and cluster ids stay as captured — they are
//! outside the six families), `created`/`last_updated` are pinned to
//! a fixed timestamp, and every `url` is rebuilt on the record base.
//! Everything else is the live serializer's output reduced to the
//! simulator's read form by `capture::normalize_object` — anything
//! that survives is exactly what the fixtures pin.
//!
//! The recorded `seed.json` re-plays the captured rows (minus
//! `url`/`display`/`assigned_object`) through the simulator's seed
//! format; the nested relation ids it carries are honored on re-seed
//! (see `state.rs`'s relation registry), which is what lets the
//! fidelity suite round-trip a capture.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chv_netbox_adapter::ownership::CustomFieldNames;
use chv_netbox_adapter::{CHV_NETBOX_DEVICE_ROLE, CHV_NETBOX_DEVICE_TYPE, CHV_NETBOX_MANUFACTURER};
use chv_netbox_sim::capture::{
    normalize_object, CaptureError, LiveNetBox, QUALIFICATION_TOKEN_ENV, QUALIFICATION_URL_ENV,
};
use chv_netbox_sim::SimKind;
use serde_json::{json, Value};

/// The pinned timestamp every recorded `created`/`last_updated` is
/// canonicalized to (the value the authored fixtures already carry).
const CANONICAL_TIMESTAMP: &str = "2026-10-09T12:00:00.000000Z";

/// Capture order — the order the fixture files (and the renumbered
/// `1..N` ids) are written in.
const FIXTURE_ORDER: [SimKind; 6] = [
    SimKind::Device,
    SimKind::VirtualMachine,
    SimKind::Interface,
    SimKind::Prefix,
    SimKind::Vlan,
    SimKind::IpAddress,
];

/// List page size for the capture requests: comfortably above the
/// canonical one-object-per-family content, under NetBox's default
/// max page.
const LIST_PAGE: usize = 200;

/// Every `chv_` custom-field name the adapter's mapping can write on
/// a projected object — composed from the adapter's own
/// [`CustomFieldNames`] (`ownership_fields()` + `enrichment_fields()`,
/// the single source of truth), so a contract change there flows into
/// this provisioning list instead of a second, forgettable list. All
/// fields are provisioned as NetBox `text` fields: the adapter only
/// ever writes string values (numbers as their decimal strings,
/// datastore facts as a comma-joined `name:kind` list).
fn qualification_custom_fields() -> Vec<String> {
    let names = CustomFieldNames::default();
    names
        .ownership_fields()
        .iter()
        .map(|field| field.to_string())
        .chain(names.enrichment_fields())
        .collect()
}

/// The content types the custom fields are assigned to.
const CONTENT_TYPES: [&str; 6] = [
    "dcim.device",
    "virtualization.virtualmachine",
    "virtualization.vminterface",
    "ipam.prefix",
    "ipam.vlan",
    "ipam.ipaddress",
];

/// Record the golden fixtures from the qualification instance.
#[tokio::test]
#[ignore = "destructive: empties and re-seeds the instance, then rewrites tests/fixtures/netbox4 (run via scripts/netbox-qualify.sh --record)"]
async fn record_netbox4_fixtures() {
    let url = std::env::var(QUALIFICATION_URL_ENV).unwrap_or_else(|_| {
        panic!(
            "fixture recording requires ${QUALIFICATION_URL_ENV} \
             (and ${QUALIFICATION_TOKEN_ENV}) — run it through \
             scripts/netbox-qualify.sh --record"
        )
    });
    let token = std::env::var(QUALIFICATION_TOKEN_ENV).unwrap_or_else(|_| {
        panic!(
            "fixture recording requires ${QUALIFICATION_TOKEN_ENV} \
             (and ${QUALIFICATION_URL_ENV}) — run it through \
             scripts/netbox-qualify.sh --record"
        )
    });
    let fixture_dir = PathBuf::from(
        std::env::var("NETBOX_FIXTURE_DIR")
            .unwrap_or_else(|_| format!("{}/tests/fixtures/netbox4", env!("CARGO_MANIFEST_DIR"))),
    );
    let record_base = std::env::var("NETBOX_RECORD_BASE_URL").unwrap_or_else(|_| url.clone());
    let live = LiveNetBox::new(&url, &token);

    // Dialect: a real NetBox answers GET /api/status/ with its
    // version; the simulator 404s everything outside the six
    // families. Real writes need PK references and prerequisite
    // objects; simulator writes use the name-dict forms its wire
    // layer accepts.
    let (real, netbox_version) = match live.api_status().await {
        Ok(status) => (
            true,
            status
                .get("netbox-version")
                .and_then(Value::as_str)
                .map(str::to_string),
        ),
        Err(_) => (false, None),
    };

    live.delete_all()
        .await
        .expect("the instance is emptied before recording");

    let provisions = if real {
        provision(&live)
            .await
            .expect("real-NetBox prerequisites are provisioned")
    } else {
        Provisions::default()
    };
    seed_canonical_set(&live, real, &provisions)
        .await
        .expect("the canonical set is seeded");

    // Capture, normalize, and renumber in fixture order.
    let mut id_map: BTreeMap<i64, i64> = BTreeMap::new();
    let mut captured: Vec<(SimKind, Vec<Value>)> = Vec::new();
    for kind in FIXTURE_ORDER {
        let rows = capture_family(&live, kind)
            .await
            .unwrap_or_else(|error| panic!("capture {kind} failed: {error}"));
        for row in &rows {
            let old_id = row["id"].as_i64().unwrap_or_else(|| {
                panic!("captured {kind} row carries no id: {row}");
            });
            let new_id = id_map.len() as i64 + 1;
            id_map.insert(old_id, new_id);
        }
        captured.push((kind, rows));
    }

    // Rewrite every row onto the canonical ids, remapped relations,
    // and pinned timestamps; write the six list fixtures.
    fs::create_dir_all(&fixture_dir).expect("fixture directory exists");
    let mut by_kind: BTreeMap<SimKind, Vec<Value>> = BTreeMap::new();
    for (kind, rows) in captured {
        let mut recorded = Vec::new();
        for row in rows {
            let mut normalized = normalize_object(kind, &row, &record_base);
            canonicalize(kind, &mut normalized, &id_map, &record_base);
            recorded.push(normalized);
        }
        let envelope = json!({
            "count": recorded.len(),
            "next": Value::Null,
            "previous": Value::Null,
            "results": recorded.clone(),
        });
        fs::write(
            fixture_dir.join(fixture_file(kind)),
            format!("{}\n", render(&envelope, 4, 0)),
        )
        .unwrap_or_else(|error| panic!("write {} failed: {error}", fixture_file(kind)));
        by_kind.insert(kind, recorded);
    }

    // The seed fixture: the recorded rows minus the read-form
    // derivations (`url`, `display`, and the IP `assigned_object`
    // the simulator derives at response time), in seed order.
    let mut seed = serde_json::Map::new();
    for kind in SimKind::ALL {
        let rows = by_kind.get(&kind).cloned().unwrap_or_default();
        let stripped: Vec<Value> = rows
            .into_iter()
            .map(|mut row| {
                if let Some(map) = row.as_object_mut() {
                    for field in ["url", "display", "assigned_object"] {
                        map.remove(field);
                    }
                }
                row
            })
            .collect();
        seed.insert(kind.collection().to_string(), Value::Array(stripped));
    }
    fs::write(
        fixture_dir.join("seed.json"),
        format!("{}\n", render(&Value::Object(seed), 2, 0)),
    )
    .expect("write seed.json");

    update_readme(&fixture_dir, real, netbox_version.as_deref());
}

// ---------------------------------------------------------------------------
// Provisioning and seeding
// ---------------------------------------------------------------------------

/// Prerequisite objects a real NetBox needs before the canonical set
/// can be created (the simulator needs none of them to EXIST — its
/// wire layer accepts name references — but it does require
/// `device_type` and `role` to be present on device creates, matching
/// NetBox 4.7's `DeviceSerializer`).
#[derive(Default)]
struct Provisions {
    site_id: i64,
    device_type_id: i64,
    role_id: i64,
}

/// Look `path?query` up; create `body` when nothing exists. Returns
/// the existing-or-created row. Idempotent, so repeated `--record`
/// runs against the same instance converge.
async fn ensure(
    live: &LiveNetBox,
    path: &str,
    query: &str,
    create_body: &Value,
) -> Result<Value, CaptureError> {
    let target = format!("{path}?{query}");
    let (status, page) = live.request(reqwest::Method::GET, &target, None).await?;
    if status == 200 {
        if let Some(existing) = page
            .get("results")
            .and_then(Value::as_array)
            .and_then(|results| results.first())
        {
            return Ok(existing.clone());
        }
    }
    let (status, created) = live
        .request(reqwest::Method::POST, path, Some(create_body))
        .await?;
    if status == 201 {
        Ok(created)
    } else {
        Err(CaptureError::Status {
            method: "POST".into(),
            url: path.to_string(),
            status,
            body: created.to_string(),
        })
    }
}

/// Provision the real-NetBox prerequisites: the `dc1` site, a
/// manufacturer + device type (NetBox requires one on every device),
/// the `chv-node` device role (required on device creates by NetBox
/// 4.7's `DeviceSerializer`, whose `role` field has no
/// `required=False` and whose model FK is non-nullable), the
/// `chv-team` tag, and the `chv_` custom fields as text fields on
/// the six content types.
///
/// DUPLICATION NOTE: the qualification compose stack's
/// `qualification-init` service
/// (`deploy/netbox-qualification/docker-compose.yml`) provisions the
/// same prerequisites through the ORM so a plain (no `--record`)
/// qualification run starts provisioned too. This copy stays because
/// the recorder must work against any NetBox instance, not just the
/// compose one — keep the two sides in sync.
async fn provision(live: &LiveNetBox) -> Result<Provisions, CaptureError> {
    ensure(
        live,
        "/api/extras/tags/",
        "slug=chv-team",
        &json!({ "name": "chv-team", "slug": "chv-team" }),
    )
    .await?;
    // Both tags the fixture architecture's metadata produces (team
    // label + environment), mirroring the compose init's tag list.
    ensure(
        live,
        "/api/extras/tags/",
        "slug=chv-env-production",
        &json!({ "name": "chv-env-production", "slug": "chv-env-production" }),
    )
    .await?;
    let site = ensure(
        live,
        "/api/dcim/sites/",
        "slug=dc1",
        &json!({ "name": "dc1", "slug": "dc1", "status": "active" }),
    )
    .await?;
    let manufacturer = ensure(
        live,
        "/api/dcim/manufacturers/",
        "slug=chv",
        &json!({ "name": "chv", "slug": "chv" }),
    )
    .await?;
    let manufacturer_id = manufacturer["id"].as_i64().unwrap_or_default();
    let device_type = ensure(
        live,
        "/api/dcim/device-types/",
        "slug=chv-host",
        &json!({
            "manufacturer": manufacturer_id,
            "model": "chv-host",
            "slug": "chv-host",
        }),
    )
    .await?;
    let role = ensure(
        live,
        "/api/dcim/device-roles/",
        "slug=chv-node",
        &json!({ "name": "chv-node", "slug": "chv-node" }),
    )
    .await?;
    for name in qualification_custom_fields() {
        ensure(
            live,
            "/api/extras/custom-fields/",
            &format!("name={name}"),
            &json!({
                "object_types": CONTENT_TYPES,
                "type": "text",
                "name": name,
                "label": name,
            }),
        )
        .await?;
    }
    Ok(Provisions {
        site_id: site["id"].as_i64().unwrap_or_default(),
        device_type_id: device_type["id"].as_i64().unwrap_or_default(),
        role_id: role["id"].as_i64().unwrap_or_default(),
    })
}

/// The full custom-field surface the adapter's `build_objects`
/// writes for the fixture architecture's metadata: the six ownership
/// marker fields (`ManagedMarker`) plus `chv_owner` (the
/// architecture's `metadata.owner` is set, so the adapter writes the
/// owner label on every object). Field names come from the adapter's
/// [`CustomFieldNames`] so the canonical set cannot drift from the
/// contract, and every value is a string — the adapter's write form,
/// which is also what a real NetBox `text` custom field returns. This
/// is the tripwire for the provisioning lists: if a `chv_` field is
/// missing from [`provision`], seeding here fails with a 400 against
/// a real NetBox instead of recording a silently partial surface.
fn ownership_fields(external_id: &str) -> Value {
    let names = CustomFieldNames::default();
    let mut fields = BTreeMap::new();
    fields.insert(names.external_id.clone(), json!(external_id));
    fields.insert(names.architecture_id.clone(), json!("arch_01HX"));
    fields.insert(names.managed_by.clone(), json!("chv"));
    fields.insert(names.managed_state.clone(), json!("active"));
    fields.insert(names.architecture_version.clone(), json!("3"));
    fields.insert(names.mapping_version.clone(), json!("v1"));
    fields.insert(names.owner().clone(), json!("alice"));
    Value::Object(fields.into_iter().collect())
}

/// Create the canonical one-object-per-family set, parents first so
/// every reference resolves: VLAN → prefix → device → VM → interface
/// → IP address. The `arch_01HX` external ids mirror the authored
/// seed fixture's provenance markers.
async fn seed_canonical_set(
    live: &LiveNetBox,
    real: bool,
    provisions: &Provisions,
) -> Result<(), CaptureError> {
    // Tags go out in the client's write form — name dicts, the form
    // NetBox's NestedTagSerializer accepts (attrs-dict or PK) and the
    // one the adapter's build_body now sends — instead of the legacy
    // bare-slug strings.
    let tags = json!([ { "name": "chv-team" } ]);
    let vlan = live
        .create(
            SimKind::Vlan,
            &json!({
                "vid": 42,
                "name": "backend",
                "tags": tags,
                "custom_fields": ownership_fields("arch:arch_01HX:network/backend#vlan:3"),
            }),
        )
        .await?;
    let vlan_ref = if real {
        json!(vlan["id"])
    } else {
        json!({ "vid": 42 })
    };
    live.create(
        SimKind::Prefix,
        &json!({
            "prefix": "10.42.0.0/24",
            "vlan": vlan_ref,
            "description": "backend (vlan)",
            "tags": tags,
            "custom_fields": ownership_fields("arch:arch_01HX:network/backend:3"),
        }),
    )
    .await?;
    // Device references: a real NetBox resolves PKs (and the nested
    // attrs-dicts) for `site`, `device_type`, and `role`; the
    // simulator accepts the attrs-dict forms too — and requires
    // `device_type` and `role` to be present on every device create,
    // mirroring NetBox 4.7's `DeviceSerializer`. Both dialects get
    // the write forms the adapter's client sends.
    let (site_ref, device_type_field, role_field) = if real {
        (
            json!(provisions.site_id),
            json!(provisions.device_type_id),
            json!(provisions.role_id),
        )
    } else {
        (
            json!({ "name": "dc1" }),
            json!({ "manufacturer": { "slug": CHV_NETBOX_MANUFACTURER }, "slug": CHV_NETBOX_DEVICE_TYPE }),
            json!({ "slug": CHV_NETBOX_DEVICE_ROLE }),
        )
    };
    // The device's marker set extends the shared ownership surface
    // with the three device-enrichment facts the adapter's mapping
    // writes when they exist (contract rules 2/6: cpu/memory from
    // declared resources or the live snapshot, datastore facts as a
    // sorted, comma-joined `name:kind` list). Seeding all three is
    // what makes the recorder tripwire on the `chv_memory_gb` /
    // `chv_datastores` provisioning, not just the ownership fields.
    let names = CustomFieldNames::default();
    let mut device_fields = ownership_fields("arch:arch_01HX:server/chv-node-01:3")
        .as_object()
        .cloned()
        .unwrap_or_default();
    device_fields.insert(names.cpu_cores(), json!("8"));
    device_fields.insert(names.memory_gb(), json!("16"));
    device_fields.insert(names.datastores(), json!("ds-local:local,ds-nfs:nfs"));
    let device_body = json!({
        "name": "chv-node-01",
        "status": "active",
        "site": site_ref,
        "device_type": device_type_field,
        "role": role_field,
        "tags": tags,
        "custom_fields": Value::Object(device_fields),
    });
    let device = live.create(SimKind::Device, &device_body).await?;
    let device_ref = if real {
        json!(device["id"])
    } else {
        json!({ "name": "chv-node-01" })
    };
    let vm = live
        .create(
            SimKind::VirtualMachine,
            &json!({
                "name": "vm-01",
                "status": "active",
                "cluster": Value::Null,
                "device": device_ref,
                "vcpus": 2,
                "memory": 2048,
                "tags": tags,
                "custom_fields": ownership_fields("arch:arch_01HX:instance/vm-01:3"),
            }),
        )
        .await?;
    let vm_ref = if real {
        json!(vm["id"])
    } else {
        json!({ "name": "vm-01" })
    };
    let interface = live
        .create(
            SimKind::Interface,
            &json!({
                "name": "backend",
                "virtual_machine": vm_ref,
                "description": "backend",
                "type": "virtual",
                "tags": tags,
                "custom_fields": ownership_fields("arch:arch_01HX:instance/vm-01/backend:3"),
            }),
        )
        .await?;
    live.create(
        SimKind::IpAddress,
        &json!({
            "address": "10.42.0.5/24",
            "assigned_object_type": "virtualization.vminterface",
            "assigned_object_id": interface["id"],
            "tags": tags,
            "custom_fields": ownership_fields(
                "arch:arch_01HX:instance/vm-01/backend#10.42.0.5:3"
            ),
        }),
    )
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Capture and canonicalization
// ---------------------------------------------------------------------------

/// One family's rows, asserting the capture is a single complete page
/// (a dirtied instance or a broken pagination walk must fail loudly,
/// not silently truncate the fixtures).
async fn capture_family(live: &LiveNetBox, kind: SimKind) -> Result<Vec<Value>, CaptureError> {
    let target = format!("{}?limit={}", kind.api_path(), LIST_PAGE);
    let (status, page) = live.request(reqwest::Method::GET, &target, None).await?;
    if status != 200 {
        return Err(CaptureError::Status {
            method: "GET".into(),
            url: target,
            status,
            body: page.to_string(),
        });
    }
    assert!(
        page["next"].is_null(),
        "the canonical set must fit one page ({kind})"
    );
    let results = page["results"].as_array().cloned().unwrap_or_default();
    assert_eq!(
        page["count"].as_i64(),
        Some(results.len() as i64),
        "count must match the captured page ({kind})"
    );
    assert_eq!(
        results.len(),
        1,
        "the canonical set is one object per family ({kind})"
    );
    Ok(results)
}

/// Rewrite one normalized row onto the canonical form: the renumbered
/// id, the rebuilt url, relation ids remapped through the same table,
/// and the pinned timestamps.
fn canonicalize(kind: SimKind, row: &mut Value, id_map: &BTreeMap<i64, i64>, base: &str) {
    let old_id = row["id"].as_i64().expect("row id");
    let new_id = *id_map.get(&old_id).unwrap_or_else(|| {
        panic!("{kind} row id {old_id} was not renumbered");
    });
    row["id"] = json!(new_id);
    row["url"] = json!(format!("{base}{}{new_id}/", kind.api_path()));
    match kind {
        SimKind::VirtualMachine => {
            if let Some(id) = row
                .get_mut("device")
                .and_then(|device| device.get_mut("id"))
            {
                remap(id, id_map);
            }
        }
        SimKind::Interface => {
            if let Some(id) = row
                .get_mut("virtual_machine")
                .and_then(|vm| vm.get_mut("id"))
            {
                remap(id, id_map);
            }
        }
        SimKind::Prefix => {
            if let Some(id) = row.get_mut("vlan").and_then(|vlan| vlan.get_mut("id")) {
                remap(id, id_map);
            }
        }
        SimKind::IpAddress => {
            if let Some(id) = row.get_mut("assigned_object_id") {
                remap(id, id_map);
            }
            if let Some(assignment) = row.get_mut("assigned_object") {
                if let Some(id) = assignment.get_mut("id") {
                    remap(id, id_map);
                }
                if let Some(id) = assignment
                    .get_mut("virtual_machine")
                    .and_then(|vm| vm.get_mut("id"))
                {
                    remap(id, id_map);
                }
            }
        }
        SimKind::Device | SimKind::Vlan => {}
    }
    for field in ["created", "last_updated"] {
        if row.get(field).is_some() {
            row[field] = json!(CANONICAL_TIMESTAMP);
        }
    }
}

/// Replace `value` with its renumbered id when it carries a
/// six-family id (site, tag, and cluster ids are not in the map and
/// stay as captured).
fn remap(value: &mut Value, id_map: &BTreeMap<i64, i64>) {
    if let Some(old) = value.as_i64() {
        if let Some(new) = id_map.get(&old) {
            *value = json!(new);
        }
    }
}

/// The fixture file name for a kind (the names the fidelity suite
/// `include_str!`s).
fn fixture_file(kind: SimKind) -> &'static str {
    match kind {
        SimKind::Device => "dcim-devices.json",
        SimKind::VirtualMachine => "virtualization-virtual-machines.json",
        SimKind::Interface => "virtualization-interfaces.json",
        SimKind::Prefix => "ipam-prefixes.json",
        SimKind::Vlan => "ipam-vlans.json",
        SimKind::IpAddress => "ipam-ip-addresses.json",
    }
}

// ---------------------------------------------------------------------------
// Rendering and provenance
// ---------------------------------------------------------------------------

/// Pretty-print `value` with `width`-space indentation (the list
/// fixtures use 4, `seed.json` 2 — `serde_json::to_string_pretty` is
/// 2-space only). Object keys render in the `Value`'s (sorted) order,
/// matching the simulator's own byte-stable serialization; empty
/// arrays and objects render inline.
fn render(value: &Value, width: usize, depth: usize) -> String {
    let indent = " ".repeat(width * depth);
    let inner = " ".repeat(width * (depth + 1));
    match value {
        Value::Object(map) if !map.is_empty() => {
            let body = map
                .iter()
                .map(|(key, value)| {
                    format!(
                        "{inner}{}: {}",
                        serde_json::to_string(key).expect("key serializes"),
                        render(value, width, depth + 1)
                    )
                })
                .collect::<Vec<_>>()
                .join(",\n");
            format!("{{\n{body}\n{indent}}}")
        }
        Value::Array(items) if !items.is_empty() => {
            let body = items
                .iter()
                .map(|item| format!("{inner}{}", render(item, width, depth + 1)))
                .collect::<Vec<_>>()
                .join(",\n");
            format!("[\n{body}\n{indent}]")
        }
        _ => serde_json::to_string(value).expect("scalar serializes"),
    }
}

/// Replace the fixture directory README's provenance section with the
/// recorded provenance (the "never hand-edit after a real capture"
/// rule's paper trail). Skipped when no README exists (a custom
/// `NETBOX_FIXTURE_DIR` without one).
fn update_readme(dir: &Path, real: bool, netbox_version: Option<&str>) {
    let path = dir.join("README.md");
    let Some(readme) = fs::read_to_string(&path).ok() else {
        return;
    };
    let Some(start) = readme.find("## Provenance") else {
        panic!("the fixture README must carry a ## Provenance section");
    };
    let end = readme[start..]
        .find("\n## ")
        .map(|offset| start + offset + 1)
        .unwrap_or(readme.len());
    let source = if real {
        match netbox_version {
            Some(version) => format!("a live NetBox {version}"),
            None => "a live NetBox".to_string(),
        }
    } else {
        "the chv-netbox-sim simulator binary (a recorder-mechanics run, \
         NOT a live capture)"
            .to_string()
    };
    let date = chrono::Utc::now().format("%Y-%m-%d");
    let section = format!(
        "## Provenance\n\n\
         Recorded {date} from {source} by the qualification lane's\n\
         `--record` mode (`scripts/netbox-qualify.sh --record`, ADR-024\n\
         lane 3). Instance-assigned ids were renumbered to a stable 1..N\n\
         sequence in fixture order (devices, virtual machines,\n\
         interfaces, prefixes, vlans, ip addresses), six-family relation\n\
         ids remapped through the same table, `created`/`last_updated`\n\
         pinned to `{CANONICAL_TIMESTAMP}`, and every `url` rewritten\n\
         onto the `{}` record base. Everything else is the captured\n\
         serializer output reduced to the simulator's read form by\n\
         `chv-netbox-sim`'s `capture::normalize_object`. Refresh only\n\
         through the same mode — never hand-edit.\n",
        std::env::var("NETBOX_RECORD_BASE_URL").unwrap_or_else(|_| "live instance".to_string())
    );
    fs::write(
        &path,
        format!("{}{}{}", &readme[..start], section, &readme[end..]),
    )
    .unwrap_or_else(|error| panic!("README update failed: {error}"));
}
