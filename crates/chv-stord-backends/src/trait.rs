use async_trait::async_trait;
use chv_common::types::{BackendLocator, DevicePolicy};
use chv_errors::ChvError;
use chv_common::AttachmentOwnership;

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

    async fn enable_dirty_tracking(
        &self,
        volume_id: &str,
        handle: &str,
        block_size: u64,
    ) -> Result<(), ChvError>;

    async fn get_dirty_bitmap(&self, volume_id: &str, handle: &str) -> Result<Vec<u8>, ChvError>;

    async fn clear_dirty_bitmap(&self, volume_id: &str, handle: &str) -> Result<(), ChvError>;

    async fn disable_dirty_tracking(&self, volume_id: &str, handle: &str) -> Result<(), ChvError>;

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

    async fn delete_volume(&self, volume_id: &str) -> Result<(), ChvError>;

    /// Atomically snapshot the dirty bitmap and clear it.
    ///
    /// Acquires a write lock, clones the bitmap, resets it to zeros, and returns
    /// the snapshot. This prevents any window where dirty writes could be lost
    /// between a separate get + clear sequence.
    ///
    /// Default implementation calls get_dirty_bitmap + clear_dirty_bitmap sequentially
    /// (not truly atomic, but backward-compatible for backends that haven't overridden).
    async fn snapshot_and_clear_dirty_bitmap(
        &self,
        volume_id: &str,
        handle: &str,
    ) -> Result<Vec<u8>, ChvError> {
        let bitmap = self.get_dirty_bitmap(volume_id, handle).await?;
        self.clear_dirty_bitmap(volume_id, handle).await?;
        Ok(bitmap)
    }

    /// Configure I/O rate limits for a volume.
    ///
    /// Backends that support IOPS/bandwidth enforcement (e.g., via cgroup v2)
    /// should override this method. The default is a no-op.
    async fn set_io_limits(
        &self,
        _volume_id: &str,
        _iops: Option<u64>,
        _bandwidth_mbps: Option<u64>,
    ) -> Result<(), ChvError> {
        Ok(())
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
        (**self).prepare_clone(volume_id, handle, ownership, clone_name).await
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

    async fn enable_dirty_tracking(
        &self,
        volume_id: &str,
        handle: &str,
        block_size: u64,
    ) -> Result<(), ChvError> {
        (**self)
            .enable_dirty_tracking(volume_id, handle, block_size)
            .await
    }

    async fn get_dirty_bitmap(&self, volume_id: &str, handle: &str) -> Result<Vec<u8>, ChvError> {
        (**self).get_dirty_bitmap(volume_id, handle).await
    }

    async fn clear_dirty_bitmap(&self, volume_id: &str, handle: &str) -> Result<(), ChvError> {
        (**self).clear_dirty_bitmap(volume_id, handle).await
    }

    async fn disable_dirty_tracking(&self, volume_id: &str, handle: &str) -> Result<(), ChvError> {
        (**self).disable_dirty_tracking(volume_id, handle).await
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

    async fn delete_volume(&self, volume_id: &str) -> Result<(), ChvError> {
        (**self).delete_volume(volume_id).await
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

    async fn set_io_limits(
        &self,
        volume_id: &str,
        iops: Option<u64>,
        bandwidth_mbps: Option<u64>,
    ) -> Result<(), ChvError> {
        (**self)
            .set_io_limits(volume_id, iops, bandwidth_mbps)
            .await
    }
}
