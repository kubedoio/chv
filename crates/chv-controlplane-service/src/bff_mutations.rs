use async_trait::async_trait;
use chv_webui_bff::{BffError, MutationService};
use chv_webui_bff_api::chv_webui_bff_v1::MutateVmResponse;

#[derive(Clone)]
pub struct ControlPlaneMutationService;

impl ControlPlaneMutationService {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl MutationService for ControlPlaneMutationService {
    async fn mutate_vm(
        &self,
        _vm_id: String,
        _action: String,
        _force: bool,
    ) -> Result<MutateVmResponse, BffError> {
        Err(BffError::NotImplemented)
    }
}
