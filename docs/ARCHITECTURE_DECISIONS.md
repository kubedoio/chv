# Architecture Decision Records (ADRs)

## ADR 1: Control Plane / Agent Split
**Decision:** System operations are strictly separated. The `chv-controller` manages the REST API and database. The `chv-agent` runs natively on hypervisor nodes.
**Rationale:** The controller must not be bogged down or crashed by local host command executions. This provides a clean security boundary and paves the way for multi-node management in Stage 4.

## ADR 2: SQLite in WAL Mode
**Decision:** The platform uses SQLite as the sole persistent data store. It must be configured with Write-Ahead Logging (`PRAGMA journal_mode=WAL;`).
**Rationale:** Eliminates the need for a separate database daemon (like PostgreSQL) while maintaining high performance for concurrent API reads. It keeps the deployment footprint to an absolute minimum. 

## ADR 3: Frontend as Pure SPA
**Decision:** The Web UI is built in SvelteKit and compiled strictly as a Single Page Application (SPA).
**Rationale:** Internal infrastructure tools do not require SEO. Bypassing Server-Side Rendering (SSR) reduces RAM overhead and allows the static UI assets to be served directly by the Go backend or a lightweight container.

## ADR 4: Feature-Constrained MVP Primitives
**Decision:** MVP-v1 strictly supports Linux bridging (`chvbr0`) and local storage (`localdisk`). 
**Rationale:** Implementing OVS/OVN or Ceph introduces massive dependency chains and failure points. Proving the VM lifecycle works flawlessly on standard Linux primitives is mandatory before introducing distributed complexities.

## ADR 5: Node-Scoped Resource Model
**Decision:** All resources (VMs, images, storage pools, networks) are scoped to nodes. The database schema includes `node_id` foreign keys, and API endpoints provide both global (`/api/v1/vms`) and node-scoped (`/api/v1/nodes/{id}/vms`) access patterns.
**Rationale:** 
- Enables future multi-node support without schema changes
- Maintains backward compatibility with existing single-node deployments
- Provides clean API semantics for resource ownership
- Allows for future node-level permissions and quotas

**Implementation:**
- Database: Added `nodes` table with `id`, `name`, `hostname`, `ip_address`, `status` fields
- Schema: Added `node_id` columns to `networks`, `storage_pools`, `images`, `virtual_machines` tables
- API: New endpoints `/api/v1/nodes/{id}/{resource}` with node validation
- Migration: Existing data automatically associated with default "local" node

## ADR 6: Contract-First API Design
**Decision:** API contracts are designed and documented before implementation. All endpoints follow consistent patterns for request/response format, error handling, and resource naming.
**Rationale:**
- Reduces coupling between frontend and backend
- Enables parallel development
- Provides clear contracts for future API consumers (CLI, external integrations)
- Follows Hyrum's Law - every public behavior becomes a de facto contract

**Conventions:**
- Resource naming: Plural nouns (`/vms`, `/images`)
- Node-scoped resources: `/nodes/{id}/{resource}`
- Error format: `{ "error": { "code", "message", "retryable", "hint" } }`
- List responses: Include `count` field for pagination support
- Node-scoped lists: Include `node_id`, `node_name`, `resources`, `count`
