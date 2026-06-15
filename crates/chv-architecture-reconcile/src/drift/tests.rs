//! Drift detection unit tests.
//!
//! One fixture per finding type, plus idempotency, no-drift, and
//! serde-round-trip coverage. All fixtures construct minimal baseline +
//! snapshot pairs to keep failure messages readable.

use super::types::{DriftFinding, DriftReport, DriftSummary};
use super::{compute_drift, DriftStatus};

use chrono::{DateTime, Utc};
use chv_architecture_validate::fleet::{
    DatastoreInfo, ImageInfo, InventorySnapshot, NetworkInfo, NodeInfo,
};
use chv_architecture_validate::model::{
    CHVArchitecture, Datastore, DatastoreType, Image, ImageFormat, Instance, InstanceNetwork,
    InstancePlacement, Metadata, Network, NetworkType, Role, Server, ServerResources, User,
};

fn ts() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-06-15T12:00:00Z")
        .expect("static rfc3339 timestamp parses")
        .with_timezone(&Utc)
}

fn empty_baseline() -> CHVArchitecture {
    CHVArchitecture {
        api_version: "chv.kubedo.io/v1alpha1".to_string(),
        kind: "CHVArchitecture".to_string(),
        metadata: Metadata {
            name: "test".to_string(),
            display_name: None,
            description: None,
            environment: None,
            owner: None,
            labels: Default::default(),
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

fn empty_snapshot() -> InventorySnapshot {
    InventorySnapshot {
        captured_at: ts(),
        source: "test".to_string(),
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

fn bridge_network(name: &str) -> Network {
    Network {
        name: name.to_string(),
        network_type: NetworkType::Bridge,
        bridge: Some("br0".to_string()),
        vlan_id: None,
        cidr: Some("10.0.0.0/24".to_string()),
        gateway: None,
        dns: vec![],
        dhcp: None,
    }
}

fn live_network(name: &str) -> NetworkInfo {
    NetworkInfo {
        name: name.to_string(),
        bridge: Some("br0".to_string()),
        vlan_id: None,
        cidr: Some("10.0.0.0/24".to_string()),
    }
}

fn nfs_datastore(name: &str) -> Datastore {
    Datastore {
        name: name.to_string(),
        datastore_type: DatastoreType::Nfs,
        path: None,
        pool: None,
        capabilities: None,
        secret_ref: None,
    }
}

fn live_datastore(name: &str, kind: &str) -> DatastoreInfo {
    DatastoreInfo {
        name: name.to_string(),
        kind: kind.to_string(),
        capacity_gb: 100,
        free_gb: 50,
        host: None,
    }
}

fn qcow2_image(name: &str) -> Image {
    Image {
        name: name.to_string(),
        source: "http://example.com/img".to_string(),
        format: ImageFormat::Qcow2,
        datastore: None,
    }
}

fn live_image(name: &str, format: &str) -> ImageInfo {
    ImageInfo {
        name: name.to_string(),
        format: format.to_string(),
    }
}

fn live_node(name: &str, cpu_cores: u32, memory_gb: u32) -> NodeInfo {
    NodeInfo {
        name: name.to_string(),
        schedulable: true,
        cpu_cores,
        memory_gb,
        bridges: vec![],
        vlans: vec![],
        used_ips: vec![],
    }
}

// --- 1. MissingResource ------------------------------------------------

#[test]
fn compute_drift_emits_missing_resource_when_baseline_network_absent() {
    let mut baseline = empty_baseline();
    baseline.networks.push(bridge_network("public"));
    let snapshot = empty_snapshot();

    let report = compute_drift(&baseline, &snapshot);
    assert_eq!(report.status, DriftStatus::Drifted);
    assert_eq!(report.findings.len(), 1);
    match &report.findings[0] {
        DriftFinding::MissingResource {
            path, resource_ref, ..
        } => {
            assert_eq!(path, "networks[0]");
            assert_eq!(resource_ref, "network/public");
        }
        other => panic!("expected MissingResource, got {other:?}"),
    }
    assert_eq!(report.findings[0].code(), "DRIFT_MISSING_RESOURCE");
    assert_eq!(report.summary.total, 1);
    assert_eq!(
        report.summary.by_type.get("DRIFT_MISSING_RESOURCE"),
        Some(&1)
    );
}

// --- 2. UnexpectedResource ---------------------------------------------

#[test]
fn compute_drift_emits_unexpected_resource_when_live_has_extra_datastore() {
    let baseline = empty_baseline();
    let mut snapshot = empty_snapshot();
    snapshot.datastores.push(live_datastore("rogue", "nfs"));

    let report = compute_drift(&baseline, &snapshot);
    assert_eq!(report.findings.len(), 1);
    match &report.findings[0] {
        DriftFinding::UnexpectedResource {
            path, resource_ref, ..
        } => {
            assert_eq!(path, "<<live>>/datastores/rogue");
            assert_eq!(resource_ref, "datastore/rogue");
        }
        other => panic!("expected UnexpectedResource, got {other:?}"),
    }
    assert_eq!(report.findings[0].code(), "DRIFT_UNEXPECTED_RESOURCE");
    assert_eq!(
        report.summary.by_type.get("DRIFT_UNEXPECTED_RESOURCE"),
        Some(&1)
    );
}

// --- 3. FieldChanged ---------------------------------------------------

#[test]
fn compute_drift_emits_field_changed_when_datastore_kind_differs() {
    let mut baseline = empty_baseline();
    baseline.datastores.push(nfs_datastore("primary"));
    let mut snapshot = empty_snapshot();
    snapshot.datastores.push(live_datastore("primary", "lvm"));

    let report = compute_drift(&baseline, &snapshot);
    assert_eq!(report.findings.len(), 1);
    match &report.findings[0] {
        DriftFinding::FieldChanged {
            path,
            resource_ref,
            field,
            expected,
            actual,
            ..
        } => {
            assert_eq!(path, "datastores[0].type");
            assert_eq!(resource_ref, "datastore/primary");
            assert_eq!(field, "kind");
            assert_eq!(expected, "nfs");
            assert_eq!(actual, "lvm");
        }
        other => panic!("expected FieldChanged, got {other:?}"),
    }
    assert_eq!(report.findings[0].code(), "DRIFT_FIELD_CHANGED");
    assert_eq!(report.summary.by_type.get("DRIFT_FIELD_CHANGED"), Some(&1));
}

// --- 4. CapacityChanged ------------------------------------------------

#[test]
fn compute_drift_emits_capacity_changed_when_node_cpu_cores_differ() {
    let mut baseline = empty_baseline();
    baseline.servers.push(Server {
        name: "node-a".to_string(),
        management_ip: None,
        role: None,
        labels: Default::default(),
        resources: Some(ServerResources {
            cpu_cores: Some(8),
            memory_gb: Some(32),
        }),
        networks: None,
    });
    let mut snapshot = empty_snapshot();
    // Memory matches (32), CPU differs (4 vs 8) — only one finding.
    snapshot.nodes.push(live_node("node-a", 4, 32));

    let report = compute_drift(&baseline, &snapshot);
    assert_eq!(report.findings.len(), 1);
    match &report.findings[0] {
        DriftFinding::CapacityChanged {
            path,
            resource_ref,
            field,
            expected,
            actual,
            ..
        } => {
            assert_eq!(path, "servers[0].resources.cpu_cores");
            assert_eq!(resource_ref, "server/node-a");
            assert_eq!(field, "cpu_cores");
            assert_eq!(*expected, 8);
            assert_eq!(*actual, 4);
        }
        other => panic!("expected CapacityChanged, got {other:?}"),
    }
    assert_eq!(report.findings[0].code(), "DRIFT_CAPACITY_CHANGED");
    assert_eq!(
        report.summary.by_type.get("DRIFT_CAPACITY_CHANGED"),
        Some(&1)
    );
}

// --- 5. NetworkChanged -------------------------------------------------

#[test]
fn compute_drift_emits_network_changed_when_bridge_differs() {
    let mut baseline = empty_baseline();
    baseline.networks.push(bridge_network("public"));
    let mut snapshot = empty_snapshot();
    let mut live = live_network("public");
    live.bridge = Some("br1".to_string()); // baseline expects br0
    snapshot.networks.push(live);

    let report = compute_drift(&baseline, &snapshot);
    assert_eq!(report.findings.len(), 1);
    match &report.findings[0] {
        DriftFinding::NetworkChanged {
            path,
            resource_ref,
            field,
            expected,
            actual,
            ..
        } => {
            assert_eq!(path, "networks[0].bridge");
            assert_eq!(resource_ref, "network/public");
            assert_eq!(field, "bridge");
            assert_eq!(expected.as_deref(), Some("br0"));
            assert_eq!(actual.as_deref(), Some("br1"));
        }
        other => panic!("expected NetworkChanged, got {other:?}"),
    }
    assert_eq!(report.findings[0].code(), "DRIFT_NETWORK_CHANGED");
    assert_eq!(
        report.summary.by_type.get("DRIFT_NETWORK_CHANGED"),
        Some(&1)
    );
}

// --- 6. PermissionChanged ----------------------------------------------

#[test]
fn compute_drift_emits_permission_changed_when_deploy_allowed_false() {
    let mut baseline = empty_baseline();
    baseline.roles.push(Role {
        name: "operator".to_string(),
        permissions: vec!["architecture:apply".to_string()],
    });
    baseline.users.push(User {
        name: "alice".to_string(),
        display_name: None,
        email: None,
        auth: None,
        password: None,
        token: None,
        roles: vec!["operator".to_string()],
    });
    let mut snapshot = empty_snapshot();
    snapshot.deploy_allowed = false;

    let report = compute_drift(&baseline, &snapshot);
    assert_eq!(report.findings.len(), 1);
    match &report.findings[0] {
        DriftFinding::PermissionChanged {
            path, resource_ref, ..
        } => {
            assert_eq!(path, "<<permissions>>");
            assert_eq!(resource_ref, "");
        }
        other => panic!("expected PermissionChanged, got {other:?}"),
    }
    assert_eq!(report.findings[0].code(), "DRIFT_PERMISSION_CHANGED");
    assert_eq!(
        report.summary.by_type.get("DRIFT_PERMISSION_CHANGED"),
        Some(&1)
    );
}

#[test]
fn compute_drift_does_not_emit_permission_changed_when_baseline_has_no_roles() {
    // Negative case for the heuristic in `compute_drift`: when the baseline
    // declares no roles AND no user role bindings, a live `deploy_allowed=
    // false` snapshot must NOT emit PermissionChanged. The heuristic is
    // gated on the baseline having permission expectations in the first
    // place — otherwise every snapshot where deploy is disabled would
    // light up the badge.
    let baseline = empty_baseline();
    let mut snapshot = empty_snapshot();
    snapshot.deploy_allowed = false;

    let report = compute_drift(&baseline, &snapshot);
    assert!(
        report
            .findings
            .iter()
            .all(|f| !matches!(f, DriftFinding::PermissionChanged { .. })),
        "PermissionChanged must not fire when baseline has no role expectations"
    );
}

#[test]
fn compute_drift_does_not_emit_permission_changed_when_deploy_still_allowed() {
    // Negative case: baseline has roles bound to users, but the live caller
    // still has the deploy permission. PermissionChanged must NOT fire.
    let mut baseline = empty_baseline();
    baseline.roles.push(Role {
        name: "operator".to_string(),
        permissions: vec!["architecture:apply".to_string()],
    });
    baseline.users.push(User {
        name: "alice".to_string(),
        display_name: None,
        email: None,
        auth: None,
        password: None,
        token: None,
        roles: vec!["operator".to_string()],
    });
    let mut snapshot = empty_snapshot();
    snapshot.deploy_allowed = true;

    let report = compute_drift(&baseline, &snapshot);
    assert!(
        report
            .findings
            .iter()
            .all(|f| !matches!(f, DriftFinding::PermissionChanged { .. })),
        "PermissionChanged must not fire while deploy is still allowed"
    );
}

// --- 7. AttachmentChanged ----------------------------------------------

#[test]
fn compute_drift_emits_attachment_changed_when_instance_network_attachment_missing() {
    let mut baseline = empty_baseline();
    // We need a baseline-declared network to avoid MissingResource also firing.
    baseline.networks.push(bridge_network("public"));
    baseline.instances.push(Instance {
        name: "vm-1".to_string(),
        template: None,
        placement: Some(InstancePlacement {
            server: Some("node-a".to_string()),
        }),
        resources: None,
        disks: vec![],
        networks: vec![InstanceNetwork {
            name: "public".to_string(), // matches baseline network
            ip: None,
        }],
        cloud_init: None,
        backup: None,
        tags: vec![],
    });
    // Snapshot is missing the 'public' network entirely. Both
    // MissingResource (for the network) and AttachmentChanged (for the
    // instance) will fire — assert AttachmentChanged is among them with
    // exact path/resource_ref.
    let snapshot = empty_snapshot();

    let report = compute_drift(&baseline, &snapshot);
    let attachment = report
        .findings
        .iter()
        .find(|f| matches!(f, DriftFinding::AttachmentChanged { .. }))
        .expect("attachment_changed finding present");
    match attachment {
        DriftFinding::AttachmentChanged {
            path, resource_ref, ..
        } => {
            assert_eq!(path, "instances[0].networks");
            assert_eq!(resource_ref, "instance/vm-1");
        }
        _ => unreachable!(),
    }
    assert_eq!(attachment.code(), "DRIFT_ATTACHMENT_CHANGED");
    assert_eq!(
        report.summary.by_type.get("DRIFT_ATTACHMENT_CHANGED"),
        Some(&1)
    );
}

// --- Idempotency -------------------------------------------------------

#[test]
fn compute_drift_is_pure_repeated_calls_yield_identical_findings() {
    // Build a fixture that triggers all 7 finding codes simultaneously so
    // we can pin both determinism (r1 == r2) AND the documented emission
    // order in one test. The ordering invariant is documented in
    // `compute.rs` module docs and the contract spec.
    let mut baseline = empty_baseline();

    // (1) MissingResource: baseline declares 'public' network; live drops it.
    baseline.networks.push(bridge_network("public"));
    // (5) NetworkChanged: baseline declares 'lan'; live has 'lan' with a
    //     different bridge.
    let mut lan = bridge_network("lan");
    lan.bridge = Some("br0".to_string());
    baseline.networks.push(lan);
    // (3) FieldChanged on datastore.kind: baseline 'primary' is nfs, live is lvm.
    baseline.datastores.push(nfs_datastore("primary"));
    baseline.images.push(qcow2_image("ubuntu"));
    // (4) CapacityChanged: baseline declares cpu=8 for node-a; live has 4.
    baseline.servers.push(Server {
        name: "node-a".to_string(),
        management_ip: None,
        role: None,
        labels: Default::default(),
        resources: Some(ServerResources {
            cpu_cores: Some(8),
            memory_gb: Some(32),
        }),
        networks: None,
    });
    // (6) PermissionChanged: baseline expects roles+users.
    baseline.roles.push(Role {
        name: "operator".to_string(),
        permissions: vec!["architecture:apply".to_string()],
    });
    baseline.users.push(User {
        name: "alice".to_string(),
        display_name: None,
        email: None,
        auth: None,
        password: None,
        token: None,
        roles: vec!["operator".to_string()],
    });
    // (7) AttachmentChanged: instance references 'missing-net' (not in live).
    baseline.instances.push(Instance {
        name: "vm-1".to_string(),
        template: None,
        placement: Some(InstancePlacement {
            server: Some("node-a".to_string()),
        }),
        resources: None,
        disks: vec![],
        networks: vec![InstanceNetwork {
            name: "missing-net".to_string(),
            ip: None,
        }],
        cloud_init: None,
        backup: None,
        tags: vec![],
    });

    let mut snapshot = empty_snapshot();
    // Live 'lan' with mismatched bridge.
    snapshot.networks.push({
        let mut n = live_network("lan");
        n.bridge = Some("br1".to_string());
        n
    });
    // (2) UnexpectedResource: live 'rogue' datastore not in baseline.
    snapshot.datastores.push(live_datastore("rogue", "nfs"));
    // FieldChanged on primary datastore.kind.
    snapshot.datastores.push(live_datastore("primary", "lvm"));
    // FieldChanged on image.format covered by 'ubuntu' raw.
    snapshot.images.push(live_image("ubuntu", "raw"));
    // CapacityChanged: live node-a cpu=4 (baseline expected 8).
    snapshot.nodes.push(live_node("node-a", 4, 32));
    // PermissionChanged: deploy_allowed=false.
    snapshot.deploy_allowed = false;

    let r1 = compute_drift(&baseline, &snapshot);
    let r2 = compute_drift(&baseline, &snapshot);
    assert_eq!(r1, r2, "compute_drift must be deterministic");

    // All 7 finding codes must be present.
    let mut seen: std::collections::BTreeSet<&'static str> = Default::default();
    for f in &r1.findings {
        seen.insert(f.code());
    }
    for code in [
        "DRIFT_MISSING_RESOURCE",
        "DRIFT_UNEXPECTED_RESOURCE",
        "DRIFT_FIELD_CHANGED",
        "DRIFT_CAPACITY_CHANGED",
        "DRIFT_NETWORK_CHANGED",
        "DRIFT_PERMISSION_CHANGED",
        "DRIFT_ATTACHMENT_CHANGED",
    ] {
        assert!(
            seen.contains(code),
            "expected {code} in findings, got codes={:?}",
            r1.findings.iter().map(|f| f.code()).collect::<Vec<_>>()
        );
    }

    // Group order: each code's first occurrence must follow the documented
    // order. Within a group there may be multiple findings (e.g. two
    // FieldChanged entries) but the group boundaries respect the contract.
    let order_of: std::collections::HashMap<&'static str, usize> = [
        "DRIFT_MISSING_RESOURCE",
        "DRIFT_UNEXPECTED_RESOURCE",
        "DRIFT_FIELD_CHANGED",
        "DRIFT_CAPACITY_CHANGED",
        "DRIFT_NETWORK_CHANGED",
        "DRIFT_PERMISSION_CHANGED",
        "DRIFT_ATTACHMENT_CHANGED",
    ]
    .into_iter()
    .enumerate()
    .map(|(i, c)| (c, i))
    .collect();
    let mut highest_seen: usize = 0;
    for f in &r1.findings {
        let pos = *order_of.get(f.code()).expect("known code");
        assert!(
            pos >= highest_seen,
            "findings must be emitted in the documented group order; \
             {} (idx {pos}) appeared after a code with idx {highest_seen}: {:?}",
            f.code(),
            r1.findings.iter().map(|f| f.code()).collect::<Vec<_>>()
        );
        highest_seen = pos;
    }
}

// --- No drift ---------------------------------------------------------

#[test]
fn compute_drift_returns_no_drift_when_baseline_matches_snapshot() {
    let mut baseline = empty_baseline();
    baseline.networks.push(bridge_network("public"));
    baseline.datastores.push(nfs_datastore("primary"));
    baseline.images.push(qcow2_image("ubuntu"));

    let mut snapshot = empty_snapshot();
    snapshot.networks.push(live_network("public"));
    snapshot.datastores.push(live_datastore("primary", "nfs"));
    snapshot.images.push(live_image("ubuntu", "qcow2"));

    let report = compute_drift(&baseline, &snapshot);
    assert_eq!(report.status, DriftStatus::NoDrift);
    assert!(
        report.findings.is_empty(),
        "expected zero findings, got {:?}",
        report.findings
    );
    assert_eq!(report.summary.total, 0);
    assert!(report.summary.by_type.is_empty());
}

// --- Serde round-trip --------------------------------------------------

#[test]
fn drift_finding_round_trip_serializes_with_code_tag() {
    let variants: Vec<DriftFinding> = vec![
        DriftFinding::MissingResource {
            path: "networks[0]".to_string(),
            resource_ref: "network/a".to_string(),
            message: "m".to_string(),
        },
        DriftFinding::UnexpectedResource {
            path: "<<live>>/datastores/x".to_string(),
            resource_ref: "datastore/x".to_string(),
            message: "m".to_string(),
        },
        DriftFinding::FieldChanged {
            path: "datastores[0].type".to_string(),
            resource_ref: "datastore/x".to_string(),
            field: "kind".to_string(),
            expected: "nfs".to_string(),
            actual: "lvm".to_string(),
            message: "m".to_string(),
        },
        DriftFinding::CapacityChanged {
            path: "servers[0].resources.cpu_cores".to_string(),
            resource_ref: "server/n".to_string(),
            field: "cpu_cores".to_string(),
            expected: 8,
            actual: 4,
            message: "m".to_string(),
        },
        DriftFinding::NetworkChanged {
            path: "networks[0].bridge".to_string(),
            resource_ref: "network/a".to_string(),
            field: "bridge".to_string(),
            expected: Some("br0".to_string()),
            actual: Some("br1".to_string()),
            message: "m".to_string(),
        },
        DriftFinding::PermissionChanged {
            path: "<<permissions>>".to_string(),
            resource_ref: "".to_string(),
            message: "m".to_string(),
        },
        DriftFinding::AttachmentChanged {
            path: "instances[0].networks".to_string(),
            resource_ref: "instance/v".to_string(),
            message: "m".to_string(),
        },
    ];

    for v in variants {
        let json = serde_json::to_string(&v).expect("serialize");
        // Tag must appear as the 'code' field with the variant's stable code.
        let expected_code = v.code();
        assert!(
            json.contains(&format!("\"code\":\"{expected_code}\"")),
            "serialized form must carry code tag '{expected_code}': {json}"
        );
        let back: DriftFinding = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(v, back, "round-trip equality for {expected_code}");
    }
}

#[test]
fn drift_report_round_trip_preserves_summary_histogram() {
    let report = DriftReport {
        status: DriftStatus::Drifted,
        findings: vec![DriftFinding::MissingResource {
            path: "networks[0]".to_string(),
            resource_ref: "network/a".to_string(),
            message: "m".to_string(),
        }],
        summary: DriftSummary {
            total: 1,
            by_type: [("DRIFT_MISSING_RESOURCE".to_string(), 1)]
                .into_iter()
                .collect(),
        },
    };
    let json = serde_json::to_string(&report).expect("serialize");
    let back: DriftReport = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(report, back);
}
