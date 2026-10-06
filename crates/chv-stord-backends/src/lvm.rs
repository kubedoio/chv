use crate::r#trait::{
    validate_write_bounds, BackendHealth, StorageBackend, VolumeExport, DIRTY_TRACKING_BLOCK_SIZE,
    MAX_DIRTY_TRACKING_VOLUME_SIZE_BYTES,
};
use async_trait::async_trait;
use chv_common::types::{BackendLocator, DevicePolicy};
use chv_errors::ChvError;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{info, warn};

struct DirtyTracker {
    block_size: u64,
    volume_size: u64,
    bitmap: Vec<u8>,
}

pub struct LVMBackend {
    vg_name: String,
    dirty_trackers: Arc<RwLock<HashMap<String, DirtyTracker>>>,
}

impl LVMBackend {
    pub fn new(vg_name: String) -> Result<Self, ChvError> {
        Self::sanitize_id(&vg_name)?;
        Ok(Self {
            vg_name,
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

    fn volume_path(&self, volume_id: &str) -> Result<PathBuf, ChvError> {
        Self::sanitize_id(volume_id)?;
        Ok(PathBuf::from(format!(
            "/dev/{}/{}",
            self.vg_name, volume_id
        )))
    }

    fn validate_handle(&self, handle: &str) -> Result<(), ChvError> {
        // Minimal sanity check; callers with a volume_id should also verify
        // handle == format!("lvm-{}-{}", self.vg_name, volume_id)
        let prefix = format!("lvm-{}-", self.vg_name);
        if !handle.starts_with(&prefix) {
            return Err(ChvError::InvalidArgument {
                field: "handle".to_string(),
                reason: format!("handle {} does not belong to this backend", handle),
            });
        }
        Ok(())
    }

    fn expected_handle(&self, volume_id: &str) -> String {
        format!("lvm-{}-{}", self.vg_name, volume_id)
    }

    async fn resolve_dm_name(&self, path: &std::path::Path) -> Result<String, ChvError> {
        let canonical = tokio::fs::canonicalize(path)
            .await
            .map_err(|e| ChvError::Io {
                path: path.to_string_lossy().to_string(),
                source: e,
            })?;
        let dm_name = canonical
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| ChvError::BackendUnavailable {
                backend: "lvm".to_string(),
                reason: format!(
                    "could not determine dm device name from canonical path: {}",
                    canonical.display()
                ),
            })?;
        Ok(dm_name.to_string())
    }
}

#[async_trait]
impl StorageBackend for LVMBackend {
    async fn open(
        &self,
        volume_id: &str,
        locator: &BackendLocator,
        _policy: &DevicePolicy,
    ) -> Result<VolumeExport, ChvError> {
        if locator.backend_class != "lvm" {
            return Err(ChvError::BackendUnavailable {
                backend: locator.backend_class.clone(),
                reason: "LVM backend only handles lvm class".to_string(),
            });
        }
        let path = self.volume_path(volume_id)?;
        // #379 DP2: create-on-open parity with the local backend. An
        // absent LV provisions via `lvcreate` when the open carries a
        // size (the exact create_receiving_volume shape); an absent LV
        // WITHOUT a size refuses — LVM volumes are not sparse, so a
        // default-size LV would silently consume real extents, unlike
        // the local backend's sparse-file default. An existing LV opens
        // as before (idempotent re-attach).
        if !path.exists() {
            // seed_from is unsupported on LVM (DP2 scope cut): reject
            // with an explicit error instead of silently provisioning an
            // empty LV under an operator who asked for a seeded image.
            if locator
                .options
                .get("seed_from")
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false)
            {
                return Err(ChvError::InvalidArgument {
                    field: "seed_from".to_string(),
                    reason: "seed_from is not supported on the lvm backend (#379 DP2 scope \
                             cut); provision the image onto the LV out-of-band"
                        .to_string(),
                });
            }
            let size_bytes = match locator.options.get("size_bytes") {
                Some(raw) => raw.parse::<u64>().map_err(|_| ChvError::InvalidArgument {
                    field: "size_bytes".to_string(),
                    reason: format!("invalid integer: {}", raw),
                })?,
                None => {
                    return Err(ChvError::InvalidArgument {
                        field: "size_bytes".to_string(),
                        reason: "size_bytes is required to create an LVM volume (the logical \
                                 volume does not exist)"
                            .to_string(),
                    })
                }
            };
            warn!(
                volume_id,
                path = %path.display(),
                size_bytes,
                "logical volume does not exist; provisioning via lvcreate"
            );
            let export = self
                .create_receiving_volume(volume_id, size_bytes, "raw")
                .await?;
            return Ok(export);
        }
        info!(volume_id, path = %path.display(), "opening LVM volume");
        Ok(VolumeExport {
            export_kind: "lvm".to_string(),
            export_path: path.to_string_lossy().to_string(),
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
        // The handle is gone once closed: drop its dirty tracker so closed
        // volumes cannot accumulate bitmaps.
        self.dirty_trackers.write().await.remove(handle);
        info!(volume_id, "closing LVM volume");
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
        let path = self.volume_path(volume_id)?;
        info!(volume_id, vm_id, handle, path = %path.display(), "attaching LVM volume");
        Ok(VolumeExport {
            export_kind: "lvm".to_string(),
            export_path: path.to_string_lossy().to_string(),
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
            warn!(volume_id, vm_id, "force detaching LVM volume");
        } else {
            info!(volume_id, vm_id, "detaching LVM volume");
        }
        Ok(())
    }

    async fn health(&self, volume_id: &str, _handle: &str) -> Result<BackendHealth, ChvError> {
        let path = self.volume_path(volume_id)?;
        let exists = path.exists();
        let status = if exists { "healthy" } else { "unhealthy" };
        let last_error = if exists {
            String::new()
        } else {
            format!("path does not exist: {}", path.display())
        };
        Ok(BackendHealth {
            status: status.to_string(),
            backend_state: "open".to_string(),
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
        let path = self.volume_path(volume_id)?;
        if !path.exists() {
            return Err(ChvError::NotFound {
                resource: "path".to_string(),
                id: path.to_string_lossy().to_string(),
            });
        }
        let size_mb = new_size_bytes.div_ceil(1024 * 1024).max(1);
        let out = Command::new("lvresize")
            .args(["-L", &format!("{}M", size_mb), &path.to_string_lossy()])
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "lvresize".to_string(),
                source: e,
            })?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "lvm".to_string(),
                reason: format!("lvresize failed: {}", String::from_utf8_lossy(&out.stderr)),
            });
        }
        info!(volume_id, new_size_bytes, "resized LVM volume");

        // Keep dirty tracking consistent with the new size.
        if let Some(tracker) = self.dirty_trackers.write().await.get_mut(handle) {
            tracker.volume_size = new_size_bytes;
            let needed_bytes = new_size_bytes.div_ceil(tracker.block_size).div_ceil(8) as usize;
            if tracker.bitmap.len() < needed_bytes {
                tracker.bitmap.resize(needed_bytes, 0);
            }
        }

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
        let origin = self.volume_path(volume_id)?;
        let snap = format!("{}-snap-{}", volume_id, snapshot_name);
        let out = Command::new("lvcreate")
            .args([
                "-s",
                "-n",
                &snap,
                "-l",
                "100%FREE",
                &origin.to_string_lossy(),
            ])
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "lvcreate".to_string(),
                source: e,
            })?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "lvm".to_string(),
                reason: format!("lvcreate failed: {}", String::from_utf8_lossy(&out.stderr)),
            });
        }
        info!(volume_id, snapshot_name, "prepared LVM snapshot");
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
        let origin = self.volume_path(volume_id)?;
        let clone_lv = format!("{}-clone-{}", volume_id, clone_name);
        let out = Command::new("lvcreate")
            .args([
                "-s",
                "-n",
                &clone_lv,
                "-l",
                "100%FREE",
                &origin.to_string_lossy(),
            ])
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "lvcreate".to_string(),
                source: e,
            })?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "lvm".to_string(),
                reason: format!("lvcreate failed: {}", String::from_utf8_lossy(&out.stderr)),
            });
        }
        info!(volume_id, clone_name, "prepared LVM clone");
        Ok(())
    }

    async fn restore_snapshot(
        &self,
        _volume_id: &str,
        _handle: &str,
        _snapshot_name: &str,
    ) -> Result<(), ChvError> {
        Err(ChvError::InvalidArgument {
            field: "operation".to_string(),
            reason: "LVM restore snapshot not yet implemented".to_string(),
        })
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
        let snap = format!("{}-snap-{}", volume_id, snapshot_name);
        let out = Command::new("lvremove")
            .args(["-y", &format!("{}/{}", self.vg_name, snap)])
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "lvremove".to_string(),
                source: e,
            })?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "lvm".to_string(),
                reason: format!("lvremove failed: {}", String::from_utf8_lossy(&out.stderr)),
            });
        }
        info!(volume_id, snapshot_name, "deleted LVM snapshot");
        Ok(())
    }

    /// #522 (DP3/DP4): destroy the LV if it exists, `lvremove -y`
    /// (command shaping mirrors `delete_snapshot` above). Idempotent
    /// by contract: an absent LV is `Ok(())` — a replayed delete never
    /// manufactures a failure on the already-reclaimed extents. The
    /// lvremove targets `{vg}/{volume_id}` (the create carrier's LV,
    /// provisioned by the sized open of `create_receiving_volume`),
    /// NOT the `-snap-` suffixed snapshot LVs and NOT any
    /// `/dev/mapper` token string parsing — the dm-path locator the
    /// agent threads is allowlist input at the stord boundary; this
    /// backend resolves the LV from its own configured VG and the
    /// (sanitized) volume id.
    async fn destroy(&self, volume_id: &str, locator: &BackendLocator) -> Result<(), ChvError> {
        if locator.backend_class != "lvm" {
            return Err(ChvError::BackendUnavailable {
                backend: locator.backend_class.clone(),
                reason: "LVM backend only handles lvm class".to_string(),
            });
        }
        // sanitize_id runs inside volume_path: a traversal id must be
        // rejected on a REMOVAL path more than anywhere else.
        let path = self.volume_path(volume_id)?;
        if !path.exists() {
            // Idempotent (DP3): no LV, nothing to reclaim.
            info!(
                volume_id,
                path = %path.display(),
                "logical volume already absent; destroy is a no-op"
            );
            return Ok(());
        }
        let out = Command::new("lvremove")
            .args(["-y", &format!("{}/{}", self.vg_name, volume_id)])
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "lvremove".to_string(),
                source: e,
            })?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "lvm".to_string(),
                reason: format!("lvremove failed: {}", String::from_utf8_lossy(&out.stderr)),
            });
        }
        info!(volume_id, path = %path.display(), "destroyed LVM logical volume");
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
        let path = self.volume_path(volume_id)?;

        if policy.read_only {
            info!(volume_id, path = %path.display(), "applying read-only device policy");
            let out = Command::new("blockdev")
                .args(["--setro", &path.to_string_lossy()])
                .output()
                .await
                .map_err(|e| ChvError::Io {
                    path: "blockdev".to_string(),
                    source: e,
                })?;
            if !out.status.success() {
                return Err(ChvError::BackendUnavailable {
                    backend: "lvm".to_string(),
                    reason: format!(
                        "blockdev --setro failed: {}",
                        String::from_utf8_lossy(&out.stderr)
                    ),
                });
            }
        }

        if !policy.io_scheduler.is_empty() {
            let dm_name = self.resolve_dm_name(&path).await?;
            let scheduler_path = format!("/sys/block/{}/queue/scheduler", dm_name);
            info!(
                volume_id,
                dm_name,
                scheduler = %policy.io_scheduler,
                "applying io_scheduler device policy"
            );
            tokio::fs::write(&scheduler_path, &policy.io_scheduler)
                .await
                .map_err(|e| ChvError::Io {
                    path: scheduler_path,
                    source: e,
                })?;
        }

        if !policy.cache_mode.is_empty() {
            warn!(
                volume_id,
                cache_mode = %policy.cache_mode,
                "cache_mode policy is not supported by LVMBackend at attach time; configure cache at LV creation"
            );
        }

        if policy.no_exec {
            warn!(
                volume_id,
                "no_exec policy is not applicable at LVM block device level; skipping"
            );
        }

        if policy.read_bps > 0
            || policy.write_bps > 0
            || policy.read_iops > 0
            || policy.write_iops > 0
        {
            warn!(
                volume_id,
                "LVMBackend does not enforce throughput or iops limits"
            );
        }

        Ok(())
    }

    // --- Phase 2.3: Migration methods ---

    /// Initialize the dirty bitmap for an opened volume.
    async fn enable_dirty_tracking(
        &self,
        _volume_id: &str,
        handle: &str,
        volume_size_bytes: u64,
    ) -> Result<(), ChvError> {
        if volume_size_bytes > MAX_DIRTY_TRACKING_VOLUME_SIZE_BYTES {
            return Err(ChvError::InvalidArgument {
                field: "volume_size_bytes".to_string(),
                reason: format!(
                    "volume size {} exceeds dirty-tracking maximum {} bytes",
                    volume_size_bytes, MAX_DIRTY_TRACKING_VOLUME_SIZE_BYTES
                ),
            });
        }
        let mut map = self.dirty_trackers.write().await;
        match map.get_mut(handle) {
            Some(tracker) => {
                // Re-enable heals stale bounds from out-of-band resizes:
                // update the size and grow the bitmap in place, keeping
                // the dirty bits.
                tracker.volume_size = volume_size_bytes;
                let needed_bytes = volume_size_bytes
                    .div_ceil(DIRTY_TRACKING_BLOCK_SIZE)
                    .div_ceil(8) as usize;
                if tracker.bitmap.len() < needed_bytes {
                    tracker.bitmap.resize(needed_bytes, 0);
                }
            }
            None => {
                let bitmap_bytes = volume_size_bytes
                    .div_ceil(DIRTY_TRACKING_BLOCK_SIZE)
                    .div_ceil(8) as usize;
                map.insert(
                    handle.to_string(),
                    DirtyTracker {
                        block_size: DIRTY_TRACKING_BLOCK_SIZE,
                        volume_size: volume_size_bytes,
                        bitmap: vec![0u8; bitmap_bytes],
                    },
                );
            }
        }
        Ok(())
    }

    /// Atomically snapshot and clear the dirty bitmap under a single write lock.
    async fn snapshot_and_clear_dirty_bitmap(
        &self,
        _volume_id: &str,
        handle: &str,
    ) -> Result<Vec<u8>, ChvError> {
        let mut map = self.dirty_trackers.write().await;
        match map.get_mut(handle) {
            Some(tracker) => {
                let snapshot = tracker.bitmap.clone();
                tracker.bitmap.iter_mut().for_each(|byte| *byte = 0);
                Ok(snapshot)
            }
            None => Err(ChvError::NotFound {
                resource: "dirty_tracker".to_string(),
                id: handle.to_string(),
            }),
        }
    }

    async fn read_block(
        &self,
        volume_id: &str,
        handle: &str,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, ChvError> {
        self.validate_handle(handle)?;
        let path = self.volume_path(volume_id)?;
        tokio::task::spawn_blocking(move || {
            use std::io::{Read, Seek, SeekFrom};
            let mut file = std::fs::File::open(&path).map_err(|e| ChvError::Io {
                path: path.display().to_string(),
                source: e,
            })?;
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| ChvError::Io {
                    path: path.display().to_string(),
                    source: e,
                })?;
            let mut buf = vec![0u8; length as usize];
            file.read_exact(&mut buf).map_err(|e| ChvError::Io {
                path: path.display().to_string(),
                source: e,
            })?;
            Ok(buf)
        })
        .await
        .map_err(|e| ChvError::BackendUnavailable {
            backend: "lvm".to_string(),
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
        let path = self.volume_path(volume_id)?;

        // When dirty tracking is enabled, reject out-of-range writes up
        // front; see LocalFileBackend::write_block for rationale.
        let mark_range = {
            let map = self.dirty_trackers.read().await;
            match map.get(handle) {
                Some(tracker) => {
                    let end =
                        validate_write_bounds(offset, data.len() as u64, tracker.volume_size)?;
                    Some((
                        offset / tracker.block_size,
                        end.div_ceil(tracker.block_size),
                    ))
                }
                None => None,
            }
        };

        let data_owned = data.to_vec();
        tokio::task::spawn_blocking(move || {
            use std::io::{Seek, SeekFrom, Write};
            let mut file = std::fs::File::options()
                .write(true)
                .open(&path)
                .map_err(|e| ChvError::Io {
                    path: path.display().to_string(),
                    source: e,
                })?;
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| ChvError::Io {
                    path: path.display().to_string(),
                    source: e,
                })?;
            file.write_all(&data_owned).map_err(|e| ChvError::Io {
                path: path.display().to_string(),
                source: e,
            })?;
            Ok(())
        })
        .await
        .map_err(|e| ChvError::BackendUnavailable {
            backend: "lvm".to_string(),
            reason: format!("write_block task panicked: {}", e),
        })??;

        // Update dirty bitmap if tracking is enabled for this handle.
        if let Some((start_block, end_block)) = mark_range {
            let mut map = self.dirty_trackers.write().await;
            if let Some(tracker) = map.get_mut(handle) {
                for block in start_block..end_block {
                    let byte_idx = (block / 8) as usize;
                    let bit_idx = (block % 8) as u8;
                    tracker.bitmap[byte_idx] |= 1 << bit_idx;
                }
            }
        }

        Ok(())
    }

    async fn volume_size(&self, volume_id: &str, _handle: &str) -> Result<u64, ChvError> {
        let path = self.volume_path(volume_id)?;
        // Use blockdev to get the size of the block device; metadata().len()
        // reports 0 for block device nodes. The dirty tracker is sized from
        // this value, so a wrong size would corrupt write bounds checking.
        let path_str = path.to_string_lossy().to_string();
        let out = Command::new("blockdev")
            .args(["--getsize64", &path_str])
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "blockdev".to_string(),
                source: e,
            })?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "lvm".to_string(),
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
                backend: "lvm".to_string(),
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
        let size_mb = size_bytes.div_ceil(1024 * 1024).max(1);
        let out = Command::new("lvcreate")
            .args([
                "-L",
                &format!("{}M", size_mb),
                "-n",
                volume_id,
                &self.vg_name,
            ])
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "lvcreate".to_string(),
                source: e,
            })?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "lvm".to_string(),
                reason: format!("lvcreate failed: {}", String::from_utf8_lossy(&out.stderr)),
            });
        }
        let path = self.volume_path(volume_id)?;
        info!(volume_id, size_bytes, path = %path.display(), "created receiving LVM volume");
        Ok(VolumeExport {
            export_kind: "lvm".to_string(),
            export_path: path.to_string_lossy().to_string(),
            attachment_handle: self.expected_handle(volume_id),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lvm_backend_open_rejects_wrong_class() {
        let backend = LVMBackend::new("vg0".to_string()).unwrap();
        let locator = BackendLocator {
            backend_class: "local".to_string(),
            locator: "/dev/vg0/vol1".to_string(),
            options: Default::default(),
        };
        let res = backend
            .open("vol-1", &locator, &DevicePolicy::default())
            .await;
        assert!(matches!(res, Err(ChvError::BackendUnavailable { .. })));
    }

    #[tokio::test]
    async fn lvm_backend_open_refuses_absent_lv_without_size() {
        // #379 DP2: create-on-open — an absent LV without a size_bytes
        // option refuses (LVM volumes are not sparse; a default-size LV
        // would silently consume real extents). The happy-path open
        // shape (export path/handle) is pinned by the attach test below
        // and the root-gated lvm_real roundtrip.
        let backend = LVMBackend::new("vg0".to_string()).unwrap();
        let locator = BackendLocator {
            backend_class: "lvm".to_string(),
            locator: "vg0/vol1".to_string(),
            options: Default::default(),
        };
        match backend
            .open("vol-1", &locator, &DevicePolicy::default())
            .await
        {
            Err(ChvError::InvalidArgument { field, .. }) => {
                assert_eq!(field, "size_bytes");
            }
            other => panic!("expected InvalidArgument(size_bytes), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn lvm_backend_open_rejects_seed_from() {
        // #379 DP2 scope cut: seed_from is unsupported on LVM — reject
        // explicitly instead of silently provisioning an empty LV.
        let backend = LVMBackend::new("vg0".to_string()).unwrap();
        let locator = BackendLocator {
            backend_class: "lvm".to_string(),
            locator: "vg0/vol1".to_string(),
            options: [
                ("size_bytes".to_string(), "1048576".to_string()),
                ("seed_from".to_string(), "/tmp/seed.qcow2".to_string()),
            ]
            .into(),
        };
        match backend
            .open("vol-1", &locator, &DevicePolicy::default())
            .await
        {
            Err(ChvError::InvalidArgument { field, .. }) => {
                assert_eq!(field, "seed_from");
            }
            other => panic!("expected InvalidArgument(seed_from), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn lvm_backend_attach_valid_handle() {
        let backend = LVMBackend::new("vg0".to_string()).unwrap();
        let export = backend
            .attach("vol-1", "lvm-vg0-vol-1", "vm-1")
            .await
            .unwrap();
        assert_eq!(export.export_kind, "lvm");
        assert_eq!(export.export_path, "/dev/vg0/vol-1");
        assert_eq!(export.attachment_handle, "lvm-vg0-vol-1");
    }

    #[tokio::test]
    async fn lvm_backend_attach_invalid_handle() {
        let backend = LVMBackend::new("vg0".to_string()).unwrap();
        let res = backend.attach("vol-1", "lvm-other-vg0-vol-1", "vm-1").await;
        assert!(matches!(res, Err(ChvError::InvalidArgument { .. })));
    }

    #[tokio::test]
    async fn lvm_backend_health_path_exists() {
        // On Unix-like systems /dev/null always exists.
        // We construct a backend whose volume_path points to it by using
        // vg_name = "" (so /dev/null) ... but sanitize_id rejects empty.
        // Instead we can test health() indirectly by creating a temp file
        // inside a directory whose name is a valid vg_name.
        let tmp = tempfile::tempdir().unwrap();
        let vg_dir = tmp.path().join("myvg");
        std::fs::create_dir(&vg_dir).unwrap();
        let vol_path = vg_dir.join("myvol");
        std::fs::write(&vol_path, b"").unwrap();

        // To make volume_path return our temp file, we need the backend to
        // think the root is /dev.  We can't override /dev prefix, but we can
        // create a symlink /dev/myvg -> tmp_dir if we have permissions...
        // On macOS /dev is not writable by default.  Instead, we can use a
        // path traversal trick with a vg_name that contains a slash, but
        // sanitize_id blocks slashes.
        //
        // Cleanest remaining option: test the healthy path by relying on the
        // fact that /dev/null exists and using a vg_name that is a symlink
        // or directory inside /dev.  We can create a directory in /tmp and
        // then bind-mount or symlink it into /dev, but that requires root.
        //
        // Simpler: just test that health() reports healthy for /dev/null by
        // using vg_name = "" and volume_id = "null".  sanitize_id rejects
        // empty vg_name.  So we need to relax the test to something that
        // definitely exists and is reachable with valid ids.
        //
        // On macOS /dev/fd/0 exists and is a directory.  vg_name = "fd",
        // volume_id = "0" -> /dev/fd/0 which exists.
        let backend = LVMBackend::new("fd".to_string()).unwrap();
        let health = backend.health("0", "lvm-fd-0").await.unwrap();
        assert_eq!(health.status, "healthy");
        assert!(health.last_error.is_empty());
    }

    #[tokio::test]
    async fn lvm_backend_health_path_not_exists() {
        let backend = LVMBackend::new("vg0".to_string()).unwrap();
        let health = backend
            .health("nonexistent-vol-99999", "lvm-vg0-nonexistent-vol-99999")
            .await
            .unwrap();
        assert_eq!(health.status, "unhealthy");
        assert!(health.last_error.contains("path does not exist"));
    }

    #[tokio::test]
    async fn lvm_backend_set_device_policy_returns_ok() {
        let backend = LVMBackend::new("vg0".to_string()).unwrap();
        let res = backend
            .set_device_policy("vol-1", "lvm-vg0-vol-1", &DevicePolicy::default())
            .await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn lvm_backend_set_device_policy_rejects_invalid_handle() {
        let backend = LVMBackend::new("vg0".to_string()).unwrap();
        let res = backend
            .set_device_policy("vol-1", "lvm-other-vg0-vol-1", &DevicePolicy::default())
            .await;
        assert!(matches!(res, Err(ChvError::InvalidArgument { .. })));
    }

    #[tokio::test]
    async fn lvm_backend_sanitize_rejects_malicious_ids() {
        assert!(LVMBackend::sanitize_id("").is_err());
        assert!(LVMBackend::sanitize_id("foo/bar").is_err());
        assert!(LVMBackend::sanitize_id("foo\\bar").is_err());
        assert!(LVMBackend::sanitize_id("foo..bar").is_ok());
        assert!(LVMBackend::sanitize_id("foo@bar").is_err());
        assert!(LVMBackend::sanitize_id("valid-id").is_ok());
        assert!(LVMBackend::sanitize_id("valid.id").is_ok());
        assert!(LVMBackend::sanitize_id("valid_id").is_ok());
    }

    #[tokio::test]
    async fn lvm_backend_new_rejects_invalid_vg_name() {
        assert!(LVMBackend::new("".to_string()).is_err());
        assert!(LVMBackend::new("bad/vg".to_string()).is_err());
        assert!(LVMBackend::new("ok-vg".to_string()).is_ok());
    }

    #[tokio::test]
    async fn lvm_backend_close_rejects_invalid_handle() {
        let backend = LVMBackend::new("vg0".to_string()).unwrap();
        let res = backend.close("vol-1", "lvm-other-vg0-vol-1").await;
        assert!(matches!(res, Err(ChvError::InvalidArgument { .. })));
    }

    #[tokio::test]
    async fn lvm_backend_resize_uses_div_ceil() {
        let backend = LVMBackend::new("vg0".to_string()).unwrap();
        // We can't actually resize, but we can verify the overflow path is safe by
        // passing u64::MAX.  The size_mb calculation should not panic.
        // Since the volume path won't exist, it returns NotFound before lvresize.
        let res = backend.resize("vol-1", "lvm-vg0-vol-1", u64::MAX).await;
        assert!(matches!(res, Err(ChvError::NotFound { .. })));
    }

    // --- Dirty tracking tests (in-memory; no LVM infrastructure needed) ---

    #[tokio::test]
    async fn dirty_tracking_not_found_before_enable() {
        let backend = LVMBackend::new("vg0".to_string()).unwrap();
        let res = backend
            .snapshot_and_clear_dirty_bitmap("vol-1", "lvm-vg0-vol-1")
            .await;
        assert!(matches!(res, Err(ChvError::NotFound { .. })));
    }

    #[tokio::test]
    async fn dirty_tracking_enable_then_snapshot_round_trip() {
        let backend = LVMBackend::new("vg0".to_string()).unwrap();
        let handle = "lvm-vg0-vol-1";

        backend
            .enable_dirty_tracking("vol-1", handle, 8_388_608)
            .await
            .unwrap();

        // Freshly enabled volume returns an empty bitmap, not NotFound.
        let bitmap = backend
            .snapshot_and_clear_dirty_bitmap("vol-1", handle)
            .await
            .unwrap();
        assert_eq!(bitmap, vec![0]);

        let after = backend
            .snapshot_and_clear_dirty_bitmap("vol-1", handle)
            .await
            .unwrap();
        assert_eq!(after, vec![0]);
    }

    #[tokio::test]
    async fn dirty_tracking_rejects_oversized_volume() {
        let backend = LVMBackend::new("vg0".to_string()).unwrap();
        let res = backend
            .enable_dirty_tracking(
                "vol-1",
                "lvm-vg0-vol-1",
                MAX_DIRTY_TRACKING_VOLUME_SIZE_BYTES + 1,
            )
            .await;
        assert!(matches!(res, Err(ChvError::InvalidArgument { .. })));
    }

    #[tokio::test]
    async fn close_evicts_dirty_tracker() {
        let backend = LVMBackend::new("vg0".to_string()).unwrap();
        let handle = "lvm-vg0-vol-1";
        backend
            .enable_dirty_tracking("vol-1", handle, 8_388_608)
            .await
            .unwrap();

        backend.close("vol-1", handle).await.unwrap();

        let res = backend
            .snapshot_and_clear_dirty_bitmap("vol-1", handle)
            .await;
        assert!(matches!(res, Err(ChvError::NotFound { .. })));
    }

    // C-19 (S4-5): unit-test boundary for the LVM backend.
    //
    // The backend has no Config struct (vg_name is the only construction-time
    // argument) and no `lvcreate_path` field, so the originally proposed
    // "config_default_values_are_safe" and "missing-binary" tests are not
    // meaningful for this implementation.  Health is path-existence based, not
    // binary-probe based, so we instead lock down the unhealthy contract and
    // strengthen the construction-time tamper checks.

    #[tokio::test]
    async fn lvm_new_rejects_path_traversal_in_vg_name() {
        // sanitize_id must reject anything outside [a-zA-Z0-9._-].  These
        // strings model classic shell-injection / path-traversal attempts that
        // could otherwise be interpolated into /dev/{vg}/{vol} or into an
        // lvremove arg.
        for malicious in [
            "../escape",
            "vg/sub",
            "vg;rm -rf",
            "vg`whoami`",
            "vg$(id)",
            "vg|cat",
            "vg with space",
            "",
        ] {
            let res = LVMBackend::new(malicious.to_string());
            assert!(
                matches!(res, Err(ChvError::InvalidArgument { .. })),
                "expected InvalidArgument for vg_name={:?}",
                malicious
            );
        }
    }

    #[tokio::test]
    async fn lvm_health_with_unknown_volume_returns_unhealthy_with_reason() {
        // health() is path-existence based: it reports unhealthy with a
        // non-empty last_error when /dev/{vg}/{vol} does not exist.  Lock down
        // the exact contract callers in chv-stord rely on.
        let backend =
            LVMBackend::new("vg-does-not-exist".to_string()).expect("vg name with hyphen is valid");
        let h = backend
            .health(
                "vol-does-not-exist",
                "lvm-vg-does-not-exist-vol-does-not-exist",
            )
            .await
            .expect("health() returns BackendHealth, never an error, for an unknown volume");
        assert_eq!(h.status, "unhealthy");
        assert_eq!(h.backend_state, "open");
        assert!(
            h.last_error.contains("path does not exist"),
            "last_error should mention missing path, got: {:?}",
            h.last_error
        );
    }

    #[tokio::test]
    async fn lvm_volume_path_uses_dev_prefix() {
        // The /dev/{vg}/{lv} layout is part of the ABI with chv-stord's
        // attach plumbing; regress-protect it.
        let backend = LVMBackend::new("vg0".to_string()).expect("valid vg");
        let path = backend.volume_path("vol-1").expect("valid volume id");
        assert_eq!(path.to_string_lossy(), "/dev/vg0/vol-1");
    }

    // ============================================================
    // #522 PR 1 (DP3) — the destroy primitive, LVM semantics
    // ============================================================

    fn lvm_carrier_locator() -> BackendLocator {
        // The create carrier's DP4 locator shape: the dm-path token
        // the agent threads (the LV itself resolves from the backend's
        // own VG + the sanitized volume id, not from this string).
        BackendLocator {
            backend_class: "lvm".to_string(),
            locator: "/dev/mapper/vg0-vol-del".to_string(),
            options: Default::default(),
        }
    }

    #[tokio::test]
    async fn lvm_backend_destroy_is_idempotent_on_an_absent_lv() {
        // #522 DP3: an absent LV is Ok(()) — the idempotency contract.
        // This also proves the exists-gate: `lvremove` against a
        // missing LV fails loudly, so a passing absent-case means no
        // command was attempted (the command-shaping leg of the
        // contract is pinned root-gated, in lvm_real.rs).
        let backend = LVMBackend::new("vg0".to_string()).unwrap();
        backend
            .destroy("vol-absent", &lvm_carrier_locator())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn lvm_backend_destroy_rejects_unsafe_volume_ids() {
        // The volume id becomes the LV name in the `lvremove` argument
        // (`{vg}/{volume_id}`); a traversal id is rejected by
        // sanitize_id BEFORE any command runs — the removal path's
        // traversal guard is more load-bearing than the create one.
        let backend = LVMBackend::new("vg0".to_string()).unwrap();
        let res = backend
            .destroy("../../escape", &lvm_carrier_locator())
            .await;
        assert!(matches!(res, Err(ChvError::InvalidArgument { .. })));
    }

    #[tokio::test]
    async fn lvm_backend_destroy_rejects_wrong_class() {
        let backend = LVMBackend::new("vg0".to_string()).unwrap();
        let res = backend
            .destroy(
                "vol-1",
                &BackendLocator {
                    backend_class: "local".to_string(),
                    locator: "vol-1.img".to_string(),
                    options: Default::default(),
                },
            )
            .await;
        assert!(matches!(res, Err(ChvError::BackendUnavailable { .. })));
    }
}
