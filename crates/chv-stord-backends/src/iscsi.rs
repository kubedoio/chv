use crate::r#trait::{BackendHealth, StorageBackend, VolumeExport};
use async_trait::async_trait;
use chv_common::types::{BackendLocator, DevicePolicy};
use chv_errors::ChvError;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

struct DirtyTracker {
    block_size: u64,
    bitmap: Vec<u8>,
}

/// iSCSI storage backend configuration.
#[derive(Debug, Clone)]
pub struct IscsiConfig {
    /// iSCSI target portal address (e.g., "192.168.1.100:3260").
    pub portal: String,
    /// Target IQN (e.g., "iqn.2024-01.com.example:storage.target1").
    pub target_iqn: String,
    /// Initiator name (e.g., "iqn.2024-01.com.example:initiator.node1").
    pub initiator_name: String,
    /// Optional CHAP username for authentication.
    pub chap_username: Option<String>,
    /// Optional CHAP secret for authentication.
    pub chap_secret: Option<String>,
}

/// iSCSI storage backend.
///
/// Manages iSCSI LUNs via `iscsiadm` and `targetcli` CLI tools.
/// Each volume maps to a LUN on the configured iSCSI target.
pub struct IscsiBackend {
    config: IscsiConfig,
    dirty_trackers: Arc<RwLock<HashMap<String, DirtyTracker>>>,
}

impl IscsiBackend {
    pub fn new(config: IscsiConfig) -> Result<Self, ChvError> {
        if config.portal.is_empty() {
            return Err(ChvError::InvalidArgument {
                field: "portal".to_string(),
                reason: "iSCSI portal address cannot be empty".to_string(),
            });
        }
        if config.target_iqn.is_empty() {
            return Err(ChvError::InvalidArgument {
                field: "target_iqn".to_string(),
                reason: "iSCSI target IQN cannot be empty".to_string(),
            });
        }
        Ok(Self {
            config,
            dirty_trackers: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    fn sanitize_id(id: &str) -> Result<String, ChvError> {
        if id.is_empty() {
            return Err(ChvError::InvalidArgument {
                field: "id".to_string(),
                reason: "empty id".to_string(),
            });
        }
        if !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
        {
            return Err(ChvError::InvalidArgument {
                field: "id".to_string(),
                reason: format!("invalid id: {}", id),
            });
        }
        Ok(id.to_string())
    }

    fn expected_handle(&self, volume_id: &str) -> String {
        format!("iscsi-{}-{}", self.config.target_iqn, volume_id)
    }

    fn validate_handle(&self, handle: &str) -> Result<(), ChvError> {
        let prefix = format!("iscsi-{}-", self.config.target_iqn);
        if !handle.starts_with(&prefix) {
            return Err(ChvError::InvalidArgument {
                field: "handle".to_string(),
                reason: format!("handle {} does not belong to this backend", handle),
            });
        }
        Ok(())
    }

    /// Discover iSCSI targets on the portal.
    async fn discover_targets(&self) -> Result<(), ChvError> {
        let out = Command::new("iscsiadm")
            .args([
                "-m",
                "discovery",
                "-t",
                "sendtargets",
                "-p",
                &self.config.portal,
            ])
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "iscsiadm".to_string(),
                source: e,
            })?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "iscsi".to_string(),
                reason: format!(
                    "iscsiadm discovery failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                ),
            });
        }
        Ok(())
    }

    /// Login to the iSCSI target.
    async fn login(&self) -> Result<(), ChvError> {
        let mut args = vec![
            "-m".to_string(),
            "node".to_string(),
            "-T".to_string(),
            self.config.target_iqn.clone(),
            "-p".to_string(),
            self.config.portal.clone(),
            "--login".to_string(),
        ];

        // Set CHAP credentials if configured.
        if let (Some(user), Some(secret)) = (&self.config.chap_username, &self.config.chap_secret) {
            // First set authentication mode.
            let auth_out = Command::new("iscsiadm")
                .args([
                    "-m",
                    "node",
                    "-T",
                    &self.config.target_iqn,
                    "-p",
                    &self.config.portal,
                    "--op",
                    "update",
                    "-n",
                    "node.session.auth.authmethod",
                    "-v",
                    "CHAP",
                ])
                .output()
                .await
                .map_err(|e| ChvError::Io {
                    path: "iscsiadm".to_string(),
                    source: e,
                })?;
            if !auth_out.status.success() {
                warn!(
                    portal = %self.config.portal,
                    stderr = %String::from_utf8_lossy(&auth_out.stderr),
                    "failed to set CHAP auth method"
                );
            }

            // Set CHAP username.
            let _ = Command::new("iscsiadm")
                .args([
                    "-m",
                    "node",
                    "-T",
                    &self.config.target_iqn,
                    "-p",
                    &self.config.portal,
                    "--op",
                    "update",
                    "-n",
                    "node.session.auth.username",
                    "-v",
                    user,
                ])
                .output()
                .await;

            // Set CHAP secret.
            let _ = Command::new("iscsiadm")
                .args([
                    "-m",
                    "node",
                    "-T",
                    &self.config.target_iqn,
                    "-p",
                    &self.config.portal,
                    "--op",
                    "update",
                    "-n",
                    "node.session.auth.password",
                    "-v",
                    secret,
                ])
                .output()
                .await;

            // Drop the extra args that would be unused.
            let _ = args;
            args = vec![
                "-m".to_string(),
                "node".to_string(),
                "-T".to_string(),
                self.config.target_iqn.clone(),
                "-p".to_string(),
                self.config.portal.clone(),
                "--login".to_string(),
            ];
        }

        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let out = Command::new("iscsiadm")
            .args(&args_ref)
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "iscsiadm".to_string(),
                source: e,
            })?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            // Already logged in is not an error.
            if !stderr.contains("already present") {
                return Err(ChvError::BackendUnavailable {
                    backend: "iscsi".to_string(),
                    reason: format!("iscsiadm login failed: {}", stderr),
                });
            }
        }
        Ok(())
    }

    /// Logout from the iSCSI target.
    async fn logout(&self) -> Result<(), ChvError> {
        let out = Command::new("iscsiadm")
            .args([
                "-m",
                "node",
                "-T",
                &self.config.target_iqn,
                "-p",
                &self.config.portal,
                "--logout",
            ])
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "iscsiadm".to_string(),
                source: e,
            })?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            // Not logged in is not an error.
            if !stderr.contains("No matching sessions") {
                return Err(ChvError::BackendUnavailable {
                    backend: "iscsi".to_string(),
                    reason: format!("iscsiadm logout failed: {}", stderr),
                });
            }
        }
        Ok(())
    }

    /// Get the device path for a specific volume/LUN.
    /// After login, iSCSI devices appear under /dev/disk/by-path/ with a predictable name.
    fn device_path(&self, volume_id: &str) -> String {
        format!(
            "/dev/disk/by-path/ip-{}-iscsi-{}-lun-{}",
            self.config.portal, self.config.target_iqn, volume_id
        )
    }

    /// Create a LUN on the target using targetcli.
    async fn create_lun(&self, volume_id: &str, size_bytes: u64) -> Result<(), ChvError> {
        Self::sanitize_id(volume_id)?;
        let size_mb = size_bytes.div_ceil(1024 * 1024).max(1);

        // Create a fileio backstore (targetcli).
        let backstore_path = format!("/backstores/fileio/{}", volume_id);
        let file_path = format!("/var/lib/iscsi-targets/{}.img", volume_id);

        let out = Command::new("targetcli")
            .args([
                &backstore_path,
                "create",
                &file_path,
                &format!("{}M", size_mb),
            ])
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "targetcli".to_string(),
                source: e,
            })?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "iscsi".to_string(),
                reason: format!(
                    "targetcli create backstore failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                ),
            });
        }

        // Create the LUN on the target.
        let lun_path = format!("/iscsi/{}/tpg1/luns", self.config.target_iqn);
        let out = Command::new("targetcli")
            .args([
                &lun_path,
                "create",
                &format!("/backstores/fileio/{}", volume_id),
            ])
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "targetcli".to_string(),
                source: e,
            })?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "iscsi".to_string(),
                reason: format!(
                    "targetcli create lun failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                ),
            });
        }

        info!(volume_id, size_bytes, "created iSCSI LUN");
        Ok(())
    }
}

#[async_trait]
impl StorageBackend for IscsiBackend {
    async fn open(
        &self,
        volume_id: &str,
        locator: &BackendLocator,
        _policy: &DevicePolicy,
    ) -> Result<VolumeExport, ChvError> {
        if locator.backend_class != "iscsi" {
            return Err(ChvError::BackendUnavailable {
                backend: locator.backend_class.clone(),
                reason: "iSCSI backend only handles iscsi class".to_string(),
            });
        }
        Self::sanitize_id(volume_id)?;

        // Discover and login to the target.
        self.discover_targets().await?;
        self.login().await?;

        let path = self.device_path(volume_id);
        info!(volume_id, path = %path, "opened iSCSI volume");
        Ok(VolumeExport {
            export_kind: "iscsi".to_string(),
            export_path: path,
            attachment_handle: self.expected_handle(volume_id),
        })
    }

    async fn close(&self, volume_id: &str, handle: &str) -> Result<(), ChvError> {
        self.validate_handle(handle)?;
        if handle != self.expected_handle(volume_id) {
            return Err(ChvError::InvalidArgument {
                field: "handle".to_string(),
                reason: format!("handle {} does not match volume_id {}", handle, volume_id),
            });
        }

        self.logout().await?;
        info!(volume_id, "closed iSCSI volume (logged out)");
        Ok(())
    }

    async fn attach(
        &self,
        volume_id: &str,
        handle: &str,
        vm_id: &str,
    ) -> Result<VolumeExport, ChvError> {
        self.validate_handle(handle)?;
        if handle != self.expected_handle(volume_id) {
            return Err(ChvError::InvalidArgument {
                field: "handle".to_string(),
                reason: format!("handle {} does not match volume_id {}", handle, volume_id),
            });
        }
        let path = self.device_path(volume_id);
        info!(volume_id, vm_id, handle, path = %path, "attaching iSCSI volume");
        Ok(VolumeExport {
            export_kind: "iscsi".to_string(),
            export_path: path,
            attachment_handle: handle.to_string(),
        })
    }

    async fn detach(
        &self,
        volume_id: &str,
        _handle: &str,
        ownership: chv_common::AttachmentOwnership,
        force: bool,
    ) -> Result<(), ChvError> {
        let vm_id = &ownership.vm_id;
        if vm_id.is_empty() {
            return Err(ChvError::InvalidArgument {
                field: "vm_id".to_string(),
                reason: "missing vm_id for detach".to_string(),
            });
        }
        if force {
            warn!(volume_id, vm_id, "force detaching iSCSI volume");
        } else {
            info!(volume_id, vm_id, "detaching iSCSI volume");
        }
        Ok(())
    }

    async fn health(&self, volume_id: &str, _handle: &str) -> Result<BackendHealth, ChvError> {
        // Check if the iSCSI session is active.
        let out = Command::new("iscsiadm")
            .args(["-m", "session"])
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "iscsiadm".to_string(),
                source: e,
            })?;

        let stdout = String::from_utf8_lossy(&out.stdout);
        let session_active = stdout.contains(&self.config.target_iqn);

        let status = if session_active {
            "healthy"
        } else {
            "unhealthy"
        };
        let last_error = if session_active {
            String::new()
        } else {
            format!(
                "no active iSCSI session for target {} (volume {})",
                self.config.target_iqn, volume_id
            )
        };

        Ok(BackendHealth {
            status: status.to_string(),
            backend_state: if session_active {
                "connected".to_string()
            } else {
                "disconnected".to_string()
            },
            last_error,
        })
    }

    async fn resize(
        &self,
        volume_id: &str,
        handle: &str,
        new_size_bytes: u64,
    ) -> Result<(), ChvError> {
        self.validate_handle(handle)?;
        if handle != self.expected_handle(volume_id) {
            return Err(ChvError::InvalidArgument {
                field: "handle".to_string(),
                reason: format!("handle {} does not match volume_id {}", handle, volume_id),
            });
        }
        Self::sanitize_id(volume_id)?;
        let size_mb = new_size_bytes.div_ceil(1024 * 1024).max(1);

        // Resize the backing file via targetcli.
        let backstore_path = format!("/backstores/fileio/{}", volume_id);
        let out = Command::new("targetcli")
            .args([
                &backstore_path,
                "set",
                "attribute",
                &format!("size={}M", size_mb),
            ])
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "targetcli".to_string(),
                source: e,
            })?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "iscsi".to_string(),
                reason: format!(
                    "targetcli resize failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                ),
            });
        }

        info!(volume_id, new_size_bytes, "resized iSCSI LUN");
        Ok(())
    }

    async fn prepare_snapshot(
        &self,
        _volume_id: &str,
        _handle: &str,
        _ownership: chv_common::AttachmentOwnership,
        _snapshot_name: &str,
    ) -> Result<(), ChvError> {
        Err(ChvError::InvalidArgument {
            field: "operation".to_string(),
            reason: "iSCSI backend does not support snapshots directly".to_string(),
        })
    }

    async fn prepare_clone(
        &self,
        _volume_id: &str,
        _handle: &str,
        _ownership: chv_common::AttachmentOwnership,
        _clone_name: &str,
    ) -> Result<(), ChvError> {
        Err(ChvError::InvalidArgument {
            field: "operation".to_string(),
            reason: "iSCSI backend does not support clones directly".to_string(),
        })
    }

    async fn restore_snapshot(
        &self,
        _volume_id: &str,
        _handle: &str,
        _snapshot_name: &str,
    ) -> Result<(), ChvError> {
        Err(ChvError::InvalidArgument {
            field: "operation".to_string(),
            reason: "iSCSI backend does not support snapshot restore directly".to_string(),
        })
    }

    async fn delete_snapshot(
        &self,
        _volume_id: &str,
        _handle: &str,
        _snapshot_name: &str,
    ) -> Result<(), ChvError> {
        Err(ChvError::InvalidArgument {
            field: "operation".to_string(),
            reason: "iSCSI backend does not support snapshot deletion directly".to_string(),
        })
    }

    async fn set_device_policy(
        &self,
        volume_id: &str,
        handle: &str,
        policy: &DevicePolicy,
    ) -> Result<(), ChvError> {
        self.validate_handle(handle)?;
        if handle != self.expected_handle(volume_id) {
            return Err(ChvError::InvalidArgument {
                field: "handle".to_string(),
                reason: format!("handle {} does not match volume_id {}", handle, volume_id),
            });
        }

        if policy.read_only {
            let path = self.device_path(volume_id);
            let out = Command::new("blockdev")
                .args(["--setro", &path])
                .output()
                .await
                .map_err(|e| ChvError::Io {
                    path: "blockdev".to_string(),
                    source: e,
                })?;
            if !out.status.success() {
                return Err(ChvError::BackendUnavailable {
                    backend: "iscsi".to_string(),
                    reason: format!(
                        "blockdev --setro failed: {}",
                        String::from_utf8_lossy(&out.stderr)
                    ),
                });
            }
        }

        if policy.read_bps > 0
            || policy.write_bps > 0
            || policy.read_iops > 0
            || policy.write_iops > 0
        {
            warn!(
                volume_id,
                "iSCSI backend does not enforce throughput or iops limits"
            );
        }

        Ok(())
    }

    // --- Migration methods ---

    async fn read_block(
        &self,
        volume_id: &str,
        handle: &str,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, ChvError> {
        self.validate_handle(handle)?;
        let path = self.device_path(volume_id);
        let path_clone = path.clone();
        tokio::task::spawn_blocking(move || {
            use std::io::{Read, Seek, SeekFrom};
            let mut file = std::fs::File::open(&path_clone).map_err(|e| ChvError::Io {
                path: path_clone.clone(),
                source: e,
            })?;
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| ChvError::Io {
                    path: path_clone.clone(),
                    source: e,
                })?;
            let mut buf = vec![0u8; length as usize];
            file.read_exact(&mut buf).map_err(|e| ChvError::Io {
                path: path_clone,
                source: e,
            })?;
            Ok(buf)
        })
        .await
        .map_err(|e| ChvError::BackendUnavailable {
            backend: "iscsi".to_string(),
            reason: format!("read_block task panicked: {}", e),
        })?
    }

    async fn write_block(
        &self,
        volume_id: &str,
        handle: &str,
        offset: u64,
        data: &[u8],
    ) -> Result<(), ChvError> {
        self.validate_handle(handle)?;
        let path = self.device_path(volume_id);
        let data_owned = data.to_vec();
        let data_len = data.len() as u64;
        let path_clone = path.clone();
        tokio::task::spawn_blocking(move || {
            use std::io::{Seek, SeekFrom, Write};
            let mut file = std::fs::File::options()
                .write(true)
                .open(&path_clone)
                .map_err(|e| ChvError::Io {
                    path: path_clone.clone(),
                    source: e,
                })?;
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| ChvError::Io {
                    path: path_clone.clone(),
                    source: e,
                })?;
            file.write_all(&data_owned).map_err(|e| ChvError::Io {
                path: path_clone,
                source: e,
            })?;
            Ok(())
        })
        .await
        .map_err(|e| ChvError::BackendUnavailable {
            backend: "iscsi".to_string(),
            reason: format!("write_block task panicked: {}", e),
        })??;

        // Update dirty bitmap if tracking is enabled.
        let mut map = self.dirty_trackers.write().await;
        if let Some(tracker) = map.get_mut(handle) {
            let bs = tracker.block_size;
            let start_block = offset / bs;
            let end_block = (offset + data_len).div_ceil(bs);
            for block in start_block..end_block {
                let byte_idx = (block / 8) as usize;
                let bit_idx = (block % 8) as u8;
                if byte_idx < tracker.bitmap.len() {
                    tracker.bitmap[byte_idx] |= 1 << bit_idx;
                }
            }
        }

        Ok(())
    }

    async fn volume_size(&self, volume_id: &str, _handle: &str) -> Result<u64, ChvError> {
        let path = self.device_path(volume_id);
        // Use blockdev to get size of iSCSI device.
        let out = Command::new("blockdev")
            .args(["--getsize64", &path])
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "blockdev".to_string(),
                source: e,
            })?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "iscsi".to_string(),
                reason: format!(
                    "blockdev --getsize64 failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                ),
            });
        }
        let size_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
        size_str
            .parse::<u64>()
            .map_err(|_| ChvError::BackendUnavailable {
                backend: "iscsi".to_string(),
                reason: format!("could not parse blockdev output as bytes: '{}'", size_str),
            })
    }

    async fn create_receiving_volume(
        &self,
        volume_id: &str,
        size_bytes: u64,
        _format: &str,
    ) -> Result<VolumeExport, ChvError> {
        Self::sanitize_id(volume_id)?;
        if size_bytes == 0 {
            return Err(ChvError::InvalidArgument {
                field: "size_bytes".to_string(),
                reason: "size_bytes must be > 0".to_string(),
            });
        }

        self.create_lun(volume_id, size_bytes).await?;

        // Re-discover to pick up the new LUN.
        self.discover_targets().await?;
        self.login().await?;

        // Rescan sessions so the new LUN appears as a block device.
        let _ = Command::new("iscsiadm")
            .args(["-m", "session", "--rescan"])
            .output()
            .await;

        let path = self.device_path(volume_id);
        debug!(volume_id, path = %path, "created receiving iSCSI volume");
        Ok(VolumeExport {
            export_kind: "iscsi".to_string(),
            export_path: path,
            attachment_handle: self.expected_handle(volume_id),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iscsi_backend_rejects_empty_portal() {
        let config = IscsiConfig {
            portal: "".to_string(),
            target_iqn: "iqn.2024-01.com.example:target".to_string(),
            initiator_name: "iqn.2024-01.com.example:init".to_string(),
            chap_username: None,
            chap_secret: None,
        };
        assert!(IscsiBackend::new(config).is_err());
    }

    #[test]
    fn iscsi_backend_rejects_empty_target_iqn() {
        let config = IscsiConfig {
            portal: "192.168.1.100:3260".to_string(),
            target_iqn: "".to_string(),
            initiator_name: "iqn.2024-01.com.example:init".to_string(),
            chap_username: None,
            chap_secret: None,
        };
        assert!(IscsiBackend::new(config).is_err());
    }

    #[test]
    fn iscsi_backend_valid_config() {
        let config = IscsiConfig {
            portal: "192.168.1.100:3260".to_string(),
            target_iqn: "iqn.2024-01.com.example:target".to_string(),
            initiator_name: "iqn.2024-01.com.example:init".to_string(),
            chap_username: Some("user".to_string()),
            chap_secret: Some("secret".to_string()),
        };
        assert!(IscsiBackend::new(config).is_ok());
    }

    #[test]
    fn iscsi_sanitize_id_rejects_invalid() {
        assert!(IscsiBackend::sanitize_id("").is_err());
        assert!(IscsiBackend::sanitize_id("foo/bar").is_err());
        assert!(IscsiBackend::sanitize_id("valid-id").is_ok());
        assert!(IscsiBackend::sanitize_id("valid_id.1").is_ok());
    }

    #[tokio::test]
    async fn iscsi_backend_open_rejects_wrong_class() {
        let config = IscsiConfig {
            portal: "192.168.1.100:3260".to_string(),
            target_iqn: "iqn.2024-01.com.example:target".to_string(),
            initiator_name: "iqn.2024-01.com.example:init".to_string(),
            chap_username: None,
            chap_secret: None,
        };
        let backend = IscsiBackend::new(config).unwrap();
        let locator = BackendLocator {
            backend_class: "lvm".to_string(),
            locator: "vg0/vol1".to_string(),
            options: Default::default(),
        };
        let res = backend
            .open("vol-1", &locator, &DevicePolicy::default())
            .await;
        assert!(matches!(res, Err(ChvError::BackendUnavailable { .. })));
    }

    #[tokio::test]
    async fn iscsi_backend_attach_invalid_handle() {
        let config = IscsiConfig {
            portal: "192.168.1.100:3260".to_string(),
            target_iqn: "iqn.2024-01.com.example:target".to_string(),
            initiator_name: "iqn.2024-01.com.example:init".to_string(),
            chap_username: None,
            chap_secret: None,
        };
        let backend = IscsiBackend::new(config).unwrap();
        let res = backend.attach("vol-1", "bad-handle", "vm-1").await;
        assert!(matches!(res, Err(ChvError::InvalidArgument { .. })));
    }

    // C-19 (S4-5): unit-test boundary for the iSCSI backend.
    //
    // IscsiConfig does not derive serde::{Serialize, Deserialize} (and adding
    // the dep is out of scope for this change — the config is constructed
    // programmatically by chv-stord), so the originally proposed serde
    // round-trip is replaced by a Clone round-trip that guards against future
    // field-add omissions in `#[derive(Clone)]`.

    #[test]
    fn iscsi_config_clone_preserves_all_fields() {
        let original = IscsiConfig {
            portal: "10.0.0.1:3260".to_string(),
            target_iqn: "iqn.2024-01.com.example:target".to_string(),
            initiator_name: "iqn.2024-01.com.example:init".to_string(),
            chap_username: Some("u".to_string()),
            chap_secret: Some("s".to_string()),
        };
        let copy = original.clone();
        assert_eq!(copy.portal, original.portal);
        assert_eq!(copy.target_iqn, original.target_iqn);
        assert_eq!(copy.initiator_name, original.initiator_name);
        assert_eq!(copy.chap_username, original.chap_username);
        assert_eq!(copy.chap_secret, original.chap_secret);

        // None branch must also survive Clone.
        let no_chap = IscsiConfig {
            chap_username: None,
            chap_secret: None,
            ..original
        };
        let copy = no_chap.clone();
        assert!(copy.chap_username.is_none());
        assert!(copy.chap_secret.is_none());
    }

    #[test]
    fn iscsi_expected_handle_format_is_stable() {
        // The `iscsi-{iqn}-{volume_id}` handle format is observed by chv-stord
        // and chv-agent; pinning it prevents accidental ABI breaks.
        let config = IscsiConfig {
            portal: "10.0.0.1:3260".to_string(),
            target_iqn: "iqn.2024-01.com.example:storage.t1".to_string(),
            initiator_name: "iqn.2024-01.com.example:init".to_string(),
            chap_username: None,
            chap_secret: None,
        };
        let backend = IscsiBackend::new(config).expect("valid config");
        assert_eq!(
            backend.expected_handle("vol-42"),
            "iscsi-iqn.2024-01.com.example:storage.t1-vol-42"
        );
    }

    /// Health check against an unreachable iSCSI portal (RFC 5737 TEST-NET-1).
    ///
    /// `health()` shells out to `iscsiadm -m session`.  On hosts without
    /// open-iscsi installed this errors via `ChvError::Io`; on hosts with it
    /// installed but no session for our IQN it returns `Ok(BackendHealth)`
    /// with `status == "unhealthy"`.  Both are valid "not connected"
    /// signals — the test asserts we never report "healthy" for a target the
    /// node cannot possibly reach.
    ///
    /// Marked `#[ignore]`: depends on the host environment (presence of
    /// `iscsiadm`, system iSCSI database state).  Run with `--ignored` when
    /// validating on a real Linux host.
    #[tokio::test]
    #[ignore]
    async fn iscsi_health_with_unreachable_target_is_not_reported_healthy() {
        let config = IscsiConfig {
            // RFC 5737 TEST-NET-1 — guaranteed not to host an iSCSI portal.
            portal: "192.0.2.1:3260".to_string(),
            target_iqn: "iqn.2024-01.com.example:unreachable".to_string(),
            initiator_name: "iqn.2024-01.com.example:init".to_string(),
            chap_username: None,
            chap_secret: None,
        };
        let backend = IscsiBackend::new(config).expect("valid config");
        let res = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            backend.health("vol-1", "iscsi-iqn.2024-01.com.example:unreachable-vol-1"),
        )
        .await
        .expect("health() should return within 2s for a session list query");
        match res {
            Ok(h) => assert_ne!(
                h.status, "healthy",
                "health() must not report healthy for an unreachable target; got {:?}",
                h
            ),
            Err(ChvError::Io { .. }) => {
                // iscsiadm not installed on this host — acceptable.
            }
            Err(other) => panic!("unexpected error variant from health(): {:?}", other),
        }
    }
}
