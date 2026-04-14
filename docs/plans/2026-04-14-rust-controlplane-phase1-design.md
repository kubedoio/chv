# Rust Control Plane — Phase 1 Design

## Status
Approved for implementation

## Decisions made
- Strict JSON-to-SQL mapper for `DesiredStateFragment` (parse `spec_json` into typed structs, write to individual columns).
- Phase 1 is **inbound-only** — persist desired state and lifecycle commands, no outbound `chv-agent` client yet.
- Mandatory separate HTTP port (`http_bind`) for `axum` admin / health / metrics.
- Split `network_desired_state` into topology table + `network_exposures` table (1:N relationship).
- Delete `legacy/go-controlplane/` entirely as part of this work.
- Bootstrap token validation uses a PostgreSQL `bootstrap_tokens` table (hashed tokens, expiration, optional one-time-use).
- gRPC server supports **optional mTLS** via config (`tls_cert_path`, `tls_key_path`), falling back to plaintext if absent.

---

## Component Boundaries

### Control plane binary (`cmd/chv-controlplane`)
- Starts PostgreSQL pool and runs migrations.
- Loads CA cert/key for enrollment.
- Builds `ControlPlaneComponents` (enrollment, inventory, telemetry, reconcile, lifecycle).
- Spawns two Tokio tasks:
  1. `tonic::transport::Server` (gRPC)
  2. `axum::serve` (HTTP admin)

### Crate responsibilities
| Crate | Responsibility |
|-------|----------------|
| `chv-controlplane-types` | Domain primitives, config, constants, state wrappers, `BootstrapToken` validation, `NetworkExposureSpec` |
| `chv-controlplane-store` | `sqlx` repositories: nodes, observed state, desired state, operations, events, alerts, **bootstrap tokens**, **network exposures** |
| `chv-controlplane-service` | gRPC service trait definitions + implementations; axum admin router |
| `control-plane-node-api` | Tonic-generated proto types |
| `chv-config` | Config file + environment loading |
| `chv-observability` | Tracing + metrics |

### Out of scope for Phase 1
- Outbound gRPC client to `chv-agent`.
- Complex scheduling / placement logic.
- WebUI auth integration.
- Write mutations on the HTTP admin layer.

---

## Data Flow: ReconcileService

1. **Validate** `RequestMeta` and `DesiredStateFragment`.
   - `node_id` must exist.
   - `generation` must be numeric decimal string (reject non-numeric with `InvalidArgument`).
   - `kind` must match the RPC endpoint.
2. **Parse `spec_json`** into a typed Rust struct (`VmSpec`, `VolumeSpec`, `NetworkSpec`, `NodeSpec`).
3. **Persist** via `DesiredStateRepository`:
   - VM/Volume/Network: upsert base row + `*_desired_state` row.
   - Network: upsert `network_exposures` if exposure rules are present.
   - Node: upsert `node_desired_state`.
4. **Emit event** via `EventRepository`:
   - `DesiredStateApplied` on success.
   - `DesiredStateRejected` on validation/parse failure.
5. **Return** `AckResponse`.

`AcknowledgeDesiredStateVersion` records that the node has applied the fragment.

---

## Data Flow: LifecycleService

1. **Validate inputs** (node exists, IDs valid, idempotency checks).
2. **Create or retrieve `Operation`** via `OperationRepository::create_or_get`.
   - Idempotency key = hash of `(operation_type, node_id, resource_id, desired_generation)`.
   - Status = `Pending`.
   - Emit `OperationStarted` event.
3. **Update desired state** where applicable:
   - `DrainNode` → `node_desired_state = Draining`.
   - `EnterMaintenance` → `node_desired_state = Maintenance`.
   - `CreateVm` → upsert `vm_desired_state`.
4. **Return** `AckResponse` with `operation_id`.

Operations remain `Pending` until a Phase 2 background worker executes them.

---

## HTTP Admin / BFF Layer (Axum)

Mandatory separate `http_bind` port.

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/health` | Liveness (always 200 if up) |
| `GET` | `/ready` | Readiness (`SELECT 1` on DB) |
| `GET` | `/metrics` | Prometheus exposition |
| `GET` | `/admin/nodes` | List nodes + latest observed state |
| `GET` | `/admin/nodes/:id` | Single node + inventory + versions |
| `GET` | `/admin/operations` | Recent operations (paginated) |
| `GET` | `/admin/operations/:id` | Single operation status |

All Phase 1 HTTP endpoints are **read-only**.

---

## Error Mapping

Structured mapper from `ControlPlaneServiceError` to `tonic::Status`:

| Variant | `tonic::Status` |
|---------|-----------------|
| `NotFound` | `NotFound` |
| `InvalidArgument` | `InvalidArgument` |
| `Unauthorized` | `Unauthenticated` |
| `Conflict` | `AlreadyExists` / `FailedPrecondition` |
| `StaleGeneration` | `FailedPrecondition` |
| `Internal` | `Internal` |

New variants:
- `StaleGeneration { expected, received }`
- `Conflict(String)`

---

## Test Strategy

- **Unit tests:** `chv-controlplane-types` (parsing, transitions), `chv-controlplane-service` (error mapper, JSON parsers).
- **Integration tests (testcontainers Postgres):** store repositories, full service implementations (enrollment with token table, reconcile, lifecycle).
- **HTTP tests:** `tower::ServiceExt::oneshot` against the axum router with a real `StorePool`.
- gRPC surface tested via trait impl + real repositories (no mocks-only tests).

---

## Migration Strategy from Legacy Go

1. Delete `legacy/go-controlplane/` directory entirely.
2. Update `Cargo.toml` workspace if any paths referenced legacy (none do currently).
3. Update CI configs if they build/test Go code (to be verified during implementation).
4. Ensure `REPOSITORY_DIRECTION.md` and root `README.md` clearly state Rust is the active backend.

---

## Database Migrations Needed

1. `0003_bootstrap_tokens.sql` — create `bootstrap_tokens` table.
2. `0004_network_exposures.sql` — create `network_exposures` table, remove exposure columns from `network_desired_state`.
3. `0005_generation_check_constraint.sql` — optional `CHECK` constraint on generation columns to enforce numeric-only at DB level.
