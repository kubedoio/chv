//! In-crate integration tests: the simulator's NetBox semantics over
//! real HTTP (plain reqwest — no adapter client involved), plus the
//! `__`-prefixed control plane and fault injection.

mod common;

use std::time::Instant;

use chv_netbox_sim::NetboxSim;
use common::{delete, get, http, patch, post, seed, set_fault, start, start_with, state, TOKEN};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------

async fn seed_devices(sim: &NetboxSim, count: usize) {
    let devices: Vec<Value> = (0..count)
        .map(|i| {
            json!({
                "name": format!("node-{i:03}"),
                "status": "active",
                "custom_fields": { "chv_architecture_id": "arch-1" },
            })
        })
        .collect();
    seed(sim, &json!({ "devices": devices })).await;
}

#[tokio::test]
async fn list_envelope_paginates_with_limit_and_offset() {
    let sim = start().await;
    seed_devices(&sim, 5).await;

    let page = get(&sim, "/api/dcim/devices/?limit=2")
        .await
        .json::<Value>()
        .await
        .expect("page 1 is JSON");
    assert_eq!(page["count"], json!(5));
    assert_eq!(page["results"].as_array().expect("results").len(), 2);
    // First page: no previous link, next points at offset 2.
    assert_eq!(page["previous"], Value::Null);
    let next = page["next"].as_str().expect("next link").to_string();
    assert_eq!(
        next,
        format!("{}/api/dcim/devices/?limit=2&offset=2", sim.base_url())
    );

    // Following the (absolute, same-origin) next link.
    let page = http()
        .get(&next)
        .header("Authorization", format!("Token {TOKEN}"))
        .send()
        .await
        .expect("page 2 fetches")
        .json::<Value>()
        .await
        .expect("page 2 is JSON");
    assert_eq!(page["count"], json!(5));
    assert_eq!(page["results"].as_array().expect("results").len(), 2);
    // Middle page: previous targets offset 0 (offset param omitted,
    // like DRF's LimitOffsetPagination).
    assert_eq!(
        page["previous"].as_str().expect("previous link"),
        format!("{}/api/dcim/devices/?limit=2", sim.base_url())
    );

    // Last page.
    let page = get(&sim, "/api/dcim/devices/?limit=2&offset=4")
        .await
        .json::<Value>()
        .await
        .expect("page 3 is JSON");
    assert_eq!(page["results"].as_array().expect("results").len(), 1);
    assert_eq!(page["next"], Value::Null);
    assert_eq!(
        page["previous"].as_str().expect("previous link"),
        format!("{}/api/dcim/devices/?limit=2&offset=2", sim.base_url())
    );
}

#[tokio::test]
async fn offset_beyond_count_yields_empty_results_and_null_next() {
    let sim = start().await;
    seed_devices(&sim, 3).await;

    let page = get(&sim, "/api/dcim/devices/?limit=2&offset=10")
        .await
        .json::<Value>()
        .await
        .expect("page is JSON");
    assert_eq!(page["count"], json!(3));
    assert_eq!(page["results"], json!([]));
    assert_eq!(page["next"], Value::Null);
    // previous still walks back one page.
    assert_eq!(
        page["previous"].as_str().expect("previous link"),
        format!("{}/api/dcim/devices/?limit=2&offset=8", sim.base_url())
    );
}

#[tokio::test]
async fn limit_zero_returns_the_server_max_page() {
    // max_page_size forced down so limit=0 truncates.
    let sim = start_with(chv_netbox_sim::NetboxSimConfig::new(TOKEN).with_page_sizes(3, 4)).await;
    seed_devices(&sim, 5).await;

    let page = get(&sim, "/api/dcim/devices/?limit=0")
        .await
        .json::<Value>()
        .await
        .expect("page is JSON");
    assert_eq!(page["count"], json!(5));
    assert_eq!(page["results"].as_array().expect("results").len(), 4);
    assert_eq!(
        page["next"].as_str().expect("next link"),
        format!("{}/api/dcim/devices/?limit=4&offset=4", sim.base_url())
    );
}

#[tokio::test]
async fn oversized_limit_is_clamped_to_the_server_max() {
    let sim = start_with(chv_netbox_sim::NetboxSimConfig::new(TOKEN).with_page_sizes(3, 4)).await;
    seed_devices(&sim, 5).await;

    let page = get(&sim, "/api/dcim/devices/?limit=1000")
        .await
        .json::<Value>()
        .await
        .expect("page is JSON");
    assert_eq!(page["results"].as_array().expect("results").len(), 4);
}

#[tokio::test]
async fn default_page_size_applies_without_a_limit_parameter() {
    let sim = start_with(chv_netbox_sim::NetboxSimConfig::new(TOKEN).with_page_sizes(2, 100)).await;
    seed_devices(&sim, 5).await;

    let page = get(&sim, "/api/dcim/devices/")
        .await
        .json::<Value>()
        .await
        .expect("page is JSON");
    assert_eq!(page["results"].as_array().expect("results").len(), 2);
    assert_eq!(
        page["next"].as_str().expect("next link"),
        format!("{}/api/dcim/devices/?limit=2&offset=2", sim.base_url())
    );
}

// ---------------------------------------------------------------------------
// Filtering
// ---------------------------------------------------------------------------

async fn seed_filter_matrix(sim: &NetboxSim) {
    seed(
        sim,
        &json!({
            "devices": [
                { "id": 7, "name": "chv-node-01", "status": "active",
                  "custom_fields": { "chv_external_id": "ext-node-01", "chv_architecture_id": "arch-1" } },
                { "id": 8, "name": "chv-node-02", "status": "active",
                  "custom_fields": { "chv_external_id": "ext-node-02", "chv_architecture_id": "arch-2" } }
            ],
            "virtual_machines": [
                { "id": 9, "name": "vm-01", "status": "active" },
                { "id": 10, "name": "vm-02", "status": "active" }
            ],
            "interfaces": [
                { "id": 11, "name": "backend", "virtual_machine": { "name": "vm-01" }, "type": "virtual" },
                { "id": 12, "name": "backend", "virtual_machine": { "name": "vm-02" }, "type": "virtual" },
                { "id": 13, "name": "frontend", "virtual_machine": { "name": "vm-01" }, "type": "virtual" }
            ],
            "prefixes": [
                { "id": 3, "prefix": "10.42.0.0/24" },
                { "id": 4, "prefix": "10.42.1.0/24" }
            ],
            "vlans": [
                { "id": 42, "vid": 42, "name": "backend" },
                { "id": 43, "vid": 43, "name": "frontend" }
            ],
            "ip_addresses": [
                { "id": 21, "address": "10.42.0.5/24" },
                { "id": 22, "address": "10.42.0.6/32" },
                { "id": 23, "address": "10.42.0.5/32" }
            ]
        }),
    )
    .await;
}

#[tokio::test]
async fn natural_key_filter_matrix_per_kind() {
    let sim = start().await;
    seed_filter_matrix(&sim).await;

    async fn ids(sim: &NetboxSim, path: &str) -> Vec<i64> {
        let page = get(sim, path).await.json::<Value>().await.expect("JSON");
        page["results"]
            .as_array()
            .expect("results")
            .iter()
            .map(|row| row["id"].as_i64().expect("id"))
            .collect()
    }

    // Devices and VMs: name.
    assert_eq!(ids(&sim, "/api/dcim/devices/?name=chv-node-01").await, [7]);
    assert_eq!(
        ids(&sim, "/api/virtualization/virtual-machines/?name=vm-02").await,
        [10]
    );
    // Interfaces: name + parent VM (both parts of the natural key).
    assert_eq!(
        ids(&sim, "/api/virtualization/interfaces/?name=backend").await,
        [11, 12]
    );
    assert_eq!(
        ids(
            &sim,
            "/api/virtualization/interfaces/?name=backend&virtual_machine=vm-01"
        )
        .await,
        [11]
    );
    // Prefixes: prefix.
    assert_eq!(
        ids(&sim, "/api/ipam/prefixes/?prefix=10.42.0.0/24").await,
        [3]
    );
    // VLANs: vid.
    assert_eq!(ids(&sim, "/api/ipam/vlans/?vid=43").await, [43]);
    // IP addresses: mask-independent address match.
    assert_eq!(
        ids(&sim, "/api/ipam/ip-addresses/?address=10.42.0.5").await,
        [21, 23]
    );
    assert_eq!(
        ids(&sim, "/api/ipam/ip-addresses/?address=10.42.0.6").await,
        [22]
    );
    // No match → empty results, never an error.
    let empty: Vec<i64> = ids(&sim, "/api/dcim/devices/?name=missing").await;
    assert!(empty.is_empty());
}

#[tokio::test]
async fn custom_field_filters() {
    let sim = start().await;
    seed_filter_matrix(&sim).await;

    let page = get(&sim, "/api/dcim/devices/?cf_chv_architecture_id=arch-1")
        .await
        .json::<Value>()
        .await
        .expect("JSON");
    let results = page["results"].as_array().expect("results");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["id"], json!(7));

    // The idempotency-match filter the client sends
    // (cf_chv_external_id) narrows to exactly one row.
    let page = get(&sim, "/api/dcim/devices/?cf_chv_external_id=ext-node-02")
        .await
        .json::<Value>()
        .await
        .expect("JSON");
    assert_eq!(page["results"].as_array().expect("results").len(), 1);

    // Filters compose with pagination parameters.
    let page = get(
        &sim,
        "/api/dcim/devices/?cf_chv_architecture_id=arch-1&limit=50",
    )
    .await
    .json::<Value>()
    .await
    .expect("JSON");
    assert_eq!(page["results"].as_array().expect("results").len(), 1);
}

#[tokio::test]
async fn unknown_query_parameters_are_ignored() {
    let sim = start().await;
    seed_devices(&sim, 2).await;

    let page = get(&sim, "/api/dcim/devices/?not_a_filter=1")
        .await
        .json::<Value>()
        .await
        .expect("JSON");
    assert_eq!(page["count"], json!(2));
}

// ---------------------------------------------------------------------------
// Write semantics
// ---------------------------------------------------------------------------

#[tokio::test]
async fn post_assigns_id_url_and_timestamps() {
    let sim = start().await;

    let response = post(
        &sim,
        "/api/dcim/devices/",
        common::device_body("chv-node-01"),
    )
    .await;
    assert_eq!(response.status(), 201);
    let object = response.json::<Value>().await.expect("object is JSON");
    let id = object["id"].as_i64().expect("id assigned");
    assert_eq!(
        object["url"].as_str().expect("url assigned"),
        format!("{}/api/dcim/devices/{id}/", sim.base_url())
    );
    assert!(object["created"].as_str().is_some());
    assert!(object["last_updated"].as_str().is_some());
    // Status round-trips through the choice-object read form.
    assert_eq!(object["status"]["value"], json!("active"));
    assert_eq!(object["status"]["label"], json!("Active"));
    // The list shows the created object.
    let page = get(&sim, "/api/dcim/devices/")
        .await
        .json::<Value>()
        .await
        .expect("JSON");
    assert_eq!(page["count"], json!(1));
}

#[tokio::test]
async fn duplicate_natural_key_is_a_400_with_netbox_error_shape() {
    let sim = start().await;

    let response = post(&sim, "/api/dcim/devices/", common::device_body("dup")).await;
    assert_eq!(response.status(), 201);
    let response = post(&sim, "/api/dcim/devices/", common::device_body("dup")).await;
    assert_eq!(response.status(), 400);
    assert_eq!(
        response.json::<Value>().await.expect("error body"),
        json!({ "name": ["This field must be unique."] })
    );

    // The vid-keyed kinds report under their own field.
    let vlan = json!({ "vid": 42, "name": "backend" });
    assert_eq!(
        post(&sim, "/api/ipam/vlans/", vlan.clone()).await.status(),
        201
    );
    let response = post(&sim, "/api/ipam/vlans/", vlan).await;
    assert_eq!(response.status(), 400);
    assert_eq!(
        response.json::<Value>().await.expect("error body"),
        json!({ "vid": ["This field must be unique."] })
    );
}

#[tokio::test]
async fn missing_required_fields_are_400s() {
    let sim = start().await;

    let response = post(&sim, "/api/dcim/devices/", json!({ "status": "active" })).await;
    assert_eq!(response.status(), 400);
    assert_eq!(
        response.json::<Value>().await.expect("error body"),
        json!({ "name": ["This field is required."] })
    );

    // Not JSON at all.
    let response = http()
        .post(format!("{}/api/dcim/devices/", sim.base_url()))
        .header("Authorization", format!("Token {TOKEN}"))
        .body("not json")
        .send()
        .await
        .expect("POST succeeds");
    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn patch_updates_and_bumps_last_updated() {
    let sim = start().await;

    let object = post(
        &sim,
        "/api/dcim/devices/",
        common::device_body("chv-node-01"),
    )
    .await
    .json::<Value>()
    .await
    .expect("object");
    let id = object["id"].as_i64().expect("id");

    // Partial PATCH: custom_fields merge per key, status accepts the
    // string write form.
    let response = patch(
        &sim,
        &format!("/api/dcim/devices/{id}/"),
        json!({
            "status": "decommissioning",
            "custom_fields": { "chv_managed_state": "stale" },
        }),
    )
    .await;
    assert_eq!(response.status(), 200);
    let updated = response.json::<Value>().await.expect("object");
    assert_eq!(updated["status"]["value"], json!("decommissioning"));
    assert_eq!(updated["custom_fields"]["chv_managed_by"], json!("chv"));
    assert_eq!(
        updated["custom_fields"]["chv_managed_state"],
        json!("stale")
    );
    assert_ne!(
        updated["last_updated"], object["last_updated"],
        "last_updated must move on PATCH"
    );
    assert_eq!(updated["created"], object["created"]);
}

#[tokio::test]
async fn delete_returns_204_and_removes_the_object() {
    let sim = start().await;

    let object = post(&sim, "/api/dcim/devices/", common::device_body("gone"))
        .await
        .json::<Value>()
        .await
        .expect("object");
    let id = object["id"].as_i64().expect("id");

    let response = delete(&sim, &format!("/api/dcim/devices/{id}/")).await;
    assert_eq!(response.status(), 204);
    assert_eq!(response.text().await.expect("empty body"), "");

    let page = get(&sim, "/api/dcim/devices/")
        .await
        .json::<Value>()
        .await
        .expect("JSON");
    assert_eq!(page["count"], json!(0));

    // Deleting again: 404 with NetBox's body.
    let response = delete(&sim, &format!("/api/dcim/devices/{id}/")).await;
    assert_eq!(response.status(), 404);
    assert_eq!(
        response.json::<Value>().await.expect("error body"),
        json!({ "detail": "Not found." })
    );
}

// ---------------------------------------------------------------------------
// 404s outside the surface
// ---------------------------------------------------------------------------

#[tokio::test]
async fn unknown_paths_and_ids_return_netbox_404_bodies() {
    let sim = start().await;

    // Unknown endpoint families, like a real NetBox.
    for path in [
        "/api/users/users/",
        "/api/dcim/sites/",
        "/api/status/",
        "/nope",
    ] {
        let response = get(&sim, path).await;
        assert_eq!(response.status(), 404, "{path}");
        assert_eq!(
            response.json::<Value>().await.expect("error body"),
            json!({ "detail": "Not found." }),
            "{path}"
        );
    }

    // Unknown ids on known families.
    let response = patch(
        &sim,
        "/api/dcim/devices/99/",
        json!({ "status": "offline" }),
    )
    .await;
    assert_eq!(response.status(), 404);
    let response = delete(&sim, "/api/ipam/vlans/99/").await;
    assert_eq!(response.status(), 404);

    // Non-numeric ids are 404s, like DRF's pk resolution.
    let response = patch(
        &sim,
        "/api/dcim/devices/abc/",
        json!({ "status": "offline" }),
    )
    .await;
    assert_eq!(response.status(), 404);
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

#[tokio::test]
async fn auth_rejection_bodies() {
    let sim = start().await;
    seed_devices(&sim, 1).await;

    // No header.
    let response = http()
        .get(format!("{}/api/dcim/devices/", sim.base_url()))
        .send()
        .await
        .expect("GET");
    assert_eq!(response.status(), 401);
    assert_eq!(
        response
            .headers()
            .get("www-authenticate")
            .and_then(|value| value.to_str().ok()),
        Some("Token")
    );
    assert_eq!(
        response.json::<Value>().await.expect("error body"),
        json!({ "detail": "Authentication credentials were not provided." })
    );

    // Wrong token.
    let response = http()
        .get(format!("{}/api/dcim/devices/", sim.base_url()))
        .header("Authorization", "Token wrong-token")
        .send()
        .await
        .expect("GET");
    assert_eq!(response.status(), 401);
    assert_eq!(
        response.json::<Value>().await.expect("error body"),
        json!({ "detail": "Invalid token" })
    );

    // Wrong scheme.
    let response = http()
        .get(format!("{}/api/dcim/devices/", sim.base_url()))
        .header("Authorization", format!("Bearer {TOKEN}"))
        .send()
        .await
        .expect("GET");
    assert_eq!(response.status(), 401);

    // Writes are authenticated too.
    let response = http()
        .post(format!("{}/api/dcim/devices/", sim.base_url()))
        .json(&common::device_body("x"))
        .send()
        .await
        .expect("POST");
    assert_eq!(response.status(), 401);

    // Multiple configured tokens are all accepted.
    let sim = start_with(chv_netbox_sim::NetboxSimConfig::with_tokens([
        "token-a", "token-b",
    ]))
    .await;
    for token in ["token-a", "token-b"] {
        let response = http()
            .get(format!("{}/api/dcim/devices/", sim.base_url()))
            .header("Authorization", format!("Token {token}"))
            .send()
            .await
            .expect("GET");
        assert_eq!(response.status(), 200, "token {token}");
    }
}

// ---------------------------------------------------------------------------
// Faults
// ---------------------------------------------------------------------------

#[tokio::test]
async fn fault_auth_failure_forces_401_even_for_valid_tokens() {
    let sim = start().await;
    set_fault(&sim, json!({ "auth_failure": true })).await;

    let response = get(&sim, "/api/dcim/devices/").await;
    assert_eq!(response.status(), 401);
    assert_eq!(
        response.json::<Value>().await.expect("error body"),
        json!({ "detail": "Invalid token" })
    );
    // The control plane stays reachable (faults never apply to it).
    let dump = state(&sim).await;
    assert!(dump["faults"]["global"]["auth_failure"]
        .as_bool()
        .unwrap_or(false));
}

#[tokio::test]
async fn fault_rate_limit_returns_429_with_throttle_body() {
    let sim = start().await;
    set_fault(&sim, json!({ "rate_limit": true })).await;

    let response = get(&sim, "/api/ipam/vlans/").await;
    assert_eq!(response.status(), 429);
    assert!(response.headers().contains_key("retry-after"));
    assert_eq!(
        response.json::<Value>().await.expect("error body"),
        json!({ "detail": "Request was throttled." })
    );
}

#[tokio::test]
async fn fault_server_error_returns_the_configured_5xx() {
    let sim = start().await;
    set_fault(&sim, json!({ "server_error": 503 })).await;

    let response = get(&sim, "/api/ipam/vlans/").await;
    assert_eq!(response.status(), 503);

    // Only 5xx statuses are accepted.
    let response = common::control_post(&sim, "/__faults", json!({ "server_error": 418 })).await;
    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn fault_latency_delays_the_response() {
    let sim = start().await;
    set_fault(&sim, json!({ "latency_ms": 250 })).await;

    let started = Instant::now();
    let response = get(&sim, "/api/ipam/vlans/").await;
    assert_eq!(response.status(), 200);
    assert!(
        started.elapsed() >= std::time::Duration::from_millis(200),
        "latency fault must delay the response (took {:?})",
        started.elapsed()
    );
}

#[tokio::test]
async fn fault_connection_drop_breaks_the_response() {
    let sim = start().await;
    set_fault(&sim, json!({ "connection_drop": true })).await;

    // The response begins but is aborted mid-flight: either the
    // request itself fails, or the body read does — never a clean
    // well-formed page.
    if let Ok(response) = http()
        .get(format!("{}/api/ipam/vlans/", sim.base_url()))
        .header("Authorization", format!("Token {TOKEN}"))
        .send()
        .await
    {
        let body = response.text().await;
        assert!(
            body.is_err(),
            "body read must fail when the connection is dropped, got {:?}",
            body.ok()
        );
    }
}

#[tokio::test]
async fn faults_can_be_scoped_per_kind() {
    let sim = start().await;
    seed_devices(&sim, 1).await;
    set_fault(&sim, json!({ "kind": "vlan", "rate_limit": true })).await;

    let response = get(&sim, "/api/ipam/vlans/").await;
    assert_eq!(response.status(), 429);
    let response = get(&sim, "/api/dcim/devices/").await;
    assert_eq!(response.status(), 200);

    // Setting a global fault does not override the per-kind scope…
    set_fault(&sim, json!({ "auth_failure": true })).await;
    assert_eq!(get(&sim, "/api/ipam/vlans/").await.status(), 429);
    // …but applies everywhere else.
    assert_eq!(get(&sim, "/api/dcim/devices/").await.status(), 401);
}

// ---------------------------------------------------------------------------
// Control plane: seed / state / reset
// ---------------------------------------------------------------------------

#[tokio::test]
async fn seed_state_reset_round_trip() {
    let sim = start().await;

    let response = common::control_post(
        &sim,
        "/__seed",
        json!({
            "devices": [
                { "id": 7, "name": "chv-node-01", "status": "active",
                  "custom_fields": { "chv_external_id": "ext-1" },
                  "created": "2026-10-09T12:00:00.000000Z" }
            ],
            "vlans": [
                { "vid": 42, "name": "backend" },
                { "vid": 42, "name": "backend-duplicate" }
            ]
        }),
    )
    .await;
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.json::<Value>().await.expect("counts"),
        json!({
            "seeded": {
                "vlans": 2, "prefixes": 0, "ip_addresses": 0,
                "interfaces": 0, "virtual_machines": 0, "devices": 1
            }
        })
    );

    // Seeded duplicate natural keys are visible (ambiguous remote
    // state is a scenario the sim must model).
    let dump = state(&sim).await;
    assert_eq!(dump["objects"]["vlans"].as_array().expect("vlans").len(), 2);
    assert_eq!(dump["objects"]["devices"][0]["id"], json!(7));
    assert_eq!(
        dump["objects"]["devices"][0]["created"],
        json!("2026-10-09T12:00:00.000000Z")
    );
    assert_eq!(dump["next_id"], json!(8));

    // The seeded state is served through the NetBox surface too.
    let page = get(&sim, "/api/dcim/devices/?cf_chv_external_id=ext-1")
        .await
        .json::<Value>()
        .await
        .expect("JSON");
    assert_eq!(page["count"], json!(1));

    // Reset clears objects AND faults.
    set_fault(&sim, json!({ "rate_limit": true })).await;
    let response = common::control_post(&sim, "/__reset", json!({})).await;
    assert_eq!(response.status(), 200);
    let dump = state(&sim).await;
    assert_eq!(dump["objects"]["devices"], json!([]));
    assert_eq!(dump["objects"]["vlans"], json!([]));
    assert_eq!(dump["faults"]["global"], Value::Null);
    assert_eq!(dump["next_id"], json!(1));
    assert_eq!(get(&sim, "/api/ipam/vlans/").await.status(), 200);
}
