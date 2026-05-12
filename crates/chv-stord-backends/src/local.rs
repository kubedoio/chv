use crate::r#trait::{BackendHealth, StorageBackend, VolumeExport};
use async_trait::async_trait;
use chv_common::types::{BackendLocator, DevicePolicy};
use chv_errors::ChvError;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

const DEFAULT_SPARSE_SIZE_BYTES: u64 = 10 * 1024 * 1024 * 1024;

struct DirtyTracker {
    block_size: u64,
    bitmap: Vec<u8>,
}

pub struct LocalFileBackend {
    runtime_dir: PathBuf,
    dirty_trackers: Arc<RwLock<HashMap<String, DirtyTracker>>>,
}

impl LocalFileBackend {
    pub fn new(runtime_dir: PathBuf) -> Self {
        Self {
            runtime_dir,
            dirty_trackers: Arc::new(RwLock::new(HashMap::new())),
        }
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
                std::fs::rename(&raw_path, path).map_err(|e| {
                    let _ = std::fs::remove_file(&raw_path);
                    ChvError::BackendUnavailable {
                        backend: "local".to_string(),
                        reason: format!("failed to rename converted image: {}", e),
                    }
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

    /// Resolve the filesystem path for a volume given its handle.
    /// Handle format: `"local-{volume_id}-{locator}"` where locator is a
    /// relative or absolute path.
    fn path_from_handle(&self, volume_id: &str, handle: &str) -> Result<PathBuf, ChvError> {
        let prefix = format!("local-{}-", volume_id);
        if !handle.starts_with(&prefix) {
            return Err(ChvError::BackendUnavailable {
                backend: "local".to_string(),
                reason: format!("handle {} does not belong to this backend", handle),
            });
        }
        let locator_str = handle.strip_prefix(&prefix).unwrap_or(handle);
        let path = std::path::Path::new(locator_str);
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            Ok(self.runtime_dir.join(path))
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

        let path_clone = path.clone();
        let qcow2_reason_owned = qcow2_reason.to_string();
        let (exists, kind) = tokio::task::spawn_blocking(move || {
            let exists = path_clone.exists();
            let kind = if exists {
                LocalFileBackend::detect_kind(&path_clone)
            } else {
                String::new()
            };
            (exists, kind)
        })
        .await
        .map_err(|e| ChvError::BackendUnavailable {
            backend: "local".to_string(),
            reason: format!("spawn_blocking join error: {e}"),
        })?;

        if !exists {
            return Err(ChvError::NotFound {
                resource: "path".to_string(),
                id: path.to_string_lossy().to_string(),
            });
        }

        if kind == "qcow2" {
            return Err(ChvError::InvalidArgument {
                field: "format".to_string(),
                reason: qcow2_reason_owned,
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

        let path_exists_check = path.clone();
        let path_exists = tokio::task::spawn_blocking(move || path_exists_check.exists())
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "local".to_string(),
                reason: format!("spawn_blocking join error: {e}"),
            })?;

        if !path_exists {
            let size_bytes = self.parse_size_bytes(locator)?;
            let seed_from = locator
                .options
                .get("seed_from")
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            let path_clone = path.clone();
            let volume_id_owned = volume_id.to_string();

            // Resolve seed path outside spawn_blocking (needs &self)
            let seed_path = seed_from
                .as_ref()
                .map(|seed| self.resolve_optional_path(seed));

            tokio::task::spawn_blocking(move || {
                if let Some(parent) = path_clone.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| ChvError::BackendUnavailable {
                        backend: "local".to_string(),
                        reason: format!("failed to create parent directory: {}", e),
                    })?;
                }

                match seed_path {
                    Some(seed_path) => {
                        if !seed_path.exists() {
                            return Err(ChvError::NotFound {
                                resource: "seed_source".to_string(),
                                id: seed_path.to_string_lossy().to_string(),
                            });
                        }
                        std::fs::copy(&seed_path, &path_clone).map_err(|e| {
                            ChvError::BackendUnavailable {
                                backend: "local".to_string(),
                                reason: format!("failed to seed volume from image: {}", e),
                            }
                        })?;

                        if LocalFileBackend::detect_kind(&path_clone) == "qcow2" {
                            info!(
                                volume_id = %volume_id_owned,
                                path = %path_clone.display(),
                                seed = %seed_path.display(),
                                "seed image is qcow2, converting to raw"
                            );
                            LocalFileBackend::convert_qcow2_to_raw(&path_clone)?;
                        }

                        let file = std::fs::File::options()
                            .write(true)
                            .open(&path_clone)
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
                            volume_id = %volume_id_owned,
                            path = %path_clone.display(),
                            seed = %seed_path.display(),
                            size_bytes,
                            "seeded local volume from image"
                        );
                    }
                    None => {
                        warn!(
                            volume_id = %volume_id_owned,
                            path = %path_clone.display(),
                            size_bytes,
                            "path does not exist yet; creating sparse raw volume"
                        );
                        let file = std::fs::File::create(&path_clone).map_err(|e| {
                            ChvError::BackendUnavailable {
                                backend: "local".to_string(),
                                reason: format!("failed to create volume file: {}", e),
                            }
                        })?;
                        file.set_len(size_bytes)
                            .map_err(|e| ChvError::BackendUnavailable {
                                backend: "local".to_string(),
                                reason: format!("failed to set volume file size: {}", e),
                            })?;
                    }
                }
                Ok::<(), ChvError>(())
            })
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "local".to_string(),
                reason: format!("spawn_blocking join error: {e}"),
            })??;
        }

        let path_clone = path.clone();
        let export_kind =
            tokio::task::spawn_blocking(move || LocalFileBackend::detect_kind(&path_clone))
                .await
                .map_err(|e| ChvError::BackendUnavailable {
                    backend: "local".to_string(),
                    reason: format!("spawn_blocking join error: {e}"),
                })?;
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

        let path_clone = path.clone();
        let (exists, export_kind) = tokio::task::spawn_blocking(move || {
            let exists = path_clone.exists();
            let kind = LocalFileBackend::detect_kind(&path_clone);
            (exists, kind)
        })
        .await
        .map_err(|e| ChvError::BackendUnavailable {
            backend: "local".to_string(),
            reason: format!("spawn_blocking join error: {e}"),
        })?;

        if !exists {
            warn!(volume_id, vm_id, handle, path = %path.display(), "path does not exist");
        }

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

        let path_clone = path.clone();
        let exists = tokio::task::spawn_blocking(move || path_clone.exists())
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "local".to_string(),
                reason: format!("spawn_blocking join error: {e}"),
            })?;

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

        let path_clone = path.clone();
        let volume_id_owned = volume_id.to_string();
        let handle_owned = handle.to_string();
        tokio::task::spawn_blocking(move || {
            if !path_clone.exists() {
                warn!(
                    volume_id = %volume_id_owned,
                    handle = %handle_owned,
                    path = %path_clone.display(),
                    "resize called but path does not exist"
                );
                return Err(ChvError::NotFound {
                    resource: "path".to_string(),
                    id: path_clone.to_string_lossy().to_string(),
                });
            }

            let kind = LocalFileBackend::detect_kind(&path_clone);
            if kind == "qcow2" {
                let status = std::process::Command::new("qemu-img")
                    .args(["resize", "-f", "qcow2"])
                    .arg(&path_clone)
                    .arg(format!("{}", new_size_bytes))
                    .status();
                match status {
                    Ok(s) if s.success() => {
                        info!(
                            volume_id = %volume_id_owned,
                            handle = %handle_owned,
                            path = %path_clone.display(),
                            new_size_bytes,
                            "resized qcow2 volume"
                        );
                        return Ok(());
                    }
                    Ok(s) => {
                        return Err(ChvError::BackendUnavailable {
                            backend: "local".to_string(),
                            reason: format!("qemu-img resize failed with exit code {}", s),
                        });
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        return Err(ChvError::BackendUnavailable {
                            backend: "local".to_string(),
                            reason:
                                "qemu-img is not installed; install qemu-utils to resize qcow2 volumes"
                                    .to_string(),
                        });
                    }
                    Err(e) => {
                        return Err(ChvError::BackendUnavailable {
                            backend: "local".to_string(),
                            reason: format!("failed to run qemu-img: {}", e),
                        });
                    }
                }
            }

            let file = std::fs::File::options()
                .write(true)
                .open(&path_clone)
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
                volume_id = %volume_id_owned,
                handle = %handle_owned,
                path = %path_clone.display(),
                new_size_bytes,
                "resized local volume"
            );
            Ok(())
        })
        .await
        .map_err(|e| ChvError::BackendUnavailable {
            backend: "local".to_string(),
            reason: format!("spawn_blocking join error: {e}"),
        })?
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

    async fn restore_snapshot(
        &self,
        volume_id: &str,
        handle: &str,
        snapshot_name: &str,
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

        let snap = self
            .runtime_dir
            .join(format!("{}-{}.img", volume_id, snapshot_name));

        let snap_clone = snap.clone();
        let snap_exists = tokio::task::spawn_blocking(move || snap_clone.exists())
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "local".to_string(),
                reason: format!("spawn_blocking join error: {e}"),
            })?;
        if !snap_exists {
            return Err(ChvError::NotFound {
                resource: "snapshot".to_string(),
                id: snap.to_string_lossy().to_string(),
            });
        }

        // Restore to a temp file first, then atomic rename to avoid
        // corrupting the live volume if the copy fails mid-write.
        let restore_tmp = path.with_extension("img.restore-tmp");
        tokio::fs::copy(&snap, &restore_tmp)
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "local".to_string(),
                reason: format!("failed to copy snapshot to temp file: {}", e),
            })?;

        tokio::fs::rename(&restore_tmp, &path).await.map_err(|e| {
            // Best-effort cleanup of temp file on rename failure.
            // This runs in the async context but is only the error path.
            let _ = std::fs::remove_file(&restore_tmp);
            ChvError::BackendUnavailable {
                backend: "local".to_string(),
                reason: format!("failed to rename restored snapshot into place: {}", e),
            }
        })?;

        info!(
            volume_id,
            snapshot_name,
            path = %path.display(),
            "restored local snapshot"
        );
        Ok(())
    }

    async fn delete_snapshot(
        &self,
        volume_id: &str,
        handle: &str,
        snapshot_name: &str,
    ) -> Result<(), ChvError> {
        let prefix = format!("local-{}-", volume_id);
        if !handle.starts_with(&prefix) {
            return Err(ChvError::BackendUnavailable {
                backend: "local".to_string(),
                reason: format!("handle {} does not belong to this backend", handle),
            });
        }

        let snap = self
            .runtime_dir
            .join(format!("{}-{}.img", volume_id, snapshot_name));

        let snap_clone = snap.clone();
        let snap_exists = tokio::task::spawn_blocking(move || snap_clone.exists())
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "local".to_string(),
                reason: format!("spawn_blocking join error: {e}"),
            })?;
        if snap_exists {
            tokio::fs::remove_file(&snap)
                .await
                .map_err(|e| ChvError::BackendUnavailable {
                    backend: "local".to_string(),
                    reason: format!("failed to delete snapshot: {}", e),
                })?;
        }

        info!(
            volume_id,
            snapshot_name,
            path = %snap.display(),
            "deleted local snapshot"
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
            handle, "device policy accepted but not enforced by LocalFileBackend"
        );
        Ok(())
    }

    // --- Phase 2.1-2.2: Migration methods ---

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
        let path = self.path_from_handle(volume_id, handle)?;
        let path_clone = path.clone();
        let file_len = tokio::task::spawn_blocking(move || {
            std::fs::metadata(&path_clone).map(|m| m.len()).unwrap_or(0)
        })
        .await
        .map_err(|e| ChvError::BackendUnavailable {
            backend: "local".to_string(),
            reason: format!("spawn_blocking join error: {e}"),
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
            handle, block_size, bitmap_bytes, "enabled dirty tracking"
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
                info!(volume_id, handle, "cleared dirty bitmap");
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
        info!(volume_id, handle, "disabled dirty tracking");
        Ok(())
    }

    /// Atomically snapshot and clear the dirty bitmap under a single write lock.
    ///
    /// This prevents any window where a write could dirty a block between
    /// get_dirty_bitmap and clear_dirty_bitmap, which would lose that dirty
    /// information during migration sync rounds.
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
        let path = self.path_from_handle(volume_id, handle)?;
        tokio::task::spawn_blocking(move || {
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
            backend: "local".to_string(),
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
        let path = self.path_from_handle(volume_id, handle)?;
        let data_owned = data.to_vec();
        tokio::task::spawn_blocking(move || {
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
            backend: "local".to_string(),
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

    async fn volume_size(&self, volume_id: &str, handle: &str) -> Result<u64, ChvError> {
        let path = self.path_from_handle(volume_id, handle)?;
        let path_clone = path.clone();
        tokio::task::spawn_blocking(move || {
            std::fs::metadata(&path_clone)
                .map(|m| m.len())
                .map_err(|e| ChvError::Io {
                    path: path_clone.display().to_string(),
                    source: e,
                })
        })
        .await
        .map_err(|e| ChvError::BackendUnavailable {
            backend: "local".to_string(),
            reason: format!("spawn_blocking join error: {e}"),
        })?
    }

    async fn create_receiving_volume(
        &self,
        volume_id: &str,
        size_bytes: u64,
        format: &str,
    ) -> Result<VolumeExport, ChvError> {
        if size_bytes == 0 {
            return Err(ChvError::InvalidArgument {
                field: "size_bytes".to_string(),
                reason: "size_bytes must be > 0".to_string(),
            });
        }
        let filename = format!("{}.img", volume_id);
        let dest = self.runtime_dir.join(&filename);
        let dest_clone = dest.clone();
        tokio::task::spawn_blocking(move || {
            let file = std::fs::File::create(&dest_clone).map_err(|e| ChvError::Io {
                path: dest_clone.display().to_string(),
                source: e,
            })?;
            file.set_len(size_bytes).map_err(|e| ChvError::Io {
                path: dest_clone.display().to_string(),
                source: e,
            })?;
            Ok::<(), ChvError>(())
        })
        .await
        .map_err(|e| ChvError::BackendUnavailable {
            backend: "local".to_string(),
            reason: format!("spawn_blocking join error: {e}"),
        })??;
        let handle = format!("local-{}-{}", volume_id, filename);
        info!(volume_id, size_bytes, format, path = %dest.display(), "created receiving volume");
        Ok(VolumeExport {
            export_kind: format.to_string(),
            export_path: dest.to_string_lossy().to_string(),
            attachment_handle: handle,
        })
    }

    async fn delete_volume(&self, volume_id: &str) -> Result<(), ChvError> {
        let primary = self.runtime_dir.join(format!("{}.img", volume_id));
        let primary_clone = primary.clone();
        let primary_exists = tokio::task::spawn_blocking(move || primary_clone.exists())
            .await
            .map_err(|e| ChvError::BackendUnavailable {
                backend: "local".to_string(),
                reason: format!("spawn_blocking join error: {e}"),
            })?;
        if primary_exists {
            tokio::fs::remove_file(&primary)
                .await
                .map_err(|e| ChvError::Io {
                    path: primary.display().to_string(),
                    source: e,
                })?;
            info!(volume_id, path = %primary.display(), "deleted primary volume file");
        }

        // Remove related snapshot/clone files matching `{volume_id}-*.img`.
        let pattern = format!("{}-", volume_id);
        if let Ok(mut entries) = tokio::fs::read_dir(&self.runtime_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with(&pattern) && name_str.ends_with(".img") {
                    let p = entry.path();
                    if let Err(e) = tokio::fs::remove_file(&p).await {
                        warn!(volume_id, path = %p.display(), err = %e, "failed to remove snapshot file");
                    } else {
                        info!(volume_id, path = %p.display(), "deleted snapshot/clone file");
                    }
                }
            }
        }

        Ok(())
    }

    async fn set_io_limits(
        &self,
        volume_id: &str,
        iops: Option<u64>,
        bandwidth_mbps: Option<u64>,
    ) -> Result<(), ChvError> {
        if let Some(iops_val) = iops {
            tracing::info!(
                volume_id = %volume_id,
                iops = iops_val,
                "IOPS limit configured (enforcement requires cgroup v2)"
            );
        }
        if let Some(bw) = bandwidth_mbps {
            tracing::info!(
                volume_id = %volume_id,
                bandwidth_mbps = bw,
                "bandwidth limit configured (enforcement requires cgroup v2)"
            );
        }
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
    async fn local_backend_resize_malformed_qcow2_returns_backend_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vol.qcow2");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(b"QFI\xfb").unwrap();
            f.write_all(&[0u8; 100]).unwrap();
        }

        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let res = backend.resize("vol-1", "local-vol-1-vol.qcow2", 1024).await;
        assert!(matches!(res, Err(ChvError::BackendUnavailable { .. })));
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

        // Create a valid minimal qcow2 file so qemu-img convert succeeds.
        let qemu_img_ok = std::process::Command::new("qemu-img")
            .args(["create", "-f", "qcow2"])
            .arg(&seed)
            .arg("4M")
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if !qemu_img_ok {
            // Write qcow2 magic so detect_kind sees it as qcow2; qemu-img
            // is missing so convert_qcow2_to_raw will return BackendUnavailable.
            std::fs::write(&seed, b"QFI\xfb").unwrap();
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

        if qemu_img_ok {
            let export = result.unwrap();
            assert_eq!(export.export_kind, "raw");
        } else {
            let err = result.unwrap_err();
            match err {
                ChvError::BackendUnavailable { reason, .. } => {
                    assert!(
                        reason.contains("qemu-img"),
                        "error should mention qemu-img: {}",
                        reason
                    );
                }
                other => panic!("expected BackendUnavailable, got {:?}", other),
            }
        }
    }

    // --- Phase 2.1-2.2 unit tests ---

    #[tokio::test]
    async fn dirty_tracking_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        // Create a 4096-byte volume file
        let path = dir.path().join("vol.img");
        {
            let f = std::fs::File::create(&path).unwrap();
            f.set_len(4096).unwrap();
        }

        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let handle = "local-vol-dt-vol.img";

        // Enable tracking with 512-byte blocks (8 blocks total)
        backend
            .enable_dirty_tracking("vol-dt", handle, 512)
            .await
            .unwrap();

        // Bitmap should be all zeros (1 byte covers 8 blocks)
        let bitmap = backend.get_dirty_bitmap("vol-dt", handle).await.unwrap();
        assert_eq!(bitmap.len(), 1);
        assert_eq!(bitmap[0], 0x00);

        // Mark a bit manually by setting it via clear (which should keep zeros)
        backend.clear_dirty_bitmap("vol-dt", handle).await.unwrap();
        let bitmap = backend.get_dirty_bitmap("vol-dt", handle).await.unwrap();
        assert_eq!(bitmap[0], 0x00);

        // Disable tracking — subsequent get should return NotFound
        backend
            .disable_dirty_tracking("vol-dt", handle)
            .await
            .unwrap();
        let res = backend.get_dirty_bitmap("vol-dt", handle).await;
        assert!(matches!(res, Err(ChvError::NotFound { .. })));
    }

    #[tokio::test]
    async fn read_write_block_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vol.img");
        {
            let f = std::fs::File::create(&path).unwrap();
            f.set_len(4096).unwrap();
        }

        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let handle = "local-vol-rw-vol.img";
        let data = b"hello world padded to fill";

        // Write at offset 512
        backend
            .write_block("vol-rw", handle, 512, data)
            .await
            .unwrap();

        // Read back
        let got = backend
            .read_block("vol-rw", handle, 512, data.len() as u64)
            .await
            .unwrap();
        assert_eq!(got, data);
    }

    #[tokio::test]
    async fn write_block_marks_dirty_bitmap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vol.img");
        {
            let f = std::fs::File::create(&path).unwrap();
            f.set_len(4096).unwrap();
        }

        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let handle = "local-vol-db-vol.img";

        // Enable with 512-byte blocks
        backend
            .enable_dirty_tracking("vol-db", handle, 512)
            .await
            .unwrap();

        // Write to block 0 (offset 0, 1 byte)
        backend
            .write_block("vol-db", handle, 0, b"x")
            .await
            .unwrap();

        let bitmap = backend.get_dirty_bitmap("vol-db", handle).await.unwrap();
        // Block 0 = bit 0 of byte 0
        assert_eq!(bitmap[0] & 0x01, 0x01, "block 0 should be marked dirty");

        // Write to block 2 (offset 1024, 1 byte)
        backend
            .write_block("vol-db", handle, 1024, b"y")
            .await
            .unwrap();

        let bitmap = backend.get_dirty_bitmap("vol-db", handle).await.unwrap();
        // Block 2 = bit 2 of byte 0
        assert_eq!(bitmap[0] & 0x04, 0x04, "block 2 should be marked dirty");

        // Clear bitmap and verify it resets
        backend.clear_dirty_bitmap("vol-db", handle).await.unwrap();
        let bitmap = backend.get_dirty_bitmap("vol-db", handle).await.unwrap();
        assert_eq!(bitmap[0], 0x00, "bitmap should be zeroed after clear");
    }

    #[tokio::test]
    async fn create_and_delete_receiving_volume() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());

        let export = backend
            .create_receiving_volume("rcv-vol-1", 8192, "raw")
            .await
            .unwrap();

        assert_eq!(export.export_kind, "raw");
        assert!(export.export_path.ends_with("rcv-vol-1.img"));
        assert_eq!(export.attachment_handle, "local-rcv-vol-1-rcv-vol-1.img");

        let meta = std::fs::metadata(&export.export_path).unwrap();
        assert_eq!(meta.len(), 8192);

        // Delete should remove the file
        backend.delete_volume("rcv-vol-1").await.unwrap();
        assert!(
            !std::path::Path::new(&export.export_path).exists(),
            "primary volume file should be deleted"
        );
    }

    #[tokio::test]
    async fn delete_volume_also_removes_snapshots() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());

        // Create primary
        backend
            .create_receiving_volume("snap-vol", 512, "raw")
            .await
            .unwrap();

        // Create a fake snapshot file matching the pattern `{volume_id}-*.img`
        let snap_path = dir.path().join("snap-vol-snap1.img");
        std::fs::File::create(&snap_path).unwrap();

        backend.delete_volume("snap-vol").await.unwrap();

        assert!(!dir.path().join("snap-vol.img").exists());
        assert!(!snap_path.exists(), "snapshot file should be deleted");
    }

    #[tokio::test]
    async fn volume_size_returns_correct_value() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sized.img");
        {
            let f = std::fs::File::create(&path).unwrap();
            f.set_len(16384).unwrap();
        }

        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let handle = "local-sz-vol-sized.img";
        let size = backend.volume_size("sz-vol", handle).await.unwrap();
        assert_eq!(size, 16384);
    }
}
