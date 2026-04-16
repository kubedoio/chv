use async_trait::async_trait;
use chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse;

use crate::error::BffError;

#[async_trait]
pub trait MutationService: Send + Sync {
    async fn mutate_vm(
        &self,
        vm_id: String,
        action: String,
        force: bool,
    ) -> Result<MutateVmResponse, BffError>;
}
