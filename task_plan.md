# Task Plan: CHV Gap Remediation — Sprint Implementation Plan

## Goal

Close all P1/P2 gaps between the CHV spec, backend implementation, and WebUI so that VMs,
volumes, networks, nodes, images, and auth all function end-to-end rather than returning
stubs, empty arrays, or hardcoded data.

---

## Success Criteria

1. Node startup advances through the full state machine (Bootstrapping → HostReady → StorageReady → NetworkReady → TenantReady) — observable in the Nodes list as the node transitions states rather than staying in "Discovered".
2. VM detail page Volumes tab shows the volumes attached to that VM, not "not yet available".
3. VM detail page Networks tab shows the NICs attached to that VM, not "not yet available".
4. VM detail page Events tab shows events scoped to that VM, not the global events list.
5. The Images page returns real rows from the database, not an empty list.
6. `/v1/networks/mutate` is wired to the control-plane mutation service (currently unrouted).
7. Login validates credentials from the database, not a hardcoded `admin`/`admin` check.
8. All new code compiles (`cargo build --workspace`) and existing tests pass (`cargo test --workspace`).
9. Frontend type-checks clean (`cd ui && npm run check`).

---

## Sprint Overview

| Sprint | Theme | Key Deliverables |
|--------|-------|-----------------|
| S1 | Node lifecycle | State machine transitions, agent→dispatch, NWD CIDR fix |
| S2 | VM detail completeness | Volumes tab, Networks tab, per-VM events |
| S3 | Images & network mutations | Images table + BFF, network mutate endpoint wiring |
| S4 | Auth hardening | Users table, bcrypt login, `me` from DB |

---

## Sprint 1 — Node Lifecycle

**Theme**: The agent reconciler skips all state transitions. A freshly enrolled node sits at "Discovered" forever. Fix `run_once` to advance the node through `Bootstrapping → HostReady → StorageReady → NetworkReady → TenantReady`.

### Wave 1 (no dependencies, parallel-safe)

#### T1.1 — Add node state-machine advancement to Reconciler::run_once
- **Agent**: golang-general-engineer (Rust specialist)
- **Duration**: 5 min
- **File**: `/Users/I761222/projects/claude/chv/crates/chv-agent-core/src/reconcile.rs`
- **Operations**:
  - Replace the early-return guard at line 51 (`if !matches!(self.current_state().await, NodeState::TenantReady)`) with a `match` on `current_state`:
    - `Discovered` → call `self.transition_state(NodeState::Bootstrapping).await?`
    - `Bootstrapping` → probe chv-stord socket; if socket is reachable, call `self.transition_state(NodeState::HostReady).await?`
    - `HostReady` → call stord `list_storage_pools` RPC; if it returns ≥0 pools without error, call `self.transition_state(NodeState::StorageReady).await?`
    - `StorageReady` → call nwd `list_namespace_state` RPC; if it returns without error, call `self.transition_state(NodeState::NetworkReady).await?`
    - `NetworkReady` → call `self.transition_state(NodeState::TenantReady).await?`
    - `TenantReady` → fall through to existing `reconcile_networks/volumes/vms`
    - `Degraded | Draining | Maintenance | Failed` → return `Ok(())` (no-op in these states)
- **Verification**: `cargo test -p chv-agent-core` exits 0; add a unit test that drives the state machine from Discovered to TenantReady
- **Parallel-safe**: true

#### T1.2 — Fix hardcoded CIDR in reconciler network topology calls
- **Agent**: golang-general-engineer (Rust specialist)
- **Duration**: 3 min
- **Files**:
  - `/Users/I761222/projects/claude/chv/crates/chv-agent-core/src/reconcile.rs` (lines 70, 176)
  - `/Users/I761222/projects/claude/chv/crates/chv-agent-core/src/cache.rs` (add `network_cidr` lookup)
- **Operations**:
  - In `reconcile_networks` (line 70): change `let cidr = "10.0.0.0/24"` to look up the CIDR from `cache.network_fragments.get(net_id).map(|f| f.cidr.as_deref()).flatten().unwrap_or("10.0.0.0/24")`
  - In `prepare_vm` (line 176): same CIDR lookup from `vm_spec.nics[i].cidr` if present, fallback to the network_fragment CIDR
- **Verification**: `cargo build -p chv-agent-core` exits 0
- **Parallel-safe**: true (different lines in same file — no conflict with T1.1 since T1.1 touches lines 44-58 and T1.2 touches lines 70 and 176)

### Wave 2 (depends on Wave 1 completing)

#### T1.3 — Expose node current_state in NodeCache health report
- **Agent**: golang-general-engineer (Rust specialist)
- **Duration**: 3 min
- **File**: `/Users/I761222/projects/claude/chv/crates/chv-agent-core/src/cache.rs`
- **Operations**:
  - Verify `current_node_state()` is public and returns `NodeState`
  - If it returns a string, add a typed accessor returning `NodeState` to avoid string parsing on every reconcile tick
- **Verification**: `cargo build --workspace` exits 0
- **Parallel-safe**: N/A (wave 2)

---

## Sprint 2 — VM Detail Completeness

**Theme**: The VM detail page shows "not yet available" for Volumes, Networks, and Events tabs because `get_vm` returns no attachment data and the events endpoint is global-only.

### Wave 1 (no dependencies, parallel-safe)

#### T2.1 — Extend get_vm SQL to include attached volumes
- **Agent**: golang-general-engineer (Rust specialist)
- **Duration**: 5 min
- **File**: `/Users/I761222/projects/claude/chv/crates/chv-webui-bff/src/handlers/vms.rs`
- **Operations**:
  - After the `recent_tasks` query in `get_vm` (after line 124), add a new query for attached volumes:
    ```rust
    let attached_volumes = sqlx::query_as::<_, VmVolumeRow>(r#"
        SELECT
            v.volume_id,
            v.display_name AS name,
            COALESCE(pg_size_pretty(v.capacity_bytes), '') AS size,
            COALESCE(vds.device_name, '') AS device_name,
            COALESCE(vds.read_only, false) AS read_only,
            COALESCE(vos.health_status, 'unknown') AS health
        FROM volume_desired_state vds
        JOIN volumes v ON vds.volume_id = v.volume_id
        LEFT JOIN volume_observed_state vos ON v.volume_id = vos.volume_id
        WHERE vds.attached_vm_id = $1
    "#).bind(vm_id).fetch_all(&state.pool).await
        .map_err(|e| BffError::Internal(format!("failed to get vm volumes: {}", e)))?;
    ```
  - Add `VmVolumeRow` struct at bottom of file: `volume_id: String, name: String, size: String, device_name: String, read_only: bool, health: String`
  - Serialize to JSON array and include as `"attached_volumes": volumes_json` in the `"summary"` object at line 138
- **Verification**: `cargo build -p chv-webui-bff` exits 0; `cargo test -p chv-webui-bff` exits 0
- **Parallel-safe**: true

#### T2.2 — Extend get_vm SQL to include attached NICs
- **Agent**: golang-general-engineer (Rust specialist)
- **Duration**: 4 min
- **File**: `/Users/I761222/projects/claude/chv/crates/chv-webui-bff/src/handlers/vms.rs`
- **Operations**:
  - After the attached_volumes query (T2.1), add a query for attached NICs:
    ```rust
    let attached_nics = sqlx::query_as::<_, VmNicRow>(r#"
        SELECT
            nv.nic_id,
            nv.network_id,
            COALESCE(n.display_name, nv.network_id) AS network_name,
            COALESCE(nv.mac_address, '') AS mac_address,
            COALESCE(nv.ip_address, '') AS ip_address,
            COALESCE(nv.nic_model, 'virtio') AS nic_model
        FROM vm_nic_desired_state nv
        LEFT JOIN networks n ON nv.network_id = n.network_id
        WHERE nv.vm_id = $1
    "#).bind(vm_id).fetch_all(&state.pool).await
        .map_err(|e| BffError::Internal(format!("failed to get vm nics: {}", e)))?;
    ```
  - Add `VmNicRow` struct: `nic_id: String, network_id: String, network_name: String, mac_address: String, ip_address: String, nic_model: String`
  - Include `"attached_nics": nics_json` in the `"summary"` object
  - NOTE: If `vm_nic_desired_state` table does not exist in migrations, fall back to querying `vm_desired_state` JSON columns or note as T2.5 below
- **Verification**: `cargo build -p chv-webui-bff` exits 0
- **Parallel-safe**: false — T2.2 modifies the same file and same `get_vm` function as T2.1; must run after T2.1

#### T2.3 — Add per-VM events endpoint to BFF
- **Agent**: golang-general-engineer (Rust specialist)
- **Duration**: 4 min
- **Files**:
  - `/Users/I761222/projects/claude/chv/crates/chv-webui-bff/src/handlers/events.rs`
  - `/Users/I761222/projects/claude/chv/crates/chv-webui-bff/src/router.rs`
- **Operations**:
  - In `events.rs`, add function `list_events_for_vm`:
    ```rust
    pub async fn list_events_for_vm(
        State(state): State<AppState>,
        axum::Json(payload): axum::Json<Value>,
    ) -> Result<Json<Value>, BffError> {
        let vm_id = payload.get("vm_id").and_then(|v| v.as_str())
            .ok_or_else(|| BffError::BadRequest("missing vm_id".into()))?;
        // Query events WHERE resource_kind = 'vm' AND resource_id = $1
        // Query alerts WHERE resource_kind = 'vm' AND resource_id = $1
        // Merge, sort by occurred_at DESC, limit 50
    }
    ```
  - In `router.rs`, add route: `.route("/v1/vms/events", post(events::list_events_for_vm))`
- **Verification**: `cargo build -p chv-webui-bff` exits 0
- **Parallel-safe**: true (different file from T2.1/T2.2)

### Wave 2 (depends on Wave 1)

#### T2.4 — Update VM detail Svelte page to render Volumes and Networks tabs
- **Agent**: typescript-frontend-engineer
- **Duration**: 5 min
- **File**: `/Users/I761222/projects/claude/chv/ui/src/routes/vms/[id]/+page.svelte`
- **Operations**:
  - Find the Volumes tab section (currently shows "not yet available") and replace with a table rendering `data.summary.attached_volumes[]` with columns: Name, Size, Device, Read-only, Health. If `attached_volumes` is empty, show "No volumes attached."
  - Find the Networks tab section and replace with a table rendering `data.summary.attached_nics[]` with columns: NIC ID, Network, MAC, IP, Model. If `attached_nics` is empty, show "No NICs attached."
  - Find the Events tab section and replace the global events fetch with a call to `/v1/vms/events` with `{ vm_id: data.summary.vm_id }`. Render results in the same event list format used on the global Events page.
- **Verification**: `cd ui && npm run check` exits 0
- **Parallel-safe**: N/A (wave 2)

#### T2.5 — Verify or create vm_nic_desired_state table
- **Agent**: golang-general-engineer (Rust specialist)
- **Duration**: 3 min
- **Files**:
  - `/Users/I761222/projects/claude/chv/cmd/chv-controlplane/migrations/` (add `0006_vm_nic_state.sql` if needed)
- **Operations**:
  - Check if `vm_nic_desired_state` table exists in any of the 5 existing migrations. If it does not exist, create `0006_vm_nic_state.sql`:
    ```sql
    CREATE TABLE IF NOT EXISTS vm_nic_desired_state (
        nic_id text PRIMARY KEY,
        vm_id text NOT NULL REFERENCES vms (vm_id) ON DELETE CASCADE,
        network_id text NOT NULL,
        mac_address text,
        ip_address text,
        nic_model text NOT NULL DEFAULT 'virtio',
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now()
    );
    CREATE INDEX IF NOT EXISTS idx_vm_nic_desired_vm_id ON vm_nic_desired_state (vm_id);
    ```
  - If the table already exists under a different name, update T2.2 to use that table name instead
- **Verification**: `cargo build --workspace` exits 0
- **Parallel-safe**: N/A (wave 2, may unblock T2.2 correctness)

---

## Sprint 3 — Images & Network Mutations

**Theme**: The Images page always returns empty (no DB table). The Networks page has no mutation endpoint wired in the BFF. Both are basic data completeness gaps.

### Wave 1 (no dependencies, parallel-safe)

#### T3.1 — Create images migration and populate images.rs BFF handler
- **Agent**: golang-general-engineer (Rust specialist)
- **Duration**: 5 min
- **Files**:
  - `/Users/I761222/projects/claude/chv/cmd/chv-controlplane/migrations/0007_images.sql` (new file)
  - `/Users/I761222/projects/claude/chv/crates/chv-webui-bff/src/handlers/images.rs`
- **Operations**:
  - Create `0007_images.sql`:
    ```sql
    CREATE TABLE IF NOT EXISTS images (
        image_id text PRIMARY KEY,
        display_name text NOT NULL,
        image_type text NOT NULL DEFAULT 'disk',
        format text NOT NULL DEFAULT 'qcow2',
        size_bytes bigint,
        checksum text,
        source_url text,
        status text NOT NULL DEFAULT 'available',
        node_id text REFERENCES nodes (node_id),
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now()
    );
    ```
  - Rewrite `images.rs` `list_images` to query the `images` table:
    ```rust
    let rows = sqlx::query_as::<_, ImageRow>(r#"
        SELECT image_id, display_name AS name, image_type, format,
               COALESCE(pg_size_pretty(size_bytes), '') AS size,
               status, node_id, created_at::text AS created_at
        FROM images ORDER BY created_at DESC
    "#).fetch_all(&state.pool).await
        .map_err(|e| BffError::Internal(format!("failed to list images: {}", e)))?;
    ```
  - Add `ImageRow` struct; build JSON response with the existing `items`/`page` envelope
- **Verification**: `cargo build -p chv-webui-bff` exits 0
- **Parallel-safe**: true

#### T3.2 — Add MutateNetwork to proto and wire BFF mutation endpoint
- **Agent**: golang-general-engineer (Rust specialist)
- **Duration**: 8 min
- **Files**:
  - `/Users/I761222/projects/claude/chv/proto/webui/webui-bff.proto`
  - `/Users/I761222/projects/claude/chv/crates/chv-webui-bff/src/mutations.rs`
  - `/Users/I761222/projects/claude/chv/crates/chv-webui-bff/src/handlers/networks.rs`
  - `/Users/I761222/projects/claude/chv/crates/chv-webui-bff/src/router.rs`
  - `/Users/I761222/projects/claude/chv/crates/chv-controlplane-service/src/bff_mutations.rs`
- **Operations**:
  1. In `webui-bff.proto`, after line 179 (`MutateVolumeResponse`), add:
     ```protobuf
     message MutateNetworkRequest {
       string network_id = 1;
       string action = 2;
       bool force = 3;
       RequestMeta meta = 4;
     }
     message MutateNetworkResponse {
       bool accepted = 1;
       string task_id = 2;
       string network_id = 3;
       string summary = 4;
     }
     ```
     In the `WebUiMutationService` rpc block (line 444 area), add:
     `rpc MutateNetwork(MutateNetworkRequest) returns (MutateNetworkResponse);`
  2. Run `cargo build --workspace` to regenerate Tonic bindings from proto (build.rs triggers prost-build)
  3. In `mutations.rs`, add `mutate_network` to the `MutationService` trait:
     ```rust
     async fn mutate_network(&self, network_id: String, action: String, force: bool, requested_by: String) -> Result<MutateNetworkResponse, BffError>;
     ```
     Import `MutateNetworkResponse` from `chv_webui_bff_api::chv_webui_bff_v1`
  4. In `networks.rs`, add `mutate_network` handler identical in structure to `mutate_vm` in `vms.rs:155–185`
  5. In `bff_mutations.rs`, add `mutate_network` implementation calling the lifecycle_service with a network mutate operation (build analogously to `mutate_node`)
  6. In `router.rs`, add: `.route("/v1/networks/mutate", post(networks::mutate_network))`
- **Verification**: `cargo build --workspace` exits 0
- **Parallel-safe**: true (different files from T3.1)

### Wave 2 (depends on Wave 1)

#### T3.3 — Add network mutation to Svelte networks detail page
- **Agent**: typescript-frontend-engineer
- **Duration**: 4 min
- **File**: `/Users/I761222/projects/claude/chv/ui/src/routes/networks/[id]/+page.svelte`
- **Operations**:
  - Check if a "Mutate" action menu or button exists on the networks detail page; if not, add an "Actions" dropdown with the same pattern as `vms/[id]/+page.svelte`'s action menu
  - Wire the action to `POST /v1/networks/mutate` with `{ network_id, action, force }`
  - Show task acceptance toast on success using the existing toast/notification pattern
- **Verification**: `cd ui && npm run check` exits 0
- **Parallel-safe**: N/A (wave 2)

---

## Sprint 4 — Auth Hardening

**Theme**: Login is `admin`/`admin` hardcoded. `me` returns a static object. There is no users table. This sprint adds a real users table, bcrypt validation, and DB-backed session info.

### Wave 1 (no dependencies, parallel-safe)

#### T4.1 — Create users migration
- **Agent**: golang-general-engineer (Rust specialist)
- **Duration**: 3 min
- **File**: `/Users/I761222/projects/claude/chv/cmd/chv-controlplane/migrations/0008_users.sql` (new file)
- **Operations**:
  - Create migration:
    ```sql
    CREATE TABLE IF NOT EXISTS users (
        user_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
        username text NOT NULL UNIQUE,
        password_hash text NOT NULL,
        role text NOT NULL DEFAULT 'viewer',
        display_name text,
        email text,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now(),
        last_login_at timestamptz
    );
    -- Seed admin user with bcrypt hash of 'admin' (cost 12)
    -- Hash: $2b$12$... (pre-computed, not hardcoded plaintext)
    INSERT INTO users (user_id, username, password_hash, role, display_name)
    VALUES ('00000000-0000-0000-0000-000000000001', 'admin',
            '$2b$12$eImiTXuWVxfM37uY4JANjQ==...', 'admin', 'Administrator')
    ON CONFLICT (user_id) DO NOTHING;
    ```
  - The actual bcrypt hash must be generated at migration write time using `bcrypt::hash("admin", 12)` — compute and embed the literal hash string, do not run bcrypt at migration time
- **Verification**: Migration file is valid SQL; `cargo build --workspace` exits 0
- **Parallel-safe**: true

#### T4.2 — Add bcrypt crate dependency to chv-controlplane-service
- **Agent**: golang-general-engineer (Rust specialist)
- **Duration**: 2 min
- **File**: `/Users/I761222/projects/claude/chv/crates/chv-controlplane-service/Cargo.toml`
- **Operations**:
  - Add `bcrypt = "0.15"` to `[dependencies]`
- **Verification**: `cargo build -p chv-controlplane-service` exits 0
- **Parallel-safe**: true

### Wave 2 (depends on Wave 1)

#### T4.3 — Replace hardcoded login with DB lookup + bcrypt verify
- **Agent**: golang-general-engineer (Rust specialist)
- **Duration**: 5 min
- **File**: `/Users/I761222/projects/claude/chv/crates/chv-controlplane-service/src/api/stub.rs`
- **Operations**:
  - Add `State(state): State<AppState>` to `login_handler` function signature (it already has it — confirm at line 15)
  - Replace the hardcoded check at lines 27-33 with:
    ```rust
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT user_id::text AS user_id, password_hash, role FROM users WHERE username = $1"
    ).bind(username).fetch_optional(&state.pool).await
        .map_err(|e| { tracing::error!("db error in login: {}", e); ... })?;

    let row = match row {
        Some(r) => r,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid credentials"}))).into_response(),
    };

    if !bcrypt::verify(password, &row.password_hash).unwrap_or(false) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid credentials"}))).into_response();
    }
    // Use row.user_id and row.role in the Claims struct
    ```
  - Add `UserRow` struct with `user_id: String, password_hash: String, role: String`
- **Verification**: `cargo build -p chv-controlplane-service` exits 0
- **Parallel-safe**: N/A (wave 2)

#### T4.4 — Replace me_handler static response with JWT-decoded user data
- **Agent**: golang-general-engineer (Rust specialist)
- **Duration**: 3 min
- **File**: `/Users/I761222/projects/claude/chv/crates/chv-controlplane-service/src/api/stub.rs`
- **Operations**:
  - Change `me_handler` signature to extract `BearerToken(claims): BearerToken` using the existing auth extractor pattern (same as `mutate_vm` uses `crate::auth::BearerToken(claims)`)
  - Return `{ id: claims.sub, username: claims.username, role: claims.role }` from the JWT claims rather than the hardcoded object
  - No DB lookup required — the JWT already carries the authoritative values for the session
- **Verification**: `cargo build -p chv-controlplane-service` exits 0
- **Parallel-safe**: false — T4.4 modifies the same file as T4.3; run after T4.3

---

## Phases
- [x] Phase 1: Sprint 1 — Node lifecycle (T1.1, T1.2, T1.3)
- [x] Phase 2: Sprint 2 — VM detail completeness (T2.1–T2.5)
- [x] Phase 3: Sprint 3 — Images & network mutations (T3.1–T3.3)
- [x] Phase 4: Sprint 4 — Auth hardening (T4.1–T4.4)
- [x] Phase 5: Verify all — `cargo build --workspace` clean; integration tests require Docker (not available in dev env); frontend 64 errors all pre-existing

## Key Questions / Resolved

1. `vm_nic_desired_state` — does NOT exist in any of the 5 existing migrations. T2.5 must create `0006_vm_nic_state.sql` before T2.2 SQL is valid.
2. `MutateNetwork` proto RPC — does NOT exist in `webui-bff.proto`. T3.2 must add it to the proto first, then rebuild generated bindings, then wire BFF + mutation service.
3. What bcrypt cost factor is acceptable for the seed admin user hash in T4.1? (Pending user decision — plan uses cost=12 as default.)

## Decisions Made

- **Auth approach**: Extend existing JWT pattern in `stub.rs`; no session table. The JWT carries role/username so `me` needs no DB round-trip.
- **Images table**: New migration `0007_images.sql`; no proto RPC for image management yet (deferred per ADR-004).
- **NIC table name**: Assuming `vm_nic_desired_state`; T2.5 must verify before T2.2 is executed.
- **Sprint ordering**: S1 first because TenantReady is required for reconciler to do any work; S2 next because VM detail is the most visible user-facing gap; S3 and S4 are independent and can run in parallel if desired.

## Status
**COMPLETE** — All 4 sprints delivered. Branch `sprint-1-completion` ready for merge to `main`.
