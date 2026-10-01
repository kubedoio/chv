//! Integration tests for the image-import → vm-create resolution chain
//! (`POST /v1/images/import`, `POST /v1/vms/create`).
//!
//! Issue kubedoio/chv#339: `chvctl image import --url` sent a `"url"`
//! payload key while the BFF import handler read `"source_url"` — the URL
//! was silently dropped, the create-time lookup matched only the generated
//! `image_id` UUID (never the name the operator chose), and `file://`
//! source URIs were rejected as remote. Together this made
//! `vm create --image <imported-name>` unusable: the create was accepted
//! and the node-side CreateVm later failed with "image file not found".
//!
//! These tests boot the real `bff_router` and assert the fixed contract:
//!
//! - import accepts both `source_url` and the legacy `url` key;
//! - `vm create --image <name>` resolves the name to the stored local
//!   path (most recent image when display names collide);
//! - `file://` URIs are treated as local paths (scheme stripped);
//! - resolution by `image_id` still works;
//! - genuinely remote URLs are still rejected at create time;
//! - unknown image references keep the original ref (previous behavior).

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

/// Seed an operator user and return a usable JWT bearer token.
async fn seed_jwt(state: &AppState) -> String {
    sqlx::query(
        "INSERT INTO users (user_id, username, password_hash, role, must_change_password) \
         VALUES ('u-ops', 'ops', 'x', 'operator', 0)",
    )
    .execute(&state.pool)
    .await
    .expect("seed user");

    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600;
    let claims = chv_webui_bff::auth::Claims {
        sub: "u-ops".to_string(),
        username: "ops".to_string(),
        role: "operator".to_string(),
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

/// Create a VM via the BFF and return the stored vm_desired_state.image_ref.
async fn create_vm_image_ref(state: &AppState, token: &str, image: &str) -> Option<String> {
    let (status, body) = post_with_token(
        state.clone(),
        "/v1/vms/create",
        token,
        &format!(r#"{{"name":"vm-x","image_ref":"{image}"}}"#),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "vm create should be accepted, body: {body}"
    );
    let vm_id = body["vm_id"]
        .as_str()
        .expect("vm_id in response")
        .to_string();
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT image_ref FROM vm_desired_state WHERE vm_id = ?",
    )
    .bind(&vm_id)
    .fetch_one(&state.pool)
    .await
    .expect("query vm_desired_state")
}

// ---------------------------------------------------------------------------
// import: payload keys
// ---------------------------------------------------------------------------

#[tokio::test]
async fn import_accepts_source_url_key() {
    let state = build_state().await;
    let token = seed_jwt(&state).await;

    let (status, body) = post_with_token(
        state.clone(),
        "/v1/images/import",
        &token,
        r#"{"name":"img-a","source_url":"/tmp/a.img","format":"qcow2"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["source_url"], "/tmp/a.img");

    let stored: Option<String> =
        sqlx::query_scalar("SELECT source_url FROM images WHERE display_name = 'img-a'")
            .fetch_one(&state.pool)
            .await
            .expect("query images");
    assert_eq!(stored.as_deref(), Some("/tmp/a.img"));
}

#[tokio::test]
async fn import_accepts_legacy_url_key() {
    // Older chvctl builds sent only "url"; the BFF must accept it instead
    // of silently dropping the source (#339).
    let state = build_state().await;
    let token = seed_jwt(&state).await;

    let (status, body) = post_with_token(
        state.clone(),
        "/v1/images/import",
        &token,
        r#"{"name":"img-legacy","url":"/tmp/legacy.img","format":"qcow2"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    let stored: Option<String> =
        sqlx::query_scalar("SELECT source_url FROM images WHERE display_name = 'img-legacy'")
            .fetch_one(&state.pool)
            .await
            .expect("query images");
    assert_eq!(stored.as_deref(), Some("/tmp/legacy.img"));
}

// ---------------------------------------------------------------------------
// create: image resolution
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_resolves_image_by_name() {
    let state = build_state().await;
    let token = seed_jwt(&state).await;
    seed_node(&state).await;

    let (status, _) = post_with_token(
        state.clone(),
        "/v1/images/import",
        &token,
        r#"{"name":"ubuntu-noble","source_url":"/var/lib/chv/images/noble.img"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let image_ref = create_vm_image_ref(&state, &token, "ubuntu-noble")
        .await
        .expect("image_ref stored");
    assert_eq!(
        image_ref, "/var/lib/chv/images/noble.img",
        "create must resolve the imported image NAME to its local path"
    );
}

#[tokio::test]
async fn create_resolves_image_by_name_with_file_scheme() {
    let state = build_state().await;
    let token = seed_jwt(&state).await;
    seed_node(&state).await;

    let (status, _) = post_with_token(
        state.clone(),
        "/v1/images/import",
        &token,
        r#"{"name":"ubuntu-noble","source_url":"file:///var/lib/chv/images/noble.img"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let image_ref = create_vm_image_ref(&state, &token, "ubuntu-noble")
        .await
        .expect("image_ref stored");
    assert_eq!(
        image_ref, "/var/lib/chv/images/noble.img",
        "file:// URIs are local paths: the scheme must be stripped at create time"
    );
}

#[tokio::test]
async fn create_resolves_image_by_id() {
    let state = build_state().await;
    let token = seed_jwt(&state).await;
    seed_node(&state).await;

    let (status, body) = post_with_token(
        state.clone(),
        "/v1/images/import",
        &token,
        r#"{"name":"img-by-id","source_url":"/var/lib/chv/images/by-id.img"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let image_id = body["image_id"].as_str().expect("image_id").to_string();

    let image_ref = create_vm_image_ref(&state, &token, &image_id)
        .await
        .expect("image_ref stored");
    assert_eq!(image_ref, "/var/lib/chv/images/by-id.img");
}

#[tokio::test]
async fn create_name_resolution_prefers_most_recent() {
    // display_name is not unique; the name match must be deterministic
    // (most recently created image).
    let state = build_state().await;
    let token = seed_jwt(&state).await;
    seed_node(&state).await;

    let (status, _) = post_with_token(
        state.clone(),
        "/v1/images/import",
        &token,
        r#"{"name":"dupe","source_url":"/tmp/dupe-old.img"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // Distinct source so the duplicate-by-URL check does not fire.
    let (status, _) = post_with_token(
        state.clone(),
        "/v1/images/import",
        &token,
        r#"{"name":"dupe","source_url":"/tmp/dupe-new.img"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let image_ref = create_vm_image_ref(&state, &token, "dupe")
        .await
        .expect("image_ref stored");
    assert_eq!(image_ref, "/tmp/dupe-new.img");
}

#[tokio::test]
async fn create_rejects_remote_image_source() {
    let state = build_state().await;
    let token = seed_jwt(&state).await;
    seed_node(&state).await;

    let (status, _) = post_with_token(
        state.clone(),
        "/v1/images/import",
        &token,
        r#"{"name":"remote-img","source_url":"https://example.com/noble.img"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = post_with_token(
        state.clone(),
        "/v1/vms/create",
        &token,
        r#"{"name":"vm-remote","image_ref":"remote-img"}"#,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "remote sources must stay rejected: {body}"
    );
}

#[tokio::test]
async fn create_keeps_unknown_image_ref() {
    // Pre-existing behavior: an image reference that matches nothing in
    // the DB is passed through unchanged (the orchestrator decides).
    let state = build_state().await;
    let token = seed_jwt(&state).await;
    seed_node(&state).await;

    let image_ref = create_vm_image_ref(&state, &token, "/var/lib/chv/images/direct.img")
        .await
        .expect("image_ref stored");
    assert_eq!(
        image_ref, "/var/lib/chv/images/direct.img",
        "absolute-path refs pass through verbatim"
    );
}
