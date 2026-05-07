use async_trait::async_trait;
use chv_common::types::{BackendLocator, DevicePolicy};
use chv_errors::ChvError;

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
        vm_id: &str,
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
        snapshot_name: &str,
    ) -> Result<(), ChvError>;

    async fn prepare_clone(
        &self,
        volume_id: &str,
        handle: &str,
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
}
