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
//! # Module map
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
//! - [`client`] — [`client::NetBoxClient`]: the bounded, HTTPS-only,
//!   fail-closed NetBox REST client (PR 4).
//! - [`runner`] — [`runner::NetboxProjectionRunner`]: fetches remote
//!   state, computes the plan, and executes it with the
//!   abort-on-first-hard-failure / resume-by-external-id policy (PR 4).
//!
//! The pure core (mapping/ownership/plan) remains free of I/O, clocks,
//! and randomness — identical inputs always produce byte-identical
//! plans, which is what makes dry-run output stable and unit-testable.
//! The client and runner are the I/O shell and depend on nothing from
//! the control plane: the composition root
//! (`chv-controlplane-service`'s worker) loads state from the store,
//! hands plain data to the runner, and persists outcomes.
//!
//! # Conventions
//!
//! `tracing`-only logging, no panics in library code (errors are
//! returned, never unwrapped), and [`serde`] types use
//! `BTreeMap`/sorted collections exclusively so serialized output is
//! byte-stable.

#![deny(unsafe_code)]

pub mod client;
pub mod mapping;
pub mod ownership;
pub mod plan;
pub mod runner;

pub use client::{ClientError, NetBoxClient, NetBoxToken, RemoteNetBoxObject};
pub use mapping::{
    build_objects, validate_netbox_name, validate_netbox_slug, DeviceStatus, MappingError,
    MappingIssue, MappingOutput, NetBoxDevice, NetBoxInterface, NetBoxIpAddress, NetBoxKind,
    NetBoxObject, NetBoxPrefix, NetBoxVirtualMachine, NetBoxVlan, ProjectionConfigView,
    ProjectionInput, VmStatus, NETBOX_NAME_MAX_LEN,
};
pub use ownership::{
    external_id, validate_custom_field_prefix, CustomFieldNames, ManagedMarker, ManagedState,
    DEFAULT_CUSTOM_FIELD_PREFIX, MANAGED_BY_CHV, MAPPING_VERSION, RESOURCE_SLUG_INSTANCE,
    RESOURCE_SLUG_NETWORK, RESOURCE_SLUG_SERVER,
};
pub use plan::{
    compute_plan, NetBoxRemoteObject, NetboxPlanAction, NetboxProjectionPlan,
    NetboxProjectionPlanEntry, PlanContext, PlanError, PlanSummary, RetentionPolicy,
};
pub use runner::{
    NetboxEntryOutcome, NetboxEntryStatus, NetboxOutcomeSummary, NetboxProjectionInput,
    NetboxProjectionOutcome, NetboxProjectionRunner, NetboxRunnerErrorSummary, RunnerError,
};
