# Rust Control Plane Phase 1 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement the CHV control plane Phase 1 in Rust: cleanup legacy Go code, add bootstrap token validation, strict desired-state mappers, ReconcileService, LifecycleService, axum admin HTTP server, optional gRPC mTLS, and proper error-to-status mapping.

**Architecture:** Build on the existing workspace (`chv-controlplane-types`, `chv-controlplane-store`, `chv-controlplane-service`, `cmd/chv-controlplane`). Add new repositories, service modules, and an axum router. Keep desired/observed state separate. No outbound `chv-agent` client in Phase 1.

**Tech Stack:** Rust, Tokio, tonic, axum, sqlx (PostgreSQL), serde, rcgen, testcontainers, metrics-exporter-prometheus.

---

## Reference docs
- `docs/plans/2026-04-14-rust-controlplane-phase1-design.md`
- `docs/specs/adr/002-control-plane-boundary.md`
- `proto/controlplane/control-plane-node.proto`
- `cmd/chv-controlplane/migrations/0001_initial.sql`
- `cmd/chv-controlplane/migrations/0002_inventory_bootstrap.sql`

---

### Task 1: Delete legacy Go control-plane code

**Files:**
- Delete: `legacy/go-controlplane/` (entire directory tree)

**Step 1: Verify nothing in the workspace references the legacy path**

Run:
```bash
grep -r "legacy/go-controlplane" Cargo.toml .github/ cmd/ crates/ gen/ || echo "No references found"
```
Expected: No references (or only in docs that will be updated).

**Step 2: Remove the directory**

Run:
```bash
rm -rf legacy/go-controlplane
```

**Step 3: Commit**

Run:
```bash
git add -A && git commit -m "cleanup: remove legacy go-controlplane"
```

---

### Task 2: Add bootstrap token table and repository

**Files:**
- Create: `cmd/chv-controlplane/migrations/0003_bootstrap_tokens.sql`
- Create: `crates/chv-controlplane-store/src/bootstrap_tokens.rs`
- Modify: `crates/chv-controlplane-store/src/lib.rs`

**Step 1: Write migration**

Create `cmd/chv-controlplane/migrations/0003_bootstrap_tokens.sql`:
```sql
CREATE TABLE IF NOT EXISTS bootstrap_tokens (
    token_hash text PRIMARY KEY,
    description text,
    one_time_use boolean NOT NULL DEFAULT false,
    used_at timestamptz,
    expires_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS bootstrap_tokens_expires_at_idx ON bootstrap_tokens (expires_at);
```

**Step 2: Write repository module**

Create `crates/chv-controlplane-store/src/bootstrap_tokens.rs`:
```rust
use crate::{StoreError, StorePool};
use chrono::Utc;

const VALIDATE_TOKEN_SQL: &str = r#"
SELECT
    token_hash,
    one_time_use,
    used_at,
    expires_at
FROM bootstrap_tokens
WHERE token_hash = $1
"#;

const MARK_USED_SQL: &str = r#"
UPDATE bootstrap_tokens SET
    used_at = now(),
    updated_at = now()
WHERE token_hash = $1
  AND used_at IS NULL
"#;

#[derive(Clone)]
pub struct BootstrapTokenRepository {
    pool: StorePool,
}

impl BootstrapTokenRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub async fn validate_and_consume(
        &self,
        token: &str,
    ) -> Result<BootstrapTokenValidation, StoreError> {
        let token_hash = sha256(token);

        let row = sqlx::query_as::<_, BootstrapTokenRow>(VALIDATE_TOKEN_SQL)
            .bind(&token_hash)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            None => Ok(BootstrapTokenValidation::Invalid),
            Some(row) => {
                if let Some(expires_at) = row.expires_at {
                    if expires_at < Utc::now() {
                        return Ok(BootstrapTokenValidation::Expired);
                    }
                }
                if row.one_time_use && row.used_at.is_some() {
                    return Ok(BootstrapTokenValidation::AlreadyUsed);
                }
                if row.one_time_use {
                    sqlx::query(MARK_USED_SQL)
                        .bind(&token_hash)
                        .execute(&self.pool)
                        .await?;
                }
                Ok(BootstrapTokenValidation::Valid)
            }
        }
    }
}

fn sha256(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(sqlx::FromRow)]
struct BootstrapTokenRow {
    token_hash: String,
    one_time_use: bool,
    used_at: Option<chrono::DateTime<chrono::Utc>>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BootstrapTokenValidation {
    Valid,
    Invalid,
    Expired,
    AlreadyUsed,
}
```

**Step 3: Wire into store lib**

Modify `crates/chv-controlplane-store/src/lib.rs`:
- Add `mod bootstrap_tokens;`
- Add `pub use bootstrap_tokens::{BootstrapTokenRepository, BootstrapTokenValidation};`

**Step 4: Update Cargo.toml dependencies**

Modify `crates/chv-controlplane-store/Cargo.toml` to add:
```toml
sha2 = "0.10"
hex = "0.4"
chrono = { version = "0.4", default-features = false, features = ["clock"] }
```

**Step 5: Write integration test**

Add to `crates/chv-controlplane-store/src/tests.rs`:
```rust
#[tokio::test]
async fn test_bootstrap_token_validation() {
    let test_db = TestDb::new().await;
    let pool = test_db.pool.clone();
    let repo = BootstrapTokenRepository::new(pool);

    let hash = "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"; // sha256("123")
    sqlx::query("INSERT INTO bootstrap_tokens (token_hash, one_time_use) VALUES ($1, true)")
        .bind(hash)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(repo.validate_and_consume("123").await.unwrap(), BootstrapTokenValidation::Valid);
    assert_eq!(repo.validate_and_consume("123").await.unwrap(), BootstrapTokenValidation::AlreadyUsed);
    assert_eq!(repo.validate_and_consume("999").await.unwrap(), BootstrapTokenValidation::Invalid);
}
```

**Step 6: Run test**

Run:
```bash
cargo test -p chv-controlplane-store test_bootstrap_token_validation -- --nocapture
```
Expected: PASS

**Step 7: Commit**

Run:
```bash
git add -A && git commit -m "feat(store): bootstrap token repository with hash validation"
```

---

### Task 3: Enforce bootstrap token validation in enrollment

**Files:**
- Modify: `crates/chv-controlplane-service/src/enrollment.rs`
- Modify: `crates/chv-controlplane-service/src/container.rs`
- Modify: `cmd/chv-controlplane/src/bootstrap.rs`

**Step 1: Inject BootstrapTokenRepository into EnrollmentServiceImplementation**

Modify `crates/chv-controlplane-service/src/enrollment.rs`:
- Add `token_repo: BootstrapTokenRepository` field to `EnrollmentServiceImplementation`.
- In `enroll_node`, replace `if request.bootstrap_token.is_empty()` with:
```rust
match self.token_repo.validate_and_consume(&request.bootstrap_token).await? {
    BootstrapTokenValidation::Valid => {}
    _ => return Err(ControlPlaneServiceError::Unauthorized("invalid bootstrap token".into())),
}
```

**Step 2: Update container and bootstrap wiring**

Modify `crates/chv-controlplane-service/src/container.rs`:
- Add `token_repo` to `new()` and store it.

Modify `cmd/chv-controlplane/src/bootstrap.rs`:
- Build `BootstrapTokenRepository::new(pool.clone())` and pass it to `EnrollmentServiceImplementation::new`.

**Step 3: Update service tests to seed a token**

Modify `crates/chv-controlplane-service/src/tests.rs`:
- In `test_enrollment_extended_inventory_persistence`, insert a bootstrap token before calling `enroll_node`.

**Step 4: Run tests**

Run:
```bash
cargo test -p chv-controlplane-service test_enrollment -- --nocapture
```
Expected: PASS

**Step 5: Commit**

Run:
```bash
git add -A && git commit -m "feat(enrollment): validate bootstrap tokens against postgres"
```

---

### Task 4: Refactor network desired state schema (add network_exposures table)

**Files:**
- Create: `cmd/chv-controlplane/migrations/0004_network_exposures.sql`
- Create: `crates/chv-controlplane-store/src/network_exposures.rs`
- Modify: `crates/chv-controlplane-store/src/lib.rs`
- Modify: `crates/chv-controlplane-store/src/desired_state.rs`
- Modify: `cmd/chv-controlplane/migrations/0001_initial.sql` (remove exposure columns from network_desired_state)

**Step 1: Write migration**

Create `cmd/chv-controlplane/migrations/0004_network_exposures.sql`:
```sql
ALTER TABLE network_desired_state
    DROP COLUMN IF EXISTS service_name,
    DROP COLUMN IF EXISTS protocol,
    DROP COLUMN IF EXISTS listen_address,
    DROP COLUMN IF EXISTS listen_port,
    DROP COLUMN IF EXISTS target_address,
    DROP COLUMN IF EXISTS target_port,
    DROP COLUMN IF EXISTS exposure_policy;

CREATE TABLE IF NOT EXISTS network_exposures (
    network_exposure_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    network_id text NOT NULL REFERENCES networks (network_id) ON DELETE CASCADE,
    service_name text NOT NULL,
    protocol text NOT NULL,
    listen_address inet,
    listen_port integer,
    target_address inet,
    target_port integer,
    exposure_policy text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS network_exposures_network_id_idx ON network_exposures (network_id);
```

**Step 2: Write repository**

Create `crates/chv-controlplane-store/src/network_exposures.rs`:
```rust
use crate::{StoreError, StorePool};
use chv_controlplane_types::domain::ResourceId;

const UPSERT_SQL: &str = r#"
INSERT INTO network_exposures (
    network_id, service_name, protocol, listen_address, listen_port,
    target_address, target_port, exposure_policy, updated_at
)
VALUES ($1, $2, $3, $4::inet, $5, $6::inet, $7, $8, to_timestamp($9 / 1000.0))
ON CONFLICT (network_id, service_name) DO UPDATE SET
    protocol = EXCLUDED.protocol,
    listen_address = EXCLUDED.listen_address,
    listen_port = EXCLUDED.listen_port,
    target_address = EXCLUDED.target_address,
    target_port = EXCLUDED.target_port,
    exposure_policy = EXCLUDED.exposure_policy,
    updated_at = EXCLUDED.updated_at
"#;

#[derive(Clone)]
pub struct NetworkExposureRepository {
    pool: StorePool,
}

impl NetworkExposureRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, input: &NetworkExposureInput) -> Result<(), StoreError> {
        sqlx::query(UPSERT_SQL)
            .bind(input.network_id.as_str())
            .bind(&input.service_name)
            .bind(&input.protocol)
            .bind(&input.listen_address)
            .bind(input.listen_port)
            .bind(&input.target_address)
            .bind(input.target_port)
            .bind(&input.exposure_policy)
            .bind(input.updated_unix_ms)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct NetworkExposureInput {
    pub network_id: ResourceId,
    pub service_name: String,
    pub protocol: String,
    pub listen_address: Option<String>,
    pub listen_port: Option<i32>,
    pub target_address: Option<String>,
    pub target_port: Option<i32>,
    pub exposure_policy: Option<String>,
    pub updated_unix_ms: i64,
}
```

**Step 3: Wire into store lib**

Modify `crates/chv-controlplane-store/src/lib.rs`:
- Add `mod network_exposures;`
- Add `pub use network_exposures::{NetworkExposureInput, NetworkExposureRepository};`

**Step 4: Update desired_state.rs to drop exposure fields**

Modify `crates/chv-controlplane-store/src/desired_state.rs`:
- Remove `service_name`, `protocol`, `listen_address`, `listen_port`, `target_address`, `target_port`, `exposure_policy` from `NetworkDesiredStateInput` and `UPSERT_NETWORK_DESIRED_STATE_SQL`.
- Update `upsert_network` method accordingly.

**Step 5: Run store tests**

Run:
```bash
cargo test -p chv-controlplane-store -- --nocapture
```
Expected: PASS (tests that use `NetworkDesiredStateInput` may need minor updates).

**Step 6: Commit**

Run:
```bash
git add -A && git commit -m "feat(store): split network_exposures into separate table"
```

---

### Task 5: Add strict DesiredStateFragment JSON mappers

**Files:**
- Create: `crates/chv-controlplane-types/src/fragment.rs`
- Modify: `crates/chv-controlplane-types/src/lib.rs`

**Step 1: Write fragment types**

Create `crates/chv-controlplane-types/src/fragment.rs`:
```rust
use serde::Deserialize;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct VmSpec {
    pub cpu_count: Option<i32>,
    pub memory_bytes: Option<i64>,
    pub image_ref: Option<String>,
    pub boot_mode: Option<String>,
    pub desired_power_state: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct VolumeSpec {
    pub capacity_bytes: i64,
    pub volume_kind: Option<String>,
    pub storage_class: Option<String>,
    pub attached_vm_id: Option<String>,
    pub attachment_mode: Option<String>,
    pub device_name: Option<String>,
    pub read_only: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct NetworkSpec {
    pub network_class: Option<String>,
    pub exposures: Option<Vec<NetworkExposureSpec>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct NetworkExposureSpec {
    pub service_name: String,
    pub protocol: String,
    pub listen_address: Option<String>,
    pub listen_port: Option<i32>,
    pub target_address: Option<String>,
    pub target_port: Option<i32>,
    pub exposure_policy: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct NodeSpec {
    pub desired_state: String,
    pub state_reason: Option<String>,
}
```

**Step 2: Export from lib**

Modify `crates/chv-controlplane-types/src/lib.rs`:
- Add `pub mod fragment;`
- Add `pub use fragment::{NodeSpec, NetworkExposureSpec, NetworkSpec, VmSpec, VolumeSpec};`

**Step 3: Write unit tests**

Add to `crates/chv-controlplane-types/src/fragment.rs` (in `#[cfg(test)] mod tests`):
```rust
#[test]
fn vm_spec_parses_minimal() {
    let json = r#"{"cpu_count": 2, "memory_bytes": 4294967296}"#;
    let spec: VmSpec = serde_json::from_str(json).unwrap();
    assert_eq!(spec.cpu_count, Some(2));
    assert_eq!(spec.memory_bytes, Some(4294967296));
}

#[test]
fn network_spec_parses_with_exposures() {
    let json = r#"{"network_class": "bridge", "exposures": [{"service_name": "web", "protocol": "tcp"}]}"#;
    let spec: NetworkSpec = serde_json::from_str(json).unwrap();
    assert_eq!(spec.exposures.as_ref().unwrap()[0].service_name, "web");
}
```

**Step 4: Run tests**

Run:
```bash
cargo test -p chv-controlplane-types -- --nocapture
```
Expected: PASS

**Step 5: Commit**

Run:
```bash
git add -A && git commit -m "feat(types): strict DesiredStateFragment spec parsers"
```

---

### Task 6: Implement ReconcileService

**Files:**
- Create: `crates/chv-controlplane-service/src/reconcile.rs`
- Modify: `crates/chv-controlplane-service/src/lib.rs`
- Modify: `crates/chv-controlplane-service/src/server.rs`
- Modify: `crates/chv-controlplane-service/src/container.rs`
- Modify: `cmd/chv-controlplane/src/bootstrap.rs`

**Step 1: Write service trait and implementation**

Create `crates/chv-controlplane-service/src/reconcile.rs` (full trait + impl for all 5 RPCs). Key logic:
- Parse `generation` with `Generation::from_str`.
- Parse `spec_json` into `VmSpec`/`VolumeSpec`/`NetworkSpec`/`NodeSpec`.
- Call `DesiredStateRepository::upsert_vm/volume/network` or `NodeRepository::upsert_state`.
- For networks with `exposures`, call `NetworkExposureRepository::upsert` for each exposure.
- Emit `EventType::DesiredStateApplied` or `DesiredStateRejected`.

**Step 2: Add tonic server wrappers**

Modify `crates/chv-controlplane-service/src/server.rs`:
- Add `ReconcileServer` struct implementing `proto::reconcile_service_server::ReconcileService`.

**Step 3: Wire into container and bootstrap**

Modify `crates/chv-controlplane-service/src/container.rs` and `cmd/chv-controlplane/src/bootstrap.rs`:
- Add `reconcile_service` field and wiring.

**Step 4: Add service integration test**

Add to `crates/chv-controlplane-service/src/tests.rs`:
- Test `ApplyVmDesiredState` writes to `vm_desired_state`.
- Test invalid generation returns `InvalidArgument`.
- Test `ApplyNetworkDesiredState` with exposures writes to `network_exposures`.

**Step 5: Run tests**

Run:
```bash
cargo test -p chv-controlplane-service test_apply -- --nocapture
```
Expected: PASS

**Step 6: Commit**

Run:
```bash
git add -A && git commit -m "feat(service): implement ReconcileService with fragment mappers"
```

---

### Task 7: Implement LifecycleService

**Files:**
- Create: `crates/chv-controlplane-service/src/lifecycle.rs`
- Modify: `crates/chv-controlplane-service/src/lib.rs`
- Modify: `crates/chv-controlplane-service/src/server.rs`
- Modify: `crates/chv-controlplane-service/src/container.rs`
- Modify: `cmd/chv-controlplane/src/bootstrap.rs`

**Step 1: Write service trait and implementation**

Create `crates/chv-controlplane-service/src/lifecycle.rs`:
- For each RPC (`CreateVm`, `StartVm`, `StopVm`, `DeleteVm`, `AttachVolume`, `DetachVolume`, `ResizeVolume`, node management):
  - Validate node exists.
  - Create idempotency key = `format!("{}:{}:{}:{}", op_type, node_id, resource_id, generation)`.
  - Call `OperationRepository::create_or_get`.
  - Emit `EventType::OperationStarted`.
  - For node commands (`DrainNode`, `EnterMaintenance`, etc.), also update `node_desired_state`.
  - For `CreateVm`, also upsert `vm_desired_state`.

**Step 2: Add tonic server wrappers**

Modify `crates/chv-controlplane-service/src/server.rs`:
- Add `LifecycleServer`.

**Step 3: Wire into container and bootstrap**

Same pattern as Task 6.

**Step 4: Add integration tests**

Add to `crates/chv-controlplane-service/src/tests.rs`:
- `test_create_vm_creates_operation` — verifies `operations` row with `Pending`.
- `test_duplicate_idempotency_returns_same_operation`.
- `test_drain_node_updates_desired_state`.

**Step 5: Run tests**

Run:
```bash
cargo test -p chv-controlplane-service test_create_vm test_duplicate test_drain -- --nocapture
```
Expected: PASS

**Step 6: Commit**

Run:
```bash
git add -A && git commit -m "feat(service): implement LifecycleService with operation journal"
```

---

### Task 8: Fix error-to-gRPC-status mapping

**Files:**
- Modify: `crates/chv-controlplane-service/src/error.rs`
- Modify: `crates/chv-controlplane-service/src/server.rs`

**Step 1: Expand error variants**

Modify `crates/chv-controlplane-service/src/error.rs`:
```rust
#[derive(Debug, Error)]
pub enum ControlPlaneServiceError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("stale generation: expected {expected}, received {received}")]
    StaleGeneration { expected: String, received: String },
    #[error("internal error: {0}")]
    Internal(String),
    #[error("store error: {0}")]
    Store(#[from] chv_controlplane_store::StoreError),
}

impl From<ControlPlaneServiceError> for tonic::Status {
    fn from(err: ControlPlaneServiceError) -> Self {
        match err {
            ControlPlaneServiceError::NotFound(msg) => tonic::Status::not_found(msg),
            ControlPlaneServiceError::InvalidArgument(msg) => tonic::Status::invalid_argument(msg),
            ControlPlaneServiceError::Unauthorized(msg) => tonic::Status::unauthenticated(msg),
            ControlPlaneServiceError::Conflict(msg) => tonic::Status::already_exists(msg),
            ControlPlaneServiceError::StaleGeneration { expected, received } => {
                tonic::Status::failed_precondition(format!(
                    "stale generation: expected {expected}, received {received}"
                ))
            }
            _ => tonic::Status::internal(err.to_string()),
        }
    }
}
```

**Step 2: Replace blanket internal mapping in server.rs**

In all server methods, replace:
```rust
.map_err(|e| Status::internal(e.to_string()))?
```
with:
```rust
.map_err(Into::into)?
```

**Step 3: Write unit test for mapper**

Add to `crates/chv-controlplane-service/src/tests.rs`:
```rust
#[test]
fn test_error_to_status_mapping() {
    use tonic::Status;
    let err = ControlPlaneServiceError::NotFound("node-x".into());
    let status: Status = err.into();
    assert_eq!(status.code(), tonic::Code::NotFound);

    let err = ControlPlaneServiceError::StaleGeneration { expected: "5".into(), received: "3".into() };
    let status: Status = err.into();
    assert_eq!(status.code(), tonic::Code::FailedPrecondition);
}
```

**Step 4: Run tests**

Run:
```bash
cargo test -p chv-controlplane-service test_error_to_status -- --nocapture
```
Expected: PASS

**Step 5: Commit**

Run:
```bash
git add -A && git commit -m "feat(service): structured error to tonic::Status mapping"
```

---

### Task 9: Add optional mTLS to gRPC server

**Files:**
- Modify: `crates/chv-config/src/lib.rs` (or `chv-controlplane-types/src/config.rs`)
- Modify: `cmd/chv-controlplane/src/bootstrap.rs`
- Modify: `crates/chv-controlplane-service/src/container.rs`

**Step 1: Add TLS config fields**

Modify `crates/chv-controlplane-types/src/config.rs` (or wherever control-plane config lives):
```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TlsConfig {
    pub ca_cert_path: Option<std::path::PathBuf>,
    pub ca_key_path: Option<std::path::PathBuf>,
    pub tls_cert_path: Option<std::path::PathBuf>,
    pub tls_key_path: Option<std::path::PathBuf>,
}
```

**Step 2: Load server TLS credentials in bootstrap**

Modify `cmd/chv-controlplane/src/bootstrap.rs`:
- If `config.tls.tls_cert_path` and `tls_key_path` are present, read PEMs and build `tonic::transport::ServerTlsConfig`.
- Pass an `Option<tonic::transport::ServerTlsConfig>` into `ControlPlaneRuntime`.

**Step 3: Apply TLS in ControlPlaneService::run**

Modify `crates/chv-controlplane-service/src/container.rs` (or wherever `run()` is):
```rust
let mut server = tonic::transport::Server::builder();
if let Some(tls_config) = &self.runtime.tls_config {
    server = server.tls_config(tls_config.clone()).map_err(|e| ...)?;
}
server
    .add_service(...)
    .serve(addr)
    .await
```

**Step 4: Run binary to verify plaintext still works**

Run:
```bash
cargo run -p chv-controlplane -- /dev/null 2>&1 | head -5 || true
```
Expected: Server starts (may error on missing config, but compiles).

**Step 5: Commit**

Run:
```bash
git add -A && git commit -m "feat(controlplane): optional mTLS for gRPC server"
```

---

### Task 10: Add axum HTTP admin server

**Files:**
- Create: `crates/chv-controlplane-service/src/api/mod.rs`
- Create: `crates/chv-controlplane-service/src/api/health.rs`
- Create: `crates/chv-controlplane-service/src/api/router.rs`
- Create: `crates/chv-controlplane-service/src/api/nodes.rs`
- Create: `crates/chv-controlplane-service/src/api/operations.rs`
- Modify: `crates/chv-controlplane-service/src/lib.rs`
- Modify: `crates/chv-controlplane-service/Cargo.toml`
- Modify: `cmd/chv-controlplane/src/bootstrap.rs`
- Modify: `cmd/chv-controlplane/src/config.rs`

**Step 1: Add axum to Cargo.toml**

Modify `crates/chv-controlplane-service/Cargo.toml`:
```toml
axum = { workspace = true }
tower = { workspace = true }
```

**Step 2: Write router and handlers**

Create `crates/chv-controlplane-service/src/api/mod.rs`:
```rust
pub mod health;
pub mod nodes;
pub mod operations;
pub mod router;
```

Create `crates/chv-controlplane-service/src/api/router.rs`:
```rust
use axum::{routing::get, Router};
use chv_controlplane_store::StorePool;
use std::sync::Arc;

pub fn admin_router(pool: StorePool) -> Router {
    Router::new()
        .route("/health", get(health::health_handler))
        .route("/ready", get(health::ready_handler))
        .route("/metrics", get(health::metrics_handler))
        .route("/admin/nodes", get(nodes::list_nodes))
        .route("/admin/nodes/:id", get(nodes::get_node))
        .route("/admin/operations", get(operations::list_operations))
        .route("/admin/operations/:id", get(operations::get_operation))
        .with_state(Arc::new(pool))
}
```

Create `crates/chv-controlplane-service/src/api/health.rs`:
```rust
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chv_controlplane_store::StorePool;
use std::sync::Arc;

pub async fn health_handler() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok"}))
}

pub async fn ready_handler(State(pool): State<Arc<StorePool>>) -> impl IntoResponse {
    let result = sqlx::query("SELECT 1").fetch_one(pool.as_ref()).await;
    match result {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"status": "not_ready"})),
        ),
    }
}

pub async fn metrics_handler() -> impl IntoResponse {
    // Prometheus exporter via chv-observability
    let mut buf = String::new();
    if let Err(e) = metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
        .and_then(|_| {
            metrics::prometheus::renderer().render(&mut buf);
            Ok(())
        })
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("metrics error: {e}"));
    }
    (StatusCode::OK, buf)
}
```

Create `crates/chv-controlplane-service/src/api/nodes.rs` and `operations.rs` with basic SQL queries returning JSON.

**Step 3: Spawn axum server in bootstrap**

Modify `cmd/chv-controlplane/src/bootstrap.rs`:
- Build `admin_router(pool.clone())`.
- Spawn `tokio::spawn(axum::serve(http_listener, router))`.

**Step 4: Add HTTP integration test**

Add to `crates/chv-controlplane-service/src/tests.rs`:
```rust
#[tokio::test]
async fn test_health_endpoint() {
    use axum::http::StatusCode;
    use tower::ServiceExt;

    let test_db = chv_controlplane_store::test_util::TestDb::new().await;
    let app = chv_controlplane_service::api::router::admin_router(test_db.pool.clone());

    let response = app
        .oneshot(axum::http::Request::get("/health").body(()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
```

**Step 5: Run tests**

Run:
```bash
cargo test -p chv-controlplane-service test_health_endpoint -- --nocapture
```
Expected: PASS

**Step 6: Commit**

Run:
```bash
git add -A && git commit -m "feat(api): axum admin HTTP server with health, ready, metrics"
```

---

### Task 11: Final integration verification

**Step 1: Run all workspace tests**

Run:
```bash
cargo test --workspace -- --nocapture
```
Expected: All tests pass.

**Step 2: Check formatting and clippy**

Run:
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```
Expected: Clean.

**Step 3: Commit any final fixes**

Run:
```bash
git add -A && git commit -m "chore: final formatting and clippy fixes for phase 1"
```

---

## Execution Handoff

**Plan complete and saved to `docs/plans/2026-04-14-rust-controlplane-phase1-implementation.md`.**

Two execution options:

1. **Subagent-Driven (this session)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Parallel Session (separate)** — Open a new session with `executing-plans`, batch execution with checkpoints.

Which approach do you prefer?
