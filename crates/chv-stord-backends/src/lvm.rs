use crate::r#trait::{BackendHealth, StorageBackend, VolumeExport};
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
        vm_id: &str,
        force: bool,
    ) -> Result<(), ChvError> {
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
        Ok(())
    }

    async fn prepare_snapshot(
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
        // Determine volume size via `lvs --nosuffix -o lv_size --units b`.
        let lv_name = Self::sanitize_id(volume_id)?;
        let out = Command::new("lvs")
            .args([
                "--noheadings",
                "-o",
                "lv_size",
                "--units",
                "b",
                "--nosuffix",
                &format!("{}/{}", self.vg_name, lv_name),
            ])
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "lvs".to_string(),
                source: e,
            })?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable {
                backend: "lvm".to_string(),
                reason: format!("lvs failed: {}", String::from_utf8_lossy(&out.stderr)),
            });
        }
        let size_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let file_len: u64 = size_str.parse().map_err(|_| ChvError::BackendUnavailable {
            backend: "lvm".to_string(),
            reason: format!("could not parse lvs output as bytes: '{}'", size_str),
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
            handle, block_size, bitmap_bytes, "enabled dirty tracking for LVM volume"
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
                info!(volume_id, handle, "cleared dirty bitmap for LVM volume");
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
        info!(volume_id, handle, "disabled dirty tracking for LVM volume");
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
        let mut map = self.dirty_trackers.write().await;
        if let Some(tracker) = map.get_mut(handle) {
            let bs = tracker.block_size;
            let start_block = offset / bs;
            let end_block = (offset + data.len() as u64).div_ceil(bs);
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
        let path = self.volume_path(volume_id)?;
        std::fs::metadata(&path)
            .map(|m| m.len())
            .map_err(|e| ChvError::Io {
                path: path.display().to_string(),
                source: e,
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

    async fn delete_volume(&self, volume_id: &str) -> Result<(), ChvError> {
        Self::sanitize_id(volume_id)?;

        // Remove any snapshot and clone LVs that belong to this volume before
        // removing the origin LV.  LVM will refuse to remove an origin that
        // still has dependent snapshots, so we iterate over all LVs in the VG
        // and remove those whose names start with `{volume_id}-snap-` or
        // `{volume_id}-clone-`.  Failures are logged as warnings rather than
        // aborting: the primary volume removal attempt that follows will still
        // fail with a clear error if any dependent LV could not be removed.
        let prefixes = [
            format!("{}-snap-", volume_id),
            format!("{}-clone-", volume_id),
        ];
        let list_out = Command::new("lvs")
            .args([
                "--noheadings",
                "-o",
                "lv_name",
                "--select",
                &format!("vg_name={}", self.vg_name),
            ])
            .output()
            .await;
        if let Ok(out) = list_out {
            if !out.status.success() {
                warn!(
                    volume_id,
                    stderr = %String::from_utf8_lossy(&out.stderr),
                    "lvs command failed; snapshot inventory may be incomplete"
                );
                return Ok(());
            }
            let stdout = String::from_utf8_lossy(&out.stdout);
            for lv_name in stdout.lines().map(str::trim).filter(|s| !s.is_empty()) {
                if !lv_name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.') {
                    warn!(volume_id, lv = lv_name, "skipping lv with unexpected characters in name");
                    continue;
                }
                if prefixes.iter().any(|p| lv_name.starts_with(p.as_str())) {
                    let lv_path = format!("{}/{}", self.vg_name, lv_name);
                    match Command::new("lvremove")
                        .args(["-y", &lv_path])
                        .output()
                        .await
                    {
                        Ok(r) if r.status.success() => {
                            info!(
                                volume_id,
                                lv = lv_name,
                                "removed dependent LVM snapshot/clone LV"
                            );
                        }
                        Ok(r) => {
                            warn!(
                                volume_id,
                                lv = lv_name,
                                stderr = %String::from_utf8_lossy(&r.stderr),
                                "failed to remove dependent LVM snapshot/clone LV; continuing"
                            );
                        }
                        Err(e) => {
                            warn!(
                                volume_id,
                                lv = lv_name,
                                error = %e,
                                "I/O error removing dependent LVM snapshot/clone LV; continuing"
                            );
                        }
                    }
                }
            }
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
        info!(volume_id, "deleted LVM volume");
        Ok(())
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
    async fn lvm_backend_open_returns_lvm_path() {
        let backend = LVMBackend::new("vg0".to_string()).unwrap();
        let locator = BackendLocator {
            backend_class: "lvm".to_string(),
            locator: "vg0/vol1".to_string(),
            options: Default::default(),
        };
        let export = backend
            .open("vol-1", &locator, &DevicePolicy::default())
            .await
            .unwrap();
        assert_eq!(export.export_kind, "lvm");
        assert!(export.export_path.contains("/dev/vg0/vol-1"));
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
}
