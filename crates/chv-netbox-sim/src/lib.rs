//! `chv-netbox-sim` — a stateful, in-process NetBox 4.x simulator for
//! CHV's integration tests and the interactive demo harness (ADR-024,
//! issue kubedoio/chv#586).
//!
//! # What this is — and what it must never be
//!
//! - **Dev-only tool.** A library for in-process tests plus an opt-in
//!   `netbox-sim` binary behind the `bin` cargo feature. The simulator
//!   must never appear in a production dependency graph.
//! - **Never the source of truth for the wire format.** Its behavioral
//!   requirements are the mapping contract
//!   (`docs/specs/architecture-designer/contracts/netbox-mapping-contract.md`,
//!   section "NetBox REST surface used (v1)"); its wire shapes are
//!   pinned by golden fixtures under `tests/fixtures/netbox4/` with
//!   recorded provenance. Fixtures are never hand-edited after a real
//!   capture; they are refreshed only by the qualification `--record`
//!   mode against a live NetBox (PR 5 of the #586 campaign).
//! - **Exactly the client's used surface.** The six endpoint families
//!   (`dcim/devices`, `virtualization/virtual-machines`,
//!   `virtualization/interfaces`, `ipam/prefixes`, `ipam/vlans`,
//!   `ipam/ip-addresses`) with `GET` lists (`limit`/`offset`
//!   pagination, natural-key and `cf_<field>` custom-field filters),
//!   `POST`/`PATCH`/`DELETE`, and `Authorization: Token` auth.
//!   Anything else returns `404 {"detail": "Not found."}` like a real
//!   NetBox.
//!
//! # Test-control plane
//!
//! The `__`-prefixed endpoints (`POST /__seed`, `GET /__state`,
//! `POST /__reset`, `POST /__faults`) are test-harness surface, not
//! part of the NetBox contract, and are obviously never provided by a
//! real NetBox. They are unauthenticated by design (the simulator is
//! bound to loopback — the `netbox-sim` binary refuses non-loopback
//! binds) and must never be referenced by production code.
//!
//! # Fidelity notes
//!
//! Behaviors deliberately modeled on real NetBox 4.x but not yet
//! captured from a live instance (the PR 5 qualification run is the
//! tripwire that will pin them):
//!
//! - Default page size 50 (`PAGINATE_COUNT` analog) and `limit=0` /
//!   oversized `limit` clamped to the server max page size 1000
//!   (`MAX_PAGE_SIZE` analog).
//! - Malformed `limit`/`offset` values fall back to the defaults
//!   (default page size / offset 0) instead of failing the request:
//!   NetBox's `OptionalLimitOffsetPagination.get_limit` and DRF's
//!   `get_offset` wrap the parse in try/except.
//! - Pagination links (`next`/`previous`) carry the **effective**
//!   `limit`: DRF's `LimitOffsetPagination.get_next_link` — which
//!   NetBox 4.x's `OptionalLimitOffsetPagination` inherits unchanged
//!   — rewrites the request URL with
//!   `replace_query_param(url, "limit", self.limit)`, so a request
//!   without a `limit` gets the default page size in its links and
//!   an oversized `limit` is echoed as the clamped value.
//! - `PATCH` merges `custom_fields` per key (a partial
//!   `custom_fields` patch keeps unmentioned keys — required for the
//!   adapter's `mark_stale`, which patches only the managed-state
//!   field).
//! - Foreign keys (device on a VM, parent VM on an interface, VLAN on
//!   a prefix) are *not* validated against existing rows: the
//!   mapping contract's kind order creates children before parents,
//!   so the write path must accept not-yet-existing references.
//!   Nested relations carry a synthetic id from a per-instance
//!   registry (or the real row id when the target exists).
//! - Maskless IP addresses are stored with a `/32` (v4) or `/128`
//!   (v6) suffix, like NetBox's own normalization.
//! - IP-address uniqueness keys on the full with-mask address
//!   (`10.42.0.5/24` and `10.42.0.5/32` are distinct rows — NetBox's
//!   unique constraint includes the mask); list *filtering* stays
//!   mask-independent, like NetBox's `address` search.
//! - Deleting a device nulls the `device` reference on virtual
//!   machines (real NetBox: `VirtualMachine.device` is
//!   `on_delete=SET_NULL`).
//! - Deleting a VLAN that a prefix still references is refused with
//!   `409 {"detail": ...}` and clears nothing. Real NetBox's
//!   `Prefix.vlan` is `on_delete=PROTECT`, and Django surfaces the
//!   resulting `ProtectedError` through the REST API as an HTTP 500
//!   — this simulator answers 409 with an explanatory body instead.
//!   Deliberate deviation: identical "the delete failed, nothing
//!   changed" semantics for the client, without modeling a server
//!   crash; revisit if the qualification run pins the real shape.
//! - Non-API paths (unknown endpoints, non-numeric ids) answer with
//!   the JSON body `{"detail": "Not found."}`, whereas a real NetBox
//!   renders an HTML error page for non-API URLs. The JSON body is
//!   more convenient for tests, is pinned by test, and is outside
//!   the adapter client's surface (it only ever reaches API paths).
//! - The cursor-style `?start=` parameter (real NetBox: cursor
//!   pagination mode, `pk >= start`) is silently ignored as a no-op
//!   filter. That is fine for the current write-path-only surface,
//!   but a future read-path integration MUST implement it before
//!   relying on it.
//!
//! # Conventions
//!
//! `tracing`-only logging (never `println!`), no panics in request
//! handlers (malformed input is a 400, unknown ids a 404), and
//! serde output uses sorted maps so responses are byte-stable.

pub mod config;
pub mod fault;
pub mod kind;
pub mod server;
pub mod state;
pub mod wire;

pub use config::NetboxSimConfig;
pub use fault::FaultConfig;
pub use kind::SimKind;
pub use server::NetboxSim;
pub use state::{SeedPayload, SimShared, SimState};
pub use wire::WireError;
