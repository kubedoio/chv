//! Image registration check.

use std::borrow::Cow;
use std::collections::HashSet;

use chv_controlplane_types::architecture::{Finding, Severity};

use crate::codes;
use crate::fleet::InventorySnapshot;
use crate::model::CHVArchitecture;

pub(super) fn check(model: &CHVArchitecture, inv: &InventorySnapshot) -> Vec<Finding> {
    let mut findings = Vec::new();

    let live: HashSet<&str> = inv.images.iter().map(|i| i.name.as_str()).collect();

    for (idx, img) in model.images.iter().enumerate() {
        if !live.contains(img.name.as_str()) {
            findings.push(Finding {
                severity: Severity::Error,
                code: Cow::Borrowed(codes::IMAGE_NOT_FOUND),
                message: format!(
                    "declared image {} is not registered with any datastore",
                    img.name
                ),
                path: Some(format!("images[{idx}].name")),
                resource_ref: Some(format!("image/{}", img.name)),
                blocking: true,
                suggestion: Some(
                    "import the image into a datastore before deploying this architecture".into(),
                ),
            });
        }
    }

    findings
}
