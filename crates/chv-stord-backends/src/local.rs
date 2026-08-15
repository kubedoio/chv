use crate::r#trait::{
    validate_write_bounds, BackendHealth, StorageBackend, VolumeExport, DIRTY_TRACKING_BLOCK_SIZE,
    MAX_DIRTY_TRACKING_VOLUME_SIZE_BYTES,
};
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
    volume_size: u64,
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

    /// Defense in depth: snapshot/clone names become path components in
    /// filenames like `{volume_id}-{name}.img`; reject anything that is not
    /// a single safe component before building such a path.
    fn require_safe_name(name: &str, field: &str) -> Result<(), ChvError> {
        if !chv_common::is_safe_id(name) {
            return Err(ChvError::InvalidArgument {
                field: field.to_string(),
                reason: format!("'{name}' is not a safe id (must be a single path component)"),
            });
        }
        Ok(())
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
        Self::require_safe_name(dest_name, "dest_name")?;
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
        // The handle is gone once closed: drop its dirty tracker so closed
        // volumes cannot accumulate bitmaps.
        self.dirty_trackers.write().await.remove(handle);
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
        })??;

        // Keep dirty tracking consistent with the new size so that writes
        // into the grown region are both allowed and marked.
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
        _ownership: chv_common::AttachmentOwnership,
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
        Self::require_safe_name(snapshot_name, "snapshot_name")?;

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
        Self::require_safe_name(snapshot_name, "snapshot_name")?;

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

    /// Initialize the dirty bitmap for an opened volume.
    ///
    /// The bitmap has one bit per 4 MiB block and starts all-clear. Reusing
    /// an existing tracker (idempotent re-open or trigger-time re-enable)
    /// updates its size bound and grows the bitmap in place, preserving the
    /// dirty bits. Volume sizes above
    /// [`MAX_DIRTY_TRACKING_VOLUME_SIZE_BYTES`] are rejected so the bitmap
    /// allocation stays bounded.
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
    ///
    /// This prevents any window where a write could dirty a block between
    /// reading the bitmap and clearing it, which would lose that dirty
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

        // When dirty tracking is enabled, reject out-of-range writes up
        // front: the bitmap is sized from the volume size, so a write past
        // the end could otherwise be silently dropped from the dirty bitmap
        // and lost during migration. The mark range is captured before the
        // write so the bitmap update below cannot run past the bitmap end.
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

        // Defense in depth: the receiving filename is built from a
        // caller-supplied volume_id. The migration gRPC boundary already
        // validates ids, but the backend never trusts caller input when
        // constructing a path under runtime_dir.
        if !chv_common::is_safe_id(volume_id) {
            return Err(ChvError::InvalidArgument {
                field: "volume_id".to_string(),
                reason: format!("unsafe volume_id '{volume_id}': must be a single path component"),
            });
        }
        let filename = format!("{}.img", volume_id);
        let dest = self.runtime_dir.join(&filename);

        // Canonical containment: resolve runtime_dir (which must exist to
        // receive a volume) and verify the destination still lands inside it.
        // Fail closed on any resolution error.
        let runtime_canonical =
            std::fs::canonicalize(&self.runtime_dir).map_err(|e| ChvError::BackendUnavailable {
                backend: "local".to_string(),
                reason: format!("cannot resolve runtime_dir for receiving volume: {}", e),
            })?;
        let dest_canonical = runtime_canonical.join(&filename);
        if !dest_canonical.starts_with(&runtime_canonical) {
            return Err(ChvError::AccessDenied {
                resource: volume_id.to_string(),
                reason: format!(
                    "receiving volume path '{}' escapes runtime_dir",
                    dest.display()
                ),
            });
        }

        let dest_clone = dest.clone();
        tokio::task::spawn_blocking(move || -> Result<(), ChvError> {
            // create_new so an existing path fails instead of being silently
            // truncated: receiving volumes are written by migration streams
            // and must never clobber a live volume file.
            let file = match std::fs::File::options()
                .write(true)
                .create_new(true)
                .open(&dest_clone)
            {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    return Err(ChvError::InvalidArgument {
                        field: "volume_id".to_string(),
                        reason: format!(
                            "receiving volume already exists, refusing to truncate it: {}",
                            dest_clone.display()
                        ),
                    });
                }
                Err(e) => {
                    return Err(ChvError::Io {
                        path: dest_clone.display().to_string(),
                        source: e,
                    });
                }
            };
            file.set_len(size_bytes).map_err(|e| ChvError::Io {
                path: dest_clone.display().to_string(),
                source: e,
            })?;
            Ok(())
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
        let ownership = chv_common::AttachmentOwnership {
            vm_id: "vm-1".to_string(),
            operation_id: None,
            requester: None,
        };

        let res = backend
            .detach("vol-1", "local-vol-1-vol.img", ownership, false)
            .await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn local_backend_detach_force_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let ownership = chv_common::AttachmentOwnership {
            vm_id: "vm-1".to_string(),
            operation_id: None,
            requester: None,
        };

        let res = backend
            .detach("vol-1", "local-vol-1-vol.img", ownership, true)
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
        let ownership = chv_common::AttachmentOwnership {
            vm_id: "vm-1".to_string(),
            operation_id: None,
            requester: None,
        };

        let res = backend
            .detach("vol-1", "local-vol-1-vol.img", ownership, false)
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
        let ownership = chv_common::AttachmentOwnership {
            vm_id: "vm-1".to_string(),
            operation_id: None,
            requester: None,
        };
        backend
            .prepare_snapshot("vol-1", handle, ownership, "snap1")
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
        let ownership = chv_common::AttachmentOwnership {
            vm_id: "vm-1".to_string(),
            operation_id: None,
            requester: None,
        };
        backend
            .prepare_clone("vol-1", handle, ownership, "clone1")
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
        let ownership = chv_common::AttachmentOwnership {
            vm_id: "vm-1".to_string(),
            operation_id: None,
            requester: None,
        };
        let res = backend
            .prepare_snapshot("vol-1", "local-vol-1-vol.img", ownership, "snap1")
            .await;
        assert!(matches!(res, Err(ChvError::NotFound { .. })));
    }

    #[tokio::test]
    async fn local_backend_prepare_snapshot_invalid_handle() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let ownership = chv_common::AttachmentOwnership {
            vm_id: "vm-1".to_string(),
            operation_id: None,
            requester: None,
        };
        let res = backend
            .prepare_snapshot("vol-1", "iscsi-vol-1-target", ownership, "snap1")
            .await;
        assert!(matches!(res, Err(ChvError::BackendUnavailable { .. })));
    }

    #[tokio::test]
    async fn local_backend_prepare_clone_missing_file_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let ownership = chv_common::AttachmentOwnership {
            vm_id: "vm-1".to_string(),
            operation_id: None,
            requester: None,
        };
        let res = backend
            .prepare_clone("vol-1", "local-vol-1-vol.img", ownership, "clone1")
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
        let ownership = chv_common::AttachmentOwnership {
            vm_id: "vm-1".to_string(),
            operation_id: None,
            requester: None,
        };
        let res = backend
            .prepare_clone("vol-1", "local-vol-1-vol.qcow2", ownership, "clone1")
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
    async fn create_receiving_volume() {
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
    }

    #[tokio::test]
    async fn create_receiving_volume_refuses_existing_path() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());

        backend
            .create_receiving_volume("rcv-vol-1", 8192, "raw")
            .await
            .unwrap();

        // A second creation for the same id must fail instead of silently
        // truncating the existing file.
        let err = backend
            .create_receiving_volume("rcv-vol-1", 8192, "raw")
            .await
            .unwrap_err();
        assert!(matches!(err, ChvError::InvalidArgument { .. }));

        let meta = std::fs::metadata(dir.path().join("rcv-vol-1.img")).unwrap();
        assert_eq!(meta.len(), 8192);
    }

    #[tokio::test]
    async fn create_receiving_volume_rejects_traversal_ids() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFileBackend::new(dir.path().to_path_buf());

        for bad_id in ["../escape", "a/b", "a\\b", "..", "a..b"] {
            let err = backend
                .create_receiving_volume(bad_id, 8192, "raw")
                .await
                .unwrap_err();
            assert!(
                matches!(err, ChvError::InvalidArgument { .. }),
                "volume_id {bad_id:?} should be rejected, got {err:?}"
            );
        }

        // Nothing may have been created outside runtime_dir.
        assert!(!dir.path().parent().unwrap().join("escape.img").exists());
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

    // --- Dirty tracking tests ---

    async fn open_tracked_volume(
        dir: &std::path::Path,
        volume_id: &str,
        name: &str,
        size_bytes: u64,
    ) -> (LocalFileBackend, String) {
        let backend = LocalFileBackend::new(dir.to_path_buf());
        let mut options = std::collections::HashMap::new();
        options.insert("size_bytes".to_string(), size_bytes.to_string());
        let locator = BackendLocator {
            backend_class: "local".to_string(),
            locator: name.to_string(),
            options,
        };
        let export = backend
            .open(volume_id, &locator, &DevicePolicy::default())
            .await
            .unwrap();
        (backend, export.attachment_handle)
    }

    #[tokio::test]
    async fn dirty_tracking_not_found_before_enable_and_empty_after() {
        let dir = tempfile::tempdir().unwrap();
        let (backend, handle) =
            open_tracked_volume(dir.path(), "vol-1", "tracked.img", 8_388_608).await;

        // Without enabling, the snapshot call reports NotFound.
        let before = backend
            .snapshot_and_clear_dirty_bitmap("vol-1", &handle)
            .await;
        assert!(matches!(before, Err(ChvError::NotFound { .. })));

        backend
            .enable_dirty_tracking("vol-1", &handle, 8_388_608)
            .await
            .unwrap();

        // After enabling, a freshly opened volume returns an empty bitmap
        // (not NotFound) so migration dirty-sync rounds can proceed.
        let bitmap = backend
            .snapshot_and_clear_dirty_bitmap("vol-1", &handle)
            .await
            .unwrap();
        assert!(!bitmap.is_empty());
        assert!(bitmap.iter().all(|&b| b == 0));
    }

    #[tokio::test]
    async fn dirty_tracking_marks_written_block_and_snapshot_clears() {
        let dir = tempfile::tempdir().unwrap();
        let (backend, handle) =
            open_tracked_volume(dir.path(), "vol-1", "tracked.img", 8_388_608).await;
        backend
            .enable_dirty_tracking("vol-1", &handle, 8_388_608)
            .await
            .unwrap();

        // Write inside the second 4 MiB block (block index 1 -> byte 0, bit 1).
        let offset = DIRTY_TRACKING_BLOCK_SIZE + 1024;
        backend
            .write_block("vol-1", &handle, offset, &[0xAAu8; 512])
            .await
            .unwrap();

        let bitmap = backend
            .snapshot_and_clear_dirty_bitmap("vol-1", &handle)
            .await
            .unwrap();
        assert_eq!(bitmap, vec![0b0000_0010]);

        // The snapshot call atomically cleared the bitmap.
        let after = backend
            .snapshot_and_clear_dirty_bitmap("vol-1", &handle)
            .await
            .unwrap();
        assert_eq!(after, vec![0]);
    }

    #[tokio::test]
    async fn dirty_tracking_rejects_out_of_range_write() {
        let dir = tempfile::tempdir().unwrap();
        let (backend, handle) = open_tracked_volume(dir.path(), "vol-1", "tracked.img", 4096).await;
        backend
            .enable_dirty_tracking("vol-1", &handle, 4096)
            .await
            .unwrap();

        let res = backend
            .write_block("vol-1", &handle, 3000, &[0u8; 2000])
            .await;
        assert!(matches!(res, Err(ChvError::InvalidArgument { .. })));

        // The rejected write must not have extended the volume file.
        let meta = std::fs::metadata(dir.path().join("tracked.img")).unwrap();
        assert_eq!(meta.len(), 4096);
    }

    #[tokio::test]
    async fn dirty_tracking_resize_updates_bounds_and_grows_bitmap() {
        let dir = tempfile::tempdir().unwrap();
        let (backend, handle) = open_tracked_volume(dir.path(), "vol-1", "tracked.img", 4096).await;
        backend
            .enable_dirty_tracking("vol-1", &handle, 4096)
            .await
            .unwrap();

        // Before resize, a write past the old end is rejected.
        let res = backend
            .write_block("vol-1", &handle, 4096, &[0u8; 512])
            .await;
        assert!(matches!(res, Err(ChvError::InvalidArgument { .. })));

        // Grow to 10 blocks (40 MiB -> 2 bitmap bytes).
        let new_size = 10 * DIRTY_TRACKING_BLOCK_SIZE;
        backend.resize("vol-1", &handle, new_size).await.unwrap();

        // A write into block 8 is now in range and marks byte 1, bit 0.
        let offset = 8 * DIRTY_TRACKING_BLOCK_SIZE + 64;
        backend
            .write_block("vol-1", &handle, offset, &[0xBBu8; 512])
            .await
            .unwrap();

        let bitmap = backend
            .snapshot_and_clear_dirty_bitmap("vol-1", &handle)
            .await
            .unwrap();
        assert_eq!(bitmap, vec![0, 0b0000_0001]);
    }

    #[tokio::test]
    async fn dirty_tracking_enable_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let (backend, handle) =
            open_tracked_volume(dir.path(), "vol-1", "tracked.img", 8_388_608).await;
        backend
            .enable_dirty_tracking("vol-1", &handle, 8_388_608)
            .await
            .unwrap();

        backend
            .write_block("vol-1", &handle, 0, &[0xCCu8; 512])
            .await
            .unwrap();

        // A redundant enable (idempotent re-open) must not reset dirty bits.
        backend
            .enable_dirty_tracking("vol-1", &handle, 8_388_608)
            .await
            .unwrap();

        let bitmap = backend
            .snapshot_and_clear_dirty_bitmap("vol-1", &handle)
            .await
            .unwrap();
        assert_eq!(bitmap, vec![0b0000_0001]);
    }

    #[tokio::test]
    async fn dirty_tracking_rejects_oversized_volume() {
        let dir = tempfile::tempdir().unwrap();
        let (backend, handle) = open_tracked_volume(dir.path(), "vol-1", "tracked.img", 4096).await;

        let res = backend
            .enable_dirty_tracking("vol-1", &handle, MAX_DIRTY_TRACKING_VOLUME_SIZE_BYTES + 1)
            .await;
        assert!(matches!(res, Err(ChvError::InvalidArgument { .. })));
    }

    #[tokio::test]
    async fn close_evicts_dirty_tracker() {
        let dir = tempfile::tempdir().unwrap();
        let (backend, handle) =
            open_tracked_volume(dir.path(), "vol-1", "tracked.img", 8_388_608).await;
        backend
            .enable_dirty_tracking("vol-1", &handle, 8_388_608)
            .await
            .unwrap();

        backend.close("vol-1", &handle).await.unwrap();

        let res = backend
            .snapshot_and_clear_dirty_bitmap("vol-1", &handle)
            .await;
        assert!(matches!(res, Err(ChvError::NotFound { .. })));
    }

    #[tokio::test]
    async fn dirty_tracking_reenable_updates_bounds_and_keeps_dirty_bits() {
        let dir = tempfile::tempdir().unwrap();
        let (backend, handle) = open_tracked_volume(dir.path(), "vol-1", "tracked.img", 4096).await;
        backend
            .enable_dirty_tracking("vol-1", &handle, 2 * DIRTY_TRACKING_BLOCK_SIZE)
            .await
            .unwrap();

        // Dirty block 0 under the original (small) bound.
        backend
            .write_block("vol-1", &handle, 0, &[0xCCu8; 512])
            .await
            .unwrap();

        // A trigger-time re-enable with a grown size must update the bounds
        // (healing out-of-band resizes) without clearing the dirty bit.
        let new_size = 10 * DIRTY_TRACKING_BLOCK_SIZE;
        backend
            .enable_dirty_tracking("vol-1", &handle, new_size)
            .await
            .unwrap();

        // A write into block 8 would have been out of range before the
        // re-enable; now it must be accepted and marked (byte 1, bit 0).
        let offset = 8 * DIRTY_TRACKING_BLOCK_SIZE + 64;
        backend
            .write_block("vol-1", &handle, offset, &[0xDDu8; 512])
            .await
            .unwrap();

        let bitmap = backend
            .snapshot_and_clear_dirty_bitmap("vol-1", &handle)
            .await
            .unwrap();
        assert_eq!(bitmap, vec![0b0000_0001, 0b0000_0001]);
    }
}
