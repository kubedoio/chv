//! Secret-ref existence and deploy-permission checks.
//!
//! Phase 3 leaves the secret-store inventory unmodelled — every `secret_ref`
//! that appears in the architecture currently triggers `SECRET_REF_MISSING`
//! unless the snapshot supplies a sentinel: any datastore named `*-with-secret`
//! is considered to satisfy a known reference for tests. Real implementations
//! will gain a `secrets: Vec<SecretInfo>` slot on the snapshot once a secret
//! repository ships; the check function below is the single place to update.
//!
//! For now we surface every concrete `secret_ref` from the model that does
//! not appear in `inv.datastores[].name` (used as a placeholder secret
//! registry — replaced by a real one in a follow-up). This keeps the code
//! present and tested while the contract evolves.

use std::borrow::Cow;
use std::collections::HashSet;

use chv_controlplane_types::architecture::{Finding, Severity};

use crate::codes;
use crate::fleet::InventorySnapshot;
use crate::model::CHVArchitecture;

pub(super) fn check(model: &CHVArchitecture, inv: &InventorySnapshot) -> Vec<Finding> {
    let mut findings = Vec::new();

    if !inv.deploy_allowed {
        findings.push(Finding {
            severity: Severity::Error,
            code: Cow::Borrowed(codes::PERMISSION_DENIED_DEPLOY),
            message: "caller lacks the architecture:apply permission required to deploy".into(),
            path: None,
            resource_ref: None,
            blocking: true,
            suggestion: Some(
                "ask an administrator to grant architecture:apply on this project".into(),
            ),
        });
    }

    // Collect every concrete secret_ref present in the model (datastore +
    // backup target). User-side `password` / `token` literals are caught
    // by the static `RAW_SECRET_FORBIDDEN` check; here we only validate
    // explicit references.
    let mut refs: Vec<(String, String, String)> = Vec::new(); // (path, resource_ref, secret_name)
    for (idx, ds) in model.datastores.iter().enumerate() {
        if let Some(s) = ds.secret_ref.as_deref() {
            refs.push((
                format!("datastores[{idx}].secret_ref"),
                format!("datastore/{}", ds.name),
                s.to_string(),
            ));
        }
    }
    for (idx, bt) in model.backup_targets.iter().enumerate() {
        if let Some(s) = bt.secret_ref.as_deref() {
            refs.push((
                format!("backup_targets[{idx}].secret_ref"),
                format!("backup_target/{}", bt.name),
                s.to_string(),
            ));
        }
    }

    // Placeholder secret registry — see module doc comment.
    let known: HashSet<&str> = inv.datastores.iter().map(|d| d.name.as_str()).collect();

    for (path, resource_ref, secret_name) in refs {
        if !known.contains(secret_name.as_str()) {
            findings.push(Finding {
                severity: Severity::Error,
                code: Cow::Borrowed(codes::SECRET_REF_MISSING),
                message: format!(
                    "secret reference {secret_name} is not present in the platform secret store"
                ),
                path: Some(path),
                resource_ref: Some(resource_ref),
                blocking: true,
                suggestion: Some(format!(
                    "create secret {secret_name} or remove the reference"
                )),
            });
        }
    }

    findings
}
