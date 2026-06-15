//! Secret-ref existence and deploy-permission checks.
//!
//! `SECRET_REF_MISSING` matches each concrete `secret_ref` in the model
//! against `inv.secrets[].name`. When `inv.secrets_complete == false` (no
//! authoritative `SecretRepository` ships yet), the finding downgrades to
//! warning severity rather than blocking — the absence of the secret store
//! cannot be distinguished from a real missing secret. Once a real
//! `SecretRepository` ships and the provider sets
//! `secrets_complete = true`, the finding promotes to a blocking error.

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

    let known: HashSet<&str> = inv.secrets.iter().map(|s| s.name.as_str()).collect();

    // Severity downgrades when the secret-store inventory is incomplete —
    // we cannot confidently call a reference "missing" if we never saw the
    // real registry. The check still fires so operators are aware the
    // reference is unverified; it just doesn't block deployment.
    let (severity, blocking, suffix) = if inv.secrets_complete {
        (Severity::Error, true, String::new())
    } else {
        (
            Severity::Warning,
            false,
            " (secret store inventory is incomplete — verification deferred)".to_string(),
        )
    };

    for (path, resource_ref, secret_name) in refs {
        if !known.contains(secret_name.as_str()) {
            findings.push(Finding {
                severity,
                code: Cow::Borrowed(codes::SECRET_REF_MISSING),
                message: format!(
                    "secret reference {secret_name} is not present in the platform secret store{suffix}"
                ),
                path: Some(path),
                resource_ref: Some(resource_ref),
                blocking,
                suggestion: Some(format!(
                    "create secret {secret_name} or remove the reference"
                )),
            });
        }
    }

    findings
}
