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
    pub fn new(vg_name: String) -> Self {
        Self { vg_name }
    }

    fn volume_path(&self, volume_id: &str) -> PathBuf {
        PathBuf::from(format!("/dev/{}/{}", self.vg_name, volume_id))
    }
}

#[async_trait]
impl StorageBackend for LVMBackend {
    async fn open(&self, volume_id: &str, locator: &BackendLocator, _policy: &DevicePolicy) -> Result<VolumeExport, ChvError> {
        if locator.backend_class != "lvm" {
            return Err(ChvError::BackendUnavailable {
                backend: locator.backend_class.clone(),
                reason: "LVM backend only handles lvm class".to_string(),
            });
        }
        let path = self.volume_path(volume_id);
        info!(volume_id, path = %path.display(), "opening LVM volume");
        Ok(VolumeExport {
            export_kind: "lvm".to_string(),
            export_path: path.to_string_lossy().to_string(),
            attachment_handle: format!("lvm-{}-{}", self.vg_name, volume_id),
        })
    }

    async fn close(&self, volume_id: &str, _handle: &str) -> Result<(), ChvError> {
        info!(volume_id, "closing LVM volume");
        Ok(())
    }

    async fn attach(&self, volume_id: &str, handle: &str, vm_id: &str) -> Result<VolumeExport, ChvError> {
        let prefix = format!("lvm-{}-", self.vg_name);
        if !handle.starts_with(&prefix) {
            return Err(ChvError::BackendUnavailable {
                backend: "lvm".to_string(),
                reason: format!("handle {} does not belong to this backend", handle),
            });
        }
        let path = self.volume_path(volume_id);
        info!(volume_id, vm_id, handle, path = %path.display(), "attaching LVM volume");
        Ok(VolumeExport {
            export_kind: "lvm".to_string(),
            export_path: path.to_string_lossy().to_string(),
            attachment_handle: handle.to_string(),
        })
    }

    async fn detach(&self, volume_id: &str, _handle: &str, vm_id: &str, force: bool) -> Result<(), ChvError> {
        if force {
            warn!(volume_id, vm_id, "force detaching LVM volume");
        } else {
            info!(volume_id, vm_id, "detaching LVM volume");
        }
        Ok(())
    }

    async fn health(&self, volume_id: &str, _handle: &str) -> Result<BackendHealth, ChvError> {
        let path = self.volume_path(volume_id);
        let status = if path.exists() { "healthy" } else { "unhealthy" };
        let last_error = if path.exists() { String::new() } else { format!("path does not exist: {}", path.display()) };
        Ok(BackendHealth { status: status.to_string(), backend_state: "open".to_string(), last_error })
    }

    async fn resize(&self, volume_id: &str, handle: &str, new_size_bytes: u64) -> Result<(), ChvError> {
        let prefix = format!("lvm-{}-", self.vg_name);
        if !handle.starts_with(&prefix) {
            return Err(ChvError::BackendUnavailable { backend: "lvm".to_string(), reason: format!("handle {} does not belong to this backend", handle) });
        }
        let path = self.volume_path(volume_id);
        if !path.exists() {
            return Err(ChvError::NotFound { resource: "path".to_string(), id: path.to_string_lossy().to_string() });
        }
        let size_mb = ((new_size_bytes + 1024 * 1024 - 1) / (1024 * 1024)).max(1);
        let out = Command::new("lvresize")
            .args(["-L", &format!("{}M", size_mb), &path.to_string_lossy()])
            .output()
            .await
            .map_err(|e| ChvError::Io { path: "lvresize".to_string(), source: e })?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable { backend: "lvm".to_string(), reason: format!("lvresize failed: {}", String::from_utf8_lossy(&out.stderr)) });
        }
        info!(volume_id, new_size_bytes, "resized LVM volume");
        Ok(())
    }

    async fn prepare_snapshot(&self, volume_id: &str, handle: &str, snapshot_name: &str) -> Result<(), ChvError> {
        let prefix = format!("lvm-{}-", self.vg_name);
        if !handle.starts_with(&prefix) {
            return Err(ChvError::BackendUnavailable { backend: "lvm".to_string(), reason: format!("handle {} does not belong to this backend", handle) });
        }
        let origin = self.volume_path(volume_id);
        let snap = format!("{}-snap-{}", volume_id, snapshot_name);
        let out = Command::new("lvcreate")
            .args(["-s", "-n", &snap, "-L", "1G", &origin.to_string_lossy()])
            .output()
            .await
            .map_err(|e| ChvError::Io { path: "lvcreate".to_string(), source: e })?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable { backend: "lvm".to_string(), reason: format!("lvcreate failed: {}", String::from_utf8_lossy(&out.stderr)) });
        }
        info!(volume_id, snapshot_name, "prepared LVM snapshot");
        Ok(())
    }

    async fn prepare_clone(&self, volume_id: &str, handle: &str, clone_name: &str) -> Result<(), ChvError> {
        let prefix = format!("lvm-{}-", self.vg_name);
        if !handle.starts_with(&prefix) {
            return Err(ChvError::BackendUnavailable { backend: "lvm".to_string(), reason: format!("handle {} does not belong to this backend", handle) });
        }
        let origin = self.volume_path(volume_id);
        let clone_lv = format!("{}-clone-{}", volume_id, clone_name);
        let out = Command::new("lvcreate")
            .args(["-s", "-n", &clone_lv, "-L", "1G", &origin.to_string_lossy()])
            .output()
            .await
            .map_err(|e| ChvError::Io { path: "lvcreate".to_string(), source: e })?;
        if !out.status.success() {
            return Err(ChvError::BackendUnavailable { backend: "lvm".to_string(), reason: format!("lvcreate failed: {}", String::from_utf8_lossy(&out.stderr)) });
        }
        info!(volume_id, clone_name, "prepared LVM clone");
        Ok(())
    }

    async fn set_device_policy(&self, volume_id: &str, _handle: &str, _policy: &DevicePolicy) -> Result<(), ChvError> {
        info!(volume_id, "device policy accepted but not enforced by LVMBackend");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lvm_backend_open_rejects_wrong_class() {
        let backend = LVMBackend::new("vg0".to_string());
        let locator = BackendLocator { backend_class: "local".to_string(), locator: "/dev/vg0/vol1".to_string(), options: Default::default() };
        let res = backend.open("vol-1", &locator, &DevicePolicy::default()).await;
        assert!(matches!(res, Err(ChvError::BackendUnavailable { .. })));
    }

    #[tokio::test]
    async fn lvm_backend_open_returns_lvm_path() {
        let backend = LVMBackend::new("vg0".to_string());
        let locator = BackendLocator { backend_class: "lvm".to_string(), locator: "vg0/vol1".to_string(), options: Default::default() };
        let export = backend.open("vol-1", &locator, &DevicePolicy::default()).await.unwrap();
        assert_eq!(export.export_kind, "lvm");
        assert!(export.export_path.contains("/dev/vg0/vol-1"));
    }
}
