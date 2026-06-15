//! Deterministic ordering of plan changes.
//!
//! The reconciler applies resources in dependency order: roles before
//! users (so role bindings can resolve), datastores before images (images
//! land on a datastore), images before templates, templates before
//! instances, and so on. Within a single resource type, action ordering
//! puts non-destructive work first and deletes last so a rolling apply
//! can converge before tearing down anything that might still be in use.
//!
//! Ordering must be **total** — given any input ordering of the same set
//! of changes, [`order_changes`] must return the same output. We use a
//! `(resource_priority, action_priority, name)` key with a stable sort to
//! achieve that. The unit tests assert this property explicitly.

use chv_controlplane_types::architecture::{PlanAction, PlanChange, PlanMode, ResourceType};

/// Sort `changes` into the canonical apply order for `mode`.
///
/// Pure function: takes ownership of the input vector to avoid an extra
/// clone, mutates it in place via stable sort, and returns it.
pub fn order_changes(mut changes: Vec<PlanChange>, mode: PlanMode) -> Vec<PlanChange> {
    changes.sort_by(|a, b| {
        let ka = (
            resource_priority(a.resource_type),
            action_priority(a.action, mode),
            a.resource_name.as_str(),
        );
        let kb = (
            resource_priority(b.resource_type),
            action_priority(b.action, mode),
            b.resource_name.as_str(),
        );
        ka.cmp(&kb)
    });
    changes
}

/// Coarse rank for a resource type. Lower numbers apply first.
///
/// Roles are first because users reference them. Users come next because
/// instance cloud-init users may reference platform users. Datastores must
/// land before networks/images/templates that reference them. Backup
/// targets are last among the modelled resources because backup policies
/// reference them.
///
/// The match is exhaustive on purpose: adding a future `ResourceType`
/// variant must fail to compile here, forcing a deliberate placement
/// decision rather than letting the new variant fall through to a
/// silent default. The exact numeric priorities are a pragmatic choice
/// — the property tests in `tests.rs` only require that ordering be
/// total and deterministic — so future variants can pick any unused
/// integer that respects the ordering invariants the comments describe.
fn resource_priority(rt: ResourceType) -> u8 {
    match rt {
        // Projects scope every later resource; build them first.
        ResourceType::Project => 5,
        // Roles must precede the users that reference them.
        ResourceType::Role => 10,
        // SSH keys are referenced by both platform users and instance users.
        ResourceType::SshKey => 15,
        // Platform users reference roles and SSH keys.
        ResourceType::User => 20,
        // Servers (physical/virtual hosts) precede the datastores and
        // networks that bind to them.
        ResourceType::Server => 25,
        // Datastores back images/volumes; they must land before consumers.
        ResourceType::Datastore => 30,
        ResourceType::Network => 40,
        ResourceType::Image => 50,
        ResourceType::Template => 60,
        ResourceType::Instance => 70,
        // Instance-scoped users follow the instance creation.
        ResourceType::InstanceUser => 75,
        // Backup targets gate backup policies.
        ResourceType::BackupTarget => 80,
        ResourceType::BackupPolicy => 90,
    }
}

/// Rank for an action under a given mode. Lower numbers apply first.
///
/// In `apply`-style modes (`Apply` / `DryRun` / `Confirm`) we apply
/// `Create` → `Update` → `Replace` → `NoOp` → `Delete`. Deletes go last
/// so callers can converge new state before removing the old.
///
/// In `Destroy` mode the diff already emits only `Delete`s, so the
/// secondary key collapses to a constant — but we keep the function total
/// so callers can mix actions safely if a future caller wishes to.
fn action_priority(a: PlanAction, mode: PlanMode) -> u8 {
    match mode {
        PlanMode::Destroy => match a {
            PlanAction::Delete => 0,
            PlanAction::Replace => 10,
            PlanAction::Update => 20,
            PlanAction::NoOp => 30,
            PlanAction::Create => 40,
        },
        PlanMode::Apply | PlanMode::DryRun | PlanMode::Confirm => match a {
            PlanAction::Create => 0,
            PlanAction::Update => 10,
            PlanAction::Replace => 20,
            PlanAction::NoOp => 30,
            PlanAction::Delete => 40,
        },
    }
}
