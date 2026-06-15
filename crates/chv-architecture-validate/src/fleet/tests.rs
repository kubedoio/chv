//! Unit tests for fleet checks. One per finding code (13) plus a happy
//! path and an `ALL_CODES` membership test.

use chrono::Utc;
use chv_controlplane_types::architecture::Severity;
use std::collections::BTreeMap;

use crate::codes;
use crate::fleet::{
    check_fleet, BackupTargetInfo, DatastoreInfo, ImageInfo, InventorySnapshot, NetworkInfo,
    NodeInfo,
};
use crate::model::{
    CHVArchitecture, Datastore, DatastoreType, Image, ImageFormat, Instance, InstanceDisk,
    InstanceNetwork, InstancePlacement, InstanceResources, Metadata, Network, NetworkType,
    Template,
};

fn empty_arch(name: &str) -> CHVArchitecture {
    CHVArchitecture {
        api_version: "chv.kubedo.io/v1alpha1".into(),
        kind: "CHVArchitecture".into(),
        metadata: Metadata {
            name: name.into(),
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
    // Defaults to "fully complete" so per-test setup is minimal — tests for
    // the incomplete-inventory severity downgrade explicitly flip the
    // relevant `*_complete` flag to false.
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

fn ok_node(name: &str) -> NodeInfo {
    NodeInfo {
        name: name.into(),
        schedulable: true,
        cpu_cores: 16,
        memory_gb: 64,
        bridges: vec!["br0".into()],
        vlans: vec![100],
        used_ips: vec![],
    }
}

fn instance_on(name: &str, host: &str) -> Instance {
    Instance {
        name: name.into(),
        template: None,
        placement: Some(InstancePlacement {
            server: Some(host.into()),
        }),
        resources: None,
        disks: vec![],
        networks: vec![],
        cloud_init: None,
        backup: None,
        tags: vec![],
    }
}

fn has_code(findings: &[chv_controlplane_types::architecture::Finding], code: &str) -> bool {
    findings.iter().any(|f| f.code.as_ref() == code)
}

#[test]
fn host_not_found_emitted() {
    let mut a = empty_arch("t");
    a.instances.push(instance_on("vm1", "missing"));
    let inv = empty_inv();
    let f = check_fleet(&a, &inv);
    assert!(has_code(&f, codes::HOST_NOT_FOUND));
    assert!(!has_code(&f, codes::HOST_NOT_SCHEDULABLE));
}

#[test]
fn host_not_schedulable_emitted() {
    let mut a = empty_arch("t");
    a.instances.push(instance_on("vm1", "host1"));
    let mut inv = empty_inv();
    let mut node = ok_node("host1");
    node.schedulable = false;
    inv.nodes.push(node);
    let f = check_fleet(&a, &inv);
    assert!(has_code(&f, codes::HOST_NOT_SCHEDULABLE));
    assert!(!has_code(&f, codes::HOST_NOT_FOUND));
}

#[test]
fn insufficient_cpu_emitted() {
    let mut a = empty_arch("t");
    let mut inst = instance_on("vm1", "host1");
    inst.resources = Some(InstanceResources {
        cpu: Some(64),
        memory_mb: None,
    });
    a.instances.push(inst);
    let mut inv = empty_inv();
    inv.nodes.push(ok_node("host1"));
    let f = check_fleet(&a, &inv);
    assert!(has_code(&f, codes::INSUFFICIENT_CPU));
}

#[test]
fn insufficient_memory_emitted() {
    let mut a = empty_arch("t");
    let mut inst = instance_on("vm1", "host1");
    inst.resources = Some(InstanceResources {
        cpu: None,
        memory_mb: Some(128 * 1024),
    });
    a.instances.push(inst);
    let mut inv = empty_inv();
    inv.nodes.push(ok_node("host1"));
    let f = check_fleet(&a, &inv);
    assert!(has_code(&f, codes::INSUFFICIENT_MEMORY));
}

#[test]
fn bridge_unavailable_emitted() {
    let mut a = empty_arch("t");
    a.networks.push(Network {
        name: "n1".into(),
        network_type: NetworkType::Bridge,
        bridge: Some("br99".into()),
        vlan_id: None,
        cidr: None,
        gateway: None,
        dns: vec![],
        dhcp: None,
    });
    let mut inv = empty_inv();
    inv.nodes.push(ok_node("h1"));
    let f = check_fleet(&a, &inv);
    assert!(has_code(&f, codes::BRIDGE_UNAVAILABLE));
}

#[test]
fn vlan_unavailable_emitted() {
    let mut a = empty_arch("t");
    a.networks.push(Network {
        name: "n1".into(),
        network_type: NetworkType::Vlan,
        bridge: None,
        vlan_id: Some(4000),
        cidr: None,
        gateway: None,
        dns: vec![],
        dhcp: None,
    });
    let mut inv = empty_inv();
    inv.nodes.push(ok_node("h1"));
    let f = check_fleet(&a, &inv);
    assert!(has_code(&f, codes::VLAN_UNAVAILABLE));
}

#[test]
fn ip_already_used_emitted() {
    let mut a = empty_arch("t");
    let mut inst = instance_on("vm1", "h1");
    inst.networks.push(InstanceNetwork {
        name: "n1".into(),
        ip: Some("10.0.0.5".into()),
    });
    a.instances.push(inst);
    let mut inv = empty_inv();
    let mut node = ok_node("h1");
    node.used_ips.push("10.0.0.5".into());
    inv.nodes.push(node);
    let f = check_fleet(&a, &inv);
    assert!(has_code(&f, codes::IP_ALREADY_USED));
}

#[test]
fn network_facts_incomplete_downgrades_to_warning() {
    // When `network_facts_complete=false`, all three network checks
    // (BRIDGE_UNAVAILABLE, VLAN_UNAVAILABLE, IP_ALREADY_USED) emit as
    // non-blocking warnings rather than blocking errors.
    use chv_controlplane_types::architecture::Severity;
    let mut a = empty_arch("t");
    a.networks.push(Network {
        name: "n1".into(),
        network_type: NetworkType::Bridge,
        bridge: Some("br-missing".into()),
        vlan_id: Some(4000),
        cidr: None,
        gateway: None,
        dns: vec![],
        dhcp: None,
    });
    let mut inst = instance_on("vm1", "h1");
    inst.networks.push(InstanceNetwork {
        name: "n1".into(),
        ip: Some("10.0.0.5".into()),
    });
    a.instances.push(inst);
    let mut inv = empty_inv();
    inv.network_facts_complete = false;
    let mut node = ok_node("h1");
    node.used_ips.push("10.0.0.5".into());
    inv.nodes.push(node);
    let f = check_fleet(&a, &inv);
    for code in [
        codes::BRIDGE_UNAVAILABLE,
        codes::VLAN_UNAVAILABLE,
        codes::IP_ALREADY_USED,
    ] {
        let finding = f
            .iter()
            .find(|x| x.code.as_ref() == code)
            .unwrap_or_else(|| panic!("{code} expected in incomplete-network-facts mode"));
        assert!(
            matches!(finding.severity, Severity::Warning),
            "{code} should be Warning when network_facts_complete=false; got {:?}",
            finding.severity
        );
        assert!(!finding.blocking, "{code} should be non-blocking warning");
    }
}

#[test]
fn datastore_not_found_emitted() {
    let mut a = empty_arch("t");
    a.datastores.push(Datastore {
        name: "ds-missing".into(),
        datastore_type: DatastoreType::Qcow2Dir,
        path: None,
        pool: None,
        capabilities: None,
        secret_ref: None,
    });
    let inv = empty_inv();
    let f = check_fleet(&a, &inv);
    assert!(has_code(&f, codes::DATASTORE_NOT_FOUND));
}

#[test]
fn datastore_insufficient_capacity_emitted() {
    let mut a = empty_arch("t");
    a.datastores.push(Datastore {
        name: "ds1".into(),
        datastore_type: DatastoreType::Qcow2Dir,
        path: None,
        pool: None,
        capabilities: None,
        secret_ref: None,
    });
    let mut inst = instance_on("vm1", "h1");
    inst.disks.push(InstanceDisk {
        name: "root".into(),
        size_gb: Some(500),
        datastore: Some("ds1".into()),
    });
    a.instances.push(inst);
    let mut inv = empty_inv();
    inv.nodes.push(ok_node("h1"));
    inv.datastores.push(DatastoreInfo {
        name: "ds1".into(),
        kind: "qcow2-dir".into(),
        capacity_gb: 100,
        free_gb: 50,
        host: None,
    });
    let f = check_fleet(&a, &inv);
    assert!(has_code(&f, codes::DATASTORE_INSUFFICIENT_CAPACITY));
}

#[test]
fn image_not_found_emitted() {
    let mut a = empty_arch("t");
    a.images.push(Image {
        name: "img-missing".into(),
        source: "https://example/img.qcow2".into(),
        format: ImageFormat::Qcow2,
        datastore: None,
    });
    let inv = empty_inv();
    let f = check_fleet(&a, &inv);
    assert!(has_code(&f, codes::IMAGE_NOT_FOUND));
}

#[test]
fn backup_target_unreachable_warns_when_incomplete() {
    let mut a = empty_arch("t");
    a.backup_targets.push(crate::model::BackupTarget {
        name: "bt1".into(),
        target_type: "s3".into(),
        endpoint: None,
        datastore: None,
        user: None,
        secret_ref: None,
    });
    let mut inv = empty_inv();
    inv.backup_targets_complete = false;
    inv.backup_targets.push(BackupTargetInfo {
        name: "bt1".into(),
        reachable: false,
    });
    let f = check_fleet(&a, &inv);
    assert!(has_code(&f, codes::BACKUP_TARGET_UNREACHABLE));
    let bt = f
        .iter()
        .find(|x| x.code.as_ref() == codes::BACKUP_TARGET_UNREACHABLE)
        .unwrap();
    assert_eq!(bt.severity, Severity::Warning);
    assert!(!bt.blocking);
}

#[test]
fn backup_target_unreachable_errors_when_complete() {
    let mut a = empty_arch("t");
    a.backup_targets.push(crate::model::BackupTarget {
        name: "bt1".into(),
        target_type: "s3".into(),
        endpoint: None,
        datastore: None,
        user: None,
        secret_ref: None,
    });
    let mut inv = empty_inv();
    inv.backup_targets.push(BackupTargetInfo {
        name: "bt1".into(),
        reachable: false,
    });
    let f = check_fleet(&a, &inv);
    let bt = f
        .iter()
        .find(|x| x.code.as_ref() == codes::BACKUP_TARGET_UNREACHABLE)
        .unwrap();
    assert_eq!(bt.severity, Severity::Error);
    assert!(bt.blocking);
}

#[test]
fn secret_ref_missing_emitted_when_secret_store_complete() {
    // secrets_complete=true: SECRET_REF_MISSING is a blocking error.
    let mut a = empty_arch("t");
    a.datastores.push(Datastore {
        name: "ds1".into(),
        datastore_type: DatastoreType::CephRbd,
        path: None,
        pool: None,
        capabilities: None,
        secret_ref: Some("ceph-key".into()),
    });
    let mut inv = empty_inv();
    inv.datastores.push(DatastoreInfo {
        name: "ds1".into(),
        kind: "ceph-rbd".into(),
        capacity_gb: 1000,
        free_gb: 1000,
        host: None,
    });
    // Authoritative secret store with a different name — `ceph-key` is missing.
    inv.secrets.push(crate::fleet::SecretInfo {
        name: "other-secret".into(),
        kind: "opaque".into(),
    });
    inv.secrets_complete = true;
    let f = check_fleet(&a, &inv);
    let finding = f
        .iter()
        .find(|x| x.code.as_ref() == codes::SECRET_REF_MISSING)
        .expect("SECRET_REF_MISSING expected");
    assert!(matches!(
        finding.severity,
        chv_controlplane_types::architecture::Severity::Error
    ));
    assert!(finding.blocking);
}

#[test]
fn secret_ref_missing_warns_when_secret_store_incomplete() {
    // secrets_complete=false: same model triggers a non-blocking warning,
    // because the absence of the secret store cannot be distinguished from
    // a real missing secret.
    let mut a = empty_arch("t");
    a.datastores.push(Datastore {
        name: "ds1".into(),
        datastore_type: DatastoreType::CephRbd,
        path: None,
        pool: None,
        capabilities: None,
        secret_ref: Some("ceph-key".into()),
    });
    let mut inv = empty_inv();
    inv.secrets_complete = false;
    let f = check_fleet(&a, &inv);
    let finding = f
        .iter()
        .find(|x| x.code.as_ref() == codes::SECRET_REF_MISSING)
        .expect("SECRET_REF_MISSING expected");
    assert!(matches!(
        finding.severity,
        chv_controlplane_types::architecture::Severity::Warning
    ));
    assert!(!finding.blocking);
}

#[test]
fn secret_ref_present_in_authoritative_store_does_not_emit() {
    // When the secret IS in the authoritative store, no finding fires —
    // verifies the placeholder regression (datastore-name-as-secret-name)
    // is gone.
    let mut a = empty_arch("t");
    a.datastores.push(Datastore {
        name: "ds-ceph".into(),
        datastore_type: DatastoreType::CephRbd,
        path: None,
        pool: None,
        capabilities: None,
        secret_ref: Some("ceph-key".into()),
    });
    let mut inv = empty_inv();
    // Old placeholder logic would have suppressed the finding because the
    // datastore name matched the secret name. New logic only consults
    // `inv.secrets`.
    inv.datastores.push(DatastoreInfo {
        name: "ceph-key".into(), // intentionally same name as the secret_ref
        kind: "ceph-rbd".into(),
        capacity_gb: 1000,
        free_gb: 1000,
        host: None,
    });
    inv.secrets.push(crate::fleet::SecretInfo {
        name: "ceph-key".into(),
        kind: "opaque".into(),
    });
    inv.secrets_complete = true;
    let f = check_fleet(&a, &inv);
    assert!(
        !has_code(&f, codes::SECRET_REF_MISSING),
        "secret present in authoritative store must NOT emit SECRET_REF_MISSING; findings={f:?}"
    );
}

#[test]
fn permission_denied_deploy_emitted() {
    let a = empty_arch("t");
    let mut inv = empty_inv();
    inv.deploy_allowed = false;
    let f = check_fleet(&a, &inv);
    assert!(has_code(&f, codes::PERMISSION_DENIED_DEPLOY));
}

#[test]
fn happy_path_no_findings() {
    // A small but realistic architecture that all checks accept.
    let mut a = empty_arch("good");
    a.networks.push(Network {
        name: "n1".into(),
        network_type: NetworkType::Bridge,
        bridge: Some("br0".into()),
        vlan_id: Some(100),
        cidr: Some("10.0.0.0/24".into()),
        gateway: None,
        dns: vec![],
        dhcp: None,
    });
    a.datastores.push(Datastore {
        name: "ds1".into(),
        datastore_type: DatastoreType::Qcow2Dir,
        path: None,
        pool: None,
        capabilities: None,
        secret_ref: None,
    });
    a.images.push(Image {
        name: "ubuntu".into(),
        source: "s".into(),
        format: ImageFormat::Qcow2,
        datastore: Some("ds1".into()),
    });
    a.templates.push(Template {
        name: "small".into(),
        image: "ubuntu".into(),
        cpu: Some(2),
        memory_mb: Some(2048),
        disk_gb: Some(10),
        datastore: Some("ds1".into()),
        network: Some("n1".into()),
    });
    let mut inst = instance_on("vm1", "host1");
    inst.template = Some("small".into());
    inst.disks.push(InstanceDisk {
        name: "root".into(),
        size_gb: Some(10),
        datastore: Some("ds1".into()),
    });
    a.instances.push(inst);

    let mut inv = empty_inv();
    inv.nodes.push(ok_node("host1"));
    inv.datastores.push(DatastoreInfo {
        name: "ds1".into(),
        kind: "qcow2-dir".into(),
        capacity_gb: 1000,
        free_gb: 500,
        host: None,
    });
    inv.images.push(ImageInfo {
        name: "ubuntu".into(),
        format: "qcow2".into(),
    });
    inv.networks.push(NetworkInfo {
        name: "n1".into(),
        bridge: Some("br0".into()),
        vlan_id: Some(100),
        cidr: Some("10.0.0.0/24".into()),
    });

    let f = check_fleet(&a, &inv);
    assert!(f.is_empty(), "expected no findings, got {f:?}");
}

#[test]
fn every_emitted_code_lives_in_all_codes() {
    // Drive every check at least once and confirm each emitted code is in
    // the registry. Builds one architecture+inventory pair that triggers
    // all 13 codes.
    let mut a = empty_arch("everything");
    // HOST_NOT_FOUND
    a.instances.push(instance_on("vm-missing-host", "ghost"));
    // HOST_NOT_SCHEDULABLE
    a.instances.push(instance_on("vm-cordoned", "cordoned"));
    // INSUFFICIENT_CPU + INSUFFICIENT_MEMORY (two separate instances)
    let mut big_cpu = instance_on("vm-big-cpu", "small");
    big_cpu.resources = Some(InstanceResources {
        cpu: Some(64),
        memory_mb: None,
    });
    a.instances.push(big_cpu);
    let mut big_mem = instance_on("vm-big-mem", "small");
    big_mem.resources = Some(InstanceResources {
        cpu: None,
        memory_mb: Some(128 * 1024),
    });
    a.instances.push(big_mem);
    // BRIDGE_UNAVAILABLE
    a.networks.push(Network {
        name: "bad-bridge".into(),
        network_type: NetworkType::Bridge,
        bridge: Some("nonexistent".into()),
        vlan_id: None,
        cidr: None,
        gateway: None,
        dns: vec![],
        dhcp: None,
    });
    // VLAN_UNAVAILABLE
    a.networks.push(Network {
        name: "bad-vlan".into(),
        network_type: NetworkType::Vlan,
        bridge: None,
        vlan_id: Some(9999),
        cidr: None,
        gateway: None,
        dns: vec![],
        dhcp: None,
    });
    // IP_ALREADY_USED
    let mut conflict = instance_on("vm-ip-conflict", "small");
    conflict.networks.push(InstanceNetwork {
        name: "bad-bridge".into(),
        ip: Some("10.0.0.99".into()),
    });
    a.instances.push(conflict);
    // DATASTORE_NOT_FOUND
    a.datastores.push(Datastore {
        name: "ghost-ds".into(),
        datastore_type: DatastoreType::Qcow2Dir,
        path: None,
        pool: None,
        capabilities: None,
        secret_ref: None,
    });
    // DATASTORE_INSUFFICIENT_CAPACITY
    a.datastores.push(Datastore {
        name: "tight-ds".into(),
        datastore_type: DatastoreType::Qcow2Dir,
        path: None,
        pool: None,
        capabilities: None,
        secret_ref: None,
    });
    let mut hungry = instance_on("vm-hungry", "small");
    hungry.disks.push(InstanceDisk {
        name: "root".into(),
        size_gb: Some(9000),
        datastore: Some("tight-ds".into()),
    });
    a.instances.push(hungry);
    // IMAGE_NOT_FOUND
    a.images.push(Image {
        name: "ghost-img".into(),
        source: "s".into(),
        format: ImageFormat::Qcow2,
        datastore: None,
    });
    // BACKUP_TARGET_UNREACHABLE
    a.backup_targets.push(crate::model::BackupTarget {
        name: "bt-down".into(),
        target_type: "s3".into(),
        endpoint: None,
        datastore: None,
        user: None,
        secret_ref: Some("nope-secret".into()), // SECRET_REF_MISSING
    });

    // Build the inventory.
    let mut inv = empty_inv();
    let mut small = ok_node("small");
    small.cpu_cores = 4;
    small.memory_gb = 4;
    small.bridges = vec!["br0".into()];
    small.vlans = vec![100];
    small.used_ips = vec!["10.0.0.99".into()];
    inv.nodes.push(small);
    let mut cordoned = ok_node("cordoned");
    cordoned.schedulable = false;
    inv.nodes.push(cordoned);
    inv.datastores.push(DatastoreInfo {
        name: "tight-ds".into(),
        kind: "qcow2-dir".into(),
        capacity_gb: 100,
        free_gb: 10,
        host: None,
    });
    inv.backup_targets.push(BackupTargetInfo {
        name: "bt-down".into(),
        reachable: false,
    });
    inv.deploy_allowed = false; // PERMISSION_DENIED_DEPLOY

    let findings = check_fleet(&a, &inv);
    let expected = [
        codes::HOST_NOT_FOUND,
        codes::HOST_NOT_SCHEDULABLE,
        codes::INSUFFICIENT_CPU,
        codes::INSUFFICIENT_MEMORY,
        codes::BRIDGE_UNAVAILABLE,
        codes::VLAN_UNAVAILABLE,
        codes::IP_ALREADY_USED,
        codes::DATASTORE_NOT_FOUND,
        codes::DATASTORE_INSUFFICIENT_CAPACITY,
        codes::IMAGE_NOT_FOUND,
        codes::BACKUP_TARGET_UNREACHABLE,
        codes::SECRET_REF_MISSING,
        codes::PERMISSION_DENIED_DEPLOY,
    ];
    for c in expected {
        assert!(has_code(&findings, c), "expected code {c} missing");
        assert!(
            codes::ALL_CODES.contains(&c),
            "code {c} not in ALL_CODES registry"
        );
    }

    // Every emitted code must also appear in ALL_CODES.
    for f in &findings {
        let code: &str = f.code.as_ref();
        assert!(
            codes::ALL_CODES.contains(&code),
            "emitted code {code} is not registered"
        );
    }
}
