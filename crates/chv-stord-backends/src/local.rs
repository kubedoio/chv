use crate::r#trait::{BackendHealth, StorageBackend, VolumeExport};
use async_trait::async_trait;
use chv_common::types::{BackendLocator, DevicePolicy};
use chv_errors::ChvError;
use std::path::PathBuf;
use tracing::{info, warn};

pub struct LocalFileBackend {
    runtime_dir: PathBuf,
}

impl LocalFileBackend {
    pub fn new(runtime_dir: PathBuf) -> Self {
        Self { runtime_dir }
    }

    fn resolve_path(&self, locator: &BackendLocator) -> PathBuf {
        if std::path::Path::new(&locator.locator).is_absolute() {
            PathBuf::from(&locator.locator)
        } else {
            self.runtime_dir.join(&locator.locator)
        }
    }

    fn detect_kind(path: &std::path::Path) -> String {
        if let Ok(mut f) = std::fs::File::open(path) {
            let mut buf = [0u8; 4];
            if std::io::Read::read_exact(&mut f, &mut buf).is_ok() {
                // QCOW magic: 'Q', 'F', 'I', 0xfb
                if &buf == b"QFI\xfb" {
                    return "qcow2".to_string();
                }
            }
        }
        "raw".to_string()
    }
}

#[async_trait]
impl StorageBackend for LocalFileBackend {
    async fn open(
        &self,
        volume_id: &str,
        locator: &BackendLocator,
        _policy: &DevicePolicy,
    ) -> Result<VolumeExport, ChvError> {
        if locator.backend_class != "local" && locator.backend_class != "local-file" {
            return Err(ChvError::BackendUnavailable {
                backend: locator.backend_class.clone(),
                reason: "local backend only handles local class".to_string(),
            });
        }

        let path = self.resolve_path(locator);
        info!(volume_id, path = %path.display(), "opening local volume");

        if !path.exists() {
            warn!(volume_id, path = %path.display(), "path does not exist yet");
        }

        let export_kind = Self::detect_kind(&path);
        let attachment_handle = format!("local-{}-{}", volume_id, locator.locator);

        Ok(VolumeExport {
            export_kind,
            export_path: path.to_string_lossy().to_string(),
            attachment_handle,
        })
    }

    async fn close(&self, volume_id: &str, handle: &str) -> Result<(), ChvError> {
        info!(volume_id, handle, "closing local volume");
        Ok(())
    }

    async fn attach(
        &self,
        volume_id: &str,
        handle: &str,
        vm_id: &str,
    ) -> Result<VolumeExport, ChvError> {
        let prefix = format!("local-{}-", volume_id);
        if !handle.starts_with(&prefix) {
            return Err(ChvError::BackendUnavailable {
                backend: "local".to_string(),
                reason: format!("handle {} does not belong to this backend", handle),
            });
        }

        let locator_str = handle.strip_prefix(&prefix).unwrap_or(handle);
        let path = std::path::Path::new(locator_str);
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.runtime_dir.join(path)
        };

        info!(volume_id, vm_id, handle, path = %path.display(), "attaching local volume");

        if !path.exists() {
            warn!(volume_id, vm_id, handle, path = %path.display(), "path does not exist");
        }

        let export_kind = Self::detect_kind(&path);

        Ok(VolumeExport {
            export_kind,
            export_path: path.to_string_lossy().to_string(),
            attachment_handle: handle.to_string(),
        })
    }

    async fn detach(
        &self,
        volume_id: &str,
        handle: &str,
        vm_id: &str,
        force: bool,
    ) -> Result<(), ChvError> {
        let prefix = format!("local-{}-", volume_id);
        if !handle.starts_with(&prefix) {
            return Err(ChvError::BackendUnavailable {
                backend: "local".to_string(),
                reason: format!("handle {} does not belong to this backend", handle),
            });
        }

        if force {
            warn!(volume_id, vm_id, handle, "force detaching local volume");
        } else {
            info!(volume_id, vm_id, handle, "detaching local volume");
        }

        Ok(())
    }

    async fn health(&self, volume_id: &str, handle: &str) -> Result<BackendHealth, ChvError> {
        // Derive expected path from handle: local-{volume_id}-{locator}
        let prefix = format!("local-{}-", volume_id);
        let path_str = if handle.starts_with(&prefix) {
            handle.strip_prefix(&prefix).unwrap_or(handle)
        } else {
            handle
        };
        let path = std::path::Path::new(path_str);
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.runtime_dir.join(path)
        };

        let status = if path.exists() {
            "healthy"
        } else {
            "unhealthy"
        };
        let last_error = if path.exists() {
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
        let prefix = format!("local-{}-", volume_id);
        if !handle.starts_with(&prefix) {
            return Err(ChvError::BackendUnavailable {
                backend: "local".to_string(),
                reason: format!("handle {} does not belong to this backend", handle),
            });
        }

        let locator_str = handle.strip_prefix(&prefix).unwrap_or(handle);
        let path = std::path::Path::new(locator_str);
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.runtime_dir.join(path)
        };

        if !path.exists() {
            warn!(
                volume_id,
                handle,
                path = %path.display(),
                "resize called but path does not exist"
            );
            return Ok(());
        }

        let kind = Self::detect_kind(&path);
        if kind == "qcow2" {
            warn!(
                volume_id,
                handle,
                path = %path.display(),
                "qcow2 resize is not yet implemented"
            );
            return Ok(());
        }

        let file = std::fs::File::options().write(true).open(&path).map_err(|e| {
            ChvError::BackendUnavailable {
                backend: "local".to_string(),
                reason: format!("failed to open file for resize: {}", e),
            }
        })?;
        file.set_len(new_size_bytes).map_err(|e| {
            ChvError::BackendUnavailable {
                backend: "local".to_string(),
                reason: format!("failed to resize file: {}", e),
            }
        })?;

        info!(
            volume_id,
            handle,
            path = %path.display(),
            new_size_bytes,
            "resized local volume"
        );
        Ok(())
    }

    async fn set_device_policy(
        &self,
        volume_id: &str,
        handle: &str,
        _policy: &DevicePolicy,
    ) -> Result<(), ChvError> {
        let prefix = format!("local-{}-", volume_id);
        if !handle.starts_with(&prefix) {
            return Err(ChvError::BackendUnavailable {
                backend: "local".to_string(),
                reason: format!("handle {} does not belong to this backend", handle),
            });
        }

        info!(
            volume_id,
            handle,
            "device policy accepted but not enforced by LocalFileBackend"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[tokio::test]
    async fn local_backend_open_resolves_path() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let locator = BackendLocator {
            backend_class: "local".to_string(),
            locator: "test.img".to_string(),
            options: Default::default(),
        };

        let export = backend.open("vol-1", &locator, &DevicePolicy::default()).await.unwrap();
        assert_eq!(export.export_kind, "raw");
        assert!(export.export_path.ends_with("test.img"));
    }

    #[tokio::test]
    async fn local_backend_idempotent_open() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let locator = BackendLocator {
            backend_class: "local".to_string(),
            locator: "vol.img".to_string(),
            options: Default::default(),
        };

        let e1 = backend.open("vol-1", &locator, &DevicePolicy::default()).await.unwrap();
        let e2 = backend.open("vol-1", &locator, &DevicePolicy::default()).await.unwrap();
        assert_eq!(e1.attachment_handle, e2.attachment_handle);
    }

    #[tokio::test]
    async fn local_backend_qcow2_detection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("disk.qcow2");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(b"QFI\xfb").unwrap();
            f.write_all(&[0u8; 100]).unwrap();
        }

        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let locator = BackendLocator {
            backend_class: "local".to_string(),
            locator: path.to_string_lossy().to_string(),
            options: Default::default(),
        };

        let export = backend.open("vol-1", &locator, &DevicePolicy::default()).await.unwrap();
        assert_eq!(export.export_kind, "qcow2");
    }

    #[tokio::test]
    async fn local_backend_rejects_wrong_class() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let locator = BackendLocator {
            backend_class: "iscsi".to_string(),
            locator: "tgt".to_string(),
            options: Default::default(),
        };

        let res = backend.open("vol-1", &locator, &DevicePolicy::default()).await;
        assert!(matches!(res, Err(ChvError::BackendUnavailable { .. })));
    }

    #[tokio::test]
    async fn local_backend_attach_succeeds_with_valid_handle() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vol.img");
        std::fs::File::create(&path).unwrap();

        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let handle = "local-vol-1-vol.img";
        let export = backend.attach("vol-1", handle, "vm-1").await.unwrap();

        assert_eq!(export.export_kind, "raw");
        assert_eq!(export.attachment_handle, handle);
        assert!(export.export_path.ends_with("vol.img"));
    }

    #[tokio::test]
    async fn local_backend_attach_fails_with_invalid_handle() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());

        let res = backend.attach("vol-1", "iscsi-vol-1-target", "vm-1").await;
        assert!(matches!(res, Err(ChvError::BackendUnavailable { .. })));
    }

    #[tokio::test]
    async fn local_backend_detach_succeeds_with_valid_handle() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());

        let res = backend.detach("vol-1", "local-vol-1-vol.img", "vm-1", false).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn local_backend_detach_force_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());

        let res = backend.detach("vol-1", "local-vol-1-vol.img", "vm-1", true).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn local_backend_resize_raw_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vol.img");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(&[0u8; 512]).unwrap();
        }

        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let handle = "local-vol-1-vol.img";
        backend.resize("vol-1", handle, 1024).await.unwrap();

        let meta = std::fs::metadata(&path).unwrap();
        assert_eq!(meta.len(), 1024);
    }

    #[tokio::test]
    async fn local_backend_set_device_policy_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());

        let res = backend
            .set_device_policy("vol-1", "local-vol-1-vol.img", &DevicePolicy::default())
            .await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn local_backend_resize_rejects_invalid_handle() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());

        let res = backend.resize("vol-1", "iscsi-vol-1-target", 1024).await;
        assert!(matches!(res, Err(ChvError::BackendUnavailable { .. })));
    }
}
