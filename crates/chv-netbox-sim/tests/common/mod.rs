#![allow(dead_code)] // shared scaffolding; not every suite uses every helper

//! Shared scaffolding for the simulator's integration test suites.

use chv_netbox_sim::{NetboxSim, NetboxSimConfig};
use serde_json::Value;

/// The token every helper (and the compatibility suite's clients)
/// presents.
pub const TOKEN: &str = "sim-test-token";

pub async fn start() -> NetboxSim {
    NetboxSim::start(NetboxSimConfig::new(TOKEN))
        .await
        .expect("simulator starts")
}

pub async fn start_with(config: NetboxSimConfig) -> NetboxSim {
    NetboxSim::start(config).await.expect("simulator starts")
}

pub fn http() -> reqwest::Client {
    reqwest::Client::new()
}

/// An authenticated GET against the NetBox surface.
pub async fn get(sim: &NetboxSim, path_and_query: &str) -> reqwest::Response {
    http()
        .get(format!("{}{}", sim.base_url(), path_and_query))
        .header("Authorization", format!("Token {TOKEN}"))
        .send()
        .await
        .expect("GET succeeds")
}

/// An authenticated POST with a JSON body.
pub async fn post(sim: &NetboxSim, path: &str, body: Value) -> reqwest::Response {
    http()
        .post(format!("{}{}", sim.base_url(), path))
        .header("Authorization", format!("Token {TOKEN}"))
        .json(&body)
        .send()
        .await
        .expect("POST succeeds")
}

/// An authenticated PATCH with a JSON body.
pub async fn patch(sim: &NetboxSim, path: &str, body: Value) -> reqwest::Response {
    http()
        .patch(format!("{}{}", sim.base_url(), path))
        .header("Authorization", format!("Token {TOKEN}"))
        .json(&body)
        .send()
        .await
        .expect("PATCH succeeds")
}

/// An authenticated DELETE.
pub async fn delete(sim: &NetboxSim, path: &str) -> reqwest::Response {
    http()
        .delete(format!("{}{}", sim.base_url(), path))
        .header("Authorization", format!("Token {TOKEN}"))
        .send()
        .await
        .expect("DELETE succeeds")
}

/// A control-plane POST (no auth — the `__` plane is unauthenticated
/// by design).
pub async fn control_post(sim: &NetboxSim, path: &str, body: Value) -> reqwest::Response {
    http()
        .post(format!("{}{}", sim.base_url(), path))
        .json(&body)
        .send()
        .await
        .expect("control-plane POST succeeds")
}

/// Seed via the control plane, asserting it succeeded.
pub async fn seed(sim: &NetboxSim, payload: &Value) {
    let response = control_post(sim, "/__seed", payload.clone()).await;
    assert_eq!(
        response.status(),
        200,
        "seed must succeed: {}",
        response.text().await.expect("body")
    );
}

/// Set a fault configuration (global unless the body carries a kind).
pub async fn set_fault(sim: &NetboxSim, body: Value) {
    let response = control_post(sim, "/__faults", body).await;
    assert_eq!(
        response.status(),
        200,
        "set_fault must succeed: {}",
        response.text().await.expect("body")
    );
}

/// The full state dump.
pub async fn state(sim: &NetboxSim) -> Value {
    let response = http()
        .get(format!("{}/__state", sim.base_url()))
        .send()
        .await
        .expect("state GET succeeds");
    assert_eq!(response.status(), 200);
    response.json().await.expect("state is JSON")
}

/// A minimal valid device create body.
pub fn device_body(name: &str) -> Value {
    serde_json::json!({
        "name": name,
        "status": "active",
        "site": { "name": "dc1" },
        "tags": ["chv-team"],
        "custom_fields": { "chv_architecture_id": "arch-1", "chv_managed_by": "chv" },
    })
}
