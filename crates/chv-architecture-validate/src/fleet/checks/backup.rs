//! Backup target reachability and missing-secret checks.
//!
//! `BACKUP_TARGET_UNREACHABLE` is emitted as a **warning** when the snapshot
//! reports `backup_targets_complete = false` (BackupTargetRepository is
//! still a stub). It upgrades to **error** once the inventory is
//! authoritative — at that point an unreachable target is a hard blocker
//! for deploys that depend on it.

use std::borrow::Cow;
use std::collections::HashMap;

use chv_controlplane_types::architecture::{Finding, Severity};

use crate::codes;
use crate::fleet::InventorySnapshot;
use crate::model::CHVArchitecture;

pub(super) fn check(model: &CHVArchitecture, inv: &InventorySnapshot) -> Vec<Finding> {
    let mut findings = Vec::new();

    let live: HashMap<&str, bool> = inv
        .backup_targets
        .iter()
        .map(|b| (b.name.as_str(), b.reachable))
        .collect();

    let downgrade = !inv.backup_targets_complete;

    for (idx, target) in model.backup_targets.iter().enumerate() {
        let reachable = live.get(target.name.as_str()).copied();
        match reachable {
            Some(true) => {}
            Some(false) | None => {
                let (severity, blocking) = if downgrade {
                    (Severity::Warning, false)
                } else {
                    (Severity::Error, true)
                };
                findings.push(Finding {
                    severity,
                    code: Cow::Borrowed(codes::BACKUP_TARGET_UNREACHABLE),
                    message: format!(
                        "backup target {} is unreachable ({} inventory)",
                        target.name,
                        if downgrade {
                            "best-effort"
                        } else {
                            "authoritative"
                        }
                    ),
                    path: Some(format!("backup_targets[{idx}].name")),
                    resource_ref: Some(format!("backup_target/{}", target.name)),
                    blocking,
                    suggestion: Some(
                        "verify endpoint and credentials, then refresh the inventory snapshot"
                            .into(),
                    ),
                });
            }
        }
    }

    findings
}
