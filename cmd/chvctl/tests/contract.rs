//! #372 contract-test harness — chvctl's real request path against the
//! real BFF router (design §4 Option B, adopted 2026-10-06 §5.1 DP1;
//! PR 1 of the #372 decomposition).
//!
//! Root cause this harness kills (issue #372): chvctl builds request
//! bodies inline in each command handler and the BFF reads them with
//! ad-hoc `payload.get(...)` extraction — nothing shares or pins the
//! vocabulary, so a renamed field or route on either side keeps
//! compiling and the failure surfaces only at runtime (400 / 404 / a
//! silent drop / a hang). No test anywhere drove chvctl's own
//! request-building code; these rows do.
//!
//! Architecture:
//! - `AppState` is built exactly like the BFF's own integration suite
//!   (in-memory SQLite + `run_migrations` + a recording
//!   `MutationService` stub — the `tests/volume_snapshot_clone.rs`
//!   pattern, same shape in 21 BFF test files).
//! - The server is the control plane's `admin_router(state, metrics)`,
//!   NOT the plain `bff_router`: admin_router merges bff_router and
//!   additionally mounts `/admin/migrations/{id}/cancel`
//!   (`chv-controlplane-service/src/api/router.rs`), the route the
//!   repointed `migrate cancel` row (design §2.3/DP4) needs — a
//!   bff_router-only listener would 404 that row even post-fix. It
//!   reuses the same `AppState`, `admin_middleware` accepts the seeded
//!   BFF JWTs, and constructing it needs a `SharedConvergenceMetrics`
//!   (a default `convergence_metrics::new_shared()` instance suffices).
//! - The listener is a real ephemeral TCP socket
//!   (`TcpListener::bind("127.0.0.1:0")` + `axum::serve`) because
//!   chvctl's `BffClient` is a reqwest wrapper — the operator's actual
//!   wire path, including method, path, headers, and body.
//! - Each row invokes `commands::<group>::execute(&client, args,
//!   &format)` — the operator's actual command code.
//!
//! Per-row assertions (design §4 step 4):
//! - the HTTP status is not 404/405 (route + method exist) and not a
//!   `missing <field>` 400 (field names accepted) — implied by the
//!   command returning `Ok(())` for green rows;
//! - `RecordingMutations` received the expected method:args where a
//!   mutation is forwarded;
//! - every display column chvctl prints exists in a response item
//!   (kills the §2.7(a) display-drift class). The column lists below
//!   deliberately duplicate the `print_list` literals in
//!   `cmd/chvctl/src/commands/*.rs` — that duplication IS the pin: if
//!   either chvctl's columns or the BFF's response shape drift, the row
//!   fails.
//!
//! Row taxonomy at main (design §2 is the authority):
//! - GREEN rows: the command's route, method, field names, mutation
//!   forwarding, and display columns all match the BFF today.
//! - PINNED-BROKEN rows: known drift, pinned as it behaves TODAY with a
//!   TODO referencing the design §2 section and the PR that flips the
//!   row (PR 2 live fixes, PR 3 removals, PR 4 migrate reads). The
//!   harness must pass at main — red-where-known means asserting the
//!   current broken behavior, not failing.
//!
//! Pinned-broken rows in this file:
//! - `user delete` — 400 `missing user_id` (§2.1) → PR 2;
//! - `storage` group — 404 on all four subcommands (§2.2) → PR 3;
//! - `migrate` group — 404 on all four subcommands (§2.3) → PR 2/3/4;
//! - `backup` group — 404 on both subcommands (§2.4) → PR 3;
//! - `task watch` — hangs forever (§2.5) → PR 2 (timeout + poll pin);
//! - `network create --vlan` — `vlan` silently dropped (§2.6) → PR 2;
//! - display columns: `task list` (`type`, `created_at`), `network
//!   list` (`cidr`, `vlan`, `status`), `volume list` (`attached_to`),
//!   `image list` (`format`) — §2.7(a) → PR 2.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::extract::Request;
use axum::middleware::{self, Next};
use chv_common::SystemClock;
use chv_controlplane_store::{
    AlertRepository, ApplyRunRepository, BackupRepository, DesiredStateRepository,
    DriftReportRepository, EventRepository, ImageRepository, NetworkRepository, NodeRepository,
    ObservedStateRepository, OperationRepository, TopologyRepository,
};
use chv_webui_bff::mutations::MutationService;
use chv_webui_bff::{AppState, BffError};
use chvctl::client::{BffClient, CliError};
use chvctl::commands::{
    auth, backup, health, image, migrate, network, node, storage, task, user, vm, volume,
};
use chvctl::output::OutputFormat;
use serde_json::Value;
use sqlx::sqlite::SqlitePoolOptions;

// ---------------------------------------------------------------------------
// Recording mutation-service stub (the BFF test-suite pattern)
// ---------------------------------------------------------------------------

/// Records every mutation call as `method:arg:arg` for assertions.
#[derive(Default)]
struct RecordingMutations {
    calls: Mutex<Vec<String>>,
}

impl RecordingMutations {
    fn record(&self, entry: String) {
        self.calls.lock().unwrap().push(entry);
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    /// Assert the exact `method:args` entry reached the mutation service.
    fn assert_recorded(&self, expected: &str) {
        let calls = self.calls();
        assert!(
            calls.iter().any(|c| c == expected),
            "mutation service did not receive {expected:?}; calls: {calls:?}"
        );
    }
}

fn vm_response(vm_id: &str) -> chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse {
    chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse {
        accepted: true,
        task_id: format!("op-{vm_id}"),
        vm_id: vm_id.to_string(),
        summary: "recorded".to_string(),
    }
}

fn node_response(node_id: &str) -> chv_webui_bff_api::chv_webui_bff_v1::MutateNodeResponse {
    chv_webui_bff_api::chv_webui_bff_v1::MutateNodeResponse {
        accepted: true,
        task_id: format!("op-{node_id}"),
        node_id: node_id.to_string(),
        summary: "recorded".to_string(),
    }
}

fn volume_response(volume_id: &str) -> chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse {
    chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse {
        accepted: true,
        task_id: format!("op-{volume_id}"),
        volume_id: volume_id.to_string(),
        summary: "recorded".to_string(),
    }
}

fn network_response(
    network_id: &str,
) -> chv_webui_bff_api::chv_webui_bff_v1::MutateNetworkResponse {
    chv_webui_bff_api::chv_webui_bff_v1::MutateNetworkResponse {
        accepted: true,
        task_id: format!("op-{network_id}"),
        network_id: network_id.to_string(),
        summary: "recorded".to_string(),
    }
}

#[async_trait]
impl MutationService for RecordingMutations {
    async fn mutate_vm(
        &self,
        vm_id: String,
        action: String,
        force: bool,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse, BffError> {
        self.record(format!("mutate_vm:{vm_id}:{action}:{force}"));
        Ok(vm_response(&vm_id))
    }
    async fn migrate_vm(
        &self,
        vm_id: String,
        target_node_id: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse, BffError> {
        self.record(format!("migrate_vm:{vm_id}:{target_node_id}"));
        Ok(vm_response(&vm_id))
    }
    async fn snapshot_vm(
        &self,
        vm_id: String,
        destination: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse, BffError> {
        self.record(format!("snapshot_vm:{vm_id}:{destination}"));
        Ok(vm_response(&vm_id))
    }
    async fn restore_snapshot(
        &self,
        vm_id: String,
        source: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse, BffError> {
        self.record(format!("restore_snapshot:{vm_id}:{source}"));
        Ok(vm_response(&vm_id))
    }
    async fn mutate_node(
        &self,
        node_id: String,
        action: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateNodeResponse, BffError> {
        self.record(format!("mutate_node:{node_id}:{action}"));
        Ok(node_response(&node_id))
    }
    async fn mutate_volume(
        &self,
        volume_id: String,
        action: String,
        force: bool,
        _resize_bytes: Option<u64>,
        _vm_id: Option<String>,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
        self.record(format!("mutate_volume:{volume_id}:{action}:{force}"));
        Ok(volume_response(&volume_id))
    }
    async fn snapshot_volume(
        &self,
        volume_id: String,
        snapshot_name: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
        self.record(format!("snapshot_volume:{volume_id}:{snapshot_name}"));
        Ok(volume_response(&volume_id))
    }
    async fn restore_volume_snapshot(
        &self,
        volume_id: String,
        snapshot_name: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
        self.record(format!(
            "restore_volume_snapshot:{volume_id}:{snapshot_name}"
        ));
        Ok(volume_response(&volume_id))
    }
    async fn delete_volume_snapshot(
        &self,
        volume_id: String,
        snapshot_name: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
        self.record(format!(
            "delete_volume_snapshot:{volume_id}:{snapshot_name}"
        ));
        Ok(volume_response(&volume_id))
    }
    async fn clone_volume(
        &self,
        source_volume_id: String,
        target_volume_id: String,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateVolumeResponse, BffError> {
        self.record(format!(
            "clone_volume:{source_volume_id}:{target_volume_id}"
        ));
        Ok(volume_response(&target_volume_id))
    }
    async fn mutate_network(
        &self,
        network_id: String,
        action: String,
        force: bool,
        _requested_by: String,
    ) -> Result<chv_webui_bff_api::chv_webui_bff_v1::MutateNetworkResponse, BffError> {
        self.record(format!("mutate_network:{network_id}:{action}:{force}"));
        Ok(network_response(&network_id))
    }
}

// ---------------------------------------------------------------------------
// Harness: AppState + admin_router on a real ephemeral TCP listener
// ---------------------------------------------------------------------------

struct Harness {
    url: String,
    pool: sqlx::SqlitePool,
    jwt_secret: String,
    mutations: Arc<RecordingMutations>,
    /// Number of POST /v1/tasks requests the server has served — the
    /// `task watch` row's poll-hit assertion (design §2.5).
    task_polls: Arc<AtomicUsize>,
}

impl Harness {
    async fn start() -> Self {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory sqlite");
        chv_controlplane_store::run_migrations(&pool, None)
            .await
            .expect("run migrations");

        let mutations = Arc::new(RecordingMutations::default());
        let state = build_state(pool.clone(), mutations.clone());

        // A default/empty convergence-metrics instance suffices for these
        // rows — the harness never exercises convergence paths.
        let metrics = chv_controlplane_service::convergence_metrics::new_shared();
        let app = chv_controlplane_service::api::router::admin_router(state, metrics);

        let task_polls = Arc::new(AtomicUsize::new(0));
        let counter = task_polls.clone();
        let app = app.layer(middleware::from_fn(move |req: Request, next: Next| {
            let counter = counter.clone();
            async move {
                if req.uri().path() == "/v1/tasks" {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
                next.run(req).await
            }
        }));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral listener");
        let port = listener.local_addr().expect("local addr").port();
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("contract harness server");
        });

        Self {
            url: format!("http://127.0.0.1:{port}"),
            pool,
            jwt_secret: "test-secret".to_string(),
            mutations,
            task_polls,
        }
    }

    fn client(&self, token: Option<String>) -> BffClient {
        BffClient::new(self.url.clone(), token)
    }

    /// The BFF test-suite `seed_jwt_as` pattern: a user row plus a signed
    /// token for the harness's own JWT secret. `sub` is `u-{role}`.
    async fn seed_jwt_as(&self, role: &str) -> String {
        let user_id = format!("u-{role}");
        sqlx::query(
            "INSERT INTO users (user_id, username, password_hash, role, must_change_password) \
             VALUES (?, ?, 'x', ?, 0)",
        )
        .bind(&user_id)
        .bind(role)
        .bind(role)
        .execute(&self.pool)
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
            &jsonwebtoken::EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .expect("encode test token")
    }

    async fn seed_node(&self) {
        sqlx::query(
            "INSERT INTO nodes (node_id, hostname, display_name) VALUES ('n-1', 'h-1', 'Node 1')",
        )
        .execute(&self.pool)
        .await
        .expect("seed node");
    }

    /// A VM owned by the seeded operator (`u-operator`), on node `n-1`.
    async fn seed_vm(&self, vm_id: &str) {
        sqlx::query("INSERT INTO vms (vm_id, node_id, display_name, owner_id) VALUES (?, 'n-1', ?, 'u-operator')")
            .bind(vm_id)
            .bind(format!("VM {vm_id}"))
            .execute(&self.pool)
            .await
            .expect("seed vm");
    }

    /// Desired-state row for [`Harness::seed_vm`] — required by the
    /// resize/delete handlers (generation bump, delta quota math).
    async fn seed_vm_desired_state(&self, vm_id: &str) {
        sqlx::query(
            "INSERT INTO vm_desired_state \
             (vm_id, desired_generation, desired_status, desired_power_state, target_node_id, cpu_count, memory_bytes) \
             VALUES (?, 1, 'Active', 'Running', 'n-1', 2, 2147483648)",
        )
        .bind(vm_id)
        .execute(&self.pool)
        .await
        .expect("seed vm desired state");
    }

    /// A volume owned by the seeded operator, on node `n-1` (the
    /// `volume_snapshot_clone.rs` seeding shape).
    async fn seed_volume(&self, volume_id: &str) {
        sqlx::query(
            "INSERT INTO volumes (volume_id, node_id, display_name, owner_id, capacity_bytes, updated_at) \
             VALUES (?, 'n-1', ?, 'u-operator', 10737418240, '2026-01-01T00:00:00Z')",
        )
        .bind(volume_id)
        .bind(format!("{volume_id}-disk"))
        .execute(&self.pool)
        .await
        .expect("seed volume");
    }

    /// A bridge network owned by the seeded operator, with a desired-state
    /// row (the `network_resolution.rs` seeding shape).
    async fn seed_network(&self, network_id: &str) {
        sqlx::query(
            "INSERT INTO networks (network_id, node_id, display_name, network_class, owner_id, created_at, updated_at) \
             VALUES (?, 'n-1', ?, 'bridge', 'u-operator', strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
        )
        .bind(network_id)
        .bind(format!("net {network_id}"))
        .execute(&self.pool)
        .await
        .expect("seed network");
        sqlx::query(
            "INSERT INTO network_desired_state (network_id, desired_generation, desired_status, cidr, gateway, dhcp_enabled, ipam_mode, is_default, requested_at, updated_at) \
             VALUES (?, 1, 'Pending', '10.70.0.0/24', '10.70.0.1', 1, 'internal', 0, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
        )
        .bind(network_id)
        .execute(&self.pool)
        .await
        .expect("seed network desired state");
    }

    async fn seed_image(&self, image_id: &str) {
        sqlx::query(
            "INSERT INTO images \
             (image_id, display_name, image_type, format, size_bytes, checksum, source_url, os, version, usage_count, status, node_id, created_at, updated_at) \
             VALUES (?, ?, 'disk', 'qcow2', 1073741824, NULL, ?, '', '', 0, 'available', NULL, datetime('now'), datetime('now'))",
        )
        .bind(image_id)
        .bind(format!("img {image_id}"))
        .bind(format!("/tmp/{image_id}.img"))
        .execute(&self.pool)
        .await
        .expect("seed image");
    }

    async fn seed_operation(&self, operation_id: &str) {
        sqlx::query(
            "INSERT INTO operations (operation_id, idempotency_key, resource_kind, resource_id, operation_type, status, requested_by, requested_at, created_at, updated_at) \
             VALUES (?, ?, 'vm', 'vm-1', 'CreateVm', 'Succeeded', 'u-operator', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .bind(operation_id)
        .bind(format!("contract-{operation_id}"))
        .execute(&self.pool)
        .await
        .expect("seed operation");
    }
}

/// The BFF test-suite `AppState` builder (`tests/volume_snapshot_clone.rs`
/// pattern): in-memory pool, all repositories, recording mutation stub.
fn build_state(pool: sqlx::SqlitePool, mutations: Arc<RecordingMutations>) -> AppState {
    AppState {
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
        mutations,
        jwt_secret: "test-secret".to_string(),
        agent_runtime_dir: std::path::PathBuf::from("/var/lib/chv/agent"),
        cache: chv_webui_bff::BffCache::new(5),
        clock: Arc::new(SystemClock),
        pool,
    }
}

// ---------------------------------------------------------------------------
// Assertion helpers
// ---------------------------------------------------------------------------

/// Fetch a list endpoint's items through the same client the command
/// used (the command itself only prints; the row re-fetches to inspect
/// the response shape the columns render from).
async fn list_items(client: &BffClient, path: &str) -> Vec<Value> {
    let resp = client
        .post(path, &serde_json::json!({}))
        .await
        .expect("list request through the harness server");
    resp.get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

/// Every column chvctl's `print_list` prints exists as a key in every
/// response item (the §2.7(a) display-drift guard, green form).
fn assert_columns_present(items: &[Value], columns: &[&str]) {
    assert!(
        !items.is_empty(),
        "expected at least one item to check display columns {columns:?}"
    );
    for (i, item) in items.iter().enumerate() {
        for col in columns {
            assert!(
                item.get(*col).is_some(),
                "item {i} is missing column {col:?} — chvctl prints it, the BFF does not \
                 serve it (display drift)"
            );
        }
    }
}

/// The pinned-broken form of the display-drift guard: chvctl prints
/// these columns but the BFF does not serve them, so the column renders
/// empty. Asserting the ABSENCE pins today's drift; PR 2 (DP10) flips
/// these rows to `assert_columns_present`.
fn assert_columns_absent(items: &[Value], columns: &[&str]) {
    assert!(!items.is_empty(), "expected at least one item");
    for (i, item) in items.iter().enumerate() {
        for col in columns {
            assert!(
                item.get(*col).is_none(),
                "item {i} unexpectedly serves column {col:?} — the pinned-broken drift \
                 assumption no longer holds; flip this row to assert_columns_present"
            );
        }
    }
}

/// Assert a command failed with exactly the given API status (the
/// pinned-broken rows' 400/404 pins).
fn assert_api_error(result: Result<(), CliError>, status: u16) {
    match result {
        Err(CliError::Api { status: s, .. }) => assert_eq!(
            s, status,
            "expected the pinned HTTP {status} drift behavior"
        ),
        other => panic!("expected HTTP {status}, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Rows — auth
// ---------------------------------------------------------------------------

/// `chvctl login` — GREEN. Fields match `handlers/auth.rs` (`username`,
/// `password` → `token`); the credentials write is redirected to a
/// tempdir via `XDG_CONFIG_HOME`. The login rate limiter is a static
/// in-process map; a single row stays far under its window.
#[tokio::test]
async fn login_row() {
    let h = Harness::start().await;
    let hash = bcrypt::hash("contract-login-password", 12).expect("hash login password");
    sqlx::query(
        "INSERT INTO users (user_id, username, password_hash, role, must_change_password) \
         VALUES ('u-login', 'loginuser', ?, 'viewer', 0)",
    )
    .bind(&hash)
    .execute(&h.pool)
    .await
    .expect("seed login user");

    // `config::save_credentials` writes under `dirs::config_dir()`, which
    // honors XDG_CONFIG_HOME on Linux. Restore the previous value after
    // the row (review round 1: the process-global set was never cleared —
    // harmless today since only this row reads the config dir, but a
    // later row must not inherit this tempdir silently).
    let dir = tempfile::tempdir().expect("login config tempdir");
    let prior_xdg = std::env::var("XDG_CONFIG_HOME").ok();
    std::env::set_var("XDG_CONFIG_HOME", dir.path());

    let client = h.client(None);
    auth::execute(
        &client,
        auth::LoginArgs {
            username: "loginuser".to_string(),
            password: "contract-login-password".to_string(),
        },
    )
    .await
    .expect("chvctl login against /v1/auth/login");

    let credentials = std::fs::read_to_string(dir.path().join("chvctl/credentials"))
        .expect("credentials file written under XDG_CONFIG_HOME");
    assert!(!credentials.trim().is_empty(), "token was persisted");

    match prior_xdg {
        Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
        None => std::env::remove_var("XDG_CONFIG_HOME"),
    }
}

// ---------------------------------------------------------------------------
// Rows — vm (all GREEN at main)
// ---------------------------------------------------------------------------

/// `chvctl vm list` — GREEN. Columns match `handlers/vms.rs::list_vms`.
#[tokio::test]
async fn vm_list_row() {
    let h = Harness::start().await;
    h.seed_node().await;
    h.seed_vm("vm-1").await;
    let token = h.seed_jwt_as("operator").await;
    let client = h.client(Some(token));

    vm::execute(&client, vm::VmCommands::List, &OutputFormat::Json)
        .await
        .expect("chvctl vm list against POST /v1/vms");

    let items = list_items(&client, "/v1/vms").await;
    assert_columns_present(
        &items,
        &["vm_id", "name", "power_state", "node_id", "cpu", "memory"],
    );
}

/// `chvctl vm get <id>` — GREEN.
#[tokio::test]
async fn vm_get_row() {
    let h = Harness::start().await;
    h.seed_node().await;
    h.seed_vm("vm-1").await;
    let token = h.seed_jwt_as("operator").await;

    vm::execute(
        &h.client(Some(token)),
        vm::VmCommands::Get {
            vm_id: "vm-1".to_string(),
        },
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl vm get against POST /v1/vms/get");
}

/// `chvctl vm create` — GREEN. Field names (`name`, `cpu_count`,
/// `memory_bytes`, `image_ref`, `network_id`) are the BFF create_vm
/// contract; the create needs an enrolled node (the handler's
/// default-placement rule). §2.7(b) records the capability gap (no
/// `--node`/`--storage-class`/`--disk-size-gb`/`--cloud-init` flags) —
/// those land in PR 2 (DP9) with their own rows.
#[tokio::test]
async fn vm_create_row() {
    let h = Harness::start().await;
    h.seed_node().await;
    let token = h.seed_jwt_as("operator").await;
    let client = h.client(Some(token));

    vm::execute(
        &client,
        vm::VmCommands::Create {
            name: "contract-vm".to_string(),
            cpu: Some(2),
            memory: Some("512M".to_string()),
            image: Some("default".to_string()),
            network: Some("default".to_string()),
        },
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl vm create against POST /v1/vms/create");

    // The VM actually landed (route + field names accepted end-to-end).
    let items = list_items(&client, "/v1/vms").await;
    let vm = items
        .iter()
        .find(|i| i.get("name").and_then(Value::as_str) == Some("contract-vm"))
        .expect("created VM is visible in vm list");
    // Optional-field probes (review round 1): `cpu_count`/`memory_bytes`
    // are optional-with-default in create_vm, so a BFF-side rename would
    // silently default them and keep a name-only check green. vm list
    // serves them display-formatted (cpu as a string, memory
    // human-readable) — assert the created shape round-tripped exactly
    // what chvctl sent (512M = 536870912 bytes = "512.0 MiB").
    assert_eq!(
        vm.get("cpu").and_then(Value::as_str),
        Some("2"),
        "cpu_count must round-trip — a BFF rename would silently default it"
    );
    assert_eq!(
        vm.get("memory").and_then(Value::as_str),
        Some("512.0 MiB"),
        "memory_bytes must round-trip — a BFF rename would silently default it"
    );
    // `image_ref`/`network_id` are also optional-with-default but are not
    // served by vm list (only cpu/memory are); they stay name-probed here
    // — the PR-2 flags (DP9) add their own rows with shape assertions.
}

/// `chvctl vm start` — GREEN; forwarded to the mutation service.
#[tokio::test]
async fn vm_start_row() {
    let h = Harness::start().await;
    h.seed_node().await;
    h.seed_vm("vm-1").await;
    let token = h.seed_jwt_as("operator").await;

    vm::execute(
        &h.client(Some(token)),
        vm::VmCommands::Start {
            vm_id: "vm-1".to_string(),
        },
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl vm start against POST /v1/vms/mutate");
    h.mutations.assert_recorded("mutate_vm:vm-1:start:false");
}

/// `chvctl vm stop` — GREEN; forwarded to the mutation service.
#[tokio::test]
async fn vm_stop_row() {
    let h = Harness::start().await;
    h.seed_node().await;
    h.seed_vm("vm-1").await;
    let token = h.seed_jwt_as("operator").await;

    vm::execute(
        &h.client(Some(token)),
        vm::VmCommands::Stop {
            vm_id: "vm-1".to_string(),
        },
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl vm stop against POST /v1/vms/mutate");
    h.mutations.assert_recorded("mutate_vm:vm-1:stop:false");
}

/// `chvctl vm reboot` — GREEN; the wire action is `restart` (the BFF's
/// mutation vocabulary), forwarded to the mutation service.
#[tokio::test]
async fn vm_reboot_row() {
    let h = Harness::start().await;
    h.seed_node().await;
    h.seed_vm("vm-1").await;
    let token = h.seed_jwt_as("operator").await;

    vm::execute(
        &h.client(Some(token)),
        vm::VmCommands::Reboot {
            vm_id: "vm-1".to_string(),
        },
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl vm reboot against POST /v1/vms/mutate");
    h.mutations.assert_recorded("mutate_vm:vm-1:restart:false");
}

/// `chvctl vm delete` — GREEN. The delete is a BFF-internal journaled
/// path (tombstone + operation row), not a mutation-service forward.
#[tokio::test]
async fn vm_delete_row() {
    let h = Harness::start().await;
    h.seed_node().await;
    h.seed_vm("vm-1").await;
    h.seed_vm_desired_state("vm-1").await;
    let token = h.seed_jwt_as("operator").await;

    vm::execute(
        &h.client(Some(token)),
        vm::VmCommands::Delete {
            vm_id: "vm-1".to_string(),
        },
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl vm delete against POST /v1/vms/delete");
}

/// `chvctl vm migrate` — GREEN; forwarded to the mutation service with
/// the `target_node_id` field name (this is the path `migrate start`
/// should delegate to — design §2.3).
#[tokio::test]
async fn vm_migrate_row() {
    let h = Harness::start().await;
    h.seed_node().await;
    h.seed_vm("vm-1").await;
    let token = h.seed_jwt_as("operator").await;

    vm::execute(
        &h.client(Some(token)),
        vm::VmCommands::Migrate {
            vm_id: "vm-1".to_string(),
            to: "n-2".to_string(),
        },
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl vm migrate against POST /v1/vms/mutate");
    h.mutations.assert_recorded("migrate_vm:vm-1:n-2");
}

/// `chvctl vm resize` — GREEN (`cpu_count` / `memory_bytes` field names).
#[tokio::test]
async fn vm_resize_row() {
    let h = Harness::start().await;
    h.seed_node().await;
    h.seed_vm("vm-1").await;
    h.seed_vm_desired_state("vm-1").await;
    let token = h.seed_jwt_as("operator").await;

    vm::execute(
        &h.client(Some(token)),
        vm::VmCommands::Resize {
            vm_id: "vm-1".to_string(),
            cpu: Some(4),
            memory: Some("1G".to_string()),
        },
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl vm resize against POST /v1/vms/resize");
}

// ---------------------------------------------------------------------------
// Rows — node (all GREEN at main)
// ---------------------------------------------------------------------------

/// `chvctl node list` — GREEN. Columns match `handlers/nodes.rs`.
#[tokio::test]
async fn node_list_row() {
    let h = Harness::start().await;
    h.seed_node().await;
    let token = h.seed_jwt_as("operator").await;
    let client = h.client(Some(token));

    node::execute(&client, node::NodeCommands::List, &OutputFormat::Json)
        .await
        .expect("chvctl node list against POST /v1/nodes");

    let items = list_items(&client, "/v1/nodes").await;
    assert_columns_present(
        &items,
        &["node_id", "name", "state", "health", "cpu", "memory"],
    );
}

/// `chvctl node get <id>` — GREEN.
#[tokio::test]
async fn node_get_row() {
    let h = Harness::start().await;
    h.seed_node().await;
    let token = h.seed_jwt_as("operator").await;

    node::execute(
        &h.client(Some(token)),
        node::NodeCommands::Get {
            node_id: "n-1".to_string(),
        },
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl node get against POST /v1/nodes/get");
}

/// `chvctl node drain` — GREEN; forwarded to the mutation service.
#[tokio::test]
async fn node_drain_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("operator").await;

    node::execute(
        &h.client(Some(token)),
        node::NodeCommands::Drain {
            node_id: "n-1".to_string(),
        },
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl node drain against POST /v1/nodes/mutate");
    h.mutations.assert_recorded("mutate_node:n-1:drain");
}

/// `chvctl node maintenance --enable` — GREEN; action
/// `enter_maintenance` forwarded to the mutation service.
#[tokio::test]
async fn node_maintenance_enter_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("operator").await;

    node::execute(
        &h.client(Some(token)),
        node::NodeCommands::Maintenance {
            node_id: "n-1".to_string(),
            enable: true,
        },
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl node maintenance (enter) against POST /v1/nodes/mutate");
    h.mutations
        .assert_recorded("mutate_node:n-1:enter_maintenance");
}

/// `chvctl node maintenance` (no --enable) — GREEN; action
/// `exit_maintenance` forwarded to the mutation service.
#[tokio::test]
async fn node_maintenance_exit_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("operator").await;

    node::execute(
        &h.client(Some(token)),
        node::NodeCommands::Maintenance {
            node_id: "n-1".to_string(),
            enable: false,
        },
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl node maintenance (exit) against POST /v1/nodes/mutate");
    h.mutations
        .assert_recorded("mutate_node:n-1:exit_maintenance");
}

// ---------------------------------------------------------------------------
// Rows — image (route/field GREEN; `image list` display-pinned)
// ---------------------------------------------------------------------------

/// `chvctl image list` — route/fields GREEN; display PINNED-BROKEN:
/// chvctl prints `format`, which no item serves (design §2.7(a));
/// PR 2 (DP10) corrects the column and flips this to
/// `assert_columns_present`.
#[tokio::test]
async fn image_list_row() {
    let h = Harness::start().await;
    h.seed_image("img-1").await;
    let token = h.seed_jwt_as("operator").await;
    let client = h.client(Some(token));

    image::execute(&client, image::ImageCommands::List, &OutputFormat::Json)
        .await
        .expect("chvctl image list against POST /v1/images");

    let items = list_items(&client, "/v1/images").await;
    assert_columns_present(&items, &["image_id", "name", "size", "status"]);
    // TODO(#372 PR 2, design §2.7(a)/DP10): `format` is a phantom column —
    // the BFF's list items never serve it, so the column renders empty.
    assert_columns_absent(&items, &["format"]);
}

/// `chvctl image import` — GREEN. `source_url` is the BFF contract key;
/// the redundant dual-key `url` send (design §2.1 residual / DP10) is
//  dropped in PR 2 — the server alias stays either way.
#[tokio::test]
async fn image_import_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("operator").await;
    let client = h.client(Some(token));

    image::execute(
        &client,
        image::ImageCommands::Import {
            name: "contract-image".to_string(),
            url: "/tmp/chv-contract-row.img".to_string(),
            format: "qcow2".to_string(),
        },
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl image import against POST /v1/images/import");
}

/// `chvctl image delete` — GREEN.
#[tokio::test]
async fn image_delete_row() {
    let h = Harness::start().await;
    h.seed_image("img-1").await;
    let token = h.seed_jwt_as("operator").await;

    image::execute(
        &h.client(Some(token)),
        image::ImageCommands::Delete {
            image_id: "img-1".to_string(),
        },
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl image delete against POST /v1/images/delete");
}

// ---------------------------------------------------------------------------
// Rows — volume (route/field GREEN; `volume list` display-pinned)
// ---------------------------------------------------------------------------

/// `chvctl volume list` — route/fields GREEN; display PINNED-BROKEN:
/// chvctl prints `attached_to`, which no item serves (design §2.7(a));
/// PR 2 (DP10) corrects the column.
#[tokio::test]
async fn volume_list_row() {
    let h = Harness::start().await;
    h.seed_node().await;
    h.seed_volume("vol-1").await;
    let token = h.seed_jwt_as("operator").await;
    let client = h.client(Some(token));

    volume::execute(&client, volume::VolumeCommands::List, &OutputFormat::Json)
        .await
        .expect("chvctl volume list against POST /v1/volumes");

    let items = list_items(&client, "/v1/volumes").await;
    assert_columns_present(&items, &["volume_id", "name", "size", "status"]);
    // TODO(#372 PR 2, design §2.7(a)/DP10): `attached_to` is a phantom
    // column — the BFF serves `attached_vm_id`/`attached_vm_name`.
    assert_columns_absent(&items, &["attached_to"]);
}

/// `chvctl volume snapshot` — GREEN; `snapshot_name` field name (#373's
//  fix) forwarded to the mutation service.
#[tokio::test]
async fn volume_snapshot_row() {
    let h = Harness::start().await;
    h.seed_node().await;
    h.seed_volume("vol-1").await;
    let token = h.seed_jwt_as("operator").await;

    volume::execute(
        &h.client(Some(token)),
        volume::VolumeCommands::Snapshot {
            volume_id: "vol-1".to_string(),
            name: "snap-1".to_string(),
        },
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl volume snapshot against POST /v1/volumes/snapshot");
    h.mutations.assert_recorded("snapshot_volume:vol-1:snap-1");
}

/// `chvctl volume clone` — GREEN; `source_volume_id`/`target_volume_id`
/// field names (#373's fix) forwarded to the mutation service.
#[tokio::test]
async fn volume_clone_row() {
    let h = Harness::start().await;
    h.seed_node().await;
    h.seed_volume("vol-1").await;
    let token = h.seed_jwt_as("operator").await;

    volume::execute(
        &h.client(Some(token)),
        volume::VolumeCommands::Clone {
            volume_id: "vol-1".to_string(),
            name: "vol-2".to_string(),
        },
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl volume clone against POST /v1/volumes/clone");
    h.mutations.assert_recorded("clone_volume:vol-1:vol-2");
}

// ---------------------------------------------------------------------------
// Rows — network (route/field GREEN; list display-pinned; --vlan pinned)
// ---------------------------------------------------------------------------

/// `chvctl network list` — route/fields GREEN; display PINNED-BROKEN:
/// chvctl prints `cidr`, `vlan`, and `status`, which no item serves
/// (design §2.7(a)); PR 2 (DP10) corrects the columns.
#[tokio::test]
async fn network_list_row() {
    let h = Harness::start().await;
    h.seed_node().await;
    h.seed_network("net-1").await;
    let token = h.seed_jwt_as("operator").await;
    let client = h.client(Some(token));

    network::execute(&client, network::NetworkCommands::List, &OutputFormat::Json)
        .await
        .expect("chvctl network list against POST /v1/networks");

    let items = list_items(&client, "/v1/networks").await;
    assert_columns_present(&items, &["network_id", "name"]);
    // TODO(#372 PR 2, design §2.7(a)/DP10): `cidr`, `vlan`, `status` are
    // phantom columns — the BFF serves scope/health/dhcp_enabled/etc.
    assert_columns_absent(&items, &["cidr", "vlan", "status"]);
}

/// `chvctl network create` (no --vlan) — GREEN.
#[tokio::test]
async fn network_create_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("operator").await;
    let client = h.client(Some(token));

    network::execute(
        &client,
        network::NetworkCommands::Create {
            name: "contract-net".to_string(),
            cidr: "10.60.0.0/24".to_string(),
            vlan: None,
        },
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl network create against POST /v1/networks/create");

    let items = list_items(&client, "/v1/networks").await;
    let net = items
        .iter()
        .find(|i| i.get("name").and_then(Value::as_str) == Some("contract-net"))
        .expect("created network is visible in network list");
    // Optional-field probe (review round 1): `cidr` is optional in
    // create_network and is NOT served by network list (a §2.7(a)
    // phantom column), so a name-only check can't see a BFF-side cidr
    // rename (it would silently default). Fetch the detail route — which
    // DOES serve cidr from network_desired_state — and assert the value
    // round-tripped exactly.
    let network_id = net
        .get("network_id")
        .and_then(Value::as_str)
        .expect("network list serves network_id");
    let detail = client
        .post(
            "/v1/networks/get",
            &serde_json::json!({ "network_id": network_id }),
        )
        .await
        .expect("network detail through the harness server");
    let detail = detail
        .get("detail")
        .expect("networks/get nests the payload under \"detail\"");
    assert_eq!(
        detail.get("cidr").and_then(Value::as_str),
        Some("10.60.0.0/24"),
        "cidr must round-trip — a BFF rename would silently default it"
    );
}

/// `chvctl network create --vlan` — PINNED-BROKEN (design §2.6/DP7): the
/// BFF's create_network reads no `vlan` key — the flag is silently
/// dropped with no error, and the capability does not exist at any
/// layer. The row pins today's behavior (the request still succeeds —
/// accepted-but-ignored); PR 2 removes the flag and this row becomes a
/// rejection/absence assertion.
#[tokio::test]
async fn network_create_vlan_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("operator").await;
    let client = h.client(Some(token));

    network::execute(
        &client,
        network::NetworkCommands::Create {
            name: "contract-vlan-net".to_string(),
            cidr: "10.61.0.0/24".to_string(),
            vlan: Some(42),
        },
        &OutputFormat::Json,
    )
    .await
    // TODO(#372 PR 2, design §2.6/DP7): the request succeeding IS the
    // drift pin — `vlan` is read by nothing (no table column, no UI
    // field) and silently dropped.
    .expect("chvctl network create --vlan is accepted (and the vlan silently dropped)");

    let items = list_items(&client, "/v1/networks").await;
    assert_columns_absent(&items, &["vlan"]);
}

/// `chvctl network delete` — GREEN.
#[tokio::test]
async fn network_delete_row() {
    let h = Harness::start().await;
    h.seed_node().await;
    h.seed_network("net-1").await;
    let token = h.seed_jwt_as("operator").await;

    network::execute(
        &h.client(Some(token)),
        network::NetworkCommands::Delete {
            network_id: "net-1".to_string(),
        },
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl network delete against POST /v1/networks/delete");
}

// ---------------------------------------------------------------------------
// Rows — task (list display-pinned; watch hang-pinned)
// ---------------------------------------------------------------------------

/// `chvctl task list` — route/fields GREEN; display PINNED-BROKEN:
/// chvctl prints `type` and `created_at`, which no item serves (design
/// §2.7(a)); PR 2 (DP10) corrects the columns.
#[tokio::test]
async fn task_list_row() {
    let h = Harness::start().await;
    h.seed_operation("op-1").await;
    let token = h.seed_jwt_as("operator").await;
    let client = h.client(Some(token));

    task::execute(&client, task::TaskCommands::List, &OutputFormat::Json)
        .await
        .expect("chvctl task list against POST /v1/tasks");

    let items = list_items(&client, "/v1/tasks").await;
    assert_columns_present(&items, &["task_id", "status", "resource_id"]);
    // TODO(#372 PR 2, design §2.7(a)/DP10): `type` and `created_at` are
    // phantom columns — the BFF serves `operation` and
    // `started_unix_ms`/`finished_unix_ms`.
    assert_columns_absent(&items, &["type", "created_at"]);
}

/// `chvctl task watch` — PINNED-BROKEN (design §2.5/DP6): the command
/// polls `POST /v1/tasks` with `{"task_id": ...}`, but the handler reads
/// only pagination filters (the key is ignored) and the response has no
/// top-level `status` — so the loop prints `Status: unknown` every 2 s
/// forever. The row wraps the command in a timeout and pins BOTH the
/// hang and the fact that at least one poll hit the endpoint. PR 2 (new
/// `POST /v1/tasks/get` + vocabulary fix + poll cap) flips this row to a
/// completion assertion.
#[tokio::test]
async fn task_watch_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("operator").await;
    let client = h.client(Some(token));

    let started = std::time::Instant::now();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(6),
        task::execute(
            &client,
            task::TaskCommands::Watch {
                task_id: "op-none".to_string(),
            },
            &OutputFormat::Json,
        ),
    )
    .await;

    // TODO(#372 PR 2, design §2.5/DP6): the timeout firing IS the drift
    // pin — `watch` never terminates today.
    assert!(
        result.is_err(),
        "pre-fix `task watch` must hang (design §2.5); it returned {result:?}"
    );
    assert!(
        started.elapsed() >= std::time::Duration::from_secs(5),
        "the command hung (polled and slept), it did not fail fast"
    );
    assert!(
        h.task_polls.load(Ordering::SeqCst) >= 1,
        "at least one poll reached POST /v1/tasks"
    );
}

// ---------------------------------------------------------------------------
// Rows — user (list/create GREEN; delete 400-pinned)
// ---------------------------------------------------------------------------

/// `chvctl user list` — GREEN (admin tier). Columns match
/// `handlers/users.rs`.
#[tokio::test]
async fn user_list_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("admin").await;
    let client = h.client(Some(token));

    user::execute(&client, user::UserCommands::List, &OutputFormat::Json)
        .await
        .expect("chvctl user list against POST /v1/users");

    let items = list_items(&client, "/v1/users").await;
    assert_columns_present(&items, &["username", "role", "created_at"]);
}

/// `chvctl user create` — GREEN (admin tier).
#[tokio::test]
async fn user_create_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("admin").await;

    user::execute(
        &h.client(Some(token)),
        user::UserCommands::Create {
            username: "contract-user".to_string(),
            password: "contract-password-123".to_string(),
            role: "viewer".to_string(),
        },
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl user create against POST /v1/users/create");
}

/// `chvctl user delete` — PINNED-BROKEN (design §2.1/DP2): chvctl sends
/// `{"username": ...}` where the BFF requires `user_id`, so every
/// invocation 400s with `missing user_id` — the command has never
/// worked. The row pins today's exact behavior; PR 2 changes the
/// positional arg to `user_id` and flips this row to `Ok(())`.
#[tokio::test]
async fn user_delete_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("admin").await;

    let result = user::execute(
        &h.client(Some(token)),
        user::UserCommands::Delete {
            username: "contract-user".to_string(),
        },
        &OutputFormat::Json,
    )
    .await;
    // TODO(#372 PR 2, design §2.1/DP2): the 400 IS the drift pin.
    assert_api_error(result, 400);
}

// ---------------------------------------------------------------------------
// Rows — storage group (all 404-pinned; design §2.2/DP3 → removal PR 3)
// ---------------------------------------------------------------------------

/// `chvctl storage list` — PINNED-BROKEN (design §2.2/DP3): chvctl calls
/// `GET /v1/storage/pools`; the BFF serves `POST /v1/storage-pools` (and
/// the storage_pools catalog is a phantom surface — #379 C4). PR 3
/// removes the group.
#[tokio::test]
async fn storage_list_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("operator").await;

    let result = storage::execute(
        &h.client(Some(token)),
        storage::StorageCommands::List,
        &OutputFormat::Json,
    )
    .await;
    // TODO(#372 PR 3, design §2.2/DP3): the 404 IS the drift pin.
    assert_api_error(result, 404);
}

/// `chvctl storage show` — PINNED-BROKEN (design §2.2/DP3): no per-pool
/// get route exists anywhere. PR 3 removes the group.
#[tokio::test]
async fn storage_show_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("operator").await;

    let result = storage::execute(
        &h.client(Some(token)),
        storage::StorageCommands::Show {
            pool_id: "pool-1".to_string(),
        },
        &OutputFormat::Json,
    )
    .await;
    // TODO(#372 PR 3, design §2.2/DP3): the 404 IS the drift pin.
    assert_api_error(result, 404);
}

/// `chvctl storage create` — PINNED-BROKEN (design §2.2/DP3): chvctl
/// calls `POST /v1/storage/pools` with `{name, backend, path}`; the BFF
/// serves `POST /v1/storage-pools/create` reading
/// `{name, node_id, pool_type|backend_class, ...}` — path, method, and
/// field names all drift. PR 3 removes the group.
#[tokio::test]
async fn storage_create_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("operator").await;

    let result = storage::execute(
        &h.client(Some(token)),
        storage::StorageCommands::Create {
            name: "contract-pool".to_string(),
            backend: "local".to_string(),
            path: Some("/var/lib/chv/pools".to_string()),
        },
        &OutputFormat::Json,
    )
    .await;
    // TODO(#372 PR 3, design §2.2/DP3): the 404 IS the drift pin.
    assert_api_error(result, 404);
}

/// `chvctl storage delete` — PINNED-BROKEN (design §2.2/DP3): no delete
/// route exists anywhere. PR 3 removes the group.
#[tokio::test]
async fn storage_delete_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("operator").await;

    let result = storage::execute(
        &h.client(Some(token)),
        storage::StorageCommands::Delete {
            pool_id: "pool-1".to_string(),
        },
        &OutputFormat::Json,
    )
    .await;
    // TODO(#372 PR 3, design §2.2/DP3): the 404 IS the drift pin.
    assert_api_error(result, 404);
}

// ---------------------------------------------------------------------------
// Rows — migrate group (all 404-pinned; design §2.3/DP4)
// ---------------------------------------------------------------------------

/// `chvctl migrate start` — PINNED-BROKEN (design §2.3/DP4): chvctl
/// calls `POST /v1/migrations` with `{vm_id, target_node}`; the real
/// entry point is the vm-mutate migrate action (`target_node_id`) that
/// `chvctl vm migrate` already drives. PR 4 repoints the command and
/// flips this row (PR 3 removes it if the repoint is declined).
#[tokio::test]
async fn migrate_start_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("operator").await;

    let result = migrate::execute(
        &h.client(Some(token)),
        migrate::MigrateCommands::Start {
            vm_id: "vm-1".to_string(),
            target_node: "n-2".to_string(),
        },
        &OutputFormat::Json,
    )
    .await;
    // TODO(#372 PR 4, design §2.3/DP4): the 404 IS the drift pin.
    assert_api_error(result, 404);
}

/// `chvctl migrate status` — PINNED-BROKEN (design §2.3/DP4): no
/// migration read route exists. PR 3 removes the subcommand (PR 4
/// optionally adds viewer-tier read routes).
#[tokio::test]
async fn migrate_status_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("operator").await;

    let result = migrate::execute(
        &h.client(Some(token)),
        migrate::MigrateCommands::Status {
            migration_id: "mig-1".to_string(),
        },
        &OutputFormat::Json,
    )
    .await;
    // TODO(#372 PR 3/PR 4, design §2.3/DP4): the 404 IS the drift pin.
    assert_api_error(result, 404);
}

/// `chvctl migrate cancel` — PINNED-BROKEN (design §2.3/DP4): chvctl
/// calls `POST /v1/migrations/{id}/cancel`; the real route is
/// `POST /admin/migrations/{id}/cancel` on the CP admin router (admin
/// tier) — which this harness deliberately serves, so the repointed row
/// in PR 4 flips green without harness changes.
#[tokio::test]
async fn migrate_cancel_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("admin").await;

    let result = migrate::execute(
        &h.client(Some(token)),
        migrate::MigrateCommands::Cancel {
            migration_id: "mig-1".to_string(),
        },
        &OutputFormat::Json,
    )
    .await;
    // TODO(#372 PR 4, design §2.3/DP4): the 404 IS the drift pin — the
    // cancel capability exists on /admin/migrations/{id}/cancel.
    assert_api_error(result, 404);
}

/// `chvctl migrate list` — PINNED-BROKEN (design §2.3/DP4): no migration
/// list route exists. PR 3 removes the subcommand (PR 4 optionally adds
/// the viewer-tier read route).
#[tokio::test]
async fn migrate_list_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("operator").await;

    let result = migrate::execute(
        &h.client(Some(token)),
        migrate::MigrateCommands::List,
        &OutputFormat::Json,
    )
    .await;
    // TODO(#372 PR 3/PR 4, design §2.3/DP4): the 404 IS the drift pin.
    assert_api_error(result, 404);
}

// ---------------------------------------------------------------------------
// Rows — backup group (both 404-pinned; design §2.4/DP5 → removal PR 3)
// ---------------------------------------------------------------------------

/// `chvctl backup list` — PINNED-BROKEN (design §2.4/DP5): chvctl calls
/// `GET /v1/backups`; the BFF serves `GET /v1/backups/jobs|schedules|
/// restores`. PR 3 removes the group.
#[tokio::test]
async fn backup_list_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("operator").await;

    let result = backup::execute(
        &h.client(Some(token)),
        backup::BackupCommands::List,
        &OutputFormat::Json,
    )
    .await;
    // TODO(#372 PR 3, design §2.4/DP5): the 404 IS the drift pin.
    assert_api_error(result, 404);
}

/// `chvctl backup run` — PINNED-BROKEN (design §2.4/DP5): chvctl calls
/// `POST /v1/backups/run` with `{vm_id, label?}`; the BFF serves
/// `POST /v1/backups/jobs/:job_id/execute` — and even repointed, the
/// live worker's execute is a guaranteed-fail no-op ("Backup is not
/// DR"). PR 3 removes the group.
#[tokio::test]
async fn backup_run_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("operator").await;

    let result = backup::execute(
        &h.client(Some(token)),
        backup::BackupCommands::Run {
            vm_id: "vm-1".to_string(),
            label: Some("nightly".to_string()),
        },
        &OutputFormat::Json,
    )
    .await;
    // TODO(#372 PR 3, design §2.4/DP5): the 404 IS the drift pin.
    assert_api_error(result, 404);
}

// ---------------------------------------------------------------------------
// Rows — health (all GREEN at main; fixed under #320)
// ---------------------------------------------------------------------------

/// `chvctl health check` — GREEN (`GET /v1/health`).
#[tokio::test]
async fn health_check_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("operator").await;

    health::execute(
        &h.client(Some(token)),
        health::HealthCommands::Check,
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl health check against GET /v1/health");
}

/// `chvctl health cluster` — GREEN (`GET /v1/cluster/health`).
#[tokio::test]
async fn health_cluster_row() {
    let h = Harness::start().await;
    let token = h.seed_jwt_as("operator").await;

    health::execute(
        &h.client(Some(token)),
        health::HealthCommands::Cluster,
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl health cluster against GET /v1/cluster/health");
}

/// `chvctl health report <node>` — GREEN (`GET /v1/nodes/{id}/health`).
#[tokio::test]
async fn health_report_row() {
    let h = Harness::start().await;
    h.seed_node().await;
    let token = h.seed_jwt_as("operator").await;

    health::execute(
        &h.client(Some(token)),
        health::HealthCommands::Report {
            node_id: "n-1".to_string(),
        },
        &OutputFormat::Json,
    )
    .await
    .expect("chvctl health report against GET /v1/nodes/{id}/health");
}
