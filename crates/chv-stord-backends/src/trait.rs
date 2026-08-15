use async_trait::async_trait;
use chv_common::types::{BackendLocator, DevicePolicy};
use chv_common::AttachmentOwnership;
use chv_errors::ChvError;

/// Block size used for dirty-bitmap tracking: one bit per block.
///
/// Must match the block size the migration sender uses (its
/// `DEFAULT_BLOCK_SIZE`) so that bitmap bits map 1:1 to migration chunks.
pub const DIRTY_TRACKING_BLOCK_SIZE: u64 = 4 * 1024 * 1024; // 4 MiB

/// Maximum volume size accepted for dirty-bitmap tracking (16 TiB).
///
/// The bitmap stores one bit per [`DIRTY_TRACKING_BLOCK_SIZE`] block, so
/// capping the volume size bounds the bitmap allocation: 16 TiB needs at
/// most 512 KiB of bitmap. Larger volumes are rejected with
/// `InvalidArgument` instead of allocating an unbounded bitmap.
pub const MAX_DIRTY_TRACKING_VOLUME_SIZE_BYTES: u64 = 16 * 1024 * 1024 * 1024 * 1024;

/// Validate a block-write range against the volume size.
///
/// Returns the exclusive end offset (`offset + data_len`) on success. The
/// dirty bitmap is sized from the volume size, so `write_block`
/// implementations use this check to guarantee that a bitmap update can
/// never run past the end of the bitmap.
pub(crate) fn validate_write_bounds(
    offset: u64,
    data_len: u64,
    volume_size: u64,
) -> Result<u64, ChvError> {
    let end = offset
        .checked_add(data_len)
        .ok_or_else(|| ChvError::InvalidArgument {
            field: "offset".to_string(),
            reason: format!(
                "write range offset {} + length {} overflows u64",
                offset, data_len
            ),
        })?;
    if end > volume_size {
        return Err(ChvError::InvalidArgument {
            field: "write_block".to_string(),
            reason: format!(
                "write range {}..{} exceeds volume size {} bytes",
                offset, end, volume_size
            ),
        });
    }
    Ok(end)
}

#[derive(Debug, Clone)]
pub struct VolumeExport {
    pub export_kind: String,
    pub export_path: String,
    pub attachment_handle: String,
}

#[derive(Debug, Clone)]
pub struct BackendHealth {
    pub status: String,
    pub backend_state: String,
    pub last_error: String,
}

#[async_trait]
pub trait StorageBackend: Send + Sync + 'static {
    async fn open(
        &self,
        volume_id: &str,
        locator: &BackendLocator,
        policy: &DevicePolicy,
    ) -> Result<VolumeExport, ChvError>;

    async fn close(&self, volume_id: &str, handle: &str) -> Result<(), ChvError>;

    async fn attach(
        &self,
        volume_id: &str,
        handle: &str,
        vm_id: &str,
    ) -> Result<VolumeExport, ChvError>;

    async fn detach(
        &self,
        volume_id: &str,
        handle: &str,
        ownership: AttachmentOwnership,
        force: bool,
    ) -> Result<(), ChvError>;

    async fn health(&self, volume_id: &str, handle: &str) -> Result<BackendHealth, ChvError>;

    async fn resize(
        &self,
        volume_id: &str,
        handle: &str,
        new_size_bytes: u64,
    ) -> Result<(), ChvError>;

    async fn prepare_snapshot(
        &self,
        volume_id: &str,
        handle: &str,
        ownership: AttachmentOwnership,
        snapshot_name: &str,
    ) -> Result<(), ChvError>;

    async fn prepare_clone(
        &self,
        volume_id: &str,
        handle: &str,
        ownership: AttachmentOwnership,
        clone_name: &str,
    ) -> Result<(), ChvError>;

    async fn restore_snapshot(
        &self,
        volume_id: &str,
        handle: &str,
        snapshot_name: &str,
    ) -> Result<(), ChvError>;

    async fn delete_snapshot(
        &self,
        volume_id: &str,
        handle: &str,
        snapshot_name: &str,
    ) -> Result<(), ChvError>;

    async fn set_device_policy(
        &self,
        volume_id: &str,
        handle: &str,
        policy: &DevicePolicy,
    ) -> Result<(), ChvError>;

    // --- Phase 2.1-2.2: Migration methods ---

    async fn read_block(
        &self,
        volume_id: &str,
        handle: &str,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, ChvError>;

    async fn write_block(
        &self,
        volume_id: &str,
        handle: &str,
        offset: u64,
        data: &[u8],
    ) -> Result<(), ChvError>;

    async fn volume_size(&self, volume_id: &str, handle: &str) -> Result<u64, ChvError>;

    async fn create_receiving_volume(
        &self,
        volume_id: &str,
        size_bytes: u64,
        format: &str,
    ) -> Result<VolumeExport, ChvError>;

    /// Initialize dirty-bitmap tracking for a volume that has been opened.
    ///
    /// The bitmap is sized from `volume_size_bytes` with one bit per
    /// [`DIRTY_TRACKING_BLOCK_SIZE`] block and starts all-clear. chv-stord
    /// calls this when a volume session is opened so that migration
    /// dirty-sync rounds can always snapshot a bitmap for an open volume
    /// (an empty one when nothing was written yet).
    ///
    /// The default implementation is a no-op: backends without dirty
    /// tracking accept the call and simply don't track writes.
    async fn enable_dirty_tracking(
        &self,
        _volume_id: &str,
        _handle: &str,
        _volume_size_bytes: u64,
    ) -> Result<(), ChvError> {
        Ok(())
    }

    /// Atomically snapshot the dirty bitmap and clear it.
    ///
    /// Acquires a write lock, clones the bitmap, resets it to zeros, and returns
    /// the snapshot. This prevents any window where dirty writes could be lost
    /// between a separate get + clear sequence.
    ///
    /// The default implementation returns `NotFound`: backends that support
    /// dirty tracking must override it.
    async fn snapshot_and_clear_dirty_bitmap(
        &self,
        _volume_id: &str,
        handle: &str,
    ) -> Result<Vec<u8>, ChvError> {
        Err(ChvError::NotFound {
            resource: "dirty_tracker".to_string(),
            id: handle.to_string(),
        })
    }
}

/// Blanket implementation allowing `Box<dyn StorageBackend>` to be used as a
/// `StorageBackend`. This enables runtime backend selection in chv-stord.
#[async_trait]
impl StorageBackend for Box<dyn StorageBackend> {
    async fn open(
        &self,
        volume_id: &str,
        locator: &BackendLocator,
        policy: &DevicePolicy,
    ) -> Result<VolumeExport, ChvError> {
        (**self).open(volume_id, locator, policy).await
    }

    async fn close(&self, volume_id: &str, handle: &str) -> Result<(), ChvError> {
        (**self).close(volume_id, handle).await
    }

    async fn attach(
        &self,
        volume_id: &str,
        handle: &str,
        vm_id: &str,
    ) -> Result<VolumeExport, ChvError> {
        (**self).attach(volume_id, handle, vm_id).await
    }

    async fn detach(
        &self,
        volume_id: &str,
        handle: &str,
        ownership: AttachmentOwnership,
        force: bool,
    ) -> Result<(), ChvError> {
        (**self).detach(volume_id, handle, ownership, force).await
    }

    async fn health(&self, volume_id: &str, handle: &str) -> Result<BackendHealth, ChvError> {
        (**self).health(volume_id, handle).await
    }

    async fn resize(
        &self,
        volume_id: &str,
        handle: &str,
        new_size_bytes: u64,
    ) -> Result<(), ChvError> {
        (**self).resize(volume_id, handle, new_size_bytes).await
    }

    async fn prepare_snapshot(
        &self,
        volume_id: &str,
        handle: &str,
        ownership: AttachmentOwnership,
        snapshot_name: &str,
    ) -> Result<(), ChvError> {
        (**self)
            .prepare_snapshot(volume_id, handle, ownership, snapshot_name)
            .await
    }

    async fn prepare_clone(
        &self,
        volume_id: &str,
        handle: &str,
        ownership: AttachmentOwnership,
        clone_name: &str,
    ) -> Result<(), ChvError> {
        (**self)
            .prepare_clone(volume_id, handle, ownership, clone_name)
            .await
    }

    async fn restore_snapshot(
        &self,
        volume_id: &str,
        handle: &str,
        snapshot_name: &str,
    ) -> Result<(), ChvError> {
        (**self)
            .restore_snapshot(volume_id, handle, snapshot_name)
            .await
    }

    async fn delete_snapshot(
        &self,
        volume_id: &str,
        handle: &str,
        snapshot_name: &str,
    ) -> Result<(), ChvError> {
        (**self)
            .delete_snapshot(volume_id, handle, snapshot_name)
            .await
    }

    async fn set_device_policy(
        &self,
        volume_id: &str,
        handle: &str,
        policy: &DevicePolicy,
    ) -> Result<(), ChvError> {
        (**self).set_device_policy(volume_id, handle, policy).await
    }

    async fn read_block(
        &self,
        volume_id: &str,
        handle: &str,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, ChvError> {
        (**self).read_block(volume_id, handle, offset, length).await
    }

    async fn write_block(
        &self,
        volume_id: &str,
        handle: &str,
        offset: u64,
        data: &[u8],
    ) -> Result<(), ChvError> {
        (**self).write_block(volume_id, handle, offset, data).await
    }

    async fn volume_size(&self, volume_id: &str, handle: &str) -> Result<u64, ChvError> {
        (**self).volume_size(volume_id, handle).await
    }

    async fn create_receiving_volume(
        &self,
        volume_id: &str,
        size_bytes: u64,
        format: &str,
    ) -> Result<VolumeExport, ChvError> {
        (**self)
            .create_receiving_volume(volume_id, size_bytes, format)
            .await
    }

    async fn enable_dirty_tracking(
        &self,
        volume_id: &str,
        handle: &str,
        volume_size_bytes: u64,
    ) -> Result<(), ChvError> {
        (**self)
            .enable_dirty_tracking(volume_id, handle, volume_size_bytes)
            .await
    }

    async fn snapshot_and_clear_dirty_bitmap(
        &self,
        volume_id: &str,
        handle: &str,
    ) -> Result<Vec<u8>, ChvError> {
        (**self)
            .snapshot_and_clear_dirty_bitmap(volume_id, handle)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_write_bounds_accepts_in_range_writes() {
        // A write ending exactly at the volume size is in range.
        assert_eq!(validate_write_bounds(3, 7, 10).unwrap(), 10);
        assert_eq!(validate_write_bounds(0, 0, 0).unwrap(), 0);
        assert_eq!(validate_write_bounds(0, 0, 4096).unwrap(), 0);
    }

    #[test]
    fn validate_write_bounds_rejects_out_of_range_writes() {
        assert!(matches!(
            validate_write_bounds(0, 100, 99),
            Err(ChvError::InvalidArgument { .. })
        ));
        assert!(matches!(
            validate_write_bounds(50, 51, 100),
            Err(ChvError::InvalidArgument { .. })
        ));
    }

    #[test]
    fn validate_write_bounds_rejects_offset_overflow() {
        assert!(matches!(
            validate_write_bounds(u64::MAX, 1, u64::MAX),
            Err(ChvError::InvalidArgument { .. })
        ));
    }
}
