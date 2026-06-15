//! Datastore presence and capacity fit.

use std::borrow::Cow;
use std::collections::HashMap;

use chv_controlplane_types::architecture::{Finding, Severity};

use crate::codes;
use crate::fleet::InventorySnapshot;
use crate::model::CHVArchitecture;

pub(super) fn check(model: &CHVArchitecture, inv: &InventorySnapshot) -> Vec<Finding> {
    let mut findings = Vec::new();

    let live: HashMap<&str, &crate::fleet::DatastoreInfo> = inv
        .datastores
        .iter()
        .map(|d| (d.name.as_str(), d))
        .collect();

    // Aggregate planned consumption per datastore from instance disks.
    // Templates contribute their disk_gb default to instances that adopt
    // them; instance disk overrides win where present.
    let mut planned_gb: HashMap<&str, u64> = HashMap::new();

    // First, surface DATASTORE_NOT_FOUND for direct references.
    for (idx, ds) in model.datastores.iter().enumerate() {
        if !live.contains_key(ds.name.as_str()) {
            findings.push(Finding {
                severity: Severity::Error,
                code: Cow::Borrowed(codes::DATASTORE_NOT_FOUND),
                message: format!(
                    "declared datastore {} not present in fleet inventory",
                    ds.name
                ),
                path: Some(format!("datastores[{idx}].name")),
                resource_ref: Some(format!("datastore/{}", ds.name)),
                blocking: true,
                suggestion: Some(
                    "provision the datastore on a host before deploying this architecture".into(),
                ),
            });
        }
    }

    // Walk instances. For each disk, determine target datastore (instance
    // override → template default) and accumulate.
    for (idx, inst) in model.instances.iter().enumerate() {
        let template = inst
            .template
            .as_deref()
            .and_then(|n| model.templates.iter().find(|t| t.name == n));

        for (didx, disk) in inst.disks.iter().enumerate() {
            let ds_name = disk
                .datastore
                .as_deref()
                .or_else(|| template.and_then(|t| t.datastore.as_deref()));
            let Some(ds_name) = ds_name else { continue };
            if !live.contains_key(ds_name) {
                findings.push(Finding {
                    severity: Severity::Error,
                    code: Cow::Borrowed(codes::DATASTORE_NOT_FOUND),
                    message: format!(
                        "instance {} disk {} targets datastore {} which is not in the fleet",
                        inst.name, disk.name, ds_name
                    ),
                    path: Some(format!("instances[{idx}].disks[{didx}].datastore")),
                    resource_ref: Some(format!("instance/{}", inst.name)),
                    blocking: true,
                    suggestion: Some("declare the datastore or pick an existing one".into()),
                });
                continue;
            }
            let size = u64::from(
                disk.size_gb
                    .or_else(|| template.and_then(|t| t.disk_gb))
                    .unwrap_or(0),
            );
            *planned_gb.entry(ds_name).or_insert(0) += size;
        }
    }

    // Capacity check: per-datastore sum vs free.
    for (ds_name, planned) in &planned_gb {
        if let Some(ds) = live.get(ds_name) {
            if *planned > ds.free_gb {
                findings.push(Finding {
                    severity: Severity::Error,
                    code: Cow::Borrowed(codes::DATASTORE_INSUFFICIENT_CAPACITY),
                    message: format!(
                        "datastore {} has {} GB free but architecture plans {} GB",
                        ds.name, ds.free_gb, planned
                    ),
                    path: None,
                    resource_ref: Some(format!("datastore/{}", ds.name)),
                    blocking: true,
                    suggestion: Some(
                        "shrink instance disks, expand the datastore, or split across datastores"
                            .into(),
                    ),
                });
            }
        }
    }

    findings
}
