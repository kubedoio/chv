//! Pure fleet checks. Each sub-module owns a small, related code group
//! and exports a single `pub(super) fn check_*` function returning a
//! `Vec<Finding>`.

use chv_controlplane_types::architecture::Finding;

use super::InventorySnapshot;
use crate::model::CHVArchitecture;

mod backup;
mod datastore;
mod host;
mod image;
mod network;
mod permissions;

/// Run every fleet check and concatenate the findings. Order is stable:
/// hosts → networks → datastores → images → backups → permissions. Tests
/// pin against codes, not positions, so re-ordering is allowed.
pub fn check_fleet(model: &CHVArchitecture, inv: &InventorySnapshot) -> Vec<Finding> {
    let mut out = Vec::new();
    out.extend(host::check(model, inv));
    out.extend(network::check(model, inv));
    out.extend(datastore::check(model, inv));
    out.extend(image::check(model, inv));
    out.extend(backup::check(model, inv));
    out.extend(permissions::check(model, inv));
    out
}
