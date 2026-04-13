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
}
