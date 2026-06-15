//! End-to-end smoke test for the plan/diff/order surface.
//!
//! Builds a small but realistic [`CHVArchitecture`] containing one of each
//! modelled resource, an empty inventory, and asserts that:
//!
//! 1. The plan contains exactly one Create per modelled resource.
//! 2. The order honours the priority table in [`super::order`].
//! 3. Replaying the plan against a snapshot that already reflects each
//!    modelled resource yields zero changes (idempotency).

use std::collections::BTreeMap;

use chrono::Utc;
use chv_architecture_validate::fleet::{
    BackupTargetInfo, DatastoreInfo, ImageInfo, InventorySnapshot, NetworkInfo,
};
use chv_architecture_validate::model::{
    BackupTarget, CHVArchitecture, Datastore, DatastoreType, Image, ImageFormat, Instance,
    Metadata, Network, NetworkType, Role, Template, User,
};
use chv_controlplane_types::architecture::{PlanAction, PlanMode, ResourceType};

use super::{build_plan, compute};

fn ts() -> chrono::DateTime<Utc> {
    Utc::now()
}

fn modelled_arch() -> CHVArchitecture {
    CHVArchitecture {
        api_version: "chv.kubedo.io/v1alpha1".into(),
        kind: "CHVArchitecture".into(),
        metadata: Metadata {
            name: "smoke".into(),
            display_name: None,
            description: None,
            environment: None,
            owner: None,
            labels: BTreeMap::new(),
        },
        servers: vec![],
        networks: vec![Network {
            name: "public".into(),
            network_type: NetworkType::Bridge,
            bridge: Some("br0".into()),
            vlan_id: None,
            cidr: None,
            gateway: None,
            dns: vec![],
            dhcp: None,
        }],
        datastores: vec![Datastore {
            name: "ds-a".into(),
            datastore_type: DatastoreType::Qcow2Dir,
            path: None,
            pool: None,
            capabilities: None,
            secret_ref: None,
        }],
        backup_targets: vec![BackupTarget {
            name: "offsite".into(),
            target_type: "s3".into(),
            endpoint: None,
            datastore: None,
            user: None,
            secret_ref: None,
        }],
        backup_policies: vec![],
        images: vec![Image {
            name: "ubuntu".into(),
            source: "https://example/disk.qcow2".into(),
            format: ImageFormat::Qcow2,
            datastore: None,
        }],
        templates: vec![Template {
            name: "small".into(),
            image: "ubuntu".into(),
            cpu: Some(2),
            memory_mb: Some(2048),
            disk_gb: Some(20),
            datastore: None,
            network: None,
        }],
        instances: vec![Instance {
            name: "app-01".into(),
            template: Some("small".into()),
            placement: None,
            resources: None,
            disks: vec![],
            networks: vec![],
            cloud_init: None,
            backup: None,
            tags: vec![],
        }],
        ssh_keys: vec![],
        instance_users: vec![],
        roles: vec![Role {
            name: "admin".into(),
            permissions: vec!["*".into()],
        }],
        users: vec![User {
            name: "alice".into(),
            display_name: None,
            email: None,
            auth: None,
            password: None,
            token: None,
            roles: vec!["admin".into()],
        }],
        projects: vec![],
    }
}

fn empty_inv() -> InventorySnapshot {
    InventorySnapshot {
        captured_at: ts(),
        source: "smoke".into(),
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

#[test]
fn first_apply_emits_create_per_modelled_resource_in_canonical_order() {
    let arch = modelled_arch();
    let inv = empty_inv();
    let diff = compute(&arch, &inv, PlanMode::Apply);

    // 1 role + 1 user + 1 datastore + 1 network + 1 image + 1 template
    // + 1 instance + 1 backup_target = 8 changes.
    assert_eq!(diff.changes.len(), 8, "{:?}", diff.changes);

    let order: Vec<ResourceType> = diff.changes.iter().map(|c| c.resource_type).collect();
    assert_eq!(
        order,
        vec![
            ResourceType::Role,
            ResourceType::User,
            ResourceType::Datastore,
            ResourceType::Network,
            ResourceType::Image,
            ResourceType::Template,
            ResourceType::Instance,
            ResourceType::BackupTarget,
        ]
    );

    for c in &diff.changes {
        assert_eq!(c.action, PlanAction::Create);
    }
}

#[test]
fn second_apply_with_matching_snapshot_yields_no_changes() {
    let arch = modelled_arch();
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

    // Templates, instances, users, roles are NOT yet tracked by snapshots;
    // they will still emit creates. Drop them from the model so the test
    // genuinely asserts the snapshot-suppression path for the four types
    // that ARE tracked.
    let mut arch_pruned = arch.clone();
    arch_pruned.templates.clear();
    arch_pruned.instances.clear();
    arch_pruned.users.clear();
    arch_pruned.roles.clear();

    let diff = compute(&arch_pruned, &inv, PlanMode::Apply);
    assert!(
        diff.changes.is_empty(),
        "expected no changes, got: {:?}",
        diff.changes
    );
}

#[test]
fn build_plan_carries_summary_and_warnings() {
    let arch = modelled_arch();
    let inv = empty_inv();
    let warnings = vec!["plan was generated against a stale snapshot".into()];
    let plan = build_plan(&arch, &inv, PlanMode::Apply, warnings.clone());
    assert_eq!(plan.mode, PlanMode::Apply);
    assert_eq!(plan.changes.len(), 8);
    assert_eq!(plan.summary.create, 8);
    assert_eq!(plan.summary.delete, 0);
    assert_eq!(plan.summary.warnings, 1);
    assert_eq!(plan.warnings, warnings);
}
