//! NetBox projection plan: deterministic diff of desired objects against
//! an in-memory view of NetBox state (design §8, mapping contract
//! "Ownership and collision semantics").
//!
//! [`compute_plan`] is pure: it consumes the [`MappingOutput`] built by
//! [`crate::mapping::build_objects`] and a slice of
//! [`NetBoxRemoteObject`]s — the abstraction PR 4's client fills from
//! real HTTP lookups — and produces a [`NetboxProjectionPlan`] whose
//! entries are `create | update | no_op | conflict | stale`.
//!
//! # Collision semantics (implemented exactly)
//!
//! ```text
//! lookup by chv_external_id (fully-marked remote object)
//!   ├─ found, chv_managed_by == "chv"
//!   │     ├─ content equal          → no_op
//!   │     └─ content differs        → update
//!   └─ found, foreign/absent owner  → conflict (never write)
//! not found by external id
//!   ├─ natural key free                 → create
//!   ├─ natural key occupied, foreign    → conflict (never write)
//!   └─ natural key occupied, chv-owned
//!        object of this architecture     → update (partial-failure
//!                                          resume / version bump)
//! ```
//!
//! The "found by external id" lookup models NetBox's custom-field filter
//! (`?cf_chv_external_id=…`): it matches remote objects carrying a
//! **complete** [`ManagedMarker`] with our external id. A partially
//! written object (create succeeded, custom fields incomplete) is missed
//! by that lookup but recovered by the natural-key branch, which
//! recognises it by its raw external id + `chv_managed_by` and resumes
//! as an `update` — no duplicate create.
//!
//! The write guard is unconditional: no `create`/`update` entry is ever
//! proposed for an object whose `chv_managed_by` is not `chv`, and
//! `conflict` entries carry empty `changes`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::mapping::{MappingIssue, MappingOutput, NetBoxKind, NetBoxObject};
use crate::ownership::{CustomFieldNames, ManagedMarker, MANAGED_BY_CHV};

/// Placeholder shown for a field that is unset on one side of a diff.
const UNSET: &str = "(unset)";

/// Plan action, per the mapping contract's plan-entry shape. Named
/// `NetboxPlanAction` (not `PlanAction`) to avoid confusion with the
/// architecture-reconcile `PlanAction` in `chv-controlplane-types`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetboxPlanAction {
    Create,
    Update,
    NoOp,
    Conflict,
    Stale,
}

impl NetboxPlanAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::NoOp => "no_op",
            Self::Conflict => "conflict",
            Self::Stale => "stale",
        }
    }

    /// Deterministic tiebreak rank for entry ordering.
    const fn rank(self) -> u8 {
        self as u8
    }
}

/// Retention policy for removed CHV resources (design DP12).
///
/// Both policies emit `stale` action entries; what happens to the
/// object afterwards is the runner's concern (PR 4). Under `MarkStale`
/// (the default) nothing is ever deleted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionPolicy {
    #[default]
    MarkStale,
    Delete,
}

impl RetentionPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MarkStale => "mark_stale",
            Self::Delete => "delete",
        }
    }
}

/// One plan entry, mirroring the contract's JSON shape exactly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetboxProjectionPlanEntry {
    pub action: NetboxPlanAction,
    pub kind: NetBoxKind,
    /// `servers/<name>`-style CHV resource reference.
    pub chv_resource_ref: String,
    /// Natural key of the NetBox object (BTreeMap → byte-stable JSON).
    pub netbox_natural_key: BTreeMap<String, String>,
    /// The external id the desired object carries (or would carry).
    pub external_id: String,
    /// Human-readable, secret-free explanation.
    pub reason: String,
    /// `"field: old → new"` diff strings; empty unless `action ==
    /// update`.
    pub changes: Vec<String>,
}

/// Aggregate entry counts by action.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanSummary {
    pub create: i64,
    pub update: i64,
    pub no_op: i64,
    pub conflict: i64,
    pub stale: i64,
}

/// The full projection plan — what a dry-run returns and an export
/// executes. Deterministic and secret-free: BTreeMaps everywhere, no
/// timestamps, so identical inputs serialize byte-identically.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetboxProjectionPlan {
    /// Mapping contract version (`v1`).
    pub mapping_version: String,
    pub architecture_id: String,
    pub architecture_version: u64,
    /// Retention policy in effect; recorded so runners (PR 4) and the
    /// UI can render `stale` handling without re-reading config.
    pub retention: RetentionPolicy,
    pub summary: PlanSummary,
    /// Ordered by (kind rank, name, action rank).
    pub entries: Vec<NetboxProjectionPlanEntry>,
}

/// In-memory view of one NetBox object the projection cares about.
///
/// PR 4's client fills these from real HTTP lookups (custom-field and
/// natural-key queries); the pure core only consumes them. `content`
/// mirrors [`NetBoxObject::content_fields`] so desired-vs-remote
/// equality is a plain map comparison.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetBoxRemoteObject {
    pub kind: NetBoxKind,
    pub natural_key: BTreeMap<String, String>,
    /// All NetBox custom fields on the object (chv ownership marker,
    /// enrichment, and any foreign fields).
    pub custom_fields: BTreeMap<String, String>,
    /// Flattened content fields (name, site, status, …).
    pub content: BTreeMap<String, String>,
}

/// Ownership / config context [`compute_plan`] needs beyond the desired
/// objects themselves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanContext {
    pub architecture_id: String,
    pub architecture_version: u64,
    /// Custom-field names (prefix-configurable) used for marker parsing.
    pub names: CustomFieldNames,
    /// Retention policy recorded on the plan.
    pub retention: RetentionPolicy,
}

impl PlanContext {
    /// Context with default (`chv_`) custom-field names and the
    /// `mark_stale` retention default.
    pub fn new(architecture_id: impl Into<String>, architecture_version: u64) -> Self {
        Self {
            architecture_id: architecture_id.into(),
            architecture_version,
            names: CustomFieldNames::default(),
            retention: RetentionPolicy::default(),
        }
    }
}

/// Compute the projection plan: desired objects + reported mapping
/// issues versus the remote NetBox state view.
///
/// Deterministic: entry order is (kind rank, name, action rank) and all
/// serialized shapes use BTreeMaps, so the same inputs always produce a
/// byte-identical plan.
pub fn compute_plan(
    desired: &MappingOutput,
    remote: &[NetBoxRemoteObject],
    context: &PlanContext,
) -> NetboxProjectionPlan {
    let names = &context.names;
    let mut entries = Vec::new();

    for object in &desired.objects {
        let Some(ext_id) = object.custom_fields().get(&names.external_id).cloned() else {
            // Built objects always carry the marker; a miss here means a
            // hand-assembled input. Skip rather than panic — the object
            // simply cannot be reconciled.
            continue;
        };
        let natural_key = object.natural_key();

        // 1. Lookup by external id: a fully-marked remote object
        //    carrying our external id.
        let ext_match = remote.iter().find(|r| {
            ManagedMarker::parse(&r.custom_fields, names)
                .map(|marker| marker.external_id == ext_id)
                .unwrap_or(false)
        });
        if let Some(remote_object) = ext_match {
            let marker =
                ManagedMarker::parse(&remote_object.custom_fields, names).expect("parsed above");
            if marker.is_owned_by_chv() && marker.architecture_id == context.architecture_id {
                let changes = content_changes(object, remote_object, names);
                let (action, reason) = if changes.is_empty() {
                    (
                        NetboxPlanAction::NoOp,
                        "remote object matches the desired projection".to_string(),
                    )
                } else {
                    (
                        NetboxPlanAction::Update,
                        "chv-owned remote object differs from the desired projection".to_string(),
                    )
                };
                entries.push(entry(action, object, natural_key, ext_id, reason, changes));
            } else {
                entries.push(entry(
                    NetboxPlanAction::Conflict,
                    object,
                    natural_key,
                    ext_id,
                    format!(
                        "external id matches a NetBox object not owned by this chv architecture (chv_managed_by={:?})",
                        marker.managed_by
                    ),
                    Vec::new(),
                ));
            }
            continue;
        }

        // 2. Not found by external id → natural key.
        let nk_match = remote
            .iter()
            .find(|r| r.kind == object.kind() && r.natural_key == natural_key);
        match nk_match {
            None => entries.push(entry(
                NetboxPlanAction::Create,
                object,
                natural_key,
                ext_id,
                "natural key is free and no remote object carries this external id".to_string(),
                Vec::new(),
            )),
            Some(remote_object) => {
                let carries_ext_id = remote_object
                    .custom_fields
                    .get(&names.external_id)
                    .map(|v| v == &ext_id)
                    .unwrap_or(false);
                let chv_owned = remote_object
                    .custom_fields
                    .get(&names.managed_by)
                    .map(|v| v == MANAGED_BY_CHV)
                    .unwrap_or(false);
                let same_architecture = remote_object
                    .custom_fields
                    .get(&names.architecture_id)
                    .map(|v| v == &context.architecture_id)
                    .unwrap_or(false);

                // Partial-failure resume (chv-owned object carrying our
                // external id — the id itself embeds the architecture id,
                // which is sufficient proof even when the marker is
                // incomplete) or version bump (external id changed, object
                // still ours and still this architecture).
                if chv_owned && (carries_ext_id || same_architecture) {
                    let reason = if carries_ext_id {
                        "partial-failure resume: natural key occupied by a chv-owned object carrying this external id".to_string()
                    } else {
                        "natural key occupied by a chv-owned object of this architecture; external id changed".to_string()
                    };
                    let changes = content_changes(object, remote_object, names);
                    entries.push(entry(
                        NetboxPlanAction::Update,
                        object,
                        natural_key,
                        ext_id,
                        reason,
                        changes,
                    ));
                } else {
                    entries.push(entry(
                        NetboxPlanAction::Conflict,
                        object,
                        natural_key,
                        ext_id,
                        "natural key occupied by an object not owned by this chv architecture"
                            .to_string(),
                        Vec::new(),
                    ));
                }
            }
        }
    }

    // Invalid names (mapping contract rule 1) → conflict entries; never
    // silently dropped, never renamed.
    for issue in &desired.issues {
        entries.push(issue_conflict_entry(issue));
    }

    // Stale: chv-owned objects of this architecture whose natural key no
    // longer matches any desired object (design DP12).
    for remote_object in remote {
        let Some(marker) = ManagedMarker::parse(&remote_object.custom_fields, names) else {
            continue;
        };
        if !marker.is_owned_by_chv() || marker.architecture_id != context.architecture_id {
            continue;
        }
        let still_desired = desired.objects.iter().any(|o| {
            o.kind() == remote_object.kind && o.natural_key() == remote_object.natural_key
        });
        if still_desired {
            continue;
        }
        entries.push(NetboxProjectionPlanEntry {
            action: NetboxPlanAction::Stale,
            kind: remote_object.kind,
            chv_resource_ref: resource_ref_from_external_id(&marker.external_id),
            netbox_natural_key: remote_object.natural_key.clone(),
            external_id: marker.external_id.clone(),
            reason: format!(
                "chv-owned object of this architecture whose CHV source disappeared; retention policy: {}",
                context.retention.as_str()
            ),
            changes: Vec::new(),
        });
    }

    // Deterministic ordering: kind rank, then name, then action rank.
    entries.sort_by_key(entry_sort_key);

    let mut summary = PlanSummary::default();
    for entry in &entries {
        match entry.action {
            NetboxPlanAction::Create => summary.create += 1,
            NetboxPlanAction::Update => summary.update += 1,
            NetboxPlanAction::NoOp => summary.no_op += 1,
            NetboxPlanAction::Conflict => summary.conflict += 1,
            NetboxPlanAction::Stale => summary.stale += 1,
        }
    }

    NetboxProjectionPlan {
        mapping_version: crate::ownership::MAPPING_VERSION.to_string(),
        architecture_id: context.architecture_id.clone(),
        architecture_version: context.architecture_version,
        retention: context.retention,
        summary,
        entries,
    }
}

fn entry(
    action: NetboxPlanAction,
    object: &NetBoxObject,
    natural_key: BTreeMap<String, String>,
    ext_id: String,
    reason: String,
    changes: Vec<String>,
) -> NetboxProjectionPlanEntry {
    NetboxProjectionPlanEntry {
        action,
        kind: object.kind(),
        chv_resource_ref: resource_ref(object),
        netbox_natural_key: natural_key,
        external_id: ext_id,
        reason,
        changes,
    }
}

/// The `chv_resource_ref` for a desired object, following the contract's
/// `<plural-kind>/<name>` example.
fn resource_ref(object: &NetBoxObject) -> String {
    use NetBoxObject as O;
    match object {
        O::Device(d) => format!("servers/{}", d.name),
        O::VirtualMachine(v) => format!("instances/{}", v.name),
        O::Interface(i) => format!("instances/{}/networks/{}", i.virtual_machine, i.name),
        O::Prefix(p) => format!("networks/{}", p.description),
        O::Vlan(v) => format!("networks/{}", v.name),
        O::IpAddress(a) => match &a.assigned_to_interface {
            Some(interface) => format!("instances/{interface}"),
            None => format!("instances/{}", a.address),
        },
    }
}

/// Best-effort `chv_resource_ref` for a remote object whose CHV source
/// disappeared: recover the `<kind>/<name>` segment of the external id.
fn resource_ref_from_external_id(ext_id: &str) -> String {
    // arch:<architecture_id>:<kind>/<name>:<version>
    ext_id.split(':').nth(2).unwrap_or_default().to_string()
}

fn issue_conflict_entry(issue: &MappingIssue) -> NetboxProjectionPlanEntry {
    let mut natural_key = BTreeMap::new();
    natural_key.insert("name".to_string(), issue.name.clone());
    NetboxProjectionPlanEntry {
        action: NetboxPlanAction::Conflict,
        kind: issue.kind,
        chv_resource_ref: issue.chv_resource_ref.clone(),
        netbox_natural_key: natural_key,
        external_id: issue.external_id.clone(),
        reason: format!("name fails NetBox charset validation: {}", issue.reason),
        changes: Vec::new(),
    }
}

/// Per-field diff of a desired object against its remote counterpart:
/// content fields plus every custom field we project. Keys are iterated
/// in BTreeMap order, so the list is deterministic.
fn content_changes(
    object: &NetBoxObject,
    remote: &NetBoxRemoteObject,
    names: &CustomFieldNames,
) -> Vec<String> {
    let mut desired = object.content_fields();
    for (key, value) in object.custom_fields() {
        desired.insert(key.clone(), value.clone());
    }

    let mut changes = Vec::new();
    for (key, value) in &desired {
        let remote_value = remote
            .content
            .get(key)
            .or_else(|| remote.custom_fields.get(key));
        match remote_value {
            Some(old) if old == value => {}
            Some(old) => changes.push(format!("{key}: {old} → {value}")),
            None => changes.push(format!("{key}: {UNSET} → {value}")),
        }
    }
    // Prefixed custom fields present remotely but no longer projected
    // count as removals; foreign (non-prefixed) fields are ignored.
    for (key, old) in &remote.custom_fields {
        if key.starts_with(&names.prefix) && !desired.contains_key(key) {
            changes.push(format!("{key}: {old} → {UNSET}"));
        }
    }
    changes
}

fn entry_sort_key(entry: &NetboxProjectionPlanEntry) -> (u8, String, u8) {
    (entry.kind.rank(), entry_name(entry), entry.action.rank())
}

fn entry_name(entry: &NetboxProjectionPlanEntry) -> String {
    entry
        .netbox_natural_key
        .get("name")
        .or_else(|| entry.netbox_natural_key.get("prefix"))
        .or_else(|| entry.netbox_natural_key.get("address"))
        .or_else(|| entry.netbox_natural_key.values().next())
        .cloned()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use serde_json;

    use super::*;
    use crate::mapping::build_objects;
    use crate::mapping::testsupport::{
        architecture_with_extra_server, projection_input, test_architecture, ARCH_ID,
    };

    /// Simulate "NetBox currently mirrors exactly these objects".
    fn mirror(objects: &[NetBoxObject]) -> Vec<NetBoxRemoteObject> {
        objects
            .iter()
            .map(|object| NetBoxRemoteObject {
                kind: object.kind(),
                natural_key: object.natural_key(),
                custom_fields: object.custom_fields().clone(),
                content: object.content_fields(),
            })
            .collect()
    }

    fn desired(version: u64) -> crate::mapping::MappingOutput {
        let arch = test_architecture();
        build_objects(&projection_input(&arch, version, None)).expect("builds")
    }

    fn device_ext_id(output: &crate::mapping::MappingOutput, name: &str) -> String {
        output
            .objects
            .iter()
            .find(|o| o.kind() == NetBoxKind::Device && o.name() == name)
            .and_then(|o| o.custom_fields().get("chv_external_id").cloned())
            .expect("device external id")
    }

    #[test]
    fn identical_reprojection_is_all_no_op() {
        let desired = desired(3);
        let remote = mirror(&desired.objects);
        let plan = compute_plan(&desired, &remote, &PlanContext::new(ARCH_ID, 3));

        assert!(!plan.entries.is_empty());
        assert!(plan
            .entries
            .iter()
            .all(|e| e.action == NetboxPlanAction::NoOp));
        assert_eq!(plan.summary.no_op as usize, plan.entries.len());
        assert_eq!(plan.summary.create, 0);
        assert_eq!(plan.summary.update, 0);
        assert_eq!(plan.summary.conflict, 0);
        assert_eq!(plan.summary.stale, 0);
        assert_eq!(plan.mapping_version, "v1");
    }

    #[test]
    fn version_bump_updates_with_field_diffs() {
        let old = desired(3);
        let remote = mirror(&old.objects);

        // New version with changed declared resources.
        let mut arch = test_architecture();
        arch.servers[0].resources = Some(chv_architecture_validate::model::ServerResources {
            cpu_cores: Some(8),
            memory_gb: Some(16),
        });
        let new = build_objects(&projection_input(&arch, 4, None)).expect("builds");

        let plan = compute_plan(&new, &remote, &PlanContext::new(ARCH_ID, 4));
        assert_eq!(plan.summary.update as usize, plan.entries.len());
        assert_eq!(plan.summary.no_op, 0);

        let device_entry = plan
            .entries
            .iter()
            .find(|e| e.kind == NetBoxKind::Device)
            .expect("device entry");
        assert_eq!(device_entry.action, NetboxPlanAction::Update);
        assert!(
            device_entry.changes.iter().any(|c| c == "cpu_cores: 4 → 8"),
            "expected cpu diff in {:?}",
            device_entry.changes
        );
        assert!(
            device_entry
                .changes
                .iter()
                .any(|c| c == "memory_gb: 8 → 16"),
            "expected memory diff in {:?}",
            device_entry.changes
        );
        assert!(
            device_entry
                .changes
                .iter()
                .any(|c| c.starts_with("chv_external_id: arch:arch_01HX:server/chv-node-01:3 → ")),
            "expected external-id version diff in {:?}",
            device_entry.changes
        );
    }

    #[test]
    fn name_collision_with_foreign_object_is_conflict_without_write() {
        let desired = desired(3);
        let foreign = NetBoxRemoteObject {
            kind: NetBoxKind::Device,
            natural_key: [("name".to_string(), "chv-node-01".to_string())].into(),
            custom_fields: BTreeMap::new(),
            content: [("name".to_string(), "chv-node-01".to_string())].into(),
        };
        let plan = compute_plan(&desired, &[foreign], &PlanContext::new(ARCH_ID, 3));

        let device_entry = plan
            .entries
            .iter()
            .find(|e| e.kind == NetBoxKind::Device)
            .expect("device entry");
        assert_eq!(device_entry.action, NetboxPlanAction::Conflict);
        assert!(device_entry.reason.contains("natural key occupied"));
        // Never propose a write.
        assert!(device_entry.changes.is_empty());
    }

    #[test]
    fn external_id_match_with_foreign_owner_is_conflict() {
        let desired = desired(3);
        let ext_id = device_ext_id(&desired, "chv-node-01");
        // Full marker, but owned by someone else.
        let mut custom_fields = ManagedMarker {
            external_id: ext_id.clone(),
            architecture_id: ARCH_ID.to_string(),
            managed_by: "netops".to_string(),
            managed_state: crate::ownership::ManagedState::Active,
            architecture_version: 3,
            mapping_version: "v1".to_string(),
        }
        .to_custom_fields(&CustomFieldNames::default());
        custom_fields.insert("chv_owner".to_string(), "alice".to_string());
        let foreign = NetBoxRemoteObject {
            kind: NetBoxKind::Device,
            natural_key: [("name".to_string(), "chv-node-01".to_string())].into(),
            custom_fields,
            content: BTreeMap::new(),
        };
        let plan = compute_plan(&desired, &[foreign], &PlanContext::new(ARCH_ID, 3));

        let device_entry = plan
            .entries
            .iter()
            .find(|e| e.kind == NetBoxKind::Device)
            .expect("device entry");
        assert_eq!(device_entry.action, NetboxPlanAction::Conflict);
        assert!(device_entry
            .reason
            .contains("not owned by this chv architecture"));
        assert!(device_entry.changes.is_empty());
        assert_eq!(plan.summary.conflict, 1);
    }

    #[test]
    fn partial_failure_resume_is_update_not_duplicate_create() {
        let desired = desired(3);
        let ext_id = device_ext_id(&desired, "chv-node-01");
        // Partially-written object: create succeeded, custom fields
        // incomplete — the indexed external-id lookup misses it.
        let partial = NetBoxRemoteObject {
            kind: NetBoxKind::Device,
            natural_key: [("name".to_string(), "chv-node-01".to_string())].into(),
            custom_fields: [
                ("chv_external_id".to_string(), ext_id),
                ("chv_managed_by".to_string(), "chv".to_string()),
            ]
            .into(),
            content: BTreeMap::new(),
        };
        let plan = compute_plan(&desired, &[partial], &PlanContext::new(ARCH_ID, 3));

        let device_entry = plan
            .entries
            .iter()
            .find(|e| e.kind == NetBoxKind::Device)
            .expect("device entry");
        assert_eq!(device_entry.action, NetboxPlanAction::Update);
        assert!(device_entry.reason.contains("partial-failure resume"));
        // No duplicate create is proposed for the occupied natural key;
        // the other (absent) objects still create.
        assert!(
            !plan
                .entries
                .iter()
                .any(|e| e.action == NetboxPlanAction::Create
                    && e.kind == NetBoxKind::Device
                    && e.netbox_natural_key.get("name").map(String::as_str) == Some("chv-node-01")),
            "must not propose a duplicate create"
        );
        assert_eq!(
            plan.summary.create as usize,
            desired.objects.len() - 1,
            "only the absent objects create"
        );
        // The resume diff includes the missing ownership fields.
        assert!(
            device_entry
                .changes
                .iter()
                .any(|c| c.starts_with("chv_managed_state: (unset) → ")),
            "expected marker backfill in {:?}",
            device_entry.changes
        );
    }

    #[test]
    fn removed_chv_resource_is_marked_stale() {
        // Remote state built from an architecture with an extra server.
        let remote_arch = architecture_with_extra_server();
        let remote_desired =
            build_objects(&projection_input(&remote_arch, 3, None)).expect("builds");
        let remote = mirror(&remote_desired.objects);

        // Desired state without that server.
        let desired = desired(3);
        let plan = compute_plan(&desired, &remote, &PlanContext::new(ARCH_ID, 3));

        let stale_entry = plan
            .entries
            .iter()
            .find(|e| e.action == NetboxPlanAction::Stale)
            .expect("stale entry for the removed server");
        assert_eq!(stale_entry.kind, NetBoxKind::Device);
        assert_eq!(
            stale_entry
                .netbox_natural_key
                .get("name")
                .map(String::as_str),
            Some("chv-node-02")
        );
        assert_eq!(
            stale_entry.chv_resource_ref, "server/chv-node-02",
            "resource ref recovered from the external id"
        );
        assert!(stale_entry.reason.contains("mark_stale"));
        assert_eq!(plan.summary.stale, 1);
        // Everything still desired is a no_op.
        assert_eq!(plan.summary.no_op as usize, desired.objects.len());
    }

    #[test]
    fn delete_retention_still_emits_stale_entries() {
        let remote_arch = architecture_with_extra_server();
        let remote_desired =
            build_objects(&projection_input(&remote_arch, 3, None)).expect("builds");
        let remote = mirror(&remote_desired.objects);
        let desired = desired(3);

        let mut context = PlanContext::new(ARCH_ID, 3);
        context.retention = RetentionPolicy::Delete;
        let plan = compute_plan(&desired, &remote, &context);

        assert_eq!(plan.summary.stale, 1);
        assert_eq!(plan.retention, RetentionPolicy::Delete);
        assert!(plan
            .entries
            .iter()
            .any(|e| e.action == NetboxPlanAction::Stale && e.reason.contains("delete")));
    }

    #[test]
    fn plan_serialization_is_byte_stable() {
        let desired = desired(3);
        let remote = mirror(&desired.objects);
        let context = PlanContext::new(ARCH_ID, 3);

        let plan_a = compute_plan(&desired, &remote, &context);
        let plan_b = compute_plan(&desired, &remote, &context);
        assert_eq!(
            serde_json::to_string(&plan_a).unwrap(),
            serde_json::to_string(&plan_b).unwrap()
        );

        // Remote order must not influence the output.
        let mut shuffled = remote.clone();
        shuffled.reverse();
        let plan_c = compute_plan(&desired, &shuffled, &context);
        assert_eq!(
            serde_json::to_string(&plan_a).unwrap(),
            serde_json::to_string(&plan_c).unwrap()
        );

        // Entries are ordered by kind rank.
        let ranks: Vec<u8> = plan_a.entries.iter().map(|e| e.kind.rank()).collect();
        let mut sorted = ranks.clone();
        sorted.sort();
        assert_eq!(ranks, sorted);
    }

    #[test]
    fn invalid_name_becomes_conflict_entry_with_reason() {
        let mut arch = test_architecture();
        arch.servers.push(chv_architecture_validate::model::Server {
            name: "Bad Node!".to_string(),
            management_ip: None,
            role: None,
            labels: BTreeMap::new(),
            resources: None,
            networks: None,
        });
        let output = build_objects(&projection_input(&arch, 3, None)).expect("builds");
        assert_eq!(output.issues.len(), 1);

        let plan = compute_plan(&output, &[], &PlanContext::new(ARCH_ID, 3));
        let conflict = plan
            .entries
            .iter()
            .find(|e| e.action == NetboxPlanAction::Conflict)
            .expect("conflict entry for the invalid name");
        assert_eq!(conflict.kind, NetBoxKind::Device);
        assert_eq!(conflict.chv_resource_ref, "servers/Bad Node!");
        assert!(conflict
            .reason
            .contains("name fails NetBox charset validation"));
        assert_eq!(conflict.changes.len(), 0);
        // Nothing silently dropped: every valid object is still planned.
        assert_eq!(plan.summary.create as usize, output.objects.len());
    }

    #[test]
    fn foreign_architecture_object_at_our_natural_key_is_conflict() {
        // Isolation of object families (design §7): a chv-owned object
        // belonging to a *different* architecture is not ours to touch.
        let desired = desired(3);
        let foreign_arch = NetBoxRemoteObject {
            kind: NetBoxKind::Device,
            natural_key: [("name".to_string(), "chv-node-01".to_string())].into(),
            custom_fields: ManagedMarker {
                external_id: crate::ownership::external_id(
                    "arch_OTHER",
                    "server",
                    "chv-node-01",
                    1,
                ),
                architecture_id: "arch_OTHER".to_string(),
                managed_by: "chv".to_string(),
                managed_state: crate::ownership::ManagedState::Active,
                architecture_version: 1,
                mapping_version: "v1".to_string(),
            }
            .to_custom_fields(&CustomFieldNames::default()),
            content: BTreeMap::new(),
        };
        let plan = compute_plan(&desired, &[foreign_arch], &PlanContext::new(ARCH_ID, 3));
        let device_entry = plan
            .entries
            .iter()
            .find(|e| e.kind == NetBoxKind::Device)
            .expect("device entry");
        assert_eq!(device_entry.action, NetboxPlanAction::Conflict);
        assert!(device_entry.changes.is_empty());
        // And it is not reported stale either (not our architecture).
        assert_eq!(plan.summary.stale, 0);
    }

    #[test]
    fn stale_object_matching_a_desired_natural_key_is_not_stale() {
        // Version bump: the remote object at our natural key is ours, so
        // it becomes an update — not a stale entry.
        let old = desired(3);
        let remote = mirror(&old.objects);
        let new = desired(4);
        let plan = compute_plan(&new, &remote, &PlanContext::new(ARCH_ID, 4));
        assert_eq!(plan.summary.stale, 0);
        assert_eq!(plan.summary.update as usize, new.objects.len());
    }

    #[test]
    fn prefix_configured_markers_drive_the_plan() {
        let arch = test_architecture();
        let input = crate::mapping::ProjectionInput {
            architecture: &arch,
            architecture_id: ARCH_ID,
            architecture_version: 3,
            snapshot: None,
            config: crate::mapping::ProjectionConfigView {
                custom_field_prefix: "acme_".to_string(),
                site_name: None,
            },
        };
        let desired = build_objects(&input).expect("builds");
        let remote = mirror(&desired.objects);

        let mut context = PlanContext::new(ARCH_ID, 3);
        context.names = CustomFieldNames::new("acme_");
        let plan = compute_plan(&desired, &remote, &context);
        assert!(plan
            .entries
            .iter()
            .all(|e| e.action == NetboxPlanAction::NoOp));

        // Under the default prefix the same remote state is invisible:
        // markers do not parse, natural keys are free → creates.
        let default_context = PlanContext::new(ARCH_ID, 3);
        let plan = compute_plan(&desired, &remote, &default_context);
        assert!(plan
            .entries
            .iter()
            .all(|e| e.action == NetboxPlanAction::Create));
    }
}
