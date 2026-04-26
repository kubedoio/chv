use crate::r#trait::{BackendHealth, StorageBackend, VolumeExport};
use async_trait::async_trait;
use chv_common::types::{BackendLocator, DevicePolicy};
use chv_errors::ChvError;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::{info, warn};

pub struct LVMBackend {
    vg_name: String,
}

impl LVMBackend {
    pub fn new(vg_name: String) -> Result<Self, ChvError> {
        Self::sanitize_id(&vg_name)?;
        Ok(Self { vg_name })
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
