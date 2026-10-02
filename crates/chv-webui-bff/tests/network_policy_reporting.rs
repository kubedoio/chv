//! Integration tests for `POST /v1/networks/update` policy reporting
//! (issue kubedoio/chv#355).
//!
//! The firewall_rules update is persisted to the CP DB but is NOT
//! dispatched to a live node: the policy travels with the VM spec and the
//! Core executor applies it at attach time (default-deny + the operator's
//! rules). The update response must SAY so (`policy_application` field)
//! instead of implying the rules are live on the node — the "dead config"
//! half of the defect was precisely that a 200 implied applied state.
//!
//! Pinned here:
//! - an update carrying `firewall_rules` reports attach-time application;
//! - an update without policy fields carries no such note (nothing policy
//!   -related was touched).

use std::sync::Arc;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chv_common::SystemClock;
use chv_controlplane_store::{
    AlertRepository, ApplyRunRepository, BackupRepository, DesiredStateRepository,
    DriftReportRepository, EventRepository, ImageRepository, NetworkRepository, NodeRepository,
    ObservedStateRepository, OperationRepository, TopologyRepository,
};
use chv_webui_bff::mutations::MutationService;
use chv_webui_bff::{AppState, BffError};
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

struct NoopMutations;

#[async_trait]
impl MutationService for NoopMutations {
    async fn mutate_vm(
        &self,
        _vm_id: String,
        _action: String,
        _force: bool,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse, BffError> {
        unreachable!()
    }
    async fn migrate_vm(
        &self,
        _vm_id: String,
        _target_node_id: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse, BffError> {
        unreachable!()
    }
    async fn snapshot_vm(
        &self,
        _vm_id: String,
        _destination: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse, BffError> {
        unreachable!()
    }
    async fn restore_snapshot(
        &self,
        _vm_id: String,
        _source: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse, BffError> {
        unreachable!()
    }
    async fn mutate_node(
        &self,
        _node_id: String,
        _action: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateNodeResponse, BffError> {
        unreachable!()
    }
    async fn mutate_volume(
        &self,
        _volume_id: String,
        _action: String,
        _force: bool,
        _resize_bytes: Option<u64>,
        _vm_id: Option<String>,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
        unreachable!()
    }
    async fn snapshot_volume(
        &self,
        _volume_id: String,
        _snapshot_name: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
        unreachable!()
    }
    async fn restore_volume_snapshot(
        &self,
        _vm_id: String,
        _snapshot_name: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
        unreachable!()
    }
    async fn delete_volume_snapshot(
        &self,
        _volume_id: String,
        _snapshot_name: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
        unreachable!()
    }
    async fn clone_volume(
        &self,
        _source_volume_id: String,
        _target_volume_id: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
        unreachable!()
    }
    async fn mutate_network(
        &self,
        _network_id: String,
        _action: String,
        _force: bool,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateNetworkResponse, BffError> {
        unreachable!()
    }
}

async fn build_state() -> AppState {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");
    chv_controlplane_store::run_migrations(&pool, None)
        .await
        .expect("run migrations");

    AppState {
        pool: pool.clone(),
        node_repo: NodeRepository::new(pool.clone()),
        operation_repo: OperationRepository::new(pool.clone()),
        event_repo: EventRepository::new(pool.clone()),
        alert_repo: AlertRepository::new(pool.clone()),
        desired_state_repo: DesiredStateRepository::new(pool.clone()),
        observed_state_repo: ObservedStateRepository::new(pool.clone()),
        backup_repo: BackupRepository::new(pool.clone()),
        topology_repo: TopologyRepository::new(pool.clone()),
        network_repo: NetworkRepository::new(pool.clone()),
        image_repo: ImageRepository::new(pool.clone()),
        apply_runs: Arc::new(ApplyRunRepository::new(pool.clone())),
        drift_reports: Arc::new(DriftReportRepository::new(pool.clone())),
        mutations: Arc::new(NoopMutations),
        jwt_secret: "test-secret".to_string(),
        agent_runtime_dir: std::path::PathBuf::from("/var/lib/chv/agent"),
        cache: chv_webui_bff::BffCache::new(5),
        clock: Arc::new(SystemClock),
    }
}

/// Seed a user with the given role ('operator'/'admin') and return a
/// usable JWT bearer token.
async fn seed_jwt_as(state: &AppState, role: &str) -> String {
    let user_id = format!("u-{role}");
    sqlx::query(
        "INSERT INTO users (user_id, username, password_hash, role, must_change_password) \
         VALUES (?, ?, 'x', ?, 0)",
    )
    .bind(&user_id)
    .bind(role)
    .bind(role)
    .execute(&state.pool)
    .await
    .expect("seed user");

    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600;
    let claims = chv_webui_bff::auth::Claims {
        sub: user_id,
        username: role.to_string(),
        role: role.to_string(),
        exp,
        must_change_password: false,
    };
    let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
    jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    )
    .expect("encode test token")
}

async fn seed_jwt(state: &AppState) -> String {
    seed_jwt_as(state, "operator").await
}

/// Seed one enrolled node so create_vm has a placement target.
async fn seed_node(state: &AppState) {
    sqlx::query(
        "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('n-1', 'h-1', 'Node 1')",
    )
    .execute(&state.pool)
    .await
    .expect("seed node");
}

async fn post_with_token(
    state: AppState,
    path: &str,
    token: &str,
    body: &str,
) -> (StatusCode, serde_json::Value) {
    let app = chv_webui_bff::bff_router(state.clone()).with_state(state);
    let req = Request::builder()
        .method("POST")
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    };
    (status, body)
}

/// Create an operator network; return its generated network_id.
async fn create_network(state: &AppState, token: &str, name: &str, cidr: &str) -> String {
    let (status, body) = post_with_token(
        state.clone(),
        "/v1/networks/create",
        token,
        &format!(r#"{{"name":"{name}","cidr":"{cidr}"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "network create body: {body}");
    body["network_id"]
        .as_str()
        .expect("network_id in response")
        .to_string()
}

#[tokio::test]
async fn update_network_reports_attach_time_policy_application() {
    let state = build_state().await;
    let token = seed_jwt(&state).await;
    seed_node(&state).await;
    let net_id = create_network(&state, &token, "tenant-a", "10.99.0.0/24").await;

    let (status, body) = post_with_token(
        state.clone(),
        "/v1/networks/update",
        &token,
        &format!(
            r#"{{"network_id":"{net_id}","firewall_rules":[{{"direction":"inbound","action":"accept","protocol":"icmp","source_cidr":"10.99.0.0/24"}}]}}"#
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "update body: {body}");
    let note = body["policy_application"].as_str().unwrap_or_else(|| {
        panic!(
            "policy_application note must be present when firewall_rules ride the update: {body}"
        )
    });
    assert!(
        note.contains("pending"),
        "the note must state the policy is not yet live: {note}"
    );

    // The snapshot is persisted (the attach-time mechanism reads it).
    let stored: Option<String> = sqlx::query_scalar(
        "SELECT firewall_rules_json FROM network_desired_state WHERE network_id = ?",
    )
    .bind(&net_id)
    .fetch_one(&state.pool)
    .await
    .expect("query stored policy");
    assert!(
        stored.unwrap_or_default().contains("icmp"),
        "the rules must be persisted for the attach-time path to pick up"
    );
}

#[tokio::test]
async fn update_network_clearing_rules_reports_the_stale_policy_residual() {
    // The honest half of the clear story (#355): `[]` never dispatches
    // (an empty ruleset would engage default-deny with no allows), and
    // nwd re-asserts the last recorded non-empty policy on every new
    // attach — so a previously-applied policy STAYS in force. The
    // response must say that instead of implying the clear took effect.
    let state = build_state().await;
    let token = seed_jwt(&state).await;
    seed_node(&state).await;
    let net_id = create_network(&state, &token, "tenant-a", "10.99.0.0/24").await;

    // First apply a real ruleset...
    let (status, body) = post_with_token(
        state.clone(),
        "/v1/networks/update",
        &token,
        &format!(
            r#"{{"network_id":"{net_id}","firewall_rules":[{{"direction":"inbound","action":"accept","protocol":"icmp"}}]}}"#
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "apply body: {body}");
    assert!(
        body["policy_application"]
            .as_str()
            .unwrap()
            .contains("pending"),
        "non-empty rules report pending application: {body}"
    );

    // ...then clear it.
    let (status, body) = post_with_token(
        state.clone(),
        "/v1/networks/update",
        &token,
        &format!(r#"{{"network_id":"{net_id}","firewall_rules":[]}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "clear body: {body}");
    let note = body["policy_application"]
        .as_str()
        .unwrap_or_else(|| panic!("clear must carry its own note: {body}"));
    assert!(
        note.contains("cleared"),
        "the note must identify the update as a clear: {note}"
    );
    assert!(
        note.contains("stays in force"),
        "the note must state the stale-policy residual: {note}"
    );
}

#[tokio::test]
async fn update_network_rejects_non_array_firewall_rules() {
    // A non-array firewall_rules (JSON null, a scalar, an object) would
    // be stored verbatim and dispatched verbatim to nwd at attach time,
    // where it fails rule parsing and bricks every subsequent VM create
    // on the network. Reject at the API boundary instead.
    let state = build_state().await;
    let token = seed_jwt(&state).await;
    seed_node(&state).await;
    let net_id = create_network(&state, &token, "tenant-a", "10.99.0.0/24").await;

    for bad in ["null", "\"allow-all\"", "{\"rules\":[]}"] {
        let (status, body) = post_with_token(
            state.clone(),
            "/v1/networks/update",
            &token,
            &format!(r#"{{"network_id":"{net_id}","firewall_rules":{bad}}}"#),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "non-array firewall_rules ({bad}) must be rejected: {body}"
        );
        assert!(
            body["message"]
                .as_str()
                .unwrap_or_default()
                .contains("firewall_rules must be an array"),
            "the rejection must name the field: {body}"
        );
    }

    // Nothing was stored by the rejected updates.
    let stored: Option<String> = sqlx::query_scalar(
        "SELECT firewall_rules_json FROM network_desired_state WHERE network_id = ?",
    )
    .bind(&net_id)
    .fetch_one(&state.pool)
    .await
    .expect("query stored policy");
    assert!(
        stored.as_deref().unwrap_or("").is_empty(),
        "rejected payloads must not persist: {stored:?}"
    );
}

#[tokio::test]
async fn update_network_without_policy_fields_carries_no_note() {
    let state = build_state().await;
    let token = seed_jwt(&state).await;
    seed_node(&state).await;
    let net_id = create_network(&state, &token, "tenant-a", "10.99.0.0/24").await;

    let (status, body) = post_with_token(
        state.clone(),
        "/v1/networks/update",
        &token,
        &format!(r#"{{"network_id":"{net_id}","name":"renamed"}}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "update body: {body}");
    assert!(
        body.get("policy_application").is_none(),
        "no policy note when no policy field was updated: {body}"
    );
    assert_eq!(
        body["detail"]["name"].as_str(),
        Some("renamed"),
        "rename applied: {body}"
    );
}
