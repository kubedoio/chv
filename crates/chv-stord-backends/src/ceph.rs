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

/// Ceph RBD storage backend configuration.
#[derive(Debug, Clone)]
pub struct CephRbdConfig {
    /// Ceph cluster name (default: "ceph").
    pub cluster_name: String,
    /// RBD pool name (e.g., "rbd", "volumes").
    pub pool_name: String,
    /// Ceph user (e.g., "admin", "client.chv").
    pub user: String,
    /// Path to keyring file.
    pub keyring_path: String,
    /// Monitor addresses (comma-separated, e.g., "mon1:6789,mon2:6789").
    pub monitors: String,
}

/// Ceph RBD storage backend.
///
/// Manages RBD images via the `rbd` CLI tool.
/// Each volume maps to an RBD image in the configured pool.
pub struct CephRbdBackend {
    config: CephRbdConfig,
    dirty_trackers: Arc<RwLock<HashMap<String, DirtyTracker>>>,
}

impl CephRbdBackend {
    pub fn new(config: CephRbdConfig) -> Result<Self, ChvError> {
        if config.pool_name.is_empty() {
            return Err(ChvError::InvalidArgument {
                field: "pool_name".to_string(),
                reason: "Ceph pool name cannot be empty".to_string(),
            });
        }
        if config.user.is_empty() {
            return Err(ChvError::InvalidArgument {
                field: "user".to_string(),
                reason: "Ceph user cannot be empty".to_string(),
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
        format!("rbd-{}-{}", self.config.pool_name, volume_id)
    }

    fn validate_handle(&self, handle: &str) -> Result<(), ChvError> {
        let prefix = format!("rbd-{}-", self.config.pool_name);
        if !handle.starts_with(&prefix) {
            return Err(ChvError::InvalidArgument {
                field: "handle".to_string(),
                reason: format!("handle {} does not belong to this backend", handle),
            });
        }
        Ok(())
    }

    /// Full image spec: pool/image
    fn image_spec(&self, volume_id: &str) -> String {
        format!("{}/{}", self.config.pool_name, volume_id)
    }

    /// Common rbd CLI args for cluster/auth configuration.
    fn common_args(&self) -> Vec<String> {
        let mut args = vec![
            "--cluster".to_string(),
            self.config.cluster_name.clone(),
            "--id".to_string(),
            self.config.user.clone(),
        ];
        if !self.config.keyring_path.is_empty() {
            args.push("--keyring".to_string());
            args.push(self.config.keyring_path.clone());
        }
        if !self.config.monitors.is_empty() {
            args.push("-m".to_string());
            args.push(self.config.monitors.clone());
        }
        args
    }

    /// Run an rbd command with common args prepended.
    async fn run_rbd(&self, subcommand_args: &[&str]) -> Result<std::process::Output, ChvError> {
        let common = self.common_args();
        let mut all_args: Vec<&str> = common.iter().map(|s| s.as_str()).collect();
        all_args.extend_from_slice(subcommand_args);

        Command::new("rbd")
            .args(&all_args)
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "rbd".to_string(),
                source: e,
            })
    }
}

#[async_trait]
impl StorageBackend for CephRbdBackend {
    async fn open(
        &self,
        volume_id: &str,
        locator: &BackendLocator,
        _policy: &DevicePolicy,
    ) -> Result<VolumeExport, ChvError> {
        if locator.backend_class != "ceph" && locator.backend_class != "rbd" {
            return Err(ChvError::BackendUnavailable {
                backend: locator.backend_class.clone(),
                reason: "CephRbd backend only handles ceph/rbd class".to_string(),
            });
        }
        Self::sanitize_id(volume_id)?;

        // Map the RBD image to a local block device.
        let spec = self.image_spec(volume_id);
        let out = self.run_rbd(&["map", &spec]).await?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            // Already mapped is acceptable.
            if !stderr.contains("already being watched") {
                return Err(ChvError::BackendUnavailable {
                    backend: "ceph".to_string(),
                    reason: format!("rbd map failed: {}", stderr),
                });
            }
        }

        // The mapped device path is printed to stdout.
        let device_path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let path = if device_path.starts_with("/dev/") {
            device_path
        } else {
            // Fallback: query the mapped device.
            let show_out = self.run_rbd(&["showmapped", "--format", "json"]).await?;
            let mapped_path = Self::find_mapped_device(
                &String::from_utf8_lossy(&show_out.stdout),
                &self.config.pool_name,
                volume_id,
            );
            mapped_path
                .unwrap_or_else(|| format!("/dev/rbd/{}/{}", self.config.pool_name, volume_id))
        };

        info!(volume_id, path = %path, "opened Ceph RBD volume");
        Ok(VolumeExport {
            export_kind: "rbd".to_string(),
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

        // Unmap the RBD device.
        let spec = self.image_spec(volume_id);
        let out = self.run_rbd(&["unmap", &spec]).await?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            // Not mapped is not an error.
            if !stderr.contains("not mapped") && !stderr.contains("not found") {
                return Err(ChvError::BackendUnavailable {
                    backend: "ceph".to_string(),
                    reason: format!("rbd unmap failed: {}", stderr),
                });
            }
        }

        info!(volume_id, "closed Ceph RBD volume (unmapped)");
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
        let path = format!("/dev/rbd/{}/{}", self.config.pool_name, volume_id);
        info!(volume_id, vm_id, handle, path = %path, "attaching Ceph RBD volume");
        Ok(VolumeExport {
            export_kind: "rbd".to_string(),
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
            return Err(ChvError::InvalidArgument { field: "vm_id".to_string(), reason: "missing vm_id for detach".to_string() });
        }
        if force {
            warn!(volume_id, vm_id, "force detaching Ceph RBD volume");
        } else {
            info!(volume_id, vm_id, "detaching Ceph RBD volume");
        }
        Ok(())
    }

    async fn health(&self, volume_id: &str, _handle: &str) -> Result<BackendHealth, ChvError> {
        let spec = self.image_spec(volume_id);
        let out = self.run_rbd(&["info", &spec, "--format", "json"]).await?;

        if out.status.success() {
            Ok(BackendHealth {
                status: "healthy".to_string(),
                backend_state: "open".to_string(),
                last_error: String::new(),
            })
        } else {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            Ok(BackendHealth {
                status: "unhealthy".to_string(),
                backend_state: "error".to_string(),
                last_error: stderr,
            })
        }
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

        let spec = self.image_spec(volume_id);
        let size_str = new_size_bytes.to_string();
        let out = self
            .run_rbd(&["resize", "--size", &size_str, &spec])
            .await?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "ceph".to_string(),
                reason: format!(
                    "rbd resize failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                ),
            });
        }
        info!(volume_id, new_size_bytes, "resized Ceph RBD image");
        Ok(())
    }

    async fn prepare_snapshot(
        &self,
        volume_id: &str,
        handle: &str,
        _ownership: chv_common::AttachmentOwnership,
        snapshot_name: &str,
    ) -> Result<(), ChvError> {
        self.validate_handle(handle)?;
        if handle != self.expected_handle(volume_id) {
            return Err(ChvError::InvalidArgument {
                field: "handle".to_string(),
                reason: format!("handle {} does not match volume_id {}", handle, volume_id),
            });
        }
        Self::sanitize_id(snapshot_name)?;

        let snap_spec = format!("{}/{}@{}", self.config.pool_name, volume_id, snapshot_name);
        let out = self.run_rbd(&["snap", "create", &snap_spec]).await?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "ceph".to_string(),
                reason: format!(
                    "rbd snap create failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                ),
            });
        }
        info!(volume_id, snapshot_name, "created Ceph RBD snapshot");
        Ok(())
    }

    async fn prepare_clone(
        &self,
        volume_id: &str,
        handle: &str,
        _ownership: chv_common::AttachmentOwnership,
        clone_name: &str,
    ) -> Result<(), ChvError> {
        self.validate_handle(handle)?;
        if handle != self.expected_handle(volume_id) {
            return Err(ChvError::InvalidArgument {
                field: "handle".to_string(),
                reason: format!("handle {} does not match volume_id {}", handle, volume_id),
            });
        }
        Self::sanitize_id(clone_name)?;

        // RBD clone requires a snapshot; create a temporary one.
        let snap_name = format!("clone-snap-{}", clone_name);
        let snap_spec = format!("{}/{}@{}", self.config.pool_name, volume_id, snap_name);
        let out = self.run_rbd(&["snap", "create", &snap_spec]).await?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "ceph".to_string(),
                reason: format!(
                    "rbd snap create for clone failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                ),
            });
        }

        // Protect the snapshot for cloning.
        let out = self.run_rbd(&["snap", "protect", &snap_spec]).await?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if !stderr.contains("already protected") {
                return Err(ChvError::BackendUnavailable {
                    backend: "ceph".to_string(),
                    reason: format!("rbd snap protect failed: {}", stderr),
                });
            }
        }

        // Clone from the snapshot.
        let clone_spec = self.image_spec(clone_name);
        let out = self.run_rbd(&["clone", &snap_spec, &clone_spec]).await?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "ceph".to_string(),
                reason: format!("rbd clone failed: {}", String::from_utf8_lossy(&out.stderr)),
            });
        }

        info!(volume_id, clone_name, "prepared Ceph RBD clone");
        Ok(())
    }

    async fn restore_snapshot(
        &self,
        volume_id: &str,
        handle: &str,
        snapshot_name: &str,
    ) -> Result<(), ChvError> {
        self.validate_handle(handle)?;
        if handle != self.expected_handle(volume_id) {
            return Err(ChvError::InvalidArgument {
                field: "handle".to_string(),
                reason: format!("handle {} does not match volume_id {}", handle, volume_id),
            });
        }
        Self::sanitize_id(snapshot_name)?;

        let snap_spec = format!("{}/{}@{}", self.config.pool_name, volume_id, snapshot_name);
        let out = self.run_rbd(&["snap", "rollback", &snap_spec]).await?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "ceph".to_string(),
                reason: format!(
                    "rbd snap rollback failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                ),
            });
        }
        info!(volume_id, snapshot_name, "restored Ceph RBD snapshot");
        Ok(())
    }

    async fn delete_snapshot(
        &self,
        volume_id: &str,
        handle: &str,
        snapshot_name: &str,
    ) -> Result<(), ChvError> {
        self.validate_handle(handle)?;
        if handle != self.expected_handle(volume_id) {
            return Err(ChvError::InvalidArgument {
                field: "handle".to_string(),
                reason: format!("handle {} does not match volume_id {}", handle, volume_id),
            });
        }
        Self::sanitize_id(snapshot_name)?;

        let snap_spec = format!("{}/{}@{}", self.config.pool_name, volume_id, snapshot_name);

        // Unprotect if protected (required before removal).
        let _ = self.run_rbd(&["snap", "unprotect", &snap_spec]).await;

        let out = self.run_rbd(&["snap", "rm", &snap_spec]).await?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "ceph".to_string(),
                reason: format!(
                    "rbd snap rm failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                ),
            });
        }
        info!(volume_id, snapshot_name, "deleted Ceph RBD snapshot");
        Ok(())
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
            let path = format!("/dev/rbd/{}/{}", self.config.pool_name, volume_id);
            info!(volume_id, path = %path, "applying read-only device policy");
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
                    backend: "ceph".to_string(),
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
            // Ceph RBD supports QoS via image metadata but not via blockdev.
            warn!(
                volume_id,
                "CephRbd backend does not enforce throughput or iops limits at block device level"
            );
        }

        Ok(())
    }

    // --- Migration methods ---

    async fn enable_dirty_tracking(
        &self,
        volume_id: &str,
        handle: &str,
        block_size: u64,
    ) -> Result<(), ChvError> {
        if block_size == 0 {
            return Err(ChvError::InvalidArgument {
                field: "block_size".to_string(),
                reason: "block_size must be > 0".to_string(),
            });
        }
        self.validate_handle(handle)?;

        // Query volume size via rbd info.
        let spec = self.image_spec(volume_id);
        let out = self.run_rbd(&["info", &spec, "--format", "json"]).await?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "ceph".to_string(),
                reason: format!("rbd info failed: {}", String::from_utf8_lossy(&out.stderr)),
            });
        }

        let info_json: serde_json::Value =
            serde_json::from_slice(&out.stdout).map_err(|e| ChvError::BackendUnavailable {
                backend: "ceph".to_string(),
                reason: format!("failed to parse rbd info JSON: {}", e),
            })?;

        let file_len = info_json["size"]
            .as_u64()
            .ok_or_else(|| ChvError::BackendUnavailable {
                backend: "ceph".to_string(),
                reason: "rbd info missing 'size' field".to_string(),
            })?;

        let num_blocks = file_len.div_ceil(block_size);
        let bitmap_bytes = num_blocks.div_ceil(8) as usize;

        let tracker = DirtyTracker {
            block_size,
            bitmap: vec![0u8; bitmap_bytes],
        };
        let mut map = self.dirty_trackers.write().await;
        map.insert(handle.to_string(), tracker);
        info!(
            volume_id,
            handle, block_size, bitmap_bytes, "enabled dirty tracking for Ceph RBD volume"
        );
        Ok(())
    }

    async fn get_dirty_bitmap(&self, _volume_id: &str, handle: &str) -> Result<Vec<u8>, ChvError> {
        let map = self.dirty_trackers.read().await;
        match map.get(handle) {
            Some(t) => Ok(t.bitmap.clone()),
            None => Err(ChvError::NotFound {
                resource: "dirty_tracker".to_string(),
                id: handle.to_string(),
            }),
        }
    }

    async fn clear_dirty_bitmap(&self, volume_id: &str, handle: &str) -> Result<(), ChvError> {
        let mut map = self.dirty_trackers.write().await;
        match map.get_mut(handle) {
            Some(t) => {
                t.bitmap.iter_mut().for_each(|b| *b = 0);
                info!(
                    volume_id,
                    handle, "cleared dirty bitmap for Ceph RBD volume"
                );
                Ok(())
            }
            None => Err(ChvError::NotFound {
                resource: "dirty_tracker".to_string(),
                id: handle.to_string(),
            }),
        }
    }

    async fn disable_dirty_tracking(&self, volume_id: &str, handle: &str) -> Result<(), ChvError> {
        let mut map = self.dirty_trackers.write().await;
        map.remove(handle);
        info!(
            volume_id,
            handle, "disabled dirty tracking for Ceph RBD volume"
        );
        Ok(())
    }

    async fn read_block(
        &self,
        volume_id: &str,
        handle: &str,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, ChvError> {
        self.validate_handle(handle)?;
        // For block-level reads, use the mapped device path directly.
        let device_path = format!("/dev/rbd/{}/{}", self.config.pool_name, volume_id);
        let path_clone = device_path.clone();
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
            backend: "ceph".to_string(),
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
        let device_path = format!("/dev/rbd/{}/{}", self.config.pool_name, volume_id);
        let data_owned = data.to_vec();
        let data_len = data.len() as u64;
        let path_clone = device_path.clone();
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
            backend: "ceph".to_string(),
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
        let spec = self.image_spec(volume_id);
        let out = self.run_rbd(&["info", &spec, "--format", "json"]).await?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "ceph".to_string(),
                reason: format!("rbd info failed: {}", String::from_utf8_lossy(&out.stderr)),
            });
        }

        let info_json: serde_json::Value =
            serde_json::from_slice(&out.stdout).map_err(|e| ChvError::BackendUnavailable {
                backend: "ceph".to_string(),
                reason: format!("failed to parse rbd info JSON: {}", e),
            })?;

        info_json["size"]
            .as_u64()
            .ok_or_else(|| ChvError::BackendUnavailable {
                backend: "ceph".to_string(),
                reason: "rbd info missing 'size' field".to_string(),
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

        let spec = self.image_spec(volume_id);
        let size_str = size_bytes.to_string();
        let out = self
            .run_rbd(&["create", "--size", &size_str, &spec])
            .await?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "ceph".to_string(),
                reason: format!(
                    "rbd create failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                ),
            });
        }

        // Map the new image.
        let map_out = self.run_rbd(&["map", &spec]).await?;
        let device_path = if map_out.status.success() {
            let p = String::from_utf8_lossy(&map_out.stdout).trim().to_string();
            if p.starts_with("/dev/") {
                p
            } else {
                format!("/dev/rbd/{}/{}", self.config.pool_name, volume_id)
            }
        } else {
            format!("/dev/rbd/{}/{}", self.config.pool_name, volume_id)
        };

        debug!(volume_id, size_bytes, path = %device_path, "created receiving Ceph RBD volume");
        Ok(VolumeExport {
            export_kind: "rbd".to_string(),
            export_path: device_path,
            attachment_handle: self.expected_handle(volume_id),
        })
    }

    async fn delete_volume(&self, volume_id: &str) -> Result<(), ChvError> {
        Self::sanitize_id(volume_id)?;

        // Unmap first (best-effort).
        let spec = self.image_spec(volume_id);
        let _ = self.run_rbd(&["unmap", &spec]).await;

        // Remove the image.
        let out = self.run_rbd(&["rm", &spec]).await?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "ceph".to_string(),
                reason: format!("rbd rm failed: {}", String::from_utf8_lossy(&out.stderr)),
            });
        }
        info!(volume_id, "deleted Ceph RBD image");
        Ok(())
    }
}

impl CephRbdBackend {
    /// Parse `rbd showmapped --format json` output to find the device path for a given pool/image.
    fn find_mapped_device(json_str: &str, pool: &str, image: &str) -> Option<String> {
        let parsed: serde_json::Value = serde_json::from_str(json_str).ok()?;
        // showmapped returns either an object or array depending on rbd version.
        if let Some(obj) = parsed.as_object() {
            for (_, entry) in obj {
                if entry.get("pool")?.as_str()? == pool && entry.get("name")?.as_str()? == image {
                    return entry.get("device")?.as_str().map(|s| s.to_string());
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CephRbdConfig {
        CephRbdConfig {
            cluster_name: "ceph".to_string(),
            pool_name: "rbd".to_string(),
            user: "admin".to_string(),
            keyring_path: "/etc/ceph/ceph.client.admin.keyring".to_string(),
            monitors: "mon1:6789".to_string(),
        }
    }

    #[test]
    fn ceph_backend_rejects_empty_pool() {
        let mut config = test_config();
        config.pool_name = "".to_string();
        assert!(CephRbdBackend::new(config).is_err());
    }

    #[test]
    fn ceph_backend_rejects_empty_user() {
        let mut config = test_config();
        config.user = "".to_string();
        assert!(CephRbdBackend::new(config).is_err());
    }

    #[test]
    fn ceph_backend_valid_config() {
        let config = test_config();
        assert!(CephRbdBackend::new(config).is_ok());
    }

    #[test]
    fn ceph_sanitize_id_rejects_invalid() {
        assert!(CephRbdBackend::sanitize_id("").is_err());
        assert!(CephRbdBackend::sanitize_id("foo/bar").is_err());
        assert!(CephRbdBackend::sanitize_id("valid-id").is_ok());
        assert!(CephRbdBackend::sanitize_id("valid_id.1").is_ok());
    }

    #[test]
    fn ceph_image_spec_format() {
        let config = test_config();
        let backend = CephRbdBackend::new(config).unwrap();
        assert_eq!(backend.image_spec("vol-1"), "rbd/vol-1");
    }

    #[test]
    fn ceph_find_mapped_device_parses_json() {
        let json = r#"{"0":{"id":"0","pool":"rbd","namespace":"","name":"vol-1","snap":"-","device":"/dev/rbd0"}}"#;
        let result = CephRbdBackend::find_mapped_device(json, "rbd", "vol-1");
        assert_eq!(result, Some("/dev/rbd0".to_string()));
    }

    #[test]
    fn ceph_find_mapped_device_not_found() {
        let json = r#"{"0":{"id":"0","pool":"rbd","namespace":"","name":"other-vol","snap":"-","device":"/dev/rbd0"}}"#;
        let result = CephRbdBackend::find_mapped_device(json, "rbd", "vol-1");
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn ceph_backend_open_rejects_wrong_class() {
        let config = test_config();
        let backend = CephRbdBackend::new(config).unwrap();
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
    async fn ceph_backend_attach_invalid_handle() {
        let config = test_config();
        let backend = CephRbdBackend::new(config).unwrap();
        let res = backend.attach("vol-1", "bad-handle", "vm-1").await;
        assert!(matches!(res, Err(ChvError::InvalidArgument { .. })));
    }

    #[tokio::test]
    async fn ceph_backend_attach_valid_handle() {
        let config = test_config();
        let backend = CephRbdBackend::new(config).unwrap();
        let export = backend
            .attach("vol-1", "rbd-rbd-vol-1", "vm-1")
            .await
            .unwrap();
        assert_eq!(export.export_kind, "rbd");
        assert_eq!(export.export_path, "/dev/rbd/rbd/vol-1");
        assert_eq!(export.attachment_handle, "rbd-rbd-vol-1");
    }

    // C-19 (S4-5): unit-test boundary for the Ceph RBD backend.
    //
    // CephRbdConfig does not derive serde::{Serialize, Deserialize}, so the
    // proposed serde round-trip is out of scope for this change — chv-stord
    // builds the config programmatically.  Pool-name validation today is
    // emptiness-only; we lock that contract down and document the absence of
    // stricter pool-name validation as a known gap for later hardening.

    #[test]
    fn ceph_config_pool_name_validation_is_emptiness_only() {
        // A pool name consisting only of whitespace is technically invalid for
        // RBD ("  " is not a real pool), but the current backend only rejects
        // strictly-empty strings.  This test pins the *current* behavior: if
        // we ever tighten validation, update this test along with the change.
        let mut config = test_config();
        config.pool_name = "   ".to_string();
        assert!(
            CephRbdBackend::new(config).is_ok(),
            "whitespace-only pool name is currently accepted; tighten validation in a follow-up"
        );

        let mut config = test_config();
        config.pool_name = "".to_string();
        assert!(
            matches!(
                CephRbdBackend::new(config),
                Err(ChvError::InvalidArgument { .. })
            ),
            "empty pool name must be rejected"
        );
    }

    /// Health check with a non-existent keyring path.
    ///
    /// `health()` shells out to `rbd info`.  On a host without `rbd`
    /// installed this surfaces `ChvError::Io`; on a host with `rbd` installed
    /// but pointing at a missing keyring + bogus monitor the command exits
    /// non-zero and the backend returns `Ok(BackendHealth)` with
    /// `status == "unhealthy"`.  Both are valid "not connected" signals — the
    /// test asserts we never report "healthy" when authentication cannot
    /// possibly succeed.
    ///
    /// Marked `#[ignore]`: depends on the host having (or not having) the
    /// `rbd` binary; behavior differs across CI environments.
    #[tokio::test]
    #[ignore]
    async fn ceph_health_with_missing_keyring_is_not_reported_healthy() {
        let config = CephRbdConfig {
            cluster_name: "ceph".to_string(),
            pool_name: "rbd".to_string(),
            user: "admin".to_string(),
            keyring_path: "/nonexistent/keyring".to_string(),
            monitors: "192.0.2.1:6789".to_string(),
        };
        let backend = CephRbdBackend::new(config).expect("config is structurally valid");
        let res = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            backend.health("vol-1", "rbd-rbd-vol-1"),
        )
        .await
        .expect("health() should return within 2s when the rbd command fails fast");
        match res {
            Ok(h) => assert_ne!(
                h.status, "healthy",
                "health() must not report healthy with a missing keyring; got {:?}",
                h
            ),
            Err(ChvError::Io { .. }) => {
                // rbd binary not installed on this host — acceptable.
            }
            Err(other) => panic!("unexpected error variant from health(): {:?}", other),
        }
    }
}
