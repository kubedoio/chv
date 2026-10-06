//! The agent's view of the node's stord backend (#379 DP4/DP5).
//!
//! stord is single-backend per daemon: the node's `stord.toml`
//! `backend_type` selects the one backend every volume open on the node
//! dispatches to. The agent needs two facts from that file — the backend
//! class (to report as `NodeInventory.storage_classes`, DP4) and the LVM
//! volume group (to shape the DP5 dm-path locator for LVM-class opens) —
//! and it already has a validated acquisition route for the file itself:
//! `AgentConfig.stord_config_path`, the #385 pass-through key the
//! supervisor parses for respawn validation.
//!
//! Source-of-truth rules (design §2.4/DP4):
//!
//! - `stord_config_path` set and parseable → the file's `backend_type`
//!   (absent key = `local`, mirroring stord's own default) and
//!   `lvm_volume_group`.
//! - `stord_config_path` absent → `local`/no VG: the supervisor-managed
//!   stord's generated config never sets `backend_type`, so the node
//!   provably runs local.
//! - unreadable/malformed file → `local`/no VG with a loud warn (never
//!   worse than the supervisor's own degrade-to-generated fallback; the
//!   stord class validation remains the enforcement boundary).
//!
//! This replaces the pre-#379 directory probe, which reported
//! `["localdisk"]` on LVM nodes (it probed `storage/localdisk`
//! subdirectories, never consulting stord's `backend_type`).

use std::path::Path;
use tracing::warn;

/// The stord backend class and LVM volume group the agent learned from
/// the operator's `stord.toml` (via `AgentConfig.stord_config_path`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StordBackendInfo {
    /// The stord `backend_type` vocabulary value (`local`/`iscsi`/
    /// `ceph`/`lvm`); `local` when absent or unknown-at-source.
    pub backend_class: String,
    /// The configured `lvm_volume_group`, when the file names one.
    pub lvm_volume_group: Option<String>,
}

impl Default for StordBackendInfo {
    fn default() -> Self {
        Self {
            backend_class: chv_hypervisor_api::resources::DEFAULT_BACKEND_CLASS.to_string(),
            lvm_volume_group: None,
        }
    }
}

impl StordBackendInfo {
    /// Learn the node's stord backend from `AgentConfig.stord_config_path`
    /// (#379 DP4): `None` (the config default) or an unusable file means
    /// the supervisor-managed local backend; a parseable file contributes
    /// its `backend_type` (absent key = local) and `lvm_volume_group`.
    pub fn from_stord_config_path(stord_config_path: Option<&Path>) -> Self {
        let Some(path) = stord_config_path else {
            return Self::default();
        };
        match chv_config::load_stord_config(Some(path)) {
            Ok(cfg) => Self {
                backend_class: cfg.backend_type.clone().unwrap_or_else(|| {
                    chv_hypervisor_api::resources::DEFAULT_BACKEND_CLASS.to_string()
                }),
                lvm_volume_group: cfg.lvm_volume_group.clone(),
            },
            Err(e) => {
                warn!(
                    config = %path.display(),
                    error = %e,
                    "stord_config_path unreadable or malformed; assuming the local stord backend for inventory and LVM locator shaping"
                );
                Self::default()
            }
        }
    }

    /// The volume group LVM-class opens are located against: the
    /// configured VG, or stord's own `chv-vg` default (DP5 — the
    /// locator is an allowlist token; the backend re-derives the device
    /// from the sanitized volume id).
    pub fn volume_group(&self) -> &str {
        self.lvm_volume_group
            .as_deref()
            .unwrap_or(chv_hypervisor_api::resources::DEFAULT_LVM_VOLUME_GROUP)
    }

    /// The classes this node's stord offers (DP4): exactly the daemon's
    /// single backend — stord cannot serve a second class on the same
    /// node (design §2.2).
    pub fn offered_storage_classes(&self) -> Vec<String> {
        vec![self.backend_class.clone()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_stord_toml(dir: &Path, body: &str) -> std::path::PathBuf {
        let path = dir.join("stord.toml");
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn absent_config_path_means_local() {
        // The config default (no stord_config_path): the supervisor-
        // managed stord's generated config never sets backend_type, so
        // the node provably runs local — the same answer stord itself
        // would give.
        let info = StordBackendInfo::from_stord_config_path(None);
        assert_eq!(info.backend_class, "local");
        assert_eq!(info.lvm_volume_group, None);
        assert_eq!(info.offered_storage_classes(), vec!["local"]);
        assert_eq!(info.volume_group(), "chv-vg");
    }

    #[test]
    fn lvm_config_contributes_class_and_vg() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_stord_toml(
            dir.path(),
            r#"
socket_path = "/run/chv/stord/api.sock"
runtime_dir = "/var/lib/chv/stord"
log_level = "info"
backend_type = "lvm"
lvm_volume_group = "qual-vg"
"#,
        );
        let info = StordBackendInfo::from_stord_config_path(Some(&path));
        assert_eq!(info.backend_class, "lvm");
        assert_eq!(info.lvm_volume_group.as_deref(), Some("qual-vg"));
        assert_eq!(info.offered_storage_classes(), vec!["lvm"]);
        assert_eq!(info.volume_group(), "qual-vg");
    }

    #[test]
    fn config_without_backend_type_defaults_to_local() {
        // backend_type absent = stord's own B1 default (local), and the
        // VG key alone does not flip the class.
        let dir = tempfile::tempdir().unwrap();
        let path = write_stord_toml(
            dir.path(),
            r#"
socket_path = "/run/chv/stord/api.sock"
runtime_dir = "/var/lib/chv/stord"
log_level = "info"
lvm_volume_group = "some-vg"
"#,
        );
        let info = StordBackendInfo::from_stord_config_path(Some(&path));
        assert_eq!(info.backend_class, "local");
        assert_eq!(info.volume_group(), "some-vg");
    }

    #[test]
    fn malformed_config_degrades_to_local() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_stord_toml(dir.path(), "not = [valid toml");
        let info = StordBackendInfo::from_stord_config_path(Some(&path));
        assert_eq!(info, StordBackendInfo::default());
    }

    #[test]
    fn missing_file_degrades_to_local() {
        let dir = tempfile::tempdir().unwrap();
        let info = StordBackendInfo::from_stord_config_path(Some(&dir.path().join("no-such.toml")));
        assert_eq!(info, StordBackendInfo::default());
    }
}
