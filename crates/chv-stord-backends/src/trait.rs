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

    async fn set_device_policy(
        &self,
        volume_id: &str,
        handle: &str,
        policy: &DevicePolicy,
    ) -> Result<(), ChvError>;
}
