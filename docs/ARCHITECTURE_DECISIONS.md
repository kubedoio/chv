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
