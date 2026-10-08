//! NetBox projection adapter — pure core (PR 2 of the #239 plan).
//!
//! Projects an applied CHV architecture topology into NetBox
//! DCIM/IPAM objects as a **bounded, downstream projection** — CHV stays
//! the lifecycle authority, NetBox is an inventory view. See:
//!
//! - Design: `docs/design/issue-239-netbox-projection.md` (DP1–DP12)
//! - ADR: `docs/specs/adr/023-netbox-projection.md`
//! - Mapping contract (the specification this crate implements):
//!   `docs/specs/architecture-designer/contracts/netbox-mapping-contract.md`
//! - API contract (PR 4): `docs/specs/architecture-designer/contracts/netbox-api-contract.md`
//! - Plan: `docs/plans/2026-10-08-netbox-projection-implementation-plan.md`
//!
//! # PR-2 boundary — pure core, no I/O
//!
//! This crate currently ships only the pure, deterministic core:
//!
//! - [`ownership`] — the external-id format, the prefix-configurable
//!   custom-field name set, and the [`ownership::ManagedMarker`] write
//!   guard parsed from NetBox custom fields.
//! - [`mapping`] — [`mapping::build_objects`]: the applied
//!   [`CHVArchitecture`](chv_architecture_validate::model::CHVArchitecture)
//!   (optionally enriched with a live
//!   [`InventorySnapshot`](chv_architecture_validate::fleet::InventorySnapshot))
//!   mapped onto NetBox object models, with the v1 exclusion list and
//!   secret exclusion enforced.
//! - [`plan`] — [`plan::compute_plan`]: the deterministic
//!   create/update/no_op/conflict/stale diff against an in-memory
//!   [`plan::NetBoxRemoteObject`] view of NetBox state.
//!
//! No HTTP client, no tokio, no sqlx, no clock: identical inputs always
//! produce byte-identical plans, which is what makes dry-run output
//! stable and unit-testable. The REST client, runner, and worker arrive
//! in PR 4; config and persistence in PR 3.
//!
//! # Conventions
//!
//! `tracing`-only logging (nothing in the pure core needs to log), no
//! panics in library code (errors are returned, never unwrapped), and
//! [`serde`] types use `BTreeMap`/sorted collections exclusively so
//! serialized output is byte-stable.

#![deny(unsafe_code)]

pub mod mapping;
pub mod ownership;
pub mod plan;

pub use mapping::{
    build_objects, validate_netbox_name, DeviceStatus, MappingError, MappingIssue, MappingOutput,
    NetBoxDevice, NetBoxInterface, NetBoxIpAddress, NetBoxKind, NetBoxObject, NetBoxPrefix,
    NetBoxVirtualMachine, NetBoxVlan, ProjectionConfigView, ProjectionInput, VmStatus,
    NETBOX_NAME_MAX_LEN,
};
pub use ownership::{
    external_id, CustomFieldNames, ManagedMarker, ManagedState, DEFAULT_CUSTOM_FIELD_PREFIX,
    MANAGED_BY_CHV, MAPPING_VERSION,
};
pub use plan::{
    compute_plan, NetBoxRemoteObject, NetboxPlanAction, NetboxProjectionPlan,
    NetboxProjectionPlanEntry, PlanContext, PlanSummary, RetentionPolicy,
};
