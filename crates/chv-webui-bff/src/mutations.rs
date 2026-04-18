use async_trait::async_trait;
use chv_webui_bff_api::chv_webui_bff_v1::{
    MutateNetworkResponse, MutateNodeResponse, MutateVmResponse, MutateVolumeResponse,
};

use crate::error::BffError;

#[async_trait]
pub trait MutationService: Send + Sync {
    async fn mutate_vm(
        &self,
        vm_id: String,
        action: String,
        force: bool,
        requested_by: String,
    ) -> Result<MutateVmResponse, BffError>;

    async fn mutate_node(
        &self,
        node_id: String,
        action: String,
        requested_by: String,
    ) -> Result<MutateNodeResponse, BffError>;

    async fn mutate_volume(
        &self,
        volume_id: String,
        action: String,
        force: bool,
        resize_bytes: Option<u64>,
        requested_by: String,
    ) -> Result<MutateVolumeResponse, BffError>;

    async fn mutate_network(
        &self,
        network_id: String,
        action: String,
        force: bool,
        requested_by: String,
    ) -> Result<MutateNetworkResponse, BffError>;
}
