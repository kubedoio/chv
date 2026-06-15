//! Pure drift detection algorithm.
//!
//! [`compute_drift`] takes a baseline architecture document and a live
//! [`InventorySnapshot`] and returns a [`DriftReport`] enumerating
//! deviations. The function is **pure**: no I/O, no clocks, no async; given
//! the same inputs it always returns equal output.
//!
//! # Ordering invariant
//!
//! Findings are emitted in this stable, deterministic order so equality
//! tests (and hence the `idempotency` test) hold:
//!
//! 1. `MissingResource` — in baseline order: networks, then datastores,
//!    then images.
//! 2. `UnexpectedResource` — in snapshot order: networks, then datastores,
//!    then images.
//! 3. `FieldChanged` — baseline order: datastores (by index), then images
//!    (by index).
//! 4. `CapacityChanged` — baseline order: servers (by index, fields in
//!    `cpu_cores` then `memory_gb` order), then datastores (by index, fields
//!    in `capacity_gb` then `free_gb` order).
//! 5. `NetworkChanged` — baseline order: networks (by index, fields in
//!    `bridge`, `vlan_id`, `cidr` order).
//! 6. `PermissionChanged` — at most one finding.
//! 7. `AttachmentChanged` — baseline order: instances (by index, attachments
//!    by index).

use chv_architecture_validate::fleet::InventorySnapshot;
use chv_architecture_validate::model::CHVArchitecture;
use chv_controlplane_types::architecture::DriftStatus;
use std::collections::{BTreeMap, HashMap, HashSet};

use super::types::{DriftFinding, DriftReport, DriftSummary};

/// Compute drift findings for a baseline architecture against a live fleet
/// snapshot.
///
/// See the module-level docs for the ordering invariant findings respect.
pub fn compute_drift(baseline: &CHVArchitecture, snapshot: &InventorySnapshot) -> DriftReport {
    let mut findings: Vec<DriftFinding> = Vec::new();

    // Build name indices into the snapshot once. The snapshot is small
    // (cluster-scale, hundreds of items at most) so HashMap lookup wins.
    let snap_networks: HashMap<&str, &chv_architecture_validate::fleet::NetworkInfo> = snapshot
        .networks
        .iter()
        .map(|n| (n.name.as_str(), n))
        .collect();
    let snap_datastores: HashMap<&str, &chv_architecture_validate::fleet::DatastoreInfo> = snapshot
        .datastores
        .iter()
        .map(|d| (d.name.as_str(), d))
        .collect();
    let snap_images: HashMap<&str, &chv_architecture_validate::fleet::ImageInfo> = snapshot
        .images
        .iter()
        .map(|i| (i.name.as_str(), i))
        .collect();
    let snap_nodes: HashMap<&str, &chv_architecture_validate::fleet::NodeInfo> = snapshot
        .nodes
        .iter()
        .map(|n| (n.name.as_str(), n))
        .collect();

    // Reverse: baseline name sets, used for UnexpectedResource detection.
    let base_network_names: HashSet<&str> =
        baseline.networks.iter().map(|n| n.name.as_str()).collect();
    let base_datastore_names: HashSet<&str> = baseline
        .datastores
        .iter()
        .map(|d| d.name.as_str())
        .collect();
    let base_image_names: HashSet<&str> = baseline.images.iter().map(|i| i.name.as_str()).collect();

    // === 1. MissingResource ===========================================
    for (idx, net) in baseline.networks.iter().enumerate() {
        if !snap_networks.contains_key(net.name.as_str()) {
            findings.push(DriftFinding::MissingResource {
                path: format!("networks[{idx}]"),
                resource_ref: format!("network/{}", net.name),
                message: format!(
                    "network '{}' is declared in the baseline but absent from the live fleet",
                    net.name
                ),
            });
        }
    }
    for (idx, ds) in baseline.datastores.iter().enumerate() {
        if !snap_datastores.contains_key(ds.name.as_str()) {
            findings.push(DriftFinding::MissingResource {
                path: format!("datastores[{idx}]"),
                resource_ref: format!("datastore/{}", ds.name),
                message: format!(
                    "datastore '{}' is declared in the baseline but absent from the live fleet",
                    ds.name
                ),
            });
        }
    }
    for (idx, img) in baseline.images.iter().enumerate() {
        if !snap_images.contains_key(img.name.as_str()) {
            findings.push(DriftFinding::MissingResource {
                path: format!("images[{idx}]"),
                resource_ref: format!("image/{}", img.name),
                message: format!(
                    "image '{}' is declared in the baseline but absent from the live fleet",
                    img.name
                ),
            });
        }
    }

    // === 2. UnexpectedResource =======================================
    for net in &snapshot.networks {
        if !base_network_names.contains(net.name.as_str()) {
            findings.push(DriftFinding::UnexpectedResource {
                path: format!("<<live>>/networks/{}", net.name),
                resource_ref: format!("network/{}", net.name),
                message: format!(
                    "network '{}' exists in the live fleet but is not declared in the baseline",
                    net.name
                ),
            });
        }
    }
    for ds in &snapshot.datastores {
        if !base_datastore_names.contains(ds.name.as_str()) {
            findings.push(DriftFinding::UnexpectedResource {
                path: format!("<<live>>/datastores/{}", ds.name),
                resource_ref: format!("datastore/{}", ds.name),
                message: format!(
                    "datastore '{}' exists in the live fleet but is not declared in the baseline",
                    ds.name
                ),
            });
        }
    }
    for img in &snapshot.images {
        if !base_image_names.contains(img.name.as_str()) {
            findings.push(DriftFinding::UnexpectedResource {
                path: format!("<<live>>/images/{}", img.name),
                resource_ref: format!("image/{}", img.name),
                message: format!(
                    "image '{}' exists in the live fleet but is not declared in the baseline",
                    img.name
                ),
            });
        }
    }

    // === 3. FieldChanged =============================================
    // Datastore.kind: baseline uses a typed enum we serialize via serde to
    // get the wire form; live uses a free-form String. Compare wire forms.
    for (idx, ds) in baseline.datastores.iter().enumerate() {
        if let Some(live) = snap_datastores.get(ds.name.as_str()) {
            let expected = datastore_kind_wire(&ds.datastore_type);
            if expected != live.kind {
                findings.push(DriftFinding::FieldChanged {
                    path: format!("datastores[{idx}].type"),
                    resource_ref: format!("datastore/{}", ds.name),
                    field: "kind".to_string(),
                    expected: expected.to_string(),
                    actual: live.kind.clone(),
                    message: format!(
                        "datastore '{}' kind changed: baseline='{}', live='{}'",
                        ds.name, expected, live.kind
                    ),
                });
            }
        }
    }
    for (idx, img) in baseline.images.iter().enumerate() {
        if let Some(live) = snap_images.get(img.name.as_str()) {
            let expected = image_format_wire(&img.format);
            if expected != live.format {
                findings.push(DriftFinding::FieldChanged {
                    path: format!("images[{idx}].format"),
                    resource_ref: format!("image/{}", img.name),
                    field: "format".to_string(),
                    expected: expected.to_string(),
                    actual: live.format.clone(),
                    message: format!(
                        "image '{}' format changed: baseline='{}', live='{}'",
                        img.name, expected, live.format
                    ),
                });
            }
        }
    }

    // === 4. CapacityChanged ==========================================
    // Server resources versus snapshot node specs. We only emit when the
    // baseline declares a value (Some) — an unset baseline is interpreted
    // as "I don't care", not "must be 0".
    for (idx, server) in baseline.servers.iter().enumerate() {
        let Some(live_node) = snap_nodes.get(server.name.as_str()) else {
            continue;
        };
        let Some(resources) = &server.resources else {
            continue;
        };
        if let Some(expected_cpu) = resources.cpu_cores {
            if i64::from(expected_cpu) != i64::from(live_node.cpu_cores) {
                findings.push(DriftFinding::CapacityChanged {
                    path: format!("servers[{idx}].resources.cpu_cores"),
                    resource_ref: format!("server/{}", server.name),
                    field: "cpu_cores".to_string(),
                    expected: i64::from(expected_cpu),
                    actual: i64::from(live_node.cpu_cores),
                    message: format!(
                        "server '{}' cpu_cores changed: baseline={}, live={}",
                        server.name, expected_cpu, live_node.cpu_cores
                    ),
                });
            }
        }
        if let Some(expected_mem) = resources.memory_gb {
            if i64::from(expected_mem) != i64::from(live_node.memory_gb) {
                findings.push(DriftFinding::CapacityChanged {
                    path: format!("servers[{idx}].resources.memory_gb"),
                    resource_ref: format!("server/{}", server.name),
                    field: "memory_gb".to_string(),
                    expected: i64::from(expected_mem),
                    actual: i64::from(live_node.memory_gb),
                    message: format!(
                        "server '{}' memory_gb changed: baseline={}, live={}",
                        server.name, expected_mem, live_node.memory_gb
                    ),
                });
            }
        }
    }
    // Datastore capacity / free. The baseline schema does not currently
    // carry capacity numbers, so for MVP we have nothing to compare on this
    // axis. We keep the loop scaffolded so a future schema addition slots
    // straight in. (See docs/specs/architecture-designer/contracts/yaml-contract.md.)
    // Intentionally no findings emitted here today.

    // === 5. NetworkChanged ===========================================
    for (idx, net) in baseline.networks.iter().enumerate() {
        let Some(live) = snap_networks.get(net.name.as_str()) else {
            continue;
        };

        // Compare bridge.
        if net.bridge != live.bridge {
            findings.push(DriftFinding::NetworkChanged {
                path: format!("networks[{idx}].bridge"),
                resource_ref: format!("network/{}", net.name),
                field: "bridge".to_string(),
                expected: net.bridge.clone(),
                actual: live.bridge.clone(),
                message: format!(
                    "network '{}' bridge changed: baseline={}, live={}",
                    net.name,
                    render_opt(&net.bridge),
                    render_opt(&live.bridge)
                ),
            });
        }

        // Compare vlan_id (stringify for the on-the-wire shape).
        let expected_vlan = net.vlan_id.map(|v| v.to_string());
        let actual_vlan = live.vlan_id.map(|v| v.to_string());
        if expected_vlan != actual_vlan {
            findings.push(DriftFinding::NetworkChanged {
                path: format!("networks[{idx}].vlan_id"),
                resource_ref: format!("network/{}", net.name),
                field: "vlan_id".to_string(),
                expected: expected_vlan.clone(),
                actual: actual_vlan.clone(),
                message: format!(
                    "network '{}' vlan_id changed: baseline={}, live={}",
                    net.name,
                    render_opt(&expected_vlan),
                    render_opt(&actual_vlan)
                ),
            });
        }

        // Compare cidr.
        if net.cidr != live.cidr {
            findings.push(DriftFinding::NetworkChanged {
                path: format!("networks[{idx}].cidr"),
                resource_ref: format!("network/{}", net.name),
                field: "cidr".to_string(),
                expected: net.cidr.clone(),
                actual: live.cidr.clone(),
                message: format!(
                    "network '{}' cidr changed: baseline={}, live={}",
                    net.name,
                    render_opt(&net.cidr),
                    render_opt(&live.cidr)
                ),
            });
        }
    }

    // === 6. PermissionChanged ========================================
    // Heuristic: emit iff the baseline has any role/permission expectations
    // AND the live caller has lost the deploy permission. This avoids
    // flagging every snapshot where deploy is disabled regardless of
    // baseline intent.
    let baseline_expects_permissions =
        !baseline.roles.is_empty() || baseline.users.iter().any(|u| !u.roles.is_empty());
    if baseline_expects_permissions && !snapshot.deploy_allowed {
        findings.push(DriftFinding::PermissionChanged {
            path: "<<permissions>>".to_string(),
            resource_ref: String::new(),
            message:
                "caller no longer holds the architecture:apply permission required by the baseline"
                    .to_string(),
        });
    }

    // === 7. AttachmentChanged ========================================
    // Pragmatic MVP: flag instance.networks attachments whose target
    // network is missing from the live snapshot. The richer "placement
    // server moved" check needs a per-node instance list which the
    // snapshot does not carry today (see InventorySnapshot in
    // chv-architecture-validate::fleet::inventory).
    for (idx, instance) in baseline.instances.iter().enumerate() {
        let mut missing_attachments: Vec<&str> = Vec::new();
        for attachment in &instance.networks {
            if !snap_networks.contains_key(attachment.name.as_str()) {
                missing_attachments.push(attachment.name.as_str());
            }
        }
        if !missing_attachments.is_empty() {
            findings.push(DriftFinding::AttachmentChanged {
                path: format!("instances[{idx}].networks"),
                resource_ref: format!("instance/{}", instance.name),
                message: format!(
                    "instance '{}' references network(s) [{}] that are missing from the live fleet",
                    instance.name,
                    missing_attachments.join(", ")
                ),
            });
        }
    }

    tracing::debug!(
        target: "chv_architecture_reconcile::drift",
        findings_count = findings.len(),
        "computed drift report"
    );

    let total = findings.len() as i64;
    let mut by_type: BTreeMap<String, i64> = BTreeMap::new();
    for f in &findings {
        *by_type.entry(f.code().to_string()).or_insert(0) += 1;
    }
    let status = if findings.is_empty() {
        DriftStatus::NoDrift
    } else {
        DriftStatus::Drifted
    };

    DriftReport {
        status,
        findings,
        summary: DriftSummary { total, by_type },
    }
}

// --- helpers -----------------------------------------------------------

/// Wire form of `DatastoreType` matching the YAML enum (`kebab-case`).
fn datastore_kind_wire(kind: &chv_architecture_validate::model::DatastoreType) -> &'static str {
    use chv_architecture_validate::model::DatastoreType as T;
    match kind {
        T::Qcow2Dir => "qcow2-dir",
        T::CephRbd => "ceph-rbd",
        T::Nfs => "nfs",
        T::Lvm => "lvm",
        T::Zfs => "zfs",
        T::Unknown => "unknown",
    }
}

/// Wire form of `ImageFormat` matching the YAML enum (`lowercase`).
fn image_format_wire(format: &chv_architecture_validate::model::ImageFormat) -> &'static str {
    use chv_architecture_validate::model::ImageFormat as F;
    match format {
        F::Qcow2 => "qcow2",
        F::Raw => "raw",
        F::Unknown => "unknown",
    }
}

/// Render an optional value for human-readable messages. `None` becomes the
/// literal `<unset>` so log lines stay greppable.
fn render_opt(v: &Option<String>) -> String {
    match v {
        Some(s) => format!("'{s}'"),
        None => "<unset>".to_string(),
    }
}
