//! Pure-data diff between a desired [`CHVArchitecture`] and a captured
//! [`InventorySnapshot`].
//!
//! Phase-4 scope:
//!
//! * **Networks, datastores, images, backup_targets** are diffed by name
//!   against the snapshot's authoritative lists. Anything in `desired` that
//!   is missing from the snapshot becomes a `Create`.
//! * **Instances, templates, users, roles** are not yet carried by
//!   [`InventorySnapshot`] — every desired entry of these kinds becomes a
//!   `Create`. Phase 5 introduces field-level updates.
//! * **Destroy mode** inverts the model: every desired resource becomes a
//!   `Delete`. The snapshot is not consulted; the BFF treats destroy as
//!   "tear down whatever this architecture asserts ownership of".
//!
//! The output is fed through [`super::order::order_changes`] before being
//! returned, so callers can rely on a stable apply order.

use chv_architecture_validate::{fleet::InventorySnapshot, model::CHVArchitecture};
use chv_controlplane_types::architecture::{PlanAction, PlanChange, PlanMode, ResourceType, Risk};

use super::order::order_changes;

/// Result of [`compute`]. A thin wrapper around `Vec<PlanChange>` kept as a
/// distinct type so future enrichments (e.g. cycle warnings) have a place
/// to live without breaking callers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diff {
    /// Ordered list of changes.
    pub changes: Vec<PlanChange>,
}

/// Compute the [`Diff`] between `desired` and `snapshot` under `mode`.
///
/// Pure function: no I/O, deterministic, allocator-only.
pub fn compute(desired: &CHVArchitecture, snapshot: &InventorySnapshot, mode: PlanMode) -> Diff {
    let action = primary_action(mode);
    let mut changes: Vec<PlanChange> = Vec::new();

    // Snapshot lookup sets — only consulted for `Apply`/`DryRun`/`Confirm`.
    // `Destroy` ignores the snapshot.
    let consult_snapshot = !matches!(mode, PlanMode::Destroy);

    // Networks
    for n in &desired.networks {
        if consult_snapshot && snapshot.networks.iter().any(|i| i.name == n.name) {
            continue;
        }
        changes.push(make_change(action, ResourceType::Network, &n.name));
    }

    // Datastores
    for d in &desired.datastores {
        if consult_snapshot && snapshot.datastores.iter().any(|i| i.name == d.name) {
            continue;
        }
        changes.push(make_change(action, ResourceType::Datastore, &d.name));
    }

    // Images
    for img in &desired.images {
        if consult_snapshot && snapshot.images.iter().any(|i| i.name == img.name) {
            continue;
        }
        changes.push(make_change(action, ResourceType::Image, &img.name));
    }

    // Backup targets
    for t in &desired.backup_targets {
        if consult_snapshot && snapshot.backup_targets.iter().any(|i| i.name == t.name) {
            continue;
        }
        changes.push(make_change(action, ResourceType::BackupTarget, &t.name));
    }

    // Templates — snapshot does not yet track templates; emit unconditionally
    // (subject to mode).
    for t in &desired.templates {
        changes.push(make_change(action, ResourceType::Template, &t.name));
    }

    // Instances — snapshot does not yet track instances; emit unconditionally.
    for i in &desired.instances {
        changes.push(make_change(action, ResourceType::Instance, &i.name));
    }

    // Users — snapshot does not yet track platform users.
    for u in &desired.users {
        changes.push(make_change(action, ResourceType::User, &u.name));
    }

    // Roles — snapshot does not yet track roles.
    for r in &desired.roles {
        changes.push(make_change(action, ResourceType::Role, &r.name));
    }

    Diff {
        changes: order_changes(changes, mode),
    }
}

fn primary_action(mode: PlanMode) -> PlanAction {
    match mode {
        PlanMode::Destroy => PlanAction::Delete,
        // Apply / DryRun / Confirm all emit "first-apply" Creates in Phase 4.
        PlanMode::Apply | PlanMode::DryRun | PlanMode::Confirm => PlanAction::Create,
    }
}

fn make_change(action: PlanAction, rt: ResourceType, name: &str) -> PlanChange {
    let risk = risk_for(action);
    let requires_confirmation = matches!(action, PlanAction::Delete | PlanAction::Replace);
    PlanChange {
        action,
        resource_type: rt,
        resource_name: name.to_string(),
        resource_ref: format!("{}/{name}", resource_type_slug(rt)),
        description: describe(action, rt, name),
        risk,
        requires_confirmation,
    }
}

fn risk_for(action: PlanAction) -> Risk {
    match action {
        PlanAction::Create => Risk::Low,
        PlanAction::Update => Risk::Medium,
        PlanAction::Replace => Risk::High,
        PlanAction::Delete => Risk::Destructive,
        PlanAction::NoOp => Risk::Low,
    }
}

fn resource_type_slug(rt: ResourceType) -> &'static str {
    match rt {
        ResourceType::Server => "server",
        ResourceType::Network => "network",
        ResourceType::Datastore => "datastore",
        ResourceType::BackupTarget => "backup_target",
        ResourceType::BackupPolicy => "backup_policy",
        ResourceType::Image => "image",
        ResourceType::Template => "template",
        ResourceType::Instance => "instance",
        ResourceType::SshKey => "ssh_key",
        ResourceType::InstanceUser => "instance_user",
        ResourceType::Role => "role",
        ResourceType::User => "user",
        ResourceType::Project => "project",
    }
}

fn describe(action: PlanAction, rt: ResourceType, name: &str) -> String {
    let verb = match action {
        PlanAction::Create => "Create",
        PlanAction::Update => "Update",
        PlanAction::Replace => "Replace",
        PlanAction::Delete => "Delete",
        PlanAction::NoOp => "No-op for",
    };
    format!("{verb} {} {name}", resource_type_slug(rt).replace('_', " "))
}
