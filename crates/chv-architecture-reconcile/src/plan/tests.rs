//! Unit and property tests for the [`super::diff::compute`] / [`super::order::order_changes`]
//! / [`super::is_expired`] surface.

use std::collections::BTreeMap;

use chrono::{Duration, TimeZone, Utc};
use chv_architecture_validate::fleet::{
    BackupTargetInfo, DatastoreInfo, ImageInfo, InventorySnapshot, NetworkInfo,
};
use chv_architecture_validate::model::{
    BackupTarget, CHVArchitecture, Datastore, DatastoreType, Image, ImageFormat, Instance,
    Metadata, Network, NetworkType,
};
use chv_common::clock::ManualClock;
use chv_controlplane_types::architecture::{
    ArchitectureId, ArchitecturePlan, ArchitecturePlanId, ArchitectureVersionId, PlanAction,
    PlanChange, PlanMode, PlanStatus, ResourceType, Risk,
};
use proptest::prelude::*;

use super::diff::compute;
use super::is_expired;
use super::order::order_changes;

fn empty_arch() -> CHVArchitecture {
    CHVArchitecture {
        api_version: "chv.kubedo.io/v1alpha1".into(),
        kind: "CHVArchitecture".into(),
        metadata: Metadata {
            name: "tests".into(),
            display_name: None,
            description: None,
            environment: None,
            owner: None,
            labels: BTreeMap::new(),
        },
        servers: vec![],
        networks: vec![],
        datastores: vec![],
        backup_targets: vec![],
        backup_policies: vec![],
        images: vec![],
        templates: vec![],
        instances: vec![],
        ssh_keys: vec![],
        instance_users: vec![],
        roles: vec![],
        users: vec![],
        projects: vec![],
    }
}

fn empty_inv() -> InventorySnapshot {
    InventorySnapshot {
        captured_at: Utc::now(),
        source: "test".into(),
        nodes: vec![],
        networks: vec![],
        datastores: vec![],
        images: vec![],
        backup_targets: vec![],
        backup_targets_complete: true,
        secrets: vec![],
        secrets_complete: true,
        network_facts_complete: true,
        deploy_allowed: true,
    }
}

fn net(name: &str) -> Network {
    Network {
        name: name.into(),
        network_type: NetworkType::Bridge,
        bridge: Some("br0".into()),
        vlan_id: None,
        cidr: None,
        gateway: None,
        dns: vec![],
        dhcp: None,
    }
}

fn ds(name: &str) -> Datastore {
    Datastore {
        name: name.into(),
        datastore_type: DatastoreType::Qcow2Dir,
        path: None,
        pool: None,
        capabilities: None,
        secret_ref: None,
    }
}

fn img(name: &str) -> Image {
    Image {
        name: name.into(),
        source: "https://example/disk.qcow2".into(),
        format: ImageFormat::Qcow2,
        datastore: None,
    }
}

fn instance(name: &str) -> Instance {
    Instance {
        name: name.into(),
        template: None,
        placement: None,
        resources: None,
        disks: vec![],
        networks: vec![],
        cloud_init: None,
        backup: None,
        tags: vec![],
    }
}

fn bt(name: &str) -> BackupTarget {
    BackupTarget {
        name: name.into(),
        target_type: "s3".into(),
        endpoint: None,
        datastore: None,
        user: None,
        secret_ref: None,
    }
}

// ------------------ unit tests -------------------------------

#[test]
fn empty_model_and_snapshot_yield_no_changes() {
    let arch = empty_arch();
    let inv = empty_inv();
    let diff = compute(&arch, &inv, PlanMode::Apply);
    assert!(diff.changes.is_empty());
}

#[test]
fn one_network_in_model_yields_one_create() {
    let mut arch = empty_arch();
    arch.networks.push(net("public"));
    let inv = empty_inv();
    let diff = compute(&arch, &inv, PlanMode::Apply);
    assert_eq!(diff.changes.len(), 1);
    let c = &diff.changes[0];
    assert_eq!(c.action, PlanAction::Create);
    assert_eq!(c.resource_type, ResourceType::Network);
    assert_eq!(c.resource_name, "public");
    assert_eq!(c.resource_ref, "network/public");
    assert_eq!(c.risk, Risk::Low);
    assert!(!c.requires_confirmation);
    assert!(c.description.contains("Create"));
    assert!(c.description.contains("public"));
}

#[test]
fn destroy_mode_emits_delete_for_every_desired_resource() {
    let mut arch = empty_arch();
    arch.networks.push(net("net-a"));
    arch.datastores.push(ds("ds-a"));
    arch.instances.push(instance("inst-a"));
    let inv = empty_inv();
    let diff = compute(&arch, &inv, PlanMode::Destroy);
    assert_eq!(diff.changes.len(), 3);
    for c in &diff.changes {
        assert_eq!(c.action, PlanAction::Delete);
        assert_eq!(c.risk, Risk::Destructive);
        assert!(c.requires_confirmation, "destroy must require confirmation");
    }
}

#[test]
fn apply_orders_datastores_before_instances() {
    let mut arch = empty_arch();
    // Insert in inverted order to prove it is the diff that orders, not input.
    arch.instances.push(instance("inst-a"));
    arch.datastores.push(ds("ds-a"));
    let inv = empty_inv();
    let diff = compute(&arch, &inv, PlanMode::Apply);
    assert_eq!(diff.changes.len(), 2);
    assert_eq!(diff.changes[0].resource_type, ResourceType::Datastore);
    assert_eq!(diff.changes[1].resource_type, ResourceType::Instance);
}

#[test]
fn snapshot_match_suppresses_create_for_modelled_resources() {
    let mut arch = empty_arch();
    arch.networks.push(net("public"));
    arch.datastores.push(ds("ds-a"));
    arch.images.push(img("ubuntu"));
    arch.backup_targets.push(bt("offsite"));
    let mut inv = empty_inv();
    inv.networks.push(NetworkInfo {
        name: "public".into(),
        bridge: Some("br0".into()),
        vlan_id: None,
        cidr: None,
    });
    inv.datastores.push(DatastoreInfo {
        name: "ds-a".into(),
        kind: "qcow2-dir".into(),
        capacity_gb: 100,
        free_gb: 50,
        host: None,
    });
    inv.images.push(ImageInfo {
        name: "ubuntu".into(),
        format: "qcow2".into(),
    });
    inv.backup_targets.push(BackupTargetInfo {
        name: "offsite".into(),
        reachable: true,
    });

    let diff = compute(&arch, &inv, PlanMode::Apply);
    assert!(
        diff.changes.is_empty(),
        "expected no changes when snapshot already has every modelled resource, got {:?}",
        diff.changes
    );
}

#[test]
fn risk_table_matches_action() {
    use super::diff::compute;
    let mut arch = empty_arch();
    arch.networks.push(net("n-1"));
    let inv = empty_inv();
    // Apply -> Create -> Low / no confirmation
    let diff = compute(&arch, &inv, PlanMode::Apply);
    assert_eq!(diff.changes[0].risk, Risk::Low);
    assert!(!diff.changes[0].requires_confirmation);
    // Destroy -> Delete -> Destructive / requires confirmation
    let diff = compute(&arch, &inv, PlanMode::Destroy);
    assert_eq!(diff.changes[0].risk, Risk::Destructive);
    assert!(diff.changes[0].requires_confirmation);
}

// ------------------ is_expired tests -------------------------

fn sample_plan(expires_at: chrono::DateTime<Utc>) -> ArchitecturePlan {
    ArchitecturePlan {
        id: ArchitecturePlanId::new("plan-1").unwrap(),
        architecture_id: ArchitectureId::new("topo-1").unwrap(),
        architecture_version_id: ArchitectureVersionId::new("v-1").unwrap(),
        inventory_snapshot_id: None,
        mode: PlanMode::Apply,
        status: PlanStatus::RequiresConfirmation,
        plan_json: None,
        summary_json: None,
        created_by: None,
        created_at: expires_at - Duration::minutes(15),
        expires_at,
        confirmed_at: None,
        confirmed_by: None,
        discarded_at: None,
        discarded_by: None,
    }
}

#[test]
fn is_expired_false_before_ttl() {
    let now = Utc.with_ymd_and_hms(2026, 6, 13, 10, 0, 0).unwrap();
    let clock = ManualClock::new(now);
    let plan = sample_plan(now + Duration::minutes(5));
    assert!(!is_expired(&plan, &clock));
}

#[test]
fn is_expired_true_after_ttl() {
    let now = Utc.with_ymd_and_hms(2026, 6, 13, 10, 16, 0).unwrap();
    let clock = ManualClock::new(now);
    let plan = sample_plan(now - Duration::minutes(1));
    assert!(is_expired(&plan, &clock));
}

#[test]
fn is_expired_clock_advance_flips_state() {
    let now = Utc.with_ymd_and_hms(2026, 6, 13, 10, 0, 0).unwrap();
    let clock = ManualClock::new(now);
    let plan = sample_plan(now + Duration::minutes(15));
    assert!(!is_expired(&plan, &clock));
    clock.advance(Duration::minutes(15) + Duration::seconds(1));
    assert!(is_expired(&plan, &clock));
}

// ------------------ property tests --------------------------

/// Build a synthetic InventorySnapshot that includes every modelled
/// resource of the architecture so that a second `compute` call returns
/// no changes — modelling "the apply just landed" idempotency.
fn snapshot_for(arch: &CHVArchitecture) -> InventorySnapshot {
    let mut inv = empty_inv();
    for n in &arch.networks {
        inv.networks.push(NetworkInfo {
            name: n.name.clone(),
            bridge: None,
            vlan_id: None,
            cidr: None,
        });
    }
    for d in &arch.datastores {
        inv.datastores.push(DatastoreInfo {
            name: d.name.clone(),
            kind: "qcow2-dir".into(),
            capacity_gb: 0,
            free_gb: 0,
            host: None,
        });
    }
    for i in &arch.images {
        inv.images.push(ImageInfo {
            name: i.name.clone(),
            format: "qcow2".into(),
        });
    }
    for b in &arch.backup_targets {
        inv.backup_targets.push(BackupTargetInfo {
            name: b.name.clone(),
            reachable: true,
        });
    }
    inv
}

fn name_strategy() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9-]{0,8}".prop_map(|s| s.to_string())
}

prop_compose! {
    fn arb_arch()(
        networks in proptest::collection::vec(name_strategy(), 0..5),
        datastores in proptest::collection::vec(name_strategy(), 0..5),
        images in proptest::collection::vec(name_strategy(), 0..5),
        backup_targets in proptest::collection::vec(name_strategy(), 0..3),
    ) -> CHVArchitecture {
        let mut arch = empty_arch();
        // Dedupe to satisfy Phase-1 schema invariant (unique names).
        let mut seen = std::collections::HashSet::new();
        for n in networks { if seen.insert(("n", n.clone())) { arch.networks.push(net(&n)); } }
        for d in datastores { if seen.insert(("d", d.clone())) { arch.datastores.push(ds(&d)); } }
        for i in images { if seen.insert(("i", i.clone())) { arch.images.push(img(&i)); } }
        for b in backup_targets { if seen.insert(("b", b.clone())) { arch.backup_targets.push(bt(&b)); } }
        arch
    }
}

proptest! {
    /// Idempotency at the diff level: applying a plan once and running
    /// `compute` again with a snapshot that reflects the apply yields zero
    /// changes.
    #[test]
    fn applying_plan_twice_yields_empty(arch in arb_arch()) {
        let inv_before = empty_inv();
        let first = compute(&arch, &inv_before, PlanMode::Apply);
        // Apply: assume the snapshot now reflects every modelled resource.
        let inv_after = snapshot_for(&arch);
        let second = compute(&arch, &inv_after, PlanMode::Apply);
        prop_assert!(second.changes.is_empty(),
            "second compute should be empty after apply; first.len={} second.len={}",
            first.changes.len(), second.changes.len());
    }
}

/// Synthesize a vector of arbitrary PlanChange records and shuffle it many
/// ways; assert `order_changes` is total — every shuffle produces the
/// same output.
///
/// The corpus deliberately includes several entries that share the same
/// `(resource_type, action)` primary key but differ in `resource_name`.
/// Without those, the secondary tie-break by name is never exercised and
/// "totality" would only be proved up to the point where the primary key
/// is unique. The dup block below has 5 same-key entries (Create / Network)
/// plus three more pairs (Create / Instance, Update / Network, Delete / Image)
/// so multiple primary keys collide on the secondary sort.
#[test]
fn order_is_total_and_deterministic() {
    let base: Vec<PlanChange> = vec![
        // Distinct primary keys covering the full type space.
        mk_change(PlanAction::Create, ResourceType::Instance, "z"),
        mk_change(PlanAction::Create, ResourceType::Datastore, "ds-1"),
        mk_change(PlanAction::Update, ResourceType::Network, "net-1"),
        mk_change(PlanAction::Delete, ResourceType::Image, "img-1"),
        mk_change(PlanAction::Create, ResourceType::Role, "admin"),
        mk_change(PlanAction::Create, ResourceType::User, "alice"),
        mk_change(PlanAction::NoOp, ResourceType::Template, "t-1"),
        mk_change(PlanAction::Replace, ResourceType::Instance, "a"),
        mk_change(PlanAction::Create, ResourceType::Network, "net-1"),
        mk_change(PlanAction::Create, ResourceType::Datastore, "ds-2"),
        mk_change(PlanAction::Create, ResourceType::BackupTarget, "off"),
        // Duplicate-keyed block: 5 entries sharing (Create, Network),
        // forcing the (resource_name) tie-break to drive total ordering.
        mk_change(PlanAction::Create, ResourceType::Network, "net-aaa"),
        mk_change(PlanAction::Create, ResourceType::Network, "net-bbb"),
        mk_change(PlanAction::Create, ResourceType::Network, "net-ccc"),
        mk_change(PlanAction::Create, ResourceType::Network, "net-ddd"),
        mk_change(PlanAction::Create, ResourceType::Network, "net-eee"),
        // Additional duplicate-keyed pairs across other types/actions so
        // the tie-break is exercised at multiple points in the canonical
        // order, not just one.
        mk_change(PlanAction::Create, ResourceType::Instance, "inst-x"),
        mk_change(PlanAction::Create, ResourceType::Instance, "inst-y"),
        mk_change(PlanAction::Update, ResourceType::Network, "net-2"),
        mk_change(PlanAction::Update, ResourceType::Network, "net-3"),
        mk_change(PlanAction::Delete, ResourceType::Image, "img-2"),
        mk_change(PlanAction::Delete, ResourceType::Image, "img-3"),
    ];

    let canonical = order_changes(base.clone(), PlanMode::Apply);

    // Sanity: the canonical order must place all five Create/Network
    // duplicates contiguously and in name-ascending order.
    let net_creates: Vec<&str> = canonical
        .iter()
        .filter(|c| c.resource_type == ResourceType::Network && c.action == PlanAction::Create)
        .map(|c| c.resource_name.as_str())
        .collect();
    assert_eq!(
        net_creates,
        vec!["net-1", "net-aaa", "net-bbb", "net-ccc", "net-ddd", "net-eee"],
        "duplicate-keyed entries must tie-break on resource_name"
    );

    // 100 deterministic shuffles using a tiny LCG.
    let mut state: u64 = 0xdead_beef_cafe_d00d;
    for _ in 0..100 {
        let mut shuffled = base.clone();
        for i in (1..shuffled.len()).rev() {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let j = (state as usize) % (i + 1);
            shuffled.swap(i, j);
        }
        let ordered = order_changes(shuffled, PlanMode::Apply);
        assert_eq!(
            ordered, canonical,
            "order_changes is not total: shuffle produced different output"
        );
    }
}

fn mk_change(action: PlanAction, rt: ResourceType, name: &str) -> PlanChange {
    PlanChange {
        action,
        resource_type: rt,
        resource_name: name.into(),
        resource_ref: format!("{:?}/{name}", rt),
        description: format!("{:?} {:?} {name}", action, rt),
        risk: Risk::Low,
        requires_confirmation: false,
    }
}
