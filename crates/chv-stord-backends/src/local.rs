use crate::r#trait::{BackendHealth, StorageBackend, VolumeExport};
use async_trait::async_trait;
use chv_common::types::{BackendLocator, DevicePolicy};
use chv_errors::ChvError;
use std::path::PathBuf;
use tracing::{info, warn};

const DEFAULT_SPARSE_SIZE_BYTES: u64 = 10 * 1024 * 1024 * 1024;

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

    fn resolve_optional_path(&self, path: &str) -> PathBuf {
        if std::path::Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.runtime_dir.join(path)
        }
    }

    fn parse_size_bytes(&self, locator: &BackendLocator) -> Result<u64, ChvError> {
        match locator.options.get("size_bytes") {
            Some(raw) => {
                let parsed = raw.parse::<u64>().map_err(|_| ChvError::InvalidArgument {
                    field: "size_bytes".to_string(),
                    reason: format!("invalid integer: {}", raw),
                })?;
                if parsed == 0 {
                    return Err(ChvError::InvalidArgument {
                        field: "size_bytes".to_string(),
                        reason: "size_bytes must be > 0".to_string(),
                    });
                }
                Ok(parsed)
            }
            None => Ok(DEFAULT_SPARSE_SIZE_BYTES),
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

    fn convert_qcow2_to_raw(path: &std::path::Path) -> Result<(), ChvError> {
        let raw_path = path.with_extension("img.raw");
        let status = std::process::Command::new("qemu-img")
            .args(["convert", "-f", "qcow2", "-O", "raw"])
            .arg(path)
            .arg(&raw_path)
            .status();
        match status {
            Ok(s) if s.success() => {
                std::fs::rename(&raw_path, path).map_err(|e| ChvError::BackendUnavailable {
                    backend: "local".to_string(),
                    reason: format!("failed to rename converted image: {}", e),
                })?;
                info!(path = %path.display(), "converted qcow2 seed image to raw");
                Ok(())
            }
            Ok(s) => {
                let _ = std::fs::remove_file(&raw_path);
                Err(ChvError::BackendUnavailable {
                    backend: "local".to_string(),
                    reason: format!("qemu-img convert failed with exit code {}", s),
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let _ = std::fs::remove_file(path);
                Err(ChvError::BackendUnavailable {
                    backend: "local".to_string(),
                    reason: "seed image is qcow2 but qemu-img is not installed; install qemu-utils or convert the image to raw".to_string(),
                })
            }
            Err(e) => {
                let _ = std::fs::remove_file(&raw_path);
                Err(ChvError::BackendUnavailable {
                    backend: "local".to_string(),
                    reason: format!("failed to run qemu-img: {}", e),
                })
            }
        }
    }

    async fn copy_volume(
        &self,
        volume_id: &str,
        handle: &str,
        dest_name: &str,
        op_label: &str,
        qcow2_reason: &str,
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
            return Err(ChvError::NotFound {
                resource: "path".to_string(),
                id: path.to_string_lossy().to_string(),
            });
        }

        let kind = Self::detect_kind(&path);
        if kind == "qcow2" {
            return Err(ChvError::InvalidArgument {
                field: "format".to_string(),
                reason: qcow2_reason.to_string(),
            });
        }

        let dest = self
            .runtime_dir
            .join(format!("{}-{}.img", volume_id, dest_name));
        tokio::fs::copy(&path, &dest)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "local".to_string(),
                reason: format!("failed to copy file for {}: {}", op_label, e),
            })?;

        info!(
            volume_id,
            handle,
            path = %path.display(),
            dest = %dest.display(),
            "prepared local {}", op_label
        );
        Ok(())
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
        if locator.backend_class != "local"
            && locator.backend_class != "local-file"
            && locator.backend_class != "localdisk"
        {
            return Err(ChvError::BackendUnavailable {
                backend: locator.backend_class.clone(),
                reason: "local backend only handles local class".to_string(),
            });
        }

        let path = self.resolve_path(locator);
        info!(volume_id, path = %path.display(), "opening local volume");

        if !path.exists() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| ChvError::BackendUnavailable {
                    backend: "local".to_string(),
                    reason: format!("failed to create parent directory: {}", e),
                })?;
            }

            let size_bytes = self.parse_size_bytes(locator)?;
            let seed_from = locator
                .options
                .get("seed_from")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty());

            match seed_from {
                Some(seed) => {
                    let seed_path = self.resolve_optional_path(seed);
                    if !seed_path.exists() {
                        return Err(ChvError::NotFound {
                            resource: "seed_source".to_string(),
                            id: seed_path.to_string_lossy().to_string(),
                        });
                    }
                    std::fs::copy(&seed_path, &path).map_err(|e| ChvError::BackendUnavailable {
                        backend: "local".to_string(),
                        reason: format!("failed to seed volume from image: {}", e),
                    })?;

                    if Self::detect_kind(&path) == "qcow2" {
                        info!(
                            volume_id,
                            path = %path.display(),
                            seed = %seed_path.display(),
                            "seed image is qcow2, converting to raw"
                        );
                        Self::convert_qcow2_to_raw(&path)?;
                    }

                    let file = std::fs::File::options()
                        .write(true)
                        .open(&path)
                        .map_err(|e| ChvError::BackendUnavailable {
                            backend: "local".to_string(),
                            reason: format!("failed to open seeded volume: {}", e),
                        })?;
                    if file.metadata().map(|m| m.len()).unwrap_or(0) < size_bytes {
                        file.set_len(size_bytes)
                            .map_err(|e| ChvError::BackendUnavailable {
                                backend: "local".to_string(),
                                reason: format!("failed to expand seeded volume: {}", e),
                            })?;
                    }
                    info!(
                        volume_id,
                        path = %path.display(),
                        seed = %seed_path.display(),
                        size_bytes,
                        "seeded local volume from image"
                    );
                }
                None => {
                    warn!(
                        volume_id,
                        path = %path.display(),
                        size_bytes,
                        "path does not exist yet; creating sparse raw volume"
                    );
                    let file =
                        std::fs::File::create(&path).map_err(|e| ChvError::BackendUnavailable {
                            backend: "local".to_string(),
                            reason: format!("failed to create volume file: {}", e),
                        })?;
                    file.set_len(size_bytes)
                        .map_err(|e| ChvError::BackendUnavailable {
                            backend: "local".to_string(),
                            reason: format!("failed to set volume file size: {}", e),
                        })?;
                }
            }
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
            return Err(ChvError::NotFound {
                resource: "path".to_string(),
                id: path.to_string_lossy().to_string(),
            });
        }

        let kind = Self::detect_kind(&path);
        if kind == "qcow2" {
            warn!(
                volume_id,
                handle,
                path = %path.display(),
                "qcow2 resize is not yet implemented"
            );
            return Err(ChvError::InvalidArgument {
                field: "new_size_bytes".to_string(),
                reason: "qcow2 resize not supported".to_string(),
            });
        }

        let file = std::fs::File::options()
            .write(true)
            .open(&path)
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "local".to_string(),
                reason: format!("failed to open file for resize: {}", e),
            })?;
        file.set_len(new_size_bytes)
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "local".to_string(),
                reason: format!("failed to resize file: {}", e),
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

    async fn prepare_snapshot(
        &self,
        volume_id: &str,
        handle: &str,
        snapshot_name: &str,
    ) -> Result<(), ChvError> {
        self.copy_volume(
            volume_id,
            handle,
            snapshot_name,
            "snapshot",
            "qcow2 snapshot not supported",
        )
        .await
    }

    async fn prepare_clone(
        &self,
        volume_id: &str,
        handle: &str,
        clone_name: &str,
    ) -> Result<(), ChvError> {
        self.copy_volume(
            volume_id,
            handle,
            clone_name,
            "clone",
            "qcow2 clone not supported",
        )
        .await
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
            handle, "device policy accepted but not enforced by LocalFileBackend"
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

        let export = backend
            .open("vol-1", &locator, &DevicePolicy::default())
            .await
            .unwrap();
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

        let e1 = backend
            .open("vol-1", &locator, &DevicePolicy::default())
            .await
            .unwrap();
        let e2 = backend
            .open("vol-1", &locator, &DevicePolicy::default())
            .await
            .unwrap();
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

        let export = backend
            .open("vol-1", &locator, &DevicePolicy::default())
            .await
            .unwrap();
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

        let res = backend
            .open("vol-1", &locator, &DevicePolicy::default())
            .await;
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

        let res = backend
            .detach("vol-1", "local-vol-1-vol.img", "vm-1", false)
            .await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn local_backend_detach_force_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());

        let res = backend
            .detach("vol-1", "local-vol-1-vol.img", "vm-1", true)
            .await;
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

    #[tokio::test]
    async fn local_backend_resize_missing_file_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());

        let res = backend.resize("vol-1", "local-vol-1-vol.img", 1024).await;
        assert!(matches!(res, Err(ChvError::NotFound { .. })));
    }

    #[tokio::test]
    async fn local_backend_resize_qcow2_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vol.qcow2");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(b"QFI\xfb").unwrap();
            f.write_all(&[0u8; 100]).unwrap();
        }

        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let res = backend.resize("vol-1", "local-vol-1-vol.qcow2", 1024).await;
        assert!(matches!(res, Err(ChvError::InvalidArgument { .. })));
    }

    #[tokio::test]
    async fn local_backend_prepare_snapshot_raw_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vol.img");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(&[0u8; 512]).unwrap();
        }

        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let handle = "local-vol-1-vol.img";
        backend
            .prepare_snapshot("vol-1", handle, "snap1")
            .await
            .unwrap();

        let dest = dir.path().join("vol-1-snap1.img");
        assert!(dest.exists());
        assert_eq!(std::fs::metadata(&dest).unwrap().len(), 512);
    }

    #[tokio::test]
    async fn local_backend_prepare_clone_raw_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vol.img");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(&[0u8; 512]).unwrap();
        }

        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let handle = "local-vol-1-vol.img";
        backend
            .prepare_clone("vol-1", handle, "clone1")
            .await
            .unwrap();

        let dest = dir.path().join("vol-1-clone1.img");
        assert!(dest.exists());
        assert_eq!(std::fs::metadata(&dest).unwrap().len(), 512);
    }

    #[tokio::test]
    async fn local_backend_prepare_snapshot_missing_file_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let res = backend
            .prepare_snapshot("vol-1", "local-vol-1-vol.img", "snap1")
            .await;
        assert!(matches!(res, Err(ChvError::NotFound { .. })));
    }

    #[tokio::test]
    async fn local_backend_prepare_snapshot_invalid_handle() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let res = backend
            .prepare_snapshot("vol-1", "iscsi-vol-1-target", "snap1")
            .await;
        assert!(matches!(res, Err(ChvError::BackendUnavailable { .. })));
    }

    #[tokio::test]
    async fn local_backend_prepare_clone_missing_file_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let res = backend
            .prepare_clone("vol-1", "local-vol-1-vol.img", "clone1")
            .await;
        assert!(matches!(res, Err(ChvError::NotFound { .. })));
    }

    #[tokio::test]
    async fn local_backend_prepare_clone_qcow2_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vol.qcow2");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(b"QFI\xfb").unwrap();
            f.write_all(&[0u8; 100]).unwrap();
        }

        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let res = backend
            .prepare_clone("vol-1", "local-vol-1-vol.qcow2", "clone1")
            .await;
        assert!(matches!(res, Err(ChvError::InvalidArgument { .. })));
    }

    #[tokio::test]
    async fn local_backend_open_with_seed_and_size_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let seed = dir.path().join("seed.img");
        {
            let mut f = std::fs::File::create(&seed).unwrap();
            f.write_all(&[1u8; 512]).unwrap();
        }

        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let mut options = std::collections::HashMap::new();
        options.insert("seed_from".to_string(), seed.to_string_lossy().to_string());
        options.insert("size_bytes".to_string(), "4096".to_string());
        let locator = BackendLocator {
            backend_class: "local".to_string(),
            locator: "seeded.img".to_string(),
            options,
        };

        let export = backend
            .open("vol-1", &locator, &DevicePolicy::default())
            .await
            .unwrap();
        assert!(export.export_path.ends_with("seeded.img"));
        let meta = std::fs::metadata(dir.path().join("seeded.img")).unwrap();
        assert_eq!(meta.len(), 4096);
    }

    #[tokio::test]
    async fn local_backend_rejects_invalid_size_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let mut options = std::collections::HashMap::new();
        options.insert("size_bytes".to_string(), "abc".to_string());
        let locator = BackendLocator {
            backend_class: "local".to_string(),
            locator: "bad-size.img".to_string(),
            options,
        };

        let err = backend
            .open("vol-1", &locator, &DevicePolicy::default())
            .await
            .unwrap_err();
        assert!(matches!(err, ChvError::InvalidArgument { .. }));
    }

    #[tokio::test]
    async fn local_backend_seed_qcow2_triggers_conversion() {
        let dir = tempfile::tempdir().unwrap();
        let seed = dir.path().join("seed-qcow2.img");
        {
            let mut f = std::fs::File::create(&seed).unwrap();
            // Write qcow2 magic header followed by enough data to be a valid-looking file
            let mut header = vec![0u8; 512];
            header[0..4].copy_from_slice(b"QFI\xfb");
            f.write_all(&header).unwrap();
        }

        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let mut options = std::collections::HashMap::new();
        options.insert("seed_from".to_string(), seed.to_string_lossy().to_string());
        options.insert("size_bytes".to_string(), "4096".to_string());
        let locator = BackendLocator {
            backend_class: "local".to_string(),
            locator: "qcow2-seeded.img".to_string(),
            options,
        };

        let result = backend
            .open("vol-1", &locator, &DevicePolicy::default())
            .await;

        if std::process::Command::new("qemu-img").arg("--version").status().is_ok() {
            let export = result.unwrap();
            assert_eq!(export.export_kind, "raw");
        } else {
            let err = result.unwrap_err();
            match err {
                ChvError::BackendUnavailable { reason, .. } => {
                    assert!(reason.contains("qemu-img"), "error should mention qemu-img: {}", reason);
                }
                other => panic!("expected BackendUnavailable, got {:?}", other),
            }
        }
    }
}
